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

    /// Configuration error
    #[error("Configuration error: {0}")]
    #[allow(dead_code)]
    ConfigError(String),

    /// Request validation error
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Context length exceeded (triggers /compact in Claude Code)
    #[error("Context length exceeded: {message}")]
    ContextLengthExceeded { message: String },

    /// Internal server error
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Maps an HTTP status code to the corresponding OpenAI error type string.
fn openai_error_type_for_status(status: u16) -> &'static str {
    match status {
        400 => "invalid_request_error",
        401 => "authentication_error",
        402 => "insufficient_credits",
        403 => "moderation_error",
        404 => "not_found_error",
        408 => "timeout_error",
        413 => "request_too_large",
        429 => "rate_limit_error",
        500 => "provider_error",
        502 => "upstream_error",
        529 => "overloaded_error",
        _ => "invalid_request_error",
    }
}

/// Maps an HTTP status code to the corresponding Anthropic error type string.
fn anthropic_error_type_for_status(status: u16) -> &'static str {
    match status {
        401 => "authentication_error",
        403 => "permission_error",
        404 => "not_found_error",
        413 => "request_too_large",
        429 => "rate_limit_error",
        529 => "overloaded_error",
        s if s >= 500 => "api_error",
        _ => "invalid_request_error",
    }
}

/// Renders an `ApiError` in OpenAI's error envelope format:
/// `{"error":{"code":<status>,"type":"...","message":"...","metadata":{}}}`
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match self {
            ApiError::AuthError(msg) => (StatusCode::UNAUTHORIZED, "authentication_error", msg),
            ApiError::InvalidModel(msg) => (StatusCode::BAD_REQUEST, "invalid_request_error", msg),
            ApiError::KiroApiError { status, message } => {
                let status_code =
                    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                let error_type = openai_error_type_for_status(status_code.as_u16());
                (status_code, error_type, message)
            }
            ApiError::ConfigError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "provider_error", msg),
            ApiError::ValidationError(msg) => (StatusCode::BAD_REQUEST, "invalid_request_error", msg),
            ApiError::ContextLengthExceeded { message } => {
                let body = Json(json!({
                    "error": {
                        "code": 413,
                        "type": "request_too_large",
                        "message": message,
                        "metadata": {},
                    }
                }));
                return (StatusCode::PAYLOAD_TOO_LARGE, body).into_response();
            }
            ApiError::Internal(err) => {
                tracing::error!("Internal error: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "provider_error",
                    "Internal server error".to_string(),
                )
            }
        };

        let body = Json(json!({
            "error": {
                "code": status.as_u16(),
                "type": error_type,
                "message": message,
                "metadata": {},
            }
        }));

        (status, body).into_response()
    }
}

/// Wrapper that renders an `ApiError` in Anthropic's error envelope format:
/// `{"type":"error","error":{"type":"...","message":"..."}}`
pub struct AnthropicApiError(pub ApiError);

impl IntoResponse for AnthropicApiError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match self.0 {
            ApiError::AuthError(msg) => (StatusCode::UNAUTHORIZED, "authentication_error", msg),
            ApiError::InvalidModel(msg) => (StatusCode::BAD_REQUEST, "invalid_request_error", msg),
            ApiError::KiroApiError { status, message } => {
                let status_code =
                    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                let error_type = anthropic_error_type_for_status(status_code.as_u16());
                (status_code, error_type, message)
            }
            ApiError::ConfigError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "api_error", msg),
            ApiError::ValidationError(msg) => (StatusCode::BAD_REQUEST, "invalid_request_error", msg),
            ApiError::ContextLengthExceeded { message } => {
                let body = Json(json!({
                    "type": "error",
                    "error": {
                        "type": "invalid_request_error",
                        "message": message,
                    }
                }));
                return (StatusCode::BAD_REQUEST, body).into_response();
            }
            ApiError::Internal(e) => {
                tracing::error!("Internal error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "api_error", "Internal server error".to_string())
            }
        };

        let body = Json(json!({
            "type": "error",
            "error": {
                "type": error_type,
                "message": message,
            }
        }));

        (status, body).into_response()
    }
}

/// Result type alias for API operations
#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;

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
    async fn test_error_response_conversion() {
        let err = ApiError::AuthError("Invalid token".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let err = ApiError::InvalidModel("gpt-4".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let err = ApiError::KiroApiError {
            status: 429,
            message: "Rate limit".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn test_config_error_response() {
        let err = ApiError::ConfigError("Bad config".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_validation_error_response() {
        let err = ApiError::ValidationError("Missing field".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_context_length_exceeded_openai_format() {
        let err = ApiError::ContextLengthExceeded {
            message: "Input is too long.".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["type"], "request_too_large");
        assert_eq!(json["error"]["code"], 413);
        assert_eq!(json["error"]["message"], "Input is too long.");
    }

    #[tokio::test]
    async fn test_context_length_exceeded_anthropic_format() {
        let err = AnthropicApiError(ApiError::ContextLengthExceeded {
            message: "Input is too long.".to_string(),
        });
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["type"], "error");
        assert_eq!(json["error"]["type"], "invalid_request_error");
        assert_eq!(json["error"]["message"], "Input is too long.");
    }

    #[tokio::test]
    async fn test_anthropic_error_format() {
        // Auth error
        let err = AnthropicApiError(ApiError::AuthError("bad token".to_string()));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["type"], "error");
        assert_eq!(json["error"]["type"], "authentication_error");

        // Rate limit (429)
        let err = AnthropicApiError(ApiError::KiroApiError { status: 429, message: "slow down".to_string() });
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["type"], "rate_limit_error");

        // Request too large (413)
        let err = AnthropicApiError(ApiError::KiroApiError { status: 413, message: "too big".to_string() });
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["type"], "request_too_large");

        // Overloaded (529)
        let err = AnthropicApiError(ApiError::KiroApiError { status: 529, message: "overloaded".to_string() });
        let response = err.into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["type"], "overloaded_error");
    }

    #[tokio::test]
    async fn test_internal_error_response() {
        let err = ApiError::Internal(anyhow::anyhow!("Unexpected error"));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_kiro_api_error_invalid_status() {
        let err = ApiError::KiroApiError {
            status: 1000,
            message: "Unknown error".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_kiro_api_error_various_statuses() {
        let cases = [
            (400u16, StatusCode::BAD_REQUEST),
            (403, StatusCode::FORBIDDEN),
            (404, StatusCode::NOT_FOUND),
            (500, StatusCode::INTERNAL_SERVER_ERROR),
            (503, StatusCode::SERVICE_UNAVAILABLE),
        ];
        for (status, expected) in cases {
            let err = ApiError::KiroApiError { status, message: "err".to_string() };
            assert_eq!(err.into_response().status(), expected);
        }
    }
}
