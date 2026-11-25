pub mod system_cmd;
pub mod uwp;
pub mod watcher;
pub mod win32;

use std::collections::HashSet;

use crate::config::AppPaths;
use crate::platform::icon::ensure_icon_cached;
use crate::search::index::{ItemKind, RawSearchItem};
use crate::storage::plugin_repo::InstalledPluginRecord;

/// Performs a full scan across Win32 shortcuts, UWP apps, built-in system commands, and enabled plugins.
/// Icon extraction is intentionally decoupled into [`warm_missing_icons`] so index swaps are never blocked.
pub fn scan_all_items(
    custom_dirs: &[String],
    plugins: &[InstalledPluginRecord],
) -> Vec<RawSearchItem> {
    let mut items = Vec::new();
    let mut seen_names = HashSet::new();

    // 1. Win32 Start Menu & custom dir apps
    for item in win32::scan_win32_apps(custom_dirs) {
        let dedup_key = item.name.to_lowercase();
        if seen_names.insert(dedup_key) {
            items.push(item);
        }
    }

    // 2. UWP / MSIX apps from shell:AppsFolder
    for item in uwp::scan_uwp_apps() {
        let dedup_key = item.name.to_lowercase();
        if seen_names.insert(dedup_key) {
            items.push(item);
        }
    }

    // 3. Built-in system commands
    items.extend(system_cmd::builtin_system_commands());

    // 4. Installed and enabled plugins
    for plugin in plugins.iter().filter(|p| p.enabled) {
        for feature in &plugin.manifest.features {
            items.push(RawSearchItem {
                id: format!("plugin:{}:{}", plugin.id, feature.code),
                kind: ItemKind::Plugin,
                name: feature.explain.clone(),
                subtitle: format!("插件 · {}", plugin.name),
                exec_path: format!("{}:{}", plugin.id, feature.code),
                exec_args: String::new(),
                working_dir: String::new(),
                icon_key: String::new(),
                keywords: feature.cmds.clone(),
            });
        }
    }

    items
}

/// Extracts and caches missing 64x64 PNG icons for scanned items on a background worker thread.
/// Returns the number of newly cached icon files.
pub fn warm_missing_icons(items: &[RawSearchItem], paths: &AppPaths) -> usize {
    let mut extracted = 0usize;
    for item in items {
        if item.icon_key.is_empty() {
            continue;
        }
        let icon_file = paths.icon_file(&item.icon_key);
        if icon_file.exists() {
            continue;
        }
        let source = match item.kind {
            ItemKind::App => item.exec_path.clone(),
            ItemKind::UwpApp => format!("shell:AppsFolder\\{}", item.exec_path),
            _ => continue,
        };
        if ensure_icon_cached(&source, &icon_file).is_ok() {
            extracted += 1;
        }
    }
    extracted
}
