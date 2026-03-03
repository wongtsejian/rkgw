#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;

use super::{McpTransport, McpTransportError};
use crate::mcp::types::{JsonRpcRequest, JsonRpcResponse};

/// HTTP transport for MCP JSON-RPC 2.0.
///
/// Stateless POST-based transport — each request is an independent HTTP call.
/// Auth headers are injected into every request.
pub struct HttpTransport {
    url: String,
    client: Client,
    headers: HashMap<String, String>,
    timeout: Duration,
    connected: AtomicBool,
}

impl HttpTransport {
    pub fn new(url: String, headers: HashMap<String, String>, timeout_secs: u64) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .unwrap_or_default();

        Self {
            url,
            client,
            headers,
            timeout: Duration::from_secs(timeout_secs),
            connected: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl McpTransport for HttpTransport {
    async fn send_request(
        &self,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse, McpTransportError> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(McpTransportError::Closed);
        }

        let mut req_builder = self.client.post(&self.url).json(request);

        // Inject auth headers
        for (key, value) in &self.headers {
            req_builder = req_builder.header(key.as_str(), value.as_str());
        }

        let response = tokio::time::timeout(self.timeout, req_builder.send())
            .await
            .map_err(|_| McpTransportError::Timeout)?
            .map_err(|e| McpTransportError::RequestFailed(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(McpTransportError::RequestFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let json_response: JsonRpcResponse = response
            .json()
            .await
            .map_err(|e| McpTransportError::ParseError(e.to_string()))?;

        Ok(json_response)
    }

    async fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    async fn connect(&mut self) -> Result<(), McpTransportError> {
        // HTTP is stateless — validate URL format only
        if self.url.is_empty() {
            return Err(McpTransportError::ConnectionFailed(
                "Empty URL".to_string(),
            ));
        }
        if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
            return Err(McpTransportError::ConnectionFailed(format!(
                "Invalid URL scheme: {}",
                self.url
            )));
        }
        self.connected.store(true, Ordering::Relaxed);
        tracing::debug!(url = %self.url, "HTTP transport connected");
        Ok(())
    }

    async fn close(&mut self) -> Result<(), McpTransportError> {
        self.connected.store(false, Ordering::Relaxed);
        tracing::debug!(url = %self.url, "HTTP transport closed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_transport_creation() {
        let headers = HashMap::from([("Authorization".to_string(), "Bearer test".to_string())]);
        let transport = HttpTransport::new("https://example.com/mcp".to_string(), headers, 30);
        assert_eq!(transport.url, "https://example.com/mcp");
        assert!(!transport.connected.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_http_transport_connect_validates_url() {
        let mut transport = HttpTransport::new(String::new(), HashMap::new(), 30);
        let result = transport.connect().await;
        assert!(result.is_err());

        let mut transport =
            HttpTransport::new("ftp://invalid.com".to_string(), HashMap::new(), 30);
        let result = transport.connect().await;
        assert!(result.is_err());

        let mut transport =
            HttpTransport::new("https://valid.com/mcp".to_string(), HashMap::new(), 30);
        let result = transport.connect().await;
        assert!(result.is_ok());
        assert!(transport.is_connected().await);
    }

    #[tokio::test]
    async fn test_http_transport_close() {
        let mut transport =
            HttpTransport::new("https://example.com/mcp".to_string(), HashMap::new(), 30);
        transport.connect().await.unwrap();
        assert!(transport.is_connected().await);

        transport.close().await.unwrap();
        assert!(!transport.is_connected().await);
    }

    #[tokio::test]
    async fn test_http_transport_send_when_closed() {
        let transport =
            HttpTransport::new("https://example.com/mcp".to_string(), HashMap::new(), 30);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "ping".to_string(),
            params: None,
            id: Some(serde_json::json!(1)),
        };
        let result = transport.send_request(&req).await;
        assert!(matches!(result, Err(McpTransportError::Closed)));
    }
}
