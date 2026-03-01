use std::collections::HashMap;
use std::sync::atomic::Ordering;

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use chrono::Utc;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Mutex;
use sysinfo::{Pid, ProcessesToUpdate, System};
use uuid::Uuid;

use rust_embed::Embed;

use crate::auth::oauth;
use crate::config::parse_debug_mode;
use crate::error::ApiError;
use crate::routes::AppState;
use crate::web_ui::config_api::{
    classify_config_change, get_config_field_descriptions, validate_config_field, ChangeType,
};

/// Embedded React SPA assets from web-ui/dist/
#[derive(Embed)]
#[folder = "web-ui/dist/"]
struct WebAssets;

fn mime_from_path(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        _ => "application/octet-stream",
    }
}

/// Serve the SPA index page (root route).
pub async fn spa_index() -> Response {
    serve_embedded("index.html")
}

/// Fallback handler: serves embedded assets for /_ui/* paths,
/// returns 404 for everything else.
pub async fn spa_fallback(request: axum::http::Request<Body>) -> Response {
    let path = request.uri().path();
    if let Some(sub) = path.strip_prefix("/_ui/") {
        serve_embedded(sub)
    } else {
        (StatusCode::NOT_FOUND, "Not found").into_response()
    }
}

fn serve_embedded(path: &str) -> Response {
    if let Some(file) = WebAssets::get(path) {
        let content_type = mime_from_path(path);
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, content_type)],
            file.data,
        )
            .into_response()
    } else if let Some(file) = WebAssets::get("index.html") {
        // SPA fallback: serve index.html for client-side routes
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            file.data,
        )
            .into_response()
    } else {
        (StatusCode::NOT_FOUND, "Not found").into_response()
    }
}

/// GET /ui/api/metrics - Current metrics snapshot
pub async fn get_metrics(State(state): State<AppState>) -> Json<Value> {
    Json(state.metrics.to_json_snapshot())
}

/// GET /ui/api/system - System info (CPU, memory, uptime)
pub async fn get_system_info() -> Json<Value> {
    let pid = Pid::from_u32(std::process::id());
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);

    let (cpu_usage, memory) = system
        .process(pid)
        .map(|p| (p.cpu_usage(), p.memory()))
        .unwrap_or((0.0, 0));

    let uptime = System::uptime();

    Json(json!({
        "cpu_usage": cpu_usage,
        "memory_bytes": memory,
        "memory_mb": memory as f64 / 1024.0 / 1024.0,
        "uptime_seconds": uptime,
    }))
}

/// GET /ui/api/models - List available models
pub async fn get_models(State(state): State<AppState>) -> Json<Value> {
    let model_ids = state.model_cache.get_all_model_ids();
    Json(json!({ "models": model_ids }))
}

#[derive(Deserialize)]
pub struct LogsQuery {
    pub search: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// GET /ui/api/logs - Paginated log entries with optional search
pub async fn get_logs(
    State(state): State<AppState>,
    Query(params): Query<LogsQuery>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);

    let (entries, total): (Vec<Value>, usize) = if let Ok(buffer) = state.log_buffer.lock() {
        let filtered: Vec<Value> = buffer
            .iter()
            .filter(|entry| {
                params
                    .search
                    .as_ref()
                    .is_none_or(|s| entry.message.to_lowercase().contains(&s.to_lowercase()))
            })
            .skip(offset)
            .take(limit)
            .map(|entry| {
                json!({
                    "timestamp": entry.timestamp.to_rfc3339(),
                    "level": entry.level.to_string(),
                    "message": entry.message,
                })
            })
            .collect();
        let total = buffer.len();
        (filtered, total)
    } else {
        (Vec::new(), 0)
    };

    Json(json!({
        "logs": entries,
        "total": total,
        "limit": limit,
        "offset": offset,
    }))
}

/// Mask a sensitive string: show first 4 and last 4 chars, or "****" if too short.
/// Uses `.chars()` for safe character-boundary slicing (avoids UTF-8 panics).
fn mask_sensitive(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() > 8 {
        let prefix: String = chars[..4].iter().collect();
        let suffix: String = chars[chars.len() - 4..].iter().collect();
        format!("{}...{}", prefix, suffix)
    } else if chars.is_empty() {
        String::new()
    } else {
        "****".to_string()
    }
}

/// GET /ui/api/config - Current configuration (with masked secrets and setup status)
pub async fn get_config(State(state): State<AppState>) -> Json<Value> {
    let setup_complete = state.setup_complete.load(Ordering::Relaxed);

    // Clone the config snapshot and drop the read guard before any .await
    let config = state.config.read().unwrap().clone();

    let masked_key = mask_sensitive(&config.proxy_api_key);

    let masked_refresh_token = if let Some(ref db) = state.config_db {
        match db.get_refresh_token().await {
            Ok(Some(t)) => mask_sensitive(&t),
            _ => String::new(),
        }
    } else {
        String::new()
    };

    Json(json!({
        "setup_complete": setup_complete,
        "config": {
            "server_host": config.server_host,
            "server_port": config.server_port,
            "proxy_api_key": masked_key,
            "kiro_refresh_token": masked_refresh_token,
            "kiro_region": config.kiro_region,
            "oauth_sso_region": if let Some(ref db) = state.config_db {
                db.get("oauth_sso_region").await.ok().flatten().unwrap_or_default()
            } else {
                String::new()
            },
            "streaming_timeout": config.streaming_timeout,
            "first_token_timeout": config.first_token_timeout,
            "http_max_connections": config.http_max_connections,
            "http_connect_timeout": config.http_connect_timeout,
            "http_request_timeout": config.http_request_timeout,
            "http_max_retries": config.http_max_retries,
            "log_level": config.log_level,
            "debug_mode": format!("{:?}", config.debug_mode).to_lowercase(),
            "fake_reasoning_enabled": config.fake_reasoning_enabled,
            "fake_reasoning_max_tokens": config.fake_reasoning_max_tokens,
            "truncation_recovery": config.truncation_recovery,
            "tool_description_max_length": config.tool_description_max_length,
            "dashboard": config.dashboard,
        }
    }))
}

/// PUT /ui/api/config - Update configuration with validation and hot-reload
pub async fn update_config(
    State(state): State<AppState>,
    Json(updates): Json<HashMap<String, Value>>,
) -> Result<Json<Value>, ApiError> {
    // Validate all fields first
    for (key, value) in &updates {
        validate_config_field(key, value).map_err(ApiError::ValidationError)?;
    }

    let mut updated = Vec::new();
    let mut hot_reloaded = Vec::new();
    let mut requires_restart = Vec::new();

    // Persist to DB
    if let Some(ref config_db) = state.config_db {
        for (key, value) in &updates {
            let value_str = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            config_db
                .set(key, &value_str, "web_ui")
                .await
                .map_err(ApiError::Internal)?;
        }
    }

    // Classify and apply changes
    for (key, value) in &updates {
        updated.push(key.clone());

        match classify_config_change(key) {
            ChangeType::HotReload => {
                if apply_config_field(&state, key, value) {
                    hot_reloaded.push(key.clone());
                } else {
                    tracing::warn!(key = %key, value = ?value, "Failed to parse config value for hot-reload; change persisted but not applied at runtime");
                }
            }
            ChangeType::RequiresRestart => {
                requires_restart.push(key.clone());
            }
        }
    }

    Ok(Json(json!({
        "updated": updated,
        "hot_reloaded": hot_reloaded,
        "requires_restart": requires_restart,
    })))
}

/// Apply a single config field update to the runtime Config via write lock.
/// Returns `true` if the field was successfully applied, `false` if parsing failed.
fn apply_config_field(state: &AppState, key: &str, value: &Value) -> bool {
    let value_str = match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };

    let mut config = state.config.write().unwrap_or_else(|p| p.into_inner());
    match key {
        "log_level" => {
            config.log_level = value_str;
            true
        }
        "debug_mode" => {
            config.debug_mode = parse_debug_mode(&value_str);
            true
        }
        "fake_reasoning_enabled" => match value_str.parse() {
            Ok(v) => {
                config.fake_reasoning_enabled = v;
                true
            }
            Err(_) => false,
        },
        "fake_reasoning_max_tokens" => match value_str.parse() {
            Ok(v) => {
                config.fake_reasoning_max_tokens = v;
                true
            }
            Err(_) => false,
        },
        "truncation_recovery" => match value_str.parse() {
            Ok(v) => {
                config.truncation_recovery = v;
                true
            }
            Err(_) => false,
        },
        "tool_description_max_length" => match value_str.parse() {
            Ok(v) => {
                config.tool_description_max_length = v;
                true
            }
            Err(_) => false,
        },
        "first_token_timeout" => match value_str.parse() {
            Ok(v) => {
                config.first_token_timeout = v;
                true
            }
            Err(_) => false,
        },
        _ => false,
    }
}

/// Request body for initial setup.
#[derive(Deserialize)]
pub struct SetupRequest {
    pub proxy_api_key: String,
    pub kiro_refresh_token: String,
    #[serde(default = "default_region")]
    pub region: String,
}

fn default_region() -> String {
    "us-east-1".to_string()
}

/// POST /ui/api/setup - Initial setup (no auth required)
pub async fn setup(
    State(state): State<AppState>,
    Json(body): Json<SetupRequest>,
) -> Result<Json<Value>, ApiError> {
    if body.proxy_api_key.is_empty() {
        return Err(ApiError::ValidationError(
            "proxy_api_key cannot be empty".to_string(),
        ));
    }
    if body.kiro_refresh_token.is_empty() {
        return Err(ApiError::ValidationError(
            "kiro_refresh_token cannot be empty".to_string(),
        ));
    }
    if body.region.is_empty() {
        return Err(ApiError::ValidationError(
            "region cannot be empty".to_string(),
        ));
    }

    // Atomically set from false to true to prevent race conditions
    if state
        .setup_complete
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(ApiError::ValidationError(
            "Setup has already been completed".to_string(),
        ));
    }

    let config_db_ref = state
        .config_db
        .as_ref()
        .ok_or_else(|| {
            state.setup_complete.store(false, Ordering::SeqCst);
            ApiError::ConfigError("Config database not available".to_string())
        })?
        .clone();

    if let Err(e) = config_db_ref
        .save_initial_setup(&body.proxy_api_key, &body.kiro_refresh_token, &body.region)
        .await
    {
        state.setup_complete.store(false, Ordering::SeqCst);
        return Err(ApiError::Internal(e));
    }

    // Update runtime config
    {
        let mut config = state.config.write().unwrap_or_else(|p| p.into_inner());
        config.proxy_api_key = body.proxy_api_key;
        config.kiro_region = body.region;
    }

    // Reinitialize AuthManager with the newly saved credentials
    let threshold = {
        let cfg = state.config.read().unwrap_or_else(|p| p.into_inner());
        cfg.token_refresh_threshold
    };
    match crate::auth::AuthManager::new(config_db_ref, threshold).await {
        Ok(new_auth) => {
            let mut auth_lock = state.auth_manager.write().await;
            *auth_lock = new_auth;
        }
        Err(e) => {
            tracing::warn!(error = ?e, "Failed to initialize auth after setup; proxy may not work until restart");
        }
    }

    Ok(Json(json!({ "success": true })))
}

/// GET /ui/api/config/schema - Field metadata for the config UI
pub async fn get_config_schema() -> Json<Value> {
    let descriptions = get_config_field_descriptions();

    let mut fields = serde_json::Map::new();
    for (key, description) in &descriptions {
        let change_type = classify_config_change(key);
        let requires_restart = change_type == ChangeType::RequiresRestart;

        let mut field = serde_json::Map::new();
        field.insert("description".to_string(), json!(description));
        field.insert("requires_restart".to_string(), json!(requires_restart));

        match *key {
            "log_level" => {
                field.insert("type".to_string(), json!("string"));
                field.insert(
                    "options".to_string(),
                    json!(["trace", "debug", "info", "warn", "error"]),
                );
            }
            "debug_mode" => {
                field.insert("type".to_string(), json!("string"));
                field.insert("options".to_string(), json!(["off", "errors", "all"]));
            }
            "server_port"
            | "fake_reasoning_max_tokens"
            | "tool_description_max_length"
            | "first_token_timeout" => {
                field.insert("type".to_string(), json!("number"));
            }
            "fake_reasoning_enabled" | "truncation_recovery" => {
                field.insert("type".to_string(), json!("boolean"));
            }
            _ => {
                field.insert("type".to_string(), json!("string"));
            }
        }

        fields.insert(key.to_string(), Value::Object(field));
    }

    Json(json!({ "fields": fields }))
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<usize>,
}

/// Keys whose values must be masked in config history responses.
const SENSITIVE_CONFIG_KEYS: &[&str] = &["proxy_api_key", "kiro_refresh_token", "oauth_client_secret"];

/// GET /ui/api/config/history - Config change history
pub async fn get_config_history(
    State(state): State<AppState>,
    Query(params): Query<HistoryQuery>,
) -> Result<Json<Value>, ApiError> {
    let limit = params.limit.unwrap_or(50);

    if let Some(ref config_db) = state.config_db {
        let history = config_db
            .get_history(limit)
            .await
            .map_err(ApiError::Internal)?;

        let entries: Vec<Value> = history
            .iter()
            .map(|c| {
                let is_sensitive = SENSITIVE_CONFIG_KEYS.contains(&c.key.as_str());
                let old_value = if is_sensitive {
                    c.old_value.as_deref().map(mask_sensitive)
                } else {
                    c.old_value.clone()
                };
                let new_value = if is_sensitive {
                    mask_sensitive(&c.new_value)
                } else {
                    c.new_value.clone()
                };
                json!({
                    "key": c.key,
                    "old_value": old_value,
                    "new_value": new_value,
                    "changed_at": c.changed_at,
                    "source": c.source,
                })
            })
            .collect();

        return Ok(Json(json!({ "history": entries })));
    }

    Ok(Json(json!({ "history": [] })))
}

// --- OAuth flow types and state ---

struct OAuthPendingState {
    /// PKCE code_verifier (browser flow only)
    code_verifier: Option<String>,
    client_id: String,
    client_secret: String,
    client_secret_expires_at: i64,
    device_code: Option<String>,
    region: String,
    start_url: String,
    proxy_api_key: String,
    redirect_uri: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

static OAUTH_PENDING: Lazy<Mutex<HashMap<String, OAuthPendingState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Clean up expired entries (>10 min) from OAUTH_PENDING.
fn cleanup_expired_pending() {
    let now = Utc::now();
    if let Ok(mut map) = OAUTH_PENDING.lock() {
        map.retain(|_, v| (now - v.created_at).num_minutes() < 10);
    }
}

#[derive(Deserialize)]
pub struct OAuthStartRequest {
    pub region: String,
    pub start_url: String,
    pub flow: String,
    pub proxy_api_key: String,
}

/// POST /_ui/api/oauth/start - Begin OAuth flow (browser or device)
pub async fn oauth_start(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<OAuthStartRequest>,
) -> Result<Json<Value>, ApiError> {
    // Validate inputs
    if body.region.is_empty() {
        return Err(ApiError::ValidationError("region cannot be empty".into()));
    }
    if body.start_url.is_empty() {
        return Err(ApiError::ValidationError(
            "start_url cannot be empty".into(),
        ));
    }
    if body.proxy_api_key.len() < 8 {
        return Err(ApiError::ValidationError(
            "proxy_api_key must be at least 8 characters".into(),
        ));
    }
    if body.flow != "browser" && body.flow != "device" {
        return Err(ApiError::ValidationError(
            "flow must be 'browser' or 'device'".into(),
        ));
    }

    cleanup_expired_pending();

    // Build redirect_uri early so we can pass it to register_client for browser flow
    let redirect_uri = if body.flow == "browser" {
        let host = headers
            .get("host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("localhost");
        Some(format!("http://{}/_ui/api/oauth/callback", host))
    } else {
        None
    };

    // Register OAuth client (includes grantTypes + redirectUris for browser flow)
    let http_client = reqwest::Client::new();
    let start_url_opt = if body.start_url.is_empty() {
        None
    } else {
        Some(body.start_url.as_str())
    };
    let registration = oauth::register_client(
        &http_client,
        &body.region,
        &body.flow,
        redirect_uri.as_deref(),
        start_url_opt,
    )
    .await
    .map_err(ApiError::Internal)?;

    match body.flow.as_str() {
        "browser" => {
            let pkce = oauth::generate_pkce();

            let redirect_uri = redirect_uri.unwrap(); // always Some for browser flow

            let authorize_url = oauth::build_authorize_url(
                &body.region,
                &registration.client_id,
                &redirect_uri,
                &pkce,
            );

            let state_key = pkce.state.clone();
            let code_verifier = pkce.code_verifier.clone();

            if let Ok(mut map) = OAUTH_PENDING.lock() {
                map.insert(
                    state_key,
                    OAuthPendingState {
                        code_verifier: Some(code_verifier),
                        client_id: registration.client_id,
                        client_secret: registration.client_secret,
                        client_secret_expires_at: registration.client_secret_expires_at,
                        device_code: None,
                        region: body.region,
                        start_url: body.start_url,
                        proxy_api_key: body.proxy_api_key,
                        redirect_uri: Some(redirect_uri),
                        created_at: Utc::now(),
                    },
                );
            }

            Ok(Json(json!({
                "flow": "browser",
                "authorize_url": authorize_url,
            })))
        }
        "device" => {
            let device_auth = oauth::start_device_authorization(
                &http_client,
                &body.region,
                &registration.client_id,
                &registration.client_secret,
                &body.start_url,
            )
            .await
            .map_err(ApiError::Internal)?;

            let device_code_id = Uuid::new_v4().to_string();

            let response = json!({
                "flow": "device",
                "user_code": device_auth.user_code,
                "verification_uri": device_auth.verification_uri,
                "verification_uri_complete": device_auth.verification_uri_complete,
                "device_code_id": device_code_id,
                "expires_in": device_auth.expires_in,
                "interval": device_auth.interval,
            });

            if let Ok(mut map) = OAUTH_PENDING.lock() {
                map.insert(
                    device_code_id,
                    OAuthPendingState {
                        code_verifier: None,
                        client_id: registration.client_id,
                        client_secret: registration.client_secret,
                        client_secret_expires_at: registration.client_secret_expires_at,
                        device_code: Some(device_auth.device_code),
                        region: body.region,
                        start_url: body.start_url,
                        proxy_api_key: body.proxy_api_key,
                        redirect_uri: None,
                        created_at: Utc::now(),
                    },
                );
            }

            Ok(Json(response))
        }
        _ => unreachable!(),
    }
}

#[derive(Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: String,
}

/// GET /_ui/api/oauth/callback - Browser redirect callback
pub async fn oauth_callback(
    State(state): State<AppState>,
    Query(params): Query<OAuthCallbackQuery>,
) -> Result<Response, ApiError> {
    // Look up pending state
    let pending = {
        let mut map = OAUTH_PENDING
            .lock()
            .map_err(|_| ApiError::Internal(anyhow::anyhow!("Failed to lock pending state")))?;
        map.remove(&params.state)
    };

    let pending = pending.ok_or_else(|| {
        ApiError::ValidationError("Invalid or expired OAuth state parameter".into())
    })?;

    let code_verifier = pending.code_verifier.ok_or_else(|| {
        ApiError::Internal(anyhow::anyhow!("Missing PKCE code_verifier for browser flow"))
    })?;

    let redirect_uri = pending.redirect_uri.ok_or_else(|| {
        ApiError::Internal(anyhow::anyhow!("Missing redirect_uri for browser flow"))
    })?;

    // Exchange code for tokens
    let http_client = reqwest::Client::new();
    let tokens = oauth::exchange_authorization_code(
        &http_client,
        &pending.region,
        &pending.client_id,
        &params.code,
        &redirect_uri,
        &code_verifier,
    )
    .await
    .map_err(ApiError::Internal)?;

    let refresh_token = tokens
        .refresh_token
        .as_deref()
        .unwrap_or(&tokens.access_token);

    // Save to config DB
    let config_db_ref = state
        .config_db
        .as_ref()
        .ok_or_else(|| ApiError::ConfigError("Config database not available".into()))?
        .clone();

    config_db_ref
        .save_oauth_setup(
            &pending.proxy_api_key,
            refresh_token,
            &pending.region,
            &pending.client_id,
            &pending.client_secret,
            &pending.client_secret_expires_at.to_string(),
            &pending.start_url,
        )
        .await
        .map_err(ApiError::Internal)?;

    // Update runtime config
    {
        let mut config = state.config.write().unwrap_or_else(|p| p.into_inner());
        config.proxy_api_key = pending.proxy_api_key;
        config.kiro_region = "us-east-1".to_string();
    }

    // Set setup_complete
    state.setup_complete.store(true, Ordering::SeqCst);

    // Reinitialize AuthManager
    let threshold = {
        let cfg = state.config.read().unwrap_or_else(|p| p.into_inner());
        cfg.token_refresh_threshold
    };
    match crate::auth::AuthManager::new(config_db_ref, threshold).await {
        Ok(new_auth) => {
            let mut auth_lock = state.auth_manager.write().await;
            *auth_lock = new_auth;
        }
        Err(e) => {
            tracing::warn!(error = ?e, "Failed to initialize auth after OAuth setup");
        }
    }

    // Redirect back to UI
    Ok(Html(
        "<html><script>window.location='/_ui/?setup=complete'</script></html>".to_string(),
    )
    .into_response())
}

#[derive(Deserialize)]
pub struct DevicePollRequest {
    pub device_code_id: String,
}

/// POST /_ui/api/oauth/device/poll - Poll device code authorization status
pub async fn oauth_device_poll(
    State(state): State<AppState>,
    Json(body): Json<DevicePollRequest>,
) -> Result<Json<Value>, ApiError> {
    // Read pending state (don't remove yet)
    let (client_id, client_secret, client_secret_expires_at, device_code, region, start_url, proxy_api_key) = {
        let map = OAUTH_PENDING
            .lock()
            .map_err(|_| ApiError::Internal(anyhow::anyhow!("Failed to lock pending state")))?;
        let pending = map.get(&body.device_code_id).ok_or_else(|| {
            ApiError::ValidationError("Invalid or expired device_code_id".into())
        })?;
        let dc = pending.device_code.clone().ok_or_else(|| {
            ApiError::Internal(anyhow::anyhow!("Missing device_code for device flow"))
        })?;
        (
            pending.client_id.clone(),
            pending.client_secret.clone(),
            pending.client_secret_expires_at,
            dc,
            pending.region.clone(),
            pending.start_url.clone(),
            pending.proxy_api_key.clone(),
        )
    };

    let http_client = reqwest::Client::new();
    let result = oauth::poll_device_token(
        &http_client,
        &region,
        &client_id,
        &client_secret,
        &device_code,
    )
    .await
    .map_err(ApiError::Internal)?;

    use crate::auth::PollResult;
    match result {
        PollResult::Pending => Ok(Json(json!({ "status": "pending" }))),
        PollResult::SlowDown => Ok(Json(json!({ "status": "slow_down" }))),
        PollResult::Success(tokens) => {
            // Remove from pending
            if let Ok(mut map) = OAUTH_PENDING.lock() {
                map.remove(&body.device_code_id);
            }

            let refresh_token = tokens
                .refresh_token
                .as_deref()
                .unwrap_or(&tokens.access_token);

            let config_db_ref = state
                .config_db
                .as_ref()
                .ok_or_else(|| ApiError::ConfigError("Config database not available".into()))?
                .clone();

            config_db_ref
                .save_oauth_setup(
                    &proxy_api_key,
                    refresh_token,
                    &region,
                    &client_id,
                    &client_secret,
                    &client_secret_expires_at.to_string(),
                    &start_url,
                )
                .await
                .map_err(ApiError::Internal)?;

            // Update runtime config
            {
                let mut config = state.config.write().unwrap_or_else(|p| p.into_inner());
                config.proxy_api_key = proxy_api_key;
                config.kiro_region = "us-east-1".to_string();
            }

            // Set setup_complete
            state.setup_complete.store(true, Ordering::SeqCst);

            // Reinitialize AuthManager
            let threshold = {
                let cfg = state.config.read().unwrap_or_else(|p| p.into_inner());
                cfg.token_refresh_threshold
            };
            match crate::auth::AuthManager::new(config_db_ref, threshold).await {
                Ok(new_auth) => {
                    let mut auth_lock = state.auth_manager.write().await;
                    *auth_lock = new_auth;
                }
                Err(e) => {
                    tracing::warn!(error = ?e, "Failed to initialize auth after device OAuth setup");
                }
            }

            Ok(Json(json!({ "status": "complete" })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricsCollector;

    #[tokio::test]
    async fn test_get_system_info() {
        let result = get_system_info().await;
        let value = result.0;
        assert!(value["cpu_usage"].is_number());
        assert!(value["memory_bytes"].is_number());
        assert!(value["uptime_seconds"].is_number());
    }

    #[test]
    fn test_metrics_to_json_snapshot() {
        let collector = MetricsCollector::new();
        collector.record_request_end(100.0, "test-model", 50, 100);

        let snapshot = collector.to_json_snapshot();
        assert!(snapshot["total_requests"].is_number());
        assert!(snapshot["latency"]["p50"].is_number());
        assert!(snapshot["models"].is_array());

        let models = snapshot["models"].as_array().unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["name"], "test-model");
    }

    #[test]
    fn test_metrics_snapshot_empty() {
        let collector = MetricsCollector::new();
        let snapshot = collector.to_json_snapshot();
        assert_eq!(snapshot["total_requests"], 0);
        assert_eq!(snapshot["total_errors"], 0);
        assert_eq!(snapshot["active_connections"], 0);
    }

    #[test]
    fn test_mask_sensitive() {
        assert_eq!(mask_sensitive("abcdefghij"), "abcd...ghij");
        assert_eq!(mask_sensitive("short"), "****");
        assert_eq!(mask_sensitive(""), "");
        assert_eq!(mask_sensitive("12345678"), "****");
        assert_eq!(mask_sensitive("123456789"), "1234...6789");
    }

    #[tokio::test]
    async fn test_get_config_schema_has_fields() {
        let result = get_config_schema().await;
        let value = result.0;
        let fields = value["fields"].as_object().unwrap();
        assert!(fields.contains_key("log_level"));
        assert!(fields.contains_key("server_port"));
        assert!(fields.contains_key("proxy_api_key"));

        let log_level = fields["log_level"].as_object().unwrap();
        assert!(log_level.contains_key("options"));
        assert_eq!(log_level["requires_restart"], false);

        let server_port = fields["server_port"].as_object().unwrap();
        assert_eq!(server_port["requires_restart"], true);
    }
}
