use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use notify::RecommendedWatcher;

use crate::commands::updater::UpdateMetadata;
use crate::config::{AppPaths, AppSettings};
use crate::error::AppResult;
use crate::plugins::backend::PluginBackendManager;
use crate::plugins::manager::ActivePluginSession;
use crate::search::SearchEngine;
use crate::storage::plugin_repo::InstalledPluginRecord;
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

/// RAII guard that suppresses hide-on-blur while a native modal (context menu or file dialog)
/// is active, guaranteeing cleanup even on early return or panic.
pub struct BlurSuppressGuard {
    counter: Arc<AtomicUsize>,
}

impl Drop for BlurSuppressGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
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
    suppress_blur_count: Arc<AtomicUsize>,
    pub context_menu_actions: Mutex<HashMap<String, ContextMenuAction>>,
    pub cached_plugin_webviews: Mutex<HashMap<String, CachedPluginWebview>>,
    pub plugins_cache: RwLock<HashMap<String, InstalledPluginRecord>>,
    pub plugin_backends: PluginBackendManager,
    pub dir_watcher: Mutex<Option<RecommendedWatcher>>,
    pub pending_update: tokio::sync::Mutex<Option<tauri_plugin_updater::Update>>,
    pub cached_update_meta: Mutex<Option<UpdateMetadata>>,
}

impl AppState {
    pub fn new(paths: AppPaths, db: Database, settings: AppSettings) -> Self {
        let initial_plugins: HashMap<String, InstalledPluginRecord> = db
            .list_plugins()
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect();

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
            suppress_blur_count: Arc::new(AtomicUsize::new(0)),
            context_menu_actions: Mutex::new(HashMap::new()),
            cached_plugin_webviews: Mutex::new(HashMap::new()),
            plugins_cache: RwLock::new(initial_plugins),
            plugin_backends: PluginBackendManager::new(),
            dir_watcher: Mutex::new(None),
            pending_update: tokio::sync::Mutex::new(None),
            cached_update_meta: Mutex::new(None),
        }
    }

    /// Acquires an RAII guard that suppresses hide-on-blur while dropped.
    pub fn suppress_blur_guard(&self) -> BlurSuppressGuard {
        self.suppress_blur_count.fetch_add(1, Ordering::SeqCst);
        BlurSuppressGuard {
            counter: Arc::clone(&self.suppress_blur_count),
        }
    }

    pub fn is_blur_suppressed(&self) -> bool {
        self.suppress_blur_count.load(Ordering::SeqCst) > 0
    }

    /// Refreshes the in-memory plugin metadata cache from SQLite and returns the sorted list.
    pub fn refresh_plugins_cache(&self) -> AppResult<Vec<InstalledPluginRecord>> {
        let list = self.db.list_plugins()?;
        if let Ok(mut guard) = self.plugins_cache.write() {
            guard.clear();
            for item in &list {
                guard.insert(item.id.clone(), item.clone());
            }
        }
        Ok(list)
    }

    /// O(1) in-memory lookup for an enabled plugin record without hitting SQLite.
    pub fn get_enabled_plugin(&self, plugin_id: &str) -> Option<InstalledPluginRecord> {
        self.plugins_cache
            .read()
            .ok()
            .and_then(|map| map.get(plugin_id).filter(|p| p.enabled).cloned())
    }

    /// Returns cached plugin records sorted by display name.
    pub fn list_cached_plugins(&self) -> Vec<InstalledPluginRecord> {
        let mut list: Vec<InstalledPluginRecord> = self
            .plugins_cache
            .read()
            .map(|map| map.values().cloned().collect())
            .unwrap_or_default();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    pub fn settings_open(&self) -> bool {
        self.content_mode
            .lock()
            .map(|mode| matches!(*mode, ContentMode::Settings))
            .unwrap_or(false)
    }

    pub fn set_settings_open(&self, open: bool) -> bool {
        let mut mode = self
            .content_mode
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let was_open = matches!(*mode, ContentMode::Settings);
        if open {
            *mode = ContentMode::Settings;
        } else if was_open {
            *mode = ContentMode::Search;
        }
        was_open
    }

    pub fn active_plugin(&self) -> Option<ActivePluginSession> {
        self.content_mode.lock().ok().and_then(|mode| match &*mode {
            ContentMode::Plugin(session) => Some(session.clone()),
            _ => None,
        })
    }

    pub fn take_active_plugin(&self) -> Option<ActivePluginSession> {
        let mut mode = self
            .content_mode
            .lock()
            .unwrap_or_else(|err| err.into_inner());
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
