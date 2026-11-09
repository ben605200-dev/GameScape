//! Saved desktop layouts (Layout Editor): named snapshots of the wallpaper
//! state, desktop widgets and appearance, saved and switched in one click.
//!
//! Privacy: everything lives in the local database; nothing leaves the PC.

use serde::{Deserialize, Serialize};

use crate::desktop_host::{self, HostState};
use crate::storage::{self, AppState, LastWallpaper};

/// Widget kinds the desktop host can place; anything else in a snapshot is
/// skipped on apply.
const KNOWN_KINDS: &[&str] = &[
    "clock",
    "system",
    "wallpaper",
    "calendar",
    "countdown",
    "text",
    "launcher",
];

/// One widget inside a layout snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutWidget {
    pub kind: String,
    pub monitor: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub enabled: bool,
    pub settings: Option<String>,
}

/// The desktop state captured by a layout.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LayoutSnapshot {
    /// The wallpaper that was applied when the layout was saved.
    pub wallpaper: Option<LastWallpaper>,
    /// Fit mode ("fill", "fit", ...) at save time.
    pub wallpaper_fit: Option<String>,
    /// `Some(false)` when wallpapers were turned off at save time.
    pub wallpaper_enabled: Option<bool>,
    /// App appearance theme at save time.
    pub theme: Option<String>,
    pub widgets: Vec<LayoutWidget>,
}

/// A layout as shown in the Layout Editor (snapshot summarised).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRow {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub wallpaper_title: Option<String>,
    pub wallpaper_kind: Option<String>,
    pub wallpaper_media_id: Option<i64>,
    pub wallpaper_thumb: Option<String>,
    /// Whether the layout's wallpaper is still in the library. `None` when
    /// the layout has no wallpaper.
    pub wallpaper_available: Option<bool>,
    pub widget_count: usize,
    pub fit: Option<String>,
    pub theme: Option<String>,
}

fn validate_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("give the layout a name first".to_string());
    }
    if trimmed.chars().count() > 60 {
        return Err("layout names are limited to 60 characters".to_string());
    }
    Ok(trimmed.to_string())
}

/// Captures the current desktop state into a snapshot.
fn snapshot_now(state: &AppState) -> Result<LayoutSnapshot, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|e| format!("settings lock poisoned: {e}"))?
        .clone();
    let widgets = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("database lock poisoned: {e}"))?;
        storage::list_widgets(&conn)?
    };
    Ok(LayoutSnapshot {
        wallpaper: settings.last_wallpaper.clone(),
        wallpaper_fit: settings.wallpaper_fit.clone(),
        wallpaper_enabled: settings.wallpaper_enabled,
        theme: settings.theme.clone(),
        widgets: widgets
            .into_iter()
            .map(|widget| LayoutWidget {
                kind: widget.kind,
                monitor: widget.monitor,
                x: widget.x,
                y: widget.y,
                width: widget.width,
                height: widget.height,
                enabled: widget.enabled,
                settings: widget.settings,
            })
            .collect(),
    })
}

/// Saves the current desktop as a layout (new when `id` is `None`, otherwise
/// refreshes that layout in place). Returns the layout id.
pub fn save(state: &AppState, id: Option<i64>, name: String) -> Result<i64, String> {
    let name = validate_name(&name)?;
    let snapshot = snapshot_now(state)?;
    let json = serde_json::to_string(&snapshot)
        .map_err(|e| format!("cannot serialise the layout: {e}"))?;
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    let id = match id {
        Some(id) => {
            let changed = conn
                .execute(
                    "UPDATE layouts
                     SET name = ?1, snapshot = ?2, updated_at = datetime('now')
                     WHERE id = ?3",
                    rusqlite::params![name, json, id],
                )
                .map_err(|e| format!("cannot update the layout: {e}"))?;
            if changed == 0 {
                return Err("this layout no longer exists".to_string());
            }
            id
        }
        None => {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM layouts", [], |row| row.get(0))
                .unwrap_or(0);
            if count >= 50 {
                return Err("layout limit reached (50) — remove one first".to_string());
            }
            conn.execute(
                "INSERT INTO layouts (name, snapshot) VALUES (?1, ?2)",
                rusqlite::params![name, json],
            )
            .map_err(|e| format!("cannot save the layout: {e}"))?;
            conn.last_insert_rowid()
        }
    };
    drop(conn);
    let wallpaper = match &snapshot.wallpaper {
        Some(value) => value.title.as_str(),
        None => "none",
    };
    desktop_host::log_line(&format!(
        "layout save: id={id} '{}' ({} widgets, wallpaper: {wallpaper})",
        name,
        snapshot.widgets.len()
    ));
    Ok(id)
}

/// Renames a layout without touching its snapshot.
pub fn rename(state: &AppState, id: i64, name: String) -> Result<(), String> {
    let name = validate_name(&name)?;
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    let changed = conn
        .execute(
            "UPDATE layouts SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![name, id],
        )
        .map_err(|e| format!("cannot rename the layout: {e}"))?;
    if changed == 0 {
        return Err("this layout no longer exists".to_string());
    }
    desktop_host::log_line(&format!("layout rename: id={id} '{name}'"));
    Ok(())
}

pub fn remove(state: &AppState, id: i64) -> Result<(), String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    let changed = conn
        .execute("DELETE FROM layouts WHERE id = ?1", [id])
        .map_err(|e| format!("cannot remove the layout: {e}"))?;
    if changed == 0 {
        return Err("this layout no longer exists".to_string());
    }
    desktop_host::log_line(&format!("layout remove: id={id}"));
    Ok(())
}

/// Lists layouts with their snapshots summarised for the editor cards.
pub fn list(state: &AppState) -> Result<Vec<LayoutRow>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    let mut statement = conn
        .prepare("SELECT id, name, snapshot, created_at, updated_at FROM layouts ORDER BY id")
        .map_err(|e| format!("cannot read layouts: {e}"))?;
    let mapped = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("cannot read layouts: {e}"))?;

    let mut rows = Vec::new();
    for row in mapped {
        let (id, name, snapshot_json, created_at, updated_at) =
            row.map_err(|e| format!("cannot read layouts: {e}"))?;
        let snapshot: LayoutSnapshot =
            serde_json::from_str(&snapshot_json).unwrap_or_default();
        let media_id = snapshot.wallpaper.as_ref().map(|value| value.media_id);
        let (thumb, available) = match media_id {
            Some(media_id) => {
                let found: Option<Option<String>> = conn
                    .query_row(
                        "SELECT thumb_path FROM media WHERE id = ?1",
                        [media_id],
                        |row| row.get(0),
                    )
                    .ok();
                match found {
                    Some(thumb) => (thumb, Some(true)),
                    None => (None, Some(false)),
                }
            }
            None => (None, None),
        };
        rows.push(LayoutRow {
            id,
            name,
            created_at,
            updated_at,
            wallpaper_title: snapshot.wallpaper.as_ref().map(|value| value.title.clone()),
            wallpaper_kind: snapshot.wallpaper.as_ref().map(|value| value.kind.clone()),
            wallpaper_media_id: media_id,
            wallpaper_thumb: thumb,
            wallpaper_available: available,
            widget_count: snapshot.widgets.len(),
            fit: snapshot.wallpaper_fit.clone(),
            theme: snapshot.theme.clone(),
        });
    }
    Ok(rows)
}

/// Applies a layout: restores appearance, replaces the desktop widgets and
/// re-applies the saved wallpaper through the same paths a manual apply uses.
pub fn apply(
    app: &tauri::AppHandle,
    state: &AppState,
    host: &HostState,
    id: i64,
) -> Result<String, String> {
    let (name, snapshot) = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("database lock poisoned: {e}"))?;
        conn.query_row(
            "SELECT name, snapshot FROM layouts WHERE id = ?1",
            [id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|e| format!("this layout was not found: {e}"))?
    };
    let snapshot: LayoutSnapshot =
        serde_json::from_str(&snapshot).map_err(|e| format!("the layout is unreadable: {e}"))?;
    desktop_host::log_line(&format!(
        "layout apply: id={id} '{name}' started ({} widgets)",
        snapshot.widgets.len()
    ));

    // Validate everything before touching the desktop so a broken snapshot
    // cannot leave the setup half-applied.
    let enabled = snapshot.wallpaper_enabled.unwrap_or(true);
    if enabled {
        if let Some(wallpaper) = &snapshot.wallpaper {
            let exists: bool = {
                let conn = state
                    .db
                    .lock()
                    .map_err(|e| format!("database lock poisoned: {e}"))?;
                conn.query_row(
                    "SELECT COUNT(*) FROM media WHERE id = ?1",
                    [wallpaper.media_id],
                    |row| row.get::<_, i64>(0),
                )
                .map(|count| count > 0)
                .unwrap_or(false)
            };
            if !exists {
                return Err(format!(
                    "the wallpaper \u{201c}{}\u{201d} is no longer in your library — \
                     re-link or re-import it, or save the layout again",
                    wallpaper.title
                ));
            }
            if let Some(monitor) = wallpaper.monitor {
                let count = app
                    .available_monitors()
                    .map_err(|e| format!("cannot list monitors: {e}"))?
                    .len() as u32;
                if monitor >= count {
                    return Err(format!(
                        "this layout's wallpaper targets monitor {}, but only {count} \
                         monitor(s) are connected",
                        monitor + 1
                    ));
                }
            }
            if wallpaper.kind == "interactive" {
                let parts: Vec<&str> = wallpaper.path.split('|').collect();
                if parts.len() != 3 {
                    return Err("this layout's interactive wallpaper has a broken effect token"
                        .to_string());
                }
            }
        }
    }

    // Appearance + fit first: the wallpaper apply below reads the fit mode.
    {
        let mut settings = state
            .settings
            .lock()
            .map_err(|e| format!("settings lock poisoned: {e}"))?;
        if snapshot.wallpaper_fit.is_some() {
            settings.wallpaper_fit = snapshot.wallpaper_fit.clone();
        }
        if snapshot.theme.is_some() {
            settings.theme = snapshot.theme.clone();
        }
        if let Some(enabled) = snapshot.wallpaper_enabled {
            settings.wallpaper_enabled = Some(enabled);
        }
        storage::save_settings(&settings)?;
    }

    // Replace the widget set, then let the desktop host place it.
    {
        let _serial = host.serial_lock()?;
        let rows = {
            let conn = state
                .db
                .lock()
                .map_err(|e| format!("database lock poisoned: {e}"))?;
            conn.execute("DELETE FROM widgets", [])
                .map_err(|e| format!("cannot clear the widgets: {e}"))?;
            for widget in &snapshot.widgets {
                if !KNOWN_KINDS.contains(&widget.kind.as_str()) {
                    continue;
                }
                conn.execute(
                    "INSERT INTO widgets
                       (kind, monitor, x, y, width, height, enabled, settings)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![
                        widget.kind,
                        widget.monitor,
                        widget.x,
                        widget.y,
                        widget.width,
                        widget.height,
                        widget.enabled as i64,
                        widget.settings,
                    ],
                )
                .map_err(|e| format!("cannot restore the widgets: {e}"))?;
            }
            storage::list_widgets(&conn)?
        };
        desktop_host::sync_widgets_rows(app, host, &rows)?;
        desktop_host::log_line(&format!(
            "layout apply: widgets replaced ({} in the layout, {} placed)",
            snapshot.widgets.len(),
            rows.len()
        ));
    }

    // The wallpaper side.
    if !enabled {
        desktop_host::turn_off(app, state, host)?;
        desktop_host::log_line("layout apply: wallpapers off (as saved)");
    } else if let Some(wallpaper) = &snapshot.wallpaper {
        if wallpaper.kind == "interactive" {
            let parts: Vec<&str> = wallpaper.path.split('|').collect();
            desktop_host::apply_interactive(
                app,
                state,
                host,
                wallpaper.media_id,
                parts[0],
                parts[1],
                parts[2],
            )?;
        } else {
            desktop_host::apply_library_item(app, state, host, wallpaper.media_id, wallpaper.monitor)?;
        }
    } else {
        desktop_host::log_line("layout apply: no wallpaper saved in this layout");
    }

    desktop_host::log_line(&format!("layout apply: id={id} '{name}' done"));
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_state() -> AppState {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.execute_batch(
            "CREATE TABLE widgets (
               id INTEGER PRIMARY KEY,
               kind TEXT NOT NULL,
               monitor INTEGER NOT NULL DEFAULT 0,
               x INTEGER NOT NULL DEFAULT 40,
               y INTEGER NOT NULL DEFAULT 40,
               width INTEGER NOT NULL DEFAULT 360,
               height INTEGER NOT NULL DEFAULT 220,
               enabled INTEGER NOT NULL DEFAULT 1,
               settings TEXT
             );
             CREATE TABLE layouts (
               id INTEGER PRIMARY KEY,
               name TEXT NOT NULL,
               snapshot TEXT NOT NULL,
               created_at TEXT NOT NULL DEFAULT (datetime('now')),
               updated_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE TABLE media (id INTEGER PRIMARY KEY, thumb_path TEXT);",
        )
        .expect("schema");
        conn.execute(
            "INSERT INTO widgets (kind, monitor, x, y, enabled, settings)
             VALUES ('clock', 0, 10, 20, 1, '{\"font\":\"Segoe UI\"}')",
            [],
        )
        .expect("seed widget");
        let mut settings = storage::Settings::default();
        settings.theme = Some("dark".to_string());
        settings.wallpaper_fit = Some("fill".to_string());
        AppState {
            db: std::sync::Mutex::new(conn),
            settings: std::sync::Mutex::new(settings),
        }
    }

    #[test]
    fn name_validation_trims_and_rejects() {
        assert_eq!(validate_name("  Gaming  ").unwrap(), "Gaming");
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"x".repeat(61)).is_err());
    }

    #[test]
    fn snapshot_round_trips_through_json() {
        let snapshot = LayoutSnapshot {
            wallpaper: Some(LastWallpaper {
                kind: "static".to_string(),
                media_id: 7,
                path: "C:/pictures/one.png".to_string(),
                format: Some("png".to_string()),
                title: "One".to_string(),
                monitor: Some(1),
            }),
            wallpaper_fit: Some("fit".to_string()),
            wallpaper_enabled: Some(true),
            theme: Some("dark".to_string()),
            widgets: vec![LayoutWidget {
                kind: "text".to_string(),
                monitor: 0,
                x: 60,
                y: 60,
                width: 340,
                height: 140,
                enabled: true,
                settings: Some("{\"text\":\"hi\"}".to_string()),
            }],
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let parsed: LayoutSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.widgets.len(), 1);
        assert_eq!(parsed.widgets[0].kind, "text");
        assert_eq!(parsed.wallpaper.as_ref().unwrap().media_id, 7);
        assert_eq!(parsed.theme.as_deref(), Some("dark"));
    }

    #[test]
    fn save_list_rename_remove_cycle() {
        let state = test_state();
        let id = save(&state, None, "  Night owl  ".to_string()).expect("save");
        let rows = list(&state).expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Night owl");
        assert_eq!(rows[0].widget_count, 1);
        assert_eq!(rows[0].fit.as_deref(), Some("fill"));
        assert_eq!(rows[0].theme.as_deref(), Some("dark"));
        assert_eq!(rows[0].wallpaper_available, None);

        // Updating in place refreshes the snapshot.
        let same = save(&state, Some(id), "Night owl".to_string()).expect("refresh");
        assert_eq!(same, id);

        rename(&state, id, "Day owl".to_string()).expect("rename");
        assert_eq!(list(&state).unwrap()[0].name, "Day owl");

        // Unknown ids are rejected.
        assert!(rename(&state, 999, "x".to_string()).is_err());
        assert!(save(&state, Some(999), "x".to_string()).is_err());

        remove(&state, id).expect("remove");
        assert!(list(&state).unwrap().is_empty());
        assert!(remove(&state, id).is_err());
    }
}
