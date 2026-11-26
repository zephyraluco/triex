use std::collections::HashMap;

use rusqlite::params;

use crate::error::AppResult;
use crate::search::index::{ItemKind, RawSearchItem};
use crate::storage::{now_unix_secs, Database};

#[derive(Debug, Clone, Copy)]
pub struct UsageRecord {
    pub launch_count: u64,
    pub last_used_at: i64,
}

impl Database {
    /// Replaces the cached indexed items in a single transaction.
    pub fn save_cached_items(&self, items: &[RawSearchItem]) -> AppResult<()> {
        let now = now_unix_secs();
        self.with_conn(|conn| {
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM apps_cache", [])?;
            {
                let mut stmt = tx.prepare(
                    "INSERT OR REPLACE INTO apps_cache
                     (id, kind, name, subtitle, exec_path, exec_args, working_dir, icon_key, keywords_json, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                )?;
                for item in items {
                    let keywords_json = serde_json::to_string(&item.keywords)?;
                    stmt.execute(params![
                        item.id,
                        item.kind.as_str(),
                        item.name,
                        item.subtitle,
                        item.exec_path,
                        item.exec_args,
                        item.working_dir,
                        item.icon_key,
                        keywords_json,
                        now,
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
    }

    /// Loads previously cached items for instant cold startup before background rescan completes.
    pub fn load_cached_items(&self) -> AppResult<Vec<RawSearchItem>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, kind, name, subtitle, exec_path, exec_args, working_dir, icon_key, keywords_json
                 FROM apps_cache",
            )?;
            let rows = stmt.query_map([], |row| {
                let kind_str: String = row.get(1)?;
                let keywords_json: String = row.get(8)?;
                let keywords: Vec<String> =
                    serde_json::from_str(&keywords_json).unwrap_or_default();
                Ok(RawSearchItem {
                    id: row.get(0)?,
                    kind: ItemKind::parse(&kind_str),
                    name: row.get(2)?,
                    subtitle: row.get(3)?,
                    exec_path: row.get(4)?,
                    exec_args: row.get(5)?,
                    working_dir: row.get(6)?,
                    icon_key: row.get(7)?,
                    keywords,
                })
            })?;

            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })
    }

    /// Increments launch frequency and updates timestamp for frecency ranking.
    pub fn record_item_launch(&self, item_id: &str) -> AppResult<UsageRecord> {
        let now = now_unix_secs();
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO usage_history (item_id, launch_count, last_used_at)
                 VALUES (?1, 1, ?2)
                 ON CONFLICT(item_id) DO UPDATE SET
                   launch_count = usage_history.launch_count + 1,
                   last_used_at = excluded.last_used_at",
                params![item_id, now],
            )?;

            let count: i64 = conn.query_row(
                "SELECT launch_count FROM usage_history WHERE item_id = ?1",
                params![item_id],
                |row| row.get(0),
            )?;

            Ok(UsageRecord {
                launch_count: count.max(0) as u64,
                last_used_at: now,
            })
        })
    }

    pub fn remove_item_usage(&self, item_id: &str) -> AppResult<()> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM usage_history WHERE item_id = ?1",
                params![item_id],
            )?;
            Ok(())
        })
    }

    pub fn load_all_usage(&self) -> AppResult<HashMap<String, UsageRecord>> {
        self.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT item_id, launch_count, last_used_at FROM usage_history")?;
            let rows = stmt.query_map([], |row| {
                let item_id: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                let last_used_at: i64 = row.get(2)?;
                Ok((
                    item_id,
                    UsageRecord {
                        launch_count: count.max(0) as u64,
                        last_used_at,
                    },
                ))
            })?;

            let mut map = HashMap::new();
            for row in rows {
                let (k, v) = row?;
                map.insert(k, v);
            }
            Ok(map)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_item_launch_should_increment_count() {
        let db = Database::open_in_memory().expect("in-memory db");
        let first = db.record_item_launch("app:vscode").expect("first launch");
        let second = db.record_item_launch("app:vscode").expect("second launch");
        assert_eq!(first.launch_count, 1);
        assert_eq!(second.launch_count, 2);
    }

    #[test]
    fn save_cached_items_should_not_fail_on_duplicate_id() {
        let db = Database::open_in_memory().expect("in-memory db");
        let item = RawSearchItem {
            id: "app:dup".into(),
            kind: ItemKind::App,
            name: "App A".into(),
            subtitle: String::new(),
            exec_path: "a.lnk".into(),
            exec_args: String::new(),
            working_dir: String::new(),
            icon_key: "dup".into(),
            keywords: Vec::new(),
        };
        db.save_cached_items(&[item.clone(), item])
            .expect("save should succeed");
        assert_eq!(db.load_cached_items().expect("load").len(), 1);
    }
}
