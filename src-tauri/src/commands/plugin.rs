use tauri::{AppHandle, Emitter, Manager, State, Webview};

use crate::error::{AppError, AppResult};
use crate::plugins::manager::{
    close_active_plugin, detach_active_plugin, open_plugin, ActivePluginSession,
};
use crate::plugins::permission::{plugin_id_from_label, require_permission};
use crate::state::AppState;
use crate::storage::plugin_repo::{InstalledPluginRecord, PluginKvEntry};

#[tauri::command]
pub fn list_plugins(state: State<'_, AppState>) -> AppResult<Vec<InstalledPluginRecord>> {
    state.db.list_plugins()
}

#[tauri::command]
pub fn set_plugin_enabled(
    plugin_id: String,
    enabled: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state.db.set_plugin_enabled(&plugin_id, enabled)?;
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::commands::search::trigger_rebuild_index(&handle).await;
    });
    Ok(())
}

#[tauri::command]
pub fn open_plugin_command(
    plugin_id: String,
    feature_code: String,
    app: AppHandle,
) -> AppResult<ActivePluginSession> {
    open_plugin(&app, &plugin_id, &feature_code)
}

#[tauri::command]
pub fn close_plugin_command(app: AppHandle) -> AppResult<()> {
    close_active_plugin(&app)
}

#[tauri::command]
pub fn detach_plugin_command(app: AppHandle) -> AppResult<()> {
    detach_active_plugin(&app)
}

#[tauri::command]
pub fn set_plugin_sub_input(
    placeholder: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    if let Ok(mut guard) = state.active_plugin.lock()
        && let Some(session) = guard.as_mut()
    {
        session.sub_input_placeholder = placeholder.clone();
    }
    let _ = app.emit("triex://sub-input-placeholder", placeholder);
    Ok(())
}

#[tauri::command]
pub fn forward_sub_input_value(value: String, app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let label = state
        .active_plugin
        .lock()
        .map_err(|_| AppError::Platform("Plugin lock poisoned".to_string()))?
        .as_ref()
        .map(|s| s.webview_label.clone());

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
