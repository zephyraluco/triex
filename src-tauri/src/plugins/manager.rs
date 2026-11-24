use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::error::{AppError, AppResult};
use crate::platform::window::set_logical_height;
use crate::state::AppState;

const PLUGIN_BRIDGE_JS: &str = include_str!("../../resources/plugin-bridge.js");
const LAUNCHER_WIDTH: f64 = 800.0;
const HEADER_HEIGHT: f64 = 64.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivePluginSession {
    pub plugin_id: String,
    pub plugin_name: String,
    pub feature_code: String,
    pub webview_label: String,
    pub sub_input_placeholder: String,
}

/// Mounts a plugin UI as a sandboxed child webview inside the main launcher window.
pub fn open_plugin(
    app: &AppHandle,
    plugin_id: &str,
    feature_code: &str,
) -> AppResult<ActivePluginSession> {
    let state = app.state::<AppState>();
    let plugins = state.db.list_plugins()?;
    let plugin = plugins
        .into_iter()
        .find(|p| p.id == plugin_id && p.enabled)
        .ok_or_else(|| AppError::NotFound(format!("Plugin '{plugin_id}' not found or disabled")))?;

    close_active_plugin(app)?;

    let main_window = app
        .get_window("main")
        .ok_or_else(|| AppError::Platform("Main window not available".to_string()))?;

    let plugin_height = plugin.manifest.height.clamp(240.0, 720.0);
    if let Some(main_wv_win) = app.get_webview_window("main") {
        set_logical_height(&main_wv_win, LAUNCHER_WIDTH, HEADER_HEIGHT + plugin_height);
    }

    let label = format!("plugin-{}", plugin.id);
    let url_str = format!(
        "plugin://{}/{}",
        plugin.id,
        plugin.manifest.entry.trim_start_matches('/')
    );
    let parsed_url: tauri::Url = url_str
        .parse()
        .map_err(|e| AppError::Plugin(format!("Invalid plugin URL '{url_str}': {e}")))?;

    let builder = WebviewBuilder::new(&label, WebviewUrl::External(parsed_url))
        .initialization_script(PLUGIN_BRIDGE_JS);

    let _child = main_window.add_child(
        builder,
        LogicalPosition::new(0.0, HEADER_HEIGHT),
        LogicalSize::new(LAUNCHER_WIDTH, plugin_height),
    )?;

    let session = ActivePluginSession {
        plugin_id: plugin.id.clone(),
        plugin_name: plugin.name.clone(),
        feature_code: feature_code.to_string(),
        webview_label: label,
        sub_input_placeholder: format!("在 {} 中输入...", plugin.name),
    };

    if let Ok(mut guard) = state.active_plugin.lock() {
        *guard = Some(session.clone());
    }

    let _ = app.emit("triex://plugin-opened", &session);
    Ok(session)
}

/// Closes any currently embedded child plugin webview and returns the launcher to search mode.
pub fn close_active_plugin(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let existing = state
        .active_plugin
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());

    if let Some(session) = existing {
        if let Some(wv) = app.get_webview(&session.webview_label) {
            let _ = wv.close();
        }
        let _ = app.emit("triex://plugin-closed", &session.plugin_id);
    }

    Ok(())
}

/// Detaches the active plugin into its own independent OS window.
pub fn detach_active_plugin(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let existing = state
        .active_plugin
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .ok_or_else(|| AppError::NotFound("No active plugin to detach".to_string()))?;

    let plugins = state.db.list_plugins()?;
    let plugin = plugins
        .into_iter()
        .find(|p| p.id == existing.plugin_id)
        .ok_or_else(|| AppError::NotFound(format!("Plugin '{}' not found", existing.plugin_id)))?;

    close_active_plugin(app)?;

    let detached_label = format!("plugin-detached-{}", plugin.id);
    if let Some(win) = app.get_webview_window(&detached_label) {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    let url_str = format!(
        "plugin://{}/{}",
        plugin.id,
        plugin.manifest.entry.trim_start_matches('/')
    );
    let parsed_url: tauri::Url = url_str
        .parse()
        .map_err(|e| AppError::Plugin(format!("Invalid plugin URL '{url_str}': {e}")))?;

    WebviewWindowBuilder::new(app, &detached_label, WebviewUrl::External(parsed_url))
        .title(&plugin.name)
        .inner_size(LAUNCHER_WIDTH, plugin.manifest.height.clamp(360.0, 800.0))
        .initialization_script(PLUGIN_BRIDGE_JS)
        .center()
        .build()?;

    Ok(())
}
