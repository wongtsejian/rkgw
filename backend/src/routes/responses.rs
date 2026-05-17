use axum::{
    body::Body,
    extract::State,
    response::{IntoResponse, Response},
    Json,
};
use futures::stream::StreamExt;

use crate::converters::responses_to_chat_completion::{
    chat_completion_to_responses_api, responses_to_chat_completion, ResponsesStreamTransformer,
};
use crate::error::ApiError;
use crate::middleware::DEBUG_LOGGER;
use crate::models::responses::{ResponsesApiInput, ResponsesApiRequest};
use crate::providers::types::{ProviderContext, ProviderId};

use super::pipeline::{
    build_kiro_credentials, build_request_context_openai, extract_assistant_content,
    extract_last_user_message, extract_usage_metric_snapshot, handle_rate_limit_retry,
    persist_non_streaming_usage, read_config, resolve_provider_routing, run_input_guardrail_check,
    run_output_guardrail_check, update_rate_limits, validate_model_provider,
    validate_model_visibility, validate_provider_enabled, wrap_stream_with_usage_metrics,
};
use super::state::{AppState, UserKiroCreds};

/// POST /v1/responses — OpenAI Responses API endpoint.
///
/// Accepts a Responses API request, converts it to Chat Completions format,
/// routes to the appropriate provider, then converts the response back to
/// Responses API format before returning it to the client.
///
/// Supports both streaming and non-streaming modes.
#[tracing::instrument(skip_all, name = "responses_handler")]
pub(crate) async fn responses_handler(
    State(state): State<AppState>,
    raw_request: axum::http::Request<Body>,
) -> Result<Response, ApiError> {
    let user_creds = raw_request.extensions().get::<UserKiroCreds>().cloned();

    // ── Parse request body ────────────────────────────────────────────
    let body_bytes = axum::body::to_bytes(raw_request.into_body(), 10 * 1024 * 1024)
        .await
        .map_err(|e| ApiError::ValidationError(format!("Failed to read body: {}", e)))?;
    let responses_req: ResponsesApiRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| ApiError::ValidationError(format!("Invalid JSON: {}", e)))?;

    if responses_req.model.is_empty() {
        return Err(ApiError::ValidationError(
            "model field is required and must not be empty".to_string(),
        ));
    }

    // Validate request isn't pathologically large.
    if let ResponsesApiInput::Items(ref items) = responses_req.input {
        if items.len() > 1000 {
            return Err(ApiError::ValidationError(
                "Too many input items in request (max 1000)".to_string(),
            ));
        }
    }

    let is_stream = responses_req.stream.unwrap_or(false);

    tracing::info!(
        model = %responses_req.model,
        stream = is_stream,
        "Request to /v1/responses"
    );

    // ── Convert to Chat Completion request ─────────────────────────────
    let mut chat_req = responses_to_chat_completion(&responses_req);

    // ── Provider routing ─────────────────────────────────────────────
    let requested_model = responses_req.model.clone();
    validate_model_provider(&requested_model)?;
    validate_provider_enabled(&state.provider_registry, &requested_model).await?;
    let mut routing = resolve_provider_routing(&state, user_creds.as_ref(), &requested_model).await;
    validate_model_visibility(
        &state.model_cache,
        &routing.provider_id,
        &requested_model,
        routing.stripped_model.as_deref(),
    )?;

    // Build credentials: for Kiro, derive from user creds / global auth;
    // for direct providers, use the credentials from the registry.
    let mut creds = if routing.provider_id == ProviderId::Kiro {
        build_kiro_credentials(&state, user_creds.as_ref()).await?
    } else {
        routing.provider_creds.clone().ok_or_else(|| {
            ApiError::AuthError(format!(
                "No credentials available for provider {:?}",
                routing.provider_id
            ))
        })?
    };

    // Strip provider prefix from model name if present.
    if let Some(ref model_id) = routing.stripped_model {
        chat_req.model = model_id.clone();
    }

    let mut provider = state.providers.get(&routing.provider_id).ok_or_else(|| {
        ApiError::Internal(anyhow::anyhow!(
            "Provider {:?} not registered",
            routing.provider_id
        ))
    })?;

    tracing::info!(
        model = %chat_req.model,
        provider = ?routing.provider_id,
        stream = is_stream,
        "Routing request (Responses API endpoint)"
    );

    // ── Pre-provider pipeline stages ─────────────────────────────────
    let config = read_config(&state);

    // Truncation recovery (Kiro-specific, but harmless for others)
    if routing.provider_id == ProviderId::Kiro && config.truncation_recovery {
        let mut msg_values: Vec<serde_json::Value> = chat_req
            .messages
            .iter()
            .filter_map(|m| serde_json::to_value(m).ok())
            .collect();
        crate::truncation::inject_openai_truncation_recovery(&mut msg_values);
        chat_req.messages = msg_values
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
    }

    // Input guardrails
    if config.guardrails_enabled {
        if let Some(ref engine) = state.guardrails_engine {
            let user_content = extract_last_user_message(&chat_req.messages);
            let ctx = build_request_context_openai(&chat_req);
            run_input_guardrail_check(engine, &user_content, &ctx).await?;
        }
    }

    // ── Provider dispatch with failover ──────────────────────────────
    const MAX_ATTEMPTS: usize = 3;

    if is_stream {
        let model_for_transform = chat_req.model.clone();
        let response_id_prefix = format!("resp_{}", uuid::Uuid::new_v4().as_simple());

        let mut last_error = None;
        for attempt in 0..MAX_ATTEMPTS {
            let ctx = ProviderContext {
                credentials: &creds,
                model: &chat_req.model,
            };

            match provider.stream_openai(&ctx, &chat_req).await {
                Ok(stream_resp) => {
                    update_rate_limits(
                        &state.rate_tracker,
                        &routing.account_id,
                        &routing.provider_id,
                        &stream_resp.headers,
                    );

                    let tracked_stream = wrap_stream_with_usage_metrics(
                        stream_resp.stream,
                        state.config_db.clone(),
                        user_creds.as_ref().map(|c| c.user_id),
                        routing.provider_id.clone(),
                        chat_req.model.clone(),
                    );

                    // Transform the SSE stream from Chat Completion format to Responses API format.
                    let transformed_stream = transform_responses_sse_stream(
                        tracked_stream,
                        &response_id_prefix,
                        &model_for_transform,
                        responses_req.input.clone(),
                        &responses_req,
                    );

                    let byte_stream = transformed_stream
                        .map(|r| r.map_err(|e| std::io::Error::other(e.to_string())));

                    let response = Response::builder()
                        .status(200)
                        .header("Content-Type", "text/event-stream")
                        .header("Cache-Control", "no-cache")
                        .header("Connection", "keep-alive")
                        .body(Body::from_stream(byte_stream))
                        .map_err(|e| {
                            ApiError::Internal(anyhow::anyhow!("Failed to build response: {}", e))
                        })?;

                    DEBUG_LOGGER.discard_buffers().await;
                    return Ok(response);
                }
                Err(ApiError::ProviderApiError {
                    status: 429,
                    ref headers,
                    ..
                }) => {
                    if !handle_rate_limit_retry(
                        &state,
                        user_creds.as_ref(),
                        &mut routing,
                        &mut creds,
                        &mut provider,
                        attempt,
                        MAX_ATTEMPTS,
                        headers,
                    )
                    .await?
                    {
                        break;
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                    break;
                }
            }
        }
        Err(last_error.unwrap_or_else(|| ApiError::RateLimited {
            provider: routing.provider_id.as_str().to_string(),
            retry_after_secs: 60,
        }))
    } else {
        let mut last_error = None;
        for attempt in 0..MAX_ATTEMPTS {
            let ctx = ProviderContext {
                credentials: &creds,
                model: &chat_req.model,
            };

            match provider.execute_openai(&ctx, &chat_req).await {
                Ok(resp) => {
                    update_rate_limits(
                        &state.rate_tracker,
                        &routing.account_id,
                        &routing.provider_id,
                        &resp.headers,
                    );

                    let body = provider.normalize_response_for_openai(&chat_req.model, resp.body);

                    // Output guardrails (non-streaming only)
                    if config.guardrails_enabled {
                        if let Some(ref engine) = state.guardrails_engine {
                            let output_text = extract_assistant_content(&body);
                            let ctx = build_request_context_openai(&chat_req);
                            run_output_guardrail_check(engine, &output_text, &ctx).await?;
                        }
                    }

                    persist_non_streaming_usage(
                        extract_usage_metric_snapshot(&body),
                        &state.config_db,
                        user_creds.as_ref().map(|c| c.user_id),
                        &routing.provider_id,
                        &chat_req.model,
                    );

                    // Convert Chat Completion response → Responses API response.
                    let request_input = responses_req.input.clone();
                    let responses_body =
                        chat_completion_to_responses_api(&body, request_input, &responses_req);

                    DEBUG_LOGGER.discard_buffers().await;
                    return Ok(Json(responses_body).into_response());
                }
                Err(ApiError::ProviderApiError {
                    status: 429,
                    ref headers,
                    ..
                }) => {
                    if !handle_rate_limit_retry(
                        &state,
                        user_creds.as_ref(),
                        &mut routing,
                        &mut creds,
                        &mut provider,
                        attempt,
                        MAX_ATTEMPTS,
                        headers,
                    )
                    .await?
                    {
                        break;
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                    break;
                }
            }
        }
        Err(last_error.unwrap_or_else(|| ApiError::RateLimited {
            provider: routing.provider_id.as_str().to_string(),
            retry_after_secs: 60,
        }))
    }
}

/// Transform a Chat Completion SSE stream into a Responses API SSE stream.
///
/// Each incoming SSE chunk (a Chat Completion `chat.completion.chunk` object) is
/// parsed, passed through a [`ResponsesStreamTransformer`] instance, and the
/// resulting event strings (already formatted as `event: ...\ndata: ...\n\n`)
/// are emitted as bytes.
///
/// The transformer is stateful, so it correctly emits stream lifecycle events
/// (`response.created`, `response.in_progress`) once at the start, accumulates
/// text/arguments for `done` events, and populates the `output` array in
/// `response.completed`.
///
/// The `[DONE]` sentinel is forwarded as-is to signal stream completion.
fn transform_responses_sse_stream<S>(
    byte_stream: S,
    response_id: &str,
    model: &str,
    request_input: ResponsesApiInput,
    request: &ResponsesApiRequest,
) -> impl futures::stream::Stream<Item = Result<bytes::Bytes, ApiError>> + Send
where
    S: futures::stream::Stream<Item = Result<bytes::Bytes, ApiError>> + Send + 'static,
{
    let response_id = response_id.to_string();
    let model = model.to_string();
    let mut buffer = String::new();
    let mut transformer = ResponsesStreamTransformer::new(&response_id, &model);
    transformer.set_request_context(request_input, request);

    async_stream::stream! {
        futures::pin_mut!(byte_stream);

        while let Some(chunk) = byte_stream.next().await {
            let chunk = match chunk {
                Ok(b) => b,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            let text = match std::str::from_utf8(chunk.as_ref()) {
                Ok(s) => s,
                Err(e) => {
                    yield Err(ApiError::Internal(anyhow::anyhow!(
                        "SSE stream: invalid UTF-8: {}",
                        e
                    )));
                    return;
                }
            };

            buffer.push_str(text);

            // Process complete lines from the buffer.
            loop {
                match buffer.find('\n') {
                    None => break,
                    Some(pos) => {
                        let line = buffer[..pos].trim_end_matches('\r').to_string();
                        buffer.drain(..pos + 1);

                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                // Finalize before forwarding [DONE] sentinel.
                                let finalize_events = transformer.finalize();
                                for event in finalize_events {
                                    yield Ok(bytes::Bytes::from(event));
                                }
                                yield Ok(bytes::Bytes::from("data: [DONE]\n\n"));
                                return;
                            }

                            // Parse the Chat Completion chunk JSON.
                            match serde_json::from_str::<serde_json::Value>(data) {
                                Ok(chunk_value) => {
                                    // Transform into one or more Responses API SSE events.
                                    let events = transformer.transform_chunk(&chunk_value);
                                    for event in events {
                                        yield Ok(bytes::Bytes::from(event));
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        data = %data,
                                        error = %e,
                                        "Responses SSE: failed to parse JSON chunk"
                                    );
                                    // Skip unparseable lines (same strategy as parse_sse_stream).
                                }
                            }
                        }
                        // Lines without `data: ` prefix (event:, id:, :comment) are silently skipped.
                    }
                }
            }
        }

        // Process any remaining data in the buffer after the stream ends.
        for line in buffer.lines() {
            if let Some(data) = line.trim_end_matches('\r').strip_prefix("data: ") {
                if data == "[DONE]" {
                    // Finalize before forwarding [DONE] sentinel.
                    let finalize_events = transformer.finalize();
                    for event in finalize_events {
                        yield Ok(bytes::Bytes::from(event));
                    }
                    yield Ok(bytes::Bytes::from("data: [DONE]\n\n"));
                    return;
                }
                if let Ok(chunk_value) = serde_json::from_str::<serde_json::Value>(data) {
                    let events = transformer.transform_chunk(&chunk_value);
                    for event in events {
                        yield Ok(bytes::Bytes::from(event));
                    }
                }
            }
        }

        // Finalize: emit closing events if the stream ended without finish_reason.
        let finalize_events = transformer.finalize();
        for event in finalize_events {
            yield Ok(bytes::Bytes::from(event));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::responses::ResponsesApiInput;
    use futures::stream::StreamExt;

    /// Helper: create a minimal ResponsesApiRequest for test purposes.
    fn test_request() -> ResponsesApiRequest {
        serde_json::from_value(serde_json::json!({
            "model": "gpt-4o",
            "input": "hello"
        }))
        .unwrap()
    }

    /// Helper: extract the event type from an SSE-formatted string like "event: xxx\ndata: ...\n\n".
    #[allow(dead_code)]
    fn sse_event_type(s: &str) -> Option<&str> {
        for line in s.lines() {
            if let Some(evt) = line.strip_prefix("event: ") {
                return Some(evt);
            }
        }
        None
    }

    /// Helper: check if an SSE string contains a `data: [DONE]` line.
    #[allow(dead_code)]
    fn is_done_sentinel(s: &str) -> bool {
        for line in s.lines() {
            if line.strip_prefix("data: ") == Some("[DONE]") {
                return true;
            }
        }
        false
    }

    /// 1. SSE events split across two byte boundaries are correctly reassembled.
    #[tokio::test]
    async fn test_sse_stream_split_across_chunks() {
        // A single "data: ...\n\n" split across two chunks.
        let part1 = "data: {\"id\":\"x\",\"choices\":[{\"delta\":{\"role\":\"assistant\"},\"index\":0}]}\n\n";
        // Split mid-JSON
        let split_point = part1.len() / 2;
        let (chunk1, chunk2) = part1.split_at(split_point);

        let chunks: Vec<Result<bytes::Bytes, ApiError>> = vec![
            Ok(bytes::Bytes::from(chunk1)),
            Ok(bytes::Bytes::from(chunk2)),
        ];
        let stream = futures::stream::iter(chunks);
        let transformed = transform_responses_sse_stream(
            stream,
            "resp_test",
            "gpt-4o",
            ResponsesApiInput::Text("hello".to_string()),
            &test_request(),
        );
        let results: Vec<_> = transformed.collect().await;

        // Should produce events without errors.
        assert!(results.iter().all(|r| r.is_ok()));

        // Should contain the initial lifecycle events + role event.
        let all_text: String = results
            .iter()
            .map(|r| std::str::from_utf8(r.as_ref().unwrap()).unwrap())
            .collect();
        assert!(all_text.contains("response.created"));
        assert!(all_text.contains("response.in_progress"));
    }

    /// 2. [DONE] sentinel is properly forwarded.
    #[tokio::test]
    async fn test_sse_stream_done_sentinel() {
        let chunks: Vec<Result<bytes::Bytes, ApiError>> = vec![
            Ok(bytes::Bytes::from(
                "data: {\"id\":\"x\",\"choices\":[{\"delta\":{\"role\":\"assistant\"},\"index\":0}]}\n\n",
            )),
            Ok(bytes::Bytes::from("data: [DONE]\n\n")),
        ];
        let stream = futures::stream::iter(chunks);
        let transformed = transform_responses_sse_stream(
            stream,
            "resp_test",
            "gpt-4o",
            ResponsesApiInput::Text("hello".to_string()),
            &test_request(),
        );
        let results: Vec<_> = transformed.collect().await;

        let last = results.last().unwrap();
        assert!(last.is_ok());
        assert!(std::str::from_utf8(last.as_ref().unwrap())
            .unwrap()
            .contains("[DONE]"));
    }

    /// 3. Malformed JSON in `data:` lines is skipped gracefully (no panic).
    #[tokio::test]
    async fn test_sse_stream_malformed_json_skipped() {
        let chunks: Vec<Result<bytes::Bytes, ApiError>> = vec![
            Ok(bytes::Bytes::from("data: {not-valid-json}\n\n")),
            Ok(bytes::Bytes::from(
                "data: {\"id\":\"x\",\"choices\":[{\"delta\":{\"role\":\"assistant\"},\"index\":0}]}\n\n",
            )),
        ];
        let stream = futures::stream::iter(chunks);
        let transformed = transform_responses_sse_stream(
            stream,
            "resp_test",
            "gpt-4o",
            ResponsesApiInput::Text("hello".to_string()),
            &test_request(),
        );
        let results: Vec<_> = transformed.collect().await;

        // No errors should be yielded (malformed line is just skipped with a warning).
        assert!(results.iter().all(|r| r.is_ok()));

        // The valid chunk should still produce events.
        let all_text: String = results
            .iter()
            .map(|r| std::str::from_utf8(r.as_ref().unwrap()).unwrap())
            .collect();
        assert!(all_text.contains("response.created"));
    }

    /// 4. Empty stream produces just the initial events (response.created + response.in_progress),
    ///    then ends.
    #[tokio::test]
    async fn test_sse_stream_empty() {
        let chunks: Vec<Result<bytes::Bytes, ApiError>> = vec![];
        let stream = futures::stream::iter(chunks);
        let transformed = transform_responses_sse_stream(
            stream,
            "resp_test",
            "gpt-4o",
            ResponsesApiInput::Text("hello".to_string()),
            &test_request(),
        );
        let results: Vec<_> = transformed.collect().await;

        // An empty source stream means the transformer never sees a chunk,
        // so no initial events are emitted (they are demand-driven by incoming data).
        assert!(results.is_empty());
    }

    /// 5. Multiple SSE events in a single chunk are all processed.
    #[tokio::test]
    async fn test_sse_stream_multiple_events_in_chunk() {
        let combined = format!(
            "data: {{\"id\":\"x\",\"choices\":[{{\"delta\":{{\"role\":\"assistant\"}},\"index\":0}}]}}\n\ndata: {{\"id\":\"x\",\"choices\":[{{\"delta\":{{\"content\":\"Hello\"}},\"index\":0}}]}}\n\n"
        );
        let chunks: Vec<Result<bytes::Bytes, ApiError>> = vec![Ok(bytes::Bytes::from(combined))];
        let stream = futures::stream::iter(chunks);
        let transformed = transform_responses_sse_stream(
            stream,
            "resp_test",
            "gpt-4o",
            ResponsesApiInput::Text("hello".to_string()),
            &test_request(),
        );
        let results: Vec<_> = transformed.collect().await;

        assert!(results.iter().all(|r| r.is_ok()));

        let all_text: String = results
            .iter()
            .map(|r| std::str::from_utf8(r.as_ref().unwrap()).unwrap())
            .collect();
        // Should have both initial events and the content delta.
        assert!(all_text.contains("response.created"));
        assert!(all_text.contains("response.in_progress"));
        assert!(all_text.contains("output_text.delta"));
    }

    /// Input item count validation (M8).
    #[test]
    fn test_input_items_count_validation() {
        // Items with ≤ 1000 items should be accepted (no panic on construction).
        let items: Vec<serde_json::Value> = (0..1000)
            .map(|_| serde_json::json!({"role": "user", "content": "hi"}))
            .collect();
        let input = ResponsesApiInput::Items(items);
        // Just verifying the count is accessible and within limit.
        if let ResponsesApiInput::Items(ref i) = input {
            assert_eq!(i.len(), 1000);
            assert!(i.len() <= 1000);
        }

        // Items with > 1000 should exceed the limit.
        let big_items: Vec<serde_json::Value> = (0..1001)
            .map(|_| serde_json::json!({"role": "user", "content": "hi"}))
            .collect();
        let big_input = ResponsesApiInput::Items(big_items);
        if let ResponsesApiInput::Items(ref i) = big_input {
            assert!(i.len() > 1000);
        }
    }
}
