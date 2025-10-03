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

echo "[git-http-v2] starting server with smart v2 (git backend)"
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
  echo $SERVER_PID > /tmp/forge-server.pid
)

echo "[git-http-v2] waiting for server to become ready..."
for i in {1..60}; do
  if curl -s "$SERVER_URL" >/dev/null 2>&1; then break; fi
  sleep 0.2
done

echo "[git-http-v2] preparing a bare repo with one commit"
mkdir -p "$REPOS_DIR/$REPO_NAME.git"
git init --bare "$REPOS_DIR/$REPO_NAME.git" >/dev/null
touch "$REPOS_DIR/$REPO_NAME.git/git-daemon-export-ok"

WORK=$(mktemp -d)
git -C "$WORK" init >/dev/null
echo hello > "$WORK/README.md"
git -C "$WORK" add README.md >/dev/null
git -C "$WORK" -c user.email=test@example.com -c user.name=test commit -m "init" >/dev/null
git -C "$WORK" branch -M main >/dev/null
git -C "$WORK" remote add origin "$REPOS_DIR/$REPO_NAME.git"
git -C "$WORK" push origin main >/dev/null

echo "[git-http-v2] ls-remote (v2)"
git -c protocol.version=2 ls-remote "$SERVER_URL/$REPO_NAME" >/dev/null

echo "[git-http-v2] clone (v2)"
DEST=$(mktemp -d)
set +e
git -c protocol.version=2 clone "$SERVER_URL/$REPO_NAME" "$DEST/repo" 2>/tmp/git-clone.err
STATUS=$?
set -e

if [[ $STATUS -ne 0 ]]; then
  echo "Clone failed (expected if pure-Rust pack not implemented). Switching to git backend confirms path works."
  # We already run with git backend; failure here is unexpected.
  echo "--- git stderr ---"; cat /tmp/git-clone.err; echo "-------------------"
  exit 1
fi

echo "[git-http-v2] clone succeeded"

echo "[git-http-v2] push should be forbidden"
set +e
git -C "$DEST/repo" -c http.extraheader="Git-Protocol: version=2" push "$SERVER_URL/$REPO_NAME" HEAD:main >/tmp/git-push.out 2>&1
PUSH_STATUS=$?
set -e

if [[ $PUSH_STATUS -eq 0 ]]; then
  echo "Push unexpectedly succeeded"
  cat /tmp/git-push.out
  exit 1
fi

echo "[git-http-v2] done"
