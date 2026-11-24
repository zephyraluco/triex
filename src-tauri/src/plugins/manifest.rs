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
    #[serde(default = "default_entry")]
    pub entry: String,
    #[serde(default = "default_height")]
    pub height: f64,
    #[serde(default)]
    pub features: Vec<PluginFeature>,
    #[serde(default)]
    pub permissions: Vec<String>,
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

impl PluginManifest {
    pub fn load_from_dir(plugin_dir: &Path) -> AppResult<Self> {
        let path = plugin_dir.join("plugin.json");
        let raw = std::fs::read_to_string(&path).map_err(|e| {
            AppError::Plugin(format!("Cannot read plugin.json in {}: {e}", plugin_dir.display()))
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
        Ok(())
    }
}
