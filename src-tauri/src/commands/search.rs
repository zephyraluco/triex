use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, Webview};

use crate::error::{AppError, AppResult};
use crate::platform::scanner::{
    scan_all_items, warm_missing_icons, watcher::spawn_directory_watcher,
    win32::start_menu_directories,
};
use crate::plugins::{permission::require_host_webview, sync_installed_plugins};
use crate::search::index::SearchResultItem;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub request_id: u64,
    pub items: Vec<SearchResultItem>,
    pub total_indexed: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub total_indexed: usize,
}

#[tauri::command]
pub fn search_items(
    query: String,
    request_id: u64,
    limit: Option<usize>,
    webview: Webview,
    state: State<'_, AppState>,
) -> AppResult<SearchResponse> {
    require_host_webview(&webview)?;
    let settings = state.current_settings();
    let engine = state
        .search
        .lock()
        .map_err(|_| AppError::Platform("Search engine mutex poisoned".to_string()))?;

    let items = engine.search(&query, limit, &settings.pinned_items);
    let total_indexed = engine.len();

    Ok(SearchResponse {
        request_id,
        items,
        total_indexed,
    })
}

#[tauri::command]
pub async fn rebuild_index(webview: Webview, app: AppHandle) -> AppResult<IndexStatus> {
    require_host_webview(&webview)?;
    trigger_rebuild_index(&app).await
}

/// Coalesces concurrent rebuild requests, publishes the new in-memory search index immediately,
/// and warms any missing application PNG icons asynchronously in the background.
pub async fn trigger_rebuild_index(app: &AppHandle) -> AppResult<IndexStatus> {
    let state = app.state::<AppState>();
    let request = state.index_requested.fetch_add(1, Ordering::SeqCst) + 1;
    let mut completed = state.index_rebuild.lock().await;
    if *completed >= request {
        let total_indexed = state
            .search
            .lock()
            .map_err(|_| AppError::Platform("Search lock poisoned".into()))?
            .len();
        return Ok(IndexStatus { total_indexed });
    }

    loop {
        let generation = state.index_requested.load(Ordering::SeqCst);
        let settings = state.current_settings();
        let paths = state.paths.clone();
        let handle = app.clone();

        let (mut replacement, scanned, snapshot_settings) =
            tokio::task::spawn_blocking(move || -> AppResult<_> {
                let state = handle.state::<AppState>();
                sync_installed_plugins(&paths, &state.db)?;
                let plugins = state.db.list_plugins()?;
                let scanned = scan_all_items(&settings.custom_scan_dirs, &plugins);
                let usage = state.db.load_all_usage()?;
                let mut engine = crate::search::SearchEngine::new();
                engine.rebuild(scanned.clone(), &usage, &settings.excluded_apps);
                Ok((engine, scanned, settings))
            })
            .await
            .map_err(|e| AppError::Platform(format!("Index scan failed: {e}")))??;

        let current_settings = state.current_settings();
        if generation != state.index_requested.load(Ordering::SeqCst)
            || snapshot_settings.custom_scan_dirs != current_settings.custom_scan_dirs
            || snapshot_settings.excluded_apps != current_settings.excluded_apps
        {
            continue;
        }

        let total_indexed = {
            let mut current = state
                .search
                .lock()
                .map_err(|_| AppError::Platform("Search lock poisoned".into()))?;
            replacement.refresh_usage(&state.db.load_all_usage()?);
            let count = replacement.len();
            *current = replacement;
            count
        };

        if let Err(err) = state.db.save_cached_items(&scanned) {
            tracing::warn!("Failed to persist app cache: {err}");
        }

        *completed = generation;
        let status = IndexStatus { total_indexed };
        let _ = app.emit("triex://index-updated", &status);

        // Warm missing PNG icons off the critical index-swap path.
        let icon_paths = state.paths.clone();
        let icon_app = app.clone();
        let icon_status = status.clone();
        tauri::async_runtime::spawn(async move {
            let extracted =
                tokio::task::spawn_blocking(move || warm_missing_icons(&scanned, &icon_paths))
                    .await
                    .unwrap_or(0);
            if extracted > 0 {
                let _ = icon_app.emit("triex://index-updated", &icon_status);
            }
        });

        return Ok(status);
    }
}

pub fn refresh_directory_watcher(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let mut dirs = start_menu_directories(&state.current_settings().custom_scan_dirs);
    if state.paths.plugins_dir.is_dir() && !dirs.contains(&state.paths.plugins_dir) {
        dirs.push(state.paths.plugins_dir.clone());
    }
    let handle = app.clone();
    let watcher = spawn_directory_watcher(dirs, move || {
        let handle = handle.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = trigger_rebuild_index(&handle).await {
                tracing::error!("Directory index refresh failed: {err}");
            }
        });
    });
    *state
        .dir_watcher
        .lock()
        .map_err(|_| AppError::Platform("Watcher lock poisoned".into()))? = watcher;
    Ok(())
}
