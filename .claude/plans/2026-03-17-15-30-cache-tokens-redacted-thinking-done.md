# Plan: Cache Tokens & Redacted Thinking (#109, #110, #112, #113, #121)

## Consultation Summary

### Backend (Explore Agent 1 — Cache Tokens)
- `ChatCompletionUsage` has 4 fields: `prompt_tokens`, `completion_tokens`, `total_tokens`, `credits_used` — no cache fields
- `AnthropicUsage` has 2 fields: `input_tokens`, `output_tokens` — no cache fields
- Converters (`kiro_to_openai.rs`, `kiro_to_anthropic.rs`) only map basic input/output tokens
- Cross-format streaming (`cross_format.rs`) passes `MessageDeltaUsage { output_tokens }` — no cache fields
- DB schema `usage_records` has no cache columns; `insert_usage_record()` takes no cache params
- Cost module (`cost.rs`) uses 2-tuple pricing `(input_cost, output_cost)` — no cache pricing tier
- Route handlers extract `prompt_tokens`/`completion_tokens` from response body, ignore cache fields

### Backend (Explore Agent 2 — Redacted Thinking)
- `ContentBlock` enum has 5 variants: `Text`, `Thinking { thinking, signature }`, `Image`, `ToolUse`, `ToolResult` — no `RedactedThinking`
- `Delta` enum has 3 variants: `TextDelta`, `ThinkingDelta`, `InputJsonDelta` — no `SignatureDelta`
- `thinking_parser.rs` is a stateful FSM (24 tests) — parses `<thinking>` XML tags, no redaction logic
- Cross-format streaming maps `ThinkingDelta` → `reasoning_content` but has no redacted variant handling
- Kiro's `ContentBlock` only has `Text` — no thinking support from Kiro (thinking comes from direct Anthropic/OpenAI providers)

### Testing (Explore Agent 3)
- Model files (`openai.rs`, `anthropic.rs`, `kiro.rs`) have no test modules — pure data structs
- Converter files have extensive tests (148 total): `core.rs` 60, `anthropic_to_kiro.rs` 30, etc.
- Streaming tests: `mod.rs` 30, `cross_format.rs` 13, `sse.rs` 11
- Test pattern: `#[cfg(test)] mod tests` at bottom, `test_<function>_<scenario>` naming
- Usage conversion tests exist in converter files — extend with cache token assertions

## Scope Analysis

Two independent tracks that touch overlapping files but different fields:

| Track | Issues | Complexity | Files |
|-------|--------|-----------|-------|
| A: Cache Tokens | #109, #113, #121 | Medium | 6 files modify |
| B: Redacted Thinking | #110, #112 | Small-Medium | 4 files modify |

**Total: backend-only change, no frontend, no DB migration, no infra.**

## File Manifest

| File | Action | Owner | Track | Wave |
|------|--------|-------|-------|------|
| `backend/src/models/openai.rs` | modify | rust-backend-engineer | A | 1 |
| `backend/src/models/anthropic.rs` | modify | rust-backend-engineer | A+B | 1 |
| `backend/src/converters/kiro_to_openai.rs` | modify | rust-backend-engineer | A | 2 |
| `backend/src/converters/kiro_to_anthropic.rs` | modify | rust-backend-engineer | A | 2 |
| `backend/src/streaming/cross_format.rs` | modify | rust-backend-engineer | A+B | 2 |
| `backend/src/converters/openai_to_anthropic.rs` | modify | rust-backend-engineer | A+B | 2 |
| `backend/src/converters/anthropic_to_openai.rs` | modify | rust-backend-engineer | A+B | 2 |

## Wave 1: Model Types (foundations — no dependencies)

### Task 1.1: Add cache token fields to usage models (#113)

**`backend/src/models/openai.rs`** — Add `PromptTokensDetails` struct and field:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTokensDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<i32>,
}

// Add to ChatCompletionUsage:
#[serde(skip_serializing_if = "Option::is_none")]
pub prompt_tokens_details: Option<PromptTokensDetails>,
```

**`backend/src/models/anthropic.rs`** — Add cache fields to `AnthropicUsage`:
```rust
pub struct AnthropicUsage {
    pub input_tokens: i32,
    pub output_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i32>,
}
```

**Tests**: Serde round-trip tests for both structs (serialize with cache fields, deserialize without, and vice versa).

### Task 1.2: Add redacted_thinking to Anthropic models (#112)

**`backend/src/models/anthropic.rs`** — Add `RedactedThinking` variant to `ContentBlock`:
```rust
RedactedThinking {
    data: String,
},
```

Add `SignatureDelta` variant to `Delta`:
```rust
SignatureDelta {
    signature: String,
},
```

**Tests**: Serde round-trip tests — serialize/deserialize `RedactedThinking` block and `SignatureDelta`.

## Wave 2: Converters & Streaming (depends on Wave 1)

### Task 2.1: Map cache tokens in Kiro converters (#121 partial)

**`backend/src/converters/kiro_to_openai.rs`** — Map cache tokens from Kiro response:
```rust
// In usage construction:
prompt_tokens_details: None, // Kiro doesn't expose cache tokens yet
```
Note: Kiro format doesn't include cache fields. The field exists for passthrough from direct Anthropic/OpenAI providers.

**`backend/src/converters/kiro_to_anthropic.rs`** — Same:
```rust
cache_creation_input_tokens: None,
cache_read_input_tokens: None,
```

**Tests**: Verify cache fields are `None` in Kiro converter output. Update existing converter tests to include the new fields.

### Task 2.2: Map cache tokens in cross-format converters (#121)

**`backend/src/converters/openai_to_anthropic.rs`** — No response conversion (request-only converter), skip.

**`backend/src/converters/anthropic_to_openai.rs`** — No response conversion (request-only converter), skip.

**`backend/src/streaming/cross_format.rs`** — Map cache tokens in streaming usage events:

OpenAI → Anthropic translator: When constructing `MessageDeltaUsage`, pass through cache fields from `ChatCompletionUsage` if present. Note: `MessageDeltaUsage` only has `output_tokens` — cache tokens flow in the `message_start` event's `AnthropicUsage`, not in delta.

Anthropic → OpenAI translator: When constructing final `ChatCompletionUsage` from `MessageDelta`, include `prompt_tokens_details` if cache data available from the `message_start` event.

**Implementation detail**: The `message_start` event carries `AnthropicUsage` (with cache fields). The translator needs to capture these from `MessageStart` and include them in the final OpenAI chunk that carries usage. Add state field to `AnthropicToOpenAITranslator`:
```rust
cached_usage: Option<AnthropicUsage>, // captured from message_start
```

**Tests**: Add streaming translation tests that include cache token fields in usage events.

### Task 2.3: Handle redacted thinking in converters (#110)

**`backend/src/streaming/cross_format.rs`** — Handle new variants:

Anthropic → OpenAI: `SignatureDelta` → skip (no OpenAI equivalent, signatures are Anthropic-specific).

OpenAI → Anthropic: No change needed (OpenAI doesn't have redacted thinking).

**`backend/src/converters/anthropic_to_openai.rs`** — Request converter, no response handling. Skip.

**`backend/src/converters/openai_to_anthropic.rs`** — Request converter. If input messages contain `RedactedThinking` content blocks (multi-turn replay), pass them through as-is.

**Tests**: Streaming test with `SignatureDelta` event, round-trip test with `RedactedThinking` in message history.

## Wave 3: Verification & Cleanup

### Task 3.1: Run quality gates
- `cargo clippy --all-targets` — zero warnings
- `cargo fmt --check` — no diffs
- `cargo test --lib` — all tests pass (including new ones)

### Task 3.2: Update compilation sites
After adding new fields to `AnthropicUsage` and `ChatCompletionUsage`, fix all struct construction sites that don't use `..Default` or explicit field initialization. Key locations:
- `kiro_to_openai.rs` lines 94, 101 — add `prompt_tokens_details: None`
- `kiro_to_anthropic.rs` lines 61, 66 — add `cache_creation_input_tokens: None, cache_read_input_tokens: None`
- `cross_format.rs` lines ~68, ~347 — add new fields
- `integration_test.rs` — if it constructs usage structs directly

## Interface Contracts

### OpenAI Cache Token Format (API spec)
```json
{
  "usage": {
    "prompt_tokens": 100,
    "completion_tokens": 50,
    "total_tokens": 150,
    "prompt_tokens_details": {
      "cached_tokens": 80
    }
  }
}
```

### Anthropic Cache Token Format (API spec)
```json
{
  "usage": {
    "input_tokens": 100,
    "output_tokens": 50,
    "cache_creation_input_tokens": 20,
    "cache_read_input_tokens": 80
  }
}
```

### Anthropic Redacted Thinking (API spec)
```json
{
  "type": "redacted_thinking",
  "data": "base64-encoded-encrypted-data"
}
```

### Anthropic Signature Delta (streaming)
```json
{
  "type": "signature_delta",
  "signature": "partial-signature-string"
}
```

### Cross-Format Cache Token Mapping
| OpenAI | Anthropic | Direction |
|--------|-----------|-----------|
| `prompt_tokens_details.cached_tokens` | `cache_read_input_tokens` | bidirectional |
| (no equivalent) | `cache_creation_input_tokens` | Anthropic-only, pass through |

## Out of Scope (deliberate exclusions)

These were identified during exploration but are NOT part of this plan:

1. **DB schema migration for cache tokens** — The `usage_records` table doesn't track cache tokens. Adding columns is a separate concern (#109 acceptance criteria mentions "update routes to extract cache tokens" but the DB schema change should be a follow-up to avoid scope creep).
2. **Cost calculation with cache pricing** — `cost.rs` doesn't account for cache discounts (Anthropic charges ~10% for cache reads). Separate issue.
3. **Thinking parser changes** — `thinking_parser.rs` handles XML tag parsing for fake reasoning. `RedactedThinking` is a structured JSON content block, not an XML tag — no parser changes needed.
4. **Kiro format cache tokens** — Kiro's `KiroUsage` doesn't include cache fields. If/when Kiro adds them, update converters separately.

## Verification

```bash
cd backend && cargo clippy --all-targets   # zero warnings
cd backend && cargo fmt --check            # no diffs
cd backend && cargo test --lib             # all tests pass
```

## Recommended Preset

```
/team-implement --preset backend-feature
```

Team size: 1 `rust-backend-engineer` (all changes are backend-only, single-service, no cross-layer work). A `backend-qa` agent is optional but the implementation agent can write tests inline.

## Issue Mapping

| Wave | Task | Issues Resolved |
|------|------|----------------|
| 1.1 | Cache token usage model fields | #113 |
| 1.2 | RedactedThinking + SignatureDelta variants | #112 |
| 2.1-2.2 | Cache token mapping in converters | #121 |
| 2.3 | Redacted thinking in converters/streaming | #110 |
| 3.1 | Quality gates | #109 (acceptance criteria: tests pass) |
