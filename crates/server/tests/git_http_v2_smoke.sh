#!/usr/bin/env bash
set -euo pipefail

# Smoke test for Smart HTTP v2 advertise endpoint.
# Requirements: git, curl

PORT=${PORT:-$(( ( RANDOM % 10000 ) + 30000 ))}
SERVER_URL=${SERVER_URL:-http://127.0.0.1:$PORT}
REPO_NAME=${REPO_NAME:-alpha}

echo "[git-http-v2] checking info/refs advertise for $REPO_NAME"
code=$(curl -s -o /tmp/adv.bin -w "%{http_code}" \
  -H 'Git-Protocol: version=2' \
  "$SERVER_URL/$REPO_NAME/info/refs?service=git-upload-pack")

test "$code" = "200" || { echo "unexpected status $code"; exit 1; }

if ! hexdump -C /tmp/adv.bin | head -n 2 | grep -q "version 2"; then
  echo "missing v2 banner in advertise"
  exit 1
fi

echo "[git-http-v2] advertise looks OK"
