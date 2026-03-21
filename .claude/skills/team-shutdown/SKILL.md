---
name: team-shutdown
description: Gracefully shut down a running agent team. Commits pending changes and terminates agents. Use when user says 'shutdown team', 'stop agents', 'kill team', 'terminate agents', or 'clean up team'.
argument-hint: "[team-name]"
allowed-tools:
  - Bash
  - Read
  - Grep
  - Glob
  - SendMessage
  - AskUserQuestion
  - TeamDelete
  - TaskList
  - TaskGet
---

# Team Shutdown

Gracefully shut down a running agent team. Saves work, terminates agents, and cleans up resources.

## Steps

### 1. Resolve Team

If `team-name` provided in `$ARGUMENTS`, use it. Otherwise:
1. Check `~/.claude/teams/` for active teams
2. If exactly one team exists, use it
3. If multiple teams exist, ask the user via AskUserQuestion which team to shut down

### 2. Pre-Shutdown Summary

Read team config (`~/.claude/teams/{team-name}/config.json`) and TaskList to display:

```
Team: {team-name}
Members: {count} agents

Agent          | Status      | Current Task
---------------|-------------|-------------------
backend-eng    | Idle        | (completed: 3 tasks)
frontend-eng   | In Progress | Build settings page
backend-qa     | Idle        | (completed: 1 task)

Tasks: {completed}/{total} completed
Pending: {list of incomplete tasks}
```

If there are incomplete tasks, warn the user and ask for confirmation via AskUserQuestion.

### 3. Save Work

1. Check for uncommitted changes:
   ```bash
   git status --porcelain
   ```
2. If changes exist, commit them:
   ```bash
   git add -A && git commit -m "chore: save work-in-progress before team shutdown"
   ```
3. Check for unpushed commits:
   ```bash
   git log @{u}.. --oneline 2>/dev/null
   ```
4. If unpushed commits exist, push them:
   ```bash
   git push
   ```

### 4. Terminate Agents

Send `shutdown_request` to all teammates in order:
1. **Workers first** — all non-coordinator agents
2. **Coordinator last** — if one exists

```
SendMessage({
  to: "{agent-name}",
  message: { type: "shutdown_request", reason: "Team shutdown requested by user" }
})
```

Wait briefly for each agent to acknowledge.

### 5. Clean Up Resources

1. `TeamDelete` to remove team config and task directories

### 6. Report

```
Shutdown complete:
- Team: {team-name}
- Agents terminated: {count}
- Tasks completed: {n}/{total}
- Branch: {branch-name} (pushed / not pushed)
```
