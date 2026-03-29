# Plan: Remediate Critical + High Review Findings (S-001 through S-012)

## Context

Combined Claude team + Codex deep review of the Harbangan gateway identified 3 critical and 9 high-severity findings across client endpoints, model routing, multi-provider support, and request/response conversion. This plan addresses all 12 findings in dependency-ordered waves.

The biggest risks are concentrated in the **direct provider path** (not Kiro): cross-format non-streaming requests/responses silently drop tools, provider prefixes are advisory not binding, token refresh can target the wrong provider, and streaming traffic never records usage.

## Consultation Summary

- **rust-backend-engineer**: 11 findings directly affect backend/src/. Handler duplication is the largest DRY violation. Provider-level converters (`anthropic.rs:44`, `mod.rs:23`) are dangerously simplified vs the full `converters/` module. `pick_best_provider()` ignores explicit prefixes. `ensure_fresh_token()` infers provider from model name before routing resolves.
- **database-engineer**: `insert_usage_record` signature is `(user_id, provider, model, input_tokens, output_tokens, cost)`. No schema changes needed for streaming usage — same function works. Token helpers (`get_user_provider_token`, `delete_user_provider_token`) are provider-wide, not account-specific — multi-account safety concern confirmed.
- **backend-qa**: 354 proxy-related tests exist. Round-trip test pattern in `converters/mod.rs` is well-structured but all use `tools: None`. `kiro_to_openai.rs` has only 2 tests. `create_test_state()` exists in `routes/mod.rs:99` but lacks DB-backed test state.
- **react-frontend-engineer**: No frontend changes needed. Usage UI auto-displays more data when streaming records are added. Model list changes are transparent.
- **devops-engineer**: No Docker/infra changes. Config already has `http_max_connections`, `http_connect_timeout`, `http_request_timeout` in `config.rs:43-46`.
- **frontend-qa**: No E2E test impact.
- **document-writer**: CLAUDE.md should document provider prefix semantics after S-003 fix.

## File Manifest

| File | Action | Owner | Wave |
|------|--------|-------|------|
| `backend/src/routes/pipeline.rs` | modify | rust-backend-engineer | 1 |
| `backend/src/routes/openai.rs` | modify | rust-backend-engineer | 1,2 |
| `backend/src/routes/anthropic.rs` | modify | rust-backend-engineer | 1,2 |
| `backend/src/providers/registry.rs` | modify | rust-backend-engineer | 1 |
| `backend/src/providers/types.rs` | modify | rust-backend-engineer | 2 |
| `backend/src/providers/mod.rs` | modify | rust-backend-engineer | 1,2 |
| `backend/src/providers/anthropic.rs` | modify | rust-backend-engineer | 1,2 |
| `backend/src/providers/openai_codex.rs` | modify | rust-backend-engineer | 1,2 |
| `backend/src/providers/copilot.rs` | modify | rust-backend-engineer | 1,2 |
| `backend/src/providers/custom.rs` | modify | rust-backend-engineer | 1,2 |
| `backend/src/streaming/mod.rs` | modify | rust-backend-engineer | 2 |
| `backend/src/streaming/sse.rs` | modify | rust-backend-engineer | 2 |
| `backend/src/converters/openai_to_anthropic.rs` | modify | rust-backend-engineer | 2 |
| `backend/src/converters/anthropic_to_openai.rs` | modify | rust-backend-engineer | 2 |
| `backend/src/converters/mod.rs` | modify | rust-backend-engineer | 3 |
| `backend/src/converters/kiro_to_openai.rs` | modify | rust-backend-engineer | 3 |
| `backend/src/converters/kiro_to_anthropic.rs` | modify | rust-backend-engineer | 3 |

---

## Wave 1: Safety + Routing Fixes (No Behavioral Change Risk)

Isolated, low-risk fixes that don't change the request/response format.

### Task 1.1: Fix `.unwrap()` panic on provider credentials [S-008]
**Assigned**: rust-backend-engineer
**Files**: `routes/anthropic.rs:76`, `routes/openai.rs:117`
**Change**: Replace `routing.provider_creds.clone().unwrap()` with:
```rust
routing.provider_creds.clone().ok_or_else(|| {
    ApiError::AuthError(format!("No credentials available for provider {:?}", routing.provider_id))
})?
```
**Complexity**: Small (2 one-line changes)

### Task 1.2: Enforce provider prefix as binding [S-003]
**Assigned**: rust-backend-engineer
**Files**: `providers/registry.rs` — `resolve_provider_with_balancing()` (~line 422), `resolve_provider()` (~line 334), `resolve_from_proxy_creds()` (~line 554)

**Change**: When `parse_prefixed_model()` returns a provider, that provider is **binding** — no fallback to Copilot/Kiro.

Add an `is_explicit_prefix` flag:
```rust
let (native, is_explicit_prefix) = if let Some((provider, _)) = Self::parse_prefixed_model(model) {
    (provider, true)
} else if let Some(provider) = Self::provider_for_model(model) {
    (provider, false)
} else {
    return (ProviderId::Kiro, None, None);
};
```

When `is_explicit_prefix == true`:
- In `resolve_provider_with_balancing()`: only load accounts for the specified provider (skip Copilot candidates at line 482, skip admin pool for other providers). If no candidates exist, **return a new sentinel** `(specified_provider, None, None)` — NOT Kiro.
- In the route handlers (`openai.rs`, `anthropic.rs`): after routing, if `provider_creds` is `None` AND the model had an explicit prefix, return `ApiError::ValidationError("No credentials configured for provider {prefix}")` instead of attempting Kiro.
- In `resolve_provider()`: same logic — when explicit prefix, bypass `pick_best_provider()`, directly look up the specified provider.

**Codex correction**: The original plan returned `ProviderId::Kiro` which would silently fall through to Kiro. The fix returns the specified provider with `None` creds, letting the Task 1.1 error handling catch it.

**Complexity**: Medium

### Task 1.3: Fix token refresh targeting wrong provider [S-006]
**Assigned**: rust-backend-engineer
**Files**: `providers/registry.rs`, `routes/pipeline.rs`

**Codex correction**: Moving `ensure_fresh_token()` after routing breaks the cache contract — `resolve_provider()` returns cached creds without expiry re-check (registry.rs:357), so moving refresh after would use stale tokens.

**Revised approach**: Keep `ensure_fresh_token()` before routing, but fix the provider inference to respect explicit prefixes:

In `ensure_fresh_token()` (registry.rs:201), change provider determination:
```rust
// Before: let Some(target) = Self::provider_for_model(model) else { return; };
// After:
let target = if let Some((provider, _)) = Self::parse_prefixed_model(model) {
    provider
} else if let Some(provider) = Self::provider_for_model(model) {
    provider
} else {
    return;
};
```

This ensures `openai_codex/claude-sonnet-4` refreshes OpenAI Codex tokens (not Anthropic), while unprefixed `claude-sonnet-4` still refreshes Anthropic as before.

**Complexity**: Small (3-line change in one function)

### Task 1.4: Share HTTP client across providers [S-010]
**Assigned**: rust-backend-engineer
**Files**: `providers/mod.rs` (build_provider_map), `providers/anthropic.rs`, `providers/openai_codex.rs`, `providers/copilot.rs`, `providers/custom.rs`

**Codex correction**: Config already has `http_max_connections`, `http_connect_timeout`, `http_request_timeout` in `config.rs:43-46`. Use those instead of hardcoded values.

**Change**: In `build_provider_map()` (which already receives config via its parameters):
```rust
let config = config.read().unwrap_or_else(|p| p.into_inner());
let shared_client = reqwest::Client::builder()
    .pool_max_idle_per_host(config.http_max_connections)
    .connect_timeout(Duration::from_secs(config.http_connect_timeout))
    .timeout(Duration::from_secs(config.http_request_timeout))
    .build()
    .expect("Failed to build HTTP client");
```
Update each provider: `AnthropicProvider::new(client: reqwest::Client)`, etc. Store `client` instead of creating a new one.

**Complexity**: Medium (modify 5 provider files + build_provider_map)

---

## Wave 2: Cross-Format Conversion + Streaming Fixes

These change request/response behavior — highest-impact changes.

### Task 2.1: Replace provider-level request converters with full converters [S-004]
**Assigned**: rust-backend-engineer
**Files**: `providers/anthropic.rs`, `providers/mod.rs`, `converters/openai_to_anthropic.rs`, `converters/anthropic_to_openai.rs`

**Step 1 — Fix full converters to handle tools**:

In `converters/openai_to_anthropic.rs`:
- Line 103 sets `tools: None` — change to map `req.tools` to Anthropic format
- Map OpenAI tool definitions `{type: "function", function: {name, description, parameters}}` → Anthropic `{name, description, input_schema}`
- Map `tool_calls` in assistant messages → Anthropic `tool_use` content blocks
- Map `role: "tool"` messages → Anthropic `tool_result` content blocks (requires buffering/reordering per `openai_to_kiro.rs:139` and `core.rs:842` patterns)

In `converters/anthropic_to_openai.rs`:
- Map Anthropic tool definitions `{name, description, input_schema}` → OpenAI format
- Map `tool_use` content blocks in assistant messages → OpenAI `tool_calls`
- Map `tool_result` content blocks → `role: "tool"` messages

**Codex correction**: Tool-result ordering is complex — reuse buffering pattern from `openai_to_kiro.rs:139` and orphan stripping from `core.rs:842`. Not a simple field mapping.

**Step 2 — Replace provider-level converters**:

In `providers/anthropic.rs`: replace `openai_to_anthropic_body()` with call to `converters::openai_to_anthropic::openai_to_anthropic()` + `serde_json::to_value()`.

In `providers/mod.rs`: replace `anthropic_to_openai_body()` with call to `converters::anthropic_to_openai::anthropic_to_openai()` + `serde_json::to_value()`.

**Complexity**: Large

### Task 2.2: Fix non-streaming response normalizers [S-005]
**Assigned**: rust-backend-engineer
**Files**: `providers/anthropic.rs:215` (normalize_response_for_openai), `providers/openai_codex.rs:178` (normalize_response_for_anthropic)

**Change in `AnthropicProvider::normalize_response_for_openai()`**:
- Iterate ALL content blocks (not just first)
- Map `type: "tool_use"` blocks → OpenAI `tool_calls` array: `{id, type: "function", function: {name, arguments}}`
- Map `stop_reason: "tool_use"` → `finish_reason: "tool_calls"`
- Concatenate text blocks as `content`

**Change in OpenAI-compatible providers' `normalize_response_for_anthropic()`**:
- **Codex note**: The shared normalizer is in `openai_codex.rs:178` — this is the change point for all OpenAI-compatible providers (OpenAICodex, Copilot, Custom use same normalizer via trait defaults or shared function)
- Map `tool_calls` array → Anthropic `tool_use` content blocks
- Map `finish_reason: "tool_calls"` → `stop_reason: "tool_use"`

**Complexity**: Large

### Task 2.3: Add streaming usage tracking [S-001]
**Assigned**: rust-backend-engineer
**Files**: `routes/openai.rs`, `routes/anthropic.rs`, `routes/pipeline.rs`

**Approach**: Wrap the streaming response with a usage-extracting tap.

**Codex correction**: Not all providers include usage in streaming. The wrapper must:
1. Only persist when tokens > 0 (avoid bogus zero-token records from cross_format.rs:188)
2. Handle the case where no usage chunk exists (skip DB insert entirely)
3. For providers that don't emit usage, consider estimating via tiktoken (existing `tokenizer.rs`)

In `pipeline.rs`, add:
```rust
pub fn wrap_stream_with_usage_tracking(
    stream: Pin<Box<dyn Stream<Item = ProviderStreamItem> + Send>>,
    config_db: Option<Arc<ConfigDb>>,
    user_id: Option<Uuid>,
    provider_id: ProviderId,
    model: String,
) -> Pin<Box<dyn Stream<Item = ProviderStreamItem> + Send>>
```

This wrapper:
1. Passes all chunks through unchanged (no latency impact)
2. Inspects each `Bytes` chunk: if it contains `data: {..."usage":...}`, extract token counts
3. On stream end, if tokens > 0, spawns `tokio::spawn` to call `insert_usage_record`

Apply in both handlers' streaming branches, after the provider returns the stream.

**Complexity**: Medium

### Task 2.4: Fix SseParser buffer reallocation [S-011]
**Assigned**: rust-backend-engineer
**Files**: `streaming/mod.rs:520`, `streaming/sse.rs:53`

**Change**: Replace buffer slice + `to_string()` with `drain()`:
```rust
// mod.rs:520 — Before: self.buffer = self.buffer[json_end + 1..].to_string();
self.buffer.drain(..json_end + 1);

// sse.rs:53 — Before: buffer = buffer[pos + 1..].to_string();
buffer.drain(..pos + 1);
```
**Complexity**: Small (2 one-line changes)

### Task 2.5: Stabilize /v1/models namespace [S-007]
**Assigned**: rust-backend-engineer
**Files**: `routes/openai.rs` (get_models_handler), `providers/types.rs` (ProviderId::from_str)

**Codex correction**: Adding `openai/` display without updating `ProviderId::from_str()` would advertise IDs that clients can't send back. Must be coordinated:

**Change**:
1. In `ProviderId::from_str()` (`types.rs:89`): accept `"openai"` as alias for `OpenAICodex`
2. In `get_models_handler()`: for registry models, use consistent prefixed format
3. Ensure response `model` field matches the requested model ID (don't let prefix stripping cause drift)

**Complexity**: Medium

### Task 2.6: Extract shared dispatch loop [S-009]
**Assigned**: rust-backend-engineer
**Files**: `routes/pipeline.rs`, `routes/openai.rs`, `routes/anthropic.rs`

**Change**: Extract the retry/failover loop into a generic function in `pipeline.rs`. Use closures for format-specific operations:

```rust
pub async fn dispatch_with_failover(
    state: &AppState,
    user_creds: Option<&UserKiroCreds>,
    model: &str,
    provider_id: &ProviderId,
    initial_creds: ProviderCredentials,
    initial_routing: ProviderRouting,
    execute_fn: impl AsyncFn(...) -> Result<ProviderResponse, ApiError>,
    stream_fn: impl AsyncFn(...) -> Result<ProviderStreamResponse, ApiError>,
    normalize_fn: impl Fn(&dyn Provider, &str, Value) -> Value,
    extract_usage_fn: impl Fn(&Value) -> (i32, i32),
    extract_user_content_fn: impl Fn(...) -> String,
    extract_assistant_content_fn: impl Fn(&Value) -> String,
    build_request_context_fn: impl Fn(...) -> RequestContext,
    config: Config,
    is_streaming: bool,
) -> Result<Response, ApiError>
```

The OpenAI and Anthropic handlers become thin wrappers: parse body → validate → call `dispatch_with_failover` with format-specific closures.

**Complexity**: Large (major refactor, but eliminates ~200 lines of duplication)

---

## Wave 3: Test Coverage

**All Wave 3 tasks assigned to rust-backend-engineer** (file ownership: `backend/src/**` is owned by rust-backend-engineer per team-coordination.md).

### Task 3.1: Tool calling round-trip tests [S-002]
**Files**: `converters/mod.rs` (add to existing `#[cfg(test)] mod tests`)

Add tests using the existing `openai_req`/`anthropic_req` helpers:
- OpenAI tool_calls → Anthropic tool_use → OpenAI tool_calls
- Anthropic tool_use → OpenAI tool_calls → Anthropic tool_use
- Tool results through both paths
- Multiple concurrent tool calls
- ~8 new tests

### Task 3.2: Kiro→client response converter tests [S-012]
**Files**: `converters/kiro_to_openai.rs`, `converters/kiro_to_anthropic.rs`

Expand from 2/3 tests to cover:
- Multi-tool responses (`tool_uses` in KiroResponse)
- Empty/null content blocks
- Usage data propagation
- Stop reason mapping

**Codex correction**: `KiroResponse` doesn't model thinking blocks (only text + tool_uses in `models/kiro.rs:56,77`), so thinking block tests belong in streaming tests, not here.

### Task 3.3: Provider routing + handler verification tests [S-002]
**Files**: `providers/registry.rs` (tests), `routes/openai.rs` (tests), `routes/anthropic.rs` (tests)

Add tests for:
- Explicit prefix enforced (S-003): prefixed model only routes to specified provider
- Token refresh targets correct provider for prefixed models (S-006)
- Provider credentials `None` returns `AuthError` not panic (S-008): test in `routes/openai.rs` and `routes/anthropic.rs`
- Usage tracking wrapper fires for streaming (S-001)

Use existing `create_test_state()` from `routes/mod.rs:99`.

---

## Verification

```bash
# After each wave:
cd backend && cargo fmt
cd backend && cargo clippy --all-targets   # Zero new warnings
cd backend && cargo test --lib             # All 820+ tests pass + new tests

# Specific test groups:
cargo test --lib converters::              # Round-trip tests with tools
cargo test --lib providers::registry::     # Prefix enforcement tests
cargo test --lib providers::types::        # Provider type tests
cargo test --lib streaming::               # Buffer + usage tests
```

## Branch

`fix/remediate-review-findings`

## Review Status
- Codex review: **passed with adjustments**
- Codex findings: 5 high, 4 medium, 1 low — all valid, all addressed
- Key adjustments:
  1. Task 1.2: Return specified provider (not Kiro) when no creds, let handler error
  2. Task 1.3: Fix provider inference in `ensure_fresh_token()` instead of moving the call
  3. Task 1.4: Use existing config values (`http_max_connections` etc.) not hardcoded
  4. Task 2.1: Tool-result ordering requires reuse of buffering pattern from core.rs
  5. Task 2.3: Only persist usage when tokens > 0 to avoid bogus zero records
  6. Task 2.5: Must update `ProviderId::from_str()` to accept `openai` alias
  7. Wave 3: All test tasks assigned to rust-backend-engineer (file ownership rule)
- Disputed findings: 0
