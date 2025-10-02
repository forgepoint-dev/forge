#!/usr/bin/env bash
set -euo pipefail

SERVER_URL=${SERVER_URL:-http://localhost:8000}
REPO_DIR=${REPO_DIR:-$(mktemp -d)}
REPO_NAME=${REPO_NAME:-alpha}

cleanup() { rm -rf "$REPO_DIR" >/dev/null 2>&1 || true; }
trap cleanup EXIT

mkdir -p "$REPO_DIR/$REPO_NAME.git"
pushd "$REPO_DIR/$REPO_NAME.git" >/dev/null
git init --bare >/dev/null
popd >/dev/null

echo "[git-http-v2] expecting ls-remote to work once server is hooked to this path"
echo "Create this repo under FORGE_REPOS_PATH and run:"
echo "  git -c protocol.version=2 ls-remote $SERVER_URL/$REPO_NAME"
