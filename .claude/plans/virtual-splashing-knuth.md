# Plan: GH Operation Skills (SKILL.md format)

## Files to create/modify

- `.claude/skills/merge-pr/SKILL.md` — **create** (replace `instructions.md`), inline execution, `allowed-tools: [Bash, AskUserQuestion]`, `disable-model-invocation: true`
- `.claude/skills/merge-pr/instructions.md` — **delete**
- `.claude/skills/create-issue/SKILL.md` — **create**, forks to `kanban-master` agent. Accepts `$ARGUMENTS` as issue description. Reads board constants from kanban-master agent def. `allowed-tools: [Bash, Read, Grep, Glob, AskUserQuestion, Agent]`
- `.claude/skills/board-status/SKILL.md` — **create**, inline `gh project item-list` + formatting. `allowed-tools: [Bash]`, `argument-hint: "[--filter label] [--status column]"`
- `.claude/skills/close-issues/SKILL.md` — **create**, inline `gh issue close` with PR link. `allowed-tools: [Bash, AskUserQuestion]`, `argument-hint: "<issue-numbers> [--pr N]"`, `disable-model-invocation: true`

## Frontmatter pattern (match existing skills)
Each SKILL.md uses: `name`, `description` (keyword-rich, includes trigger phrases), `argument-hint`, `allowed-tools`, `disable-model-invocation` for destructive ops.

## Skill details

### merge-pr (migrate from instructions.md)
- Same logic as current `instructions.md`, reformatted with proper YAML frontmatter
- `disable-model-invocation: true` — user controls when to merge
- Injects live context: `!`git branch --show-current`` and `!`gh pr view --json number,title,state``

### create-issue
- Spawns `kanban-master` agent (`context: fork`, `agent: kanban-master`) for board field setup
- Accepts: `$0` = title, `$1` = service label, optional `--priority`, `--size`
- Agent reads board constants from `.claude/agents/kanban-master.md:69-82`
- If no args, uses `AskUserQuestion` to gather title, service, priority, size

### board-status
- Inline `gh project item-list 3 --owner if414013 --format json` + format as table
- Filters: `--status "In progress"`, `--label "service:backend"`, or show all
- No side effects — safe for Claude auto-invocation

### close-issues
- `disable-model-invocation: true` — destructive (closes issues)
- Accepts comma-separated issue numbers + optional `--pr N` for linking
- Confirms with user before closing
- Updates board status to Done

## Verification
```bash
ls .claude/skills/*/SKILL.md | wc -l  # expect 7 (humanizer, rename-plan, team-*, merge-pr, create-issue, board-status, close-issues)
```
