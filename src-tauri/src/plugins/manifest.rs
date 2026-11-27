use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Declarative manifest (`plugin.json`) placed at the root of each installed plugin package.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default = "default_entry")]
    pub entry: String,
    #[serde(default = "default_height")]
    pub height: f64,
    #[serde(default)]
    pub backend: Option<PluginBackendConfig>,
    #[serde(default)]
    pub features: Vec<PluginFeature>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// Optional managed backend service specification for plugins that pair a Web UI with
/// an independent background process (communicating via JSON-RPC 2.0 over stdio).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginBackendConfig {
    #[serde(default = "default_backend_kind")]
    pub kind: BackendKind,
    /// Relative path inside the plugin directory (e.g. `backend/server.exe`).
    pub entry: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackendKind {
    Process,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginFeature {
    pub code: String,
    pub explain: String,
    #[serde(default)]
    pub cmds: Vec<String>,
}

fn default_entry() -> String {
    "index.html".to_string()
}

fn default_height() -> f64 {
    500.0
}

fn default_backend_kind() -> BackendKind {
    BackendKind::Process
}

fn default_idle_timeout_secs() -> u64 {
    60
}

impl PluginManifest {
    pub fn load_from_dir(plugin_dir: &Path) -> AppResult<Self> {
        let path = plugin_dir.join("plugin.json");
        let raw = std::fs::read_to_string(&path).map_err(|e| {
            AppError::Plugin(format!(
                "Cannot read plugin.json in {}: {e}",
                plugin_dir.display()
            ))
        })?;
        let manifest: Self = serde_json::from_str(&raw)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> AppResult<()> {
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(AppError::Plugin(format!(
                "Invalid plugin id '{}': only [a-zA-Z0-9_-] allowed",
                self.id
            )));
        }
        if self.name.trim().is_empty() {
            return Err(AppError::Plugin("Plugin name cannot be empty".to_string()));
        }
        if let Some(logo) = &self.logo
            && (logo.trim().is_empty() || logo.contains(".."))
        {
            return Err(AppError::Plugin(format!(
                "Invalid plugin logo path '{logo}'"
            )));
        }
        if let Some(backend) = &self.backend {
            if backend.entry.trim().is_empty() || backend.entry.contains("..") {
                return Err(AppError::Plugin(format!(
                    "Invalid backend entry path '{}'",
                    backend.entry
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_parse_manifest_with_logo_and_process_backend() {
        let raw = r#"{
            "id": "demo-tool",
            "name": "Demo Tool",
            "version": "1.0.0",
            "logo": "logo.png",
            "backend": {
                "kind": "process",
                "entry": "backend/server.exe",
                "args": ["--stdio"]
            },
            "permissions": ["backend", "process"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(raw).unwrap();
        manifest.validate().unwrap();
        assert_eq!(manifest.logo.as_deref(), Some("logo.png"));
        let backend = manifest.backend.unwrap();
        assert_eq!(backend.kind, BackendKind::Process);
        assert_eq!(backend.entry, "backend/server.exe");
        assert_eq!(backend.idle_timeout_secs, 60);
    }

    #[test]
    fn should_reject_path_traversal_in_backend_entry() {
        let raw = r#"{
            "id": "bad-plugin",
            "name": "Bad Plugin",
            "version": "1.0.0",
            "backend": { "entry": "../evil.exe" }
        }"#;
        let manifest: PluginManifest = serde_json::from_str(raw).unwrap();
        assert!(manifest.validate().is_err());
    }
}
