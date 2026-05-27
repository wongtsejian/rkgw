use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ==================================================================================================
// Responses API — Request Models (POST /v1/responses)
// ==================================================================================================

/// Top-level request for the OpenAI Responses API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesApiRequest {
    pub model: String,
    pub input: ResponsesApiInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// Temperature for sampling. The Responses API spec uses full-precision floats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Top-p for nucleus sampling. The Responses API spec uses full-precision floats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ResponsesApiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// background mode — return immediately, process asynchronously.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    /// service_tier — priority routing hint (e.g. "auto", "default").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    /// safety_identifier — provider-specific safety classification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
    /// text_format — alternative to `text` for specifying structured output via
    /// a Pydantic model schema. Converted to `text` before processing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_format: Option<serde_json::Value>,
    /// context_management — automatic context window management settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_management: Option<serde_json::Value>,
    /// include — list of includable fields (e.g. ["file_search_call.results"]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
    /// prompt — prompt object for prompt management.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<serde_json::Value>,
}

/// Input can be either a plain text string or an array of input items.
///
/// A plain string is equivalent to `[{"role": "user", "content": "<string>"}]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponsesApiInput {
    /// Simple text input → converts to a user message.
    Text(String),
    /// Array of input items (messages, function_call, function_call_output).
    Items(Vec<serde_json::Value>),
}

// --------------------------------------------------------------------------------------------------
// Input item helpers (for conversion to Chat Completions format)
// --------------------------------------------------------------------------------------------------

/// A function call from a previous turn (maps to assistant message with `tool_calls`).
/// Kept for Responses API spec completeness; currently only used in tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InputFunctionCall {
    #[serde(rename = "type")]
    pub item_type: String, // always "function_call"
    pub name: String,
    pub arguments: String,
    pub call_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// A function call output / tool result (maps to tool message).
/// Kept for Responses API spec completeness; currently only used in tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InputFunctionCallOutput {
    #[serde(rename = "type")]
    pub item_type: String, // always "function_call_output"
    pub call_id: String,
    pub output: String,
}

// --------------------------------------------------------------------------------------------------
// Tool definition (flat structure, different from Chat Completions)
// --------------------------------------------------------------------------------------------------

/// A tool in the Responses API has a **flat** structure where `name`, `description`,
/// and `parameters` sit at the top level rather than nested under a `function` key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesApiTool {
    #[serde(rename = "type")]
    pub tool_type: String, // "function", "web_search_preview", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    /// Catch-all for tool-type-specific fields (e.g. search context size for web_search).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ==================================================================================================
// Responses API — Response Models
// ==================================================================================================

/// Top-level response for the OpenAI Responses API.
/// Kept for spec completeness; currently only used in tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesApiResponse {
    pub id: String,
    pub object: String, // "response"
    pub created_at: i64,
    pub status: String, // "completed" | "incomplete"
    pub model: String,
    pub output: Vec<ResponseOutputItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ResponsesApiUsage>,

    // Echo back request parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    /// Temperature echoed back from the request. Uses f64 for full precision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Top-p echoed back from the request. Uses f64 for full precision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ResponsesApiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

#[allow(dead_code)]
impl ResponsesApiResponse {
    pub fn new(
        id: String,
        model: String,
        status: String,
        output: Vec<ResponseOutputItem>,
        usage: Option<ResponsesApiUsage>,
    ) -> Self {
        Self {
            id,
            object: "response".to_string(),
            created_at: chrono::Utc::now().timestamp(),
            status,
            model,
            output,
            usage,
            parallel_tool_calls: None,
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            tool_choice: None,
            tools: None,
            text: None,
            truncation: None,
            reasoning: None,
            instructions: None,
            previous_response_id: None,
            store: None,
            metadata: None,
            user: None,
        }
    }
}

/// An output item is either a message, a function call, or a reasoning item.
/// Kept for spec completeness; currently only used in tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseOutputItem {
    /// An assistant message with text content.
    #[serde(rename = "message")]
    Message(ResponseOutputMessage),
    /// A function call from the model.
    #[serde(rename = "function_call")]
    FunctionCall(ResponseFunctionToolCall),
    /// A reasoning item (thinking tokens from reasoning models).
    #[serde(rename = "reasoning")]
    Reasoning(ResponseOutputMessage),
    /// An image generation call (e.g. DALL-E produced images).
    #[serde(rename = "image_generation_call")]
    ImageGenerationCall(ResponseImageGenerationCall),
}

/// An assistant message in the output array.
/// Kept for spec completeness; currently only used in tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseOutputMessage {
    pub id: String,
    pub status: String, // "completed" | "incomplete"
    pub role: String,   // "assistant"
    pub content: Vec<ResponseContentPart>,
}

/// A content part within a message — currently only text, but extensible.
/// Kept for spec completeness; currently only used in tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseContentPart {
    #[serde(rename = "output_text")]
    OutputText(ResponseOutputText),
}

/// A text content part inside a message.
/// Kept for spec completeness; currently only used in tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseOutputText {
    pub text: String,
    #[serde(default)]
    pub annotations: Vec<serde_json::Value>,
}

/// An image generation output item.
///
/// When a model generates images (e.g. DALL-E), the Responses API represents
/// each generated image as an `image_generation_call` output item with a
/// base64-encoded `result`.
///
/// Example:
/// ```json
/// {
///   "type": "image_generation_call",
///   "id": "img_abc123",
///   "status": "completed",
///   "result": "iVBORw0KGgo..."
/// }
/// ```
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseImageGenerationCall {
    pub id: String,
    pub status: String, // "completed" | "incomplete" | "in_progress" | "failed"
    /// Base64-encoded image data (no data-URI prefix).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

/// A function/tool call in the output array.
/// Kept for spec completeness; currently only used in tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFunctionToolCall {
    pub id: String,
    pub call_id: String,
    pub name: String,
    pub arguments: String,
    pub status: String, // "completed" | "incomplete"
}

// --------------------------------------------------------------------------------------------------
// Usage
// --------------------------------------------------------------------------------------------------

/// Kept for spec completeness; currently only used in tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesApiUsage {
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub total_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens_details: Option<InputTokensDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens_details: Option<OutputTokensDetails>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputTokensDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_tokens: Option<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputTokensDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_tokens: Option<i32>,
}

// ==================================================================================================
// Responses API — Streaming Event Models (SSE)
// ==================================================================================================

/// A single streaming event in the Responses API SSE stream.
///
/// The `type` field discriminates the event kind. We use an untagged representation
/// where each variant carries the event-specific data.
/// Kept for spec completeness; currently only used in tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseStreamEvent {
    /// The response object has been created.
    #[serde(rename = "response.created")]
    ResponseCreated { response: ResponsesApiResponse },
    /// The response is in progress (first token / processing started).
    #[serde(rename = "response.in_progress")]
    ResponseInProgress { response: ResponsesApiResponse },
    /// A new output item has been added to the response.
    #[serde(rename = "response.output_item.added")]
    OutputItemAdded {
        output_index: i32,
        item: ResponseOutputItem,
    },
    /// A new content part has been added to a message output item.
    #[serde(rename = "response.content_part.added")]
    ContentPartAdded {
        output_index: i32,
        content_index: i32,
        part: ResponseContentPart,
    },
    /// A text delta for an output_text content part.
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta {
        output_index: i32,
        content_index: i32,
        delta: String,
    },
    /// A function-call arguments delta.
    #[serde(rename = "response.function_call_arguments.delta")]
    FunctionCallArgumentsDelta {
        output_index: i32,
        call_id: String,
        delta: String,
    },
    /// A content part is done.
    #[serde(rename = "response.content_part.done")]
    ContentPartDone {
        output_index: i32,
        content_index: i32,
        part: ResponseContentPart,
    },
    /// An output item is done.
    #[serde(rename = "response.output_item.done")]
    OutputItemDone {
        output_index: i32,
        item: ResponseOutputItem,
    },
    /// The response is fully completed.
    #[serde(rename = "response.completed")]
    ResponseCompleted { response: ResponsesApiResponse },
}

// ==================================================================================================
// Helper: convert ResponsesApiInput → Vec<serde_json::Value> (Chat Completions messages)
// ==================================================================================================

impl ResponsesApiInput {
    /// Convert the input into a list of message-like JSON values suitable for
    /// conversion to Chat Completions format.
    pub fn into_message_values(self) -> Vec<serde_json::Value> {
        match self {
            ResponsesApiInput::Text(text) => {
                vec![serde_json::json!({
                    "role": "user",
                    "content": text
                })]
            }
            ResponsesApiInput::Items(items) => items,
        }
    }
}

// ==================================================================================================
// Tests
// ==================================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---- Request round-trip tests ----

    #[test]
    fn test_request_with_text_input() {
        let json = json!({
            "model": "o3",
            "input": "Tell me a bedtime story",
            "stream": true
        });
        let req: ResponsesApiRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.model, "o3");
        assert!(
            matches!(req.input, ResponsesApiInput::Text(ref s) if s == "Tell me a bedtime story")
        );
        assert_eq!(req.stream, Some(true));

        // Round-trip
        let back = serde_json::to_value(&req).unwrap();
        assert_eq!(back["model"], "o3");
        assert_eq!(back["input"], "Tell me a bedtime story");
    }

    #[test]
    fn test_request_with_array_input() {
        let json = json!({
            "model": "o3",
            "input": [
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi there!"}
            ]
        });
        let req: ResponsesApiRequest = serde_json::from_value(json).unwrap();
        assert!(matches!(req.input, ResponsesApiInput::Items(_)));

        // Round-trip
        let back = serde_json::to_value(&req).unwrap();
        assert_eq!(back["input"][0]["role"], "user");
    }

    #[test]
    fn test_request_skip_serializing_none() {
        let req = ResponsesApiRequest {
            model: "o3".to_string(),
            input: ResponsesApiInput::Text("hello".to_string()),
            instructions: None,
            stream: None,
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
            reasoning: None,
            text: None,
            metadata: None,
            previous_response_id: None,
            store: None,
            truncation: None,
            user: None,
            background: None,
            service_tier: None,
            safety_identifier: None,
            text_format: None,
            context_management: None,
            include: None,
            prompt: None,
        };
        let v = serde_json::to_value(&req).unwrap();
        // Only model and input should appear
        assert!(v.get("instructions").is_none());
        assert!(v.get("stream").is_none());
        assert!(v.get("temperature").is_none());
        assert!(v.get("tools").is_none());
    }

    #[test]
    fn test_request_full() {
        let json = json!({
            "model": "o3",
            "input": "Hello",
            "instructions": "You are a helpful assistant",
            "stream": true,
            "temperature": 0.7,
            "top_p": 0.9,
            "max_output_tokens": 4096,
            "tools": [{"type": "function", "name": "get_weather", "description": "Get weather", "parameters": {"type": "object"}}],
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "reasoning": {"effort": "medium"},
            "text": {"format": {"type": "json_schema"}},
            "metadata": {"key": "val"},
            "previous_response_id": null,
            "store": true,
            "truncation": "auto",
            "user": "user-123"
        });
        let req: ResponsesApiRequest = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(
            req.instructions.as_deref(),
            Some("You are a helpful assistant")
        );
        assert_eq!(req.temperature, Some(0.7));
        assert_eq!(req.max_output_tokens, Some(4096));
        assert!(req.tools.is_some());

        // Round-trip
        let back = serde_json::to_value(&req).unwrap();
        assert_eq!(back["model"], "o3");
        assert_eq!(back["tools"][0]["type"], "function");
    }

    // ---- Input conversion tests ----

    #[test]
    fn test_text_input_into_messages() {
        let input = ResponsesApiInput::Text("Hello world".to_string());
        let msgs = input.into_message_values();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "Hello world");
    }

    // ---- Tool model round-trip ----

    #[test]
    fn test_responses_api_tool_round_trip() {
        let tool = ResponsesApiTool {
            tool_type: "function".to_string(),
            name: Some("get_weather".to_string()),
            description: Some("Get weather info".to_string()),
            parameters: Some(json!({"type": "object"})),
            strict: Some(true),
            extra: HashMap::new(),
        };
        let v = serde_json::to_value(&tool).unwrap();
        assert_eq!(v["type"], "function");
        assert_eq!(v["name"], "get_weather");
        assert_eq!(v["strict"], true);

        let back: ResponsesApiTool = serde_json::from_value(v).unwrap();
        assert_eq!(back.tool_type, "function");
        assert_eq!(back.name.unwrap(), "get_weather");
    }

    #[test]
    fn test_responses_api_tool_with_extra_fields() {
        let json = json!({
            "type": "web_search_preview",
            "search_context_size": "high"
        });
        let tool: ResponsesApiTool = serde_json::from_value(json).unwrap();
        assert_eq!(tool.tool_type, "web_search_preview");
        assert_eq!(tool.extra["search_context_size"], "high");
    }

    // ---- Response round-trip tests ----

    #[test]
    fn test_response_round_trip() {
        let resp = ResponsesApiResponse {
            id: "resp_abc".to_string(),
            object: "response".to_string(),
            created_at: 1234567890,
            status: "completed".to_string(),
            model: "o3".to_string(),
            output: vec![ResponseOutputItem::Message(ResponseOutputMessage {
                id: "msg_1".to_string(),
                status: "completed".to_string(),
                role: "assistant".to_string(),
                content: vec![ResponseContentPart::OutputText(ResponseOutputText {
                    text: "Hello!".to_string(),
                    annotations: vec![],
                })],
            })],
            usage: Some(ResponsesApiUsage {
                input_tokens: 10,
                output_tokens: 20,
                total_tokens: 30,
                input_tokens_details: Some(InputTokensDetails {
                    cached_tokens: Some(0),
                    text_tokens: None,
                    audio_tokens: None,
                }),
                output_tokens_details: Some(OutputTokensDetails {
                    reasoning_tokens: Some(0),
                    text_tokens: None,
                    image_tokens: None,
                }),
            }),
            parallel_tool_calls: Some(true),
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_output_tokens: Some(4096),
            tool_choice: Some(json!("auto")),
            tools: Some(vec![]),
            text: Some(json!({"format": {"type": "text"}})),
            truncation: Some("disabled".to_string()),
            reasoning: Some(json!({"effort": null, "summary": null})),
            instructions: None,
            previous_response_id: None,
            store: Some(true),
            metadata: Some(json!({})),
            user: None,
        };

        let v = serde_json::to_value(&resp).unwrap();
        assert_eq!(v["id"], "resp_abc");
        assert_eq!(v["object"], "response");
        assert_eq!(v["output"][0]["type"], "message");
        assert_eq!(v["usage"]["input_tokens"], 10);

        // Round-trip
        let back: ResponsesApiResponse = serde_json::from_value(v).unwrap();
        assert_eq!(back.id, "resp_abc");
        assert_eq!(back.output.len(), 1);
    }

    #[test]
    fn test_response_with_function_call_output() {
        let resp = ResponsesApiResponse {
            id: "resp_fc".to_string(),
            object: "response".to_string(),
            created_at: 1234567890,
            status: "completed".to_string(),
            model: "o3".to_string(),
            output: vec![ResponseOutputItem::FunctionCall(ResponseFunctionToolCall {
                id: "fc_1".to_string(),
                call_id: "call_1".to_string(),
                name: "get_weather".to_string(),
                arguments: r#"{"location":"Paris"}"#.to_string(),
                status: "completed".to_string(),
            })],
            usage: None,
            parallel_tool_calls: None,
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            tool_choice: None,
            tools: None,
            text: None,
            truncation: None,
            reasoning: None,
            instructions: None,
            previous_response_id: None,
            store: None,
            metadata: None,
            user: None,
        };

        let v = serde_json::to_value(&resp).unwrap();
        assert_eq!(v["output"][0]["type"], "function_call");
        assert_eq!(v["output"][0]["name"], "get_weather");
        assert_eq!(v["output"][0]["arguments"], r#"{"location":"Paris"}"#);

        // Round-trip
        let back: ResponsesApiResponse = serde_json::from_value(v).unwrap();
        assert_eq!(back.id, "resp_fc");
    }

    // ---- Usage round-trip ----

    #[test]
    fn test_usage_round_trip() {
        let usage = ResponsesApiUsage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            input_tokens_details: Some(InputTokensDetails {
                cached_tokens: Some(80),
                text_tokens: None,
                audio_tokens: None,
            }),
            output_tokens_details: Some(OutputTokensDetails {
                reasoning_tokens: Some(20),
                text_tokens: None,
                image_tokens: None,
            }),
        };
        let v = serde_json::to_value(&usage).unwrap();
        assert_eq!(v["input_tokens_details"]["cached_tokens"], 80);
        assert_eq!(v["output_tokens_details"]["reasoning_tokens"], 20);

        let back: ResponsesApiUsage = serde_json::from_value(v).unwrap();
        assert_eq!(back.total_tokens, 150);
    }

    #[test]
    fn test_usage_skip_serializing_none() {
        let usage = ResponsesApiUsage {
            input_tokens: 10,
            output_tokens: 5,
            total_tokens: 15,
            input_tokens_details: None,
            output_tokens_details: None,
        };
        let v = serde_json::to_value(&usage).unwrap();
        assert!(v.get("input_tokens_details").is_none());
        assert!(v.get("output_tokens_details").is_none());
    }

    // ---- Streaming event tests ----

    #[test]
    fn test_stream_event_output_text_delta() {
        let event = ResponseStreamEvent::OutputTextDelta {
            output_index: 0,
            content_index: 0,
            delta: "Hello".to_string(),
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "response.output_text.delta");
        assert_eq!(v["output_index"], 0);
        assert_eq!(v["delta"], "Hello");

        // Round-trip
        let back: ResponseStreamEvent = serde_json::from_value(v).unwrap();
        if let ResponseStreamEvent::OutputTextDelta { delta, .. } = back {
            assert_eq!(delta, "Hello");
        } else {
            panic!("expected OutputTextDelta variant");
        }
    }

    #[test]
    fn test_stream_event_function_call_arguments_delta() {
        let event = ResponseStreamEvent::FunctionCallArgumentsDelta {
            output_index: 1,
            call_id: "call_abc".to_string(),
            delta: r#"{"loc"#.to_string(),
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "response.function_call_arguments.delta");
        assert_eq!(v["output_index"], 1);
        assert_eq!(v["call_id"], "call_abc");

        let back: ResponseStreamEvent = serde_json::from_value(v).unwrap();
        if let ResponseStreamEvent::FunctionCallArgumentsDelta { delta, .. } = back {
            assert_eq!(delta, r#"{"loc"#);
        } else {
            panic!("expected FunctionCallArgumentsDelta variant");
        }
    }

    #[test]
    fn test_stream_event_output_item_added() {
        let item = ResponseOutputItem::Message(ResponseOutputMessage {
            id: "msg_1".to_string(),
            status: "in_progress".to_string(),
            role: "assistant".to_string(),
            content: vec![],
        });
        let event = ResponseStreamEvent::OutputItemAdded {
            output_index: 0,
            item,
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "response.output_item.added");
        assert_eq!(v["item"]["type"], "message");
        assert_eq!(v["item"]["id"], "msg_1");

        let back: ResponseStreamEvent = serde_json::from_value(v).unwrap();
        if let ResponseStreamEvent::OutputItemAdded { output_index, .. } = back {
            assert_eq!(output_index, 0);
        } else {
            panic!("expected OutputItemAdded variant");
        }
    }

    #[test]
    fn test_stream_event_content_part_added() {
        let part = ResponseContentPart::OutputText(ResponseOutputText {
            text: "".to_string(),
            annotations: vec![],
        });
        let event = ResponseStreamEvent::ContentPartAdded {
            output_index: 0,
            content_index: 0,
            part,
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "response.content_part.added");
        assert_eq!(v["part"]["type"], "output_text");
    }

    #[test]
    fn test_stream_event_completed() {
        let resp = ResponsesApiResponse::new(
            "resp_done".to_string(),
            "o3".to_string(),
            "completed".to_string(),
            vec![],
            Some(ResponsesApiUsage {
                input_tokens: 10,
                output_tokens: 20,
                total_tokens: 30,
                input_tokens_details: None,
                output_tokens_details: None,
            }),
        );
        let event = ResponseStreamEvent::ResponseCompleted { response: resp };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "response.completed");
        assert_eq!(v["response"]["id"], "resp_done");
        assert_eq!(v["response"]["object"], "response");
    }

    // ---- Output item enum tests ----

    #[test]
    fn test_output_item_message() {
        let json = json!({
            "type": "message",
            "id": "msg_1",
            "status": "completed",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "Hi!", "annotations": []}]
        });
        let item: ResponseOutputItem = serde_json::from_value(json).unwrap();
        if let ResponseOutputItem::Message(msg) = item {
            assert_eq!(msg.id, "msg_1");
            assert_eq!(msg.content.len(), 1);
        } else {
            panic!("expected Message variant");
        }
    }

    #[test]
    fn test_output_item_function_call() {
        let json = json!({
            "type": "function_call",
            "id": "fc_1",
            "call_id": "call_1",
            "name": "get_weather",
            "arguments": "{\"location\":\"Paris\"}",
            "status": "completed"
        });
        let item: ResponseOutputItem = serde_json::from_value(json).unwrap();
        if let ResponseOutputItem::FunctionCall(fc) = item {
            assert_eq!(fc.id, "fc_1");
            assert_eq!(fc.call_id, "call_1");
            assert_eq!(fc.name, "get_weather");
        } else {
            panic!("expected FunctionCall variant");
        }
    }

    // ---- InputFunctionCall / InputFunctionCallOutput round-trips ----

    #[test]
    fn test_output_item_image_generation_call() {
        let item = ResponseOutputItem::ImageGenerationCall(ResponseImageGenerationCall {
            id: "img_abc".to_string(),
            status: "completed".to_string(),
            result: Some("iVBORw0KGgo=".to_string()),
        });
        let v = serde_json::to_value(&item).unwrap();
        assert_eq!(v["type"], "image_generation_call");
        assert_eq!(v["id"], "img_abc");
        assert_eq!(v["status"], "completed");
        assert_eq!(v["result"], "iVBORw0KGgo=");

        // Round-trip
        let back: ResponseOutputItem = serde_json::from_value(v).unwrap();
        if let ResponseOutputItem::ImageGenerationCall(img) = back {
            assert_eq!(img.id, "img_abc");
            assert_eq!(img.result.as_deref(), Some("iVBORw0KGgo="));
        } else {
            panic!("expected ImageGenerationCall variant");
        }
    }

    #[test]
    fn test_output_item_image_generation_call_no_result() {
        let item = ResponseOutputItem::ImageGenerationCall(ResponseImageGenerationCall {
            id: "img_inprogress".to_string(),
            status: "in_progress".to_string(),
            result: None,
        });
        let v = serde_json::to_value(&item).unwrap();
        assert_eq!(v["type"], "image_generation_call");
        // result should be omitted when None.
        assert!(v.get("result").is_none());
    }

    #[test]
    fn test_input_function_call_round_trip() {
        let fc = InputFunctionCall {
            item_type: "function_call".to_string(),
            name: "get_weather".to_string(),
            arguments: "{}".to_string(),
            call_id: "call_1".to_string(),
            id: Some("fc_1".to_string()),
        };
        let v = serde_json::to_value(&fc).unwrap();
        assert_eq!(v["type"], "function_call");
        assert_eq!(v["name"], "get_weather");
        assert_eq!(v["call_id"], "call_1");

        let back: InputFunctionCall = serde_json::from_value(v).unwrap();
        assert_eq!(back.name, "get_weather");
        assert_eq!(back.call_id, "call_1");
    }

    #[test]
    fn test_input_function_call_output_round_trip() {
        let fco = InputFunctionCallOutput {
            item_type: "function_call_output".to_string(),
            call_id: "call_1".to_string(),
            output: "Sunny".to_string(),
        };
        let v = serde_json::to_value(&fco).unwrap();
        assert_eq!(v["type"], "function_call_output");
        assert_eq!(v["call_id"], "call_1");

        let back: InputFunctionCallOutput = serde_json::from_value(v).unwrap();
        assert_eq!(back.output, "Sunny");
        assert_eq!(back.call_id, "call_1");
    }
}
