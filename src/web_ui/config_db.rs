use std::collections::HashMap;

use anyhow::{Context, Result};
use sqlx::PgPool;

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

/// PostgreSQL-backed configuration persistence.
pub struct ConfigDb {
    pool: PgPool,
}

impl ConfigDb {
    /// Connect to the PostgreSQL database and run migrations.
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url)
            .await
            .context("Failed to connect to PostgreSQL")?;
        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Create tables if they don't already exist.
    async fn run_migrations(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version    INTEGER NOT NULL,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create schema_version table")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS config (
                key         TEXT PRIMARY KEY NOT NULL,
                value       TEXT NOT NULL,
                value_type  TEXT NOT NULL DEFAULT 'string',
                updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                description TEXT
            )",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create config table")?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS config_history (
                id         SERIAL PRIMARY KEY,
                key        TEXT NOT NULL,
                old_value  TEXT,
                new_value  TEXT NOT NULL,
                changed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                source     TEXT NOT NULL DEFAULT 'web_ui'
            )",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create config_history table")?;

        // Record schema version 1 if not present
        let count: Option<i64> = sqlx::query_scalar("SELECT COUNT(*) FROM schema_version")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(Some(0));

        if count.unwrap_or(0) == 0 {
            sqlx::query("INSERT INTO schema_version (version) VALUES ($1)")
                .bind(1_i32)
                .execute(&self.pool)
                .await
                .context("Failed to insert schema version")?;
        }

        Ok(())
    }

    /// Get a single config value by key.
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let result: Option<String> = sqlx::query_scalar("SELECT value FROM config WHERE key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to query config value")?;

        Ok(result)
    }

    /// Upsert a config value and record the change in history.
    /// All operations (read old value, upsert, history insert, prune) run in a single transaction.
    pub async fn set(&self, key: &str, value: &str, source: &str) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin transaction for config set")?;

        // Fetch old value for history
        let old_value: Option<String> =
            sqlx::query_scalar("SELECT value FROM config WHERE key = $1")
                .bind(key)
                .fetch_optional(&mut *tx)
                .await
                .context("Failed to fetch old config value")?;

        sqlx::query(
            "INSERT INTO config (key, value, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT(key) DO UPDATE SET value = EXCLUDED.value, updated_at = EXCLUDED.updated_at",
        )
        .bind(key)
        .bind(value)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("Failed to upsert config key '{}'", key))?;

        sqlx::query(
            "INSERT INTO config_history (key, old_value, new_value, source)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(key)
        .bind(&old_value)
        .bind(value)
        .bind(source)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("Failed to record config history for '{}'", key))?;

        // Prune old history entries, keeping the most recent 1000
        sqlx::query(
            "DELETE FROM config_history WHERE id NOT IN (SELECT id FROM config_history ORDER BY id DESC LIMIT 1000)",
        )
        .execute(&mut *tx)
        .await
        .context("Failed to prune config history")?;

        tx.commit()
            .await
            .context("Failed to commit config set transaction")?;

        Ok(())
    }

    /// Get all config key-value pairs.
    pub async fn get_all(&self) -> Result<HashMap<String, String>> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM config")
            .fetch_all(&self.pool)
            .await
            .context("Failed to query all config")?;

        let mut map = HashMap::new();
        for (k, v) in rows {
            map.insert(k, v);
        }
        Ok(map)
    }

    /// Get recent config change history.
    pub async fn get_history(&self, limit: usize) -> Result<Vec<ConfigChange>> {
        let rows: Vec<(String, Option<String>, String, String, String)> = sqlx::query_as(
            "SELECT key, old_value, new_value, changed_at::TEXT, source
             FROM config_history
             ORDER BY id DESC
             LIMIT $1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .context("Failed to query config history")?;

        let changes = rows
            .into_iter()
            .map(
                |(key, old_value, new_value, changed_at, source)| ConfigChange {
                    key,
                    old_value,
                    new_value,
                    changed_at,
                    source,
                },
            )
            .collect();

        Ok(changes)
    }

    /// Overlay persisted config values onto an existing Config struct.
    pub async fn load_into_config(&self, config: &mut Config) -> Result<()> {
        let all = self.get_all().await?;

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
                "tls_cert_path" => {
                    config.tls_cert_path = Some(std::path::PathBuf::from(value));
                }
                "tls_key_path" => {
                    config.tls_key_path = Some(std::path::PathBuf::from(value));
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Check if initial setup has been completed (proxy_api_key and kiro_refresh_token both exist).
    pub async fn is_setup_complete(&self) -> bool {
        let has_key = self.get("proxy_api_key").await.ok().flatten().is_some();
        let has_token = self
            .get("kiro_refresh_token")
            .await
            .ok()
            .flatten()
            .is_some();
        has_key && has_token
    }

    /// Save initial setup configuration (proxy key, refresh token, region).
    /// All writes are wrapped in a single transaction for atomicity.
    pub async fn save_initial_setup(
        &self,
        proxy_key: &str,
        refresh_token: &str,
        region: &str,
    ) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin transaction for initial setup")?;

        let keys_values: &[(&str, &str)] = &[
            ("proxy_api_key", proxy_key),
            ("kiro_refresh_token", refresh_token),
            ("kiro_region", region),
            ("setup_complete", "true"),
        ];

        for &(key, value) in keys_values {
            let old_value: Option<String> =
                sqlx::query_scalar("SELECT value FROM config WHERE key = $1")
                    .bind(key)
                    .fetch_optional(&mut *tx)
                    .await
                    .context("Failed to fetch old config value during setup")?;

            sqlx::query(
                "INSERT INTO config (key, value, updated_at)
                 VALUES ($1, $2, NOW())
                 ON CONFLICT(key) DO UPDATE SET value = EXCLUDED.value, updated_at = EXCLUDED.updated_at",
            )
            .bind(key)
            .bind(value)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("Failed to upsert config key '{}' during setup", key))?;

            sqlx::query(
                "INSERT INTO config_history (key, old_value, new_value, source)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(key)
            .bind(&old_value)
            .bind(value)
            .bind("setup")
            .execute(&mut *tx)
            .await
            .with_context(|| {
                format!("Failed to record config history for '{}' during setup", key)
            })?;
        }

        tx.commit()
            .await
            .context("Failed to commit initial setup transaction")?;

        Ok(())
    }

    /// Get the stored Kiro refresh token.
    pub async fn get_refresh_token(&self) -> Result<Option<String>> {
        self.get("kiro_refresh_token").await
    }

    /// Save OAuth-based setup (all fields in one transaction).
    #[allow(clippy::too_many_arguments)]
    pub async fn save_oauth_setup(
        &self,
        proxy_key: &str,
        refresh_token: &str,
        region: &str,
        client_id: &str,
        client_secret: &str,
        client_secret_expires_at: &str,
        start_url: &str,
    ) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin transaction for OAuth setup")?;

        let keys_values: &[(&str, &str)] = &[
            ("proxy_api_key", proxy_key),
            ("kiro_refresh_token", refresh_token),
            ("kiro_region", "us-east-1"),
            ("oauth_sso_region", region),
            ("oauth_client_id", client_id),
            ("oauth_client_secret", client_secret),
            ("oauth_client_secret_expires_at", client_secret_expires_at),
            ("oauth_start_url", start_url),
            ("setup_complete", "true"),
        ];

        for &(key, value) in keys_values {
            let old_value: Option<String> =
                sqlx::query_scalar("SELECT value FROM config WHERE key = $1")
                    .bind(key)
                    .fetch_optional(&mut *tx)
                    .await
                    .context("Failed to fetch old config value during OAuth setup")?;

            sqlx::query(
                "INSERT INTO config (key, value, updated_at)
                 VALUES ($1, $2, NOW())
                 ON CONFLICT(key) DO UPDATE SET value = EXCLUDED.value, updated_at = EXCLUDED.updated_at",
            )
            .bind(key)
            .bind(value)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("Failed to upsert config key '{}' during OAuth setup", key))?;

            sqlx::query(
                "INSERT INTO config_history (key, old_value, new_value, source)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(key)
            .bind(&old_value)
            .bind(value)
            .bind("oauth_setup")
            .execute(&mut *tx)
            .await
            .with_context(|| {
                format!(
                    "Failed to record config history for '{}' during OAuth setup",
                    key
                )
            })?;
        }

        tx.commit()
            .await
            .context("Failed to commit OAuth setup transaction")?;

        Ok(())
    }

    /// Get OAuth client credentials from config.
    #[allow(dead_code)]
    pub async fn get_oauth_client(&self) -> Result<Option<(String, String, String)>> {
        let client_id = self.get("oauth_client_id").await?;
        let client_secret = self.get("oauth_client_secret").await?;
        let expires_at = self
            .get("oauth_client_secret_expires_at")
            .await?
            .unwrap_or_default();
        match (client_id, client_secret) {
            (Some(id), Some(secret)) => Ok(Some((id, secret, expires_at))),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Connect to a test PostgreSQL database. Returns None if DATABASE_URL is not set.
    async fn setup_test_db() -> Option<ConfigDb> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let db = ConfigDb::connect(&url).await.ok()?;
        // Clean tables for a fresh test
        sqlx::query("DELETE FROM config_history")
            .execute(&db.pool)
            .await
            .ok();
        sqlx::query("DELETE FROM config")
            .execute(&db.pool)
            .await
            .ok();
        Some(db)
    }

    fn create_test_config() -> Config {
        Config {
            proxy_api_key: "test-key".to_string(),
            ..Config::with_defaults()
        }
    }

    #[tokio::test]
    async fn test_connect_creates_tables() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        let count: Option<i64> = sqlx::query_scalar("SELECT COUNT(*) FROM schema_version")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(count, Some(1));
    }

    #[tokio::test]
    async fn test_set_and_get() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        db.set("log_level", "debug", "test").await.unwrap();
        let val = db.get("log_level").await.unwrap();
        assert_eq!(val, Some("debug".to_string()));
    }

    #[tokio::test]
    async fn test_get_missing_key() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        let val = db.get("nonexistent").await.unwrap();
        assert_eq!(val, None);
    }

    #[tokio::test]
    async fn test_set_upsert() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        db.set("log_level", "info", "test").await.unwrap();
        db.set("log_level", "debug", "test").await.unwrap();
        let val = db.get("log_level").await.unwrap();
        assert_eq!(val, Some("debug".to_string()));
    }

    #[tokio::test]
    async fn test_get_all() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        db.set("key1", "val1", "test").await.unwrap();
        db.set("key2", "val2", "test").await.unwrap();
        let all = db.get_all().await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all.get("key1").unwrap(), "val1");
        assert_eq!(all.get("key2").unwrap(), "val2");
    }

    #[tokio::test]
    async fn test_get_history() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        db.set("log_level", "info", "init").await.unwrap();
        db.set("log_level", "debug", "web_ui").await.unwrap();
        let history = db.get_history(10).await.unwrap();
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

    #[tokio::test]
    async fn test_get_history_limit() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        for i in 0..5 {
            db.set("key", &format!("val{}", i), "test").await.unwrap();
        }
        let history = db.get_history(2).await.unwrap();
        assert_eq!(history.len(), 2);
    }

    #[tokio::test]
    async fn test_set_and_load_config() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        db.set("log_level", "info", "test").await.unwrap();
        db.set("server_port", "8000", "test").await.unwrap();
        db.set("fake_reasoning_enabled", "true", "test")
            .await
            .unwrap();
        db.set("truncation_recovery", "true", "test").await.unwrap();

        let mut loaded = create_test_config();
        loaded.log_level = "changed".to_string();
        loaded.server_port = 9999;

        db.load_into_config(&mut loaded).await.unwrap();

        assert_eq!(loaded.log_level, "info");
        assert_eq!(loaded.server_port, 8000);
        assert!(loaded.fake_reasoning_enabled);
        assert!(loaded.truncation_recovery);
    }

    #[tokio::test]
    async fn test_load_into_config_debug_mode() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        db.set("debug_mode", "errors", "test").await.unwrap();
        let mut config = create_test_config();
        db.load_into_config(&mut config).await.unwrap();
        assert_eq!(config.debug_mode, DebugMode::Errors);
    }

    #[tokio::test]
    async fn test_is_setup_complete_false_when_empty() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        assert!(!db.is_setup_complete().await);
    }

    #[tokio::test]
    async fn test_is_setup_complete_false_with_only_key() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        db.set("proxy_api_key", "test-key", "test").await.unwrap();
        assert!(!db.is_setup_complete().await);
    }

    #[tokio::test]
    async fn test_is_setup_complete_true() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        db.set("proxy_api_key", "test-key", "test").await.unwrap();
        db.set("kiro_refresh_token", "test-token", "test")
            .await
            .unwrap();
        assert!(db.is_setup_complete().await);
    }

    #[tokio::test]
    async fn test_save_initial_setup() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        db.save_initial_setup("my-key", "my-token", "us-west-2")
            .await
            .unwrap();

        assert_eq!(
            db.get("proxy_api_key").await.unwrap(),
            Some("my-key".to_string())
        );
        assert_eq!(
            db.get("kiro_refresh_token").await.unwrap(),
            Some("my-token".to_string())
        );
        assert_eq!(
            db.get("kiro_region").await.unwrap(),
            Some("us-west-2".to_string())
        );
        assert_eq!(
            db.get("setup_complete").await.unwrap(),
            Some("true".to_string())
        );
        assert!(db.is_setup_complete().await);
    }

    #[tokio::test]
    async fn test_get_refresh_token() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        assert_eq!(db.get_refresh_token().await.unwrap(), None);
        db.set("kiro_refresh_token", "my-token", "test")
            .await
            .unwrap();
        assert_eq!(
            db.get_refresh_token().await.unwrap(),
            Some("my-token".to_string())
        );
    }

    #[tokio::test]
    async fn test_load_into_config_ignores_unknown_keys() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping: DATABASE_URL not set");
            return;
        };
        db.set("unknown_key", "whatever", "test").await.unwrap();
        let mut config = create_test_config();
        // Should not panic
        db.load_into_config(&mut config).await.unwrap();
    }
}
