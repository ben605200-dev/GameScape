//! Personal library: the import pipeline and library queries.
//!
//! Import flow per file: extension allowlist → magic bytes → SHA-256 →
//! duplicate check → decode probe → managed copy → thumbnail → database row.
//! Every file gets an explicit status so the UI can report exactly what
//! happened (imported / duplicate / unsupported / invalid / corrupted /
//! failed) — no silent failures.
//!
//! Privacy: files are copied into the user's chosen library folder, decoded
//! locally, and never leave the machine. Imported wallpapers are private by
//! default by design.

use std::fs;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::ImageFormat;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::storage::{self, AppState};

/// Safety cap: refuses absurd imports rather than eating all memory.
const MAX_IMPORT_BYTES: u64 = 128 * 1024 * 1024;
/// Thumbnail target width in pixels.
const THUMBNAIL_WIDTH: u32 = 480;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportItemResult {
    pub file_name: String,
    /// imported | duplicate | unsupported | invalid | corrupted | failed
    pub status: String,
    pub detail: Option<String>,
    pub id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub imported: usize,
    pub duplicates: usize,
    pub failed: usize,
    pub items: Vec<ImportItemResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    /// JSON array string of tags (parsed by the UI).
    pub tags: Option<String>,
    pub format: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub bytes: Option<i64>,
    pub sha256: Option<String>,
    pub thumb_path: Option<String>,
    pub file_path: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSummary {
    pub id: i64,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionMembership {
    pub collection_id: i64,
    pub media_id: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityIssue {
    pub id: i64,
    pub title: String,
    /// missing | changed
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    pub library_dir: String,
}

/* ------------------------------------------------------------------ */
/* Helpers                                                             */
/* ------------------------------------------------------------------ */

/// Magic-byte sniffing — the extension must agree with the actual content.
fn detect_format(bytes: &[u8]) -> Option<(&'static str, ImageFormat)> {
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some(("jpg", ImageFormat::Jpeg));
    }
    if bytes.len() >= 8 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(("png", ImageFormat::Png));
    }
    if bytes.len() >= 6 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return Some(("gif", ImageFormat::Gif));
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some(("webp", ImageFormat::WebP));
    }
    None
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Human-friendly default title from the file name (cleaned, never a path).
fn sanitized_stem(file_name: &str) -> String {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported wallpaper");
    let cleaned: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Imported wallpaper".to_string()
    } else {
        trimmed.to_string()
    }
}

/* ------------------------------------------------------------------ */
/* Import pipeline                                                     */
/* ------------------------------------------------------------------ */

pub fn import_files(
    state: &AppState,
    paths: &[String],
    on_progress: &mut dyn FnMut(usize, usize, &ImportItemResult),
) -> Result<ImportSummary, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|e| format!("settings lock poisoned: {e}"))?
        .clone();
    let library_dir = storage::effective_library_dir(&settings)?;
    fs::create_dir_all(&library_dir)
        .map_err(|e| format!("cannot create library folder {}: {e}", library_dir.display()))?;
    let thumbs_dir = storage::thumbnails_dir()?;
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;

    let total = paths.len();
    let mut summary = ImportSummary {
        imported: 0,
        duplicates: 0,
        failed: 0,
        items: Vec::new(),
    };

    for (index, path_str) in paths.iter().enumerate() {
        let path = PathBuf::from(path_str);
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("(unknown)")
            .to_string();

        let result = import_one(&conn, &path, &file_name, &library_dir, &thumbs_dir);
        match result.status.as_str() {
            "imported" => summary.imported += 1,
            "duplicate" => summary.duplicates += 1,
            _ => summary.failed += 1,
        }
        on_progress(index + 1, total, &result);
        summary.items.push(result);
    }

    Ok(summary)
}

fn import_one(
    conn: &rusqlite::Connection,
    path: &Path,
    file_name: &str,
    library_dir: &Path,
    thumbs_dir: &Path,
) -> ImportItemResult {
    let fail = |status: &str, detail: String| ImportItemResult {
        file_name: file_name.to_string(),
        status: status.to_string(),
        detail: Some(detail),
        id: None,
    };

    let extension_ok = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "gif"
            )
        })
        .unwrap_or(false);
    if !extension_ok {
        return fail(
            "unsupported",
            "GameScape imports JPG, PNG, WEBP and GIF images (MP4 arrives with the media pipeline)."
                .to_string(),
        );
    }

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => return fail("failed", format!("cannot read the file: {error}")),
    };
    if metadata.len() == 0 {
        return fail("invalid", "the file is empty".to_string());
    }
    if metadata.len() > MAX_IMPORT_BYTES {
        return fail("invalid", "file is larger than the 128 MB import limit".to_string());
    }

    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => return fail("failed", format!("cannot read the file: {error}")),
    };

    let Some((format_ext, format)) = detect_format(&bytes) else {
        return fail(
            "invalid",
            "the file is damaged or is not really an image".to_string(),
        );
    };

    let hash = sha256_hex(&bytes);

    // Duplicate check: exact content match anywhere in the library.
    if let Ok((existing_id, existing_title)) = conn.query_row(
        "SELECT id, title FROM media WHERE sha256 = ?1 LIMIT 1",
        [&hash],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
    ) {
        return ImportItemResult {
            file_name: file_name.to_string(),
            status: "duplicate".to_string(),
            detail: Some(format!("already in your library as \"{existing_title}\"")),
            id: Some(existing_id),
        };
    }

    // Decode probe: proves the file is a real, readable image.
    let decoded = match image::load_from_memory_with_format(&bytes, format) {
        Ok(decoded) => decoded,
        Err(_) => {
            return fail(
                "corrupted",
                "the image could not be decoded — it may be damaged".to_string(),
            )
        }
    };
    let (width, height) = (decoded.width(), decoded.height());

    // Managed copy under a collision-proof content-addressed name.
    let short = &hash[..12];
    let target = library_dir.join(format!("{short}.{format_ext}"));
    if !target.exists() {
        if let Err(error) = fs::write(&target, &bytes) {
            return fail("failed", format!("cannot copy into the library: {error}"));
        }
    }

    // Thumbnail (JPEG, up to THUMBNAIL_WIDTH wide).
    let thumb_path = thumbs_dir.join(format!("{short}.jpg"));
    let scale = THUMBNAIL_WIDTH as f32 / width as f32;
    let thumbnail = if scale < 1.0 {
        decoded.resize(
            THUMBNAIL_WIDTH,
            ((height as f32) * scale).round().max(1.0) as u32,
            FilterType::Triangle,
        )
    } else {
        decoded
    };
    if let Err(error) = thumbnail
        .to_rgb8()
        .save_with_format(&thumb_path, ImageFormat::Jpeg)
    {
        return fail("failed", format!("cannot generate thumbnail: {error}"));
    }

    let title = sanitized_stem(file_name);
    let aspect = width as f64 / height as f64;

    let insert = conn.execute(
        "INSERT INTO media (source, path, managed, title, format, width, height, aspect, bytes, sha256, thumb_path, status)
         VALUES ('local_import', ?1, 1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'ok')",
        rusqlite::params![
            target.to_string_lossy(),
            title,
            format_ext,
            width as i64,
            height as i64,
            aspect,
            bytes.len() as i64,
            hash,
            thumb_path.to_string_lossy(),
        ],
    );

    match insert {
        Ok(_) => ImportItemResult {
            file_name: file_name.to_string(),
            status: "imported".to_string(),
            detail: Some(format!("{width}×{height} {format_ext}")),
            id: Some(conn.last_insert_rowid()),
        },
        Err(error) => fail("failed", format!("cannot save to the library database: {error}")),
    }
}

/* ------------------------------------------------------------------ */
/* Queries                                                             */
/* ------------------------------------------------------------------ */

pub fn list_library(state: &AppState) -> Result<Vec<LibraryItem>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;

    let mut statement = conn
        .prepare(
            "SELECT id, title, description, tags, format, width, height, bytes, sha256, thumb_path, path, created_at
             FROM media ORDER BY id DESC",
        )
        .map_err(|e| format!("cannot query the library: {e}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(LibraryItem {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                tags: row.get(3)?,
                format: row.get(4)?,
                width: row.get(5)?,
                height: row.get(6)?,
                bytes: row.get(7)?,
                sha256: row.get(8)?,
                thumb_path: row.get(9)?,
                file_path: row.get(10)?,
                added_at: row.get(11)?,
            })
        })
        .map_err(|e| format!("cannot query the library: {e}"))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| format!("cannot read a library row: {e}"))?);
    }
    Ok(items)
}

/* ------------------------------------------------------------------ */
/* Metadata editing                                                    */
/* ------------------------------------------------------------------ */

pub fn update_metadata(
    state: &AppState,
    id: i64,
    title: &str,
    description: &str,
    tags: &[String],
) -> Result<(), String> {
    let title = title.trim();
    if title.is_empty() || title.len() > 120 {
        return Err("title must be 1–120 characters".to_string());
    }
    let description = description.trim();
    if description.len() > 500 {
        return Err("description is limited to 500 characters".to_string());
    }

    let mut clean_tags: Vec<String> = Vec::new();
    for tag in tags {
        let cleaned = tag.trim().to_lowercase();
        if cleaned.is_empty() {
            continue;
        }
        if cleaned.len() > 32 {
            return Err("each tag is limited to 32 characters".to_string());
        }
        if !clean_tags.contains(&cleaned) {
            clean_tags.push(cleaned);
        }
        if clean_tags.len() > 20 {
            return Err("at most 20 tags are allowed".to_string());
        }
    }

    let tags_json = serde_json::to_string(&clean_tags).map_err(|e| e.to_string())?;
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    let updated = conn
        .execute(
            "UPDATE media SET title = ?1, description = ?2, tags = ?3 WHERE id = ?4",
            rusqlite::params![
                title,
                if description.is_empty() {
                    None
                } else {
                    Some(description)
                },
                tags_json,
                id
            ],
        )
        .map_err(|e| format!("cannot update the entry: {e}"))?;
    if updated == 0 {
        return Err("library item not found".to_string());
    }
    Ok(())
}

/* ------------------------------------------------------------------ */
/* Collections                                                         */
/* ------------------------------------------------------------------ */

pub fn collections_list(state: &AppState) -> Result<Vec<CollectionSummary>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    let mut statement = conn
        .prepare(
            "SELECT c.id, c.name, COUNT(ci.media_id)
             FROM collections c
             LEFT JOIN collection_items ci ON ci.collection_id = c.id
             GROUP BY c.id
             ORDER BY c.name COLLATE NOCASE",
        )
        .map_err(|e| format!("cannot query collections: {e}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(CollectionSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                count: row.get(2)?,
            })
        })
        .map_err(|e| format!("cannot query collections: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| format!("cannot read a collection row: {e}"))?);
    }
    Ok(items)
}

pub fn collection_create(state: &AppState, name: &str) -> Result<i64, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 80 {
        return Err("collection names are 1–80 characters".to_string());
    }
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    let existing: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM collections WHERE name = ?1 COLLATE NOCASE",
            [name],
            |row| row.get(0),
        )
        .map_err(|e| format!("cannot check the collection name: {e}"))?;
    if existing > 0 {
        return Err("a collection with this name already exists".to_string());
    }
    conn.execute("INSERT INTO collections (name) VALUES (?1)", [name])
        .map_err(|e| format!("cannot create the collection: {e}"))?;
    Ok(conn.last_insert_rowid())
}

pub fn collection_delete(state: &AppState, id: i64) -> Result<(), String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    conn.execute("DELETE FROM collections WHERE id = ?1", [id])
        .map_err(|e| format!("cannot delete the collection: {e}"))?;
    Ok(())
}

pub fn collection_set_membership(
    state: &AppState,
    collection_id: i64,
    media_id: i64,
    member: bool,
) -> Result<(), String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    if member {
        conn.execute(
            "INSERT OR IGNORE INTO collection_items (collection_id, media_id) VALUES (?1, ?2)",
            [collection_id, media_id],
        )
        .map_err(|e| format!("cannot add to the collection: {e}"))?;
    } else {
        conn.execute(
            "DELETE FROM collection_items WHERE collection_id = ?1 AND media_id = ?2",
            [collection_id, media_id],
        )
        .map_err(|e| format!("cannot remove from the collection: {e}"))?;
    }
    Ok(())
}

pub fn collection_memberships(state: &AppState) -> Result<Vec<CollectionMembership>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    let mut statement = conn
        .prepare("SELECT collection_id, media_id FROM collection_items")
        .map_err(|e| format!("cannot query memberships: {e}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(CollectionMembership {
                collection_id: row.get(0)?,
                media_id: row.get(1)?,
            })
        })
        .map_err(|e| format!("cannot query memberships: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| format!("cannot read a membership row: {e}"))?);
    }
    Ok(items)
}

/* ------------------------------------------------------------------ */
/* Integrity                                                           */
/* ------------------------------------------------------------------ */

/// Check every managed entry against the filesystem: reports files that are
/// missing entirely or whose size no longer matches.
pub fn check_integrity(state: &AppState) -> Result<Vec<IntegrityIssue>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    let mut statement = conn
        .prepare("SELECT id, title, path, bytes FROM media")
        .map_err(|e| format!("cannot query the library: {e}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })
        .map_err(|e| format!("cannot query the library: {e}"))?;

    let mut issues = Vec::new();
    for row in rows {
        let (id, title, path, bytes) = row.map_err(|e| format!("cannot read a library row: {e}"))?;
        let Some(path) = path else {
            issues.push(IntegrityIssue {
                id,
                title,
                status: "missing".to_string(),
            });
            continue;
        };
        match fs::metadata(&path) {
            Err(_) => issues.push(IntegrityIssue {
                id,
                title,
                status: "missing".to_string(),
            }),
            Ok(metadata) => {
                if let Some(expected) = bytes {
                    if metadata.len() as i64 != expected {
                        issues.push(IntegrityIssue {
                            id,
                            title,
                            status: "changed".to_string(),
                        });
                    }
                }
            }
        }
    }
    Ok(issues)
}

/// Point an entry at a replacement file: validates it like an import, copies
/// it into the library, regenerates the thumbnail, and updates the row while
/// keeping id, title, description and tags.
pub fn relink(state: &AppState, id: i64, new_path: &str) -> Result<(), String> {
    let path = PathBuf::from(new_path);
    let extension_ok = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "gif"
            )
        })
        .unwrap_or(false);
    if !extension_ok {
        return Err("only JPG, PNG, WEBP and GIF files can be relinked".to_string());
    }

    let bytes = fs::read(&path).map_err(|e| format!("cannot read the file: {e}"))?;
    let Some((format_ext, format)) = detect_format(&bytes) else {
        return Err("the file is damaged or is not really an image".to_string());
    };
    let decoded = image::load_from_memory_with_format(&bytes, format)
        .map_err(|_| "the image could not be decoded".to_string())?;
    let (width, height) = (decoded.width(), decoded.height());
    let hash = sha256_hex(&bytes);
    let short = &hash[..12];

    let settings = state
        .settings
        .lock()
        .map_err(|e| format!("settings lock poisoned: {e}"))?
        .clone();
    let library_dir = storage::effective_library_dir(&settings)?;
    fs::create_dir_all(&library_dir)
        .map_err(|e| format!("cannot create the library folder: {e}"))?;
    let target = library_dir.join(format!("{short}.{format_ext}"));
    if !target.exists() {
        fs::write(&target, &bytes).map_err(|e| format!("cannot copy into the library: {e}"))?;
    }

    let thumbs_dir = storage::thumbnails_dir()?;
    let thumb_path = thumbs_dir.join(format!("{short}.jpg"));
    let scale = THUMBNAIL_WIDTH as f32 / width as f32;
    let thumbnail = if scale < 1.0 {
        decoded.resize(
            THUMBNAIL_WIDTH,
            ((height as f32) * scale).round().max(1.0) as u32,
            FilterType::Triangle,
        )
    } else {
        decoded
    };
    thumbnail
        .to_rgb8()
        .save_with_format(&thumb_path, ImageFormat::Jpeg)
        .map_err(|e| format!("cannot generate thumbnail: {e}"))?;

    let aspect = width as f64 / height as f64;
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;
    let updated = conn
        .execute(
            "UPDATE media SET path = ?1, format = ?2, width = ?3, height = ?4, aspect = ?5,
                              bytes = ?6, sha256 = ?7, thumb_path = ?8, status = 'ok'
             WHERE id = ?9",
            rusqlite::params![
                target.to_string_lossy(),
                format_ext,
                width as i64,
                height as i64,
                aspect,
                bytes.len() as i64,
                hash,
                thumb_path.to_string_lossy(),
                id
            ],
        )
        .map_err(|e| format!("cannot update the entry: {e}"))?;
    if updated == 0 {
        return Err("library item not found".to_string());
    }
    Ok(())
}

/* ------------------------------------------------------------------ */
/* Shell integration                                                   */
/* ------------------------------------------------------------------ */

/// Reveal a managed file in Windows Explorer (selects it). Windows-only by
/// product scope; paths are resolved server-side from the database.
pub fn reveal_item(state: &AppState, id: i64) -> Result<(), String> {
    let path = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("database lock poisoned: {e}"))?;
        conn.query_row("SELECT path FROM media WHERE id = ?1", [id], |row| {
            row.get::<_, Option<String>>(0)
        })
        .map_err(|e| format!("library item not found: {e}"))?
        .ok_or("this entry has no file on disk")?
    };
    if !Path::new(&path).exists() {
        return Err("the file is missing on disk — try Check integrity".to_string());
    }
    std::process::Command::new("explorer.exe")
        .arg("/select,")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("cannot open Explorer: {e}"))?;
    Ok(())
}

/// Open the library folder itself in Windows Explorer.
pub fn open_library_dir(state: &AppState) -> Result<(), String> {
    let settings = state
        .settings
        .lock()
        .map_err(|e| format!("settings lock poisoned: {e}"))?
        .clone();
    let dir = storage::effective_library_dir(&settings)?;
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create the library folder: {e}"))?;
    std::process::Command::new("explorer.exe")
        .arg(&dir)
        .spawn()
        .map_err(|e| format!("cannot open Explorer: {e}"))?;
    Ok(())
}

pub fn library_info(state: &AppState) -> Result<LibraryInfo, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|e| format!("settings lock poisoned: {e}"))?
        .clone();
    let dir = storage::effective_library_dir(&settings)?;
    Ok(LibraryInfo {
        library_dir: dir.to_string_lossy().to_string(),
    })
}

/// Remove an entry from the library. With `delete_files`, the managed copy
/// and its thumbnail are deleted as well (permanent — the UI must say so).
/// Original source files outside the library are never touched.
pub fn remove_from_library(state: &AppState, id: i64, delete_files: bool) -> Result<(), String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {e}"))?;

    if delete_files {
        if let Ok((path, thumb)) = conn.query_row(
            "SELECT path, thumb_path FROM media WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        ) {
            if let Some(path) = path {
                let _ = fs::remove_file(path);
            }
            if let Some(thumb) = thumb {
                let _ = fs::remove_file(thumb);
            }
        }
    }

    conn.execute("DELETE FROM media WHERE id = ?1", [id])
        .map_err(|e| format!("cannot remove the entry: {e}"))?;
    Ok(())
}
