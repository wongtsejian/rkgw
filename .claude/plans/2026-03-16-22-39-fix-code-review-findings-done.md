# Plan: Fix All Code Review Findings for Slim Proxy Mode

## Context

PR #123 (`feat/slim-proxy-mode`) re-adds proxy-only deployment mode. A 3-dimension code review (Security, Performance, Architecture) found 18 issues: 5 HIGH, 6 MEDIUM, 5 LOW, 2 INFO. This plan addresses all actionable findings (16 fixes, 2 INFO items need no action).

## Fixes by File

### 1. `backend/src/config.rs` — CR-004, CR-006, CR-013, CR-016

**CR-004 (HIGH): Add explicit `GATEWAY_MODE` env var**
- Add field `gateway_mode: GatewayMode` (enum: `Full`, `Proxy`)
- Read `GATEWAY_MODE` env var in `Config::load()` — default `Full`, set `Proxy` when `GATEWAY_MODE=proxy`
- Change `is_proxy_only()` to check `self.gateway_mode == GatewayMode::Proxy`
- In `validate()`: when proxy mode, require `PROXY_API_KEY` is set (not just present)
- Update existing tests

**CR-006 (MEDIUM): Custom Debug impl redacting secrets**
- Remove `#[derive(Debug)]`, keep `#[derive(Clone)]`
- Implement `fmt::Debug` manually, redacting `proxy_api_key`, `kiro_refresh_token`, `kiro_client_id`, `kiro_client_secret`, `google_client_secret`

**CR-013 (LOW): Minimum key length validation**
- In `validate()` proxy branch, enforce `proxy_api_key.len() >= 16`

**CR-016 (LOW): Group proxy fields into nested struct**
- Create `ProxyConfig { api_key, kiro_refresh_token, kiro_client_id, kiro_client_secret, kiro_sso_region }`
- Replace 5 `Option<String>` fields with `proxy: Option<ProxyConfig>`
- `is_proxy_only()` → `self.gateway_mode == GatewayMode::Proxy`

### 2. `backend/src/routes/state.rs` — CR-003, CR-014

**CR-003 (HIGH): Move proxy key hash to AppState**
- Add field `proxy_api_key_hash: Option<[u8; 32]>` — SHA-256 of proxy key, set once at startup
- In `main.rs` after config load: compute hash if proxy mode, store on AppState
- Middleware checks `state.proxy_api_key_hash` directly — zero lock, zero clone

**CR-014 (LOW): Dedicated proxy user UUID constant**
- Add `pub const PROXY_USER_ID: Uuid = Uuid::from_u128(0x0000_0001_0000_0000_0000_000000000001);`
- Use instead of `Uuid::nil()`

### 3. `backend/src/middleware/mod.rs` — CR-002, CR-003, CR-010, CR-012, CR-015

**CR-003 (HIGH): Lockless proxy auth fast path**
- Replace RwLock-based config read with `state.proxy_api_key_hash` check
- No lock needed — `Option<[u8; 32]>` is `Copy`

**CR-002 (HIGH): Generic error message**
- Change `.map_err(|e| ApiError::AuthError(format!("Proxy auth failed: {}", e)))` to:
  ```rust
  .map_err(|e| {
      tracing::error!(error = %e, "Proxy auth token refresh failed");
      ApiError::AuthError("Proxy authentication unavailable".to_string())
  })
  ```

**CR-010 (MEDIUM): Don't hold auth lock across await**
- Extract access_token and region, then drop guard:
  ```rust
  let (access_token, region) = {
      let auth = state.auth_manager.read().await;
      let token = auth.get_access_token().await?;
      let region = auth.get_region().await;
      (token, region)
  }; // guard dropped here
  ```

**CR-012 (LOW): Hash-based constant-time comparison**
- Hash incoming key with SHA-256, compare against pre-computed hash:
  ```rust
  let incoming_hash = Sha256::digest(raw_key.as_bytes());
  if !bool::from(incoming_hash.ct_eq(expected_hash)) { ... }
  ```
- Normalizes length — no timing leak

**CR-015 (LOW): Document empty refresh_token**
- Add comment: `// Empty — proxy mode uses global AuthManager for refresh`

### 4. `backend/src/auth/manager.rs` — CR-008, CR-011

**CR-008 (MEDIUM): Dedup bootstrap with get_access_token**
- Remove `bootstrap_proxy_credentials()` body
- Reimplement as:
  ```rust
  pub async fn bootstrap_proxy_credentials(&self) -> Result<()> {
      self.get_access_token().await?;
      tracing::info!("Proxy-only credentials bootstrapped successfully");
      Ok(())
  }
  ```
- `get_access_token()` already handles "token missing/expired → refresh" logic

**CR-011 (MEDIUM): Thundering herd double-check**
- In `refresh_token()`, after acquiring write lock on credentials, re-check if token is still expiring:
  ```rust
  async fn refresh_token(&self) -> Result<()> {
      // Double-check: another task may have refreshed while we waited for the lock
      if !self.is_token_expiring_soon().await {
          return Ok(());
      }
      let mut creds = self.credentials.write().await;
      // ... existing refresh logic
  }
  ```

### 5. `backend/src/main.rs` — CR-004, CR-005

**CR-004 (HIGH): Use gateway_mode from config**
- Change `let is_proxy_only = config.is_proxy_only();` (already works with updated method)

**CR-005 (HIGH): Conditionally mount web UI routes**
- In `build_app()`, pass `is_proxy_only` and conditionally skip web UI:
  ```rust
  fn build_app(state: routes::AppState, is_proxy_only: bool) -> Router {
      let mut app = Router::new()
          .merge(health_routes)
          .merge(openai_routes)
          .merge(anthropic_routes);
      if !is_proxy_only {
          app = app.merge(web_ui::web_ui_routes(state.clone()));
      }
      app.layer(middleware::cors_layer())
      // ... rest unchanged
  }
  ```
- Update call site to pass `is_proxy_only`

### 6. `backend/entrypoint.sh` — CR-001, CR-007, CR-009

**CR-001 (HIGH): Safe JSON construction with jq**
- Replace heredoc in `save_tokens()`:
  ```sh
  save_tokens() {
      mkdir -p "$(dirname "$TOKEN_CACHE")"
      (umask 077 && jq -n --arg rt "$1" --arg ci "$2" --arg cs "$3" \
          '{refresh_token: $rt, client_id: $ci, client_secret: $cs}' > "$TOKEN_CACHE")
  }
  ```
- Also fixes CR-007 (umask before write)

**CR-009 (MEDIUM): Add `set -u` and defensive quoting**
- Change `set -e` to `set -eu`
- Use `${VAR:-}` for optional vars: `KIRO_SSO_URL`, `KIRO_SSO_REGION`

### 7. `docker-compose.gateway.yml` — CR-004

**CR-004**: Add `GATEWAY_MODE: "proxy"` to environment section.

### 8. `.env.proxy.example` — CR-004

Add `GATEWAY_MODE=proxy` as required var.

## Files Modified

| File | Findings Fixed |
|------|---------------|
| `backend/src/config.rs` | CR-004, CR-006, CR-013, CR-016 |
| `backend/src/routes/state.rs` | CR-003, CR-014 |
| `backend/src/middleware/mod.rs` | CR-002, CR-003, CR-010, CR-012, CR-015 |
| `backend/src/auth/manager.rs` | CR-008, CR-011 |
| `backend/src/main.rs` | CR-004, CR-005 |
| `backend/entrypoint.sh` | CR-001, CR-007, CR-009 |
| `docker-compose.gateway.yml` | CR-004 |
| `.env.proxy.example` | CR-004 |
| `.env.example` | CR-004 |

## Agent Assignment

- **rust-backend-engineer**: All backend Rust files (config, state, middleware, auth, main)
- **devops-engineer**: entrypoint.sh, docker-compose.gateway.yml, .env files

## Verification

1. `cargo clippy --all-targets` — zero warnings
2. `cargo test --lib` — all existing + new tests pass
3. `cargo fmt --check` — no diffs
4. `docker compose -f docker-compose.gateway.yml config --quiet` — valid compose
