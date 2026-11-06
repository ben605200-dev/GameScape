//! Catalogue model and loader.
//!
//! For Phase 2 the catalogue is a bundled seed index compiled into the
//! binary (`assets/catalogue-seed.json`). Phase 3 replaces the source with
//! the real sync pipeline (fetch → validate → atomic swap); the data shape
//! defined here is the contract between the app, the tooling and the
//! website either way.

use serde::{Deserialize, Serialize};

const SEED_JSON: &str = include_str!("../assets/catalogue-seed.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceholderColors {
    pub dominant_hex: String,
    pub accent_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueEntry {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    /// "static" | "gif" | "mp4"
    pub media_type: String,
    pub game: Option<String>,
    pub tags: Vec<String>,
    pub resolution: Resolution,
    pub licence_id: String,
    pub added_at: String,
    pub placeholder: PlaceholderColors,
    /// Cursor-reactive wallpaper prototype flag. The desktop host ships this
    /// in Phase 5; the spike harness validates the technique today.
    #[serde(default)]
    pub interactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueIndex {
    pub schema_version: String,
    pub generated_at: String,
    pub entries: Vec<CatalogueEntry>,
}

/// Load the bundled seed index and perform minimal sanity validation.
pub fn load_seed() -> Result<CatalogueIndex, String> {
    let index: CatalogueIndex =
        serde_json::from_str(SEED_JSON).map_err(|e| format!("bundled catalogue is invalid: {e}"))?;

    if index.entries.is_empty() {
        return Err("bundled catalogue contains no entries".to_string());
    }

    let mut seen = std::collections::HashSet::new();
    for entry in &index.entries {
        if !seen.insert(entry.id.as_str()) {
            return Err(format!("bundled catalogue has duplicate id: {}", entry.id));
        }
        if !["static", "gif", "mp4"].contains(&entry.media_type.as_str()) {
            return Err(format!(
                "entry {} has unsupported media type: {}",
                entry.id, entry.media_type
            ));
        }
    }

    Ok(index)
}
