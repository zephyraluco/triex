use crate::error::{AppError, AppResult};
use crate::storage::Database;

/// Ensures the caller is a trusted host webview (`main` or `settings-view:*`),
/// preventing sandboxed `plugin-*` webviews from invoking privileged host commands.
pub fn require_host_webview(webview: &tauri::Webview) -> AppResult<()> {
    let label = webview.label();
    if label == "main" || label.starts_with("settings-view:") {
        Ok(())
    } else {
        Err(AppError::PermissionDenied(format!(
            "Plugin webview '{label}' is not permitted to invoke host command"
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

/// Verifies that the plugin has explicitly declared the required permission in `plugin.json`.
pub fn require_permission(db: &Database, plugin_id: &str, permission: &str) -> AppResult<()> {
    let plugins = db.list_plugins()?;
    let plugin = plugins
        .into_iter()
        .find(|p| p.id == plugin_id && p.enabled)
        .ok_or_else(|| AppError::NotFound(format!("Enabled plugin '{plugin_id}' not found")))?;

    if plugin.manifest.permissions.iter().any(|p| p == permission) {
        Ok(())
    } else {
        Err(AppError::PermissionDenied(format!(
            "Plugin '{plugin_id}' lacks required permission '{permission}'"
        )))
    }
}
