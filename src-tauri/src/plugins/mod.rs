pub mod manager;
pub mod manifest;
pub mod permission;

use crate::config::AppPaths;
use crate::error::AppResult;
use crate::plugins::manifest::PluginManifest;
use crate::storage::Database;

pub const EVERYTHING_ID: &str = "triex-everything";

#[cfg(windows)]
pub fn register_everything(db: &Database) -> AppResult<()> {
    let manifest = serde_json::from_str(include_str!("../../resources/everything-plugin.json"))?;
    db.upsert_plugin(&manifest, true)
}

/// Scans `%LocalAppData%\triex\plugins` for valid `plugin.json` packages and syncs them to SQLite.
pub fn sync_installed_plugins(paths: &AppPaths, db: &Database) -> AppResult<usize> {
    let mut synced = 0usize;
    if !paths.plugins_dir.is_dir() {
        return Ok(0);
    }

    for entry in std::fs::read_dir(&paths.plugins_dir)?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        match PluginManifest::load_from_dir(&path) {
            Ok(manifest) if manifest.id.eq_ignore_ascii_case(EVERYTHING_ID) => continue,
            Ok(manifest) => {
                db.upsert_plugin(&manifest, true)?;
                synced += 1;
            }
            Err(err) => {
                tracing::warn!("Skipping invalid plugin directory {}: {err}", path.display());
            }
        }
    }

    Ok(synced)
}
