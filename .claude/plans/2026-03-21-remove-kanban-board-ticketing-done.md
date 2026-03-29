# Plan: Remove GitHub Board/Kanban Ticketing from .claude/

## Context

The GitHub Project Board integration (Harbangan Board) is no longer needed. All references to board ticketing — including the kanban-master agent, board-related skills, and board sync logic embedded in other skills — should be removed. The goal is a cleaner multi-agent setup that uses only TaskList for task tracking, without the overhead of GitHub Issues/Project Board synchronization.

## Changes

### 1. Delete entire files/directories (6 items)

| Path | Reason |
|------|--------|
| `.claude/agents/kanban-master.md` | Entire agent is board orchestration |
| `.claude/skills/board-status/` (directory) | Entire skill is board status display |
| `.claude/skills/create-issue/` (directory) | Entire skill is board issue creation |
| `.claude/skills/close-issues/` (directory) | Entire skill is board issue closure |
| `.claude/agent-memory/kanban-master/MEMORY.md` | Memory for deleted agent |

### 2. Edit `.claude/agent-colors.json`

Remove the `kanban-master` entry. Result:

```json
{
  "rust-backend-engineer": "#4ECDC4",
  "react-frontend-engineer": "#45B7D1",
  "database-engineer": "#9B59B6",
  "backend-qa": "#FFA07A",
  "frontend-qa": "#DDA0DD",
  "devops-engineer": "#F0E68C",
  "document-writer": "#98D8C8"
}
```

### 3. Edit `.claude/CLAUDE.md`

- Update agent count from 8 to 7
- Remove `kanban-master.md` from structure tree
- Update skills count from 11 to 8
- Remove `create-issue/`, `board-status/`, `close-issues/` from structure tree
- Remove 3 board skill rows from Quick Reference table (create-issue, board-status, close-issues)

### 4. Edit `.claude/README.md`

- Update agent count from 8 to 7
- Remove "Orchestration Agent (1)" section (kanban-master table)
- Update skills count from 10 to 7
- Remove entire "GitHub Operations (4)" section → keep only `/merge-pr` and move it to a "Git Operations (1)" section
- Remove "Kanban Master Workflow" code block (lines 127-138)
- Remove GitHub Issues references from "Planning to Execution Flow" (lines 119, 121, 123)
- Clean up "How Plan Mode and Team Skills Connect" section to remove board sync references

### 5. Edit `.claude/skills/team-plan/SKILL.md`

- Remove entire "Phase 6: Create Board Items" section (lines 83-99)
- The plan output format (Phase 5) stays as-is — it's about writing plans, not board items

### 6. Edit `.claude/skills/team-implement/SKILL.md`

- Remove entire "Phase 5: GitHub Issues" section (lines 80-104)
- Remove board sync from Phase 8 Monitor (lines 144-147): delete the "Board sync" bullet
- Remove board references from Phase 11 Shutdown (lines 176-177): remove "Persist incomplete work to GitHub Issues" and "Update board status" bullets
- Remove "GitHub Issues created/closed" from Phase 12 Report (line 188)
- Remove "Blocked task detection" referencing GitHub Issue labels from Delegate section (line 210)
- Remove "Persist incomplete tasks to GitHub Issues" from Shutdown section (lines 213, 217)
- Renumber phases: Phase 6 becomes Phase 5, etc.

### 7. Edit `.claude/skills/team-review/SKILL.md`

- Remove the GitHub Issue creation block from Pre-flight Checks (lines 43-50)
- Remove "Update the review GitHub Issue board Status → Done" from Phase 8 Cleanup (line 234)

### 8. Edit `.claude/skills/team-debug/SKILL.md`

- Remove the GitHub Issue creation block from Phase 1 Initial Triage (lines 46-53)
- Remove "Update the debug GitHub Issue board Status → Done" from Phase 6.2 Cleanup (line 338)

### 9. Edit `.claude/skills/team-status/SKILL.md`

- Remove step 6: "Cross-reference TaskList vs GitHub Issues for drift" (line 36)

### 10. Edit `.claude/rules/plan-mode.md`

- Remove entire "Kanban Board Integration" section (lines 84-91)

### 11. Edit `.claude/rules/team-coordination.md`

- Change line 91 from "contract wins → tests decide → kanban-master arbitrates → file owner merges manually" to "contract wins → tests decide → file owner merges manually"

### 12. Edit root `CLAUDE.md` (if board references exist)

- Check for any board/kanban references and remove them

## Verification

After all edits:
1. Grep for remaining references: `rg -i "kanban|board-status|create-issue|close-issues|harbangan board|gh issue|gh project|github issue" .claude/`
2. Ensure no broken cross-references between skills/agents/rules
3. Confirm agent count is 7 everywhere, skill count is 8 everywhere (5 team + 1 git + 2 utility)
