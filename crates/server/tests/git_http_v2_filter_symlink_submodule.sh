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
  nix develop --impure -c cargo run --manifest-path crates/server/Cargo.toml --bin server >/tmp/forge-server.log 2>&1 &
  SERVER_PID=$!
)

for i in {1..60}; do
  if curl -s "$SERVER_URL/healthz" >/dev/null 2>&1; then break; fi
  sleep 0.2
done

# Prepare superproject and submodule
mkdir -p "$REPOS_DIR/$REPO_NAME.git" "$REPOS_DIR/sub.git"
git init --bare "$REPOS_DIR/$REPO_NAME.git" >/dev/null
git init --bare "$REPOS_DIR/sub.git" >/dev/null
touch "$REPOS_DIR/$REPO_NAME.git/git-daemon-export-ok"
touch "$REPOS_DIR/sub.git/git-daemon-export-ok"

SUB=$(mktemp -d)
git -C "$SUB" init >/dev/null
echo sub > "$SUB/README"
git -C "$SUB" add README >/dev/null
git -C "$SUB" -c user.email=t@e -c user.name=t commit -m init >/dev/null
git -C "$SUB" branch -M main >/dev/null
git -C "$SUB" remote add origin "$REPOS_DIR/sub.git"
git -C "$SUB" push origin main >/dev/null

TMP=$(mktemp -d)
git -C "$TMP" init >/dev/null
echo root > "$TMP/root.txt"
git -C "$TMP" add root.txt >/dev/null
git -C "$TMP" -c user.email=t@e -c user.name=t commit -m base >/dev/null
git -C "$TMP" submodule add "$SERVER_URL/../sub.git" sub >/dev/null 2>&1 || true
git -C "$TMP" commit -am "add submodule" >/dev/null || true
ln -s root.txt "$TMP/link.txt"
git -C "$TMP" add link.txt >/dev/null
git -C "$TMP" -c user.email=t@e -c user.name=t commit -m "add symlink" >/dev/null
git -C "$TMP" branch -M main >/dev/null
git -C "$TMP" remote add origin "$REPOS_DIR/$REPO_NAME.git"
git -C "$TMP" push origin main >/dev/null

DEST=$(mktemp -d)
git -c protocol.version=2 clone --filter=blob:none "$SERVER_URL/$REPO_NAME" "$DEST/repo" >/dev/null 2>&1

# Verify symlink exists as symlink and submodule entry recorded
test -L "$DEST/repo/link.txt"
git -C "$DEST/repo" ls-files --stage | grep -q "160000\s" || {
  echo "expected submodule entry missing"; exit 1;
}

echo "[git-http-v2-filter-symlink-submodule] ok"
