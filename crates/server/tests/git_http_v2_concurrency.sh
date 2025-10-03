#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)

DB_DIR=$(mktemp -d)
REPOS_DIR=$(mktemp -d -p /tmp)
EXT_DIR="$ROOT_DIR/extensions"
PORT=${PORT:-$(( ( RANDOM % 10000 ) + 30000 ))}
SERVER_URL=${SERVER_URL:-http://127.0.0.1:$PORT}
export SERVER_URL REPO_NAME
REPO_NAME=${REPO_NAME:-alpha}
N=${N:-5}

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
for i in {1..50}; do echo $i >> "$TMP/log"; done
git -C "$TMP" add log; git -C "$TMP" -c user.email=t@e -c user.name=t commit -m seed >/dev/null
git -C "$TMP" branch -M main >/dev/null
git -C "$TMP" remote add origin "$REPOS_DIR/$REPO_NAME.git"
git -C "$TMP" push origin main >/dev/null

run_clone() {
  local d; d=$(mktemp -d)
  git -c protocol.version=2 clone "$SERVER_URL/$REPO_NAME" "$d/repo" >/dev/null
}

export -f run_clone
seq 1 "$N" | xargs -n1 -P "$N" bash -lc run_clone
echo "ok"
