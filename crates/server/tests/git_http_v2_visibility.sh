#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)

DB_DIR=$(mktemp -d)
REPOS_DIR=$(mktemp -d)
EXT_DIR="$ROOT_DIR/extensions"
SERVER_URL=${SERVER_URL:-http://localhost:8000}
REPO_NAME=${REPO_NAME:-alpha}

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then kill "$SERVER_PID" 2>/dev/null || true; fi
  rm -rf "$DB_DIR" "$REPOS_DIR" >/dev/null 2>&1 || true
}
trap cleanup EXIT

(
  cd "$ROOT_DIR"/..
  FORGE_DB_PATH="$DB_DIR" \
  FORGE_REPOS_PATH="$REPOS_DIR" \
  FORGE_EXTENSIONS_DIR="$EXT_DIR" \
  FORGE_GIT_HTTP_MODE=smart \
  cargo run --manifest-path crates/server/Cargo.toml --bin server >/tmp/forge-server.log 2>&1 &
  SERVER_PID=$!
)

for i in {1..60}; do
  if curl -s "$SERVER_URL" >/dev/null 2>&1; then break; fi
  sleep 0.2
done

mkdir -p "$REPOS_DIR/$REPO_NAME.git"
git init --bare "$REPOS_DIR/$REPO_NAME.git" >/dev/null

# Without export-ok, should be 404
code=$(curl -s -o /dev/null -w "%{http_code}" "$SERVER_URL/$REPO_NAME/info/refs?service=git-upload-pack")
[[ "$code" = "404" ]] || { echo "expected 404 without export-ok"; exit 1; }

# Mark public
touch "$REPOS_DIR/$REPO_NAME.git/git-daemon-export-ok"
code=$(curl -s -o /dev/null -w "%{http_code}" "$SERVER_URL/$REPO_NAME/info/refs?service=git-upload-pack")
[[ "$code" = "200" ]] || { echo "expected 200 with export-ok"; exit 1; }

echo "ok"
