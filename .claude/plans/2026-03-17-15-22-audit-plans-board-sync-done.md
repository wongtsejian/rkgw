# Plan: Audit Plans vs GitHub Project Board + Sync Statuses

## Context

31 plan files exist in `.claude/plans/`. The Harbangan Board (GH Project #3) has 23 items. Need to:
1. Ensure all plans have board representation
2. Fix board status mismatches (items marked "In progress" but GH issue is CLOSED)
3. Close resolved GAP tracking issues
4. Create board items for active plans missing from the board

## Audit Findings

### Board Status Corrections (2 items)

These GH issues are CLOSED but the board still shows "In progress":

| Issue | Title | Board Status | GH State | Action |
|-------|-------|-------------|----------|--------|
| #97 | [e2e-tests]: adjust E2E tests for provider config DB migration | In progress | CLOSED | → Done |
| #114 | [backend]: Implement tool_choice bidirectional mapping | In progress | CLOSED | → Done |

### GAP Issues Resolved by Closed Implementation Issues (3 items)

These tracking issues describe gaps that are now fully addressed:

| GAP Issue | Resolved By | Action |
|-----------|------------|--------|
| #106 GAP-001: tool_choice mapping | #114 (CLOSED) | Close #106, board → Done |
| #107 GAP-003: response_format | #115 (CLOSED) | Close #107, board → Done |
| #108 GAP-002: streaming events | #118 + #119 (both CLOSED) | Close #108, board → Done |

### Implementation Issues Likely Resolved (3 items — need verification)

These model-level issues were prerequisites for closed converter issues:

| Issue | Title | Likely Resolved By | Action |
|-------|-------|-------------------|--------|
| #111 | Add response_format and reasoning_effort to OpenAI models | #115, #116 (both CLOSED) | Verify & close if fields exist |
| #112 | Add redacted_thinking to Anthropic ContentBlock | #118 (CLOSED, streaming handles it) | Verify & close if variant exists |
| #113 | Add cache token fields to usage models | Partial — #121 still OPEN | Leave open |

### Plan Files — Board Coverage

**Pre-board era (17 plans, all done)** — These predate the board (created 2026-03-13). No board items needed:

| Plan | Status |
|------|--------|
| google-sso-multi-user-rbac | done |
| backend-modularization | done |
| fix-ci-pipeline | done |
| gemini-provider-removal | done |
| gemini-removal-pr-description | done |
| sso-config-to-profile-page | done |
| worktree-research | done |
| password-auth-totp-2fa | done |
| secret-leak-prevention | done |
| remove-release-workflow | done |
| profile-security-section | done |
| admin-only-model-loading | done |
| mcp-registry-removal | done |
| claude-workflow-overhaul | done |
| large-file-handling-rules | done |
| github-kanban-board | done |
| worktree-multi-agent | draft |

**Post-board era — Has board items (3 plans):**

| Plan | Board Items |
|------|-------------|
| provider-config-db-migration (done) | #93, #94, #95, #96, #97, #98, #101 |
| litellm-conversion-gaps (done) | #106-#121 |
| fix-code-review-findings (draft) | Related to existing items |

**Post-board era — Missing from board, internal/tooling (6 plans, all done):**

These are internal tooling, CI, or small maintenance. Skip board creation:

| Plan | Reason to Skip |
|------|---------------|
| fix-worktree-hook-errors | Internal tooling fix |
| rename-plan-skill | Internal skill creation |
| gh-operation-skills | Internal skill creation |
| in-process-teammate-mode | Config toggle, no code change |
| fix-documentation | Docs-only, no feature |
| fix-dependabot-prs | Dependency maintenance |

**Post-board era — Missing from board, should be tracked (5 plans):**

| Plan | Status | Action |
|------|--------|--------|
| remove-tls-nginx-artifacts | done | Create issue → Done |
| e2e-auth-totp-coverage | done | Create issue → Done |
| docker-hub-publishing | done | Create issue → Done |
| multi-account-load-balancing | in-progress | Create issue → In Progress |
| multi-provider-proxy-mode | in-progress | Create issue → In Progress |

## Execution Steps

### Step 1: Fix board statuses (2 items)
Update #97 and #114 board status from "In progress" → "Done"

### Step 2: Close resolved GAP issues + update board (3 items)
Close #106, #107, #108 on GitHub. Update board status → "Done"

### Step 3: Verify and close implementation issues (2 items)
Check if #111 and #112 fields were added in the codebase. If yes, close and mark Done.

### Step 4: Create missing board items for post-board plans (5 issues)
Create GH issues for the 5 plans missing from the board, add to project, set appropriate status.

### Step 5: Rename this plan file
Rename to `2026-03-17-15-22-audit-plans-board-sync-in-progress.md`

## Verification

- `gh project item-list 3 --owner if414013` shows all items with correct statuses
- No CLOSED issues show "In progress" or "Backlog" on the board
- All post-board plans with meaningful scope have board representation
