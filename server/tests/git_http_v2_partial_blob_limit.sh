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
  FORGE_GIT_SMART_V2_BACKEND=rust \
  FORGE_LISTEN_ADDR="127.0.0.1:$PORT" \
  nix develop --impure -c cargo run --manifest-path server/Cargo.toml --bin server >/tmp/forge-server.log 2>&1 &
  SERVER_PID=$!
)

for i in {1..60}; do
  if curl -s "$SERVER_URL/healthz" >/dev/null 2>&1; then break; fi
  sleep 0.2
done

mkdir -p "$REPOS_DIR/$REPO_NAME.git"
git init --bare "$REPOS_DIR/$REPO_NAME.git" >/dev/null
touch "$REPOS_DIR/$REPO_NAME.git/git-daemon-export-ok"

TMP=$(mktemp -d)
git -C "$TMP" init >/dev/null
# Create mixed small and large blobs
for i in {1..3}; do
  head -c 200 </dev/urandom > "$TMP/small_$i.bin"
  git -C "$TMP" add "small_$i.bin" >/dev/null
done
for i in {1..3}; do
  dd if=/dev/urandom of="$TMP/large_$i.bin" bs=1M count=1 status=none
  git -C "$TMP" add "large_$i.bin" >/dev/null
done
git -C "$TMP" -c user.email=t@e -c user.name=t commit -m "add mixed blobs" >/dev/null
git -C "$TMP" branch -M main >/dev/null
git -C "$TMP" remote add origin "$REPOS_DIR/$REPO_NAME.git"
git -C "$TMP" push origin main >/dev/null

# Baseline clone (no filter)
DEST1=$(mktemp -d)
git -c protocol.version=2 clone "$SERVER_URL/$REPO_NAME" "$DEST1/repo" >/dev/null 2>&1
BASE_PACK=$(ls -1 "$DEST1/repo/.git/objects/pack"/*.pack | head -n1)
BASE_SIZE=$(stat -c %s "$BASE_PACK" 2>/dev/null || wc -c < "$BASE_PACK")

# Filtered clone (blob:limit=1024)
DEST2=$(mktemp -d)
git -c protocol.version=2 clone --filter=blob:limit=1024 "$SERVER_URL/$REPO_NAME" "$DEST2/repo" >/dev/null 2>&1 || {
  echo "blob:limit clone failed"; exit 1;
}
FILTER_PACK=$(ls -1 "$DEST2/repo/.git/objects/pack"/*.pack | head -n1)
FILTER_SIZE=$(stat -c %s "$FILTER_PACK" 2>/dev/null || wc -c < "$FILTER_PACK")

echo "baseline pack bytes: $BASE_SIZE"
echo "blob:limit pack bytes: $FILTER_SIZE"

# Expect at least 30% reduction vs baseline
awk -v base="$BASE_SIZE" -v filt="$FILTER_SIZE" 'BEGIN { if (filt >= base*0.7) exit 1; }' || {
  echo "blob:limit pack not sufficiently smaller"; exit 1;
}

echo "[git-http-v2-partial-blob-limit] ok"
