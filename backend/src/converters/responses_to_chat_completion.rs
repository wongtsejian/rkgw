/// Convert OpenAI Responses API requests to Chat Completion API requests,
/// and convert Chat Completion responses back to Responses API responses.
///
/// Based on the LiteLLM implementation of the Responses API ↔ Chat Completions mapping.
use serde_json::{json, Value};

use crate::models::openai::{
    ChatCompletionRequest, ChatMessage, FunctionCall, FunctionTool, Tool, ToolCall, ToolFunction,
};
use crate::models::responses::{ResponsesApiInput, ResponsesApiRequest};

// ==================================================================================================
// Helper: map Chat Completions usage → Responses API usage
// ==================================================================================================

/// Map a Chat Completions usage object to a Responses API usage object.
///
/// - `prompt_tokens` → `input_tokens`
/// - `completion_tokens` → `output_tokens`
/// - `total_tokens` → `total_tokens`
/// - `prompt_tokens_details.cached_tokens` → `input_tokens_details.cached_tokens`
/// - `completion_tokens_details.reasoning_tokens` → `output_tokens_details.reasoning_tokens`
fn map_chat_usage_to_responses(usage: &Value) -> Value {
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let mut usage_obj = json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": total_tokens
    });

    // Map prompt_tokens_details.cached_tokens → input_tokens_details.cached_tokens
    if let Some(ptd) = usage.get("prompt_tokens_details") {
        let cached = ptd
            .get("cached_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        usage_obj["input_tokens_details"] = json!({ "cached_tokens": cached });
    }

    // Map completion_tokens_details.reasoning_tokens → output_tokens_details.reasoning_tokens
    if let Some(ctd) = usage.get("completion_tokens_details") {
        let reasoning = ctd
            .get("reasoning_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        usage_obj["output_tokens_details"] = json!({ "reasoning_tokens": reasoning });
    }

    usage_obj
}

// ==================================================================================================
// Content part conversion: Responses API → Chat Completions
// ==================================================================================================

/// Convert Responses API content parts to Chat Completions content parts.
///
/// The Responses API uses different content part types than Chat Completions:
/// - `input_text` → `text`
/// - `input_image` → `image_url` (with structural change: string → object)
///
/// If content is a string, it passes through unchanged.
/// If content is an array, each part is converted.
/// If content is anything else (null, object, number), it passes through unchanged.
fn convert_responses_content_parts(content: &Value) -> Value {
    match content {
        // String content — pass through as-is.
        Value::String(_) => content.clone(),
        // Array of content parts — convert each part.
        Value::Array(parts) => Value::Array(
            parts
                .iter()
                .map(|part| convert_single_content_part(part))
                .collect(),
        ),
        // Everything else (null, object, number, bool) — pass through.
        _ => content.clone(),
    }
}

/// Convert a single content part from Responses API format to Chat Completions format.
fn convert_single_content_part(part: &Value) -> Value {
    let part_type = part.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match part_type {
        // input_text → text
        "input_text" => {
            let text = part.get("text").and_then(|v| v.as_str()).unwrap_or("");
            json!({
                "type": "text",
                "text": text
            })
        }
        // input_image → image_url
        // Responses API: {"type": "input_image", "image_url": "data:..."}
        // Chat Completions: {"type": "image_url", "image_url": {"url": "data:..."}}
        "input_image" => {
            let url = part.get("image_url").and_then(|v| v.as_str()).unwrap_or("");
            let detail = part.get("detail").and_then(|v| v.as_str());
            let mut image_url_obj = json!({ "url": url });
            if let Some(d) = detail {
                image_url_obj["detail"] = json!(d);
            }
            json!({
                "type": "image_url",
                "image_url": image_url_obj
            })
        }
        // Already in Chat Completions format or unknown type — pass through.
        _ => part.clone(),
    }
}

// ==================================================================================================
// Request conversion: Responses API → Chat Completions
// ==================================================================================================

/// Convert a Responses API request into a Chat Completions request.
///
/// Mapping rules:
/// - `model` → `model`
/// - `instructions` → prepended as `{"role": "system", "content": instructions}` message
/// - `input` (string) → `{"role": "user", "content": input}`
/// - `input` (array of items) → each item maps:
///   - `{role, content}` → same role message (developer → system)
///   - `{type: "function_call", ...}` → assistant message with `tool_calls`
///   - `{type: "function_call_output", ...}` → tool message
/// - `max_output_tokens` → `max_tokens`
/// - `tools` (flat) → `tools` (nested under `function`)
/// - `tool_choice` → `tool_choice` (pass through)
/// - `temperature`, `top_p`, `stream`, `parallel_tool_calls`, `user` → pass through
/// - `text.format.type == "json_schema"` → `response_format = {"type": "json_schema", "json_schema": {...}}`
/// - `text.format.type == "json_object"` → `response_format = {"type": "json_object"}`
/// - `reasoning.effort` → `reasoning_effort` (string)
/// - If `stream` is true, set `stream_options = {"include_usage": true}`
pub fn responses_to_chat_completion(req: &ResponsesApiRequest) -> ChatCompletionRequest {
    tracing::debug!(
        model = %req.model,
        input_items = match &req.input {
            ResponsesApiInput::Text(_) => 1,
            ResponsesApiInput::Items(v) => v.len(),
        },
        "Converting Responses API request to Chat Completion request"
    );

    // H5: Warn when previous_response_id is provided but unsupported.
    if req.previous_response_id.is_some() {
        tracing::warn!(
            previous_response_id = ?req.previous_response_id,
            "previous_response_id is not supported; request will be treated as a fresh conversation"
        );
    }

    let mut messages: Vec<ChatMessage> = Vec::new();

    // Prepend instructions as a system message if present.
    if let Some(instr) = &req.instructions {
        if !instr.is_empty() {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: Some(Value::String(instr.clone())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            });
        }
    }

    // Convert input items to chat messages.
    let input_items = req.input.clone().into_message_values();
    for item in input_items {
        let item_type = item.get("type").and_then(|v| v.as_str());

        match item_type {
            Some("function_call") => {
                // Map to an assistant message with tool_calls.
                // C5: call_id is required by the spec.
                let call_id = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        tracing::warn!(
                            item_type = "function_call",
                            "Input item missing required call_id/id field; upstream provider may reject"
                        );
                        ""
                    });
                let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let arguments = item.get("arguments").and_then(|v| v.as_str()).unwrap_or("");

                messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_calls: Some(vec![ToolCall {
                        id: call_id.to_string(),
                        tool_type: "function".to_string(),
                        function: FunctionCall {
                            name: name.to_string(),
                            arguments: arguments.to_string(),
                        },
                    }]),
                    tool_call_id: None,
                });
            }
            Some("function_call_output") => {
                // Map to a tool message.
                // C5: call_id is required by the spec.
                let call_id = item
                    .get("call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        tracing::warn!(
                            item_type = "function_call_output",
                            "Input item missing required call_id field; upstream provider may reject"
                        );
                        ""
                    });
                let output = item.get("output").and_then(|v| v.as_str()).unwrap_or("");

                messages.push(ChatMessage {
                    role: "tool".to_string(),
                    content: Some(Value::String(output.to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: Some(call_id.to_string()),
                });
            }
            _ => {
                // Regular message — pass through, mapping developer → system.
                let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
                let mapped_role = match role {
                    "developer" => "system",
                    other => other,
                };
                let content = item.get("content").cloned().unwrap_or(Value::Null);

                // Convert Responses API content parts to Chat Completions format.
                // Responses API uses: input_text, input_image
                // Chat Completions uses: text, image_url
                let converted_content = convert_responses_content_parts(&content);

                messages.push(ChatMessage {
                    role: mapped_role.to_string(),
                    content: Some(converted_content),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }
    }

    // Convert tools from flat structure to nested Chat Completions structure.
    // H3: Warn when non-function tools are dropped.
    // H4: Map strict field during tool conversion.
    let tools: Option<Vec<Tool>> = req.tools.as_ref().map(|tools| {
        let total_count = tools.len();
        let converted: Vec<Tool> = tools
            .iter()
            .filter_map(|t| {
                if t.tool_type != "function" {
                    tracing::warn!(
                        tool_type = %t.tool_type,
                        name = ?t.name,
                        "Dropping non-function tool (not supported by Chat Completions API)"
                    );
                    None
                } else {
                    Some(Tool::Function(FunctionTool {
                        tool_type: "function".to_string(),
                        function: ToolFunction {
                            name: t.name.clone().unwrap_or_default(),
                            description: t.description.clone(),
                            parameters: t.parameters.clone(),
                            strict: t.strict,
                        },
                    }))
                }
            })
            .collect();
        let dropped = total_count - converted.len();
        if dropped > 0 {
            tracing::warn!(
                dropped_count = dropped,
                total_count = total_count,
                "Non-function tools were dropped; model will not be able to call them"
            );
        }
        converted
    });

    // Map text.format → response_format
    let response_format = req.text.as_ref().and_then(|text| {
        let fmt = text.get("format")?;
        let fmt_type = fmt.get("type").and_then(|v| v.as_str())?;
        match fmt_type {
            "json_schema" => {
                // Build {"type": "json_schema", "json_schema": {"name": ..., "schema": ..., "strict": ...}}
                let mut schema_obj = json!({});
                if let Some(name) = fmt.get("name") {
                    schema_obj["name"] = name.clone();
                }
                if let Some(schema) = fmt.get("schema") {
                    schema_obj["schema"] = schema.clone();
                }
                if let Some(strict) = fmt.get("strict") {
                    schema_obj["strict"] = strict.clone();
                }
                Some(json!({
                    "type": "json_schema",
                    "json_schema": schema_obj
                }))
            }
            "json_object" => Some(json!({"type": "json_object"})),
            _ => None,
        }
    });

    // Map reasoning.effort → reasoning_effort
    let reasoning_effort = req.reasoning.as_ref().and_then(|r| {
        r.get("effort")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    });

    // If stream is true, include stream_options with include_usage.
    let stream_val = req.stream.unwrap_or(false);
    let stream_options = if stream_val {
        let mut map = std::collections::HashMap::new();
        map.insert("include_usage".to_string(), serde_json::Value::Bool(true));
        Some(map)
    } else {
        None
    };

    ChatCompletionRequest {
        model: req.model.clone(),
        messages,
        stream: stream_val,
        // Cast f64→f32: the Responses API uses full-precision floats, but
        // ChatCompletionRequest uses f32 for compatibility with upstream providers.
        temperature: req.temperature.map(|t| t as f32),
        top_p: req.top_p.map(|t| t as f32),
        n: None,
        max_tokens: req.max_output_tokens,
        max_completion_tokens: None,
        stop: None,
        presence_penalty: None,
        frequency_penalty: None,
        tools,
        tool_choice: req.tool_choice.clone(),
        stream_options,
        logit_bias: None,
        logprobs: None,
        top_logprobs: None,
        user: req.user.clone(),
        seed: None,
        parallel_tool_calls: req.parallel_tool_calls,
        reasoning_effort,
        response_format,
    }
}

// ==================================================================================================
// Response conversion: Chat Completions → Responses API
// ==================================================================================================

/// Convert a Chat Completion response (as JSON) back to a Responses API response.
///
/// Mapping rules:
/// - `id` → `"resp_" + id`
/// - `object` → "response"
/// - `created` → `created_at`
/// - `model` → `model`
/// - `choices[0].message.content` → output message with `output_text` content part
/// - `choices[0].message.tool_calls` → additional `function_call` output items
/// - `choices[0].finish_reason` → `status` (stop/tool_calls → "completed", length → "incomplete")
/// - `usage.prompt_tokens` → `usage.input_tokens`
/// - `usage.completion_tokens` → `usage.output_tokens`
/// - `usage.total_tokens` → `usage.total_tokens`
/// - Echo back request parameters from the original request
pub fn chat_completion_to_responses_api(
    chat_resp: &Value,
    request_input: ResponsesApiInput,
    original_request: &ResponsesApiRequest,
) -> Value {
    let chat_id = chat_resp
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let model = chat_resp
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or(&original_request.model);
    tracing::debug!(
        chat_id = %chat_id,
        model = %model,
        "Converting Chat Completion response to Responses API response"
    );
    let resp_id = format!("resp_{}", chat_id);
    let msg_id = format!("msg_{}", chat_id);

    let created_at = chat_resp
        .get("created")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| {
            tracing::warn!(
                response_id = %chat_id,
                "Chat completion response missing or non-integer 'created' field, using current time"
            );
            chrono::Utc::now().timestamp()
        });

    let model = model.to_string();

    // H6: Check for empty or missing choices array.
    let choices_empty = chat_resp
        .get("choices")
        .and_then(|c| c.as_array())
        .map_or(true, |c| c.is_empty());
    if choices_empty {
        tracing::warn!(
            chat_id = %chat_id,
            model = %model,
            "Chat completion response has empty or missing choices array"
        );
    }

    let message = chat_resp
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"));

    let text_content = message
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    // Extract reasoning_content (thinking tokens) from the message.
    // Some providers (DeepSeek, etc.) return reasoning in this field.
    let reasoning_content = message
        .and_then(|m| m.get("reasoning_content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());

    let finish_reason_opt = chat_resp
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("finish_reason"))
        .and_then(|r| r.as_str());
    let finish_reason = finish_reason_opt.unwrap_or("stop");
    if finish_reason_opt.is_none() {
        tracing::warn!(
            chat_id = %chat_id,
            "Chat completion response missing finish_reason, defaulting to stop"
        );
    }

    let status = match finish_reason {
        "length" | "content_filter" => "incomplete",
        _ => "completed",
    };

    // Build output items.
    let mut output: Vec<Value> = Vec::new();

    // If there's reasoning content, emit a reasoning output item first.
    // This follows the OpenAI Responses API convention where reasoning
    // appears as a separate output item before the message.
    if let Some(ref reasoning_text) = reasoning_content {
        if !reasoning_text.is_empty() {
            output.push(json!({
                "type": "reasoning",
                "id": format!("rs_{}", chat_id),
                "status": status,
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": reasoning_text,
                    "annotations": []
                }]
            }));
        }
    }

    // Always emit a message output item.
    // When content is null/empty and there are tool_calls, emit empty text
    // (Codex CLI expects the message content to always be an array with output_text).
    output.push(json!({
        "type": "message",
        "id": msg_id,
        "status": status,
        "role": "assistant",
        "content": [{
            "type": "output_text",
            "text": text_content,
            "annotations": []
        }]
    }));

    // Add function_call output items for each tool_call.
    if let Some(tool_calls) = message
        .and_then(|m| m.get("tool_calls"))
        .and_then(|t| t.as_array())
    {
        for tc in tool_calls {
            let tc_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let name = tc
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let arguments = tc
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(|a| a.as_str())
                .unwrap_or("");

            output.push(json!({
                "type": "function_call",
                "id": format!("fc_{}", tc_id),
                "call_id": tc_id,
                "name": name,
                "arguments": arguments,
                "status": status
            }));
        }
    }

    // Build usage object with token details (H2: using shared helper).
    let usage = chat_resp
        .get("usage")
        .map(|u| map_chat_usage_to_responses(u));

    // Build the response, echoing back request parameters.
    let mut response = json!({
        "id": resp_id,
        "object": "response",
        "created_at": created_at,
        "status": status,
        "model": model,
        "output": output
    });

    if let Some(u) = usage {
        response["usage"] = u;
    }

    // Echo back request parameters.
    if let Some(v) = original_request.parallel_tool_calls {
        response["parallel_tool_calls"] = json!(v);
    }
    if let Some(v) = original_request.temperature {
        response["temperature"] = json!(v);
    }
    if let Some(v) = original_request.top_p {
        response["top_p"] = json!(v);
    }
    if let Some(v) = original_request.max_output_tokens {
        response["max_output_tokens"] = json!(v);
    }
    if let Some(ref v) = original_request.tool_choice {
        response["tool_choice"] = v.clone();
    }
    if let Some(ref v) = original_request.tools {
        response["tools"] = json!(v);
    }
    if let Some(ref v) = original_request.text {
        response["text"] = v.clone();
    }
    if let Some(ref v) = original_request.truncation {
        response["truncation"] = json!(v);
    }
    if let Some(ref v) = original_request.reasoning {
        response["reasoning"] = v.clone();
    }
    if let Some(ref v) = original_request.instructions {
        response["instructions"] = json!(v);
    }
    if let Some(ref v) = original_request.previous_response_id {
        response["previous_response_id"] = json!(v);
    }
    if let Some(v) = original_request.store {
        response["store"] = json!(v);
    }
    if let Some(ref v) = original_request.metadata {
        response["metadata"] = v.clone();
    }
    if let Some(ref v) = original_request.user {
        response["user"] = json!(v);
    }

    // Include input in the response (echo back).
    response["input"] = json!(request_input);

    response
}

// ==================================================================================================
// Streaming: Chat Completion SSE chunks → Responses API SSE events
// ==================================================================================================

/// A tracked function call that is in-progress during streaming.
#[derive(Debug, Clone)]
pub struct ActiveFunctionCall {
    pub output_index: i32,
    pub id: String,
    pub call_id: String,
    pub name: String,
    pub arguments: String,
}

/// Stateful transformer that converts Chat Completion SSE chunks into Responses API
/// SSE events, maintaining state across the stream lifecycle.
///
/// Fixes several issues that the previous pure-function approach could not handle:
/// - **C1**: Emits `response.created` and `response.in_progress` at stream start
/// - **C2**: Emits `output_item.done` for function_call items on stream end
/// - **C3**: Populates the `output` array in `response.completed`
/// - **M1**: `content_part.done` and `output_item.done` carry accumulated text
/// - **M2**: Reasoning content uses a proper output index instead of the `-1` hack
/// - **M6**: Maps `content_filter` finish_reason to `"incomplete"` status
pub struct ResponsesStreamTransformer {
    response_id: String,
    model: String,
    initial_events_emitted: bool,
    reasoning_started: bool,
    reasoning_done: bool,
    reasoning_text: String,
    message_started: bool,
    message_text: String,
    active_function_calls: Vec<ActiveFunctionCall>,
    chunk_id: String,
    msg_id: String,
    resp_id: String,
    created: i64,
    completed_emitted: bool,
    /// Request context for echoing parameters in `response.completed`.
    request_input: Option<ResponsesApiInput>,
    request_context: Option<Value>,
}

impl ResponsesStreamTransformer {
    /// Create a new transformer for a streaming response.
    ///
    /// `response_id` is the base identifier (e.g. `"resp_<uuid>"`) used as a
    /// fallback until the first chunk provides its own `id`.
    /// `model` is the model name to include in response metadata.
    pub fn new(response_id: &str, model: &str) -> Self {
        Self {
            response_id: response_id.to_string(),
            model: model.to_string(),
            initial_events_emitted: false,
            reasoning_started: false,
            reasoning_done: false,
            reasoning_text: String::new(),
            message_started: false,
            message_text: String::new(),
            active_function_calls: Vec::new(),
            chunk_id: response_id.to_string(),
            msg_id: format!("msg_{}", response_id),
            resp_id: format!("resp_{}", response_id),
            created: 0,
            completed_emitted: false,
            request_input: None,
            request_context: None,
        }
    }

    /// Capture the request context needed to echo parameters in `response.completed`.
    ///
    /// This should be called after constructing the transformer, before any
    /// chunks are processed.
    pub fn set_request_context(&mut self, input: ResponsesApiInput, request: &ResponsesApiRequest) {
        self.request_input = Some(input);
        let mut ctx = json!({});
        if let Some(v) = request.parallel_tool_calls {
            ctx["parallel_tool_calls"] = json!(v);
        }
        if let Some(v) = request.temperature {
            ctx["temperature"] = json!(v);
        }
        if let Some(v) = request.top_p {
            ctx["top_p"] = json!(v);
        }
        if let Some(v) = request.max_output_tokens {
            ctx["max_output_tokens"] = json!(v);
        }
        if let Some(ref v) = request.tool_choice {
            ctx["tool_choice"] = v.clone();
        }
        if let Some(ref v) = request.tools {
            ctx["tools"] = json!(v);
        }
        if let Some(ref v) = request.text {
            ctx["text"] = v.clone();
        }
        if let Some(ref v) = request.truncation {
            ctx["truncation"] = json!(v);
        }
        if let Some(ref v) = request.reasoning {
            ctx["reasoning"] = v.clone();
        }
        if let Some(ref v) = request.instructions {
            ctx["instructions"] = json!(v);
        }
        if let Some(ref v) = request.previous_response_id {
            ctx["previous_response_id"] = json!(v);
        }
        if let Some(v) = request.store {
            ctx["store"] = json!(v);
        }
        if let Some(ref v) = request.metadata {
            ctx["metadata"] = v.clone();
        }
        if let Some(ref v) = request.user {
            ctx["user"] = json!(v);
        }
        self.request_context = Some(ctx);
    }

    /// The output index for the message item.
    /// If reasoning was emitted, the message comes after it at index 1; otherwise index 0.
    fn message_output_index(&self) -> i32 {
        if self.reasoning_started {
            1
        } else {
            0
        }
    }

    /// Emit `response.created` and `response.in_progress` if they haven't been emitted yet.
    fn ensure_initial_events(&mut self, events: &mut Vec<String>) {
        if self.initial_events_emitted {
            return;
        }
        self.initial_events_emitted = true;

        let response_obj = json!({
            "id": &self.resp_id,
            "object": "response",
            "created_at": self.created,
            "status": "in_progress",
            "model": &self.model,
            "output": []
        });

        events.push(format_sse_event(
            "response.created",
            &json!({
                "type": "response.created",
                "response": response_obj
            }),
        ));

        events.push(format_sse_event(
            "response.in_progress",
            &json!({
                "type": "response.in_progress",
                "response": response_obj
            }),
        ));
    }

    /// Close the reasoning output item: emit `content_part.done` + `output_item.done`.
    fn close_reasoning_item(&mut self, events: &mut Vec<String>, status: &str) {
        if !self.reasoning_started || self.reasoning_done {
            return;
        }
        self.reasoning_done = true;

        events.push(format_sse_event(
            "response.content_part.done",
            &json!({
                "type": "response.content_part.done",
                "output_index": 0,
                "content_index": 0,
                "part": {
                    "type": "output_text",
                    "text": &self.reasoning_text,
                    "annotations": []
                }
            }),
        ));

        events.push(format_sse_event(
            "response.output_item.done",
            &json!({
                "type": "response.output_item.done",
                "output_index": 0,
                "item": {
                    "type": "reasoning",
                    "id": format!("rs_{}", &self.chunk_id),
                    "status": status,
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": &self.reasoning_text,
                        "annotations": []
                    }]
                }
            }),
        ));
    }

    /// Emit the completion-level events shared between the finish_reason handler
    /// in `transform_chunk` and `finalize`.
    ///
    /// This includes: closing reasoning, starting the message item if needed,
    /// emitting `output_item.done` for active function calls, `content_part.done`
    /// and `output_item.done` for the message, building the output array,
    /// constructing the completed response object, merging request context,
    /// emitting `response.completed`, and setting `completed_emitted = true`.
    fn emit_completion_events(
        &mut self,
        events: &mut Vec<String>,
        status: &str,
        usage: Option<&Value>,
    ) {
        // Close reasoning item if still open.
        self.close_reasoning_item(events, status);

        // Ensure the message item has been started.
        if !self.message_started {
            self.start_message_item(events);
        }

        let msg_output_index = self.message_output_index();

        // Emit output_item.done for each active function call.
        for fc in &self.active_function_calls {
            events.push(format_sse_event(
                "response.output_item.done",
                &json!({
                    "type": "response.output_item.done",
                    "output_index": fc.output_index,
                    "item": {
                        "type": "function_call",
                        "id": &fc.id,
                        "call_id": &fc.call_id,
                        "name": &fc.name,
                        "arguments": &fc.arguments,
                        "status": status
                    }
                }),
            ));
        }

        // Emit response.content_part.done (message)
        events.push(format_sse_event(
            "response.content_part.done",
            &json!({
                "type": "response.content_part.done",
                "output_index": msg_output_index,
                "content_index": 0,
                "part": {
                    "type": "output_text",
                    "text": &self.message_text,
                    "annotations": []
                }
            }),
        ));

        // Emit response.output_item.done (message item)
        events.push(format_sse_event(
            "response.output_item.done",
            &json!({
                "type": "response.output_item.done",
                "output_index": msg_output_index,
                "item": {
                    "type": "message",
                    "id": &self.msg_id,
                    "status": status,
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": &self.message_text,
                        "annotations": []
                    }]
                }
            }),
        ));

        // Build the output array for response.completed.
        let mut output: Vec<Value> = Vec::new();

        if self.reasoning_started {
            output.push(json!({
                "type": "reasoning",
                "id": format!("rs_{}", &self.chunk_id),
                "status": status,
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": &self.reasoning_text,
                    "annotations": []
                }]
            }));
        }

        output.push(json!({
            "type": "message",
            "id": &self.msg_id,
            "status": status,
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": &self.message_text,
                "annotations": []
            }]
        }));

        for fc in &self.active_function_calls {
            output.push(json!({
                "type": "function_call",
                "id": &fc.id,
                "call_id": &fc.call_id,
                "name": &fc.name,
                "arguments": &fc.arguments,
                "status": status
            }));
        }

        // Build the completed response object.
        let mut completed_response = json!({
            "id": &self.resp_id,
            "object": "response",
            "created_at": self.created,
            "status": status,
            "model": &self.model,
            "output": output
        });

        // Include usage if provided (typically from the last chunk).
        if let Some(usage) = usage {
            completed_response["usage"] = map_chat_usage_to_responses(usage);
        }

        // Echo back request parameters in the completed response (H1).
        if let Some(ref ctx) = self.request_context {
            for (key, value) in ctx.as_object().unwrap_or(&serde_json::Map::new()) {
                completed_response[key] = value.clone();
            }
        }
        if let Some(ref input) = self.request_input {
            completed_response["input"] = json!(input);
        }

        // Emit response.completed
        events.push(format_sse_event(
            "response.completed",
            &json!({
                "type": "response.completed",
                "response": completed_response
            }),
        ));

        self.completed_emitted = true;
    }

    /// Start the message output item: emit `output_item.added` + `content_part.added`.
    fn start_message_item(&mut self, events: &mut Vec<String>) {
        if self.message_started {
            return;
        }
        self.message_started = true;

        let msg_output_index = self.message_output_index();

        events.push(format_sse_event(
            "response.output_item.added",
            &json!({
                "type": "response.output_item.added",
                "output_index": msg_output_index,
                "item": {
                    "type": "message",
                    "id": &self.msg_id,
                    "status": "in_progress",
                    "role": "assistant",
                    "content": []
                }
            }),
        ));

        events.push(format_sse_event(
            "response.content_part.added",
            &json!({
                "type": "response.content_part.added",
                "output_index": msg_output_index,
                "content_index": 0,
                "part": {
                    "type": "output_text",
                    "text": "",
                    "annotations": []
                }
            }),
        ));
    }

    /// Convert a single Chat Completion SSE chunk into one or more Responses API SSE events.
    ///
    /// Each event is formatted as `event: <type>\ndata: <json>\n\n`.
    ///
    /// The transformer maintains state across calls so that:
    /// - Initial lifecycle events (`response.created`, `response.in_progress`) are emitted once
    /// - Text and function-call arguments are accumulated for `done` events
    /// - Reasoning content gets its own output item with a real output index
    /// - The `output` array in `response.completed` is populated
    pub fn transform_chunk(&mut self, chunk: &Value) -> Vec<String> {
        let mut events = Vec::new();

        let choices = chunk.get("choices").and_then(|c| c.as_array());

        // Capture chunk metadata on first chunk.
        if let Some(id) = chunk.get("id").and_then(|v| v.as_str()) {
            if self.chunk_id == self.response_id {
                self.chunk_id = id.to_string();
                self.msg_id = format!("msg_{}", id);
                self.resp_id = format!("resp_{}", id);
            }
        }
        if let Some(c) = chunk.get("created").and_then(|v| v.as_i64()) {
            if self.created == 0 {
                self.created = c;
            }
        }

        if let Some(choices) = choices {
            for choice in choices {
                let delta = choice.get("delta");
                let finish_reason = choice
                    .get("finish_reason")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty() && s != &"null");

                if let Some(delta) = delta {
                    // ── 1. Reasoning content delta ────────────────────────────────
                    // Process before role so that the reasoning item is started
                    // before any message item.
                    if let Some(reasoning) = delta.get("reasoning_content").and_then(|c| c.as_str())
                    {
                        if !reasoning.is_empty() {
                            self.ensure_initial_events(&mut events);

                            if !self.reasoning_started {
                                self.reasoning_started = true;
                                events.push(format_sse_event(
                                    "response.output_item.added",
                                    &json!({
                                        "type": "response.output_item.added",
                                        "output_index": 0,
                                        "item": {
                                            "type": "reasoning",
                                            "id": format!("rs_{}", &self.chunk_id),
                                            "status": "in_progress",
                                            "role": "assistant",
                                            "content": []
                                        }
                                    }),
                                ));
                                events.push(format_sse_event(
                                    "response.content_part.added",
                                    &json!({
                                        "type": "response.content_part.added",
                                        "output_index": 0,
                                        "content_index": 0,
                                        "part": {
                                            "type": "output_text",
                                            "text": "",
                                            "annotations": []
                                        }
                                    }),
                                ));
                            }

                            self.reasoning_text.push_str(reasoning);
                            events.push(format_sse_event(
                                "response.output_text.delta",
                                &json!({
                                    "type": "response.output_text.delta",
                                    "output_index": 0,
                                    "content_index": 0,
                                    "delta": reasoning
                                }),
                            ));
                        }
                    }

                    // ── 2. Role delta ─────────────────────────────────────────────
                    if let Some(role) = delta.get("role").and_then(|r| r.as_str()) {
                        if role == "assistant" {
                            // Start the message item now unless reasoning is active
                            // (in which case we delay until content arrives).
                            if !self.reasoning_started && !self.message_started {
                                self.ensure_initial_events(&mut events);
                                self.start_message_item(&mut events);
                            }
                        }
                    }

                    // ── 3. Content delta ──────────────────────────────────────────
                    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                        if !content.is_empty() {
                            self.ensure_initial_events(&mut events);

                            if !self.message_started {
                                // Transition from reasoning → message: close reasoning first.
                                self.close_reasoning_item(&mut events, "completed");
                                self.start_message_item(&mut events);
                            }

                            self.message_text.push_str(content);
                            let msg_output_index = self.message_output_index();
                            events.push(format_sse_event(
                                "response.output_text.delta",
                                &json!({
                                    "type": "response.output_text.delta",
                                    "output_index": msg_output_index,
                                    "content_index": 0,
                                    "delta": content
                                }),
                            ));
                        }
                    }

                    // ── 4. Tool calls delta ───────────────────────────────────────
                    if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                        for tc in tool_calls {
                            let tc_index =
                                tc.get("index").and_then(|v| v.as_i64()).unwrap_or_else(|| {
                                    tracing::warn!(
                                        "Streaming tool_call delta missing 'index' field, defaulting to 0"
                                    );
                                    0
                                }) as i32;
                            let msg_output_index = self.message_output_index();
                            let fc_output_index = msg_output_index + 1 + tc_index;

                            // New function call (has id).
                            if tc.get("id").is_some() {
                                self.ensure_initial_events(&mut events);

                                if !self.message_started {
                                    self.close_reasoning_item(&mut events, "completed");
                                    self.start_message_item(&mut events);
                                }

                                let tc_id = tc
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let name = tc
                                    .get("function")
                                    .and_then(|f| f.get("name"))
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                events.push(format_sse_event(
                                    "response.output_item.added",
                                    &json!({
                                        "type": "response.output_item.added",
                                        "output_index": fc_output_index,
                                        "item": {
                                            "type": "function_call",
                                            "id": format!("fc_{}", tc_id),
                                            "call_id": tc_id,
                                            "name": name,
                                            "arguments": "",
                                            "status": "in_progress"
                                        }
                                    }),
                                ));

                                self.active_function_calls.push(ActiveFunctionCall {
                                    output_index: fc_output_index,
                                    id: format!("fc_{}", tc_id),
                                    call_id: tc_id,
                                    name,
                                    arguments: String::new(),
                                });
                            }

                            // Arguments delta.
                            if let Some(args) = tc
                                .get("function")
                                .and_then(|f| f.get("arguments"))
                                .and_then(|a| a.as_str())
                            {
                                if !args.is_empty() {
                                    // Accumulate into the matching active function call.
                                    if let Some(fc) = self
                                        .active_function_calls
                                        .iter_mut()
                                        .find(|fc| fc.output_index == fc_output_index)
                                    {
                                        fc.arguments.push_str(args);
                                    } else {
                                        tracing::warn!(
                                            output_index = fc_output_index,
                                            "Tool call arguments delta received for unmatched output_index"
                                        );
                                    }

                                    // Include call_id for client-side correlation per the Responses API spec.
                                    let call_id = self
                                        .active_function_calls
                                        .iter()
                                        .find(|fc| fc.output_index == fc_output_index)
                                        .map(|fc| fc.call_id.as_str())
                                        .unwrap_or("");

                                    events.push(format_sse_event(
                                        "response.function_call_arguments.delta",
                                        &json!({
                                            "type": "response.function_call_arguments.delta",
                                            "output_index": fc_output_index,
                                            "call_id": call_id,
                                            "delta": args
                                        }),
                                    ));
                                }
                            }
                        }
                    }
                }

                // ── Handle finish_reason ─────────────────────────────────────────
                if let Some(reason) = finish_reason {
                    let status = match reason {
                        "length" | "content_filter" => "incomplete",
                        _ => "completed",
                    };

                    self.ensure_initial_events(&mut events);
                    let usage = chunk.get("usage");
                    self.emit_completion_events(&mut events, status, usage);
                }
            }
        }

        events
    }

    /// Emit closing events if the stream ended without a `finish_reason`.
    ///
    /// If the transformer has started emitting events (i.e. `response.created` /
    /// `response.in_progress` were sent) but `response.completed` has not yet been
    /// emitted, this method emits the remaining lifecycle events with status
    /// `"incomplete"`:
    ///
    /// 1. Close the reasoning item if still open
    /// 2. Start the message item if not yet started
    /// 3. `content_part.done` and `output_item.done` for the message
    /// 4. `output_item.done` for each active function call
    /// 5. `response.completed` with status `"incomplete"`
    ///
    /// If no initial events were emitted (empty stream), this is a no-op.
    pub fn finalize(&mut self) -> Vec<String> {
        let mut events = Vec::new();

        // Nothing to finalize if we never started or already completed.
        if !self.initial_events_emitted || self.completed_emitted {
            return events;
        }

        self.emit_completion_events(&mut events, "incomplete", None);
        events
    }
}

/// Format a single SSE event as `event: <type>\ndata: <json>\n\n`.
fn format_sse_event(event_type: &str, data: &Value) -> String {
    let json = serde_json::to_string(data).unwrap_or_else(|e| {
        tracing::error!(
            event_type = event_type,
            error = %e,
            "Failed to serialize SSE event data"
        );
        "{}".to_string()
    });
    format!("event: {}\ndata: {}\n\n", event_type, json)
}

// ==================================================================================================
// Tests
// ==================================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::responses::{ResponsesApiRequest, ResponsesApiTool};
    use serde_json::json;
    use std::collections::HashMap;

    /// Helper to build a minimal ResponsesApiRequest.
    fn make_req(model: &str, input: ResponsesApiInput) -> ResponsesApiRequest {
        ResponsesApiRequest {
            model: model.to_string(),
            input,
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
        }
    }

    // ---- Request conversion tests ----

    #[test]
    fn test_simple_text_input() {
        let req = make_req("gpt-4o", ResponsesApiInput::Text("Hello".to_string()));
        let out = responses_to_chat_completion(&req);
        assert_eq!(out.model, "gpt-4o");
        assert_eq!(out.messages.len(), 1);
        assert_eq!(out.messages[0].role, "user");
        assert_eq!(out.messages[0].content, Some(json!("Hello")));
    }

    #[test]
    fn test_instructions_to_system_message() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hello".to_string()));
        req.instructions = Some("Be concise".to_string());
        let out = responses_to_chat_completion(&req);
        assert_eq!(out.messages.len(), 2);
        assert_eq!(out.messages[0].role, "system");
        assert_eq!(out.messages[0].content, Some(json!("Be concise")));
        assert_eq!(out.messages[1].role, "user");
    }

    #[test]
    fn test_instructions_empty_not_prepended() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hello".to_string()));
        req.instructions = Some("".to_string());
        let out = responses_to_chat_completion(&req);
        // Empty instructions should NOT produce a system message.
        assert_eq!(out.messages.len(), 1);
        assert_eq!(out.messages[0].role, "user");
    }

    #[test]
    fn test_array_input_with_regular_messages() {
        let input = ResponsesApiInput::Items(vec![
            json!({"role": "user", "content": "Hello"}),
            json!({"role": "assistant", "content": "Hi there"}),
        ]);
        let req = make_req("gpt-4o", input);
        let out = responses_to_chat_completion(&req);
        assert_eq!(out.messages.len(), 2);
        assert_eq!(out.messages[0].role, "user");
        assert_eq!(out.messages[0].content, Some(json!("Hello")));
        assert_eq!(out.messages[1].role, "assistant");
        assert_eq!(out.messages[1].content, Some(json!("Hi there")));
    }

    #[test]
    fn test_developer_role_mapped_to_system() {
        let input = ResponsesApiInput::Items(vec![json!({
            "role": "developer",
            "content": "You are a helpful assistant."
        })]);
        let req = make_req("gpt-4o", input);
        let out = responses_to_chat_completion(&req);
        assert_eq!(out.messages.len(), 1);
        assert_eq!(out.messages[0].role, "system");
        assert_eq!(
            out.messages[0].content,
            Some(json!("You are a helpful assistant."))
        );
    }

    #[test]
    fn test_function_call_input_item() {
        let input = ResponsesApiInput::Items(vec![
            json!({"role": "user", "content": "Call the function"}),
            json!({
                "type": "function_call",
                "name": "get_weather",
                "arguments": "{\"city\": \"SF\"}",
                "call_id": "call_abc123"
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_abc123",
                "output": "Sunny, 72°F"
            }),
        ]);
        let req = make_req("gpt-4o", input);
        let out = responses_to_chat_completion(&req);

        assert_eq!(out.messages.len(), 3);

        // First message: user
        assert_eq!(out.messages[0].role, "user");

        // Second message: assistant with tool_calls
        assert_eq!(out.messages[1].role, "assistant");
        assert!(out.messages[1].tool_calls.is_some());
        let tool_calls = out.messages[1].tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_abc123");
        assert_eq!(tool_calls[0].tool_type, "function");
        assert_eq!(tool_calls[0].function.name, "get_weather");
        assert_eq!(tool_calls[0].function.arguments, "{\"city\": \"SF\"}");

        // Third message: tool result
        assert_eq!(out.messages[2].role, "tool");
        assert_eq!(
            out.messages[2].tool_call_id,
            Some("call_abc123".to_string())
        );
        assert_eq!(out.messages[2].content, Some(json!("Sunny, 72°F")));
    }

    #[test]
    fn test_max_output_tokens_to_max_tokens() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hi".to_string()));
        req.max_output_tokens = Some(512);
        let out = responses_to_chat_completion(&req);
        assert_eq!(out.max_tokens, Some(512));
    }

    #[test]
    fn test_tools_flat_to_nested() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hi".to_string()));
        req.tools = Some(vec![ResponsesApiTool {
            tool_type: "function".to_string(),
            name: Some("get_weather".to_string()),
            description: Some("Get weather".to_string()),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                }
            })),
            strict: None,
            extra: HashMap::new(),
        }]);
        let out = responses_to_chat_completion(&req);
        assert!(out.tools.is_some());
        let tools = out.tools.as_ref().unwrap();
        assert_eq!(tools.len(), 1);
        match &tools[0] {
            Tool::Function(ft) => {
                assert_eq!(ft.tool_type, "function");
                assert_eq!(ft.function.name, "get_weather");
                assert_eq!(ft.function.description, Some("Get weather".to_string()));
                assert!(ft.function.parameters.is_some());
            }
            Tool::ServerSide(_) => panic!("Expected Function tool"),
        }
    }

    #[test]
    fn test_tool_choice_passthrough() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hi".to_string()));
        req.tool_choice = Some(json!("auto"));
        let out = responses_to_chat_completion(&req);
        assert_eq!(out.tool_choice, Some(json!("auto")));
    }

    #[test]
    fn test_tool_choice_dict_passthrough() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hi".to_string()));
        req.tool_choice = Some(json!({"type": "function", "function": {"name": "get_weather"}}));
        let out = responses_to_chat_completion(&req);
        assert_eq!(
            out.tool_choice,
            Some(json!({"type": "function", "function": {"name": "get_weather"}}))
        );
    }

    #[test]
    fn test_temperature_top_p_passthrough() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hi".to_string()));
        req.temperature = Some(0.7);
        req.top_p = Some(0.9);
        let out = responses_to_chat_completion(&req);
        // f64→f32 cast at the boundary; check within f32 epsilon.
        let temp = out.temperature.unwrap();
        let top_p = out.top_p.unwrap();
        assert!((temp - 0.7f32).abs() < f32::EPSILON);
        assert!((top_p - 0.9f32).abs() < f32::EPSILON);
    }

    #[test]
    fn test_stream_true_sets_stream_options() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hi".to_string()));
        req.stream = Some(true);
        let out = responses_to_chat_completion(&req);
        assert!(out.stream);
        assert!(out.stream_options.is_some());
        let opts = out.stream_options.as_ref().unwrap();
        assert_eq!(opts.get("include_usage"), Some(&json!(true)));
    }

    #[test]
    fn test_stream_false_no_stream_options() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hi".to_string()));
        req.stream = Some(false);
        let out = responses_to_chat_completion(&req);
        assert!(!out.stream);
        assert!(out.stream_options.is_none());
    }

    #[test]
    fn test_parallel_tool_calls_passthrough() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hi".to_string()));
        req.parallel_tool_calls = Some(false);
        let out = responses_to_chat_completion(&req);
        assert_eq!(out.parallel_tool_calls, Some(false));
    }

    #[test]
    fn test_text_format_json_schema_to_response_format() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hi".to_string()));
        req.text = Some(json!({
            "format": {
                "type": "json_schema",
                "name": "my_schema",
                "schema": {"type": "object", "properties": {"x": {"type": "integer"}}},
                "strict": true
            }
        }));
        let out = responses_to_chat_completion(&req);
        assert!(out.response_format.is_some());
        let rf = out.response_format.as_ref().unwrap();
        assert_eq!(rf["type"], "json_schema");
        assert_eq!(rf["json_schema"]["name"], "my_schema");
        assert_eq!(rf["json_schema"]["strict"], true);
    }

    #[test]
    fn test_text_format_json_object_to_response_format() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hi".to_string()));
        req.text = Some(json!({
            "format": {
                "type": "json_object"
            }
        }));
        let out = responses_to_chat_completion(&req);
        assert!(out.response_format.is_some());
        let rf = out.response_format.as_ref().unwrap();
        assert_eq!(rf["type"], "json_object");
    }

    #[test]
    fn test_reasoning_effort_mapped() {
        let mut req = make_req("o3", ResponsesApiInput::Text("Hi".to_string()));
        req.reasoning = Some(json!({"effort": "high"}));
        let out = responses_to_chat_completion(&req);
        assert_eq!(out.reasoning_effort, Some("high".to_string()));
    }

    #[test]
    fn test_user_passthrough() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Text("Hi".to_string()));
        req.user = Some("user-123".to_string());
        let out = responses_to_chat_completion(&req);
        assert_eq!(out.user, Some("user-123".to_string()));
    }

    // ---- Response conversion tests ----

    #[test]
    fn test_chat_completion_to_responses_text_only() {
        let chat_resp = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello, world!"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });

        let input = ResponsesApiInput::Text("Hi".to_string());
        let original = make_req("gpt-4o", input.clone());

        let result = chat_completion_to_responses_api(&chat_resp, input, &original);

        assert_eq!(result["id"], "resp_chatcmpl-123");
        assert_eq!(result["object"], "response");
        assert_eq!(result["created_at"], 1700000000);
        assert_eq!(result["status"], "completed");
        assert_eq!(result["model"], "gpt-4o");

        // Output should have one message item.
        let output = result["output"].as_array().unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0]["type"], "message");
        assert_eq!(output[0]["id"], "msg_chatcmpl-123");
        assert_eq!(output[0]["role"], "assistant");
        let content = output[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "output_text");
        assert_eq!(content[0]["text"], "Hello, world!");

        // Usage.
        assert_eq!(result["usage"]["input_tokens"], 10);
        assert_eq!(result["usage"]["output_tokens"], 5);
        assert_eq!(result["usage"]["total_tokens"], 15);
    }

    #[test]
    fn test_chat_completion_to_responses_with_tool_calls() {
        let chat_resp = json!({
            "id": "chatcmpl-456",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_xyz",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"city\": \"SF\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 20,
                "completion_tokens": 10,
                "total_tokens": 30
            }
        });

        let input = ResponsesApiInput::Text("What's the weather?".to_string());
        let original = make_req("gpt-4o", input.clone());

        let result = chat_completion_to_responses_api(&chat_resp, input, &original);

        assert_eq!(result["status"], "completed");

        let output = result["output"].as_array().unwrap();
        // Should have: message + function_call
        assert_eq!(output.len(), 2);

        // First: message with empty text.
        assert_eq!(output[0]["type"], "message");

        // Second: function_call.
        assert_eq!(output[1]["type"], "function_call");
        assert_eq!(output[1]["id"], "fc_call_xyz");
        assert_eq!(output[1]["call_id"], "call_xyz");
        assert_eq!(output[1]["name"], "get_weather");
        assert_eq!(output[1]["arguments"], "{\"city\": \"SF\"}");
        assert_eq!(output[1]["status"], "completed");
    }

    #[test]
    fn test_chat_completion_to_responses_length_is_incomplete() {
        let chat_resp = json!({
            "id": "chatcmpl-789",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Partial..."
                },
                "finish_reason": "length"
            }],
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 100,
                "total_tokens": 105
            }
        });

        let input = ResponsesApiInput::Text("Go on".to_string());
        let original = make_req("gpt-4o", input.clone());

        let result = chat_completion_to_responses_api(&chat_resp, input, &original);
        assert_eq!(result["status"], "incomplete");
    }

    #[test]
    fn test_chat_completion_to_responses_echoes_request_params() {
        let chat_resp = json!({
            "id": "chatcmpl-echo",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Ok"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
        });

        let input = ResponsesApiInput::Text("Hi".to_string());
        let mut original = make_req("gpt-4o", input.clone());
        original.temperature = Some(0.5);
        original.top_p = Some(0.9);
        original.parallel_tool_calls = Some(true);
        original.user = Some("user-42".to_string());
        original.instructions = Some("Be helpful".to_string());

        let result = chat_completion_to_responses_api(&chat_resp, input, &original);

        // f64 preserves full precision for typical temperature/top_p values.
        let temp = result["temperature"].as_f64().unwrap();
        let top_p = result["top_p"].as_f64().unwrap();
        assert!((temp - 0.5).abs() < f64::EPSILON);
        assert!((top_p - 0.9).abs() < f64::EPSILON);
        assert_eq!(result["parallel_tool_calls"], true);
        assert_eq!(result["user"], "user-42");
        assert_eq!(result["instructions"], "Be helpful");
    }

    // ---- Streaming chunk tests ----

    #[test]
    fn test_stream_chunk_role_emits_item_and_content_added() {
        let chunk = json!({
            "id": "chatcmpl-stream1",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant"},
                "finish_reason": null
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let events = transformer.transform_chunk(&chunk);

        // Should emit: response.created, response.in_progress, output_item.added, content_part.added
        assert_eq!(events.len(), 4);
        assert!(events[0].contains("response.created"));
        assert!(events[1].contains("response.in_progress"));
        assert!(events[2].contains("response.output_item.added"));
        assert!(events[3].contains("response.content_part.added"));
    }

    #[test]
    fn test_stream_chunk_content_emits_text_delta() {
        let role_chunk = json!({
            "id": "chatcmpl-stream2",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant"},
                "finish_reason": null
            }]
        });
        let content_chunk = json!({
            "id": "chatcmpl-stream2",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"content": "Hello"},
                "finish_reason": null
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let _ = transformer.transform_chunk(&role_chunk);
        let events = transformer.transform_chunk(&content_chunk);

        // Content chunk after role should just emit text delta (initial events already done).
        assert_eq!(events.len(), 1);
        assert!(events[0].contains("response.output_text.delta"));
        assert!(events[0].contains("Hello"));
    }

    #[test]
    fn test_stream_chunk_tool_call_delta() {
        let role_chunk = json!({
            "id": "chatcmpl-stream3",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant"},
                "finish_reason": null
            }]
        });
        let tc_chunk = json!({
            "id": "chatcmpl-stream3",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_abc",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": ""}
                    }]
                },
                "finish_reason": null
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let _ = transformer.transform_chunk(&role_chunk);
        let events = transformer.transform_chunk(&tc_chunk);

        // Should emit output_item.added for the function call.
        assert!(!events.is_empty());
        assert!(events[0].contains("response.output_item.added"));
        assert!(events[0].contains("function_call"));
    }

    #[test]
    fn test_stream_chunk_tool_call_arguments_delta() {
        let role_chunk = json!({
            "id": "chatcmpl-stream4",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant"},
                "finish_reason": null
            }]
        });
        let tc_start_chunk = json!({
            "id": "chatcmpl-stream4",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_abc",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": ""}
                    }]
                },
                "finish_reason": null
            }]
        });
        let args_chunk = json!({
            "id": "chatcmpl-stream4",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": {"arguments": "{\"city\":"}
                    }]
                },
                "finish_reason": null
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let _ = transformer.transform_chunk(&role_chunk);
        let _ = transformer.transform_chunk(&tc_start_chunk);
        let events = transformer.transform_chunk(&args_chunk);

        assert_eq!(events.len(), 1);
        assert!(events[0].contains("response.function_call_arguments.delta"));
        // The arguments delta is JSON-escaped inside the SSE data field,
        // so check for the key token rather than the exact JSON.
        assert!(events[0].contains("city"));
    }

    #[test]
    fn test_stream_chunk_finish_reason_emits_completed() {
        let role_chunk = json!({
            "id": "chatcmpl-stream5",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant"},
                "finish_reason": null
            }]
        });
        let finish_chunk = json!({
            "id": "chatcmpl-stream5",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let _ = transformer.transform_chunk(&role_chunk);
        let events = transformer.transform_chunk(&finish_chunk);

        // Should emit content_part.done, output_item.done, and response.completed.
        assert!(events.len() >= 3);

        // Find the completed event.
        let completed_event = events.iter().find(|e| e.contains("response.completed"));
        assert!(completed_event.is_some());
        let completed = completed_event.unwrap();
        assert!(completed.contains("completed"));
    }

    #[test]
    fn test_stream_chunk_finish_reason_length_is_incomplete() {
        let role_chunk = json!({
            "id": "chatcmpl-stream6",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant"},
                "finish_reason": null
            }]
        });
        let finish_chunk = json!({
            "id": "chatcmpl-stream6",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "length"
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let _ = transformer.transform_chunk(&role_chunk);
        let events = transformer.transform_chunk(&finish_chunk);

        let completed_event = events.iter().find(|e| e.contains("response.completed"));
        assert!(completed_event.is_some());
        assert!(completed_event.unwrap().contains("incomplete"));
    }

    #[test]
    fn test_stream_chunk_empty_content_no_event() {
        let role_chunk = json!({
            "id": "chatcmpl-stream7",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant"},
                "finish_reason": null
            }]
        });
        let content_chunk = json!({
            "id": "chatcmpl-stream7",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"content": ""},
                "finish_reason": null
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let _ = transformer.transform_chunk(&role_chunk);
        let events = transformer.transform_chunk(&content_chunk);

        // Empty content should not emit a text delta event.
        assert!(events.is_empty());
    }

    #[test]
    fn test_sse_event_format() {
        let event = format_sse_event(
            "response.output_text.delta",
            &json!({
                "type": "response.output_text.delta",
                "delta": "Hello"
            }),
        );

        assert!(event.starts_with("event: response.output_text.delta\n"));
        assert!(event.contains("data: "));
        assert!(event.ends_with("\n\n"));
    }

    // ---- End-to-end conversion trace tests ----

    /// Trace the full non-streaming conversion for a simple Codex CLI request.
    /// Codex sends POST /v1/responses with a text prompt →
    /// Gateway converts to /v1/chat/completions →
    /// Huawei MaaS responds with chat completion →
    /// Gateway converts back to Responses API format.
    #[test]
    fn test_e2e_simple_text_request() {
        // 1. Codex CLI request
        let input = ResponsesApiInput::Text("Tell me a bedtime story".to_string());
        let mut req = make_req("deepseek-v3", input.clone());
        req.instructions = Some("You are a helpful assistant.".to_string());
        req.temperature = Some(0.7);
        req.max_output_tokens = Some(4096);
        req.stream = Some(false);

        // 2. Convert to Chat Completion request
        let chat_req = responses_to_chat_completion(&req);
        assert_eq!(chat_req.model, "deepseek-v3");
        // Should have: system message (from instructions) + user message
        assert_eq!(chat_req.messages.len(), 2);
        assert_eq!(chat_req.messages[0].role, "system");
        assert_eq!(chat_req.messages[1].role, "user");
        assert_eq!(chat_req.max_tokens, Some(4096));
        assert!(!chat_req.stream);

        // 3. Simulate Huawei MaaS response (chat completion format)
        let maas_resp = json!({
            "id": "chatcmpl-maas-123",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "deepseek-v3",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Once upon a time, there was a unicorn..."
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 15,
                "completion_tokens": 20,
                "total_tokens": 35,
                "prompt_tokens_details": {"cached_tokens": 5},
                "completion_tokens_details": {"reasoning_tokens": 0}
            }
        });

        // 4. Convert back to Responses API format
        let resp = chat_completion_to_responses_api(&maas_resp, input, &req);
        assert_eq!(resp["id"], "resp_chatcmpl-maas-123");
        assert_eq!(resp["object"], "response");
        assert_eq!(resp["status"], "completed");
        assert_eq!(resp["model"], "deepseek-v3");

        // Output should have a single message item
        let output = resp["output"].as_array().unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0]["type"], "message");
        assert_eq!(output[0]["role"], "assistant");
        let content = output[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "output_text");
        assert_eq!(
            content[0]["text"],
            "Once upon a time, there was a unicorn..."
        );

        // Usage should have token details
        assert_eq!(resp["usage"]["input_tokens"], 15);
        assert_eq!(resp["usage"]["output_tokens"], 20);
        assert_eq!(resp["usage"]["input_tokens_details"]["cached_tokens"], 5);
        assert_eq!(
            resp["usage"]["output_tokens_details"]["reasoning_tokens"],
            0
        );
    }

    /// Trace the full conversion for a request with function calling.
    /// Codex CLI sends a request with tools → model responds with tool_call →
    /// Gateway converts both directions correctly.
    #[test]
    fn test_e2e_function_calling_round_trip() {
        // 1. Codex CLI request with tools
        let input = ResponsesApiInput::Items(vec![
            json!({"role": "user", "content": "Read the file main.rs"}),
        ]);
        let mut req = make_req("deepseek-v3", input.clone());
        req.tools = Some(vec![ResponsesApiTool {
            tool_type: "function".to_string(),
            name: Some("read_file".to_string()),
            description: Some("Read a file from disk".to_string()),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            })),
            strict: None,
            extra: HashMap::new(),
        }]);

        // 2. Convert to Chat Completion request
        let chat_req = responses_to_chat_completion(&req);
        assert!(chat_req.tools.is_some());
        let tools = chat_req.tools.as_ref().unwrap();
        assert_eq!(tools.len(), 1);
        // Tool should be in nested format
        match &tools[0] {
            Tool::Function(ft) => {
                assert_eq!(ft.function.name, "read_file");
            }
            Tool::ServerSide(_) => panic!("Expected Function tool"),
        }

        // 3. Simulate MaaS response with tool_call
        let maas_resp = json!({
            "id": "chatcmpl-tool-1",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "deepseek-v3",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\": \"main.rs\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 30, "completion_tokens": 10, "total_tokens": 40}
        });

        // 4. Convert back to Responses API format
        let resp = chat_completion_to_responses_api(&maas_resp, input, &req);
        assert_eq!(resp["status"], "completed");

        let output = resp["output"].as_array().unwrap();
        // message (empty text) + function_call
        assert_eq!(output.len(), 2);
        assert_eq!(output[0]["type"], "message");
        assert_eq!(output[1]["type"], "function_call");
        assert_eq!(output[1]["call_id"], "call_abc");
        assert_eq!(output[1]["name"], "read_file");
        assert_eq!(output[1]["arguments"], "{\"path\": \"main.rs\"}");
    }

    /// Test the multi-turn conversation flow where Codex CLI sends
    /// function_call + function_call_output from previous turns.
    #[test]
    fn test_e2e_multi_turn_with_tool_results() {
        // Codex CLI sends a follow-up with the previous tool result
        let input = ResponsesApiInput::Items(vec![
            json!({"role": "user", "content": "Read main.rs"}),
            json!({
                "type": "function_call",
                "name": "read_file",
                "arguments": "{\"path\": \"main.rs\"}",
                "call_id": "call_001",
                "id": "fc_001"
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_001",
                "output": "fn main() { println!(\"hello\"); }"
            }),
            json!({"role": "user", "content": "What does this code do?"}),
        ]);
        let req = make_req("deepseek-v3", input);

        let chat_req = responses_to_chat_completion(&req);

        // Should produce: user, assistant(tool_call), tool(result), user
        assert_eq!(chat_req.messages.len(), 4);
        assert_eq!(chat_req.messages[0].role, "user");
        assert_eq!(chat_req.messages[1].role, "assistant");
        assert!(chat_req.messages[1].tool_calls.is_some());
        assert_eq!(chat_req.messages[2].role, "tool");
        assert_eq!(
            chat_req.messages[2].tool_call_id,
            Some("call_001".to_string())
        );
        assert_eq!(chat_req.messages[3].role, "user");
    }

    /// Test reasoning content mapping from MaaS response to Responses API output.
    #[test]
    fn test_e2e_reasoning_content_in_response() {
        let chat_resp = json!({
            "id": "chatcmpl-reasoning",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "deepseek-r1",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "The answer is 42.",
                    "reasoning_content": "Let me think step by step... 6*7=42"
                },
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 50, "total_tokens": 60}
        });

        let input = ResponsesApiInput::Text("What is 6*7?".to_string());
        let req = make_req("deepseek-r1", input.clone());

        let resp = chat_completion_to_responses_api(&chat_resp, input, &req);

        let output = resp["output"].as_array().unwrap();
        // Should have: reasoning item + message item
        assert_eq!(output.len(), 2);
        assert_eq!(output[0]["type"], "reasoning");
        let reasoning_content = output[0]["content"].as_array().unwrap();
        assert_eq!(
            reasoning_content[0]["text"],
            "Let me think step by step... 6*7=42"
        );
        assert_eq!(output[1]["type"], "message");
        let msg_content = output[1]["content"].as_array().unwrap();
        assert_eq!(msg_content[0]["text"], "The answer is 42.");
    }

    // ---- Stateful streaming transformer tests ----

    /// Helper: extract the event type from an SSE event string.
    fn event_type(sse: &str) -> &str {
        // Format: "event: <type>\ndata: ...\n\n"
        sse.strip_prefix("event: ")
            .and_then(|s| s.split('\n').next())
            .unwrap_or("")
    }

    /// Helper: extract the JSON data payload from an SSE event string.
    fn event_data(sse: &str) -> Value {
        let data_line = sse
            .lines()
            .find(|l| l.starts_with("data: "))
            .unwrap_or("data: {}");
        let json_str = data_line.strip_prefix("data: ").unwrap_or("{}");
        serde_json::from_str(json_str).unwrap_or_else(|_| json!({}))
    }

    #[test]
    fn test_initial_events_emitted_once() {
        let role_chunk = json!({
            "id": "chatcmpl-init",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant"},
                "finish_reason": null
            }]
        });
        let content_chunk = json!({
            "id": "chatcmpl-init",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"content": "Hi"},
                "finish_reason": null
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let events1 = transformer.transform_chunk(&role_chunk);
        let events2 = transformer.transform_chunk(&content_chunk);

        // First chunk should emit initial events + message item start.
        assert!(events1.iter().any(|e| event_type(e) == "response.created"));
        assert!(events1
            .iter()
            .any(|e| event_type(e) == "response.in_progress"));

        // Second chunk should NOT re-emit initial events.
        assert!(!events2.iter().any(|e| event_type(e) == "response.created"));
        assert!(!events2
            .iter()
            .any(|e| event_type(e) == "response.in_progress"));
    }

    #[test]
    fn test_reasoning_content_with_proper_output_index() {
        let reasoning_chunk = json!({
            "id": "chatcmpl-reasoning",
            "created": 1700000000,
            "model": "deepseek-r1",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant", "reasoning_content": "Let me think..."},
                "finish_reason": null
            }]
        });
        let content_chunk = json!({
            "id": "chatcmpl-reasoning",
            "created": 1700000000,
            "model": "deepseek-r1",
            "choices": [{
                "index": 0,
                "delta": {"content": "The answer is 42."},
                "finish_reason": null
            }]
        });
        let finish_chunk = json!({
            "id": "chatcmpl-reasoning",
            "created": 1700000000,
            "model": "deepseek-r1",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "deepseek-r1");
        let events1 = transformer.transform_chunk(&reasoning_chunk);
        let events2 = transformer.transform_chunk(&content_chunk);
        let events3 = transformer.transform_chunk(&finish_chunk);

        // Reasoning chunk: initial events + reasoning item added + content part added + text delta
        let reasoning_added = events1
            .iter()
            .find(|e| event_type(e) == "response.output_item.added")
            .unwrap();
        let data = event_data(reasoning_added);
        assert_eq!(data["item"]["type"], "reasoning");
        assert_eq!(data["output_index"], 0); // No more -1 hack!

        // Reasoning text delta uses output_index 0
        let reasoning_delta = events1
            .iter()
            .find(|e| event_type(e) == "response.output_text.delta")
            .unwrap();
        let delta_data = event_data(reasoning_delta);
        assert_eq!(delta_data["output_index"], 0);

        // Content chunk: closes reasoning, starts message at output_index 1, emits text delta
        let msg_added = events2
            .iter()
            .find(|e| event_type(e) == "response.output_item.added")
            .unwrap();
        let msg_data = event_data(msg_added);
        assert_eq!(msg_data["item"]["type"], "message");
        assert_eq!(msg_data["output_index"], 1);

        // Content text delta uses output_index 1
        let content_delta = events2
            .iter()
            .find(|e| event_type(e) == "response.output_text.delta")
            .unwrap();
        let cdata = event_data(content_delta);
        assert_eq!(cdata["output_index"], 1);

        // Finish chunk: response.completed has populated output array
        let completed = events3
            .iter()
            .find(|e| event_type(e) == "response.completed")
            .unwrap();
        let completed_data = event_data(completed);
        let output = completed_data["response"]["output"].as_array().unwrap();
        assert_eq!(output.len(), 2);
        assert_eq!(output[0]["type"], "reasoning");
        assert_eq!(output[1]["type"], "message");
    }

    #[test]
    fn test_function_call_lifecycle() {
        let role_chunk = json!({
            "id": "chatcmpl-fc",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant"},
                "finish_reason": null
            }]
        });
        let tc_start = json!({
            "id": "chatcmpl-fc",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_abc",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": ""}
                    }]
                },
                "finish_reason": null
            }]
        });
        let tc_args1 = json!({
            "id": "chatcmpl-fc",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": {"arguments": "{\"city\":"}
                    }]
                },
                "finish_reason": null
            }]
        });
        let tc_args2 = json!({
            "id": "chatcmpl-fc",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": {"arguments": " \"SF\"}"}
                    }]
                },
                "finish_reason": null
            }]
        });
        let finish_chunk = json!({
            "id": "chatcmpl-fc",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "tool_calls"
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let _ = transformer.transform_chunk(&role_chunk);
        let e2 = transformer.transform_chunk(&tc_start);
        let _ = transformer.transform_chunk(&tc_args1);
        let _ = transformer.transform_chunk(&tc_args2);
        let e5 = transformer.transform_chunk(&finish_chunk);

        // tc_start: output_item.added for function_call
        let fc_added = e2
            .iter()
            .find(|e| event_type(e) == "response.output_item.added")
            .unwrap();
        let fc_data = event_data(fc_added);
        assert_eq!(fc_data["item"]["type"], "function_call");
        assert_eq!(fc_data["item"]["call_id"], "call_abc");
        assert_eq!(fc_data["item"]["name"], "get_weather");

        // finish_chunk: output_item.done for function_call with accumulated arguments
        let fc_done = e5
            .iter()
            .find(|e| {
                event_type(e) == "response.output_item.done"
                    && event_data(e)["item"]["type"] == "function_call"
            })
            .unwrap();
        let fc_done_data = event_data(fc_done);
        assert_eq!(fc_done_data["item"]["arguments"], "{\"city\": \"SF\"}");

        // response.completed output array has message + function_call
        let completed = e5
            .iter()
            .find(|e| event_type(e) == "response.completed")
            .unwrap();
        let completed_data = event_data(completed);
        let output = completed_data["response"]["output"].as_array().unwrap();
        assert_eq!(output.len(), 2);
        assert_eq!(output[0]["type"], "message");
        assert_eq!(output[1]["type"], "function_call");
        assert_eq!(output[1]["arguments"], "{\"city\": \"SF\"}");
    }

    #[test]
    fn test_accumulated_text_in_content_part_done() {
        let role_chunk = json!({
            "id": "chatcmpl-accum",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant"},
                "finish_reason": null
            }]
        });
        let content1 = json!({
            "id": "chatcmpl-accum",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"content": "Hello "},
                "finish_reason": null
            }]
        });
        let content2 = json!({
            "id": "chatcmpl-accum",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"content": "world!"},
                "finish_reason": null
            }]
        });
        let finish_chunk = json!({
            "id": "chatcmpl-accum",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let _ = transformer.transform_chunk(&role_chunk);
        let _ = transformer.transform_chunk(&content1);
        let _ = transformer.transform_chunk(&content2);
        let events = transformer.transform_chunk(&finish_chunk);

        // content_part.done should have accumulated text
        let cp_done = events
            .iter()
            .find(|e| event_type(e) == "response.content_part.done")
            .unwrap();
        let cp_data = event_data(cp_done);
        assert_eq!(cp_data["part"]["text"], "Hello world!");

        // output_item.done should have accumulated text
        let oi_done = events
            .iter()
            .find(|e| event_type(e) == "response.output_item.done")
            .unwrap();
        let oi_data = event_data(oi_done);
        assert_eq!(oi_data["item"]["content"][0]["text"], "Hello world!");

        // response.completed output array should have accumulated text
        let completed = events
            .iter()
            .find(|e| event_type(e) == "response.completed")
            .unwrap();
        let completed_data = event_data(completed);
        let output = completed_data["response"]["output"].as_array().unwrap();
        assert_eq!(output[0]["content"][0]["text"], "Hello world!");
    }

    #[test]
    fn test_content_filter_maps_to_incomplete() {
        let role_chunk = json!({
            "id": "chatcmpl-cf",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant"},
                "finish_reason": null
            }]
        });
        let finish_chunk = json!({
            "id": "chatcmpl-cf",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "content_filter"
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let _ = transformer.transform_chunk(&role_chunk);
        let events = transformer.transform_chunk(&finish_chunk);

        let completed = events
            .iter()
            .find(|e| event_type(e) == "response.completed")
            .unwrap();
        assert!(completed.contains("incomplete"));

        // output_item.done should also have incomplete status
        let oi_done = events
            .iter()
            .find(|e| event_type(e) == "response.output_item.done")
            .unwrap();
        let oi_data = event_data(oi_done);
        assert_eq!(oi_data["item"]["status"], "incomplete");
    }

    #[test]
    fn test_full_text_stream_event_sequence() {
        let role_chunk = json!({
            "id": "chatcmpl-seq",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant"},
                "finish_reason": null
            }]
        });
        let content_chunk = json!({
            "id": "chatcmpl-seq",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {"content": "Hi there"},
                "finish_reason": null
            }]
        });
        let finish_chunk = json!({
            "id": "chatcmpl-seq",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let e1 = transformer.transform_chunk(&role_chunk);
        let e2 = transformer.transform_chunk(&content_chunk);
        let e3 = transformer.transform_chunk(&finish_chunk);

        let all_events: Vec<&str> = [&e1, &e2, &e3]
            .iter()
            .flat_map(|evs| evs.iter().map(|e| event_type(e)))
            .collect();

        // Verify the full event sequence for a simple text response:
        let expected = vec![
            "response.created",
            "response.in_progress",
            "response.output_item.added",
            "response.content_part.added",
            "response.output_text.delta",
            "response.content_part.done",
            "response.output_item.done",
            "response.completed",
        ];
        assert_eq!(all_events, expected);
    }

    #[test]
    fn test_full_reasoning_stream_event_sequence() {
        let reasoning_chunk = json!({
            "id": "chatcmpl-rseq",
            "created": 1700000000,
            "model": "deepseek-r1",
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant", "reasoning_content": "Thinking..."},
                "finish_reason": null
            }]
        });
        let content_chunk = json!({
            "id": "chatcmpl-rseq",
            "created": 1700000000,
            "model": "deepseek-r1",
            "choices": [{
                "index": 0,
                "delta": {"content": "Answer."},
                "finish_reason": null
            }]
        });
        let finish_chunk = json!({
            "id": "chatcmpl-rseq",
            "created": 1700000000,
            "model": "deepseek-r1",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });

        let mut transformer = ResponsesStreamTransformer::new("resp_0", "deepseek-r1");
        let e1 = transformer.transform_chunk(&reasoning_chunk);
        let e2 = transformer.transform_chunk(&content_chunk);
        let e3 = transformer.transform_chunk(&finish_chunk);

        let all_events: Vec<&str> = [&e1, &e2, &e3]
            .iter()
            .flat_map(|evs| evs.iter().map(|e| event_type(e)))
            .collect();

        // Verify the full event sequence for a reasoning response:
        let expected = vec![
            "response.created",
            "response.in_progress",
            "response.output_item.added",  // reasoning (output_index: 0)
            "response.content_part.added", // reasoning
            "response.output_text.delta",  // reasoning delta
            "response.content_part.done",  // close reasoning
            "response.output_item.done",   // close reasoning
            "response.output_item.added",  // message (output_index: 1)
            "response.content_part.added", // message
            "response.output_text.delta",  // message delta
            "response.content_part.done",  // close message
            "response.output_item.done",   // close message
            "response.completed",
        ];
        assert_eq!(all_events, expected);
    }

    // ---- Edge-case tests (M5) ----

    // ---- Content part conversion tests (multimodal) ----

    #[test]
    fn test_input_text_converted_to_text() {
        let input = ResponsesApiInput::Items(vec![json!({
            "role": "user",
            "content": [
                {"type": "input_text", "text": "Describe this"},
                {"type": "input_image", "image_url": "data:image/png;base64,iVBOR"}
            ]
        })]);
        let req = make_req("gpt-4o", input);
        let out = responses_to_chat_completion(&req);
        assert_eq!(out.messages.len(), 1);
        let content = out.messages[0].content.as_ref().unwrap();
        let parts = content.as_array().unwrap();
        assert_eq!(parts.len(), 2);
        // input_text → text
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[0]["text"], "Describe this");
        // input_image → image_url
        assert_eq!(parts[1]["type"], "image_url");
        assert_eq!(parts[1]["image_url"]["url"], "data:image/png;base64,iVBOR");
    }

    #[test]
    fn test_input_image_with_detail() {
        let input = ResponsesApiInput::Items(vec![json!({
            "role": "user",
            "content": [
                {"type": "input_image", "image_url": "https://example.com/img.png", "detail": "high"}
            ]
        })]);
        let req = make_req("gpt-4o", input);
        let out = responses_to_chat_completion(&req);
        let content = out.messages[0].content.as_ref().unwrap();
        let parts = content.as_array().unwrap();
        assert_eq!(parts[0]["type"], "image_url");
        assert_eq!(parts[0]["image_url"]["url"], "https://example.com/img.png");
        assert_eq!(parts[0]["image_url"]["detail"], "high");
    }

    #[test]
    fn test_string_content_passthrough() {
        // String content should pass through unchanged (not an array of parts).
        let input = ResponsesApiInput::Items(vec![json!({
            "role": "user",
            "content": "Just a plain string"
        })]);
        let req = make_req("gpt-4o", input);
        let out = responses_to_chat_completion(&req);
        assert_eq!(out.messages[0].content, Some(json!("Just a plain string")));
    }

    #[test]
    fn test_already_chat_completions_format_passthrough() {
        // Content parts already in Chat Completions format should pass through.
        let input = ResponsesApiInput::Items(vec![json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "Hello"},
                {"type": "image_url", "image_url": {"url": "https://example.com/img.png"}}
            ]
        })]);
        let req = make_req("gpt-4o", input);
        let out = responses_to_chat_completion(&req);
        let content = out.messages[0].content.as_ref().unwrap();
        let parts = content.as_array().unwrap();
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[1]["type"], "image_url");
    }

    /// 1. Empty input array `{"input": []}` — should produce zero user messages
    ///    (only instructions if present).
    #[test]
    fn test_empty_input_array() {
        let req = make_req("gpt-4o", ResponsesApiInput::Items(vec![]));
        let chat_req = responses_to_chat_completion(&req);
        // No instructions, no input items → no messages at all.
        assert!(chat_req.messages.is_empty());
    }

    /// Empty input array with instructions → only the system message.
    #[test]
    fn test_empty_input_array_with_instructions() {
        let mut req = make_req("gpt-4o", ResponsesApiInput::Items(vec![]));
        req.instructions = Some("You are helpful.".to_string());
        let chat_req = responses_to_chat_completion(&req);
        assert_eq!(chat_req.messages.len(), 1);
        assert_eq!(chat_req.messages[0].role, "system");
    }

    /// 2. Chat completion response with `null` content and no tool_calls —
    ///    should produce empty text message.
    #[test]
    fn test_chat_completion_null_content_no_tool_calls() {
        let chat_resp = json!({
            "id": "chatcmpl-1",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null
                },
                "finish_reason": "stop"
            }]
        });
        let input = ResponsesApiInput::Text("hello".to_string());
        let orig_req = make_req("gpt-4o", input.clone());
        let result = chat_completion_to_responses_api(&chat_resp, input, &orig_req);

        // Should have a message output item with empty text.
        let output = result["output"].as_array().unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0]["type"], "message");
        let text = output[0]["content"][0]["text"].as_str().unwrap();
        assert_eq!(text, "");
    }

    /// 3. Chat completion response with no `usage` field —
    ///    should produce response without usage.
    #[test]
    fn test_chat_completion_no_usage() {
        let chat_resp = json!({
            "id": "chatcmpl-1",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello!"
                },
                "finish_reason": "stop"
            }]
        });
        let input = ResponsesApiInput::Text("hi".to_string());
        let orig_req = make_req("gpt-4o", input.clone());
        let result = chat_completion_to_responses_api(&chat_resp, input, &orig_req);

        // Should not have a usage field.
        assert!(result.get("usage").is_none());
    }

    /// 4. Chat completion response with no `choices` field at all —
    ///    should handle gracefully (empty text, completed status).
    #[test]
    fn test_chat_completion_no_choices() {
        let chat_resp = json!({
            "id": "chatcmpl-1",
            "created": 1700000000,
            "model": "gpt-4o"
        });
        let input = ResponsesApiInput::Text("hi".to_string());
        let orig_req = make_req("gpt-4o", input.clone());
        let result = chat_completion_to_responses_api(&chat_resp, input, &orig_req);

        // Should still produce a valid response with a message output item.
        assert_eq!(result["id"].as_str().unwrap(), "resp_chatcmpl-1");
        let output = result["output"].as_array().unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0]["type"], "message");
        assert_eq!(output[0]["status"], "completed");
        // Content should be empty string (no content to extract).
        assert_eq!(output[0]["content"][0]["text"], "");
    }

    /// 5. Streaming chunk with no `choices` field — should produce only initial events.
    #[test]
    fn test_stream_chunk_no_choices() {
        let chunk = json!({
            "id": "chatcmpl-1",
            "created": 1700000000,
            "model": "gpt-4o"
        });
        let mut transformer = ResponsesStreamTransformer::new("resp_0", "gpt-4o");
        let events = transformer.transform_chunk(&chunk);

        // A chunk with no choices should produce no events at all
        // (initial events are only emitted when there's actual content to process).
        assert!(events.is_empty());
    }
}
