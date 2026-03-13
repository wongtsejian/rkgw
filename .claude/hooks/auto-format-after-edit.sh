#!/bin/bash
# PostToolUse hook: auto-format files after Edit/Write (async, non-blocking)
INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

if [ -z "$FILE_PATH" ] || [ ! -f "$FILE_PATH" ]; then
  exit 0
fi

case "$FILE_PATH" in
  *.rs)
    rustfmt "$FILE_PATH" 2>/dev/null || true
    ;;
  *.ts|*.tsx|*.js|*.jsx|*.css)
    PROJ_DIR="$(echo "$FILE_PATH" | sed 's|/frontend/.*|/frontend|')"
    if [ -d "$PROJ_DIR" ]; then
      cd "$PROJ_DIR" && npx prettier --write "$FILE_PATH" 2>/dev/null || true
    fi
    ;;
esac

exit 0
