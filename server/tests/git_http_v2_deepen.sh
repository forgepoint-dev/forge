#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)

DB_DIR=$(mktemp -d)
REPOS_DIR=$(mktemp -d -p /tmp)
EXT_DIR="$ROOT_DIR/extensions"
PORT=${PORT:-$(( ( RANDOM % 10000 ) + 30000 ))}
SERVER_URL=${SERVER_URL:-http://127.0.0.1:$PORT}
REPO_NAME=${REPO_NAME:-alpha}

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then kill "$SERVER_PID" 2>/dev/null || true; fi
  rm -rf "$DB_DIR" "$REPOS_DIR" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Start server with Rust backend
(
  cd "$ROOT_DIR"/..
  FORGE_DB_PATH="$DB_DIR" \
  FORGE_REPOS_PATH="$REPOS_DIR" \
  FORGE_EXTENSIONS_DIR="$EXT_DIR" \
  FORGE_GIT_HTTP_MODE=smart \
  FORGE_GIT_HTTP_EXPORT_ALL=true \
  FORGE_GIT_SMART_V2_BACKEND=rust \
  FORGE_LISTEN_ADDR="127.0.0.1:$PORT" \
  cargo run --manifest-path server/Cargo.toml --bin server >/tmp/forge-server.log 2>&1 &
  SERVER_PID=$!
)

for i in {1..60}; do
  if curl -s "$SERVER_URL/healthz" >/dev/null 2>&1; then break; fi
  sleep 0.2
done

# Create a repo with a few commits
mkdir -p "$REPOS_DIR/$REPO_NAME.git"
git init --bare "$REPOS_DIR/$REPO_NAME.git" >/dev/null
touch "$REPOS_DIR/$REPO_NAME.git/git-daemon-export-ok"

TMP=$(mktemp -d)
git -C "$TMP" init >/dev/null
for i in {1..5}; do
  echo $i >> "$TMP/file"; git -C "$TMP" add file >/dev/null
  git -C "$TMP" -c user.email=t@e -c user.name=t commit -m "c$i" >/dev/null
done
git -C "$TMP" branch -M main >/dev/null
git -C "$TMP" remote add origin "$REPOS_DIR/$REPO_NAME.git"
git -C "$TMP" push origin main >/dev/null

# Shallow clone and then deepen
DEST=$(mktemp -d)
GIT_TRACE_PACKET=1 git -c protocol.version=2 clone --depth=1 "$SERVER_URL/$REPO_NAME" "$DEST/repo" >/tmp/deepen-clone.out 2>/tmp/deepen-clone.err
GIT_TRACE_PACKET=1 git -C "$DEST/repo" -c protocol.version=2 fetch --deepen=2 origin >/tmp/deepen-fetch.out 2>/tmp/deepen-fetch.err

echo "[git-http-v2-deepen] ok"
