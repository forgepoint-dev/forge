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
    nix develop --impure -c cargo run --manifest-path crates/server/Cargo.toml --bin server \
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

mkdir -p "$REPOS_DIR/$REPO_NAME.git"
git init --bare "$REPOS_DIR/$REPO_NAME.git" >/dev/null
touch "$REPOS_DIR/$REPO_NAME.git/git-daemon-export-ok"

TMP=$(mktemp -d)
git -C "$TMP" init >/dev/null
# Build nested tree structure depth=3 with several blobs
mkdir -p "$TMP/a/b/c"
echo x > "$TMP/root.txt"
echo y > "$TMP/a/a.txt"
echo z > "$TMP/a/b/b.txt"
echo w > "$TMP/a/b/c/c.txt"
git -C "$TMP" add -A >/dev/null
git -C "$TMP" -c user.email=t@e -c user.name=t commit -m "deep tree" >/dev/null
git -C "$TMP" branch -M main >/dev/null
git -C "$TMP" remote add origin "$REPOS_DIR/$REPO_NAME.git"
git -C "$TMP" push origin main >/dev/null

# Baseline clone
DEST1=$(mktemp -d)
git -c protocol.version=2 clone --no-checkout "$SERVER_URL/$REPO_NAME" "$DEST1/repo" >/dev/null 2>&1
BASE_SIZE=0
for pack in "$DEST1/repo/.git/objects/pack"/*.pack; do
  if [[ -f "$pack" ]]; then
    size=$(stat -c %s "$pack" 2>/dev/null || wc -c < "$pack")
    BASE_SIZE=$((BASE_SIZE + size))
  fi
done

# Filtered tree:1 clone
DEST2=$(mktemp -d)
if [[ -n "${DEBUG_FILTER_CLONE:-}" ]]; then
  git -c protocol.version=2 clone --no-checkout --filter=tree:1 "$SERVER_URL/$REPO_NAME" "$DEST2/repo" || {
    echo "tree:1 clone failed"; exit 1;
  }
else
  git -c protocol.version=2 clone --no-checkout --filter=tree:1 "$SERVER_URL/$REPO_NAME" "$DEST2/repo" >/dev/null 2>&1 || {
    echo "tree:1 clone failed"; exit 1;
  }
fi
FILTER_SIZE=0
for pack in "$DEST2/repo/.git/objects/pack"/*.pack; do
  if [[ -f "$pack" ]]; then
    size=$(stat -c %s "$pack" 2>/dev/null || wc -c < "$pack")
    FILTER_SIZE=$((FILTER_SIZE + size))
  fi
done

echo "baseline pack bytes: $BASE_SIZE"
echo "tree:1 pack bytes: $FILTER_SIZE"

count_pack_objects() {
  local repo_dir="$1"
  local total=0
  for pack in "$repo_dir/.git/objects/pack"/*.pack; do
    [[ -f "$pack" ]] || continue
    while IFS= read -r line; do
      case "$line" in
        [0-9a-f][0-9a-f]*) total=$((total + 1));;
      esac
    done < <(git -C "$repo_dir" verify-pack -v "$pack" 2>/dev/null)
  done
  echo "$total"
}

BASE_OBJECTS=$(count_pack_objects "$DEST1/repo")
FILTER_OBJECTS=$(count_pack_objects "$DEST2/repo")

echo "baseline pack objects: $BASE_OBJECTS"
echo "tree:1 pack objects: $FILTER_OBJECTS"

awk -v base="$BASE_SIZE" -v filt="$FILTER_SIZE" 'BEGIN { if (filt >= base*0.85) exit 1; }' || {
  echo "tree:1 pack not sufficiently smaller"; exit 1;
}

if [[ "$FILTER_OBJECTS" -ge "$BASE_OBJECTS" ]]; then
  echo "tree:1 object count did not drop"; exit 1;
fi

echo "[git-http-v2-filter-tree] ok"
