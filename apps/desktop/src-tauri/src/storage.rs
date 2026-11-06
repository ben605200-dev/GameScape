//! Local storage: app-data paths, settings (JSON) and the SQLite database
//! with versioned migrations.
//!
//! Privacy: everything here lives under the user's Windows app-data folder.
//! Nothing in this module ever touches the network.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/* ------------------------------------------------------------------ */
/* Paths                                                               */
/* ------------------------------------------------------------------ */

/// `%APPDATA%\GameScape` (created on first use).
pub fn app_data_dir() -> Result<PathBuf, String> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "APPDATA environment variable is not set".to_string())?;
    let dir = base.join("GameScape");
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir)
}

pub fn db_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("gamescape.db"))
}

pub fn settings_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("settings.json"))
}

/// `%LOCALAPPDATA%\GameScape\cache` (regenerable data lives here).
pub fn cache_dir() -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "LOCALAPPDATA environment variable is not set".to_string())?;
    let dir = base.join("GameScape").join("cache");
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir)
}

pub fn thumbnails_dir() -> Result<PathBuf, String> {
    let dir = cache_dir()?.join("thumbnails");
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir)
}

pub fn default_library_dir() -> Result<PathBuf, String> {
    let base = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or_else(|| "USERPROFILE environment variable is not set".to_string())?;
    Ok(base.join("Pictures").join("GameScape Library"))
}

/// The library location in effect: user setting first, default second.
pub fn effective_library_dir(settings: &Settings) -> Result<PathBuf, String> {
    if let Some(dir) = settings.library_dir.as_deref() {
        if !dir.trim().is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    default_library_dir()
}

/* ------------------------------------------------------------------ */
/* Settings (JSON)                                                     */
/* ------------------------------------------------------------------ */

/// User settings. Only the fields needed today exist; new fields are added
/// additively and default safely when missing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Option<String>,
    /// Custom library location; `None` uses the default under Pictures.
    pub library_dir: Option<String>,
}

pub fn load_settings() -> Settings {
    let Ok(path) = settings_path() else {
        return Settings::default();
    };
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

/// Atomic-ish save: write to a temporary file, then rename over the target.
pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let path = settings_path()?;
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("cannot write settings: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("cannot finalise settings: {e}"))?;
    Ok(())
}

/* ------------------------------------------------------------------ */
/* Database                                                            */
/* ------------------------------------------------------------------ */

/// Schema v1 — the tables Phases 2–3 need. Later features (layouts,
/// widgets, playlists, wallpaper state) arrive as new migrations, never by
/// editing this one.
const V1_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS media (
  id INTEGER PRIMARY KEY,
  source TEXT NOT NULL CHECK (source IN ('public_catalogue', 'local_import')),
  catalogue_id TEXT,
  path TEXT,
  managed INTEGER NOT NULL DEFAULT 0,
  title TEXT NOT NULL,
  description TEXT,
  creator TEXT,
  game TEXT,
  category TEXT,
  tags TEXT,
  format TEXT,
  width INTEGER,
  height INTEGER,
  aspect REAL,
  bytes INTEGER,
  duration_s REAL,
  sha256 TEXT,
  thumb_path TEXT,
  licence_id TEXT,
  attribution TEXT,
  status TEXT NOT NULL DEFAULT 'ok',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS media_sha256_idx ON media (sha256) WHERE sha256 IS NOT NULL;
CREATE INDEX IF NOT EXISTS media_catalogue_idx ON media (catalogue_id);

CREATE TABLE IF NOT EXISTS collections (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS collection_items (
  collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  PRIMARY KEY (collection_id, media_id)
);

CREATE TABLE IF NOT EXISTS favourites (
  catalogue_id TEXT PRIMARY KEY,
  added_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS downloads (
  id INTEGER PRIMARY KEY,
  catalogue_id TEXT NOT NULL,
  state TEXT NOT NULL,
  bytes_done INTEGER NOT NULL DEFAULT 0,
  bytes_total INTEGER,
  url TEXT,
  sha256 TEXT,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS catalogue_cache (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  source TEXT NOT NULL,
  version TEXT,
  entry_count INTEGER NOT NULL DEFAULT 0,
  loaded_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

fn migrations() -> &'static [(i64, &'static str)] {
    &[(1, V1_SQL)]
}

pub fn open_and_migrate() -> Result<Connection, String> {
    let path = db_path()?;
    let conn =
        Connection::open(&path).map_err(|e| format!("cannot open database: {e}"))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;",
    )
    .map_err(|e| format!("cannot configure database: {e}"))?;
    apply_migrations(&conn)?;
    Ok(conn)
}

fn apply_migrations(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
           version INTEGER PRIMARY KEY,
           applied_at TEXT NOT NULL DEFAULT (datetime('now'))
         );",
    )
    .map_err(|e| format!("cannot initialise migrations table: {e}"))?;

    let current: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("cannot read schema version: {e}"))?;

    for (version, sql) in migrations() {
        if *version > current {
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| format!("cannot begin migration {version}: {e}"))?;
            tx.execute_batch(sql)
                .map_err(|e| format!("migration {version} failed: {e}"))?;
            tx.execute(
                "INSERT INTO schema_migrations (version) VALUES (?1)",
                [version],
            )
            .map_err(|e| format!("cannot record migration {version}: {e}"))?;
            tx.commit()
                .map_err(|e| format!("cannot commit migration {version}: {e}"))?;
        }
    }
    Ok(())
}

pub fn schema_version(conn: &Connection) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
    .map_err(|e| format!("cannot read schema version: {e}"))
}

/* ------------------------------------------------------------------ */
/* Application state                                                   */
/* ------------------------------------------------------------------ */

/// Shared state managed by Tauri. Single connection guarded by a mutex —
/// fine for the app's access patterns (short queries from commands).
pub struct AppState {
    pub db: Mutex<Connection>,
    pub settings: Mutex<Settings>,
}

pub fn init() -> Result<AppState, String> {
    let conn = open_and_migrate()?;
    let settings = load_settings();
    Ok(AppState {
        db: Mutex::new(conn),
        settings: Mutex::new(settings),
    })
}
