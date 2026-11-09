//! GameScape desktop application core.
//!
//! Command surface for the React UI: settings, storage health and the
//! catalogue index. All data is local; nothing here uses the network.

mod artwork;
mod catalogue;
mod desktop_host;
mod layouts;
mod library;
mod storage;
mod system_info;

#[cfg(windows)]
mod spike_s1;

use tauri::{Emitter, Manager};

use catalogue::CatalogueIndex;
use storage::{AppState, Settings};

/* ------------------------------------------------------------------ */
/* Commands                                                            */
/* ------------------------------------------------------------------ */

#[tauri::command]
fn settings_get(state: tauri::State<'_, AppState>) -> Settings {
    state
        .settings
        .lock()
        .map(|settings| settings.clone())
        .unwrap_or_default()
}

#[tauri::command]
fn settings_set(state: tauri::State<'_, AppState>, settings: Settings) -> Result<(), String> {
    storage::save_settings(&settings)?;
    let mut guard = state
        .settings
        .lock()
        .map_err(|e| format!("settings lock poisoned: {e}"))?;
    *guard = settings;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DbHealth {
    ok: bool,
    schema_version: i64,
    media_count: i64,
    catalogue_count: i64,
}

#[tauri::command]
fn db_health(state: tauri::State<'_, AppState>) -> Result<DbHealth, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;

    let schema_version = storage::schema_version(&conn)?;
    let media_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM media", [], |row| row.get(0))
        .map_err(|e| format!("cannot count media: {e}"))?;
    let catalogue_count: i64 = conn
        .query_row(
            "SELECT COALESCE((SELECT entry_count FROM catalogue_cache WHERE id = 1), 0)",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("cannot read catalogue cache: {e}"))?;

    Ok(DbHealth {
        ok: true,
        schema_version,
        media_count,
        catalogue_count,
    })
}

#[tauri::command]
fn catalogue_load(state: tauri::State<'_, AppState>) -> Result<CatalogueIndex, String> {
    let index = catalogue::load_seed()?;

    if let Ok(conn) = state.db.lock() {
        let _ = conn.execute(
            "INSERT INTO catalogue_cache (id, source, version, entry_count, loaded_at)
             VALUES (1, 'bundled_seed', ?1, ?2, datetime('now'))
             ON CONFLICT(id) DO UPDATE SET
               source = excluded.source,
               version = excluded.version,
               entry_count = excluded.entry_count,
               loaded_at = excluded.loaded_at",
            rusqlite::params![index.schema_version, index.entries.len() as i64],
        );
    }

    Ok(index)
}

#[tauri::command]
async fn import_paths(
    app: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<library::ImportSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut progress = |index: usize, total: usize, item: &library::ImportItemResult| {
            let _ = app.emit(
                "imports:progress",
                serde_json::json!({
                    "index": index,
                    "total": total,
                    "fileName": item.file_name,
                    "status": item.status,
                    "detail": item.detail,
                }),
            );
        };
        library::import_files(state.inner(), &paths, &mut progress)
    })
    .await
    .map_err(|error| format!("import task failed: {error}"))?
}

#[tauri::command]
fn library_list(state: tauri::State<'_, AppState>) -> Result<Vec<library::LibraryItem>, String> {
    library::list_library(state.inner())
}

#[tauri::command]
fn library_remove(
    state: tauri::State<'_, AppState>,
    id: i64,
    delete_files: bool,
) -> Result<(), String> {
    library::remove_from_library(state.inner(), id, delete_files)
}

#[tauri::command]
fn library_update(
    state: tauri::State<'_, AppState>,
    id: i64,
    title: String,
    description: String,
    tags: Vec<String>,
) -> Result<(), String> {
    library::update_metadata(state.inner(), id, &title, &description, &tags)
}

#[tauri::command]
fn collections_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<library::CollectionSummary>, String> {
    library::collections_list(state.inner())
}

#[tauri::command]
fn collection_create(state: tauri::State<'_, AppState>, name: String) -> Result<i64, String> {
    library::collection_create(state.inner(), &name)
}

#[tauri::command]
fn collection_delete(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    library::collection_delete(state.inner(), id)
}

#[tauri::command]
fn collection_set_membership(
    state: tauri::State<'_, AppState>,
    collection_id: i64,
    media_id: i64,
    member: bool,
) -> Result<(), String> {
    library::collection_set_membership(state.inner(), collection_id, media_id, member)
}

#[tauri::command]
fn collection_memberships(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<library::CollectionMembership>, String> {
    library::collection_memberships(state.inner())
}

#[tauri::command]
fn library_check(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<library::IntegrityIssue>, String> {
    library::check_integrity(state.inner())
}

#[tauri::command]
fn library_relink(
    state: tauri::State<'_, AppState>,
    id: i64,
    new_path: String,
) -> Result<(), String> {
    library::relink(state.inner(), id, &new_path)
}

#[tauri::command]
fn library_reveal(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    library::reveal_item(state.inner(), id)
}

#[tauri::command]
fn library_open_dir(state: tauri::State<'_, AppState>) -> Result<(), String> {
    library::open_library_dir(state.inner())
}

#[tauri::command]
fn library_info(state: tauri::State<'_, AppState>) -> Result<library::LibraryInfo, String> {
    library::library_info(state.inner())
}

#[tauri::command]
fn library_set_poster(
    state: tauri::State<'_, AppState>,
    id: i64,
    data_url: String,
) -> Result<(), String> {
    library::set_poster(state.inner(), id, &data_url)
}

#[tauri::command]
fn library_read_data(state: tauri::State<'_, AppState>, id: i64) -> Result<String, String> {
    library::read_file_data_url(state.inner(), id)
}

// Desktop-host commands run off the main thread (`async`): runtime window
// creation must be dispatched to the event loop from a separate thread
// (creating a WebView2 while nested inside an IPC callback deadlocks the
// main thread), and attach/apply work blocks briefly.
#[tauri::command]
async fn host_apply_media(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    host: tauri::State<'_, desktop_host::HostState>,
    media_id: i64,
    monitor: Option<u32>,
) -> Result<String, String> {
    desktop_host::apply_library_item(&app, state.inner(), host.inner(), media_id, monitor)
}

/// Apply a cursor-reactive catalogue wallpaper (renders live on the
/// desktop and follows the mouse through the cursor bridge).
#[tauri::command]
async fn host_apply_interactive(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    host: tauri::State<'_, desktop_host::HostState>,
    media_id: i64,
    effect: String,
    dominant: String,
    accent: String,
) -> Result<(), String> {
    desktop_host::apply_interactive(
        &app,
        state.inner(),
        host.inner(),
        media_id,
        &effect,
        &dominant,
        &accent,
    )
}

#[tauri::command]
async fn host_stop(
    app: tauri::AppHandle,
    host: tauri::State<'_, desktop_host::HostState>,
) -> Result<(), String> {
    desktop_host::stop_all(&app, host.inner())
}

#[tauri::command]
fn host_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    host: tauri::State<'_, desktop_host::HostState>,
) -> desktop_host::HostReport {
    desktop_host::report(&app, host.inner(), state.inner())
}

/// Turn wallpapers off entirely: stop animation, put the previous
/// wallpaper back, and stay off on next launch until re-applied.
#[tauri::command]
async fn host_turn_off(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    host: tauri::State<'_, desktop_host::HostState>,
) -> Result<(), String> {
    desktop_host::turn_off(&app, state.inner(), host.inner())
}

#[tauri::command]
async fn host_set_paused(
    app: tauri::AppHandle,
    host: tauri::State<'_, desktop_host::HostState>,
    paused: bool,
) -> Result<(), String> {
    desktop_host::set_paused(&app, host.inner(), paused)
}

#[tauri::command]
async fn host_restore_previous(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    host: tauri::State<'_, desktop_host::HostState>,
) -> Result<(), String> {
    // Restoring means undoing everything GameScape did: stop animated
    // surfaces first, then put the previous (static) wallpapers back.
    desktop_host::stop_all(&app, host.inner())?;
    desktop_host::restore_previous(state.inner())
}

/* ------------------------------------------------------------------ */
/* Catalogue generation                                                */
/* ------------------------------------------------------------------ */

/// Renders a catalogue entry into a real wallpaper, stores it in the
/// library (source `public_catalogue`) and returns the media id. Repeated
/// calls are idempotent — the same file and row are reused.
#[tauri::command]
async fn catalogue_generate(
    state: tauri::State<'_, AppState>,
    catalogue_id: String,
) -> Result<i64, String> {
    // Already generated (or imported)? Reuse it.
    {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("database lock poisoned: {e}"))?;
        let known: Option<(i64, Option<String>)> = conn
            .query_row(
                "SELECT id, path FROM media WHERE catalogue_id = ?1 LIMIT 1",
                [&catalogue_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        if let Some((id, path)) = known {
            if let Some(path) = path {
                if std::path::Path::new(&path).exists() {
                    return Ok(id);
                }
            }
        }
    }

    let index = catalogue::load_seed()?;
    let entry = index
        .entries
        .iter()
        .find(|entry| entry.id == catalogue_id)
        .ok_or_else(|| "this catalogue entry does not exist".to_string())?;

    let rendered = artwork::render_entry(entry, 1920, 1080);
    let mut bytes: Vec<u8> = Vec::new();
    image::DynamicImage::ImageRgba8(rendered)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .map_err(|error| format!("cannot encode the wallpaper: {error}"))?;

    let hash = library::sha256_hex(&bytes);
    let library_dir = {
        let settings = state
            .settings
            .lock()
            .map_err(|e| format!("settings lock poisoned: {e}"))?;
        storage::effective_library_dir(&settings)?
    };
    std::fs::create_dir_all(&library_dir)
        .map_err(|error| format!("cannot create {}: {error}", library_dir.display()))?;
    let target = library_dir.join(format!("{}.png", &hash[..12]));

    let id = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("database lock poisoned: {e}"))?;
        // Identical bytes already present? Reuse that row.
        if let Ok(existing) =
            conn.query_row("SELECT id FROM media WHERE sha256 = ?1", [&hash], |row| {
                row.get::<_, i64>(0)
            })
        {
            return Ok(existing);
        }
        std::fs::write(&target, &bytes)
            .map_err(|error| format!("cannot store the wallpaper: {error}"))?;
        conn.execute(
            "INSERT INTO media
               (source, catalogue_id, path, managed, title, description, game, tags,
                format, width, height, bytes, sha256, licence_id, status)
             VALUES ('public_catalogue', ?1, ?2, 1, ?3, ?4, ?5, ?6, 'png', 1920, 1080, ?7, ?8, ?9, 'ok')",
            rusqlite::params![
                catalogue_id,
                target.to_string_lossy().to_string(),
                entry.title,
                entry.description,
                entry.game,
                serde_json::to_string(&entry.tags).unwrap_or_else(|_| "[]".to_string()),
                bytes.len() as i64,
                hash,
                entry.licence_id,
            ],
        )
        .map_err(|error| format!("cannot add the wallpaper to your library: {error}"))?;
        conn.last_insert_rowid()
    };

    desktop_host::log_line(&format!(
        "catalogue generated: {} (#{id})",
        entry.title
    ));
    Ok(id)
}

/* ------------------------------------------------------------------ */
/* Widgets                                                             */
/* ------------------------------------------------------------------ */

#[tauri::command]
fn widgets_list(state: tauri::State<'_, AppState>) -> Result<Vec<storage::WidgetRow>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    storage::list_widgets(&conn)
}

// Widget commands are async: they create/destroy desktop-layer windows,
// which must be dispatched from a worker thread (same rule as surfaces).
#[tauri::command]
async fn widget_create(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    host: tauri::State<'_, desktop_host::HostState>,
    kind: String,
    monitor: Option<u32>,
) -> Result<i64, String> {
    if !matches!(
        kind.as_str(),
        "clock" | "system" | "wallpaper" | "calendar" | "countdown" | "text" | "launcher"
    ) {
        return Err(format!("unknown widget kind: {kind}"));
    }
    let _serial = host.serial_lock()?;
    let (id, rows) = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("database lock poisoned: {e}"))?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM widgets", [], |row| row.get(0))
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO widgets (kind, monitor, x, y) VALUES (?1, ?2, 40, ?3)",
            rusqlite::params![kind, monitor.unwrap_or(0), 40 + (count % 6) * 60],
        )
        .map_err(|e| format!("cannot create the widget: {e}"))?;
        let id = conn.last_insert_rowid();
        let rows = storage::list_widgets(&conn)?;
        (id, rows)
    };
    desktop_host::sync_widgets_rows(&app, host.inner(), &rows)?;
    Ok(id)
}

#[tauri::command]
async fn widget_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    host: tauri::State<'_, desktop_host::HostState>,
    id: i64,
    enabled: bool,
    monitor: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    settings: Option<String>,
) -> Result<(), String> {
    let width = width.clamp(120, 2000);
    let height = height.clamp(80, 1200);
    if let Some(raw) = &settings {
        if raw.len() > 4096 {
            desktop_host::log_line(&format!("widget update: id={id} rejected (settings too large)"));
            return Err("widget settings are too large".to_string());
        }
        // Settings ride to the widget page as JSON; reject anything broken.
        if let Err(error) = serde_json::from_str::<serde_json::Value>(raw) {
            desktop_host::log_line(&format!("widget update: id={id} rejected: {error}"));
            return Err(format!("widget settings must be valid JSON: {error}"));
        }
    }
    let _serial = host.serial_lock()?;
    let rows = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("database lock poisoned: {e}"))?;
        conn.execute(
            "UPDATE widgets
             SET enabled = ?1, monitor = ?2, x = ?3, y = ?4, width = ?5, height = ?6,
                 settings = ?7
             WHERE id = ?8",
            rusqlite::params![enabled as i64, monitor, x, y, width, height, settings, id],
        )
        .map_err(|e| format!("cannot update the widget: {e}"))?;
        storage::list_widgets(&conn)?
    };
    let outcome = desktop_host::sync_widgets_rows(&app, host.inner(), &rows);
    match &outcome {
        Ok(count) => desktop_host::log_line(&format!(
            "widget update: id={id} applied (widgets on desktop: {count})"
        )),
        Err(error) => desktop_host::log_line(&format!("widget update: id={id} failed: {error}")),
    }
    outcome.map(|_| ())
}

#[tauri::command]
async fn widget_remove(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    host: tauri::State<'_, desktop_host::HostState>,
    id: i64,
) -> Result<(), String> {
    let _serial = host.serial_lock()?;
    let rows = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("database lock poisoned: {e}"))?;
        conn.execute("DELETE FROM widgets WHERE id = ?1", [id])
            .map_err(|e| format!("cannot remove the widget: {e}"))?;
        storage::list_widgets(&conn)?
    };
    desktop_host::sync_widgets_rows(&app, host.inner(), &rows)?;
    Ok(())
}

#[tauri::command]
async fn widgets_sync(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    host: tauri::State<'_, desktop_host::HostState>,
) -> Result<usize, String> {
    desktop_host::sync_widgets(&app, host.inner(), state.inner())
}

/// Dev-only harness helper (debug builds only): waits until the widget is
/// visible on the desktop, then drives the drag code path with no
/// synthesized input. Does nothing in release builds.
#[tauri::command]
fn dev_widget_drag_when_ready(
    app: tauri::AppHandle,
    id: i64,
    dx: i32,
    dy: i32,
    timeout_secs: u64,
) {
    #[cfg(not(debug_assertions))]
    let _ = (&app, id, dx, dy, timeout_secs);

    #[cfg(debug_assertions)]
    std::thread::spawn(move || {
        desktop_host::dev_drag_when_ready(&app, id, dx, dy, timeout_secs);
    });
}

/* ------------------------------------------------------------------ */
/* Layout Editor                                                       */
/* ------------------------------------------------------------------ */

#[tauri::command]
fn layouts_list(state: tauri::State<'_, AppState>) -> Result<Vec<layouts::LayoutRow>, String> {
    layouts::list(state.inner())
}

/// Saves the current desktop as a layout (`id` absent) or refreshes an
/// existing one in place.
#[tauri::command]
fn layout_save(
    state: tauri::State<'_, AppState>,
    id: Option<i64>,
    name: String,
) -> Result<i64, String> {
    layouts::save(state.inner(), id, name)
}

#[tauri::command]
fn layout_rename(
    state: tauri::State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<(), String> {
    layouts::rename(state.inner(), id, name)
}

#[tauri::command]
fn layout_duplicate(state: tauri::State<'_, AppState>, id: i64) -> Result<i64, String> {
    layouts::duplicate(state.inner(), id)
}

#[tauri::command]
fn layout_remove(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    layouts::remove(state.inner(), id)
}

/// Applies a layout: appearance, widgets and wallpaper in one click.
#[tauri::command]
async fn layout_apply(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    host: tauri::State<'_, desktop_host::HostState>,
    id: i64,
) -> Result<String, String> {
    layouts::apply(&app, state.inner(), host.inner(), id)
}

#[tauri::command]
fn system_stats() -> Result<system_info::SystemStats, String> {
    system_info::sample()
}

/// Opens a link or file from a widget (e.g. Launcher buttons). The target
/// comes from the user's own widget settings.
#[tauri::command]
fn widget_open(target: String) -> Result<(), String> {
    let target = target.trim().to_string();
    if target.is_empty() || target.len() > 500 {
        return Err("invalid link target".to_string());
    }
    desktop_host::log_line(&format!("widget open: {target}"));
    let result = if target.starts_with("http://") || target.starts_with("https://") {
        tauri_plugin_opener::open_url(target.as_str(), None::<&str>)
    } else {
        tauri_plugin_opener::open_path(target.as_str(), None::<&str>)
    };
    result.map_err(|error| {
        desktop_host::log_line(&format!("widget open failed: {error}"));
        error.to_string()
    })
}

#[tauri::command]
fn favourites_list(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    let mut statement = conn
        .prepare("SELECT catalogue_id FROM favourites ORDER BY added_at DESC")
        .map_err(|e| format!("cannot query favourites: {e}"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("cannot query favourites: {e}"))?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row.map_err(|e| format!("cannot read a favourite row: {e}"))?);
    }
    Ok(ids)
}

#[tauri::command]
fn favourite_set(
    state: tauri::State<'_, AppState>,
    catalogue_id: String,
    favourite: bool,
) -> Result<(), String> {
    let valid = !catalogue_id.is_empty()
        && catalogue_id.len() <= 120
        && catalogue_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !valid {
        return Err("invalid catalogue id".to_string());
    }

    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    if favourite {
        conn.execute(
            "INSERT OR IGNORE INTO favourites (catalogue_id) VALUES (?1)",
            [&catalogue_id],
        )
        .map_err(|e| format!("cannot save favourite: {e}"))?;
    } else {
        conn.execute(
            "DELETE FROM favourites WHERE catalogue_id = ?1",
            [&catalogue_id],
        )
        .map_err(|e| format!("cannot remove favourite: {e}"))?;
    }
    Ok(())
}

/// Dev-only automation hooks used by local harnesses (never set in normal
/// use): `GAMESCAPE_DEV_IMPORT` imports a `;`-separated list at startup,
/// `GAMESCAPE_DEV_FAVOURITE` seeds favourites, `GAMESCAPE_START_HASH`
/// deep-links the main window after load, and `GAMESCAPE_DEV_SCRIPT` runs a
/// single verification script once the window has settled.
fn dev_automation(handle: tauri::AppHandle) {
    use std::io::Write as _;

    if let Ok(spec) = std::env::var("GAMESCAPE_DEV_IMPORT") {
        let paths: Vec<String> = spec
            .split(';')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect();
        if !paths.is_empty() {
            let handle = handle.clone();
            std::thread::spawn(move || {
                let state = handle.state::<AppState>();
                let mut progress =
                    |index: usize, total: usize, item: &library::ImportItemResult| {
                        let _ = handle.emit(
                            "imports:progress",
                            serde_json::json!({
                                "index": index,
                                "total": total,
                                "fileName": item.file_name,
                                "status": item.status,
                                "detail": item.detail,
                            }),
                        );
                    };
                match library::import_files(state.inner(), &paths, &mut progress) {
                    Ok(summary) => {
                        for item in &summary.items {
                            println!(
                                "[dev-import] {} -> {}{}",
                                item.file_name,
                                item.status,
                                item.detail
                                    .as_deref()
                                    .map(|d| format!(" ({d})"))
                                    .unwrap_or_default()
                            );
                        }
                        let line = format!(
                            "IMPORT_DONE imported={} duplicates={} failed={}",
                            summary.imported, summary.duplicates, summary.failed
                        );
                        println!("[dev-import] {line}");
                        if let Ok(path) = std::env::var("GAMESCAPE_SPIKE_OUT") {
                            if let Ok(mut file) = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(path)
                            {
                                let _ = writeln!(file, "{line}");
                            }
                        }
                    }
                    Err(error) => println!("[dev-import] FAILED: {error}"),
                }
            });
        }
    }

    if let Ok(spec) = std::env::var("GAMESCAPE_DEV_FAVOURITE") {
        let ids: Vec<String> = spec
            .split(';')
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect();
        if !ids.is_empty() {
            let state = handle.state::<AppState>();
            if let Ok(conn) = state.db.lock() {
                let mut count = 0usize;
                for id in &ids {
                    let valid = id
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
                    if valid
                        && conn
                            .execute(
                                "INSERT OR IGNORE INTO favourites (catalogue_id) VALUES (?1)",
                                [id],
                            )
                            .is_ok()
                    {
                        count += 1;
                    }
                }
                let line = format!("FAVOURITES_SEEDED {count}");
                println!("[dev-fav] {line}");
                if let Ok(path) = std::env::var("GAMESCAPE_SPIKE_OUT") {
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                    {
                        let _ = writeln!(file, "{line}");
                    }
                }
            }
        }
    }

    if let Ok(hash) = std::env::var("GAMESCAPE_START_HASH") {
        let safe: String = hash
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_'))
            .collect();
        if !safe.is_empty() {
            let thread_handle = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(6));
                if let Some(window) = thread_handle.get_webview_window("main") {
                    let _ = window.eval(&format!("window.location.hash = '#/{safe}';"));
                }
            });
        }
    }

    // Arbitrary dev script (env-gated, for local verification harnesses only):
    // runs once after the window has settled.
    if let Ok(script) = std::env::var("GAMESCAPE_DEV_SCRIPT") {
        if !script.is_empty() {
            let script_handle = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(8));
                if let Some(window) = script_handle.get_webview_window("main") {
                    let _ = window.eval(&script);
                }
                println!("[dev-script] DEV_SCRIPT_RUN");
                if let Ok(path) = std::env::var("GAMESCAPE_SPIKE_OUT") {
                    use std::io::Write as _;
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                    {
                        let _ = writeln!(file, "DEV_SCRIPT_RUN");
                    }
                }
            });
        }
    }
}

/* ------------------------------------------------------------------ */
/* System tray                                                         */
/* ------------------------------------------------------------------ */

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(desktop)]
fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let open_item = MenuItem::with_id(app, "open", "Open GameScape", true, None::<&str>)?;
    let turn_off_item =
        MenuItem::with_id(app, "turn_off", "Turn off desktop wallpaper", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit GameScape", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &turn_off_item, &separator, &quit_item])?;

    let mut builder = TrayIconBuilder::with_id("gamescape-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("GameScape — desktop wallpapers")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "turn_off" => {
                let state = app.state::<AppState>();
                let host = app.state::<desktop_host::HostState>();
                if let Err(error) = desktop_host::turn_off(app, state.inner(), host.inner()) {
                    desktop_host::log_line(&format!("tray: turn off failed: {error}"));
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        show_main_window(app);
                    }
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    desktop_host::log_line("system tray: ready");
    Ok(())
}

/* ------------------------------------------------------------------ */
/* Application entry                                                   */
/* ------------------------------------------------------------------ */

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = storage::init()?;
            app.manage(state);
            app.manage(desktop_host::HostState::new());
            desktop_host::init(app.handle());
            desktop_host::startup_restore(app.handle());

            // Closing the window keeps GameScape alive in the system tray;
            // quitting lives in the tray menu.
            if let Some(window) = app.get_webview_window("main") {
                let window_for_event = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_for_event.hide();
                    }
                });
            }
            #[cfg(desktop)]
            setup_tray(app.handle())?;

            dev_automation(app.handle().clone());

            #[cfg(windows)]
            {
                // Phase 0 verification gate. Never active in normal use.
                let attach_test = std::env::var("GAMESCAPE_SPIKE_S1")
                    .map(|value| value == "1")
                    .unwrap_or(false);
                let interactive_prototype = std::env::var("GAMESCAPE_SPIKE_INTERACTIVE")
                    .map(|value| value == "1")
                    .unwrap_or(false);
                if attach_test || interactive_prototype {
                    spike_s1::run(app.handle().clone());
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            settings_get,
            settings_set,
            host_turn_off,
            host_apply_interactive,
            catalogue_generate,
            widgets_list,
            widget_create,
            widget_update,
            widget_remove,
            widgets_sync,
            dev_widget_drag_when_ready,
            layouts_list,
            layout_save,
            layout_rename,
            layout_duplicate,
            layout_remove,
            layout_apply,
            system_stats,
            widget_open,
            db_health,
            catalogue_load,
            import_paths,
            library_list,
            library_remove,
            library_update,
            library_check,
            library_relink,
            library_reveal,
            library_open_dir,
            library_info,
            library_set_poster,
            library_read_data,
            collections_list,
            collection_create,
            collection_delete,
            collection_set_membership,
            collection_memberships,
            host_apply_media,
            host_stop,
            host_status,
            host_set_paused,
            host_restore_previous,
            favourites_list,
            favourite_set
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            desktop_host::shutdown(app_handle);
        }
    });
}
