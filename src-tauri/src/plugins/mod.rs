pub mod backend;
pub mod manager;
pub mod manifest;
pub mod permission;

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use zip::ZipArchive;

use crate::config::AppPaths;
use crate::error::{AppError, AppResult};
use crate::plugins::manifest::PluginManifest;
use crate::storage::Database;

/// Supported plugin package archive extensions (underlying standard ZIP format).
pub const PLUGIN_PACKAGE_EXT: &str = "txz";
const MAX_ARCHIVE_FILES: usize = 2_000;
const MAX_ARCHIVE_BYTES: u64 = 100 * 1024 * 1024; // 100 MiB
static TEMP_EXTRACT_SEQ: AtomicU64 = AtomicU64::new(1);

/// Returns true if the path has a supported plugin package archive extension (`.txz` or `.zip`).
pub fn is_supported_package_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(PLUGIN_PACKAGE_EXT) || ext.eq_ignore_ascii_case("zip"))
}

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
            if !found_ids.contains(&record.id)
                && matches!(paths.plugin_dir(&record.id).try_exists(), Ok(false))
            {
                let _ = db.remove_plugin(&record.id);
            }
        }
    }

    Ok(synced)
}

/// Reads and validates `plugin.json` from either a local directory or a `.txz` / `.zip` archive
/// without extracting the full package, allowing the host to inspect metadata or stop an existing
/// instance before overwriting files.
pub fn inspect_plugin_manifest(source_path: &Path) -> AppResult<PluginManifest> {
    if source_path.is_dir() {
        return PluginManifest::load_from_dir(source_path);
    }
    if source_path.is_file() && is_supported_package_path(source_path) {
        let file = File::open(source_path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| AppError::Plugin(format!("Invalid plugin archive: {e}")))?;
        let manifest_entry = find_archive_manifest_entry(&mut archive)?;
        let mut entry = archive
            .by_name(&manifest_entry)
            .map_err(|e| AppError::Plugin(format!("Cannot read plugin.json in archive: {e}")))?;
        if entry.size() > 1024 * 1024 {
            return Err(AppError::Plugin("plugin.json exceeds 1 MiB limit".into()));
        }
        let mut raw = String::new();
        entry.read_to_string(&mut raw)?;
        let manifest: PluginManifest = serde_json::from_str(&raw)?;
        manifest.validate()?;
        return Ok(manifest);
    }
    Err(AppError::InvalidInput(format!(
        "Unsupported plugin source (expected directory, .txz, or .zip): {}",
        source_path.display()
    )))
}

/// Installs a plugin from either a local directory or a `.txz` / `.zip` package archive.
pub fn install_plugin_from_path(
    source_path: &Path,
    paths: &AppPaths,
    db: &Database,
) -> AppResult<PluginManifest> {
    if source_path.is_dir() {
        return install_plugin_from_dir(source_path, paths, db);
    }
    if source_path.is_file() && is_supported_package_path(source_path) {
        return install_plugin_from_package(source_path, paths, db);
    }
    Err(AppError::InvalidInput(format!(
        "Plugin path must be a directory or a .txz / .zip archive: {}",
        source_path.display()
    )))
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

/// Safely extracts a `.txz` or `.zip` plugin archive into a staging directory, validates
/// `plugin.json`, atomically replaces `%LocalAppData%\triex\plugins\<id>`, and registers it.
pub fn install_plugin_from_package(
    archive_path: &Path,
    paths: &AppPaths,
    db: &Database,
) -> AppResult<PluginManifest> {
    if !archive_path.is_file() || !is_supported_package_path(archive_path) {
        return Err(AppError::InvalidInput(format!(
            "Expected a .txz or .zip plugin package: {}",
            archive_path.display()
        )));
    }

    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| AppError::Plugin(format!("Failed to open plugin package: {e}")))?;

    if archive.len() > MAX_ARCHIVE_FILES {
        return Err(AppError::Plugin(format!(
            "Plugin package contains too many files (max {MAX_ARCHIVE_FILES})"
        )));
    }

    let manifest_entry = find_archive_manifest_entry(&mut archive)?;
    let strip_prefix: Option<PathBuf> = Path::new(&manifest_entry)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf);

    let seq = TEMP_EXTRACT_SEQ.fetch_add(1, Ordering::Relaxed);
    let staging_dir = paths
        .data_dir
        .join(format!(".plugin-staging-{}-{seq}", std::process::id()));
    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }
    std::fs::create_dir_all(&staging_dir)?;

    let extract_res = (|| -> AppResult<PluginManifest> {
        let mut total_written: u64 = 0;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| AppError::Plugin(format!("Corrupt archive entry: {e}")))?;
            let Some(enclosed) = entry.enclosed_name() else {
                let raw_name = entry.name().map(|s| s.into_owned()).unwrap_or_default();
                return Err(AppError::Plugin(format!(
                    "Rejected unsafe archive entry path: {raw_name}"
                )));
            };

            let rel_path = if let Some(prefix) = &strip_prefix {
                match enclosed.strip_prefix(prefix) {
                    Ok(stripped) => stripped.to_path_buf(),
                    Err(_) => continue,
                }
            } else {
                enclosed
            };

            if rel_path.as_os_str().is_empty() {
                continue;
            }
            if rel_path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
            {
                return Err(AppError::Plugin(format!(
                    "Rejected unsafe relative path in package: {}",
                    rel_path.display()
                )));
            }

            let out_path = staging_dir.join(&rel_path);
            if entry.is_dir() {
                std::fs::create_dir_all(&out_path)?;
                continue;
            }

            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut out_file = File::create(&out_path)?;
            let mut buf = [0u8; 16 * 1024];
            loop {
                let n = entry.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                total_written = total_written.saturating_add(n as u64);
                if total_written > MAX_ARCHIVE_BYTES {
                    return Err(AppError::Plugin(
                        "Plugin package exceeds maximum uncompressed size (100 MiB)".into(),
                    ));
                }
                std::io::Write::write_all(&mut out_file, &buf[..n])?;
            }
        }

        let manifest = PluginManifest::load_from_dir(&staging_dir)?;
        let target_dir = paths.plugin_dir(&manifest.id);
        if target_dir.exists() {
            std::fs::remove_dir_all(&target_dir)?;
        }
        std::fs::rename(&staging_dir, &target_dir).or_else(|_| -> AppResult<()> {
            copy_dir_recursive(&staging_dir, &target_dir)?;
            let _ = std::fs::remove_dir_all(&staging_dir);
            Ok(())
        })?;

        db.upsert_plugin(&manifest, true)?;
        Ok(manifest)
    })();

    if extract_res.is_err() && staging_dir.exists() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }
    extract_res
}

/// Locates `plugin.json` either at the archive root or inside a single top-level wrapper folder.
fn find_archive_manifest_entry(archive: &mut ZipArchive<File>) -> AppResult<String> {
    let mut root_hit: Option<String> = None;
    let mut nested_hits: Vec<String> = Vec::new();

    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| AppError::Plugin(format!("Failed to inspect archive entry: {e}")))?;
        let Some(enclosed) = entry.enclosed_name() else {
            let raw_name = entry.name().map(|s| s.into_owned()).unwrap_or_default();
            return Err(AppError::Plugin(format!(
                "Rejected unsafe archive entry path: {raw_name}"
            )));
        };
        if entry.is_dir() {
            continue;
        }
        let comps: Vec<_> = enclosed.components().collect();
        if comps.len() == 1 && enclosed.file_name().and_then(|n| n.to_str()) == Some("plugin.json") {
            let name = entry
                .name()
                .map_err(|e| AppError::Plugin(format!("Invalid archive entry name: {e}")))?
                .into_owned();
            root_hit = Some(name);
            break;
        }
        if comps.len() == 2 && enclosed.file_name().and_then(|n| n.to_str()) == Some("plugin.json") {
            let name = entry
                .name()
                .map_err(|e| AppError::Plugin(format!("Invalid archive entry name: {e}")))?
                .into_owned();
            nested_hits.push(name);
        }
    }

    if let Some(hit) = root_hit {
        return Ok(hit);
    }
    if nested_hits.len() == 1 {
        return Ok(nested_hits.remove(0));
    }
    Err(AppError::Plugin(
        "Archive must contain a valid plugin.json at its root (or inside a single top-level directory)".into(),
    ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn make_temp_paths(name: &str) -> AppPaths {
        let dir = std::env::temp_dir().join(format!(
            "triex-plugin-pkg-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        AppPaths::resolve(dir).unwrap()
    }

    #[test]
    fn should_install_txz_and_zip_packages_including_nested_root() {
        let paths = make_temp_paths("txz-nested");
        let db = Database::open(&paths.db_path).unwrap();
        let pkg_path = paths.data_dir.join("demo-calc.txz");

        {
            let f = File::create(&pkg_path).unwrap();
            let mut writer = ZipWriter::new(f);
            let opts = SimpleFileOptions::default();
            writer.start_file("demo-calc/plugin.json", opts).unwrap();
            writer
                .write_all(
                    br#"{"id":"demo-calc","name":"Demo Calc","version":"1.0.0","entry":"index.html"}"#,
                )
                .unwrap();
            writer.start_file("demo-calc/index.html", opts).unwrap();
            writer.write_all(b"<h1>Demo</h1>").unwrap();
            writer.finish().unwrap();
        }

        let preview = inspect_plugin_manifest(&pkg_path).unwrap();
        assert_eq!(preview.id, "demo-calc");

        let installed = install_plugin_from_path(&pkg_path, &paths, &db).unwrap();
        assert_eq!(installed.id, "demo-calc");
        assert!(paths.plugin_dir("demo-calc").join("index.html").is_file());

        let _ = std::fs::remove_dir_all(&paths.data_dir);
    }

    #[test]
    fn should_reject_path_traversal_in_archive() {
        let paths = make_temp_paths("txz-evil");
        let db = Database::open(&paths.db_path).unwrap();
        let pkg_path = paths.data_dir.join("evil.zip");

        {
            let f = File::create(&pkg_path).unwrap();
            let mut writer = ZipWriter::new(f);
            let opts = SimpleFileOptions::default();
            writer.start_file("plugin.json", opts).unwrap();
            writer
                .write_all(br#"{"id":"evil","name":"Evil","version":"1.0.0"}"#)
                .unwrap();
            writer.start_file("../escape.txt", opts).unwrap();
            writer.write_all(b"nope").unwrap();
            writer.finish().unwrap();
        }

        assert!(install_plugin_from_path(&pkg_path, &paths, &db).is_err());
        assert!(!paths.plugin_dir("evil").exists());

        let _ = std::fs::remove_dir_all(&paths.data_dir);
    }
}
