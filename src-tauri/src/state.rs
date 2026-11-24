use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, RwLock};

use notify::RecommendedWatcher;

use crate::config::{AppPaths, AppSettings};
use crate::plugins::manager::ActivePluginSession;
use crate::search::SearchEngine;
use crate::storage::Database;

/// Shared backend state registered via `tauri::Builder::manage`.
pub struct AppState {
    pub paths: AppPaths,
    pub db: Database,
    pub search: Mutex<SearchEngine>,
    pub settings: RwLock<AppSettings>,
    pub prev_foreground_hwnd: AtomicIsize,
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
