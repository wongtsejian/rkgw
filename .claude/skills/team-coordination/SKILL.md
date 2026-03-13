---
name: team-coordination
description: Reference guide for team composition patterns, file ownership rules, communication protocols, task coordination, and parallel development strategies. Use when user asks about 'file ownership rules', 'team communication', 'how to coordinate agents', or 'team sizing'.
---

# Team Coordination Reference

## 1. Team Sizing

| Complexity | Team Size | When |
|------------|-----------|------|
| Simple | 2-3 agents | Single-service change, one layer |
| Moderate | 3-4 agents | Cross-layer (backend + frontend) |
| Complex | 4-5 agents | Multi-service, infrastructure changes |

### Sizing Heuristics

- **1 agent per architectural layer** being modified (backend, frontend, infra)
- **+1 agent for QA** if the change affects streaming, auth, or converters
- **Never exceed 5 agents** — split into multiple tracks instead

---

## 2. File Ownership Rules

**One owner per file. No exceptions.**

### Harbangan Ownership Examples

| File / Area | Owner | Rationale |
|-------------|-------|-----------|
| `backend/src/routes/mod.rs` | rust-backend-engineer | Central routing and AppState |
| `backend/src/converters/*` | rust-backend-engineer | Format translation |
| `backend/src/streaming/*` | rust-backend-engineer | Event Stream parsing |
| `backend/src/auth/*` | rust-backend-engineer | Auth logic |
| `backend/src/web_ui/*` | rust-backend-engineer | Web UI API handlers |
| `backend/src/guardrails/*` | rust-backend-engineer | Guardrails engine |
| `frontend/src/pages/*` | react-frontend-engineer | UI pages |
| `frontend/src/components/*` | react-frontend-engineer | UI components |
| `frontend/src/lib/*` | react-frontend-engineer | Frontend utilities |
| `frontend/src/styles/*` | react-frontend-engineer | CSS styles |
| `backend/src/web_ui/config_db.rs` (DDL) | database-engineer | Schema migrations, table creation |
| `docker-compose*.yml` | devops-engineer | Docker config |
| `**/Dockerfile` | devops-engineer | Container builds |

### Change Request Protocol

When Agent B needs a change in Agent A's file:
1. Agent B sends a DM describing the change needed
2. Agent A evaluates and applies the change
3. Agent A confirms completion
4. Agent B proceeds with dependent task

---

## 3. Communication Protocols

### Direct Message (DM) — Default
For routine: task assignments, status updates, change requests, dependency notifications, blockers.

### Broadcast — Rare
Only for: blocking issues affecting multiple agents, architecture changes, track-level decisions.

### Anti-Patterns
- Never send structured JSON as status messages
- Never broadcast routine updates
- Never reference agents by ID — use names
- Never send empty/acknowledgment-only messages

---

## 4. Task Coordination Strategies

### Dependency Chain (Harbangan)

```
Backend Types/Models
       │
       ▼
Business Logic (converters, auth, streaming)
       │
       ▼
Route Handlers
       │
       ▼
Middleware Integration
       │
       ▼
Frontend API Client (apiFetch)
       │
       ▼
React Components/Pages
       │
       ▼
E2E Tests
```

### Parallel Work Opportunities

| Agent A | Agent B | Why It's Safe |
|---------|---------|---------------|
| Backend converter logic | Frontend UI mockup (agreed API shape) | Interface contract isolates work |
| Guardrails engine | Config persistence | Independent modules, separate files |
| Backend auth changes | Frontend CSS/styling | No runtime dependency |
| Backend unit tests | Frontend E2E test setup | Independent test infrastructure |

### Interface Contracts

Before parallel implementation, agree on API shape:
```typescript
// Backend will return:
{ "metrics": { "total_requests": 100, "avg_latency_ms": 45 } }

// Frontend will consume:
interface Metrics { totalRequests: number; avgLatencyMs: number; }
```

---

## 5. Harbangan Team Presets

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

---

## 6. Integration Patterns

### Vertical Slices (Preferred for small features)
Each agent builds complete stack for their feature slice.

### Horizontal Layers (For large features)
Each agent owns one architectural layer across all features.

### Hybrid (Recommended for complex tracks)
Phase 1: Shared infrastructure (horizontal). Phase 2: Feature slices (vertical). Phase 3: Integration testing.

| Factor | Vertical | Horizontal | Hybrid |
|--------|----------|------------|--------|
| Team size | 1-2 | 3+ | 3-5 |
| Coordination | Low | High | Medium |
| Time to first deliverable | Fast | Slow | Medium |
| Typical Harbangan use | Bug fix | Large refactor | New feature |

---

## 7. Agent Health & Respawn Protocols

### Context Exhaustion

Agents hit context window limits after processing many files/tasks. Symptoms:
- Repeated `idle_notification` messages with no task progress
- Process is running but agent does not respond to messages
- No file modifications despite having in_progress tasks

**Detection heuristic**: 3+ consecutive idle notifications + in_progress task + no file edits between them = context-exhausted.

### Respawn Protocol

When an agent is detected as context-exhausted:

1. Check `git log --oneline -20` for the agent's completed work
2. Note all in_progress and pending tasks from TaskList
3. Kill the agent process
4. Respawn via `/team-spawn --respawn-for {agent-name}` (reuses same name for ownership continuity)
5. New agent receives a handoff summary with completed commits and remaining tasks
6. Task ownership transfers automatically — no manual TaskUpdate needed

### Prevention

- **Limit task density** — max 4-5 subtasks per agent per wave. Split larger phases into sub-waves (1a, 1b) with respawn checkpoints between them.
- **Prefer many small tasks** over few large tasks. A phase with 7+ subtasks across many files will exhaust an agent's context before it can move to the next phase.
- **Proactive respawn** — after each phase completion, consider respawning the agent with a fresh context rather than waiting for exhaustion.

### Lazy Spawning

Only spawn agents when their tasks become unblocked:
- Wave 1 agents spawn immediately
- Wave 2+ agents are recorded as `deferred_agents` in team config and spawned when dependencies resolve
- This prevents 15+ minutes of idle resource consumption for blocked agents

---

## References

- [Messaging Patterns](references/messaging-patterns.md) — 8 structured message templates for inter-agent communication (task assignment, blocker reports, review findings, etc.)
- [Dependency Graphs](references/dependency-graphs.md) — 5 task dependency patterns (independent, sequential, diamond, fork-join, pipeline) with Harbangan-specific examples
- [Merge Strategies](references/merge-strategies.md) — 4 integration patterns (direct, sub-branch, trunk-based, worktree isolation) with Harbangan conflict prevention rules for backend/frontend parallel work
