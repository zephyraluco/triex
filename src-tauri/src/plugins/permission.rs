use crate::error::{AppError, AppResult};
use crate::storage::Database;

/// Extracts the owning plugin ID from a plugin webview label (`plugin-<id>`).
pub fn plugin_id_from_label(label: &str) -> AppResult<&str> {
    label
        .strip_prefix("plugin-")
        .filter(|id| !id.is_empty())
        .ok_or_else(|| {
            AppError::PermissionDenied(format!(
                "Caller webview '{label}' is not a valid plugin webview"
            ))
        })
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
