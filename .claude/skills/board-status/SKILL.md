---
name: board-status
description: |
  Show the Harbangan Project Board status — open issues grouped by status
  column with priority and size. Use when user says 'board status',
  'show board', 'what is on the board', 'project status', 'kanban status',
  or 'show open issues'.
argument-hint: "[--status backlog|ready|in-progress|in-review|done] [--service backend|frontend|infra|e2e]"
allowed-tools:
  - Bash
---

# Board Status

Display the Harbangan Board as a formatted table.

## Steps

1. **Fetch board items**:
   ```bash
   gh project item-list 3 --owner if414013 --format json \
     --jq '.items[] | {title: .content.title, number: .content.number, url: .content.url, status: .status, priority: .priority, size: .size, labels: [.labels[]?.name] }'
   ```

2. **Apply filters** from `$ARGUMENTS`:
   - `--status <column>`: filter by Status field (backlog, ready, in-progress, in-review, done)
   - `--service <name>`: filter by `service:{name}` label

3. **Format output** as a markdown table grouped by status column:
   ```
   ## In Progress (3)
   | # | Title | Priority | Size | Service |
   |---|-------|----------|------|---------|
   | #93 | Fix integration test | P0 | S | backend |
   ...

   ## Ready (2)
   ...
   ```

4. **Show summary** — total open issues, count per status column.

## Board Constants

- Owner: `if414013`
- Project number: `3`
