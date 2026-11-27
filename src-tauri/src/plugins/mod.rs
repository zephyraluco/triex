pub mod backend;
pub mod manager;
pub mod manifest;
pub mod permission;

use std::collections::HashSet;
use std::path::Path;

use crate::config::AppPaths;
use crate::error::{AppError, AppResult};
use crate::plugins::manifest::PluginManifest;
use crate::storage::Database;

/// Scans `%LocalAppData%\triex\plugins` for valid `plugin.json` packages, syncs them to SQLite,
/// and prunes database records whose plugin directories were removed from disk.
pub fn sync_installed_plugins(paths: &AppPaths, db: &Database) -> AppResult<usize> {
    let mut synced = 0usize;
    let mut found_ids = HashSet::new();

    if paths.plugins_dir.is_dir() {
        for entry in std::fs::read_dir(&paths.plugins_dir)?.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            match PluginManifest::load_from_dir(&path) {
                Ok(manifest) => {
                    found_ids.insert(manifest.id.clone());
                    db.upsert_plugin(&manifest, true)?;
                    synced += 1;
                }
                Err(err) => {
                    tracing::warn!("Skipping invalid plugin directory {}: {err}", path.display());
                }
            }
        }
    }

    // Remove stale plugin rows whose directories no longer exist on disk.
    if let Ok(existing) = db.list_plugins() {
        for record in existing {
            if !found_ids.contains(&record.id) {
                let _ = db.remove_plugin(&record.id);
            }
        }
    }

    Ok(synced)
}

/// Validates and copies a local plugin directory containing `plugin.json` into
/// `%LocalAppData%\triex\plugins\<id>`, then registers it in SQLite.
pub fn install_plugin_from_dir(
    source_dir: &Path,
    paths: &AppPaths,
    db: &Database,
) -> AppResult<PluginManifest> {
    if !source_dir.is_dir() {
        return Err(AppError::InvalidInput(format!(
            "Plugin directory does not exist: {}",
            source_dir.display()
        )));
    }
    let manifest = PluginManifest::load_from_dir(source_dir)?;
    let target_dir = paths.plugin_dir(&manifest.id);

    // Avoid self-copy when the user selects a folder already inside `plugins_dir`.
    let same_dir = match (source_dir.canonicalize(), target_dir.canonicalize()) {
        (Ok(src), Ok(dst)) => src == dst,
        _ => false,
    };
    if !same_dir {
        if target_dir.exists() {
            std::fs::remove_dir_all(&target_dir)?;
        }
        copy_dir_recursive(source_dir, &target_dir)?;
    }

    db.upsert_plugin(&manifest, true)?;
    Ok(manifest)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> AppResult<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
