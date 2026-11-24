use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{AppError, AppResult};
use crate::platform::scanner::scan_all_items;
use crate::plugins::sync_installed_plugins;
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
    state: State<'_, AppState>,
) -> AppResult<SearchResponse> {
    let settings = state.current_settings();
    let max_results = limit.unwrap_or(settings.max_results);
    let mut engine = state
        .search
        .lock()
        .map_err(|_| AppError::Platform("Search engine mutex poisoned".to_string()))?;

    let items = engine.search(&query, max_results, &settings.pinned_items);
    let total_indexed = engine.len();

    Ok(SearchResponse {
        request_id,
        items,
        total_indexed,
    })
}

#[tauri::command]
pub async fn rebuild_index(app: AppHandle) -> AppResult<IndexStatus> {
    trigger_rebuild_index(&app).await
}

/// Runs a full Windows app/UWP/plugin scan off the main thread, persists cache, and swaps memory index.
pub async fn trigger_rebuild_index(app: &AppHandle) -> AppResult<IndexStatus> {
    let state = app.state::<AppState>();
    let settings = state.current_settings();
    let paths = state.paths.clone();
    let _ = sync_installed_plugins(&paths, &state.db);
    let plugins = state.db.list_plugins().unwrap_or_default();

    let scanned = tokio::task::spawn_blocking(move || {
        scan_all_items(&settings.custom_scan_dirs, &plugins, &paths)
    })
    .await
    .map_err(|e| AppError::Platform(format!("Index scan task failed: {e}")))?;

    let state = app.state::<AppState>();
    if let Err(err) = state.db.save_cached_items(&scanned) {
        tracing::warn!("Failed to persist scanned app cache: {err}");
    }

    let usage_map = state.db.load_all_usage().unwrap_or_default();
    let excluded = state.current_settings().excluded_apps;

    let total_indexed = {
        let mut engine = state
            .search
            .lock()
            .map_err(|_| AppError::Platform("Search engine mutex poisoned".to_string()))?;
        engine.rebuild(scanned, &usage_map, &excluded);
        engine.len()
    };

    let status = IndexStatus { total_indexed };
    let _ = app.emit("triex://index-updated", &status);
    Ok(status)
}
