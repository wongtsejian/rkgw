use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::params;

use crate::config::{Config, DebugMode};

/// A record of a configuration change.
#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub key: String,
    pub old_value: Option<String>,
    pub new_value: String,
    pub changed_at: String,
    pub source: String,
}

/// SQLite-backed configuration persistence.
pub struct ConfigDb {
    conn: Mutex<rusqlite::Connection>,
}

impl ConfigDb {
    /// Open (or create) the config database at `path` and run migrations.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = rusqlite::Connection::open(path)
            .with_context(|| format!("Failed to open config database: {}", path.display()))?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.run_migrations()?;
        Ok(db)
    }

    /// Create tables if they don't already exist.
    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock().expect("config db mutex poisoned");

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version    INTEGER NOT NULL,
                applied_at TEXT    NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS config (
                key         TEXT PRIMARY KEY NOT NULL,
                value       TEXT NOT NULL,
                value_type  TEXT NOT NULL DEFAULT 'string',
                updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                description TEXT
            );

            CREATE TABLE IF NOT EXISTS config_history (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                key        TEXT NOT NULL,
                old_value  TEXT,
                new_value  TEXT NOT NULL,
                changed_at TEXT NOT NULL DEFAULT (datetime('now')),
                source     TEXT NOT NULL DEFAULT 'web_ui'
            );",
        )
        .context("Failed to run config database migrations")?;

        // Record schema version 1 if not present
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))
            .unwrap_or(0);

        if count == 0 {
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?)",
                params![1],
            )
            .context("Failed to insert schema version")?;
        }

        Ok(())
    }

    /// Get a single config value by key.
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().expect("config db mutex poisoned");
        let mut stmt = conn
            .prepare("SELECT value FROM config WHERE key = ?")
            .context("Failed to prepare get query")?;

        let result = stmt.query_row(params![key], |row| row.get(0)).ok();

        Ok(result)
    }

    /// Upsert a config value and record the change in history.
    pub fn set(&self, key: &str, value: &str, source: &str) -> Result<()> {
        let conn = self.conn.lock().expect("config db mutex poisoned");

        // Fetch old value for history
        let old_value: Option<String> = conn
            .query_row(
                "SELECT value FROM config WHERE key = ?",
                params![key],
                |row| row.get(0),
            )
            .ok();

        conn.execute(
            "INSERT INTO config (key, value, updated_at)
             VALUES (?, ?, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value],
        )
        .with_context(|| format!("Failed to upsert config key '{}'", key))?;

        conn.execute(
            "INSERT INTO config_history (key, old_value, new_value, source)
             VALUES (?, ?, ?, ?)",
            params![key, old_value, value, source],
        )
        .with_context(|| format!("Failed to record config history for '{}'", key))?;

        Ok(())
    }

    /// Get all config key-value pairs.
    pub fn get_all(&self) -> Result<HashMap<String, String>> {
        let conn = self.conn.lock().expect("config db mutex poisoned");
        let mut stmt = conn
            .prepare("SELECT key, value FROM config")
            .context("Failed to prepare get_all query")?;

        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .context("Failed to query all config")?;

        let mut map = HashMap::new();
        for row in rows {
            let (k, v) = row.context("Failed to read config row")?;
            map.insert(k, v);
        }
        Ok(map)
    }

    /// Get recent config change history.
    pub fn get_history(&self, limit: usize) -> Result<Vec<ConfigChange>> {
        let conn = self.conn.lock().expect("config db mutex poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT key, old_value, new_value, changed_at, source
                 FROM config_history
                 ORDER BY id DESC
                 LIMIT ?",
            )
            .context("Failed to prepare history query")?;

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(ConfigChange {
                    key: row.get(0)?,
                    old_value: row.get(1)?,
                    new_value: row.get(2)?,
                    changed_at: row.get(3)?,
                    source: row.get(4)?,
                })
            })
            .context("Failed to query config history")?;

        let mut changes = Vec::new();
        for row in rows {
            changes.push(row.context("Failed to read history row")?);
        }
        Ok(changes)
    }

    /// Overlay persisted config values onto an existing Config struct.
    pub fn load_into_config(&self, config: &mut Config) -> Result<()> {
        let all = self.get_all()?;

        for (key, value) in &all {
            match key.as_str() {
                "server_host" => config.server_host = value.clone(),
                "server_port" => {
                    if let Ok(v) = value.parse() {
                        config.server_port = v;
                    }
                }
                "proxy_api_key" => config.proxy_api_key = value.clone(),
                "kiro_region" => config.kiro_region = value.clone(),
                "log_level" => config.log_level = value.clone(),
                "debug_mode" => {
                    config.debug_mode = match value.to_lowercase().as_str() {
                        "errors" => DebugMode::Errors,
                        "all" => DebugMode::All,
                        _ => DebugMode::Off,
                    };
                }
                "fake_reasoning_enabled" => {
                    if let Ok(v) = value.parse() {
                        config.fake_reasoning_enabled = v;
                    }
                }
                "fake_reasoning_max_tokens" => {
                    if let Ok(v) = value.parse() {
                        config.fake_reasoning_max_tokens = v;
                    }
                }
                "truncation_recovery" => {
                    if let Ok(v) = value.parse() {
                        config.truncation_recovery = v;
                    }
                }
                "tool_description_max_length" => {
                    if let Ok(v) = value.parse() {
                        config.tool_description_max_length = v;
                    }
                }
                "first_token_timeout" => {
                    if let Ok(v) = value.parse() {
                        config.first_token_timeout = v;
                    }
                }
                "tls_enabled" => {
                    if let Ok(v) = value.parse() {
                        config.tls_enabled = v;
                    }
                }
                "tls_cert_path" => {
                    config.tls_cert_path = Some(std::path::PathBuf::from(value));
                }
                "tls_key_path" => {
                    config.tls_key_path = Some(std::path::PathBuf::from(value));
                }
                "kiro_cli_db_file" => {
                    config.kiro_cli_db_file = std::path::PathBuf::from(value);
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Persist the current Config fields into SQLite.
    pub fn save_from_config(&self, config: &Config) -> Result<()> {
        let pairs: Vec<(&str, String)> = vec![
            ("server_host", config.server_host.clone()),
            ("server_port", config.server_port.to_string()),
            ("proxy_api_key", config.proxy_api_key.clone()),
            ("kiro_region", config.kiro_region.clone()),
            ("log_level", config.log_level.clone()),
            (
                "debug_mode",
                match config.debug_mode {
                    DebugMode::Off => "off",
                    DebugMode::Errors => "errors",
                    DebugMode::All => "all",
                }
                .to_string(),
            ),
            (
                "fake_reasoning_enabled",
                config.fake_reasoning_enabled.to_string(),
            ),
            (
                "fake_reasoning_max_tokens",
                config.fake_reasoning_max_tokens.to_string(),
            ),
            (
                "truncation_recovery",
                config.truncation_recovery.to_string(),
            ),
            (
                "tool_description_max_length",
                config.tool_description_max_length.to_string(),
            ),
            (
                "first_token_timeout",
                config.first_token_timeout.to_string(),
            ),
            ("tls_enabled", config.tls_enabled.to_string()),
            (
                "kiro_cli_db_file",
                config.kiro_cli_db_file.display().to_string(),
            ),
        ];

        for (key, value) in pairs {
            self.set(key, &value, "config_sync")?;
        }

        if let Some(ref p) = config.tls_cert_path {
            self.set("tls_cert_path", &p.display().to_string(), "config_sync")?;
        }
        if let Some(ref p) = config.tls_key_path {
            self.set("tls_key_path", &p.display().to_string(), "config_sync")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::config::FakeReasoningHandling;

    fn create_test_db() -> (ConfigDb, PathBuf) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let path = std::env::temp_dir().join(format!(
            "test_config_db_{}_{}.sqlite",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let db = ConfigDb::open(&path).unwrap();
        (db, path)
    }

    fn create_test_config() -> Config {
        Config {
            server_host: "127.0.0.1".to_string(),
            server_port: 8000,
            proxy_api_key: "test-key".to_string(),
            kiro_region: "us-east-1".to_string(),
            kiro_cli_db_file: PathBuf::from("/tmp/test.db"),
            streaming_timeout: 300,
            token_refresh_threshold: 300,
            first_token_timeout: 15,
            http_max_connections: 20,
            http_connect_timeout: 30,
            http_request_timeout: 300,
            http_max_retries: 3,
            debug_mode: DebugMode::Off,
            log_level: "info".to_string(),
            tool_description_max_length: 10000,
            fake_reasoning_enabled: true,
            fake_reasoning_max_tokens: 4000,
            fake_reasoning_handling: FakeReasoningHandling::AsReasoningContent,
            truncation_recovery: true,
            dashboard: false,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            web_ui_enabled: false,
            config_db_path: None,
        }
    }

    #[test]
    fn test_open_creates_tables() {
        let (db, _tmp) = create_test_db();
        let conn = db.conn.lock().unwrap();
        // Verify tables exist by querying them
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_set_and_get() {
        let (db, _tmp) = create_test_db();

        db.set("log_level", "debug", "test").unwrap();
        let val = db.get("log_level").unwrap();
        assert_eq!(val, Some("debug".to_string()));
    }

    #[test]
    fn test_get_missing_key() {
        let (db, _tmp) = create_test_db();
        let val = db.get("nonexistent").unwrap();
        assert_eq!(val, None);
    }

    #[test]
    fn test_set_upsert() {
        let (db, _tmp) = create_test_db();

        db.set("log_level", "info", "test").unwrap();
        db.set("log_level", "debug", "test").unwrap();

        let val = db.get("log_level").unwrap();
        assert_eq!(val, Some("debug".to_string()));
    }

    #[test]
    fn test_get_all() {
        let (db, _tmp) = create_test_db();

        db.set("key1", "val1", "test").unwrap();
        db.set("key2", "val2", "test").unwrap();

        let all = db.get_all().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all.get("key1").unwrap(), "val1");
        assert_eq!(all.get("key2").unwrap(), "val2");
    }

    #[test]
    fn test_get_history() {
        let (db, _tmp) = create_test_db();

        db.set("log_level", "info", "init").unwrap();
        db.set("log_level", "debug", "web_ui").unwrap();

        let history = db.get_history(10).unwrap();
        assert_eq!(history.len(), 2);

        // Most recent first
        assert_eq!(history[0].key, "log_level");
        assert_eq!(history[0].new_value, "debug");
        assert_eq!(history[0].old_value, Some("info".to_string()));
        assert_eq!(history[0].source, "web_ui");

        assert_eq!(history[1].key, "log_level");
        assert_eq!(history[1].new_value, "info");
        assert_eq!(history[1].old_value, None);
        assert_eq!(history[1].source, "init");
    }

    #[test]
    fn test_get_history_limit() {
        let (db, _tmp) = create_test_db();

        for i in 0..5 {
            db.set("key", &format!("val{}", i), "test").unwrap();
        }

        let history = db.get_history(2).unwrap();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_save_and_load_config() {
        let (db, _tmp) = create_test_db();
        let original = create_test_config();

        db.save_from_config(&original).unwrap();

        let mut loaded = create_test_config();
        loaded.log_level = "changed".to_string();
        loaded.server_port = 9999;

        db.load_into_config(&mut loaded).unwrap();

        assert_eq!(loaded.log_level, "info");
        assert_eq!(loaded.server_port, 8000);
        assert_eq!(loaded.fake_reasoning_enabled, true);
        assert_eq!(loaded.truncation_recovery, true);
    }

    #[test]
    fn test_load_into_config_debug_mode() {
        let (db, _tmp) = create_test_db();

        db.set("debug_mode", "errors", "test").unwrap();

        let mut config = create_test_config();
        db.load_into_config(&mut config).unwrap();

        assert_eq!(config.debug_mode, DebugMode::Errors);
    }

    #[test]
    fn test_load_into_config_ignores_unknown_keys() {
        let (db, _tmp) = create_test_db();

        db.set("unknown_key", "whatever", "test").unwrap();

        let mut config = create_test_config();
        // Should not panic
        db.load_into_config(&mut config).unwrap();
    }
}
