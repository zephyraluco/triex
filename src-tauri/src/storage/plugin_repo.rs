use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::plugins::manifest::PluginManifest;
use crate::storage::{now_unix_secs, Database};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPluginRecord {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub entry: String,
    pub enabled: bool,
    pub manifest: PluginManifest,
    pub installed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginKvEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub rev: i64,
    pub updated_at: i64,
}

impl Database {
    pub fn upsert_plugin(&self, manifest: &PluginManifest, enabled: bool) -> AppResult<()> {
        let manifest_json = serde_json::to_string(manifest)?;
        let now = now_unix_secs();
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO plugins (id, name, version, description, entry, enabled, manifest_json, installed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   version = excluded.version,
                   description = excluded.description,
                   entry = excluded.entry,
                   manifest_json = excluded.manifest_json",
                params![
                    manifest.id,
                    manifest.name,
                    manifest.version,
                    manifest.description,
                    manifest.entry,
                    if enabled { 1 } else { 0 },
                    manifest_json,
                    now,
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_plugins(&self) -> AppResult<Vec<InstalledPluginRecord>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, version, description, entry, enabled, manifest_json, installed_at
                 FROM plugins ORDER BY name ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                let manifest_json: String = row.get(6)?;
                let manifest: PluginManifest = serde_json::from_str(&manifest_json)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                Ok(InstalledPluginRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    version: row.get(2)?,
                    description: row.get(3)?,
                    entry: row.get(4)?,
                    enabled: row.get::<_, i64>(5)? != 0,
                    manifest,
                    installed_at: row.get(7)?,
                })
            })?;

            let mut plugins = Vec::new();
            for row in rows {
                plugins.push(row?);
            }
            Ok(plugins)
        })
    }

    pub fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) -> AppResult<()> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE plugins SET enabled = ?1 WHERE id = ?2",
                params![if enabled { 1 } else { 0 }, plugin_id],
            )?;
            Ok(())
        })
    }

    pub fn remove_plugin(&self, plugin_id: &str) -> AppResult<()> {
        self.with_conn(|conn| {
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM plugin_kv WHERE plugin_id = ?1", params![plugin_id])?;
            tx.execute("DELETE FROM plugins WHERE id = ?1", params![plugin_id])?;
            tx.commit()?;
            Ok(())
        })
    }

    pub fn plugin_kv_get(&self, plugin_id: &str, key: &str) -> AppResult<Option<PluginKvEntry>> {
        self.with_conn(|conn| {
            let row = conn
                .query_row(
                    "SELECT key, value_json, rev, updated_at FROM plugin_kv WHERE plugin_id = ?1 AND key = ?2",
                    params![plugin_id, key],
                    |row| {
                        let value_json: String = row.get(1)?;
                        let value = serde_json::from_str(&value_json).unwrap_or(serde_json::Value::Null);
                        Ok(PluginKvEntry {
                            key: row.get(0)?,
                            value,
                            rev: row.get(2)?,
                            updated_at: row.get(3)?,
                        })
                    },
                )
                .optional()?;
            Ok(row)
        })
    }

    pub fn plugin_kv_put(
        &self,
        plugin_id: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> AppResult<PluginKvEntry> {
        let value_json = serde_json::to_string(value)?;
        let now = now_unix_secs();
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO plugin_kv (plugin_id, key, value_json, rev, updated_at)
                 VALUES (?1, ?2, ?3, 1, ?4)
                 ON CONFLICT(plugin_id, key) DO UPDATE SET
                   value_json = excluded.value_json,
                   rev = plugin_kv.rev + 1,
                   updated_at = excluded.updated_at",
                params![plugin_id, key, value_json, now],
            )?;

            let rev: i64 = conn.query_row(
                "SELECT rev FROM plugin_kv WHERE plugin_id = ?1 AND key = ?2",
                params![plugin_id, key],
                |row| row.get(0),
            )?;

            Ok(PluginKvEntry {
                key: key.to_string(),
                value: value.clone(),
                rev,
                updated_at: now,
            })
        })
    }

    pub fn plugin_kv_delete(&self, plugin_id: &str, key: &str) -> AppResult<()> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM plugin_kv WHERE plugin_id = ?1 AND key = ?2",
                params![plugin_id, key],
            )?;
            Ok(())
        })
    }
}
