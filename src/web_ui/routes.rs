use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::error::ApiError;
use crate::routes::AppState;

/// GET /ui - Dashboard page
pub async fn dashboard_page() -> Html<&'static str> {
    Html(include_str!("templates/dashboard.html"))
}

/// GET /ui/config - Config page
pub async fn config_page() -> Html<&'static str> {
    Html(include_str!("templates/config.html"))
}

/// GET /ui/static/:filename - Serve static assets
pub async fn static_asset(Path(filename): Path<String>) -> Response {
    let (content, content_type) = match filename.as_str() {
        "style.css" => (
            include_str!("static/style.css"),
            "text/css; charset=utf-8",
        ),
        "app.js" => (
            include_str!("static/app.js"),
            "application/javascript; charset=utf-8",
        ),
        _ => {
            return (StatusCode::NOT_FOUND, "Not found").into_response();
        }
    };

    ([(header::CONTENT_TYPE, content_type)], content).into_response()
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
                params.search.as_ref().map_or(true, |s| {
                    entry.message.to_lowercase().contains(&s.to_lowercase())
                })
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

/// GET /ui/api/config - Current configuration (with masked secrets)
pub async fn get_config(State(state): State<AppState>) -> Json<Value> {
    let key = &state.proxy_api_key;
    let masked_key = if key.len() > 4 {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    } else {
        "****".to_string()
    };

    Json(json!({
        "server_host": state.config.server_host,
        "server_port": state.config.server_port,
        "proxy_api_key": masked_key,
        "kiro_region": state.config.kiro_region,
        "streaming_timeout": state.config.streaming_timeout,
        "first_token_timeout": state.config.first_token_timeout,
        "http_max_connections": state.config.http_max_connections,
        "http_connect_timeout": state.config.http_connect_timeout,
        "http_request_timeout": state.config.http_request_timeout,
        "http_max_retries": state.config.http_max_retries,
        "log_level": state.config.log_level,
        "debug_mode": format!("{:?}", state.config.debug_mode),
        "fake_reasoning_enabled": state.config.fake_reasoning_enabled,
        "fake_reasoning_max_tokens": state.config.fake_reasoning_max_tokens,
        "truncation_recovery": state.config.truncation_recovery,
        "tls_enabled": state.config.tls_enabled,
        "dashboard": state.config.dashboard,
    }))
}

/// Hot-reloadable config fields (don't require restart)
const HOT_RELOAD_FIELDS: &[&str] = &[
    "log_level",
    "debug_mode",
    "fake_reasoning_enabled",
    "fake_reasoning_max_tokens",
    "truncation_recovery",
    "streaming_timeout",
    "first_token_timeout",
    "tool_description_max_length",
];

/// PUT /ui/api/config - Update configuration
pub async fn update_config(
    State(state): State<AppState>,
    Json(updates): Json<HashMap<String, Value>>,
) -> Result<Json<Value>, ApiError> {
    let valid_fields = [
        "log_level",
        "debug_mode",
        "fake_reasoning_enabled",
        "fake_reasoning_max_tokens",
        "truncation_recovery",
        "streaming_timeout",
        "first_token_timeout",
        "http_max_connections",
        "http_connect_timeout",
        "http_request_timeout",
        "http_max_retries",
        "tool_description_max_length",
    ];

    let mut updated = Vec::new();
    let mut requires_restart = Vec::new();

    for (key, _value) in &updates {
        if !valid_fields.contains(&key.as_str()) {
            return Err(ApiError::ValidationError(format!(
                "Unknown config field: {}",
                key
            )));
        }

        if HOT_RELOAD_FIELDS.contains(&key.as_str()) {
            updated.push(key.clone());
        } else {
            requires_restart.push(key.clone());
            updated.push(key.clone());
        }
    }

    if let Some(ref config_db) = state.config_db {
        for (key, value) in &updates {
            let value_str = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            config_db
                .set(key, &value_str, "web_ui")
                .map_err(|e: anyhow::Error| ApiError::Internal(e))?;
        }
    }

    Ok(Json(json!({
        "status": "ok",
        "updated": updated,
        "requires_restart": requires_restart,
    })))
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<usize>,
}

/// GET /ui/api/config/history - Config change history
pub async fn get_config_history(
    State(state): State<AppState>,
    Query(params): Query<HistoryQuery>,
) -> Result<Json<Value>, ApiError> {
    let limit = params.limit.unwrap_or(50);

    if let Some(ref config_db) = state.config_db {
        let history = config_db
            .get_history(limit)
            .map_err(|e: anyhow::Error| ApiError::Internal(e))?;

        let entries: Vec<Value> = history
            .iter()
            .map(|c| {
                json!({
                    "key": c.key,
                    "old_value": c.old_value,
                    "new_value": c.new_value,
                    "changed_at": c.changed_at,
                    "source": c.source,
                })
            })
            .collect();

        return Ok(Json(json!({ "history": entries })));
    }

    Ok(Json(json!({ "history": [] })))
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
}
