//! GameScape desktop application core.
//!
//! Command surface for the React UI: settings, storage health and the
//! catalogue index. All data is local; nothing here uses the network.

mod catalogue;
mod library;
mod storage;

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
/* Application entry                                                   */
/* ------------------------------------------------------------------ */

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = storage::init()?;
            app.manage(state);

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
            collections_list,
            collection_create,
            collection_delete,
            collection_set_membership,
            collection_memberships,
            favourites_list,
            favourite_set
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
