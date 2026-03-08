# copilot-provider-support_20260308: Add GitHub Copilot Provider Support

**Type**: feature
**Created**: 2026-03-08
**Preset**: fullstack
**Services**: backend, frontend, infra, backend-qa, frontend-qa

## Problem Statement

Add GitHub Copilot as a provider to the rkgw gateway. Copilot differs from the existing API-key-based providers (Anthropic, OpenAI, Gemini) in three ways: browser-based GitHub OAuth redirect authentication (not API key paste), short-lived Copilot bearer tokens refreshed from a long-lived GitHub OAuth token (two-layer token system), and multi-vendor model access (a single Copilot connection provides GPT-4.1, Claude Sonnet 4, Gemini Flash, etc.).

This also introduces user-configurable provider priority to handle routing conflicts when multiple providers serve the same model.

## User Story

As a gateway user with a GitHub Copilot subscription, I want to route API requests through Copilot so that I can use Copilot-hosted models (GPT-4.1, Claude Sonnet 4, Gemini Flash) via the same OpenAI/Anthropic-compatible endpoints.

## Acceptance Criteria

1. GitHub OAuth connect/disconnect flow works end-to-end from Profile page
2. Copilot bearer tokens auto-refresh in background before expiry
3. API requests route through Copilot when selected by provider priority
4. Provider status endpoint includes Copilot connection state and model list
5. CopilotSetup UI card shows status, username, plan type with CRT aesthetic
6. Backward compatible — no `GITHUB_COPILOT_CLIENT_ID` means Copilot is hidden
7. All existing tests pass, new unit tests cover Copilot-specific logic

## Scope Boundaries

**In scope:**
- Database migration v9 (user_copilot_tokens + user_provider_priority tables)
- ProviderId::Copilot enum variant and type updates
- Config fields for GitHub OAuth App credentials
- copilot_auth.rs: OAuth flow, status, disconnect, background token refresh
- copilot.rs: Provider trait implementation (OpenAI-compatible pass-through)
- copilot_token_cache DashMap in AppState
- Registry integration (load_user_tokens + resolve_provider)
- Provider status + priority backend endpoints
- CopilotSetup.tsx component + Profile page integration
- Docker-compose env var passthrough
- Unit tests for all new backend logic
- E2E tests for Copilot UI flow

**Out of scope:**
- Copilot in proxy-only mode (requires DB for token storage)
- GitHub Enterprise Server (only github.com OAuth)
- Copilot model list auto-discovery (static list for now)
- Provider priority UI (backend endpoints only; frontend drag-and-drop deferred)

## Dependencies

- Multi-provider architecture (provider trait, registry, routing dispatch) — already completed in multi-provider-support_20260307 track. No blocking dependencies.

## Key Technical Decisions

- Standalone `copilot_auth.rs` module (two-layer tokens + server-side secret don't fit provider_oauth.rs PKCE relay pattern)
- Dedicated `user_copilot_tokens` table (different lifecycle from user_provider_tokens)
- Migration v9 (v8 already taken by user_provider_tokens)
- Reuse `oauth_pending` DashMap with `copilot:` prefix (browser redirect pattern matches Google SSO)
- Background refresh task every 2 min (Copilot tokens ~30 min TTL, can't use generic TokenExchanger)
- Copilot API headers hardcoded as constants (mimics VS Code client)
