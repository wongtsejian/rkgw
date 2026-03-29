# Plan: Remove Write from team-debug and team-review

## Context

`/team-debug` and `/team-review` are read-only investigation skills — investigators and reviewers should never modify code. Both currently have `Write` in their `allowed-tools`. Remove it.

## Changes

1. `.claude/skills/team-debug/SKILL.md` — remove `- Write` from allowed-tools (line 10)
2. `.claude/skills/team-review/SKILL.md` — remove `- Write` from allowed-tools (line 10)
