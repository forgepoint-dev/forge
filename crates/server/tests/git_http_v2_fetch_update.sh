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

(
  cd "$ROOT_DIR"/..
  FORGE_DB_PATH="$DB_DIR" \
  FORGE_REPOS_PATH="$REPOS_DIR" \
  FORGE_EXTENSIONS_DIR="$EXT_DIR" \
  FORGE_GIT_HTTP_MODE=smart \
  FORGE_GIT_HTTP_EXPORT_ALL=true \
  FORGE_GIT_SMART_V2_BACKEND=git \
  FORGE_LISTEN_ADDR="127.0.0.1:$PORT" \
  cargo run --manifest-path crates/server/Cargo.toml --bin server >/tmp/forge-server.log 2>&1 &
  SERVER_PID=$!
)

for i in {1..60}; do
  if curl -s "$SERVER_URL" >/dev/null 2>&1; then break; fi
  sleep 0.2
done

mkdir -p "$REPOS_DIR/$REPO_NAME.git"
git init --bare "$REPOS_DIR/$REPO_NAME.git" >/dev/null

TMP=$(mktemp -d)
git -C "$TMP" init >/dev/null
echo 1 > "$TMP/file"; git -C "$TMP" add file; git -C "$TMP" -c user.email=t@e -c user.name=t commit -m c1 >/dev/null
git -C "$TMP" branch -M main >/dev/null
git -C "$TMP" remote add origin "$REPOS_DIR/$REPO_NAME.git"
git -C "$TMP" push origin main >/dev/null

DEST=$(mktemp -d)
git -c protocol.version=2 clone "$SERVER_URL/$REPO_NAME" "$DEST/repo" >/dev/null

# Add another commit to origin and fetch
echo 2 >> "$TMP/file"; git -C "$TMP" commit -am c2 >/dev/null
git -C "$TMP" push origin main >/dev/null

git -C "$DEST/repo" -c protocol.version=2 fetch origin >/dev/null
git -C "$DEST/repo" rev-parse FETCH_HEAD >/dev/null
echo "ok"
