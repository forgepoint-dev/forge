#!/usr/bin/env bash
set -euo pipefail

# Simple benchmark harness comparing clone times between the git http-backend
# (baseline) and the Rust Smart HTTP v2 backend. Requires `hyperfine` and `jq`.

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)

DB_DIR=$(mktemp -d)
REPOS_DIR=$(mktemp -d -p /tmp)
EXT_DIR="$ROOT_DIR/extensions"
PORT=${PORT:-$(( ( RANDOM % 10000 ) + 30000 ))}
SERVER_URL=${SERVER_URL:-http://127.0.0.1:$PORT}
REPO_NAME=${REPO_NAME:-alpha}

RUNS=${BENCH_RUNS:-5}
THRESHOLD=${BENCH_THRESHOLD:-1.20} # rust/mean vs baseline
ENFORCE=${BENCH_ENFORCE:-0}
OUT_DIR=${BENCH_OUT_DIR:-/tmp/git-http-v2-bench}
# Scenarios to bench (space-separated): simple-clone, shallow-clone, partial-blob-none, partial-tree-1, partial-blob-limit
BENCH_SCENARIOS=${BENCH_SCENARIOS:-"simple-clone"}
mkdir -p "$OUT_DIR"

cleanup_all() {
  if [[ -n "${SERVER_PID:-}" ]]; then kill "$SERVER_PID" 2>/dev/null || true; fi
  rm -rf "$DB_DIR" "$REPOS_DIR" >/dev/null 2>&1 || true
}
trap cleanup_all EXIT

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
    cargo run --manifest-path crates/server/Cargo.toml --bin server >/tmp/forge-server.log 2>&1 &
    SERVER_PID=$!
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
  echo hello > "$TMP/README.md"
  git -C "$TMP" add README.md >/dev/null
  git -C "$TMP" -c user.email=t@e -c user.name=t commit -m init >/dev/null
  git -C "$TMP" branch -M main >/dev/null
  git -C "$TMP" remote add origin "$REPOS_DIR/$REPO_NAME.git"
  git -C "$TMP" push origin main >/dev/null
}

bench_cmd_for_scenario() {
  local scenario="$1"
  case "$scenario" in
    simple-clone) echo "bash -c 'DEST=$(mktemp -d); git -c protocol.version=2 clone $SERVER_URL/$REPO_NAME \"$DEST/repo\" >/dev/null 2>&1'" ;;
    shallow-clone) echo "bash -c 'DEST=$(mktemp -d); git -c protocol.version=2 clone --depth=1 $SERVER_URL/$REPO_NAME \"$DEST/repo\" >/dev/null 2>&1'" ;;
    partial-blob-none) echo "bash -c 'DEST=$(mktemp -d); git -c protocol.version=2 clone --filter=blob:none $SERVER_URL/$REPO_NAME \"$DEST/repo\" >/dev/null 2>&1'" ;;
    partial-tree-1) echo "bash -c 'DEST=$(mktemp -d); git -c protocol.version=2 clone --filter=tree:1 $SERVER_URL/$REPO_NAME \"$DEST/repo\" >/dev/null 2>&1'" ;;
    partial-blob-limit) echo "bash -c 'DEST=$(mktemp -d); git -c protocol.version=2 clone --filter=blob:limit=1024 $SERVER_URL/$REPO_NAME \"$DEST/repo\" >/dev/null 2>&1'" ;;
    *) echo "unknown scenario: $scenario" >&2; return 2 ;;
  esac
}

jq_safe() {
  nix run nixpkgs#jq -- "$@"
}

prepare_repo

declare -a fail=()

for scenario in $BENCH_SCENARIOS; do
  echo "[bench:$scenario] baseline (git)"
  start_server git
  cmd=$(bench_cmd_for_scenario "$scenario") || { echo "invalid scenario $scenario"; exit 2; }
  nix run nixpkgs#hyperfine -- --warmup 1 --runs "$RUNS" --style=basic \
    --export-json "$OUT_DIR/${scenario}-git.json" "$cmd"
  stop_server

  echo "[bench:$scenario] rust backend"
  start_server rust
  nix run nixpkgs#hyperfine -- --warmup 1 --runs "$RUNS" --style=basic \
    --export-json "$OUT_DIR/${scenario}-rust.json" "$cmd" || true
  stop_server

  git_mean=$(jq_safe -r '.results[0].mean' "$OUT_DIR/${scenario}-git.json")
  rust_mean=$(jq_safe -r '.results[0].mean' "$OUT_DIR/${scenario}-rust.json" 2>/dev/null || echo "nan")
  ratio=$(awk -v r="$rust_mean" -v g="$git_mean" 'BEGIN{ if (g==0 || r=="" || r=="nan") {print "nan"} else {printf "%.4f", r/g} }')
  echo "[bench:$scenario] means: git=$git_mean s, rust=$rust_mean s, ratio=$ratio"

  printf "%s\n" "scenario=$scenario" "git_mean=$git_mean" "rust_mean=$rust_mean" "ratio=$ratio" >> "$OUT_DIR/summary.env"

  if [[ "$ENFORCE" == "1" ]]; then
    if awk -v ratio="$ratio" -v th="$THRESHOLD" 'BEGIN{ exit (ratio>th) ? 1 : 0 }'; then :; else fail+=("$scenario"); fi
  fi
done

if (( ${#fail[@]} > 0 )); then
  echo "Benchmark ratio exceeded threshold for: ${fail[*]}" >&2
  exit 1
fi
