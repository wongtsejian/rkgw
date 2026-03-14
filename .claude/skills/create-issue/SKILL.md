---
name: create-issue
description: |
  Create a GitHub Issue on the Harbangan Board with proper labels, priority,
  and size fields. Delegates to kanban-master agent for board field setup.
  Use when user says 'create issue', 'new ticket', 'add to board',
  'create a task', or 'file an issue'.
argument-hint: "<title> [--service backend|frontend|infra|e2e] [--priority p0|p1|p2] [--size xs|s|m|l|xl]"
allowed-tools:
  - Bash
  - Read
  - Grep
  - Glob
  - AskUserQuestion
  - Agent
---

# Create Issue

Create a GitHub Issue on the Harbangan Board with full project field setup.

## Inputs

Parse `$ARGUMENTS` for:
- **Title**: first positional argument (required)
- **--service**: `backend`, `frontend`, `infra`, `e2e` (optional, ask if missing)
- **--priority**: `p0`, `p1`, `p2` (optional, default p1)
- **--size**: `xs`, `s`, `m`, `l`, `xl` (optional, default m)

If `$ARGUMENTS` is empty or title is missing, use `AskUserQuestion` to gather:
1. Issue title
2. Service area (backend/frontend/infra/e2e)
3. Priority (P0 critical / P1 high / P2 medium)
4. Size estimate (XS/S/M/L/XL)

## Execution

Spawn a `kanban-master` agent to handle issue creation and board setup:

```
Agent({
  subagent_type: "kanban-master",
  mode: "bypassPermissions",
  prompt: "Create a GitHub Issue on the Harbangan Board..."
})
```

The agent must:
1. Create the issue with `gh issue create --project "Harbangan Board"`
2. Apply labels: `service:{service}`, type label, priority label
3. Set board fields: Status (Ready), Priority, Size
4. Return the issue URL

## Board Constants

Read from `.claude/agents/kanban-master.md` lines 69-82:
```
PROJECT_ID     = PVT_kwHOATKEhs4BRp0j
PROJECT_NUMBER = 3
OWNER          = if414013
STATUS_FIELD   = PVTSSF_lAHOATKEhs4BRp0jzg_azo8
PRIORITY_FIELD = PVTSSF_lAHOATKEhs4BRp0jzg_azuA
SIZE_FIELD     = PVTSSF_lAHOATKEhs4BRp0jzg_azuE
```

## Output

Return the created issue URL and board status to the user.
