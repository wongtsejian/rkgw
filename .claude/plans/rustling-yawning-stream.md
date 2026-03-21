# Plan: Remove Worktree Logic, Add Branch Creation to team-plan

## Context

With clear agent ownership boundaries and file ownership enforcement hooks in place, worktree isolation is no longer needed — agents can safely work on the same branch since they own distinct files. All worktree references should be removed, and `/team-plan` should specify creating a feature branch for each implementation.

## Changes

### 1. Delete `.claude/hooks/worktree-guard-stop.sh`

Entire file — no longer needed.

### 2. Edit `.claude/settings.json`

Remove the entire `Stop` hook block (lines 67-77):
```json
"Stop": [
  {
    "matcher": "",
    "hooks": [
      {
        "type": "command",
        "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/worktree-guard-stop.sh"
      }
    ]
  }
]
```

### 3. Edit `.claude/skills/team-implement/SKILL.md`

- **Remove Phase 3 (Worktree Resolution)** entirely (lines 49-59)
- **Replace Phase 9 (PR)** — remove worktree cd, use normal branch workflow:
  ```markdown
  ### Phase 8: PR

  Create PR from the feature branch:
  ```bash
  git add -A && git commit -m "feat(scope): description"
  git push -u origin feat/{feature-slug}
  gh pr create --title "feat: ..." --body "## Summary\n..."
  ```
  ```
- **Renumber phases**: Phase 4→3, Phase 5→4, etc.

### 4. Edit `.claude/skills/team-shutdown/SKILL.md`

- **Update description** — remove "cleans up worktrees"
- **Simplify Step 3 (Save Work)** — remove worktree cd, use current directory:
  ```markdown
  ### 3. Save Work

  1. Check for uncommitted changes: `git status --porcelain`
  2. If changes exist, commit them
  3. Check for unpushed commits: `git log @{u}.. --oneline`
  4. If unpushed, push them
  ```
- **Simplify Step 5 (Clean Up)** — remove worktree removal commands, keep only TeamDelete
- **Update Step 6 (Report)** — remove worktree line from output template

### 5. Edit `.claude/skills/merge-pr/SKILL.md`

- **Update description** — remove "prune worktrees"
- **Replace Step 7** — remove `git worktree prune`, keep `git pull --prune`:
  ```markdown
  7. **Pull latest**:
     ```bash
     git pull --prune
     ```
  ```

### 6. Edit `.claude/skills/team-plan/SKILL.md`

- **Add branch creation guidance** to Phase 5 (Plan Output). Add item 6:
  ```
  6. **Branch Name** — `feat/{feature-slug}` or `fix/{feature-slug}` per git workflow conventions
  ```
- **Add branch name to plan file format** template

### 7. Edit `.claude/rules/team-coordination.md`

- **Remove "Worktree isolation" row** from Integration Patterns table (line 85)
- **Remove "Lazy spawning" line** (line 97) — already removed presets, this is stale

## Files Summary

| File | Action |
|------|--------|
| `.claude/hooks/worktree-guard-stop.sh` | Delete |
| `.claude/settings.json` | Remove Stop hook block |
| `.claude/skills/team-implement/SKILL.md` | Remove Phase 3 (Worktree), update Phase 9 (PR), renumber |
| `.claude/skills/team-shutdown/SKILL.md` | Remove worktree logic from Save Work and Clean Up |
| `.claude/skills/merge-pr/SKILL.md` | Remove worktree prune from cleanup step |
| `.claude/skills/team-plan/SKILL.md` | Add branch creation to plan output |
| `.claude/rules/team-coordination.md` | Remove worktree row from integration patterns |

## Verification

1. `grep -ri "worktree\|\.trees/" .claude/` — should return nothing (except historical plans)
2. Confirm settings.json is valid JSON after Stop hook removal
