---
name: team-plan
description: Analyze scope, explore codebase, and produce implementation plans. Spawns all 7 domain agents for parallel investigation from their domain perspective, and writes structured plans. Use when user says 'plan this feature', 'analyze scope', 'what would it take to build X', or 'explore before implementing'.
argument-hint: "[feature-description] [--scope path]"
allowed-tools:
  - Bash
  - Read
  - Grep
  - Glob
  - Write
  - AskUserQuestion
  - Agent
  - TeamCreate
  - SendMessage
  - TaskCreate
  - TaskUpdate
  - TaskList
---

# Team Plan

Analyze scope, explore the codebase, and produce structured implementation plans.

## Critical Constraints

- **In-process teammate mode only** — all agents MUST run in-process (`teammateMode: "in-process"`). Never use tmux, iTerm split panes, or any other mode. Cycle between agents with Shift+Down.
- **Always spawn all 7 agents** — no conditional composition or presets
- **Codex review gate required** — no plan is approved for `/team-implement` until Phase 6 passes

## Phase 1: Load Project Context

1. Read `CLAUDE.md` Service Map to identify all services and their verification commands
2. Read `.claude/agents/*.md` to build agent registry (name, description, tools, ownership)
3. Parse the user's feature description and `--scope` path (if provided)

## Phase 2: Domain Consultation

Spawn all 7 domain agents via `TeamCreate` + `Agent` for parallel investigation:

Each agent investigates from its domain perspective:
- **rust-backend-engineer**: Affected backend modules, existing patterns, type changes, error handling approach
- **react-frontend-engineer**: Component structure, API integration points, styling approach
- **database-engineer**: Schema impact, migration needs, query patterns
- **devops-engineer**: Docker impact, env vars, deployment changes
- **backend-qa**: Test coverage gaps, test patterns to follow
- **frontend-qa**: E2E test scenarios, Playwright page objects to update
- **document-writer**: Documentation gaps, API doc updates needed

Agents without relevant findings report "no impact" and remain idle.

## Phase 3: Scope Analysis

Using consultation results:

1. **Classify affected services** — map each change to a service from the Service Map
2. **Identify file ownership boundaries** — one agent per file, no overlaps
3. **Estimate complexity** — small/medium/large per service based on:
   - Number of files to create/modify
   - Cross-service dependencies
   - Test coverage needed

## Phase 4: Wave Decomposition

Break work into dependency-ordered waves:

- **Wave 1** (foundations): Types, schemas, core logic, migrations
- **Wave 2** (consumers): Handlers, UI components, API integration
- **Wave 3** (verification): Unit tests, E2E tests, integration tests
- **Wave 4** (documentation): API docs, architecture updates (if needed)

Each wave lists:
- Tasks with file assignments
- Dependencies on prior waves
- Agent assignments
- Verification commands

## Phase 5: Plan Output

Write plan to `.claude/plans/` with:

1. **Consultation Summary** — findings from each domain agent
2. **File Manifest** — files to create/modify, one owner per file
3. **Wave Structure** — dependency graph with tasks per wave
4. **Interface Contracts** — API shapes, type definitions shared between services
5. **Verification Commands** — per-service quality gates from CLAUDE.md
6. **Branch Name** — `feat/{feature-slug}` or `fix/{feature-slug}` per git workflow conventions

### Plan File Format

```markdown
# Plan: {feature-name}

## Consultation Summary
- rust-backend-engineer: {findings}
- react-frontend-engineer: {findings}
- database-engineer: {findings}
- devops-engineer: {findings}
- backend-qa: {findings}
- frontend-qa: {findings}
- document-writer: {findings}

## File Manifest
| File | Action | Owner | Wave |
|------|--------|-------|------|
| ... | create/modify | agent-name | 1 |

## Wave 1: {name}
- [ ] Task description (assigned: agent-name)
  - Files: ...
  - Depends on: none

## Wave 2: {name}
...

## Interface Contracts
...

## Verification
...

## Branch
`feat/{feature-slug}` or `fix/{feature-slug}`

## Review Status
- Codex review: {passed / adjusted / escalated}
- Findings addressed: {count}
- Disputed findings: {count}
```

## Phase 6: Codex Plan Review Gate

After writing the plan file, invoke Codex CLI to review it. No `/team-implement` until this gate passes.

### 6.1 Run Codex Review

```bash
PLAN_FILE=".claude/plans/{plan-file}.md"
REVIEW_FILE=".claude/plans/{plan-name}-codex-review.md"

codex exec \
  -s read-only \
  --ephemeral \
  -o "$REVIEW_FILE" \
  "team-review --plan $PLAN_FILE"
```

### 6.2 Evaluate Review

Read the Codex review file and classify each finding:

| Severity | Action |
|----------|--------|
| **high** | Must address — adjust the plan |
| **medium** | Evaluate — adjust if valid, note if disputed |
| **low/info** | Acknowledge, no plan change needed |

### 6.3 Adjustment Loop (max 1 round)

If Codex found high/medium issues:
1. Evaluate each finding against the codebase (read the actual files Codex cited)
2. If the finding is **valid**: adjust the plan accordingly
3. If the finding is a **hallucination** (Codex citing nonexistent code, wrong patterns, or incorrect assumptions):
   - Note it as "disputed" in the review summary
   - Do NOT adjust the plan for hallucinated findings

After adjustments, re-run Codex review once:
```bash
codex exec -s read-only --ephemeral -o "$REVIEW_FILE" "team-review --plan $PLAN_FILE"
```

**Only 1 adjustment round.** After the second Codex review:
- If Codex approves or only has low/info findings → gate passes
- If Codex still insists on disputed findings → escalate to user

### 6.4 Escalation

If Codex and Claude disagree after 1 adjustment round, present both perspectives to the user via AskUserQuestion:

```
Codex review found issues that I believe are incorrect:

1. [Codex finding]: "..."
   [My assessment]: "This is not applicable because..."

How should we proceed?
- Accept plan as-is (override Codex)
- Adjust plan per Codex suggestions
- Let me review the specific findings
```

The user's decision is final.

### 6.5 Clean Up

Delete the ephemeral review file after the gate passes:
```bash
rm -f "$REVIEW_FILE"
```

### 6.6 Gate Result

The plan is approved for `/team-implement` only when:
1. Codex review has no unresolved high findings, AND
2. User has accepted the plan (via ExitPlanMode)

Update the plan's `## Review Status` section with the final result.
