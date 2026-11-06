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

/// Dev-only automation hooks used by local harnesses (never set in normal
/// use): `GAMESCAPE_DEV_IMPORT` imports a `;`-separated list at startup and
/// `GAMESCAPE_START_HASH` deep-links the main window after load.
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

    if let Ok(hash) = std::env::var("GAMESCAPE_START_HASH") {
        let safe: String = hash
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_'))
            .collect();
        if !safe.is_empty() {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(6));
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.eval(&format!("window.location.hash = '#/{safe}';"));
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
            library_remove
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
