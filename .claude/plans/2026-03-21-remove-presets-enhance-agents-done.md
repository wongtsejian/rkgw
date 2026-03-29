# Plan: Remove Presets, Always-Spawn All Agents, Enhance Agent Definitions

## Context

The current team skills use conditional composition (presets) to selectively spawn agents based on the task scope. The user wants ALL agents always spawned and idle for every team skill invocation — regardless of whether there are changes in that agent's domain. This simplifies the workflow: every skill gets the full team, agents stay idle until assigned work, and `/team-shutdown` terminates them explicitly.

Additionally, agent definitions need enhancement with clear ownership boundaries, scope, and cross-agent collaboration protocols. Ownership will be enforced both via documented guidelines AND PreToolUse hooks in `.claude/hooks/`.

## Part 1: Remove Presets & Always-Spawn All Agents

### 1.1 Edit `team-implement/SKILL.md`

**Remove Phase 2 (Resolve Composition)** — the entire conditional composition table (lines 36-51). Replace with:

```markdown
### Phase 2: Spawn All Agents

Always spawn all 7 domain agents via `TeamCreate` + `Agent`:

1. rust-backend-engineer
2. react-frontend-engineer
3. database-engineer
4. devops-engineer
5. backend-qa
6. frontend-qa
7. document-writer

Agents without assigned tasks remain idle and available for ad-hoc work.
```

**Remove Phase 5 (lazy per-wave spawning logic)** — replace with simple "spawn all at once":

```markdown
### Phase 5: Spawn

Spawn all 7 agents at once via `TeamCreate` + `Agent` with `team_name`.
All agents use `mode: "bypassPermissions"` and `subagent_type` matching agent name.
Agents without Wave 1 tasks remain idle until their wave begins.
```

**Remove `--preset` argument** from frontmatter `argument-hint`.

### 1.2 Edit `team-plan/SKILL.md`

**Replace Phase 2 (Parallel Exploration with 3 Explore agents)** with spawning all 7 domain agents for consultation:

```markdown
### Phase 2: Domain Consultation

Spawn all 7 domain agents via `TeamCreate` + `Agent` for parallel investigation:

Each agent investigates from its domain perspective:
- **rust-backend-engineer**: Affected backend modules, existing patterns, type changes
- **react-frontend-engineer**: Component structure, API integration points, styling
- **database-engineer**: Schema impact, migration needs, query patterns
- **devops-engineer**: Docker impact, env vars, deployment changes
- **backend-qa**: Test coverage gaps, test patterns to follow
- **frontend-qa**: E2E test scenarios, Playwright page objects
- **document-writer**: Documentation gaps, API doc updates needed

Agents without relevant findings report "no impact" and remain idle.
```

**Remove "Recommended Preset" from Phase 5** plan output (line 81, `Recommended Team Preset`).

**Add `TeamCreate`, `SendMessage`, `TaskCreate`, `TaskUpdate`, `TaskList`** to allowed-tools (currently only has Agent).

### 1.3 Edit `team-review/SKILL.md`

**Remove "Recommended Presets" table** (lines 74-84) that conditionally selects dimensions. Replace with:

```markdown
### Always Spawn All 5 Reviewers

Spawn all 5 dimension reviewers for every review:
1. Security reviewer
2. Performance reviewer
3. Architecture reviewer
4. Testing reviewer
5. Accessibility reviewer

Reviewers examining areas with no relevant changes report "no findings" for their dimension.
```

### 1.4 Edit `team-debug/SKILL.md`

**Remove "Debug Presets" table** (lines 89-96). **Replace Phase 3 (Investigation)** to spawn all 7 domain agents instead of hypothesis-specific investigators:

```markdown
### 3.1 Spawn All Domain Agents

Spawn all 7 domain agents via `TeamCreate` + `Agent`. Assign hypotheses to the most relevant agents. Agents without assigned hypotheses remain idle and available for follow-up investigation if needed.
```

**Remove error domain → single agent mapping** (lines 76-88 classifier table). Keep the table as reference but assign hypotheses to all relevant agents, not just one.

### 1.5 Edit `.claude/README.md`

- Remove entire **Team presets** table (lines 65-84)
- Remove `--preset` from team-implement argument description
- Update team-implement description: remove preset references
- Update Team Skills count to reflect changes
- Remove `--preset name` from key arguments column

### 1.6 Edit `.claude/CLAUDE.md`

- Remove any remaining `--preset` references in structure tree descriptions

### 1.7 Edit `.claude/rules/team-coordination.md`

- Remove **Team Sizing** table (lines 6-13) — no longer relevant since all agents always spawn
- Update heuristics text — remove "1 agent per architectural layer" since all spawn

---

## Part 2: Enhance Agent Definitions

Rewrite all 7 agent files with clear structure:

### Standard Template for Each Agent

```markdown
---
name: {agent-name}
description: {clear, specific description}
tools: Read, Edit, Write, Bash, Grep, Glob
model: opus
maxTurns: {80-100}
permissionMode: bypassPermissions
memory: project
---

# {Agent Name}

{One-line role summary}

## Ownership

### Files You Own (full Write/Edit access)
- `path/to/files/**` — description

### Shared Files (coordinate via DM)
- `path/to/shared/file` — owned by {other-agent}, request changes via DM

### Off-Limits (do not edit)
- `path/to/other/**` — owned by {other-agent}

## Responsibilities
- Bullet list of what this agent does

## Quality Gates
```bash
commands to verify work
```

## Cross-Agent Collaboration
- When you need {other-agent} to do X: DM with {format}
- When {other-agent} asks you to do X: evaluate and apply

## Technical Context
{Project-specific patterns, conventions, key files}
```

### 2.1 `rust-backend-engineer.md` — Enhanced

**Owns:**
- `backend/src/converters/**` — Format converters
- `backend/src/streaming/**` — AWS Event Stream parser
- `backend/src/auth/**` — Kiro token management
- `backend/src/middleware/**` — CORS, API key auth
- `backend/src/guardrails/**` — Content validation
- `backend/src/models/**` — Request/response types
- `backend/src/routes/mod.rs` — Route handlers (shared, but primary owner)
- `backend/src/web_ui/**` — Web UI handlers (except config_db.rs DDL)
- `backend/src/metrics/**` — Metrics collection
- `backend/src/resolver.rs`, `backend/src/tokenizer.rs`, `backend/src/truncation.rs`, `backend/src/cache.rs`
- `backend/src/http_client.rs`, `backend/src/error.rs`, `backend/src/log_capture.rs`
- `backend/Cargo.toml` — Dependencies (shared, primary owner)

**Shared:**
- `backend/src/web_ui/config_db.rs` — DDL blocks owned by database-engineer; Rust query code owned by you
- `backend/src/routes/mod.rs` — Others request route additions via DM

**Off-Limits:**
- `frontend/**` — react-frontend-engineer
- `docker-compose*.yml`, `**/Dockerfile` — devops-engineer
- `e2e-tests/**` — frontend-qa
- `.claude/**` — project config

### 2.2 `react-frontend-engineer.md` — Enhanced

**Owns:**
- `frontend/src/pages/**` — Page components
- `frontend/src/components/**` — Reusable components
- `frontend/src/lib/**` — Utilities (api.ts, auth.ts, useSSE.ts)
- `frontend/src/styles/**` — CSS (variables.css, global.css, components.css)
- `frontend/src/App.tsx`, `frontend/src/main.tsx`
- `frontend/package.json` — Dependencies (shared, primary owner)
- `frontend/vite.config.ts`, `frontend/tsconfig*.json`

**Off-Limits:**
- `backend/**` — rust-backend-engineer
- `docker-compose*.yml` — devops-engineer
- `e2e-tests/**` — frontend-qa

### 2.3 `database-engineer.md` — Enhanced

**Owns:**
- `backend/src/web_ui/config_db.rs` — DDL blocks only (CREATE TABLE, ALTER TABLE, IF NOT EXISTS, migration version blocks, indexes)

**Advises on (read-only, suggest via DM):**
- sqlx query patterns in any `backend/src/**` file
- Index strategy, data integrity constraints
- Query performance optimization

**Off-Limits (everything else):**
- `backend/src/**` Rust handler code — rust-backend-engineer
- `frontend/**` — react-frontend-engineer
- `docker-compose*.yml` — devops-engineer

**Fix**: Add `model: opus` to frontmatter (currently missing).

### 2.4 `devops-engineer.md` — Enhanced

**Owns:**
- `docker-compose.yml`, `docker-compose.gateway.yml`
- `frontend/Dockerfile`, any `**/Dockerfile`
- `frontend/entrypoint.sh`
- `.env.example` — Environment variable documentation

**Off-Limits:**
- `backend/src/**` — rust-backend-engineer
- `frontend/src/**` — react-frontend-engineer
- `e2e-tests/**` — frontend-qa

### 2.5 `backend-qa.md` — Enhanced

**Owns:**
- `#[cfg(test)] mod tests` blocks in any `backend/src/**` file
- Test helper functions, test fixtures, test utilities

**Scope clarification:** Writes tests only. Does NOT implement features. If a test reveals a bug, report it via DM to rust-backend-engineer.

**Off-Limits:**
- Production code in `backend/src/**` (outside test modules) — rust-backend-engineer
- `frontend/**` — react-frontend-engineer
- `e2e-tests/**` — frontend-qa

### 2.6 `frontend-qa.md` — Enhanced

**Owns:**
- `e2e-tests/**` — All Playwright E2E test specs and config

**Scope clarification:** Writes E2E tests only. Does NOT implement frontend components. If tests reveal UI bugs, report via DM to react-frontend-engineer.

**Off-Limits:**
- `frontend/src/**` — react-frontend-engineer
- `backend/**` — rust-backend-engineer

### 2.7 `document-writer.md` — Enhanced

**Owns:**
- Documentation files when explicitly created
- Notion/Slack publishing

**Scope clarification:** Read-only access to all source code for documentation purposes. Does NOT modify source code. Writes documentation only.

**Off-Limits:**
- All source code files (`backend/**`, `frontend/**`, `e2e-tests/**`)

---

## Part 3: Ownership Enforcement Hooks

Create a single hook script in `.claude/hooks/` that enforces file ownership based on the `CLAUDE_AGENT_NAME` environment variable (set automatically for spawned agents).

### 3.1 Create `.claude/hooks/enforce-file-ownership.sh`

```bash
#!/bin/bash
# PreToolUse hook: enforces file ownership per agent
# Blocks Write/Edit on files outside the agent's owned scope
# Only active when running as a spawned agent (CLAUDE_AGENT_NAME is set)

INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')
AGENT="$CLAUDE_AGENT_NAME"

# Skip if not a spawned agent or no file path
if [ -z "$AGENT" ] || [ -z "$FILE_PATH" ]; then
  exit 0
fi

# Normalize path
FILE_PATH=$(realpath --relative-to="$CLAUDE_PROJECT_DIR" "$FILE_PATH" 2>/dev/null || echo "$FILE_PATH")

deny() {
  cat <<ENDJSON
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"$1"}}
ENDJSON
  exit 0
}

case "$AGENT" in
  rust-backend-engineer)
    # Owns: backend/src/** (except config_db.rs DDL — shared)
    # Owns: backend/Cargo.toml
    echo "$FILE_PATH" | grep -qE '^backend/' && exit 0
    deny "rust-backend-engineer can only edit backend/**. DM the file owner for: $FILE_PATH"
    ;;
  react-frontend-engineer)
    # Owns: frontend/src/**, frontend/package.json, frontend/*.config.*
    echo "$FILE_PATH" | grep -qE '^frontend/' && exit 0
    deny "react-frontend-engineer can only edit frontend/**. DM the file owner for: $FILE_PATH"
    ;;
  database-engineer)
    # Owns: config_db.rs only (DDL blocks)
    echo "$FILE_PATH" | grep -qE 'config_db\.rs$' && exit 0
    deny "database-engineer can only edit config_db.rs. DM the file owner for: $FILE_PATH"
    ;;
  devops-engineer)
    # Owns: docker-compose*.yml, Dockerfile*, .env.example, entrypoint.sh
    echo "$FILE_PATH" | grep -qE '(docker-compose|Dockerfile|\.env\.example|entrypoint)' && exit 0
    deny "devops-engineer can only edit Docker/infra files. DM the file owner for: $FILE_PATH"
    ;;
  backend-qa)
    # Owns: test modules in backend/src/** only
    echo "$FILE_PATH" | grep -qE '^backend/src/' && exit 0
    deny "backend-qa can only edit backend/src/** (test modules). DM the file owner for: $FILE_PATH"
    ;;
  frontend-qa)
    # Owns: e2e-tests/**
    echo "$FILE_PATH" | grep -qE '^e2e-tests/' && exit 0
    deny "frontend-qa can only edit e2e-tests/**. DM the file owner for: $FILE_PATH"
    ;;
  document-writer)
    # Read-only — cannot edit source code
    deny "document-writer is read-only for source code. Create documentation files only."
    ;;
esac

exit 0
```

### 3.2 Register hook in `.claude/settings.json`

Add to the existing `PreToolUse` hooks array:

```json
{
  "matcher": "Write|Edit",
  "hooks": [
    {
      "type": "command",
      "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/enforce-file-ownership.sh"
    }
  ]
}
```

---

## Part 4: Update Documentation

### 4.1 `.claude/CLAUDE.md`
- Remove preset references from structure tree
- Update skill descriptions

### 4.2 `.claude/README.md`
- Remove team presets table entirely
- Remove `--preset` from argument hints
- Update team-implement description
- Update Planning to Execution Flow

### 4.3 `.claude/rules/team-coordination.md`
- Remove Team Sizing table (all agents always spawn)
- Add `config_db.rs` to Shared File Protocol
- Keep file ownership table, parallel work patterns, dependency chain

### 4.4 `.claude/rules/plan-mode.md`
- Remove preset recommendation from plan output requirements

---

## Files Summary

### Create
| File | Purpose |
|------|---------|
| `.claude/hooks/enforce-file-ownership.sh` | PreToolUse hook for ownership enforcement |

### Rewrite (major changes)
| File | What Changes |
|------|-------------|
| `.claude/agents/rust-backend-engineer.md` | Enhanced with ownership/scope/collaboration sections |
| `.claude/agents/react-frontend-engineer.md` | Enhanced with ownership/scope/collaboration sections |
| `.claude/agents/database-engineer.md` | Enhanced, add `model: opus`, clarify DDL-only scope |
| `.claude/agents/devops-engineer.md` | Enhanced with ownership/scope/collaboration sections |
| `.claude/agents/backend-qa.md` | Enhanced, clarify test-only scope |
| `.claude/agents/frontend-qa.md` | Enhanced, clarify test-only scope |
| `.claude/agents/document-writer.md` | Enhanced, clarify read-only scope |

### Edit (targeted changes)
| File | What Changes |
|------|-------------|
| `.claude/skills/team-implement/SKILL.md` | Remove presets, always spawn all 7 |
| `.claude/skills/team-plan/SKILL.md` | Replace Explore agents with all 7 domain agents |
| `.claude/skills/team-review/SKILL.md` | Remove preset table, always spawn all 5 reviewers |
| `.claude/skills/team-debug/SKILL.md` | Remove presets, use all 7 domain agents |
| `.claude/README.md` | Remove presets table, update descriptions |
| `.claude/CLAUDE.md` | Remove preset references |
| `.claude/rules/team-coordination.md` | Remove team sizing, add config_db.rs shared protocol |
| `.claude/rules/plan-mode.md` | Remove preset recommendation |
| `.claude/settings.json` | Add ownership hook to PreToolUse |

## Verification

1. `grep -ri "preset" .claude/skills/ .claude/README.md .claude/CLAUDE.md` — should return nothing
2. `grep -ri "composition" .claude/skills/` — should return nothing
3. Verify hook works: `echo '{"tool_input":{"file_path":"frontend/src/App.tsx"}}' | CLAUDE_AGENT_NAME=rust-backend-engineer CLAUDE_PROJECT_DIR=. bash .claude/hooks/enforce-file-ownership.sh` — should deny
4. All 7 agent files have consistent ownership/scope/collaboration structure
5. `grep "model:" .claude/agents/*.md` — all should show opus
