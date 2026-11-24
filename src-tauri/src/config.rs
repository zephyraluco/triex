use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::AppResult;

/// Application-wide persistent user settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub hotkey: String,
    pub theme: ThemeMode,
    pub vibrancy: VibrancyMode,
    pub max_results: usize,
    pub launch_on_startup: bool,
    pub hide_on_blur: bool,
    pub custom_scan_dirs: Vec<String>,
    pub excluded_apps: Vec<String>,
    #[serde(default)]
    pub pinned_items: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VibrancyMode {
    Mica,
    Acrylic,
    None,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hotkey: "Alt+Space".to_string(),
            theme: ThemeMode::System,
            vibrancy: VibrancyMode::Mica,
            max_results: 30,
            launch_on_startup: false,
            hide_on_blur: true,
            custom_scan_dirs: Vec::new(),
            excluded_apps: Vec::new(),
            pinned_items: Vec::new(),
        }
    }
}

/// Resolved local data directories under `%LocalAppData%\triex` (or fallback).
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub icons_dir: PathBuf,
    pub plugins_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve(base_dir: PathBuf) -> AppResult<Self> {
        let icons_dir = base_dir.join("icons");
        let plugins_dir = base_dir.join("plugins");
        let db_path = base_dir.join("triex.db");

        std::fs::create_dir_all(&base_dir)?;
        std::fs::create_dir_all(&icons_dir)?;
        std::fs::create_dir_all(&plugins_dir)?;

        Ok(Self {
            data_dir: base_dir,
            db_path,
            icons_dir,
            plugins_dir,
        })
    }

    pub fn icon_file(&self, icon_key: &str) -> PathBuf {
        self.icons_dir.join(format!("{icon_key}.png"))
    }

    pub fn plugin_dir(&self, plugin_id: &str) -> PathBuf {
        self.plugins_dir.join(plugin_id)
    }

    pub fn ensure_within_dir(base: &Path, candidate: &Path) -> bool {
        match (base.canonicalize(), candidate.canonicalize()) {
            (Ok(base_canon), Ok(cand_canon)) => cand_canon.starts_with(base_canon),
            _ => false,
        }
    }
}
