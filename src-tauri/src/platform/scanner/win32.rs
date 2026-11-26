use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::search::index::{ItemKind, RawSearchItem};

const IGNORED_NAME_FRAGMENTS: &[&str] = &[
    "uninstall",
    "卸载",
    "readme",
    "help",
    "帮助",
    "manual",
    "documentation",
    "release notes",
    "website",
    "visit ",
];

/// Returns standard Windows Start Menu directories plus any user-configured scan directories.
pub fn start_menu_directories(custom_dirs: &[String]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(program_data) = std::env::var("ProgramData") {
        let common = PathBuf::from(program_data)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        if common.is_dir() {
            dirs.push(common);
        }
    }
    if let Ok(app_data) = std::env::var("APPDATA") {
        let user = PathBuf::from(app_data)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        if user.is_dir() {
            dirs.push(user);
        }
    }
    for custom in custom_dirs {
        let path = PathBuf::from(custom.trim());
        if path.is_dir() && !dirs.contains(&path) {
            dirs.push(path);
        }
    }
    dirs
}

/// Scans Start Menu `.lnk` shortcuts and direct `.exe` files from configured directories
/// directly without resolving `.lnk` targets into executable paths.
pub fn scan_win32_apps(custom_dirs: &[String]) -> Vec<RawSearchItem> {
    let dirs = start_menu_directories(custom_dirs);
    let mut results = Vec::new();

    for dir in dirs {
        for entry in WalkDir::new(&dir)
            .max_depth(4)
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase());

            if matches!(ext.as_deref(), Some("lnk" | "exe"))
                && let Some(item) = build_app_item(path)
            {
                results.push(item);
            }
        }
    }

    results
}

fn is_noise_shortcut(name: &str) -> bool {
    let lower = name.to_lowercase();
    IGNORED_NAME_FRAGMENTS
        .iter()
        .any(|frag| lower.contains(frag))
}

fn build_app_item(path: &Path) -> Option<RawSearchItem> {
    let stem = path.file_stem()?.to_str()?.trim().to_string();
    if stem.is_empty() || is_noise_shortcut(&stem) {
        return None;
    }
    let path_str = path.to_string_lossy().to_string();
    let icon_key = hash_icon_key(&path_str);
    Some(RawSearchItem {
        id: format!("app:{icon_key}"),
        kind: ItemKind::App,
        name: stem,
        subtitle: path_str.clone(),
        exec_path: path_str,
        exec_args: String::new(),
        working_dir: path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        icon_key,
        keywords: Vec::new(),
    })
}

pub fn hash_icon_key(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.to_lowercase().as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(16);
    for byte in &digest[..8] {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}
