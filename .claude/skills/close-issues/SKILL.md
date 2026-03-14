---
name: close-issues
description: |
  Close one or more GitHub Issues with an optional PR link, and update the
  Harbangan Board status to Done. Use when user says 'close issue',
  'close issues', 'mark as done', 'resolve issues', or 'close these tickets'.
argument-hint: "<issue-numbers> [--pr N]"
disable-model-invocation: true
allowed-tools:
  - Bash
  - AskUserQuestion
---

# Close Issues

Close GitHub Issues and update board status to Done.

## Inputs

Parse `$ARGUMENTS` for:
- **Issue numbers**: comma or space-separated (e.g., `93 94 95` or `93,94,95`)
- **--pr N**: PR number to reference in close comment (optional)

If no arguments provided, use `AskUserQuestion` to ask which issues to close.

## Steps

1. **List the issues** — fetch details for each issue number:
   ```bash
   gh issue view <number> --json number,title,state -q '{number,title,state}'
   ```

2. **Confirm with user** — show the issues that will be closed and ask for confirmation:
   ```
   About to close:
   - #93: Fix integration test compilation (open)
   - #94: Add missing config_api tests (open)
   Proceed?
   ```

3. **Close each issue** with comment:
   ```bash
   gh issue close <number> --comment "Resolved in PR #<pr-number>"
   ```
   If no `--pr` provided, close without PR reference:
   ```bash
   gh issue close <number> --comment "Resolved."
   ```

4. **Update board status to Done** for each closed issue:
   ```bash
   ITEM_ID=$(gh project item-list 3 --owner if414013 --format json \
     --jq ".items[] | select(.content.number == <number>) | .id")
   gh project item-edit --project-id PVT_kwHOATKEhs4BRp0j --id $ITEM_ID \
     --field-id PVTSSF_lAHOATKEhs4BRp0jzg_azo8 --single-select-option-id 98236657
   ```

5. **Report** — list closed issues with their URLs.
