use tauri::{AppHandle, Emitter, Manager, State, Webview};

use crate::error::{AppError, AppResult};
use crate::plugins::manager::{
    close_active_plugin, detach_active_plugin, evict_plugin_webview, open_plugin,
    ActivePluginSession,
};
use crate::plugins::permission::{
    plugin_id_from_label, require_host_webview, require_permission,
};
use crate::state::AppState;
use crate::storage::plugin_repo::{InstalledPluginRecord, PluginKvEntry};

#[tauri::command]
pub fn list_plugins(
    webview: Webview,
    state: State<'_, AppState>,
) -> AppResult<Vec<InstalledPluginRecord>> {
    require_host_webview(&webview)?;
    state.db.list_plugins()
}

#[tauri::command]
pub fn set_plugin_enabled(
    plugin_id: String,
    enabled: bool,
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    require_host_webview(&webview)?;
    state.db.set_plugin_enabled(&plugin_id, enabled)?;
    if !enabled {
        let is_active = state
            .active_plugin()
            .is_some_and(|session| session.plugin_id == plugin_id);
        if is_active {
            close_active_plugin(&app)?;
        }
        evict_plugin_webview(&app, &plugin_id);
        if plugin_id == crate::plugins::EVERYTHING_ID {
            crate::commands::everything::close_detached(&app);
        }
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::commands::search::trigger_rebuild_index(&handle).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn open_plugin_command(
    plugin_id: String,
    feature_code: String,
    webview: Webview,
    app: AppHandle,
) -> AppResult<ActivePluginSession> {
    require_host_webview(&webview)?;
    open_plugin(&app, &plugin_id, &feature_code)
}

#[tauri::command]
pub fn close_plugin_command(webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    close_active_plugin(&app)
}

#[tauri::command]
pub async fn detach_plugin_command(webview: Webview, app: AppHandle) -> AppResult<()> {
    require_host_webview(&webview)?;
    detach_active_plugin(&app)
}

#[tauri::command]
pub fn set_plugin_sub_input(
    placeholder: String,
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let plugin_id = plugin_id_from_label(webview.label())?;
    let plugins = state.db.list_plugins()?;
    if !plugins.iter().any(|plugin| plugin.id == plugin_id && plugin.enabled) {
        return Err(AppError::PermissionDenied("Plugin is disabled".into()));
    }
    let mut guard = state.content_mode.lock()
        .map_err(|_| AppError::Platform("Plugin lock poisoned".into()))?;
    let crate::state::ContentMode::Plugin(session) = &mut *guard else {
        return Err(AppError::PermissionDenied("No active plugin session".into()));
    };
    if session.webview_label != webview.label() {
        return Err(AppError::PermissionDenied("Caller does not own the active session".into()));
    }
    session.sub_input_placeholder = placeholder.clone();
    drop(guard);
    let _ = app.emit("triex://sub-input-placeholder", placeholder);
    Ok(())
}

#[tauri::command]
pub fn forward_sub_input_value(
    value: String,
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    require_host_webview(&webview)?;
    let label = state.active_plugin().map(|s| s.webview_label);

    if let Some(label) = label
        && let Some(wv) = app.get_webview(&label)
    {
        let payload = serde_json::to_string(&value)?;
        let script = format!(
            "window.dispatchEvent(new CustomEvent('triex:sub-input-change', {{ detail: {payload} }}));"
        );
        let _ = wv.eval(&script);
    }
    Ok(())
}

#[tauri::command]
pub fn forward_sub_input_keydown(
    key: String,
    shift_key: bool,
    webview: Webview,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    require_host_webview(&webview)?;
    let label = state.active_plugin().map(|s| s.webview_label);

    if let Some(label) = label
        && let Some(wv) = app.get_webview(&label)
    {
        let detail = serde_json::json!({
            "key": key,
            "shiftKey": shift_key,
        });
        let script = format!(
            "window.dispatchEvent(new CustomEvent('triex:sub-input-keydown', {{ detail: {detail} }}));"
        );
        let _ = wv.eval(&script);
    }
    Ok(())
}

#[tauri::command]
pub fn plugin_db_get(
    key: String,
    webview: Webview,
    state: State<'_, AppState>,
) -> AppResult<Option<PluginKvEntry>> {
    let plugin_id = plugin_id_from_label(webview.label())?;
    require_permission(&state.db, plugin_id, "db")?;
    state.db.plugin_kv_get(plugin_id, &key)
}

#[tauri::command]
pub fn plugin_db_put(
    key: String,
    value: serde_json::Value,
    webview: Webview,
    state: State<'_, AppState>,
) -> AppResult<PluginKvEntry> {
    let plugin_id = plugin_id_from_label(webview.label())?;
    require_permission(&state.db, plugin_id, "db")?;
    state.db.plugin_kv_put(plugin_id, &key, &value)
}

#[tauri::command]
pub fn plugin_db_delete(
    key: String,
    webview: Webview,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let plugin_id = plugin_id_from_label(webview.label())?;
    require_permission(&state.db, plugin_id, "db")?;
    state.db.plugin_kv_delete(plugin_id, &key)
}
