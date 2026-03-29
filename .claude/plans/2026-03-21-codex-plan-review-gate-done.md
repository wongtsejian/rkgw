# Plan: Add Codex Plan Review Gate to /team-plan

## Context

After `/team-plan` produces a plan, we want an automatic review gate using Codex CLI before the user accepts the plan and runs `/team-implement`. Codex has a `team-review --plan` skill that checks feasibility, ownership, dependency order, verification gaps, and contradictions. However, Codex can hallucinate — so if Claude disagrees with Codex's findings after one adjustment round, escalate to the user.

## Feasibility Research

**Codex CLI supports non-interactive execution:**
```bash
codex exec \
  -s read-only \
  -o /tmp/codex-review-output.md \
  "team-review --plan .claude/plans/{plan-file}.md"
```

Key flags:
- `codex exec` — non-interactive mode
- `-s read-only` — sandbox: no file writes (review is read-only)
- `-o <file>` — write last agent message to file (the review output)
- `--json` — optional JSONL event stream to stdout
- `--ephemeral` — don't persist session files

**Context sharing via ephemeral file:**
- Codex writes review to a temp file (e.g., `.claude/plans/{plan-name}-codex-review.md`)
- Claude reads the review file and decides: accept, adjust, or escalate
- File is deleted after the review cycle completes

## Changes

### Edit `.claude/skills/team-plan/SKILL.md`

Add **Phase 6: Codex Plan Review Gate** after Phase 5 (Plan Output):

```markdown
## Phase 6: Codex Plan Review Gate

After writing the plan file, invoke Codex CLI to review it:

### 6.1 Run Codex Review

```bash
PLAN_FILE=".claude/plans/{plan-file}.md"
REVIEW_FILE=".claude/plans/{plan-name}-codex-review.md"

codex exec \
  -s read-only \
  --ephemeral \
  -o "$REVIEW_FILE" \
  "team-review --plan $PLAN_FILE"
```

### 6.2 Evaluate Review

Read the Codex review file and classify each finding:

| Severity | Action |
|----------|--------|
| **high** | Must address — adjust the plan |
| **medium** | Evaluate — adjust if valid, note if not |
| **low/info** | Acknowledge, no plan change needed |

### 6.3 Adjustment Loop (max 1 round)

If Codex found high/medium issues:
1. Evaluate each finding against the codebase (read relevant files)
2. If the finding is valid: adjust the plan and re-run Codex review
3. If the finding is a hallucination (Codex citing nonexistent code/patterns):
   - Note it as "disputed" in the review summary
   - Do NOT adjust the plan for hallucinated findings

**Only 1 adjustment round.** After the second Codex review:
- If Codex still insists on disputed findings → escalate to user
- If Codex approves or only has low/info → proceed

### 6.4 Escalation

If Codex and Claude disagree after 1 round, present both perspectives
to the user via AskUserQuestion:

```
Codex review found issues that I believe are incorrect:

1. [Codex finding]: "..."
   [My assessment]: "This is not applicable because..."

2. [Codex finding]: "..."
   [My assessment]: "This is valid, already adjusted"

How should we proceed?
- Accept plan as-is (override Codex)
- Adjust plan per Codex suggestions
- Let me review the specific findings
```

### 6.5 Clean Up

Delete the ephemeral review file after the gate passes:
```bash
rm -f "$REVIEW_FILE"
```

### 6.6 Gate Result

The plan is approved for `/team-implement` only when:
1. Codex review has no unresolved high findings, AND
2. User has accepted the plan (via ExitPlanMode)
```

### Update Phase 5 plan output format

Add a `## Review Status` section to the plan file template:

```markdown
## Review Status
- Codex review: {passed / adjusted / escalated}
- Findings addressed: {count}
- Disputed findings: {count}
```

## Files to Edit

| File | Change |
|------|--------|
| `.claude/skills/team-plan/SKILL.md` | Add Phase 6 (Codex review gate) |

## Verification

1. Run `/team-plan` on a test feature — confirm it invokes `codex exec` after writing the plan
2. Verify Codex review output is written to the ephemeral file
3. Verify Claude reads and evaluates the review
4. Test escalation path: if Codex flags something Claude disagrees with, confirm user is asked
