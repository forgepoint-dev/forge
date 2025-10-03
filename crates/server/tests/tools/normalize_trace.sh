#!/usr/bin/env bash
set -euo pipefail

# Normalize a Git GIT_TRACE_PACKET file to make diffs stable across
# client versions and environments. Reads from a file path argument
# or stdin and writes the normalized content to stdout.

INPUT="${1:-}"
if [[ -n "$INPUT" && -f "$INPUT" ]]; then
  exec < "$INPUT"
fi

awk '
  {
    line = $0
    # Drop empty lines produced by some git versions
    if (line ~ /^\s*$/) next
    # Mask agent and session identifiers which vary per run/client
    gsub(/agent=[^ ;\t\r\n]*/, "agent=<masked>", line)
    gsub(/session-id=[^ ;\t\r\n]*/, "session-id=<masked>", line)
    # Mask host/port if they leak into trace lines
    gsub(/127\.0\.0\.1:[0-9]+/, "127.0.0.1:<port>", line)
    # Normalize Git version strings when present
    gsub(/git\/[0-9][0-9.]+/, "git/<ver>", line)
    # Normalize whitespace around pkt markers
    sub(/\s+$/, "", line)
    print line
  }
'
