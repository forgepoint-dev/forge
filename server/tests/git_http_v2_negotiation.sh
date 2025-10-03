#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)

DB_DIR=$(mktemp -d)
REPOS_DIR=$(mktemp -d -p /tmp)
EXT_DIR="$ROOT_DIR/extensions"
PORT=${PORT:-$(( ( RANDOM % 10000 ) + 30000 ))}
SERVER_URL=${SERVER_URL:-http://127.0.0.1:$PORT}
REPO_NAME=${REPO_NAME:-negotiation}

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" 2>/dev/null || true
  fi
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
  FORGE_GIT_SMART_V2_ADVERTISE=rust \
  FORGE_LISTEN_ADDR="127.0.0.1:$PORT" \
  cargo run --manifest-path server/Cargo.toml --bin server >/tmp/forge-server.log 2>&1 &
  SERVER_PID=$!
)

for _ in {1..400}; do
  if curl -s "$SERVER_URL" >/dev/null 2>&1; then break; fi
  sleep 0.5
done

mkdir -p "$REPOS_DIR/$REPO_NAME.git"
git init --bare "$REPOS_DIR/$REPO_NAME.git" >/dev/null

WORKDIR=$(mktemp -d)
git -C "$WORKDIR" init >/dev/null
echo "root" > "$WORKDIR/log"
git -C "$WORKDIR" add log
git -C "$WORKDIR" -c user.email=t@e -c user.name=t commit -m "c1" >/dev/null
echo "second" >> "$WORKDIR/log"
git -C "$WORKDIR" add log
git -C "$WORKDIR" -c user.email=t@e -c user.name=t commit -m "c2" >/dev/null
echo "third" >> "$WORKDIR/log"
git -C "$WORKDIR" add log
git -C "$WORKDIR" -c user.email=t@e -c user.name=t commit -m "c3" >/dev/null
git -C "$WORKDIR" branch -M main >/dev/null
git -C "$WORKDIR" remote add origin "$REPOS_DIR/$REPO_NAME.git"
git -C "$WORKDIR" push origin main >/dev/null

HEAD_SHA=$(git -C "$WORKDIR" rev-parse HEAD)
BASE_SHA=$(git -C "$WORKDIR" rev-parse HEAD~1)
ROOT_SHA=$(git -C "$WORKDIR" rev-parse HEAD~2)
NON_EXISTENT="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

build_fetch_request() {
  local want=$1
  shift
  python3 - "$want" "$@" <<'PY'
import sys

want = sys.argv[1]
haves = [h for h in sys.argv[2:]]

def pkt(payload: str) -> None:
    data = payload.encode()
    sys.stdout.buffer.write(f"{len(data)+4:04x}".encode())
    sys.stdout.buffer.write(data)

pkt("command=fetch\n")
pkt("object-format=sha1\n")
pkt(f"want {want}\n")
for have in haves:
    pkt(f"have {have}\n")
sys.stdout.buffer.write(b"0000")
PY
}

post_fetch() {
  local request_file=$1
  local response_file=$2
  curl -sS \
    -H "Content-Type: application/x-git-upload-pack-request" \
    -H "Git-Protocol: version=2" \
    --data-binary "@$request_file" \
    "$SERVER_URL/$REPO_NAME/git-upload-pack" \
    > "$response_file"
}

assert_ack_sequence() {
  local response_file=$1
  shift
  python3 - "$response_file" "$@" <<'PY'
import sys

path = sys.argv[1]
expected = list(sys.argv[2:])

def read_pkt(stream):
    hdr = stream.read(4)
    if not hdr:
        return None, None
    length = int(hdr, 16)
    if length == 0:
        return "flush", None
    if length == 1:
        return "delim", None
    data = stream.read(length - 4)
    return "data", data

lines = []
header_seen = False

with open(path, "rb") as fh:
    while True:
        kind, data = read_pkt(fh)
        if kind is None:
            break
        if kind == "delim":
            break
        if kind != "data":
            continue
        text = data.decode().strip()
        if not header_seen:
            if text != "acknowledgments":
                continue
            header_seen = True
            continue
        lines.append(text)

if lines != expected:
    raise SystemExit(f"unexpected ack sequence: {lines} != {expected}")
PY
}

TMPDIR_RESP=$(mktemp -d)

build_fetch_request "$HEAD_SHA" "$NON_EXISTENT" > "$TMPDIR_RESP/request_nak.bin"
post_fetch "$TMPDIR_RESP/request_nak.bin" "$TMPDIR_RESP/response_nak.bin"
assert_ack_sequence "$TMPDIR_RESP/response_nak.bin" "NAK"

build_fetch_request "$HEAD_SHA" "$NON_EXISTENT" "$ROOT_SHA" "$BASE_SHA" > "$TMPDIR_RESP/request_ack.bin"
post_fetch "$TMPDIR_RESP/request_ack.bin" "$TMPDIR_RESP/response_ack.bin"
assert_ack_sequence "$TMPDIR_RESP/response_ack.bin" \
  "ACK $ROOT_SHA common" \
  "ACK $BASE_SHA common" \
  "ACK $BASE_SHA ready"

echo "ok"
