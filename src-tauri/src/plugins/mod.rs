pub mod manager;
pub mod manifest;
pub mod permission;

use crate::config::AppPaths;
use crate::error::AppResult;
use crate::plugins::manifest::PluginManifest;
use crate::storage::Database;

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
