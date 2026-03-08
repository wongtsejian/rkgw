use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::error::ApiError;
use crate::routes::{AppState, OAuthPendingState, SessionInfo};
use crate::web_ui::config_db::ConfigDb;

// ── Constants ─────────────────────────────────────────────────────

const GITHUB_AUTH_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER_URL: &str = "https://api.github.com/user";
const COPILOT_TOKEN_URL: &str = "https://api.github.com/copilot_internal/v2/token";
const COPILOT_USER_URL: &str = "https://api.github.com/copilot_internal/user";

const EDITOR_VERSION: &str = "vscode/1.97.2";
const EDITOR_PLUGIN_VERSION: &str = "copilot-chat/0.26.7";
const USER_AGENT_VALUE: &str = "GitHubCopilotChat/0.26.7";
const GITHUB_API_VERSION: &str = "2025-04-01";

// ── Response types ────────────────────────────────────────────────

#[derive(Serialize)]
pub struct CopilotStatusResponse {
    pub connected: bool,
    pub github_username: Option<String>,
    pub copilot_plan: Option<String>,
    pub expired: bool,
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct GitHubTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct GitHubUserResponse {
    login: Option<String>,
}

#[derive(Deserialize)]
struct CopilotTokenResponse {
    token: Option<String>,
    expires_at: Option<i64>,
    refresh_in: Option<i64>,
}

#[derive(Deserialize)]
struct CopilotUserResponse {
    copilot_plan: Option<String>,
}

// ── Routes ────────────────────────────────────────────────────────

pub fn copilot_routes() -> Router<AppState> {
    Router::new()
        .route("/copilot/connect", get(copilot_connect))
        .route("/copilot/callback", get(copilot_callback))
        .route("/copilot/status", get(copilot_status))
        .route("/copilot/disconnect", delete(copilot_disconnect))
}

// ── GET /copilot/connect ──────────────────────────────────────────

async fn copilot_connect(
    State(state): State<AppState>,
    request: axum::http::Request<axum::body::Body>,
) -> Result<Response, ApiError> {
    let _session = request
        .extensions()
        .get::<SessionInfo>()
        .ok_or_else(|| ApiError::AuthError("No session".to_string()))?;

    let (client_id, callback_url) = {
        let config = state.config.read().unwrap_or_else(|p| p.into_inner());
        (
            config.github_copilot_client_id.clone(),
            config.github_copilot_callback_url.clone(),
        )
    };

    if client_id.is_empty() || callback_url.is_empty() {
        return Err(ApiError::ConfigError(
            "GitHub Copilot OAuth not configured (GITHUB_COPILOT_CLIENT_ID, GITHUB_COPILOT_CALLBACK_URL)".to_string(),
        ));
    }

    // Cleanup expired entries
    let now = Utc::now();
    state
        .oauth_pending
        .retain(|_, v| (now - v.created_at).num_minutes() < 10);

    if state.oauth_pending.len() >= 10_000 {
        return Err(ApiError::Internal(anyhow::anyhow!(
            "Too many pending OAuth requests. Please try again later."
        )));
    }

    let csrf_state = Uuid::new_v4().to_string();

    // Store with "copilot:" prefix to avoid collision with Google SSO states
    state.oauth_pending.insert(
        format!("copilot:{}", csrf_state),
        OAuthPendingState {
            nonce: String::new(),
            pkce_verifier: String::new(),
            created_at: now,
        },
    );

    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&scope=read:user&state={}",
        GITHUB_AUTH_URL,
        urlencoding::encode(&client_id),
        urlencoding::encode(&callback_url),
        urlencoding::encode(&csrf_state),
    );

    Ok(Redirect::temporary(&auth_url).into_response())
}

// ── GET /copilot/callback ─────────────────────────────────────────

async fn copilot_callback(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<CallbackQuery>,
    request: axum::http::Request<axum::body::Body>,
) -> Result<Response, ApiError> {
    let session = request
        .extensions()
        .get::<SessionInfo>()
        .ok_or_else(|| ApiError::AuthError("No session".to_string()))?
        .clone();

    // Handle user denial
    if let Some(ref err) = params.error {
        let msg = urlencoding::encode(err);
        return Ok(
            Redirect::temporary(&format!("/_ui/profile?copilot=error&message={}", msg))
                .into_response(),
        );
    }

    let code = params
        .code
        .as_deref()
        .ok_or_else(|| ApiError::ValidationError("Missing code parameter".to_string()))?;

    let request_state = params
        .state
        .as_deref()
        .ok_or_else(|| ApiError::ValidationError("Missing state parameter".to_string()))?;

    // Validate CSRF state
    let pending_key = format!("copilot:{}", request_state);
    let pending = state
        .oauth_pending
        .remove(&pending_key)
        .ok_or_else(|| ApiError::ValidationError("Invalid or expired OAuth state".to_string()))?;

    let (_, pending_state) = pending;
    let age = Utc::now() - pending_state.created_at;
    if age.num_minutes() >= 10 {
        return Err(ApiError::ValidationError("OAuth state expired".to_string()));
    }

    let (client_id, client_secret) = {
        let config = state.config.read().unwrap_or_else(|p| p.into_inner());
        (
            config.github_copilot_client_id.clone(),
            config.github_copilot_client_secret.clone(),
        )
    };

    let http = reqwest::Client::new();

    // Step 1: Exchange code for GitHub access token
    let github_token = exchange_github_code(&http, &client_id, &client_secret, code).await?;

    // Step 2: Fetch GitHub username
    let github_username = fetch_github_username(&http, &github_token).await?;

    // Step 3: Fetch Copilot bearer token
    let copilot_resp = fetch_copilot_token(&http, &github_token).await?;

    // Step 4: Detect account type
    let copilot_plan = fetch_copilot_plan(&http, &github_token).await.ok();

    // Step 5: Compute base_url from plan
    let base_url = copilot_plan
        .as_deref()
        .map(base_url_for_plan)
        .unwrap_or("https://api.githubcopilot.com");

    // Step 6: Compute expires_at from epoch timestamp
    let expires_at = copilot_resp
        .expires_at
        .map(|ts| chrono::DateTime::from_timestamp(ts, 0))
        .flatten();

    // Step 7: Store in DB
    let db = state
        .config_db
        .as_ref()
        .ok_or_else(|| ApiError::ConfigError("Database not configured".to_string()))?;
    db.upsert_copilot_tokens(
        session.user_id,
        &github_token,
        Some(&github_username),
        copilot_resp.token.as_deref(),
        copilot_plan.as_deref(),
        Some(base_url),
        expires_at,
        copilot_resp.refresh_in,
    )
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to store copilot tokens: {}", e)))?;

    // Invalidate cache
    state.copilot_token_cache.remove(&session.user_id);

    tracing::info!(
        user_id = %session.user_id,
        github_username = %github_username,
        copilot_plan = ?copilot_plan,
        "Copilot connected"
    );

    Ok(Redirect::temporary("/_ui/profile?copilot=connected").into_response())
}

// ── GET /copilot/status ───────────────────────────────────────────

async fn copilot_status(
    State(state): State<AppState>,
    request: axum::http::Request<axum::body::Body>,
) -> Result<Json<CopilotStatusResponse>, ApiError> {
    let session = request
        .extensions()
        .get::<SessionInfo>()
        .ok_or_else(|| ApiError::AuthError("No session".to_string()))?;

    let db = state
        .config_db
        .as_ref()
        .ok_or_else(|| ApiError::ConfigError("Database not configured".to_string()))?;
    let row = db
        .get_copilot_tokens(session.user_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to get copilot tokens: {}", e)))?;

    match row {
        Some(r) => {
            let expired = r.expires_at.map(|exp| exp < Utc::now()).unwrap_or(true);

            Ok(Json(CopilotStatusResponse {
                connected: true,
                github_username: r.github_username,
                copilot_plan: r.copilot_plan,
                expired,
            }))
        }
        None => Ok(Json(CopilotStatusResponse {
            connected: false,
            github_username: None,
            copilot_plan: None,
            expired: false,
        })),
    }
}

// ── DELETE /copilot/disconnect ────────────────────────────────────

async fn copilot_disconnect(
    State(state): State<AppState>,
    request: axum::http::Request<axum::body::Body>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = request
        .extensions()
        .get::<SessionInfo>()
        .ok_or_else(|| ApiError::AuthError("No session".to_string()))?;

    let db = state
        .config_db
        .as_ref()
        .ok_or_else(|| ApiError::ConfigError("Database not configured".to_string()))?;
    db.delete_copilot_tokens(session.user_id)
        .await
        .map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Failed to delete copilot tokens: {}", e))
        })?;

    state.copilot_token_cache.remove(&session.user_id);

    tracing::info!(user_id = %session.user_id, "Copilot disconnected");

    Ok(Json(json!({ "status": "disconnected" })))
}

// ── Background token refresh ──────────────────────────────────────

pub fn spawn_copilot_token_refresh_task(
    config_db: Arc<ConfigDb>,
    copilot_token_cache: Arc<DashMap<Uuid, (String, String, Instant)>>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
        let http = reqwest::Client::new();

        loop {
            interval.tick().await;

            let expiring = match config_db.get_expiring_copilot_tokens().await {
                Ok(tokens) => tokens,
                Err(e) => {
                    tracing::error!(error = ?e, "Failed to query expiring Copilot tokens");
                    continue;
                }
            };

            if expiring.is_empty() {
                continue;
            }

            tracing::debug!(count = expiring.len(), "Refreshing expiring Copilot tokens");

            for row in &expiring {
                match fetch_copilot_token(&http, &row.github_token).await {
                    Ok(resp) => {
                        let expires_at = resp
                            .expires_at
                            .map(|ts| chrono::DateTime::from_timestamp(ts, 0))
                            .flatten();

                        let plan = row.copilot_plan.as_deref();
                        let base_url = row.base_url.as_deref().unwrap_or_else(|| {
                            plan.map(base_url_for_plan)
                                .unwrap_or("https://api.githubcopilot.com")
                        });

                        if let Err(e) = config_db
                            .upsert_copilot_tokens(
                                row.user_id,
                                &row.github_token,
                                row.github_username.as_deref(),
                                resp.token.as_deref(),
                                plan,
                                Some(base_url),
                                expires_at,
                                resp.refresh_in,
                            )
                            .await
                        {
                            tracing::error!(
                                user_id = %row.user_id,
                                error = ?e,
                                "Failed to store refreshed Copilot token"
                            );
                        } else {
                            // Invalidate cache so next request picks up fresh token
                            copilot_token_cache.remove(&row.user_id);
                            tracing::info!(
                                user_id = %row.user_id,
                                "Copilot token refreshed"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            user_id = %row.user_id,
                            error = ?e,
                            "Copilot token refresh failed, marking disconnected"
                        );
                        // Null out copilot_token + expires_at to mark as disconnected
                        if let Err(e2) = config_db
                            .upsert_copilot_tokens(
                                row.user_id,
                                &row.github_token,
                                row.github_username.as_deref(),
                                None,
                                row.copilot_plan.as_deref(),
                                row.base_url.as_deref(),
                                None,
                                None,
                            )
                            .await
                        {
                            tracing::error!(
                                user_id = %row.user_id,
                                error = ?e2,
                                "Failed to mark Copilot token as expired"
                            );
                        }
                        copilot_token_cache.remove(&row.user_id);
                    }
                }
            }
        }
    });
}

// ── GitHub API helpers ────────────────────────────────────────────

fn github_api_headers(github_token: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "authorization",
        format!("token {}", github_token).parse().unwrap(),
    );
    headers.insert("editor-version", EDITOR_VERSION.parse().unwrap());
    headers.insert(
        "editor-plugin-version",
        EDITOR_PLUGIN_VERSION.parse().unwrap(),
    );
    headers.insert("user-agent", USER_AGENT_VALUE.parse().unwrap());
    headers.insert("x-github-api-version", GITHUB_API_VERSION.parse().unwrap());
    headers
}

async fn exchange_github_code(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    code: &str,
) -> Result<String, ApiError> {
    let resp = http
        .post(GITHUB_TOKEN_URL)
        .header("accept", "application/json")
        .json(&serde_json::json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "code": code,
        }))
        .send()
        .await
        .map_err(|e| ApiError::CopilotAuthError(format!("GitHub token exchange failed: {}", e)))?;

    let body: GitHubTokenResponse = resp.json().await.map_err(|e| {
        ApiError::CopilotAuthError(format!("Failed to parse GitHub token response: {}", e))
    })?;

    if let Some(err) = body.error {
        let desc = body.error_description.unwrap_or_default();
        return Err(ApiError::CopilotAuthError(format!(
            "GitHub OAuth error: {} - {}",
            err, desc
        )));
    }

    body.access_token.ok_or_else(|| {
        ApiError::CopilotAuthError("GitHub token response missing access_token".to_string())
    })
}

async fn fetch_github_username(
    http: &reqwest::Client,
    github_token: &str,
) -> Result<String, ApiError> {
    let resp = http
        .get(GITHUB_USER_URL)
        .header("authorization", format!("token {}", github_token))
        .header("user-agent", USER_AGENT_VALUE)
        .send()
        .await
        .map_err(|e| ApiError::CopilotAuthError(format!("Failed to fetch GitHub user: {}", e)))?;

    let body: GitHubUserResponse = resp.json().await.map_err(|e| {
        ApiError::CopilotAuthError(format!("Failed to parse GitHub user response: {}", e))
    })?;

    body.login
        .ok_or_else(|| ApiError::CopilotAuthError("GitHub user response missing login".to_string()))
}

async fn fetch_copilot_token(
    http: &reqwest::Client,
    github_token: &str,
) -> Result<CopilotTokenResponse, ApiError> {
    let headers = github_api_headers(github_token);

    let resp = http
        .get(COPILOT_TOKEN_URL)
        .headers(headers)
        .send()
        .await
        .map_err(|e| ApiError::CopilotAuthError(format!("Failed to fetch Copilot token: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::CopilotAuthError(format!(
            "Copilot token API returned {}: {}",
            status, body
        )));
    }

    resp.json().await.map_err(|e| {
        ApiError::CopilotAuthError(format!("Failed to parse Copilot token response: {}", e))
    })
}

async fn fetch_copilot_plan(
    http: &reqwest::Client,
    github_token: &str,
) -> Result<String, ApiError> {
    let headers = github_api_headers(github_token);

    let resp = http
        .get(COPILOT_USER_URL)
        .headers(headers)
        .send()
        .await
        .map_err(|e| {
            ApiError::CopilotAuthError(format!("Failed to fetch Copilot user info: {}", e))
        })?;

    if !resp.status().is_success() {
        return Err(ApiError::CopilotAuthError(
            "Copilot user API returned non-success status".to_string(),
        ));
    }

    let body: CopilotUserResponse = resp.json().await.map_err(|e| {
        ApiError::CopilotAuthError(format!("Failed to parse Copilot user response: {}", e))
    })?;

    body.copilot_plan.ok_or_else(|| {
        ApiError::CopilotAuthError("Copilot user response missing copilot_plan".to_string())
    })
}

fn base_url_for_plan(plan: &str) -> &'static str {
    match plan {
        "business" => "https://api.business.githubcopilot.com",
        "enterprise" => "https://api.enterprise.githubcopilot.com",
        _ => "https://api.githubcopilot.com",
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url_for_plan() {
        assert_eq!(
            base_url_for_plan("individual"),
            "https://api.githubcopilot.com"
        );
        assert_eq!(
            base_url_for_plan("business"),
            "https://api.business.githubcopilot.com"
        );
        assert_eq!(
            base_url_for_plan("enterprise"),
            "https://api.enterprise.githubcopilot.com"
        );
        assert_eq!(
            base_url_for_plan("unknown"),
            "https://api.githubcopilot.com"
        );
    }

    #[test]
    fn test_copilot_status_response_serialization() {
        let resp = CopilotStatusResponse {
            connected: true,
            github_username: Some("octocat".to_string()),
            copilot_plan: Some("individual".to_string()),
            expired: false,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["connected"].as_bool().unwrap());
        assert_eq!(json["github_username"], "octocat");
        assert_eq!(json["copilot_plan"], "individual");
        assert!(!json["expired"].as_bool().unwrap());
    }

    #[test]
    fn test_copilot_status_response_disconnected() {
        let resp = CopilotStatusResponse {
            connected: false,
            github_username: None,
            copilot_plan: None,
            expired: false,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(!json["connected"].as_bool().unwrap());
        assert!(json["github_username"].is_null());
        assert!(json["copilot_plan"].is_null());
    }

    #[test]
    fn test_github_api_headers() {
        let headers = github_api_headers("ghp_test123");
        assert_eq!(headers["authorization"], "token ghp_test123");
        assert_eq!(headers["editor-version"], EDITOR_VERSION);
        assert_eq!(headers["editor-plugin-version"], EDITOR_PLUGIN_VERSION);
        assert_eq!(headers["user-agent"], USER_AGENT_VALUE);
        assert_eq!(headers["x-github-api-version"], GITHUB_API_VERSION);
    }

    #[test]
    fn test_github_token_response_error() {
        let json = r#"{"error":"bad_verification_code","error_description":"The code passed is incorrect or expired."}"#;
        let resp: GitHubTokenResponse = serde_json::from_str(json).unwrap();
        assert!(resp.error.is_some());
        assert!(resp.access_token.is_none());
    }

    #[test]
    fn test_github_token_response_success() {
        let json = r#"{"access_token":"gho_abc123","token_type":"bearer","scope":"read:user"}"#;
        let resp: GitHubTokenResponse = serde_json::from_str(json).unwrap();
        assert!(resp.error.is_none());
        assert_eq!(resp.access_token.unwrap(), "gho_abc123");
    }

    #[test]
    fn test_copilot_token_response_deserialization() {
        let json = r#"{"token":"tid=abc;exp=1234567890;sku=copilot_for_individuals","expires_at":1234567890,"refresh_in":1500}"#;
        let resp: CopilotTokenResponse = serde_json::from_str(json).unwrap();
        assert!(resp.token.is_some());
        assert_eq!(resp.expires_at, Some(1234567890));
        assert_eq!(resp.refresh_in, Some(1500));
    }

    #[test]
    fn test_copilot_user_response_deserialization() {
        let json = r#"{"copilot_plan":"business"}"#;
        let resp: CopilotUserResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.copilot_plan.unwrap(), "business");
    }

    #[test]
    fn test_base_url_for_plan_empty_string() {
        assert_eq!(
            base_url_for_plan(""),
            "https://api.githubcopilot.com"
        );
    }

    #[test]
    fn test_copilot_status_response_expired() {
        let resp = CopilotStatusResponse {
            connected: true,
            github_username: Some("octocat".to_string()),
            copilot_plan: Some("individual".to_string()),
            expired: true,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["expired"].as_bool().unwrap());
        assert!(json["connected"].as_bool().unwrap());
    }

    #[test]
    fn test_callback_query_deserialization_success() {
        let json = r#"{"code":"abc123","state":"xyz789"}"#;
        let q: CallbackQuery = serde_json::from_str(json).unwrap();
        assert_eq!(q.code.unwrap(), "abc123");
        assert_eq!(q.state.unwrap(), "xyz789");
        assert!(q.error.is_none());
    }

    #[test]
    fn test_callback_query_deserialization_error() {
        let json = r#"{"error":"access_denied"}"#;
        let q: CallbackQuery = serde_json::from_str(json).unwrap();
        assert!(q.code.is_none());
        assert!(q.state.is_none());
        assert_eq!(q.error.unwrap(), "access_denied");
    }

    #[test]
    fn test_copilot_token_response_partial_fields() {
        let json = r#"{"token":"tid=abc","expires_at":null,"refresh_in":null}"#;
        let resp: CopilotTokenResponse = serde_json::from_str(json).unwrap();
        assert!(resp.token.is_some());
        assert!(resp.expires_at.is_none());
        assert!(resp.refresh_in.is_none());
    }

    #[test]
    fn test_github_api_headers_token_format() {
        let headers = github_api_headers("ghp_abc");
        let auth = headers["authorization"].to_str().unwrap();
        assert!(auth.starts_with("token "));
        assert!(auth.ends_with("ghp_abc"));
    }

    #[test]
    fn test_copilot_routes_builds() {
        // Verify the router can be constructed without panicking
        let _router = copilot_routes();
    }

    #[test]
    fn test_github_user_response_missing_login() {
        let json = r#"{}"#;
        let resp: GitHubUserResponse = serde_json::from_str(json).unwrap();
        assert!(resp.login.is_none());
    }

    #[test]
    fn test_copilot_user_response_missing_plan() {
        let json = r#"{}"#;
        let resp: CopilotUserResponse = serde_json::from_str(json).unwrap();
        assert!(resp.copilot_plan.is_none());
    }
}
