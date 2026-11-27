use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Returns true if the webview is a trusted host webview (`main` or `settings-view:*`).
pub fn is_host_webview(webview: &tauri::Webview) -> bool {
    let label = webview.label();
    label == "main" || label.starts_with("settings-view:")
}

/// Ensures the caller is a trusted host webview (`main` or `settings-view:*`),
/// preventing sandboxed `plugin-*` webviews from invoking privileged host commands.
pub fn require_host_webview(webview: &tauri::Webview) -> AppResult<()> {
    if is_host_webview(webview) {
        Ok(())
    } else {
        Err(AppError::PermissionDenied(format!(
            "Plugin webview '{}' is not permitted to invoke host command",
            webview.label()
        )))
    }
}

/// Extracts the owning plugin ID from a plugin webview label
/// (`plugin-<id>`, `plugin-<id>:<seq>`, or `plugin-detached-<id>`).
pub fn plugin_id_from_label(label: &str) -> AppResult<&str> {
    let stripped = label
        .strip_prefix("plugin-detached-")
        .or_else(|| label.strip_prefix("plugin-"))
        .unwrap_or("");
    let id = stripped.split(':').next().unwrap_or("");
    if id.is_empty() {
        return Err(AppError::PermissionDenied(format!(
            "Caller webview '{label}' is not a valid plugin webview"
        )));
    }
    Ok(id)
}

/// Verifies in O(1) via the in-memory plugin cache that the plugin is enabled
/// and has explicitly declared the required permission in `plugin.json`.
pub fn require_permission(state: &AppState, plugin_id: &str, permission: &str) -> AppResult<()> {
    let plugin = state
        .get_enabled_plugin(plugin_id)
        .ok_or_else(|| AppError::NotFound(format!("Enabled plugin '{plugin_id}' not found")))?;

    if plugin.manifest.permissions.iter().any(|p| p == permission) {
        Ok(())
    } else {
        Err(AppError::PermissionDenied(format!(
            "Plugin '{plugin_id}' lacks required permission '{permission}'"
        )))
    }
}
