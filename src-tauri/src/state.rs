use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

use notify::RecommendedWatcher;

use crate::config::{AppPaths, AppSettings};
use crate::plugins::manager::ActivePluginSession;
use crate::search::SearchEngine;
use crate::storage::Database;

/// Action bound to a native popup context menu item.
#[derive(Debug, Clone)]
pub enum ContextMenuAction {
    Launch { item_id: String, as_admin: bool },
    TogglePin { item_id: String },
    RemoveRecent { item_id: String },
    OpenFolder { item_id: String },
    CopyPath { text: String },
    ToggleDashboard,
    RebuildIndex,
    OpenSettings,
    QuitApp,
}

/// Shared backend state registered via `tauri::Builder::manage`.
pub struct AppState {
    pub paths: AppPaths,
    pub db: Database,
    pub search: Mutex<SearchEngine>,
    pub settings: RwLock<AppSettings>,
    pub prev_foreground_hwnd: AtomicIsize,
    pub is_shown: AtomicBool,
    pub visibility_epoch: AtomicU64,
    pub in_settings_view: AtomicBool,
    pub context_menu_open: AtomicBool,
    pub context_menu_actions: Mutex<HashMap<String, ContextMenuAction>>,
    pub active_plugin: Mutex<Option<ActivePluginSession>>,
    pub dir_watcher: Mutex<Option<RecommendedWatcher>>,
}

impl AppState {
    pub fn new(paths: AppPaths, db: Database, settings: AppSettings) -> Self {
        Self {
            paths,
            db,
            search: Mutex::new(SearchEngine::new()),
            settings: RwLock::new(settings),
            prev_foreground_hwnd: AtomicIsize::new(0),
            is_shown: AtomicBool::new(true),
            visibility_epoch: AtomicU64::new(0),
            in_settings_view: AtomicBool::new(false),
            context_menu_open: AtomicBool::new(false),
            context_menu_actions: Mutex::new(HashMap::new()),
            active_plugin: Mutex::new(None),
            dir_watcher: Mutex::new(None),
        }
    }

    pub fn current_settings(&self) -> AppSettings {
        self.settings
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn store_prev_hwnd(&self, hwnd: isize) {
        if hwnd != 0 {
            self.prev_foreground_hwnd.store(hwnd, Ordering::Relaxed);
        }
    }

    pub fn take_prev_hwnd(&self) -> isize {
        self.prev_foreground_hwnd.swap(0, Ordering::Relaxed)
    }
}
