---
name: team-implement
description: Full lifecycle feature implementation — spawns teams, assigns tasks, monitors progress, verifies quality, and creates PRs. Agents remain idle after completion — use /team-shutdown to terminate. Use when user says 'implement this', 'build this feature', 'start working on X', or 'execute the plan'.
argument-hint: "[feature-or-plan-description]"
allowed-tools:
  - Bash
  - Read
  - Write
  - Grep
  - Glob
  - SendMessage
  - AskUserQuestion
  - TeamCreate
  - Agent
  - TaskCreate
  - TaskUpdate
  - TaskList
---

# Team Implement

Full lifecycle feature implementation. Spawns teams, assigns tasks, monitors progress, verifies quality, and creates PRs. Agents remain idle after completion — use `/team-shutdown` to terminate.

---

## Full Lifecycle

### Phase 1: Load Context

1. Read `CLAUDE.md` Service Map to identify all services, verification commands, and agent role keywords
2. Read `.claude/agents/*.md` to build agent registry (name, description, tools)
3. Read `.claude/agent-colors.json` for visual agent identification
4. Check for existing plan files in `.claude/plans/` matching the feature description

### Phase 2: Spawn All Agents

Always spawn all 7 domain agents via `TeamCreate` + `Agent`:

1. rust-backend-engineer
2. react-frontend-engineer
3. database-engineer
4. devops-engineer
5. backend-qa
6. frontend-qa
7. document-writer

Agents without assigned tasks remain idle and available for ad-hoc work via `SendMessage`.

### Phase 3: Branch Creation

Create a feature branch for the implementation:
```bash
git checkout -b feat/{feature-slug}
```

### Phase 4: Plan Decomposition

If a plan file exists in `.claude/plans/`, use it as input. Otherwise, decompose the feature into waves:

- **Wave 1** (foundations): Types, schemas, migrations, core logic
- **Wave 2** (consumers): Route handlers, UI components, API integration
- **Wave 3** (verification): Unit tests, E2E tests, integration tests
- **Wave 4** (documentation): API docs, architecture updates (if needed)

For each task:
- Assign one owner agent
- List files to create/modify (one owner per file)
- Define dependencies on other tasks
- Specify verification commands

### Phase 5: Assign Waves

All 7 agents are already spawned from Phase 2. Assign wave tasks to agents:
- Agents with Wave 1 tasks start working immediately
- Agents with later-wave tasks wait until dependencies resolve
- Agents without tasks remain idle and available for ad-hoc requests

### Phase 6: Assign

Send each agent their task via `SendMessage`:
- Owned files and required changes
- Interface contracts with other agents
- Dependencies and wave number
- Verification commands to run after completion

### Phase 7: Monitor

Run a health monitoring loop:

1. **Check agent activity**: `git log`, file modification times, TaskList status
2. **Classify agents**: active / quiet / stale
3. **Context exhaustion detection**: 3+ consecutive idle notifications with in_progress task and no file edits = exhausted
4. **Auto-respawn**: If context-exhausted:
   - Capture completed work from `git log`
   - Note remaining tasks from TaskList
   - Respawn agent with same name for ownership continuity
   - Send handoff summary with completed commits and remaining tasks
5. **Wave progression**: When all Wave N tasks complete, spawn deferred Wave N+1 agents

### Phase 8: Verify

Run quality gates per affected service:

| Service | Verification |
|---------|-------------|
| Backend | `cargo clippy --all-targets && cargo test --lib && cargo fmt --check` |
| Frontend | `npm run build && npm run lint` |
| Infrastructure | `docker compose config --quiet` |

Cross-service validation:
- Grep for shared types/endpoints to ensure contract consistency
- Run E2E tests if both backend and frontend changed

### Phase 9: PR

Create PR from the feature branch:
```bash
git add -A && git commit -m "feat(scope): description"
git push -u origin feat/{feature-slug}
gh pr create --title "feat: ..." --body "## Summary\n..."
```

### Phase 10: Report

Output final status:
- Work streams completed
- Verification results (pass/fail per gate)
- PR URL (if created)
- Agents remain idle — use `/team-shutdown` when done

---

## Secondary Operations

These can be invoked inline during a team session but are not primary entry points.

### Delegate (`--delegate team-name`)

Interactive task management menu:

1. **Assign task**: Select agent → describe task → create TaskList entry → send via SendMessage
2. **Send message**: Select agent → compose message → SendMessage
3. **Broadcast**: Send message to all team members
4. **Rebalance**: Move tasks between agents (update TaskList ownership)
5. **Reclaim**: Take back an unresponsive agent's tasks

Agent validation is dynamic — read team config for current members, never hardcode names.

Note: Use `/team-shutdown` to terminate the team when done.
