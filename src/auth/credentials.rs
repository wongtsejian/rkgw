use anyhow::{Context, Result};

use super::types::{AuthType, Credentials};
use crate::web_ui::config_db::ConfigDb;

/// Load credentials from the gateway's config database (KiroDesktop auth type).
///
/// Reads `kiro_refresh_token` and `kiro_region` from the config DB.
/// Returns credentials with `client_id: None, client_secret: None`,
/// which triggers the KiroDesktop auth path.
pub async fn load_from_config_db(config_db: &ConfigDb) -> Result<Credentials> {
    let refresh_token = config_db
        .get_refresh_token()
        .await?
        .context("No kiro_refresh_token found in config database")?;

    let region = config_db
        .get("kiro_region")
        .await?
        .unwrap_or_else(|| "us-east-1".to_string());

    tracing::info!("Loaded credentials from config DB (KiroDesktop auth)");

    Ok(Credentials {
        refresh_token,
        access_token: None,
        expires_at: None,
        profile_arn: None,
        region,
        client_id: None,
        client_secret: None,
        sso_region: None,
        scopes: None,
    })
}

/// Detect authentication type based on credentials
pub fn detect_auth_type(creds: &Credentials) -> AuthType {
    if creds.client_id.is_some() && creds.client_secret.is_some() {
        tracing::info!("Detected auth type: AWS SSO OIDC (kiro-cli)");
        AuthType::AwsSsoOidc
    } else {
        tracing::info!("Detected auth type: Kiro Desktop");
        AuthType::KiroDesktop
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    /// Parse datetime from various ISO 8601 formats
    fn parse_datetime(s: &str) -> Result<DateTime<Utc>> {
        // Handle Z suffix
        let normalized = if s.ends_with('Z') {
            s.replace('Z', "+00:00")
        } else {
            s.to_string()
        };

        DateTime::parse_from_rfc3339(&normalized)
            .map(|dt| dt.with_timezone(&Utc))
            .with_context(|| format!("Failed to parse datetime: {}", s))
    }

    #[test]
    fn test_parse_datetime() {
        // Test with Z suffix
        let dt = parse_datetime("2025-01-12T10:30:00Z").unwrap();
        assert_eq!(dt.to_rfc3339(), "2025-01-12T10:30:00+00:00");

        // Test with timezone
        let dt = parse_datetime("2025-01-12T10:30:00+00:00").unwrap();
        assert_eq!(dt.to_rfc3339(), "2025-01-12T10:30:00+00:00");
    }

    #[test]
    fn test_detect_auth_type_sso() {
        let creds = Credentials {
            refresh_token: "token".to_string(),
            access_token: None,
            expires_at: None,
            profile_arn: None,
            region: "us-east-1".to_string(),
            client_id: Some("client".to_string()),
            client_secret: Some("secret".to_string()),
            sso_region: None,
            scopes: None,
        };
        assert_eq!(detect_auth_type(&creds), AuthType::AwsSsoOidc);
    }

    #[test]
    fn test_detect_auth_type_kiro_desktop() {
        let creds = Credentials {
            refresh_token: "token".to_string(),
            access_token: None,
            expires_at: None,
            profile_arn: None,
            region: "us-east-1".to_string(),
            client_id: None,
            client_secret: None,
            sso_region: None,
            scopes: None,
        };
        assert_eq!(detect_auth_type(&creds), AuthType::KiroDesktop);
    }

    /// Helper to connect to the test database using DATABASE_URL.
    /// Returns None if DATABASE_URL is not set (skips database-dependent tests).
    async fn setup_test_db() -> Option<ConfigDb> {
        let url = std::env::var("DATABASE_URL").ok()?;
        ConfigDb::connect(&url).await.ok()
    }

    #[tokio::test]
    async fn test_load_from_config_db() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping test_load_from_config_db: DATABASE_URL not set");
            return;
        };
        db.set("kiro_refresh_token", "my-refresh-token", "test")
            .await
            .unwrap();
        db.set("kiro_region", "us-west-2", "test").await.unwrap();

        let creds = load_from_config_db(&db).await.unwrap();
        assert_eq!(creds.refresh_token, "my-refresh-token");
        assert_eq!(creds.region, "us-west-2");
        assert!(creds.client_id.is_none());
        assert!(creds.client_secret.is_none());

        let auth_type = detect_auth_type(&creds);
        assert_eq!(auth_type, AuthType::KiroDesktop);
    }

    #[tokio::test]
    async fn test_load_from_config_db_default_region() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping test_load_from_config_db_default_region: DATABASE_URL not set");
            return;
        };
        db.set("kiro_refresh_token", "token", "test").await.unwrap();

        let creds = load_from_config_db(&db).await.unwrap();
        assert_eq!(creds.region, "us-east-1");
    }

    #[tokio::test]
    async fn test_load_from_config_db_missing_token() {
        let Some(db) = setup_test_db().await else {
            eprintln!("Skipping test_load_from_config_db_missing_token: DATABASE_URL not set");
            return;
        };

        let result = load_from_config_db(&db).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("kiro_refresh_token"));
    }
}
