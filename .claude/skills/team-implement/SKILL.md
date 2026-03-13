---
name: team-implement
description: Full lifecycle feature implementation — spawns teams, assigns tasks, monitors progress, verifies quality, creates PRs, and shuts down. Absorbs team-spawn, team-feature, team-delegate, team-status, and team-shutdown into one unified workflow. Use when user says 'implement this', 'build this feature', 'start working on X', or 'execute the plan'.
argument-hint: "[feature-or-plan] [--preset name] [--worktree] [--no-worktree] [--shutdown team-name] [--status team-name] [--delegate team-name]"
allowed-tools:
  - Bash
  - Read
  - Write
  - Grep
  - Glob
  - SendMessage
  - AskUserQuestion
  - TeamCreate
  - TeamDelete
  - Agent
  - TaskCreate
  - TaskUpdate
  - TaskList
---

# Team Implement

Full lifecycle feature implementation. Spawns teams, assigns tasks, monitors progress, verifies quality, creates PRs, and shuts down.

## Sub-Commands

- **No flags**: Full lifecycle (spawn → implement → verify → PR → shutdown)
- `--status team-name`: Show team status only
- `--delegate team-name`: Interactive task assignment
- `--shutdown team-name`: Graceful shutdown only

---

## Full Lifecycle

### Phase 1: Load Context

1. Read `CLAUDE.md` Service Map to identify all services, verification commands, and agent role keywords
2. Read `.claude/agents/*.md` to build agent registry (name, description, tools)
3. Read `.claude/agent-colors.json` for visual agent identification
4. Check for existing plan files in `.claude/plans/` matching the feature description

### Phase 2: Resolve Composition

Select a team preset based on the feature scope. Match using keywords from the Service Map.

| Preset | Composition | Use When |
|--------|-------------|----------|
| `fullstack` | coordinator + all service agents + QA agents | Full-stack feature touching backend + frontend |
| `backend-feature` | coordinator + backend + database + backend-qa | Backend-only feature |
| `frontend-feature` | coordinator + frontend + frontend-qa | Frontend-only feature |
| `infra` | coordinator + infra + backend | Infrastructure changes |
| `docs` | coordinator + document-writer | Documentation |
| `research` | 3 general-purpose agents | Codebase exploration, investigation |
| `security` | 4 reviewer agents (OWASP, auth, deps, config) | Security audit |
| `migration` | coordinator + 2 service agents + 1 reviewer | Data/schema migration |
| `refactor` | coordinator + 2 service agents + 1 reviewer | Code refactoring |
| `hotfix` | 1 service agent + 1 QA agent | Urgent bug fix |

### Phase 3: Worktree Resolution

1. Check for active teams: `ls .trees/ 2>/dev/null`
2. If `--worktree` flag or other teams active, create worktree:
   ```bash
   BRANCH="feat/{feature-slug}"
   git worktree add .trees/{team-name} -b $BRANCH
   ```
3. If `--no-worktree`, work in main directory
4. Record working directory in team config

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

### Phase 5: GitHub Issues

For each task in the decomposition:
```bash
gh issue create --title "[service]: task description" \
  --label "service:{service},priority:{p0-p3}" \
  --body "Description, acceptance criteria, Depends on #N"
```

Apply `status:blocked` label for tasks with open dependencies.

### Phase 6: Spawn

Use lazy per-wave spawning:

1. **Wave 1 agents**: Spawn immediately via `TeamCreate` + `Agent` with `team_name`
2. **Wave 2+ agents**: Record as deferred in team config, spawn when previous wave completes

For each agent spawn:
- Use `isolation: "worktree"` if worktree is active
- Set `mode: "bypassPermissions"` for autonomous execution
- Match `subagent_type` to agent name from the registry

### Phase 7: Assign

Send each agent their task via `SendMessage`:
- Owned files and required changes
- Interface contracts with other agents
- Dependencies and wave number
- Verification commands to run after completion

### Phase 8: Monitor

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

### Phase 9: Verify

Run quality gates per affected service:

| Service | Verification |
|---------|-------------|
| Backend | `cargo clippy --all-targets && cargo test --lib && cargo fmt --check` |
| Frontend | `npm run build && npm run lint` |
| Infrastructure | `docker compose config --quiet` |

Cross-service validation:
- Grep for shared types/endpoints to ensure contract consistency
- Run E2E tests if both backend and frontend changed

### Phase 10: PR

If worktree is active:
```bash
cd .trees/{team-name}
git add -A && git commit -m "feat(scope): description"
git push -u origin feat/{feature-slug}
gh pr create --title "feat: ..." --body "## Summary\n..."
```

### Phase 11: Shutdown

Ordered termination:
1. Persist incomplete work to GitHub Issues (update status, add progress notes)
2. Commit uncommitted changes in worktree
3. Push unpushed commits
4. Workers first, coordinator last
5. `TeamDelete` for each agent
6. Remove worktree: `git worktree remove .trees/{team-name} && git worktree prune`

### Phase 12: Report

Output final status:
- Work streams completed
- GitHub Issues created/closed
- Verification results (pass/fail per gate)
- PR URL (if created)

---

## Status Sub-Command (`--status team-name`)

1. Load team config from TeamCreate output
2. Check agent activity:
   - `git log --author={agent}` for recent commits
   - File modification times in owned directories
   - TaskList status for assigned tasks
3. Classify each agent:
   - **Active**: commits or file changes in last 5 minutes
   - **Quiet**: no activity for 5-15 minutes
   - **Stale**: no activity for 15+ minutes
4. Cross-reference TaskList vs GitHub Issues for drift
5. Context exhaustion check: 3+ idle notifications with in_progress task
6. Output alerts for stale/stuck agents

---

## Delegate Sub-Command (`--delegate team-name`)

Interactive task management menu:

1. **Assign task**: Select agent → describe task → create TaskList entry → send via SendMessage
2. **Send message**: Select agent → compose message → SendMessage
3. **Broadcast**: Send message to all team members
4. **Rebalance**: Move tasks between agents (update TaskList ownership)
5. **Reclaim**: Take back an unresponsive agent's tasks

Agent validation is dynamic — read team config for current members, never hardcode names.

Blocked task detection: check GitHub Issue labels for `status:blocked`, warn before assigning.

---

## Shutdown Sub-Command (`--shutdown team-name`)

1. Check for uncommitted changes in worktree
2. Check for unpushed commits
3. Persist incomplete tasks to GitHub Issues (add progress comment, update labels)
4. Terminate workers first, coordinator last
5. `TeamDelete` for each agent
6. If worktree active:
   - `git worktree remove .trees/{team-name}`
   - `git worktree prune`
7. Clean up team config
