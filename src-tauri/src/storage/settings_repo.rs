use rusqlite::{params, OptionalExtension};

use crate::config::AppSettings;
use crate::error::AppResult;
use crate::storage::Database;

const SETTINGS_ROW_KEY: &str = "app_settings";

impl Database {
    pub fn load_settings(&self) -> AppResult<AppSettings> {
        self.with_conn(|conn| {
            let value: Option<String> = conn
                .query_row(
                    "SELECT value_json FROM settings WHERE key = ?1",
                    params![SETTINGS_ROW_KEY],
                    |row| row.get(0),
                )
                .optional()?;

            match value {
                Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
                None => Ok(AppSettings::default()),
            }
        })
    }

    pub fn save_settings(&self, settings: &AppSettings) -> AppResult<()> {
        let json = serde_json::to_string(settings)?;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO settings (key, value_json)
                 VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                params![SETTINGS_ROW_KEY, json],
            )?;
            Ok(())
        })
    }
}
