# Fix: Copilot shows EXPIRED when token was never obtained

## Context

After completing the GitHub device flow, the UI shows `[WARN] EXPIRED` when the GitHub account lacks Copilot API token access (403 `feature_flag_blocked`). The backend treats the Copilot token fetch as non-fatal, stores a GitHub-only row, and the status endpoint defaults `None` expiry to `expired = true`. The frontend always shows a success toast and then renders "EXPIRED" — confusing users into thinking a valid token expired, when in reality no Copilot token was ever obtained.

## Changes

### 1. Backend — `backend/src/web_ui/copilot_auth.rs`

**Add `has_copilot_token` to `CopilotStatusResponse` (line 60-65):**
```rust
pub struct CopilotStatusResponse {
    pub connected: bool,
    pub github_username: Option<String>,
    pub copilot_plan: Option<String>,
    pub has_copilot_token: bool,  // NEW
    pub expired: bool,
}
```

**Fix `copilot_status` handler (line 427-436):**
- Change `unwrap_or(true)` → `unwrap_or(false)` — missing expiry means "not expired", not "expired"
- Only mark `expired = true` when there IS an expiry timestamp AND it's in the past
- Populate new `has_copilot_token` from `r.copilot_token.is_some()`

```rust
Some(r) => {
    let has_copilot_token = r.copilot_token.is_some();
    let expired = r.expires_at.map(|exp| exp < Utc::now()).unwrap_or(false);
    Ok(Json(CopilotStatusResponse {
        connected: true,
        github_username: r.github_username,
        copilot_plan: r.copilot_plan,
        has_copilot_token,
        expired,
    }))
}
None => Ok(Json(CopilotStatusResponse {
    connected: false,
    github_username: None,
    copilot_plan: None,
    has_copilot_token: false,
    expired: false,
})),
```

**Update 3 existing tests** (lines 701-786) to include `has_copilot_token` field.

### 2. Frontend API type — `frontend/src/lib/api.ts`

**Add field to `CopilotStatus` (line 430-435):**
```typescript
export interface CopilotStatus {
  connected: boolean;
  github_username: string | null;
  copilot_plan: string | null;
  has_copilot_token: boolean;  // NEW
  expired: boolean;
}
```

### 3. Frontend — `frontend/src/components/CopilotSetup.tsx`

**Update status tags (lines 87-95) to show 3 states:**
- `CONNECTED` — `connected && has_copilot_token && !expired`
- `EXPIRED` — `connected && has_copilot_token && expired`
- `NO COPILOT ACCESS` — `connected && !has_copilot_token`
- `NOT CONNECTED` — `!connected`

**Update `handleComplete` (line 35-38):** Accept the poll response message and show it as the toast, so the "GitHub connected but Copilot not available" message surfaces to the user.

### 4. Frontend — `frontend/src/components/DeviceCodeDisplay.tsx`

**Pass message to onComplete (line 49-50):**
Change `done()` → `done(result.message)` on success status. Update the `onComplete` prop type to accept an optional message string.

### 5. Frontend — `frontend/src/pages/Providers.tsx`

**Update `copilotConnected` logic (line 127):**
```typescript
// Before:
.then((s) => setCopilotConnected(s.connected && !s.expired))
// After:
.then((s) => setCopilotConnected(s.connected && s.has_copilot_token && !s.expired))
```

This fixes the Status tab showing "Connected" for GitHub-only connections.

## Files Modified

| File | Change |
|------|--------|
| `backend/src/web_ui/copilot_auth.rs` | Add `has_copilot_token` field, fix `unwrap_or`, update tests |
| `frontend/src/lib/api.ts` | Add `has_copilot_token` to `CopilotStatus` |
| `frontend/src/components/CopilotSetup.tsx` | 3 status states, use poll message in toast |
| `frontend/src/components/DeviceCodeDisplay.tsx` | Pass message to `onComplete` callback |
| `frontend/src/pages/Providers.tsx` | Update `copilotConnected` predicate |

## Verification

1. `cd backend && cargo clippy --all-targets` — zero warnings
2. `cd backend && cargo test --lib copilot_auth::` — all tests pass
3. `cd frontend && npm run build && npm run lint` — zero errors
4. Open `http://localhost:5173/_ui/providers` → Connections tab → Copilot card should show `NO COPILOT ACCESS` (not EXPIRED)
5. Status tab should show Copilot as "Offline" (correct — no usable token)
