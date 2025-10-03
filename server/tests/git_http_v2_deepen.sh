#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)

DB_DIR=$(mktemp -d)
REPOS_DIR=$(mktemp -d -p /tmp)
EXT_DIR="$ROOT_DIR/extensions"
PORT=${PORT:-$(( ( RANDOM % 10000 ) + 30000 ))}
SERVER_URL=${SERVER_URL:-http://127.0.0.1:$PORT}
REPO_NAME=${REPO_NAME:-alpha}
SERVER_LOG=${SERVER_LOG:-$(mktemp /tmp/forge-server.XXXXXX.log)}
HEALTH_ATTEMPTS=${SERVER_HEALTH_ATTEMPTS:-300}
HEALTH_INTERVAL=${SERVER_HEALTH_INTERVAL:-0.2}

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    if kill -0 "$SERVER_PID" 2>/dev/null; then
      kill "$SERVER_PID" 2>/dev/null || true
    fi
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  rm -rf "$DB_DIR" "$REPOS_DIR" >/dev/null 2>&1 || true
  if [[ -z "${KEEP_SERVER_LOG:-}" ]]; then
    rm -f "$SERVER_LOG" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

# Start server with Rust backend via nix develop for consistent tooling
start_server() {
  local pidfile
  pidfile=$(mktemp)
  (
    cd "$ROOT_DIR"/..
    FORGE_DB_PATH="$DB_DIR" \
    FORGE_REPOS_PATH="$REPOS_DIR" \
    FORGE_EXTENSIONS_DIR="$EXT_DIR" \
    FORGE_GIT_HTTP_MODE=smart \
    FORGE_GIT_HTTP_EXPORT_ALL=true \
    FORGE_GIT_SMART_V2_BACKEND=rust \
    FORGE_LISTEN_ADDR="127.0.0.1:$PORT" \
    nix develop --impure -c cargo run --manifest-path server/Cargo.toml --bin server \
      >"$SERVER_LOG" 2>&1 &
    echo $! >"$pidfile"
  )
  SERVER_PID=$(cat "$pidfile")
  rm -f "$pidfile"
}

wait_for_server() {
  local attempt=1
  while (( attempt <= HEALTH_ATTEMPTS )); do
    if curl -fsS "$SERVER_URL/healthz" >/dev/null 2>&1; then
      return 0
    fi
    if [[ -n "${SERVER_PID:-}" ]] && ! kill -0 "$SERVER_PID" 2>/dev/null; then
      echo "forge server exited before becoming healthy" >&2
      tail -n 200 "$SERVER_LOG" >&2 || true
      return 1
    fi
    sleep "$HEALTH_INTERVAL"
    attempt=$(( attempt + 1 ))
  done
  echo "forge server failed to report healthy after $HEALTH_ATTEMPTS attempts" >&2
  tail -n 200 "$SERVER_LOG" >&2 || true
  return 1
}

start_server
wait_for_server

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
