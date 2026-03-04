#!/bin/sh
set -e

# Proxy-only entrypoint for rkgw backend.
# Validates required env vars and launches the gateway binary.
# KIRO_REFRESH_TOKEN is optional — if not set, the device code flow
# will run at startup and print a URL to authorize in your browser.

if [ -z "$PROXY_API_KEY" ]; then
    echo "ERROR: PROXY_API_KEY is required" >&2
    exit 1
fi

exec /app/kiro-gateway
