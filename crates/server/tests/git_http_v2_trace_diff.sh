#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
TOOLS_DIR="$ROOT_DIR/tests/tools"

DB_DIR=$(mktemp -d)
REPOS_DIR=$(mktemp -d -p /tmp)
EXT_DIR="$ROOT_DIR/extensions"
PORT=${PORT:-$(( ( RANDOM % 10000 ) + 30000 ))}
SERVER_URL=${SERVER_URL:-http://127.0.0.1:$PORT}
REPO_NAME=${REPO_NAME:-alpha}

TRACE_DIR=${TRACE_DIR:-/tmp/git-http-v2-traces}

# Scenarios to run (space-separated): simple-clone, shallow-clone, incremental-fetch,
# deepen-since, deepen-not, partial-blob-none, partial-tree-1, partial-blob-limit
TRACE_SCENARIOS=${TRACE_SCENARIOS:-"simple-clone"}

mkdir -p "$TRACE_DIR"

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then kill "$SERVER_PID" 2>/dev/null || true; fi
  rm -rf "$DB_DIR" "$REPOS_DIR" >/dev/null 2>&1 || true
}
trap cleanup EXIT

start_server() {
  local backend="$1" # git|rust
  (
    cd "$ROOT_DIR"
    FORGE_DB_PATH="$DB_DIR" \
    FORGE_REPOS_PATH="$REPOS_DIR" \
    FORGE_EXTENSIONS_DIR="$EXT_DIR" \
    FORGE_GIT_HTTP_MODE=smart \
    FORGE_GIT_HTTP_EXPORT_ALL=true \
    FORGE_GIT_SMART_V2_BACKEND="$backend" \
    FORGE_LISTEN_ADDR="127.0.0.1:$PORT" \
    nix develop --impure -c cargo run --manifest-path crates/server/Cargo.toml --bin server >/tmp/forge-server.log 2>&1 &
    SERVER_PID=$!
    echo "$SERVER_PID" > /tmp/forge-server.pid
  )
  for i in {1..60}; do
    if curl -s "$SERVER_URL/healthz" >/dev/null 2>&1; then break; fi
    sleep 0.2
  done
}

stop_server() {
  if [[ -n "${SERVER_PID:-}" ]]; then kill "$SERVER_PID" 2>/dev/null || true; fi
}

prepare_repo() {
  mkdir -p "$REPOS_DIR/$REPO_NAME.git"
  git init --bare "$REPOS_DIR/$REPO_NAME.git" >/dev/null
  touch "$REPOS_DIR/$REPO_NAME.git/git-daemon-export-ok"
  TMP=$(mktemp -d)
  git -C "$TMP" init >/dev/null
  # Create history with dates and tags for deepen-since / deepen-not
  echo hello > "$TMP/README.md" && git -C "$TMP" add README.md >/dev/null
  GIT_COMMITTER_DATE="2023-01-01T00:00:00" GIT_AUTHOR_DATE="2023-01-01T00:00:00" \
    git -C "$TMP" -c user.email=t@e -c user.name=t commit -m init >/dev/null
  echo a >> "$TMP/README.md" && git -C "$TMP" add README.md >/dev/null
  GIT_COMMITTER_DATE="2023-06-01T00:00:00" GIT_AUTHOR_DATE="2023-06-01T00:00:00" \
    git -C "$TMP" -c user.email=t@e -c user.name=t commit -m mid >/dev/null
  echo b >> "$TMP/README.md" && git -C "$TMP" add README.md >/dev/null
  GIT_COMMITTER_DATE="2024-01-01T00:00:00" GIT_AUTHOR_DATE="2024-01-01T00:00:00" \
    git -C "$TMP" -c user.email=t@e -c user.name=t commit -m latest >/dev/null
  git -C "$TMP" tag -a v1 -m v1 >/dev/null
  git -C "$TMP" branch -M main >/dev/null
  git -C "$TMP" remote add origin "$REPOS_DIR/$REPO_NAME.git"
  git -C "$TMP" push origin main >/dev/null
}

capture_simple_clone() {
  local backend="$1"; local dir="$TRACE_DIR/simple-clone"; mkdir -p "$dir"
  local raw="$dir/$backend.raw"; local norm="$dir/$backend.trace"; : > "$raw"
  GIT_TRACE_PACKET=1 git -c protocol.version=2 ls-remote "$SERVER_URL/$REPO_NAME" 2>>"$raw" >/dev/null
  local DEST; DEST=$(mktemp -d)
  GIT_TRACE_PACKET=1 git -c protocol.version=2 clone "$SERVER_URL/$REPO_NAME" "$DEST/repo" 2>>"$raw" >/dev/null
  "$TOOLS_DIR/normalize_trace.sh" "$raw" > "$norm"
}

capture_deepen_since() {
  local backend="$1"; local dir="$TRACE_DIR/deepen-since"; mkdir -p "$dir"
  local raw="$dir/$backend.raw"; local norm="$dir/$backend.trace"; : > "$raw"
  GIT_TRACE_PACKET=1 git -c protocol.version=2 ls-remote "$SERVER_URL/$REPO_NAME" 2>>"$raw" >/dev/null
  local DEST; DEST=$(mktemp -d)
  # Choose since date between mid and latest
  GIT_TRACE_PACKET=1 git -c protocol.version=2 clone --shallow-since="2023-07-01T00:00:00" \
    "$SERVER_URL/$REPO_NAME" "$DEST/repo" 2>>"$raw" >/dev/null
  "$TOOLS_DIR/normalize_trace.sh" "$raw" > "$norm"
}

capture_deepen_not() {
  local backend="$1"; local dir="$TRACE_DIR/deepen-not"; mkdir -p "$dir"
  local raw="$dir/$backend.raw"; local norm="$dir/$backend.trace"; : > "$raw"
  GIT_TRACE_PACKET=1 git -c protocol.version=2 ls-remote "$SERVER_URL/$REPO_NAME" 2>>"$raw" >/dev/null
  local DEST; DEST=$(mktemp -d)
  # Exclude tag v1 (the oldest commit)
  GIT_TRACE_PACKET=1 git -c protocol.version=2 clone --shallow-exclude=v1 \
    "$SERVER_URL/$REPO_NAME" "$DEST/repo" 2>>"$raw" >/dev/null
  "$TOOLS_DIR/normalize_trace.sh" "$raw" > "$norm"
}

capture_shallow_clone() {
  local backend="$1"; local dir="$TRACE_DIR/shallow-clone"; mkdir -p "$dir"
  local raw="$dir/$backend.raw"; local norm="$dir/$backend.trace"; : > "$raw"
  GIT_TRACE_PACKET=1 git -c protocol.version=2 ls-remote "$SERVER_URL/$REPO_NAME" 2>>"$raw" >/dev/null
  local DEST; DEST=$(mktemp -d)
  GIT_TRACE_PACKET=1 git -c protocol.version=2 clone --depth=1 "$SERVER_URL/$REPO_NAME" "$DEST/repo" 2>>"$raw" >/dev/null
  "$TOOLS_DIR/normalize_trace.sh" "$raw" > "$norm"
}

capture_incremental_fetch() {
  local backend="$1"; local dir="$TRACE_DIR/incremental-fetch"; mkdir -p "$dir"
  local raw="$dir/$backend.raw"; local norm="$dir/$backend.trace"; : > "$raw"
  # Prepare client clone without tracing
  local DEST; DEST=$(mktemp -d)
  git -c protocol.version=2 clone "$SERVER_URL/$REPO_NAME" "$DEST/repo" >/dev/null 2>&1
  # Add a new commit to origin
  local TMP2; TMP2=$(mktemp -d)
  git -C "$TMP2" init >/dev/null
  git -C "$TMP2" remote add origin "$REPOS_DIR/$REPO_NAME.git"
  git -C "$TMP2" fetch origin >/dev/null 2>&1 || true
  git -C "$TMP2" checkout -b main origin/main >/dev/null 2>&1 || git -C "$TMP2" checkout -b main >/dev/null 2>&1
  echo $(date +%s) >> "$TMP2/CHANGELOG"
  git -C "$TMP2" add CHANGELOG
  git -C "$TMP2" -c user.email=t@e -c user.name=t commit -m "chore: bump" >/dev/null
  git -C "$TMP2" push origin main >/dev/null
  # Trace fetch on client
  GIT_TRACE_PACKET=1 git -C "$DEST/repo" -c protocol.version=2 fetch origin 2>>"$raw" >/dev/null
  "$TOOLS_DIR/normalize_trace.sh" "$raw" > "$norm"
}

capture_filter_clone() {
  local backend="$1"; local filter="$2"; local key="$3"
  local dir="$TRACE_DIR/$key"; mkdir -p "$dir"
  local raw="$dir/$backend.raw"; local norm="$dir/$backend.trace"; : > "$raw"
  GIT_TRACE_PACKET=1 git -c protocol.version=2 ls-remote "$SERVER_URL/$REPO_NAME" 2>>"$raw" >/dev/null
  local DEST; DEST=$(mktemp -d)
  GIT_TRACE_PACKET=1 git -c protocol.version=2 clone --filter="$filter" "$SERVER_URL/$REPO_NAME" "$DEST/repo" 2>>"$raw" >/dev/null || true
  "$TOOLS_DIR/normalize_trace.sh" "$raw" > "$norm"
}

prepare_repo

failed=()

for scenario in $TRACE_SCENARIOS; do
  echo "[trace-diff] scenario=$scenario — baseline(git)"
  start_server git
  case "$scenario" in
    simple-clone) capture_simple_clone git ;;
    shallow-clone) capture_shallow_clone git ;;
    incremental-fetch) capture_incremental_fetch git ;;
    deepen-since) capture_deepen_since git ;;
    deepen-not) capture_deepen_not git ;;
    partial-blob-none) capture_filter_clone git "blob:none" "partial-blob-none" ;;
    partial-tree-1) capture_filter_clone git "tree:1" "partial-tree-1" ;;
    partial-blob-limit) capture_filter_clone git "blob:limit=1024" "partial-blob-limit" ;;
    *) echo "unknown scenario: $scenario"; exit 2 ;;
  esac
  stop_server

  echo "[trace-diff] scenario=$scenario — rust backend"
  start_server rust
  case "$scenario" in
    simple-clone) capture_simple_clone rust ;;
    shallow-clone) capture_shallow_clone rust ;;
    incremental-fetch) capture_incremental_fetch rust ;;
    deepen-since) capture_deepen_since rust ;;
    deepen-not) capture_deepen_not rust ;;
    partial-blob-none) capture_filter_clone rust "blob:none" "partial-blob-none" ;;
    partial-tree-1) capture_filter_clone rust "tree:1" "partial-tree-1" ;;
    partial-blob-limit) capture_filter_clone rust "blob:limit=1024" "partial-blob-limit" ;;
  esac
  stop_server

  echo "[trace-diff] comparing traces for $scenario..."
  if ! diff -u "$TRACE_DIR/$scenario/git.trace" "$TRACE_DIR/$scenario/rust.trace"; then
    echo "Trace mismatch for scenario=$scenario. Artifacts in $TRACE_DIR/$scenario" >&2
    failed+=("$scenario")
  else
    echo "[trace-diff] scenario=$scenario OK"
  fi
done

if (( ${#failed[@]} > 0 )); then
  echo "Scenarios failed: ${failed[*]}" >&2
  exit 1
fi

echo "[trace-diff] all scenarios passed"
