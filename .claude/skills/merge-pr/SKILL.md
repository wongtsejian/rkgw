---
name: merge-pr
description: |
  Squash-merge the current branch's PR, clean up local/remote branches, prune
  worktrees, and return to main. Use when user says 'merge', 'merge this PR',
  'squash merge', 'merge and cleanup', or 'land this PR'.
argument-hint: "[pr-number]"
disable-model-invocation: true
allowed-tools:
  - Bash
  - AskUserQuestion
---

# Merge PR

Squash-merge, clean up branches, and return to main.

## Current State

- Branch: !`git branch --show-current`
- PR: !`gh pr view --json number,title,state,statusCheckRollup --jq '{number,title,state,checks: [.statusCheckRollup[] | {name: .name, status: .status, conclusion: .conclusion}]}'`

## Steps

1. **Resolve PR number** — use `$ARGUMENTS` if provided, otherwise detect from current branch:
   ```bash
   gh pr view --json number -q '.number'
   ```
   If no PR exists or already merged, inform user and stop.

2. **Check CI status** — if any required checks are failing, warn and ask for confirmation before proceeding.

3. **Check for uncommitted changes** — `git status --porcelain`. If dirty, warn and stop.

4. **Merge** — squash merge and delete remote branch:
   ```bash
   gh pr merge <number> --squash --delete-branch
   ```
   If squash fails, try `--rebase`. Never use `--merge` (disabled on this repo).

5. **Switch to main**:
   ```bash
   git checkout main
   ```

6. **Delete local branch**:
   ```bash
   git branch -d <branch-name>
   ```
   If `-d` fails (not fully merged warning), ask user before using `-D`.

7. **Clean up worktrees and pull**:
   ```bash
   git worktree prune
   git pull --prune
   ```

8. **Report** — PR number/title merged, branches deleted, current branch is main.
