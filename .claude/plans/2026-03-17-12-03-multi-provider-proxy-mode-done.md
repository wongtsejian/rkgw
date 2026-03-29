# Plan: Multi-Provider Support for Proxy Mode

## Context

Proxy-only mode (`GATEWAY_MODE=proxy`) currently only works with Kiro. All 5 provider implementations exist but are inaccessible in proxy mode because credential resolution requires a database (`config_db`), which proxy mode doesn't have.

There are **two early-return points** in `ProviderRegistry.resolve_provider()` (registry.rs) that block non-Kiro providers:
1. **Line 234:** `user_id` is `None` → returns `(Kiro, None)` immediately
2. **Line 258:** `db` is `None` → returns `(Kiro, None)` after cache miss

In proxy mode, the middleware injects `UserKiroCreds` with `PROXY_USER_ID` (sentinel UUID), so `user_id` is typically `Some(PROXY_USER_ID)` — meaning line 258 (db=None) is the primary blocker. However, if Kiro credentials are not configured (user only wants Anthropic/OpenAI), `user_creds` may be `None`, hitting line 234 instead. Both paths need proxy credential fallback.

**Goal:** Enable all existing providers (Anthropic, OpenAI, Copilot, Qwen) plus a new generic OpenAI-compatible provider in proxy mode, configured entirely via environment variables.

## Consultation Summary

- **Proxy mode architecture:** Single-container, no DB, no Web UI. Auth via single `PROXY_API_KEY` SHA-256 hash comparison. Kiro credentials from `AuthManager` which reads env vars. `PROXY_USER_ID` sentinel defined in `routes/state.rs`.
- **Provider architecture:** 5 providers built at startup via `build_provider_map()` in `providers/mod.rs`. Each implements the `Provider` trait with both OpenAI and Anthropic interfaces. Model routing uses `provider_for_model()` prefix matching (claude-* → Anthropic, gpt-* → OpenAI, qwen-* → Qwen).
- **Pipeline routing (NEW):** Request routing now goes through `resolve_provider_routing()` in `routes/pipeline.rs`, which dispatches to either `resolve_provider_with_balancing()` (when config_db exists) or `resolve_provider()` (proxy-only mode). Returns a `ProviderRouting` struct with provider_id, credentials, stripped_model, and account_id.
- **Multi-account load balancing (NEW):** Commit 25eda91a added `resolve_provider_with_balancing()` and `RateLimitTracker` for rate-limit-aware account selection. This only activates when `config_db` is available, so it does not affect proxy mode.
- **AppState location (CHANGED):** AppState is now in `routes/state.rs` (was `routes/mod.rs`). New fields: `provider_registry`, `providers`, `rate_tracker`, `token_exchanger`, `provider_oauth_pending`.
- **Credential types:** Anthropic/OpenAI use static API keys. Copilot/Qwen use OAuth tokens with expiry. Kiro uses AWS STS tokens with refresh.

## Design: ProxyCredentialStore

Add a `proxy_credentials` field to `ProviderRegistry` — a `HashMap<ProviderId, ProviderCredentials>` populated at startup from env vars. Modify `resolve_provider()` to check this store at **both** early-return points (user_id=None and db=None) before falling back to Kiro.

This is the minimal-change approach: the existing routing logic (`parse_prefixed_model`, `provider_for_model`, `pick_best_provider`) stays unchanged. The pipeline routing in `routes/pipeline.rs` also stays unchanged — it already calls `resolve_provider()` in the proxy-only branch. Only the credential lookup path in `registry.rs` gets a proxy fallback.

## New Environment Variables

| Variable | Provider | Required | Example |
|----------|----------|----------|---------|
| `ANTHROPIC_API_KEY` | Anthropic | No | `sk-ant-api03-...` |
| `OPENAI_API_KEY` | OpenAI | No | `sk-proj-...` |
| `OPENAI_BASE_URL` | OpenAI | No | `https://api.openai.com/v1` (default) |
| `COPILOT_TOKEN` | Copilot | No | OAuth token (or obtained via device flow) |
| `COPILOT_BASE_URL` | Copilot | No | `https://api.githubcopilot.com` (default) |
| `QWEN_TOKEN` | Qwen | No | OAuth token (or obtained via device flow) |
| `QWEN_BASE_URL` | Qwen | No | From OAuth response |
| `CUSTOM_PROVIDER_URL` | Custom | No | `http://localhost:11434/v1` |
| `CUSTOM_PROVIDER_KEY` | Custom | No | Optional API key |
| `CUSTOM_PROVIDER_MODELS` | Custom | No | `llama3,codellama,deepseek-r1` |

At least one provider must have credentials for proxy mode to start (Kiro via existing flow, or any of the above).

## File Manifest

| File | Action | Owner | Wave |
|------|--------|-------|------|
| `backend/src/providers/types.rs` | modify | rust-backend-engineer | 1 |
| `backend/src/providers/registry.rs` | modify | rust-backend-engineer | 1 |
| `backend/src/config.rs` | modify | rust-backend-engineer | 1 |
| `backend/src/providers/custom.rs` | create | rust-backend-engineer | 2 |
| `backend/src/providers/mod.rs` | modify | rust-backend-engineer | 2 |
| `backend/src/main.rs` | modify | rust-backend-engineer | 2 |
| `backend/src/routes/pipeline.rs` | modify | rust-backend-engineer | 2 |
| `backend/src/routes/openai.rs` | modify | rust-backend-engineer | 3 |
| `backend/src/cache.rs` | modify | rust-backend-engineer | 3 |
| `backend/entrypoint.sh` | modify | devops-engineer | 3 |
| `docker-compose.gateway.yml` | modify | devops-engineer | 4 |
| `.env.example` | modify | devops-engineer | 4 |
| `.env.proxy.example` | modify | devops-engineer | 4 |

## Wave 1: Core Infrastructure

### 1.1 Add `Custom` variant to `ProviderId`
**File:** `backend/src/providers/types.rs`
- Add `Custom` variant to `ProviderId` enum (currently has: Kiro, Anthropic, OpenAICodex, Copilot, Qwen)
- Add `#[serde(rename = "custom")]` attribute
- Add `"custom"` to `as_str()`, `Display`, `FromStr` implementations
- Update existing tests

### 1.2 Extend `ProxyConfig` with provider credentials
**File:** `backend/src/config.rs`
- Add fields to `ProxyConfig` (currently only has: `api_key`, `kiro_refresh_token`, `kiro_client_id`, `kiro_client_secret`, `kiro_sso_region`):
  ```rust
  pub anthropic_api_key: Option<String>,
  pub openai_api_key: Option<String>,
  pub openai_base_url: Option<String>,
  pub copilot_token: Option<String>,
  pub copilot_base_url: Option<String>,
  pub qwen_token: Option<String>,
  pub qwen_base_url: Option<String>,
  pub custom_provider_url: Option<String>,
  pub custom_provider_key: Option<String>,
  pub custom_provider_models: Option<String>,
  ```
- Read new env vars in `Config::load()` when building `ProxyConfig` (around line 204 where `PROXY_API_KEY` is read)

### 1.3 Add `ProxyCredentialStore` to `ProviderRegistry`
**File:** `backend/src/providers/registry.rs`
- Add fields to `ProviderRegistry`:
  ```rust
  proxy_credentials: Option<HashMap<ProviderId, ProviderCredentials>>,
  custom_models: HashSet<String>,
  ```
- Add constructor: `ProviderRegistry::new_with_proxy(creds, custom_models)`
- Modify `resolve_provider()` at **two** intercept points:

  **Intercept 1 — line 234 (user_id=None):**
  ```rust
  let Some(uid) = user_id else {
      // Proxy mode without Kiro creds: check proxy credential store
      return self.resolve_from_proxy_creds(model);
  };
  ```

  **Intercept 2 — line 258 (db=None):**
  ```rust
  let Some(db) = db else {
      // Proxy mode with Kiro creds but no DB: check proxy credential store
      return self.resolve_from_proxy_creds(model);
  };
  ```

  **New helper method:**
  ```rust
  fn resolve_from_proxy_creds(&self, model: &str) -> (ProviderId, Option<ProviderCredentials>) {
      let Some(ref proxy_creds) = self.proxy_credentials else {
          return (ProviderId::Kiro, None);
      };
      // Determine target provider from model name
      let native = if let Some((provider, _)) = Self::parse_prefixed_model(model) {
          provider
      } else if let Some(provider) = Self::provider_for_model(model) {
          provider
      } else if self.custom_models.contains(model) {
          ProviderId::Custom
      } else {
          return (ProviderId::Kiro, None);
      };
      // Look up proxy credentials for that provider
      if let Some(cred) = proxy_creds.get(&native) {
          (native, Some(cred.clone()))
      } else {
          (ProviderId::Kiro, None)
      }
  }
  ```

- Extend `provider_for_model()` to check `self.custom_models` set before returning `None`

## Wave 2: Provider Implementation

### 2.1 Create `CustomProvider`
**File:** `backend/src/providers/custom.rs` (new)
- Implements `Provider` trait (defined in `providers/traits.rs`)
- OpenAI-compatible HTTP proxy: forwards requests to `base_url` with optional `Authorization: Bearer` header
- `execute_openai` → direct passthrough to target URL
- `execute_anthropic` → convert Anthropic format → OpenAI → forward → convert back
- `stream_openai` → direct SSE passthrough
- `stream_anthropic` → convert + stream + convert back
- Uses its own `reqwest::Client` (pattern from `anthropic.rs`, `openai_codex.rs`)

### 2.2 Register Custom provider + wire proxy credentials
**File:** `backend/src/providers/mod.rs`
- Add `pub mod custom;`
- Add `ProviderId::Custom` to `build_provider_map()` (currently builds: Kiro, Anthropic, OpenAICodex, Copilot, Qwen)

**File:** `backend/src/main.rs` (around line 355 where AppState is constructed)
- After `Config::load()`, build proxy credential map from `ProxyConfig` fields:
  ```rust
  let provider_registry = if config.is_proxy_only() {
      let proxy = config.proxy.as_ref().unwrap();
      let mut proxy_creds = HashMap::new();
      let mut custom_models = HashSet::new();
      if let Some(ref key) = proxy.anthropic_api_key {
          proxy_creds.insert(ProviderId::Anthropic, ProviderCredentials {
              provider: ProviderId::Anthropic,
              access_token: key.clone(),
              base_url: None,
              account_label: "proxy".into(),
          });
      }
      // ... same for OpenAI (with base_url), Copilot, Qwen, Custom
      if let Some(ref models) = proxy.custom_provider_models {
          custom_models = models.split(',').map(|s| s.trim().to_string()).collect();
      }
      Arc::new(ProviderRegistry::new_with_proxy(proxy_creds, custom_models))
  } else {
      Arc::new(ProviderRegistry::new())
  };
  ```
- Replace `provider_registry: Arc::new(providers::registry::ProviderRegistry::new())` in AppState initialization with the above

### 2.3 Update pipeline routing for proxy mode without Kiro
**File:** `backend/src/routes/pipeline.rs`
- Currently, `resolve_provider_routing()` (line 30) derives `user_id` from `user_creds.map(|c| c.user_id)` — this is `None` when proxy mode has no Kiro credentials configured
- Modify to pass `Some(PROXY_USER_ID)` when in proxy mode with no user_creds, so the provider routing still works:
  ```rust
  let user_id = user_creds.map(|c| c.user_id).or_else(|| {
      // Proxy mode without Kiro: use sentinel UUID so registry can route
      if state.proxy_api_key_hash.is_some() {
          Some(super::state::PROXY_USER_ID)
      } else {
          None
      }
  });
  ```
- This ensures `resolve_provider()` reaches the db=None intercept (1.3) instead of the user_id=None intercept

## Wave 3: Models Endpoint + Entrypoint

### 3.1 Multi-provider `/v1/models`
**File:** `backend/src/routes/openai.rs` — `get_models_handler`
- Currently merges two sources: `model_cache.get_all_model_ids()` (Kiro) + `model_cache.get_all_registry_models()` (DB-backed direct providers)
- In proxy mode, the registry models source is empty (no DB). Add a third source — static known model lists based on configured proxy credentials:
  - Anthropic: `["claude-opus-4-6", "claude-sonnet-4-6", "claude-haiku-4-5"]` if `ANTHROPIC_API_KEY` set
  - OpenAI: `["gpt-4o", "gpt-4o-mini", "o3", "o4-mini"]` if `OPENAI_API_KEY` set
  - Copilot: static list if `COPILOT_TOKEN` set
  - Qwen: static list if `QWEN_TOKEN` set
  - Custom: models from `CUSTOM_PROVIDER_MODELS`
- Access configured providers from `ProviderRegistry.proxy_credentials` (add a `configured_proxy_providers()` method)

**File:** `backend/src/providers/known_models.rs` (new) or constants in `registry.rs`
- Define known model lists per provider as constants
- Keep them simple — users can always use explicit prefix notation (`anthropic/claude-opus-4-6`) for unlisted models

### 3.2 Copilot + Qwen device flows
**File:** `backend/entrypoint.sh`
- Add GitHub device flow for Copilot:
  1. POST to `https://github.com/login/device/code` with `client_id`
  2. Display user_code + verification_uri
  3. Poll `https://github.com/login/oauth/access_token` until authorized
  4. Exchange GitHub token for Copilot token via `https://api.github.com/copilot_internal/v2/token`
  5. Export `COPILOT_TOKEN` and `COPILOT_BASE_URL`
- Add Qwen device flow:
  1. POST device authorization to Qwen OAuth endpoint (client_id: `f0304373b74a44d2b584a3fb70ca9e56`)
  2. Poll for token
  3. Export `QWEN_TOKEN` and `QWEN_BASE_URL`
- Cache all tokens to `/data/tokens.json` (extend existing Kiro token cache)
- Skip device flows when tokens already provided via env vars

## Wave 4: Docker + Docs + Tests

### 4.1 Docker Compose
**File:** `docker-compose.gateway.yml`
- Add new env vars with `${VAR:-}` defaults for all provider credentials

### 4.2 Environment examples
**File:** `.env.example`
- Add all new proxy provider env vars with comments in a "Multi-Provider Proxy" section

**File:** `.env.proxy.example`
- Add placeholder env vars for all providers (currently only has Kiro vars)

### 4.3 Unit tests
**Files:** Tests within modified files
- `registry.rs`: Test `resolve_from_proxy_creds()` — model prefix routing with proxy credentials
- `registry.rs`: Test custom model routing via `custom_models` set
- `registry.rs`: Test proxy mode with user_id=None (no Kiro creds)
- `registry.rs`: Test proxy mode with user_id=PROXY_USER_ID, db=None (has Kiro creds)
- `config.rs`: Test new env var loading into ProxyConfig
- `types.rs`: Test `Custom` ProviderId variant (serde, Display, FromStr)
- `custom.rs`: Test request forwarding (mock HTTP with wiremock or similar)
- `pipeline.rs`: Test PROXY_USER_ID injection when user_creds is None

## Interface Contracts

### Proxy Credential Resolution Flow
```
Client Request → middleware (PROXY_API_KEY hash check)
  → pipeline.rs: resolve_provider_routing()
    → user_id = user_creds.map(c.user_id).or(PROXY_USER_ID if proxy mode)
    → config_db is None → use single-account path
    → ProviderRegistry.resolve_provider(Some(PROXY_USER_ID), model, db=None)
      → parse model prefix / infer provider from name / check custom_models
      → cache miss for PROXY_USER_ID
      → db is None → resolve_from_proxy_creds(model)
        → found in proxy_credentials? → return (provider, creds)
        → not found? → return (Kiro, None)
  → ProviderRouting { provider_id, provider_creds, stripped_model, account_id: None }
  → build_kiro_credentials() if Kiro, or use provider_creds directly
  → execute via Provider trait (unchanged)
```

### Model Routing Priority (proxy mode)
1. Explicit prefix: `anthropic/claude-opus-4-6` → Anthropic (if configured)
2. Model name prefix: `claude-*` → Anthropic, `gpt-*` → OpenAI, `qwen-*` → Qwen
3. Custom model list: model in `CUSTOM_PROVIDER_MODELS` → Custom
4. Fallback: Kiro (always available via AuthManager)

If resolved provider has no proxy credentials, fall back to Kiro.

## Verification

```bash
# 1. Build
cd backend && cargo clippy --all-targets   # zero warnings
cd backend && cargo fmt --check             # no diffs
cd backend && cargo test --lib              # all tests pass

# 2. Manual test with Anthropic
GATEWAY_MODE=proxy PROXY_API_KEY=test-key-1234567890 ANTHROPIC_API_KEY=sk-ant-... \
  cargo run

curl http://localhost:9999/v1/chat/completions \
  -H "Authorization: Bearer test-key-1234567890" \
  -d '{"model":"claude-sonnet-4-6","messages":[{"role":"user","content":"hello"}]}'

# 3. Manual test with OpenAI
OPENAI_API_KEY=sk-proj-... \
curl http://localhost:9999/v1/chat/completions \
  -H "Authorization: Bearer test-key-1234567890" \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"hello"}]}'

# 4. Manual test with Custom (Ollama)
CUSTOM_PROVIDER_URL=http://localhost:11434/v1 CUSTOM_PROVIDER_MODELS=llama3 \
curl http://localhost:9999/v1/chat/completions \
  -H "Authorization: Bearer test-key-1234567890" \
  -d '{"model":"llama3","messages":[{"role":"user","content":"hello"}]}'

# 5. Check /v1/models includes all configured providers
curl http://localhost:9999/v1/models \
  -H "Authorization: Bearer test-key-1234567890"

# 6. Docker
docker compose -f docker-compose.gateway.yml --env-file .env.proxy build
docker compose -f docker-compose.gateway.yml --env-file .env.proxy up -d
```

## Recommended Preset
`/team-implement --preset backend-feature`

Primary work is backend-only (Rust). DevOps engineer needed for entrypoint.sh and Docker changes.

## Changelog (vs. original plan)

Updated 2026-03-17 to reflect codebase changes from commits 25eda91a (#124 multi-account) and 3ca0c961 (#130 cache tokens):

1. **AppState moved** from `routes/mod.rs` to `routes/state.rs` — all references updated
2. **Pipeline routing** now goes through `routes/pipeline.rs` → `resolve_provider_routing()` — added as Wave 2 file, updated interface contracts
3. **Two intercept points** in `resolve_provider()` — line 234 (user_id=None) AND line 258 (db=None) both need proxy fallback; extracted to `resolve_from_proxy_creds()` helper
4. **PROXY_USER_ID injection** in pipeline.rs — ensures proxy mode without Kiro creds still routes correctly
5. **Port corrected** — server runs on 9999 (not 8000) per docker-compose config
6. **Multi-account balancing** (new feature) — does not affect proxy mode (requires config_db), no plan changes needed
7. **Rate limiter** — new `providers/rate_limiter.rs` — no plan changes needed (proxy mode uses account_id: None)
8. **Provider OAuth relay** — new infrastructure for browser-based OAuth — no plan changes needed
9. **Two-layer model cache** — `get_models_handler` now merges Kiro cache + registry cache; updated Wave 3.1 to add a third source for proxy mode
