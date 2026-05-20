/// HuaweiMaasProvider — direct calls to Huawei MaaS /v1/chat/completions.
///
/// Huawei MaaS exposes an OpenAI Chat Completion-compatible API endpoint.
/// Handles both OpenAI-format requests (pass-through) and Anthropic-format requests
/// (converted to OpenAI format before forwarding).
use async_trait::async_trait;
use futures::stream::StreamExt;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::models::anthropic::AnthropicMessagesRequest;
use crate::models::openai::ChatCompletionRequest;
use crate::providers::anthropic_to_openai_body;
use crate::providers::openai_codex::openai_response_to_anthropic;
use crate::providers::traits::Provider;
use crate::providers::types::{
    ProviderContext, ProviderId, ProviderResponse, ProviderStreamResponse,
};
use crate::streaming::sse::parse_sse_stream;

const HUAWEI_MAAS_API_BASE: &str = "https://api-ap-southeast-1.modelarts-maas.com/openai/v1";

pub struct HuaweiMaasProvider {
    client: reqwest::Client,
    streaming_client: reqwest::Client,
}

impl HuaweiMaasProvider {
    pub fn new(client: reqwest::Client, streaming_client: reqwest::Client) -> Self {
        Self {
            client,
            streaming_client,
        }
    }

    fn base_url<'a>(&self, ctx: &ProviderContext<'a>) -> &'a str {
        ctx.credentials
            .base_url
            .as_deref()
            .unwrap_or(HUAWEI_MAAS_API_BASE)
    }

    fn completions_url(&self, ctx: &ProviderContext<'_>) -> String {
        format!("{}/chat/completions", self.base_url(ctx))
    }

    /// Strip image_url parts from content arrays — Huawei MaaS does not
    /// support vision. Injects a notice so the model can acknowledge the gap.
    fn normalize_request_body(body: &mut Value) {
        let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) else {
            return;
        };
        for msg in messages {
            let Some(content) = msg.get_mut("content") else {
                continue;
            };
            if let Some(parts) = content.as_array_mut() {
                let had_images = parts
                    .iter()
                    .any(|p| p.get("type").and_then(|t| t.as_str()) == Some("image_url"));
                parts.retain(|p| p.get("type").and_then(|t| t.as_str()) != Some("image_url"));
                if had_images {
                    parts.insert(
                        0,
                        json!({"type": "text", "text": "[Note: Image content was removed — this model does not support vision/image input.]"}),
                    );
                }
            }
        }
    }

    async fn send_request(
        &self,
        ctx: &ProviderContext<'_>,
        mut body: Value,
        stream: bool,
    ) -> Result<reqwest::Response, ApiError> {
        let url = self.completions_url(ctx);
        body["stream"] = json!(stream);
        Self::normalize_request_body(&mut body);

        let client = if stream {
            &self.streaming_client
        } else {
            &self.client
        };

        let response = client
            .post(&url)
            .header(
                "Authorization",
                format!("Bearer {}", ctx.credentials.access_token),
            )
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                ApiError::Internal(anyhow::anyhow!("Huawei MaaS request failed: {}", e))
            })?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let resp_headers = response.headers().clone();
            let error_text = match response.text().await {
                Ok(text) => text,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to read error response body");
                    format!("[unreadable error body: {}]", e)
                }
            };
            return Err(ApiError::ProviderApiError {
                provider: "huawei_maas".to_string(),
                status,
                message: error_text,
                headers: Some(resp_headers),
            });
        }

        Ok(response)
    }
}

impl Default for HuaweiMaasProvider {
    fn default() -> Self {
        Self::new(reqwest::Client::new(), reqwest::Client::new())
    }
}

#[async_trait]
impl Provider for HuaweiMaasProvider {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> ProviderId {
        ProviderId::HuaweiMaas
    }

    async fn execute_openai(
        &self,
        ctx: &ProviderContext<'_>,
        req: &ChatCompletionRequest,
    ) -> Result<ProviderResponse, ApiError> {
        let body = serde_json::to_value(req)
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Serialization failed: {}", e)))?;
        let response = self.send_request(ctx, body, false).await?;
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let body: Value = response.json().await.map_err(|e| {
            ApiError::Internal(anyhow::anyhow!(
                "Failed to parse Huawei MaaS response: {}",
                e
            ))
        })?;
        Ok(ProviderResponse {
            status,
            body,
            headers,
        })
    }

    async fn stream_openai(
        &self,
        ctx: &ProviderContext<'_>,
        req: &ChatCompletionRequest,
    ) -> Result<ProviderStreamResponse, ApiError> {
        let body = serde_json::to_value(req)
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Serialization failed: {}", e)))?;
        let response = self.send_request(ctx, body, true).await?;
        let headers = response.headers().clone();
        let stream = response.bytes_stream().map(|chunk| {
            chunk.map_err(|e| ApiError::Internal(anyhow::anyhow!("Stream error: {}", e)))
        });
        Ok(ProviderStreamResponse {
            headers,
            stream: Box::pin(stream),
        })
    }

    async fn execute_anthropic(
        &self,
        ctx: &ProviderContext<'_>,
        req: &AnthropicMessagesRequest,
    ) -> Result<ProviderResponse, ApiError> {
        let body = anthropic_to_openai_body(req);
        let response = self.send_request(ctx, body, false).await?;
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let body: Value = response.json().await.map_err(|e| {
            ApiError::Internal(anyhow::anyhow!(
                "Failed to parse Huawei MaaS response: {}",
                e
            ))
        })?;
        Ok(ProviderResponse {
            status,
            body,
            headers,
        })
    }

    async fn stream_anthropic(
        &self,
        ctx: &ProviderContext<'_>,
        req: &AnthropicMessagesRequest,
    ) -> Result<ProviderStreamResponse, ApiError> {
        let body = anthropic_to_openai_body(req);
        let response = self.send_request(ctx, body, true).await?;
        let headers = response.headers().clone();
        let byte_stream = response.bytes_stream();
        let sse_values = parse_sse_stream(byte_stream);
        let stream =
            crate::streaming::cross_format::wrap_openai_stream_as_anthropic(sse_values, &req.model);
        Ok(ProviderStreamResponse { headers, stream })
    }

    /// OpenAI responses need conversion when served through the Anthropic endpoint.
    fn normalize_response_for_anthropic(&self, model: &str, body: Value) -> Value {
        openai_response_to_anthropic(model, &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::anthropic::{AnthropicMessage, AnthropicMessagesRequest};
    use crate::providers::anthropic_to_openai_body;
    use crate::providers::types::ProviderCredentials;

    #[test]
    fn test_huawei_maas_provider_id() {
        assert_eq!(HuaweiMaasProvider::default().id(), ProviderId::HuaweiMaas);
    }

    #[test]
    fn test_completions_url_default() {
        let provider = HuaweiMaasProvider::default();
        let creds = ProviderCredentials {
            provider: ProviderId::HuaweiMaas,
            access_token: "hw-test".to_string(),
            base_url: None,
            account_label: "default".to_string(),
        };
        let model = "deepseek-v3".to_string();
        let ctx = ProviderContext {
            credentials: &creds,
            model: &model,
        };
        assert_eq!(
            provider.completions_url(&ctx),
            "https://api-ap-southeast-1.modelarts-maas.com/openai/v1/chat/completions"
        );
    }

    #[test]
    fn test_completions_url_custom_base() {
        let provider = HuaweiMaasProvider::default();
        let creds = ProviderCredentials {
            provider: ProviderId::HuaweiMaas,
            access_token: "hw-test".to_string(),
            base_url: Some("https://custom-maas.example.com".to_string()),
            account_label: "default".to_string(),
        };
        let model = "deepseek-v3".to_string();
        let ctx = ProviderContext {
            credentials: &creds,
            model: &model,
        };
        assert_eq!(
            provider.completions_url(&ctx),
            "https://custom-maas.example.com/chat/completions"
        );
    }

    #[test]
    fn test_anthropic_to_openai_body_basic() {
        let req = AnthropicMessagesRequest {
            model: "deepseek-v3".to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: json!("Hello"),
            }],
            max_tokens: 1000,
            system: None,
            stream: false,
            tools: None,
            tool_choice: None,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            metadata: None,
            thinking: None,
            disable_parallel_tool_use: None,
        };

        let body = anthropic_to_openai_body(&req);
        assert_eq!(body["model"], "deepseek-v3");
        assert_eq!(body["max_tokens"], 1000);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "Hello");
    }

    #[test]
    fn test_anthropic_to_openai_body_with_system() {
        let req = AnthropicMessagesRequest {
            model: "deepseek-v3".to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: json!("Hi"),
            }],
            max_tokens: 100,
            system: Some(json!("Be helpful")),
            stream: false,
            tools: None,
            tool_choice: None,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            metadata: None,
            thinking: None,
            disable_parallel_tool_use: None,
        };

        let body = anthropic_to_openai_body(&req);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "Be helpful");
        assert_eq!(body["messages"][1]["role"], "user");
    }

    #[test]
    fn test_openai_response_to_anthropic_maps_tool_calls_to_tool_use() {
        let body = json!({
            "id": "chatcmpl-hw-123",
            "choices": [{
                "message": {
                    "content": "Let me check that.",
                    "tool_calls": [{
                        "id": "call_abc",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"city\": \"Shanghai\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20
            }
        });

        let normalized = openai_response_to_anthropic("deepseek-v3", &body);
        assert_eq!(normalized["stop_reason"], "tool_use");
        let content = normalized["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "tool_use");
        assert_eq!(content[1]["name"], "get_weather");
    }

    #[test]
    fn test_openai_response_to_anthropic_maps_length_to_max_tokens() {
        let body = json!({
            "id": "chatcmpl-hw-456",
            "choices": [{
                "message": { "content": "Partial answer" },
                "finish_reason": "length"
            }],
            "usage": {
                "prompt_tokens": 11,
                "completion_tokens": 22
            }
        });

        let normalized = openai_response_to_anthropic("deepseek-v3", &body);
        assert_eq!(normalized["stop_reason"], "max_tokens");
    }

    #[test]
    fn test_normalize_request_body_leaves_text_array_intact() {
        let mut body = json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "Hello"},
                    {"type": "text", "text": "World"}
                ]
            }]
        });
        HuaweiMaasProvider::normalize_request_body(&mut body);
        let content = body["messages"][0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["text"], "Hello");
        assert_eq!(content[1]["text"], "World");
    }

    #[test]
    fn test_normalize_request_body_leaves_plain_string_content() {
        let mut body = json!({
            "messages": [{
                "role": "user",
                "content": "Already a string"
            }]
        });
        HuaweiMaasProvider::normalize_request_body(&mut body);
        assert_eq!(body["messages"][0]["content"], "Already a string");
    }

    #[test]
    fn test_normalize_request_body_strips_image_url_parts() {
        let mut body = json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "Describe this image"},
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,..."}}
                ]
            }]
        });
        HuaweiMaasProvider::normalize_request_body(&mut body);
        let content = body["messages"][0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert!(content[0]["text"].as_str().unwrap().contains("does not support vision"));
        assert_eq!(content[1]["text"], "Describe this image");
    }

    #[test]
    fn test_normalize_request_body_strips_images_in_subsequent_messages() {
        let mut body = json!({
            "messages": [
                {"role": "user", "content": [
                    {"type": "text", "text": "Hello"},
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,abc"}}
                ]},
                {"role": "assistant", "content": "I cannot see images."},
                {"role": "user", "content": "What about now?"},
                {"role": "user", "content": [
                    {"type": "text", "text": "Another image"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/img.png"}}
                ]}
            ]
        });
        HuaweiMaasProvider::normalize_request_body(&mut body);

        // First message: image stripped, notice injected
        let c0 = body["messages"][0]["content"].as_array().unwrap();
        assert_eq!(c0.len(), 2);
        assert!(c0[0]["text"].as_str().unwrap().contains("does not support vision"));
        assert_eq!(c0[1]["text"], "Hello");

        // Second message: plain string, untouched
        assert_eq!(body["messages"][1]["content"], "I cannot see images.");

        // Third message: plain string, untouched
        assert_eq!(body["messages"][2]["content"], "What about now?");

        // Fourth message: image stripped, notice injected
        let c3 = body["messages"][3]["content"].as_array().unwrap();
        assert_eq!(c3.len(), 2);
        assert!(c3[0]["text"].as_str().unwrap().contains("does not support vision"));
        assert_eq!(c3[1]["text"], "Another image");
    }

    #[test]
    fn test_normalize_request_body_no_messages_is_noop() {
        let mut body = json!({"model": "deepseek-v3"});
        HuaweiMaasProvider::normalize_request_body(&mut body);
        assert_eq!(body["model"], "deepseek-v3");
    }

    #[test]
    fn test_normalize_request_body_all_images_only_notice_remains() {
        let mut body = json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,abc"}},
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,def"}}
                ]
            }]
        });
        HuaweiMaasProvider::normalize_request_body(&mut body);
        let content = body["messages"][0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert!(content[0]["text"].as_str().unwrap().contains("does not support vision"));
    }
}
