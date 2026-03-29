# Plan: Fix Model Registry Populate — Use Connected Provider Tokens

## Context

`POST /_ui/api/models/registry/populate` returns `{"success":true,"models_upserted":0}` despite all providers being connected. Each provider's populate path looks for credentials in the wrong source:

| Provider | Populate looks at | User tokens actually stored in |
|---|---|---|
| Anthropic | `admin_provider_pool` (empty) | `user_provider_tokens` |
| OpenAI Codex | `admin_provider_pool` (empty) | `user_provider_tokens` |
| Kiro | Global `AuthManager` (no env creds) | `user_kiro_tokens` |
| Copilot | `get_expiring_copilot_tokens` (5-min window) | `user_copilot_tokens` (healthy token excluded) |

Live DB: `admin_provider_pool=0`, `user_provider_tokens=2`, `user_kiro_tokens=1`, `user_copilot_tokens=1`, `model_registry=0`.

## File Manifest

| File | Action | Wave |
|------|--------|------|
| `backend/src/web_ui/config_db.rs` | modify — add 2 new DB query functions | 1 |
| `backend/src/web_ui/model_registry.rs` | modify — add `fetch_kiro_models_with_token`, update `populate_provider` with fallback logic, fix copilot query | 2 |
| `backend/src/web_ui/model_registry_handlers.rs` | modify — simplify handler, remove special Kiro branching | 2 |

All files owned by `rust-backend-engineer`.

## Wave 1: New DB Functions (`config_db.rs`)

### 1a. `get_any_valid_kiro_credential()`

```rust
pub async fn get_any_valid_kiro_credential(&self) -> Result<Option<(String, Option<String>)>> {
    // Returns (access_token, Option<oauth_sso_region>)
    // Query: SELECT access_token, oauth_sso_region FROM user_kiro_tokens
    //        WHERE access_token IS NOT NULL AND token_expiry > NOW()
    //        ORDER BY token_expiry DESC LIMIT 1
}
```

Caller defaults region to `"us-east-1"` if None (matching `user_kiro.rs:338` fallback).

### 1b. `get_any_valid_copilot_token()`

```rust
pub async fn get_any_valid_copilot_token(&self) -> Result<Option<CopilotTokenRow>> {
    // Query: SELECT ... FROM user_copilot_tokens
    //        WHERE copilot_token IS NOT NULL
    //          AND (expires_at IS NULL OR expires_at > NOW())
    //        ORDER BY expires_at DESC NULLS LAST LIMIT 1
}
```

Replaces the broken `get_expiring_copilot_tokens()` usage that only finds tokens expiring within 5 minutes.

### 1c. `get_any_user_provider_credential()` — already exists

Added during debug session. Returns `Option<(String, Option<String>)>` — (access_token, base_url) from `user_provider_tokens` for any user. Just remove the `dead_code` warning.

## Wave 2: Fix Populate Logic

### 2a. Add `fetch_kiro_models_with_token()` in `model_registry.rs`

Extract the API call from `fetch_kiro_models()` into a function taking raw `(access_token, region)` instead of `&AuthManager`. Refactor `fetch_kiro_models()` to delegate to it.

### 2b. Update `populate_provider()` in `model_registry.rs`

For each provider, add user-token fallback after admin/global check:

**Kiro:**
1. Try `auth_manager` (global) → `fetch_kiro_models()`
2. Fallback: `db.get_any_valid_kiro_credential()` → `fetch_kiro_models_with_token()`

**Anthropic:**
1. Try `get_admin_pool_credential(db, "anthropic")`
2. Fallback: `db.get_any_user_provider_credential("anthropic")` → `fetch_anthropic_models()`

**OpenAI Codex:**
1. Try `get_admin_pool_credential(db, "openai_codex")`
2. Fallback: `db.get_any_user_provider_credential("openai_codex")` → `fetch_openai_compatible_models()`

**Copilot:**
1. Replace `get_expiring_copilot_tokens()` with `get_any_valid_copilot_token()`

Add `tracing::debug!` at each fallback step for diagnosability.

### 2c. Simplify `populate_models` handler in `model_registry_handlers.rs`

Remove the special-case Kiro branching (lines 191-229). Route all providers through `populate_provider()`:

```rust
for provider_id in &providers {
    let am = if *provider_id == "kiro" {
        let guard = state.auth_manager.read().await;
        if guard.has_credentials().await { Some(guard) } else { None }
    } else { None };

    let result = populate_provider(provider_id, &db, &state.http_client, am.as_deref())
        .await.map_err(|e| e.to_string());
    // ... existing match
}
```

### 2d. Cleanup

- Remove `#[allow(dead_code)]` from `get_any_user_provider_credential()`
- Delete `populate_provider_with_key()` — its functionality is subsumed by the new fallback logic in `populate_provider()`

## Verification

```bash
cd backend && cargo fmt
cd backend && cargo clippy --all-targets    # zero warnings
cd backend && cargo test --lib              # zero failures
# Manual E2E: POST /_ui/api/models/registry/populate → models_upserted > 0
```

## Branch

`fix/populate-use-connected-tokens`
