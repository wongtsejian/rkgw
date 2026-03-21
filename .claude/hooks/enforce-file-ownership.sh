#!/bin/bash
# PreToolUse hook: enforces file ownership per agent.
# Blocks Write/Edit on files outside the agent's owned scope.
# Only active when running as a spawned agent (CLAUDE_AGENT_NAME is set).

INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')
AGENT="$CLAUDE_AGENT_NAME"

# Skip if not a spawned agent or no file path
if [ -z "$AGENT" ] || [ -z "$FILE_PATH" ]; then
  exit 0
fi

# Normalize to relative path
if [ -n "$CLAUDE_PROJECT_DIR" ]; then
  FILE_PATH=$(realpath --relative-to="$CLAUDE_PROJECT_DIR" "$FILE_PATH" 2>/dev/null || echo "$FILE_PATH")
fi

deny() {
  cat <<ENDJSON
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"$1"}}
ENDJSON
  exit 0
}

case "$AGENT" in
  rust-backend-engineer)
    echo "$FILE_PATH" | grep -qE '^backend/' && exit 0
    deny "rust-backend-engineer can only edit backend/**. DM the file owner for: $FILE_PATH"
    ;;
  react-frontend-engineer)
    echo "$FILE_PATH" | grep -qE '^frontend/' && exit 0
    deny "react-frontend-engineer can only edit frontend/**. DM the file owner for: $FILE_PATH"
    ;;
  database-engineer)
    echo "$FILE_PATH" | grep -qE 'config_db\.rs$' && exit 0
    deny "database-engineer can only edit config_db.rs. DM the file owner for: $FILE_PATH"
    ;;
  devops-engineer)
    echo "$FILE_PATH" | grep -qE '(docker-compose|Dockerfile|\.env\.example|entrypoint)' && exit 0
    deny "devops-engineer can only edit Docker/infra files. DM the file owner for: $FILE_PATH"
    ;;
  backend-qa)
    echo "$FILE_PATH" | grep -qE '^backend/src/' && exit 0
    deny "backend-qa can only edit backend/src/** (test modules). DM the file owner for: $FILE_PATH"
    ;;
  frontend-qa)
    echo "$FILE_PATH" | grep -qE '^e2e-tests/' && exit 0
    deny "frontend-qa can only edit e2e-tests/**. DM the file owner for: $FILE_PATH"
    ;;
  document-writer)
    deny "document-writer is read-only for source code. Create documentation files only."
    ;;
esac

exit 0
