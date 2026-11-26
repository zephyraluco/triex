use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

use notify::RecommendedWatcher;

use crate::commands::updater::UpdateMetadata;
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
    ToggleDashboard,
    RebuildIndex,
    OpenSettings,
    QuitApp,
}

/// Tracks a kept-alive child plugin webview and its idle-eviction generation.
#[derive(Debug, Clone)]
pub struct CachedPluginWebview {
    pub label: String,
    pub epoch: u64,
}

#[derive(Debug, Clone, Default)]
pub enum ContentMode {
    #[default]
    Search,
    Settings,
    Plugin(ActivePluginSession),
}

/// Shared backend state registered via `tauri::Builder::manage`.
pub struct AppState {
    pub paths: AppPaths,
    pub db: Database,
    pub search: Mutex<SearchEngine>,
    pub settings: RwLock<AppSettings>,
    pub settings_update: Mutex<()>,
    pub index_rebuild: tokio::sync::Mutex<u64>,
    pub index_requested: AtomicU64,
    pub prev_foreground_hwnd: AtomicIsize,
    pub is_shown: AtomicBool,
    pub visibility_epoch: AtomicU64,
    pub content_mode: Mutex<ContentMode>,
    pub settings_webview_label: Mutex<Option<String>>,
    pub context_menu_open: AtomicBool,
    pub context_menu_actions: Mutex<HashMap<String, ContextMenuAction>>,
    pub cached_plugin_webviews: Mutex<HashMap<String, CachedPluginWebview>>,
    pub dir_watcher: Mutex<Option<RecommendedWatcher>>,
    pub pending_update: tokio::sync::Mutex<Option<tauri_plugin_updater::Update>>,
    pub cached_update_meta: Mutex<Option<UpdateMetadata>>,
}

impl AppState {
    pub fn new(paths: AppPaths, db: Database, settings: AppSettings) -> Self {
        Self {
            paths,
            db,
            search: Mutex::new(SearchEngine::new()),
            settings: RwLock::new(settings),
            settings_update: Mutex::new(()),
            index_rebuild: tokio::sync::Mutex::new(0),
            index_requested: AtomicU64::new(0),
            prev_foreground_hwnd: AtomicIsize::new(0),
            is_shown: AtomicBool::new(false),
            visibility_epoch: AtomicU64::new(0),
            content_mode: Mutex::new(ContentMode::Search),
            settings_webview_label: Mutex::new(None),
            context_menu_open: AtomicBool::new(false),
            context_menu_actions: Mutex::new(HashMap::new()),
            cached_plugin_webviews: Mutex::new(HashMap::new()),
            dir_watcher: Mutex::new(None),
            pending_update: tokio::sync::Mutex::new(None),
            cached_update_meta: Mutex::new(None),
        }
    }

    pub fn settings_open(&self) -> bool {
        self.content_mode.lock().map(|mode| matches!(*mode, ContentMode::Settings)).unwrap_or(false)
    }

    pub fn set_settings_open(&self, open: bool) -> bool {
        let mut mode = self.content_mode.lock().unwrap_or_else(|err| err.into_inner());
        let was_open = matches!(*mode, ContentMode::Settings);
        if open { *mode = ContentMode::Settings; }
        else if was_open { *mode = ContentMode::Search; }
        was_open
    }

    pub fn active_plugin(&self) -> Option<ActivePluginSession> {
        self.content_mode.lock().ok().and_then(|mode| match &*mode {
            ContentMode::Plugin(session) => Some(session.clone()),
            _ => None,
        })
    }

    pub fn take_active_plugin(&self) -> Option<ActivePluginSession> {
        let mut mode = self.content_mode.lock().unwrap_or_else(|err| err.into_inner());
        if matches!(*mode, ContentMode::Plugin(_))
            && let ContentMode::Plugin(session) = std::mem::take(&mut *mode)
        {
            return Some(session);
        }
        None
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
