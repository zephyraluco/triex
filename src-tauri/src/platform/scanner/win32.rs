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

/// Scans Start Menu `.lnk` shortcuts and direct `.exe` files from configured directories.
pub fn scan_win32_apps(custom_dirs: &[String]) -> Vec<RawSearchItem> {
    let dirs = start_menu_directories(custom_dirs);
    let mut results = Vec::new();

    #[cfg(windows)]
    let _com = ComGuard::new();

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

            match ext.as_deref() {
                Some("lnk") => {
                    if let Some(item) = parse_lnk_item(path) {
                        results.push(item);
                    }
                }
                Some("exe") => {
                    if let Some(item) = build_exe_item(path) {
                        results.push(item);
                    }
                }
                _ => {}
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

fn build_exe_item(path: &Path) -> Option<RawSearchItem> {
    let stem = path.file_stem()?.to_str()?.trim().to_string();
    if stem.is_empty() || is_noise_shortcut(&stem) {
        return None;
    }
    let path_str = path.to_string_lossy().to_string();
    let icon_key = hash_icon_key(&path_str);
    Some(RawSearchItem {
        id: format!("app:{icon_key}"),
        kind: ItemKind::App,
        name: stem.clone(),
        subtitle: path_str.clone(),
        exec_path: path_str,
        exec_args: String::new(),
        working_dir: path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        icon_key,
        keywords: derive_keywords(&stem, path),
    })
}

fn parse_lnk_item(lnk_path: &Path) -> Option<RawSearchItem> {
    let stem = lnk_path.file_stem()?.to_str()?.trim().to_string();
    if stem.is_empty() || is_noise_shortcut(&stem) {
        return None;
    }

    let resolved = resolve_shortcut(lnk_path);
    let lnk_str = lnk_path.to_string_lossy().to_string();

    let (target_path, args, working_dir) = match resolved {
        Some(info) if !info.target_path.is_empty() => {
            let target = Path::new(&info.target_path);
            let target_lower = info.target_path.to_lowercase();
            // Skip broken links or uninstaller binaries referenced by innocuous names.
            if target_lower.contains("unins") || target_lower.ends_with(".chm") || target_lower.ends_with(".url") {
                return None;
            }
            if !target.exists() {
                return None;
            }
            (info.target_path, info.arguments, info.working_dir)
        }
        // Keep the .lnk itself as fallback (e.g. MSI advertised shortcuts).
        _ => (lnk_str.clone(), String::new(), String::new()),
    };

    let icon_source = if Path::new(&target_path).exists() {
        &target_path
    } else {
        &lnk_str
    };
    let icon_key = hash_icon_key(icon_source);
    let keywords = derive_keywords(&stem, Path::new(&target_path));

    Some(RawSearchItem {
        id: format!("app:{icon_key}"),
        kind: ItemKind::App,
        name: stem,
        subtitle: target_path.clone(),
        // Launching the `.lnk` preserves Start Menu pinned arguments and icons reliably.
        exec_path: lnk_str,
        exec_args: args,
        working_dir,
        icon_key,
        keywords,
    })
}

fn derive_keywords(name: &str, target: &Path) -> Vec<String> {
    let mut keywords = Vec::new();
    if let Some(file_stem) = target.file_stem().and_then(|s| s.to_str())
        && !file_stem.eq_ignore_ascii_case(name)
        && !file_stem.is_empty()
    {
        keywords.push(file_stem.to_string());
    }
    keywords
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

struct ShortcutInfo {
    target_path: String,
    arguments: String,
    working_dir: String,
}

#[cfg(windows)]
struct ComGuard {
    initialized: bool,
}

#[cfg(windows)]
impl ComGuard {
    fn new() -> Self {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Self {
            initialized: hr.is_ok(),
        }
    }
}

#[cfg(windows)]
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.initialized {
            use windows::Win32::System::Com::CoUninitialize;
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(windows)]
fn resolve_shortcut(lnk_path: &Path) -> Option<ShortcutInfo> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
    use windows::Win32::System::Com::{
        CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    let wide_path: Vec<u16> = lnk_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let persist: IPersistFile = link.cast().ok()?;
        persist
            .Load(PCWSTR(wide_path.as_ptr()), STGM_READ)
            .ok()?;

        let mut target_buf = [0u16; 1024];
        let mut find_data = WIN32_FIND_DATAW::default();
        let _ = link.GetPath(&mut target_buf, &mut find_data, 0);

        let mut args_buf = [0u16; 1024];
        let _ = link.GetArguments(&mut args_buf);

        let mut dir_buf = [0u16; 1024];
        let _ = link.GetWorkingDirectory(&mut dir_buf);

        Some(ShortcutInfo {
            target_path: utf16_z_to_string(&target_buf),
            arguments: utf16_z_to_string(&args_buf),
            working_dir: utf16_z_to_string(&dir_buf),
        })
    }
}

#[cfg(not(windows))]
fn resolve_shortcut(_lnk_path: &Path) -> Option<ShortcutInfo> {
    None
}

#[cfg(windows)]
fn utf16_z_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len]).trim().to_string()
}
