# Plan: Multi-Account Support Per Provider + Rate-Limit-Aware Load Balancing

## Context

Currently each user can connect **one account per provider** (`UNIQUE(user_id, provider_id)` on `user_provider_tokens`). This limits throughput to a single API key's rate limits. Users need:
- **Rate limit distribution** — spread requests across multiple API keys
- **Cost optimization** — use cheaper accounts first, overflow to premium
- **Failover** — auto-switch on 429/5xx errors

Scope: **Full deployment mode only** (requires database). Both per-user multi-account AND admin-managed shared pool.

---

## Wave 1: Database + Core Types

### Migration v20

**Modify `user_provider_tokens`:**
```sql
ALTER TABLE user_provider_tokens
  DROP CONSTRAINT user_provider_tokens_user_id_provider_id_key;
ALTER TABLE user_provider_tokens
  ADD COLUMN account_label TEXT NOT NULL DEFAULT 'default';
ALTER TABLE user_provider_tokens
  ADD CONSTRAINT user_provider_tokens_user_provider_label_unique
  UNIQUE (user_id, provider_id, account_label);
```

**New `admin_provider_pool` table:**
```sql
CREATE TABLE admin_provider_pool (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id   TEXT NOT NULL,
    account_label TEXT NOT NULL DEFAULT 'pool-1',
    api_key       TEXT NOT NULL,
    key_prefix    TEXT NOT NULL DEFAULT '',
    base_url      TEXT,
    enabled       BOOLEAN NOT NULL DEFAULT true,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (provider_id, account_label)
);
```

### New module: `backend/src/providers/rate_limiter.rs`

```rust
pub struct AccountId {
    pub user_id: Option<Uuid>,    // None = admin pool account
    pub provider_id: ProviderId,
    pub account_label: String,
}

pub struct RateLimitState {
    pub requests_remaining: Option<u64>,
    pub tokens_remaining: Option<u64>,
    pub reset_at: Option<Instant>,
    pub limited_until: Option<Instant>,
    pub updated_at: Instant,
}

pub struct RateLimitTracker {
    states: DashMap<AccountId, RateLimitState>,
    round_robin: AtomicU64,  // tiebreaker when scores are equal
}
```

Key methods:
- `update_from_headers(account_id, headers)` — parse provider-specific headers
- `mark_limited(account_id, retry_after)` — mark 429'd account with cooldown
- `best_account(candidates) -> Option<AccountId>` — highest headroom wins
- `cleanup_stale(max_age)` — evict entries >10min old

**Header mapping per provider:**

| Provider | Requests Remaining | Tokens Remaining | Retry-After |
|----------|-------------------|-----------------|-------------|
| Anthropic | `anthropic-ratelimit-requests-remaining` | `anthropic-ratelimit-tokens-remaining` | `retry-after` |
| OpenAI | `x-ratelimit-remaining-requests` | `x-ratelimit-remaining-tokens` | `retry-after` |
| Copilot | `x-ratelimit-remaining` | — | `retry-after` |

### Type changes

**`types.rs`**: Add `account_id: AccountId` to `ProviderCredentials`

**`state.rs`**: Add `rate_tracker: Arc<RateLimitTracker>` to `AppState`

**`config_db.rs`**: New CRUD methods:
- `get_all_user_provider_tokens(user_id, provider_id) -> Vec<ProviderTokenRow>`
- `upsert_user_provider_token_labeled(user_id, provider_id, account_label, ...)`
- `delete_user_provider_token_labeled(user_id, provider_id, account_label)`
- `get_admin_pool_accounts(provider_id) -> Vec<AdminPoolRow>`
- `upsert_admin_pool_account(provider_id, account_label, api_key, ...)`
- `delete_admin_pool_account(id)`
- `set_admin_pool_account_enabled(id, enabled)`

### Files (Wave 1)

| File | Action | Owner |
|------|--------|-------|
| `backend/src/web_ui/config_db.rs` | Modify (migration v20 + CRUD) | database-engineer |
| `backend/src/providers/rate_limiter.rs` | **Create** | rust-backend-engineer |
| `backend/src/providers/types.rs` | Modify (add AccountId) | rust-backend-engineer |
| `backend/src/providers/mod.rs` | Modify (pub mod rate_limiter) | rust-backend-engineer |
| `backend/src/routes/state.rs` | Modify (add rate_tracker) | rust-backend-engineer |

---

## Wave 2: Provider Header Capture + Multi-Account Registry

### Provider trait change

`stream_openai` / `stream_anthropic` return type changes from `Stream` to `(HeaderMap, Stream)` so rate limit headers from the initial response are accessible.

Each provider implementation captures `response.headers().clone()` before consuming the response body.

### Registry rewrite (`registry.rs`)

`resolve_provider()` becomes `resolve_provider_with_balancing()`:

1. Determine target provider from model name (existing logic)
2. Load **all** user accounts for that provider (not just one)
3. Load admin pool accounts for that provider (fallback)
4. Call `rate_tracker.best_account(candidates)` — pick highest headroom
5. Return selected account's credentials

Cache changes: `HashMap<String, ProviderCredentials>` → `HashMap<String, Vec<ProviderCredentials>>`

Token refresh lock key: `(user_id, provider)` → `(user_id, provider, account_label)`

### Files (Wave 2)

| File | Action | Owner |
|------|--------|-------|
| `backend/src/providers/traits.rs` | Modify (streaming return type) | rust-backend-engineer |
| `backend/src/providers/anthropic.rs` | Modify (capture headers) | rust-backend-engineer |
| `backend/src/providers/openai_codex.rs` | Modify (capture headers) | rust-backend-engineer |
| `backend/src/providers/copilot.rs` | Modify (capture headers) | rust-backend-engineer |
| `backend/src/providers/qwen.rs` | Modify (capture headers) | rust-backend-engineer |
| `backend/src/providers/kiro.rs` | Modify (capture headers) | rust-backend-engineer |
| `backend/src/providers/registry.rs` | Modify (multi-account resolution) | rust-backend-engineer |

---

## Wave 3: Routing Failover

### Failover loop in route handlers

**Non-streaming** (`execute_openai`/`execute_anthropic`):
```
for attempt in 0..3:
    creds = resolve_with_balancing(...)
    result = provider.execute(ctx, req)
    if result.status == 429:
        tracker.mark_limited(creds.account_id, retry_after)
        continue  // next attempt skips this account
    update_rate_limits(tracker, creds.account_id, headers)
    return result
return Err(RateLimited("All accounts exhausted"))
```

**Streaming** (`stream_openai`/`stream_anthropic`):
- Same failover loop for stream **initiation** (pre-first-byte)
- Once stream starts flowing → sticky to that account, no mid-stream switching

### Files (Wave 3)

| File | Action | Owner |
|------|--------|-------|
| `backend/src/routes/pipeline.rs` | Modify (balanced routing + rate limit update) | rust-backend-engineer |
| `backend/src/routes/openai.rs` | Modify (failover loop) | rust-backend-engineer |
| `backend/src/routes/anthropic.rs` | Modify (failover loop) | rust-backend-engineer |
| `backend/src/error.rs` | Modify (add RateLimited variant) | rust-backend-engineer |

---

## Wave 4: Web UI API + Admin Pool

### New endpoints

**Admin pool management** (admin-only + CSRF):
```
GET    /_ui/api/admin/pool           — list all pool accounts
POST   /_ui/api/admin/pool           — add pool account (api_key, provider_id, label)
DELETE /_ui/api/admin/pool/:id       — remove pool account
PATCH  /_ui/api/admin/pool/:id/toggle — enable/disable
```

**User account management** (session-authenticated):
```
GET    /_ui/api/providers/:provider/accounts       — list user's accounts
DELETE /_ui/api/providers/:provider/accounts/:label — remove specific account
```

**Monitoring**:
```
GET    /_ui/api/providers/rate-limits — current rate limit state
```

**OAuth change**: OAuth callback accepts optional `account_label` query param (defaults to `"default"`).

### Files (Wave 4)

| File | Action | Owner |
|------|--------|-------|
| `backend/src/web_ui/admin_pool.rs` | **Create** | rust-backend-engineer |
| `backend/src/web_ui/provider_oauth.rs` | Modify (account_label param) | rust-backend-engineer |
| `backend/src/web_ui/mod.rs` | Modify (wire new routes) | rust-backend-engineer |

---

## Wave 5: Frontend

- Extend provider settings to show multiple accounts per provider
- "Connect Another Account" button with label input
- Admin pool management section in Admin page
- Rate-limit status display per account
- Update `api.ts` with new endpoints

### Files (Wave 5)

| File | Action | Owner |
|------|--------|-------|
| `frontend/src/lib/api.ts` | Modify (new endpoints) | react-frontend-engineer |
| `frontend/src/pages/Providers.tsx` | Modify (multi-account UI) | react-frontend-engineer |
| `frontend/src/pages/Admin.tsx` | Modify (pool management) | react-frontend-engineer |

---

## Wave 6: Testing

- Unit tests for `RateLimitTracker` (scoring, marking, cleanup, header parsing)
- Unit tests for multi-account `ProviderRegistry` resolution
- Unit tests for failover loop logic
- Integration tests for admin pool CRUD + migration v20
- E2E tests for multi-account connect flow

---

## Account Selection Algorithm Summary

```
1. Infer provider from model name (claude-* → Anthropic, gpt-* → OpenAI, etc.)
2. Load user's accounts for that provider (may be 0..N)
3. Load admin pool accounts for that provider (fallback)
4. Candidates = user_accounts ++ pool_accounts (user preferred)
5. If empty, check Copilot accounts (universal provider)
6. If still empty → Kiro fallback
7. Score each candidate by rate-limit headroom:
   - Skip accounts with limited_until > now
   - Score = requests_remaining * 100 + tokens_remaining / 1000
   - No data = high default score (assume available)
   - Tiebreak: round-robin via atomic counter
8. Return best account's credentials
```

---

## Backward Compatibility

- Existing tokens get `account_label = 'default'` via migration default
- Existing `upsert_user_provider_token()` continues working (uses `'default'` label)
- Existing OAuth flows unchanged unless `account_label` param is explicitly passed
- Single-account users see no behavioral change

---

## Verification

```bash
# Backend quality gates
cd backend && cargo clippy --all-targets   # zero warnings
cd backend && cargo fmt --check            # no diffs
cd backend && cargo test --lib             # all tests pass

# Key test areas
cargo test --lib rate_limiter::            # rate limit tracker tests
cargo test --lib registry::               # multi-account resolution tests
cargo test --lib config_db::              # migration + CRUD tests
cargo test --lib admin_pool::             # pool management tests

# Frontend
cd frontend && npm run build && npm run lint

# Manual verification
# 1. Connect 2 Anthropic accounts → verify both listed in UI
# 2. Send requests → verify round-robin across accounts in logs
# 3. Rate-limit one account → verify failover to second
# 4. Admin adds pool account → verify users fall back to pool
# 5. Streaming request → verify sticky to one account for duration
```

---

## Recommended Preset

Waves 1-4: `/team-implement --preset backend-feature` (3 agents: rust-backend-engineer, database-engineer, backend-qa)

Wave 5: `/team-implement --preset frontend-feature` (2 agents: react-frontend-engineer, frontend-qa)

Total estimated complexity: **Large** (20+ files, new module, schema migration, trait changes)
