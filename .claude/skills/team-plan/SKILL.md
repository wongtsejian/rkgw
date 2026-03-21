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
```
