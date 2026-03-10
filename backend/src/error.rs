use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// API errors that can occur during request processing
#[derive(Error, Debug)]
pub enum ApiError {
    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthError(String),

    /// Invalid model name
    #[error("Invalid model: {0}")]
    #[allow(dead_code)]
    InvalidModel(String),

    /// Error from Kiro API
    #[error("Kiro API error: {status} - {message}")]
    #[allow(clippy::enum_variant_names)]
    KiroApiError { status: u16, message: String },

    /// Upstream rejected the request because the input exceeds the model's context window.
    /// Clients should compact/truncate their conversation and retry.
    #[error("Context length exceeded: {0}")]
    ContextLengthExceeded(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    #[allow(dead_code)]
    ConfigError(String),

    /// Request validation error
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Forbidden (authenticated but not authorized)
    #[error("Forbidden: {0}")]
    #[allow(dead_code)]
    Forbidden(String),

    /// Session expired or invalid
    #[error("Session expired")]
    #[allow(dead_code)]
    SessionExpired,

    /// Email domain not in allowlist
    #[error("Domain not allowed: {0}")]
    #[allow(dead_code)]
    DomainNotAllowed(String),

    /// User has no Kiro token configured
    #[error("Kiro token required")]
    #[allow(dead_code)]
    KiroTokenRequired,

    /// User's Kiro token has expired and refresh failed
    #[error("Kiro token expired")]
    #[allow(dead_code)]
    KiroTokenExpired,

    /// Cannot remove or demote the last admin
    #[error("Cannot remove or demote the last admin user")]
    #[allow(dead_code)]
    LastAdmin,

    /// Content blocked by guardrail policy
    #[error("Content blocked by guardrail policy")]
    #[allow(dead_code)]
    GuardrailBlocked {
        violations: Vec<crate::guardrails::GuardrailValidationResult>,
        processing_time_ms: u64,
    },

    /// Content redacted by guardrail (warning)
    #[error("Content redacted by guardrail")]
    #[allow(dead_code)]
    GuardrailWarning {
        violations: Vec<crate::guardrails::GuardrailValidationResult>,
        processing_time_ms: u64,
        redacted_content: String,
    },

    /// MCP transport connection failure
    #[error("MCP connection error: {0}")]
    #[allow(dead_code)]
    McpConnectionError(String),

    /// MCP tool not found in registry
    #[error("MCP tool not found: {0}")]
    #[allow(dead_code)]
    McpToolNotFound(String),

    /// MCP server returned error during tool execution
    #[error("MCP tool execution error: {0}")]
    #[allow(dead_code)]
    McpToolExecutionError(String),

    /// MCP client configuration not found
    #[error("MCP client not found: {0}")]
    #[allow(dead_code)]
    McpClientNotFound(String),

    /// MCP JSON-RPC protocol error
    #[error("MCP protocol error: {0}")]
    #[allow(dead_code)]
    McpProtocolError(String),

    /// Generic not found
    #[error("Not found: {0}")]
    #[allow(dead_code)]
    NotFound(String),

    /// Error returned by a direct provider API (Anthropic, OpenAI, Gemini)
    #[error("Provider API error ({provider}): {status} - {message}")]
    #[allow(dead_code)]
    #[allow(clippy::enum_variant_names)]
    ProviderApiError {
        provider: String,
        status: u16,
        message: String,
    },

    /// User has not configured a key for the required provider
    #[error("Provider not configured: {0}")]
    #[allow(dead_code)]
    ProviderNotConfigured(String),

    /// Copilot authentication failed (GitHub OAuth or token exchange)
    #[error("Copilot auth failed: {0}")]
    #[allow(dead_code)]
    CopilotAuthError(String),

    /// Copilot bearer token has expired and needs re-authentication
    #[error("Copilot token expired")]
    #[allow(dead_code)]
    CopilotTokenExpired,

    /// Rate limited — too many requests for a provider credential
    #[error("Rate limited: retry after {retry_after_secs}s")]
    #[allow(dead_code)]
    RateLimited {
        provider: String,
        retry_after_secs: u64,
    },

    /// Internal server error
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        api_error_into_response(self, ErrorFormat::Anthropic)
    }
}

enum ErrorFormat {
    Anthropic,
    OpenAi,
}

fn api_error_into_response(err: ApiError, format: ErrorFormat) -> Response {
    let server_err = match format {
        ErrorFormat::Anthropic => "api_error",
        ErrorFormat::OpenAi => "server_error",
    };

    let (status, error_type, message) = match err {
        ApiError::AuthError(msg) => (StatusCode::UNAUTHORIZED, "authentication_error", msg),
        ApiError::InvalidModel(msg) => (StatusCode::BAD_REQUEST, "invalid_request_error", msg),
        ApiError::KiroApiError { status, message } => {
            let status_code =
                StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let error_type = match status {
                400 => "invalid_request_error",
                401 => "authentication_error",
                403 => "permission_error",
                404 => "not_found_error",
                413 => match format {
                    ErrorFormat::Anthropic => "request_too_large",
                    ErrorFormat::OpenAi => "invalid_request_error",
                },
                429 => "rate_limit_error",
                529 => match format {
                    ErrorFormat::Anthropic => "overloaded_error",
                    ErrorFormat::OpenAi => server_err,
                },
                _ => server_err,
            };
            (status_code, error_type, message)
        }
        ApiError::ConfigError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, server_err, msg),
        ApiError::ValidationError(msg) => (StatusCode::BAD_REQUEST, "invalid_request_error", msg),
        ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, "permission_error", msg),
        ApiError::SessionExpired => (
            StatusCode::UNAUTHORIZED,
            "authentication_error",
            "Session expired".to_string(),
        ),
        ApiError::DomainNotAllowed(domain) => (
            StatusCode::FORBIDDEN,
            "permission_error",
            format!("Email domain '{}' is not in the allowlist", domain),
        ),
        ApiError::KiroTokenRequired => (
            StatusCode::FORBIDDEN,
            "permission_error",
            "Set up your Kiro token at /_ui/profile".to_string(),
        ),
        ApiError::KiroTokenExpired => (
            StatusCode::FORBIDDEN,
            "permission_error",
            "Re-authenticate your Kiro token at /_ui/profile".to_string(),
        ),
        ApiError::LastAdmin => (
            StatusCode::CONFLICT,
            "invalid_request_error",
            "Cannot remove or demote the last admin user".to_string(),
        ),
        ApiError::GuardrailBlocked {
            ref violations,
            processing_time_ms,
        } => {
            let body = match format {
                ErrorFormat::Anthropic => Json(json!({
                    "type": "error",
                    "error": {
                        "message": "Content blocked by guardrail policy",
                        "type": "guardrail_blocked",
                        "violations": violations,
                        "processing_time_ms": processing_time_ms,
                    }
                })),
                ErrorFormat::OpenAi => Json(json!({
                    "error": {
                        "message": "Content blocked by guardrail policy",
                        "type": "invalid_request_error",
                        "param": null,
                        "code": "content_policy_violation",
                        "violations": violations,
                        "processing_time_ms": processing_time_ms,
                    }
                })),
            };
            return (StatusCode::FORBIDDEN, body).into_response();
        }
        ApiError::GuardrailWarning {
            ref violations,
            processing_time_ms,
            ref redacted_content,
        } => {
            let body = match format {
                ErrorFormat::Anthropic => Json(json!({
                    "type": "error",
                    "error": {
                        "message": "Content redacted by guardrail",
                        "type": "guardrail_warning",
                        "violations": violations,
                        "processing_time_ms": processing_time_ms,
                        "redacted_content": redacted_content,
                    }
                })),
                ErrorFormat::OpenAi => Json(json!({
                    "error": {
                        "message": "Content redacted by guardrail",
                        "type": "invalid_request_error",
                        "param": null,
                        "code": "content_policy_warning",
                        "violations": violations,
                        "processing_time_ms": processing_time_ms,
                        "redacted_content": redacted_content,
                    }
                })),
            };
            return (StatusCode::OK, body).into_response();
        }
        ApiError::McpConnectionError(msg) => (StatusCode::BAD_GATEWAY, server_err, msg),
        ApiError::McpToolNotFound(msg) => (StatusCode::NOT_FOUND, "not_found_error", msg),
        ApiError::McpToolExecutionError(msg) => (StatusCode::BAD_GATEWAY, server_err, msg),
        ApiError::McpClientNotFound(msg) => (StatusCode::NOT_FOUND, "not_found_error", msg),
        ApiError::McpProtocolError(msg) => (StatusCode::BAD_GATEWAY, server_err, msg),
        ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found_error", msg),
        ApiError::ProviderApiError {
            provider,
            status,
            message,
        } => {
            let status_code =
                StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (
                status_code,
                "provider_api_error",
                format!("[{}] {}", provider, message),
            )
        }
        ApiError::ProviderNotConfigured(provider) => (
            StatusCode::FORBIDDEN,
            "provider_not_configured",
            format!(
                "No API key configured for provider '{}'. Connect it at /_ui/profile",
                provider
            ),
        ),
        ApiError::CopilotAuthError(msg) => (StatusCode::BAD_GATEWAY, "copilot_auth_error", msg),
        ApiError::CopilotTokenExpired => (
            StatusCode::FORBIDDEN,
            "copilot_token_expired",
            "Copilot token expired. Re-connect at /_ui/profile".to_string(),
        ),
        ApiError::RateLimited {
            ref provider,
            retry_after_secs,
        } => {
            let body = Json(json!({
                "error": {
                    "message": format!("Rate limited for provider '{}'. Retry after {}s.", provider, retry_after_secs),
                    "type": "rate_limited",
                    "retry_after": retry_after_secs,
                }
            }));
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", retry_after_secs.to_string())],
                body,
            )
                .into_response();
        }
        ApiError::ContextLengthExceeded(msg) => {
            let body = match format {
                ErrorFormat::Anthropic => Json(json!({
                    "type": "error",
                    "error": {
                        "message": msg,
                        "type": "invalid_request_error",
                    }
                })),
                ErrorFormat::OpenAi => Json(json!({
                    "error": {
                        "message": msg,
                        "type": "invalid_request_error",
                        "param": null,
                        "code": "context_length_exceeded",
                    }
                })),
            };
            return (StatusCode::BAD_REQUEST, body).into_response();
        }
        ApiError::Internal(err) => {
            tracing::error!("Internal error: {:?}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                server_err,
                "Internal server error".to_string(),
            )
        }
    };

    let body = match format {
        ErrorFormat::Anthropic => Json(json!({
            "type": "error",
            "error": {
                "message": message,
                "type": error_type,
            }
        })),
        ErrorFormat::OpenAi => Json(json!({
            "error": {
                "message": message,
                "type": error_type,
                "param": null,
                "code": null,
            }
        })),
    };

    (status, body).into_response()
}

/// Marker type for the `/v1/chat/completions` handler return type.
///
/// Serializes `ApiError` in OpenAI's error format via `api_error_into_response`.
/// Differences from Anthropic format: `"server_error"` instead of `"api_error"` for 5xx,
/// `"invalid_request_error"` instead of `"request_too_large"` for 413, and
/// `"param": null, "code": null` fields in every response body.
pub struct OpenAiApiError(ApiError);

impl From<ApiError> for OpenAiApiError {
    fn from(e: ApiError) -> Self {
        OpenAiApiError(e)
    }
}

impl IntoResponse for OpenAiApiError {
    fn into_response(self) -> Response {
        api_error_into_response(self.0, ErrorFormat::OpenAi)
    }
}

/// Result type alias for API operations
#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn response_error_type(err: ApiError) -> (StatusCode, String) {
        let response = err.into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let error_type = json["error"]["type"].as_str().unwrap().to_string();
        (status, error_type)
    }

    #[test]
    fn test_error_messages() {
        let err = ApiError::AuthError("Invalid token".to_string());
        assert_eq!(err.to_string(), "Authentication failed: Invalid token");

        let err = ApiError::InvalidModel("gpt-4".to_string());
        assert_eq!(err.to_string(), "Invalid model: gpt-4");

        let err = ApiError::KiroApiError {
            status: 429,
            message: "Rate limit exceeded".to_string(),
        };
        assert_eq!(err.to_string(), "Kiro API error: 429 - Rate limit exceeded");
    }

    #[test]
    fn test_config_error_message() {
        let err = ApiError::ConfigError("Missing API key".to_string());
        assert_eq!(err.to_string(), "Configuration error: Missing API key");
    }

    #[test]
    fn test_validation_error_message() {
        let err = ApiError::ValidationError("Invalid JSON".to_string());
        assert_eq!(err.to_string(), "Validation error: Invalid JSON");
    }

    #[test]
    fn test_internal_error_message() {
        let err = ApiError::Internal(anyhow::anyhow!("Something went wrong"));
        assert_eq!(err.to_string(), "Internal error: Something went wrong");
    }

    #[tokio::test]
    async fn test_auth_error_response() {
        let (status, error_type) =
            response_error_type(ApiError::AuthError("Invalid token".to_string())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(error_type, "authentication_error");
    }

    #[tokio::test]
    async fn test_invalid_model_response() {
        let (status, error_type) =
            response_error_type(ApiError::InvalidModel("gpt-4".to_string())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error_type, "invalid_request_error");
    }

    #[tokio::test]
    async fn test_validation_error_response() {
        let (status, error_type) =
            response_error_type(ApiError::ValidationError("Missing field".to_string())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error_type, "invalid_request_error");
    }

    #[tokio::test]
    async fn test_config_error_response() {
        let (status, error_type) =
            response_error_type(ApiError::ConfigError("Bad config".to_string())).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error_type, "api_error");
    }

    #[tokio::test]
    async fn test_internal_error_response() {
        let (status, error_type) =
            response_error_type(ApiError::Internal(anyhow::anyhow!("Unexpected error"))).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error_type, "api_error");
    }

    #[tokio::test]
    async fn test_forbidden_response() {
        let (status, error_type) =
            response_error_type(ApiError::Forbidden("Access denied".to_string())).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(error_type, "permission_error");
    }

    #[tokio::test]
    async fn test_session_expired_response() {
        let (status, error_type) = response_error_type(ApiError::SessionExpired).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(error_type, "authentication_error");
    }

    #[tokio::test]
    async fn test_domain_not_allowed_response() {
        let (status, error_type) =
            response_error_type(ApiError::DomainNotAllowed("evil.com".to_string())).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(error_type, "permission_error");
    }

    #[tokio::test]
    async fn test_kiro_token_required_response() {
        let (status, error_type) = response_error_type(ApiError::KiroTokenRequired).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(error_type, "permission_error");
    }

    #[tokio::test]
    async fn test_kiro_token_expired_response() {
        let (status, error_type) = response_error_type(ApiError::KiroTokenExpired).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(error_type, "permission_error");
    }

    #[tokio::test]
    async fn test_last_admin_response() {
        let (status, error_type) = response_error_type(ApiError::LastAdmin).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(error_type, "invalid_request_error");
    }

    #[tokio::test]
    async fn test_kiro_api_error_invalid_status() {
        let (status, error_type) = response_error_type(ApiError::KiroApiError {
            status: 1000,
            message: "Unknown error".to_string(),
        })
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error_type, "api_error");
    }

    #[tokio::test]
    async fn test_kiro_api_error_type_mapping() {
        let cases: &[(u16, StatusCode, &str)] = &[
            (400, StatusCode::BAD_REQUEST, "invalid_request_error"),
            (401, StatusCode::UNAUTHORIZED, "authentication_error"),
            (403, StatusCode::FORBIDDEN, "permission_error"),
            (404, StatusCode::NOT_FOUND, "not_found_error"),
            (413, StatusCode::PAYLOAD_TOO_LARGE, "request_too_large"),
            (429, StatusCode::TOO_MANY_REQUESTS, "rate_limit_error"),
            (500, StatusCode::INTERNAL_SERVER_ERROR, "api_error"),
            (503, StatusCode::SERVICE_UNAVAILABLE, "api_error"),
            (529, StatusCode::from_u16(529).unwrap(), "overloaded_error"),
        ];

        for &(status_code, expected_status, expected_type) in cases {
            let (status, error_type) = response_error_type(ApiError::KiroApiError {
                status: status_code,
                message: "test".to_string(),
            })
            .await;
            assert_eq!(status, expected_status, "status mismatch for {status_code}");
            assert_eq!(error_type, expected_type, "type mismatch for {status_code}");
        }
    }

    // Verify the Anthropic error envelope: top-level `"type": "error"` must be
    // present so clients (e.g. Claude Code) can recognise the response and
    // trigger behaviours like auto-compaction on context-length errors.
    #[tokio::test]
    async fn test_anthropic_error_envelope_top_level_type() {
        async fn top_level_type(err: ApiError) -> Option<String> {
            let response = err.into_response();
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            json["type"].as_str().map(|s| s.to_string())
        }

        // Normal path
        assert_eq!(
            top_level_type(ApiError::AuthError("bad".to_string())).await,
            Some("error".to_string())
        );
        // Early-return paths
        assert_eq!(
            top_level_type(ApiError::ContextLengthExceeded("too long".to_string())).await,
            Some("error".to_string())
        );
        assert_eq!(
            top_level_type(ApiError::GuardrailBlocked {
                violations: vec![],
                processing_time_ms: 0,
            })
            .await,
            Some("error".to_string())
        );
        // GuardrailWarning returns 200 OK, so "type": "error" is the only
        // signal to clients that the response was redacted.
        assert_eq!(
            top_level_type(ApiError::GuardrailWarning {
                violations: vec![],
                processing_time_ms: 0,
                redacted_content: String::new(),
            })
            .await,
            Some("error".to_string())
        );
    }
}

// ==================================================================================================
// OpenAiApiError tests
// ==================================================================================================

#[cfg(test)]
mod openai_error_tests {
    use super::*;
    use axum::body::to_bytes;

    async fn openai_response_fields(err: OpenAiApiError) -> (StatusCode, String, bool, bool) {
        let response = err.into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let error_type = json["error"]["type"].as_str().unwrap().to_string();
        let has_param_null = json["error"]["param"].is_null();
        let has_code_null = json["error"]["code"].is_null();
        (status, error_type, has_param_null, has_code_null)
    }

    #[tokio::test]
    async fn test_openai_error_has_param_and_code_null() {
        let (status, error_type, has_param_null, has_code_null) =
            openai_response_fields(OpenAiApiError(ApiError::AuthError("bad key".to_string())))
                .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(error_type, "authentication_error");
        assert!(has_param_null, "param should be null");
        assert!(has_code_null, "code should be null");
    }

    #[tokio::test]
    async fn test_openai_internal_error_uses_server_error() {
        let (status, error_type, _, _) =
            openai_response_fields(OpenAiApiError(ApiError::Internal(anyhow::anyhow!("boom"))))
                .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error_type, "server_error");
    }

    #[tokio::test]
    async fn test_openai_kiro_api_error_type_mapping() {
        let cases: &[(u16, StatusCode, &str)] = &[
            (400, StatusCode::BAD_REQUEST, "invalid_request_error"),
            (401, StatusCode::UNAUTHORIZED, "authentication_error"),
            (403, StatusCode::FORBIDDEN, "permission_error"),
            (404, StatusCode::NOT_FOUND, "not_found_error"),
            (413, StatusCode::PAYLOAD_TOO_LARGE, "invalid_request_error"),
            (429, StatusCode::TOO_MANY_REQUESTS, "rate_limit_error"),
            (500, StatusCode::INTERNAL_SERVER_ERROR, "server_error"),
            (503, StatusCode::SERVICE_UNAVAILABLE, "server_error"),
            (529, StatusCode::from_u16(529).unwrap(), "server_error"),
        ];

        for &(status_code, expected_status, expected_type) in cases {
            let (status, error_type, has_param_null, has_code_null) =
                openai_response_fields(OpenAiApiError(ApiError::KiroApiError {
                    status: status_code,
                    message: "test".to_string(),
                }))
                .await;
            assert_eq!(status, expected_status, "status mismatch for {status_code}");
            assert_eq!(error_type, expected_type, "type mismatch for {status_code}");
            assert!(has_param_null, "param should be null for {status_code}");
            assert!(has_code_null, "code should be null for {status_code}");
        }
    }

    #[tokio::test]
    async fn test_openai_validation_error() {
        let (status, error_type, _, _) = openai_response_fields(OpenAiApiError(
            ApiError::ValidationError("bad input".to_string()),
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error_type, "invalid_request_error");
    }

    #[tokio::test]
    async fn test_rate_limited_returns_429_with_retry_after() {
        let err = ApiError::RateLimited {
            provider: "qwen".to_string(),
            retry_after_secs: 42,
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get("retry-after").unwrap(), "42");
    }

    // OpenAI responses must NOT have a top-level "type" field — OpenAI clients
    // don't expect it and its presence would break the format contract.
    #[tokio::test]
    async fn test_openai_error_no_top_level_type() {
        async fn has_no_top_level_type(err: OpenAiApiError) -> bool {
            let response = err.into_response();
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            json["type"].is_null()
        }

        // Normal path
        assert!(has_no_top_level_type(OpenAiApiError(ApiError::AuthError("x".into()))).await);
        // Early-return paths
        assert!(
            has_no_top_level_type(OpenAiApiError(ApiError::ContextLengthExceeded("x".into())))
                .await
        );
        assert!(
            has_no_top_level_type(OpenAiApiError(ApiError::GuardrailBlocked {
                violations: vec![],
                processing_time_ms: 0,
            }))
            .await
        );
        assert!(
            has_no_top_level_type(OpenAiApiError(ApiError::GuardrailWarning {
                violations: vec![],
                processing_time_ms: 0,
                redacted_content: String::new(),
            }))
            .await
        );
    }
}
