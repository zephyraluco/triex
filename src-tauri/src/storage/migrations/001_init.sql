-- Initial schema for Triex launcher (SQLite WAL mode)

CREATE TABLE IF NOT EXISTS apps_cache (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    subtitle TEXT NOT NULL DEFAULT '',
    exec_path TEXT NOT NULL,
    exec_args TEXT NOT NULL DEFAULT '',
    working_dir TEXT NOT NULL DEFAULT '',
    icon_key TEXT NOT NULL DEFAULT '',
    keywords_json TEXT NOT NULL DEFAULT '[]',
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS usage_history (
    item_id TEXT PRIMARY KEY NOT NULL,
    launch_count INTEGER NOT NULL DEFAULT 0,
    last_used_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS plugins (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    entry TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    manifest_json TEXT NOT NULL,
    installed_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS plugin_kv (
    plugin_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value_json TEXT NOT NULL,
    rev INTEGER NOT NULL DEFAULT 1,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (plugin_id, key)
);

CREATE INDEX IF NOT EXISTS idx_usage_last_used ON usage_history (last_used_at DESC);
CREATE INDEX IF NOT EXISTS idx_plugin_kv_plugin ON plugin_kv (plugin_id);
