# copilot-provider-support_20260308: Implementation Plan

**Status**: active
**Branch**: feat/copilot-provider-support

---

## Phase 1: Backend Foundation
Agent: rust-backend-engineer

- [x] 1.1 — Add `Copilot` variant to `ProviderId` enum in `providers/types.rs` (update `as_str()`, `Display`, `FromStr`, serde rename, unit tests)
- [x] 1.2 — Add `pub mod copilot;` to `providers/mod.rs` (stub file to avoid compile errors)
- [x] 1.3 — Add Copilot error variants to `error.rs` (`CopilotAuthError(String)` → 502, `CopilotTokenExpired` → 403)
- [x] 1.4 — Add GitHub OAuth config fields to `config.rs` (`github_copilot_client_id`, `github_copilot_client_secret`, `github_copilot_callback_url`)
- [x] 1.5 — Add migration v9 to `config_db.rs`: `user_copilot_tokens` table, `user_provider_priority` table, update `model_routes` CHECK constraint
- [x] 1.6 — Add DB methods to `config_db.rs`: `upsert_copilot_tokens`, `get_copilot_tokens`, `delete_copilot_tokens`, `has_copilot_token`, `get_expiring_copilot_tokens`, `get_user_provider_priority`, `upsert_user_provider_priority`
- [x] 1.7 — Add `copilot_token_cache: Arc<DashMap<Uuid, (String, String, Instant)>>` to AppState in `routes/mod.rs`
- [x] 1.8 — Update `.env.example` with `GITHUB_COPILOT_CLIENT_ID`, `GITHUB_COPILOT_CLIENT_SECRET`, `GITHUB_COPILOT_CALLBACK_URL`

**Verification**: `cargo clippy && cargo test --lib && cargo fmt --check`

---

## Phase 2: Backend Auth & Token Refresh
Agent: rust-backend-engineer

- [x] 2.1 — Create `web_ui/copilot_auth.rs`: GitHub OAuth connect endpoint (`GET /_ui/api/copilot/connect`) — generate state, store in `oauth_pending` with `copilot:` prefix, 302 redirect to GitHub
- [x] 2.2 — Implement OAuth callback endpoint (`GET /_ui/api/copilot/callback`) — validate state, exchange code for GitHub token, fetch Copilot bearer token + account type, compute base_url, store via `upsert_copilot_tokens`, redirect to profile
- [x] 2.3 — Implement status endpoint (`GET /_ui/api/copilot/status`) — return connected, github_username, copilot_plan, expired
- [x] 2.4 — Implement disconnect endpoint (`DELETE /_ui/api/copilot/disconnect`) — delete tokens from DB, invalidate cache
- [x] 2.5 — Register copilot routes in `web_ui/mod.rs` (`pub mod copilot_auth;` + merge `copilot_routes()` into session routes)
- [x] 2.6 — Implement `spawn_copilot_token_refresh_task()` — background task every 2 min, refresh expiring tokens via `copilot_internal/v2/token`, update DB + invalidate cache on success, null out tokens on failure
- [x] 2.7 — Spawn refresh task in `main.rs` (non-proxy-only mode with DB)

**Verification**: `cargo clippy && cargo test --lib && cargo fmt --check`

---

## Phase 3: Backend Provider & Registry Integration
Agent: rust-backend-engineer

- [x] 3.1 — Implement `CopilotProvider` in `providers/copilot.rs`: `Provider` trait with OpenAI-compatible pass-through, Copilot-specific request headers, vision detection, model name normalization
- [x] 3.2 — Extend `registry.rs:load_user_tokens()` to query `user_copilot_tokens` alongside `user_provider_tokens`
- [x] 3.3 — Update `resolve_provider()` to handle `ProviderId::Copilot` — check `copilot_token_cache`, fallback to DB query, build `ProviderCredentials`
- [x] 3.4 — Initialize `CopilotProvider` in `main.rs` and wire into provider dispatch
- [x] 3.5 — Extend `providers_status()` in `provider_oauth.rs` to include Copilot connection status + static model list
- [x] 3.6 — Guard `disconnect_provider()` in `provider_oauth.rs` to reject Copilot (redirect to dedicated endpoint)
- [x] 3.7 — Add provider priority endpoints: `GET /_ui/api/providers/priority` and `POST /_ui/api/providers/priority`
- [x] 3.8 — Integrate priority into `resolve_provider()` — when multiple providers serve a model, pick lowest priority number

**Verification**: `cargo clippy && cargo test --lib && cargo fmt --check`

---

## Phase 4: Infrastructure
Agent: devops-engineer

- [x] 4.1 — Add `GITHUB_COPILOT_CLIENT_ID`, `GITHUB_COPILOT_CLIENT_SECRET`, `GITHUB_COPILOT_CALLBACK_URL` env var passthrough to `docker-compose.yml` backend service
- [x] 4.2 — Add same env var passthrough to `docker-compose.gateway.yml` backend/gateway service

**Verification**: `docker compose config` (validates YAML syntax)

---

## Phase 5: Frontend
Agent: react-frontend-engineer

- [x] 5.1 — Add `CopilotStatus` interface and `getCopilotStatus()`, `disconnectCopilot()` functions to `src/lib/api.ts`
- [x] 5.2 — Create `src/components/CopilotSetup.tsx` — status badge (CONNECTED/NOT CONNECTED/EXPIRED), GitHub username + plan display, connect button (full browser redirect), disconnect button, URL param feedback on mount
- [x] 5.3 — Integrate `CopilotSetup` into `src/pages/Profile.tsx` with "GITHUB COPILOT" section header
- [x] 5.4 — Style CopilotSetup with CRT terminal aesthetic using existing CSS variables and component patterns

**Verification**: `cd frontend && npm run lint && npm run build`

---

## Phase 6: QA
Agents: backend-qa, frontend-qa

- [x] 6.1 — Backend: Unit tests for `ProviderId::Copilot` serialization/deserialization round-trips
- [x] 6.2 — Backend: Unit tests for `CopilotProvider` — header construction, vision detection, model name normalization
- [x] 6.3 — Backend: Unit tests for copilot_auth endpoints — connect redirect, callback state validation, status response, disconnect
- [x] 6.4 — Backend: Unit tests for provider priority resolution logic
- [x] 6.5 — Frontend: Playwright E2E test for CopilotSetup component render and disconnect flow (mocked API)
- [x] 6.6 — Frontend: Playwright E2E test for Profile page Copilot section visibility

**Verification**: `cargo test --lib && cd e2e-tests && npm run test:ui`
