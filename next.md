# Git Smart HTTP v2 Parity Roadmap (vs. git http-backend)

Goal: make our Rust Smart HTTP v2 (upload-pack) indistinguishable from `git http-backend` for clones, incremental fetches, shallow/filtered operations, and client UX. This document focuses on the remaining work only (completed items removed).

## How To Run Locally (for parity testing)

Use the Nix shell so toolchains match CI. The Justfile already wraps commands with Nix.

- Start the server with Smart HTTP v2 enabled and export gating relaxed:
  ```bash
  # From repo root
  FORGE_GIT_HTTP_MODE=smart \
  FORGE_GIT_HTTP_EXPORT_ALL=true \
  nix develop --impure -c cargo run --manifest-path server/Cargo.toml --bin server
  ```
- Toggle backend under test (baseline vs. Rust):
  - Baseline (proxy to git http-backend): `FORGE_GIT_SMART_V2_BACKEND=git`
  - Rust implementation: `FORGE_GIT_SMART_V2_BACKEND=rust`
- Optional request shaping (defaults shown):
  - `FORGE_GIT_MAX_REQUEST_BYTES=67108864`
  - `FORGE_GIT_MAX_CONCURRENCY=64`
  - `FORGE_GIT_REQUEST_TIMEOUT_MS=120000`

Quick helper tasks (preferred):

- Run baseline clone test: `just test-git-http-v2`
- Shallow clone test: `just test-git-http-v2-shallow`
- Incremental fetch test: `just test-git-http-v2-fetch`
- Concurrency smoke: `just test-git-http-v2-concurrency`
- Visibility (export-ok) check: `just test-git-http-v2-visibility`
- Rust backend e2e: `just test-git-http-v2-rust`

Related test scripts live under `server/tests/` and can be invoked directly:

- `server/tests/git_http_v2_clone.sh` — v2 ls-remote + clone (git backend)
- `server/tests/git_http_v2_shallow.sh` — depth-limited clone
- `server/tests/git_http_v2_fetch_update.sh` — add commit then fetch
- `server/tests/git_http_v2_concurrency.sh` — many parallel clones
- `server/tests/git_http_v2_visibility.sh` — honors `git-daemon-export-ok`
- `server/tests/git_http_v2_rust_backend.sh` — exercises the Rust backend end-to-end

Packet trace comparison procedure:

1. Run baseline (git) and capture trace:
   ```bash
   FORGE_GIT_SMART_V2_BACKEND=git FORGE_GIT_HTTP_MODE=smart FORGE_GIT_HTTP_EXPORT_ALL=true \
   nix develop --impure -c cargo run --manifest-path server/Cargo.toml --bin server &
   GIT_TRACE_PACKET=1 git -c protocol.version=2 clone http://127.0.0.1:8000/myrepo /tmp/repo 2> /tmp/trace.git
   kill %1
   ```
2. Run Rust backend and capture trace:
   ```bash
   FORGE_GIT_SMART_V2_BACKEND=rust FORGE_GIT_HTTP_MODE=smart FORGE_GIT_HTTP_EXPORT_ALL=true \
   nix develop --impure -c cargo run --manifest-path server/Cargo.toml --bin server &
   GIT_TRACE_PACKET=1 git -c protocol.version=2 clone http://127.0.0.1:8000/myrepo /tmp/repo2 2> /tmp/trace.rust
   kill %1
   ```
3. Diff with awareness of section boundaries (`0001` vs `0000`):
   ```bash
   diff -u /tmp/trace.git /tmp/trace.rust || true
   ```

Troubleshooting tips:

- 404 on repo: create a bare repo under `FORGE_REPOS_PATH` and add `git-daemon-export-ok`, or set `FORGE_GIT_HTTP_EXPORT_ALL=true` for local testing.
- Push over HTTP: intentionally blocked; tests expect failure on receive-pack.
- Timeouts under load: raise `FORGE_GIT_REQUEST_TIMEOUT_MS` and/or reduce concurrency.

## Plan: Remaining Protocol Parity (v2)
- Capabilities advertisement (Rust path)
  - Emit v2 banner and commands when not proxying git; include `ls-refs`, `fetch=shallow`, `fetch=filter`, `server-option`, `session-id`, `object-format`, `agent`.
  - Acceptance: `git -c protocol.version=2 ls-remote` shows same capabilities as `git http-backend` under trace.

- Request parsing completeness
  - Add remaining fetch keys: `done`, `want-ref`, `want-refs`, `no-progress`.
  - Acceptance: Round-trip fuzz test over pkt-line decoder/encoder; parse official samples without loss.

- Section framing and negotiation
  - Implement correct ACK negotiation (common/ready/multi-ack); only NAK when appropriate.
  - Respect sideband negotiation (support raw pack when sideband not requested) and `no-progress`.
  - Acceptance: `GIT_TRACE_PACKET=1 git fetch` shows correct ACKs, section order, and progress behaviour.

## Packfile Generation (Remaining)
- Delta and thin-pack
  - Implement `thin-pack` and `ofs-delta` support; fall back to REF deltas when disabled.
  - Acceptance: `git index-pack` verifies without missing bases; `verify-pack -v` OK.

- Object selection
  - Add `want-ref`/`want-refs` support; ensure traversal stops at `have` commits.
  - Acceptance: incremental fetch transfers only missing objects vs. baseline.

- Compression and performance
  - Add multi-threaded delta search and compression; reuse on-disk pack deltas when beneficial.
  - Acceptance: pack build time within ±20% of `git http-backend` on test corpora.

## Shallow and History Depth (Remaining)
- Shallow-info correctness
  - Emit `shallow`/`unshallow` lines based on `deepen`/`deepen-since`/`deepen-not`; maintain per-request shallow graph.
  - Acceptance: `git clone --depth=N`, `--deepen=N`, `--shallow-since`, `--shallow-exclude` match baseline graphs.

## Partial Clone Filters (Remaining)
- Implement and advertise `filter=blob:none`, `filter=tree:<depth>`, `filter=blob:limit=<n>`.
- Ensure omitted objects are not sent; lazy blob fetches succeed.
- Acceptance: partial clone scenarios succeed and match baseline behaviour.

## Sideband and Progress (Remaining)
- Use band 1 for data, band 2 for progress, band 3 for errors across all paths.
- Respect `no-progress` and sideband negotiation (support non-sideband pack streaming).
- Acceptance: progress visibility matches client flags and baseline.

<!-- ls-refs parity (peeled tags, HEAD, ref-prefix) is implemented; no remaining work tracked here. -->

<!-- HTTP endpoints, headers, and export gating are implemented; no remaining work tracked here. -->

## Hardening and Limits (Remaining)
- Reject oversized pkt-lines/malformed sections with band-3 ERR and graceful HTTP status.
- Enforce caps on `want`/`have` counts and maximum deepen/filter sizes.
- Acceptance: abusive requests get Git-layer errors (band 3) and appropriate HTTP status.

## Object-format support (Remaining)
- Add `sha256` repository support when upstream stabilizes dual-format support.
- Acceptance: clones/fetches from sha256 repos work with a sha256 client build.

## Observability (Remaining)
- Metrics: add histograms for negotiation, traversal, delta search, compression; label backend, filter, result.
- Tracing: structured spans for parse→negotiate→traverse→compress→stream; correlate with `session-id`.
- Acceptance: Prometheus exposes series; dashboards show p50/p95 and error rates.

## Compatibility & Edge Cases (Remaining)
- Alternate object databases; grafts/replacements; commit-graph awareness where applicable.
- Large refspaces (tens of thousands of refs) — ls-refs streaming without excessive buffering.

## Test & CI (Remaining)
- Add tests for deepen flows: `--deepen=N`, `--shallow-since`, `--shallow-exclude`.
- Add partial clone tests: `--filter=blob:none`, `--filter=tree:1`, `--filter=blob:limit=1k`.
- Refs correctness test comparing ls-refs vs. baseline.
- Packet conformance: automate `GIT_TRACE_PACKET=1` diffs (mask pack bytes) in CI.
- Property tests: pkt-line encode/decode, sideband framing, object header encoding.

## Implementation Map (for remaining work)
- `server/src/git_http/v2.rs`
  - Capability advertisement (Rust path); complete fetch key parsing; negotiation (ACKs, sideband/no-progress).
- `server/src/git_http/pack.rs`
  - Thin-pack + deltas; shallow-info; filters; multi-threaded compression; pack reuse.
- `server/src/api/server.rs`
  - Additional shaping/limits if needed for Git endpoints.

## Phased Delivery (Remaining)
- Phase A: Negotiation correctness
  - Complete fetch key parsing; ACK negotiation; sideband/no-progress behaviour.
- Phase B: Shallow + filters
  - Real `shallow-info`; implement `blob:none`, `tree:<n>`, `blob:limit`.
- Phase C: Deltas and thin-pack
  - OFS/REF delta production; thin-pack (optional) with validation; pack reuse.
- Phase D: Perf + observability
  - Parallel compression; metrics/tracing expansion; large-refspace validation.

---

Acceptance for “parity with git http-backend”: All tests in the suite pass with no material differences in on-the-wire framing or client-visible behavior, and perf is within ±20% on representative repos.

## Test Corpus & Client Matrix (Planned)
- Repositories (created under `server/tests/fixtures/repos/`):
  - `tiny.git` — 1 branch, 3 commits, 1 tag (sanity, trace readability).
  - `medium.git` — ~2k commits, 100 tags, 200 branches (ref-scale, ls-refs streaming).
  - `large.git` — >50k commits, >5k refs (stress: negotiation, memory, sideband).
  - `history-shallow.git` — long linear history (depth/deepen-* scenarios).
  - `filter-heavy.git` — many large blobs and trees (partial clone filter coverage).
  - `alt-odb.git` — uses alternates file; verify object resolution.
- Git client versions (matrix): 2.34.x (Ubuntu 22.04), 2.39.x (LTS), 2.46.x+ (latest).
  - Acceptance: behavior and traces are stable across the matrix; known benign diffs documented.

## Golden Traces & Diffing (Planned)
- Store sanitized traces at `server/tests/fixtures/git-http-v2/traces/{scenario}/{backend}.trace`.
- Normalization: strip dynamic values (`agent=git/*`, `session-id`, durations) before diffing.
- Add `server/tests/tools/normalize_trace.sh` to preprocess traces in CI; `just test-git-http-v2-diff` runs end-to-end and fails on unexpected diffs.
- Scenarios supported today by the diff harness (`server/tests/git_http_v2_trace_diff.sh`):
  - `simple-clone` (default)
  - `shallow-clone` (opt-in)
  - `incremental-fetch` (opt-in)
  - Partial clone filters (opt-in): `partial-blob-none`, `partial-tree-1`, `partial-blob-limit`
  - Run with extras: `TRACE_SCENARIOS="simple-clone shallow-clone incremental-fetch" just test-git-http-v2-diff`
- Acceptance: CI job produces masked, comparable traces for each scenario and fails on protocol regressions.

## Benchmark Harness (Planned)
- Add `just bench-git-http-v2` running N iterations per scenario with `hyperfine`.
- Metrics recorded per run: handshake time, negotiation time, pack size, total wall, CPU, peak RSS.
- Environments: single-core throttle vs. 8-core; cold vs. warm repo caches.
- Acceptance: Rust backend within ±20% wall-time vs. baseline; memory within a reasonable envelope (<1.3× baseline) on `medium.git`.
- Scenarios supported by the bench harness (`server/tests/git_http_v2_bench.sh`):
  - `simple-clone` (default), `shallow-clone`; partial-clone filters are available but likely to fail until Phase B.
  - Example: `BENCH_SCENARIOS="simple-clone shallow-clone" just bench-git-http-v2`

## HTTP Behavior & Limits (Planned)
- Headers: set `Content-Type: application/x-git-upload-pack-result` for pack responses; no cache for pack streams.
- Status codes: 200 on success; 4xx for client errors (malformed pkt-line, limit exceeded); 5xx for server faults.
- Limits surfaced via env: request bytes, concurrency, timeout, max `want`/`have` counts, max shallow/filter sizes.
- Acceptance: headers and statuses match baseline semantics across scenarios.

## Capability Advert (Info/Refs) — Rust Path
- Added a Rust advertisement path gated behind `FORGE_GIT_SMART_V2_ADVERTISE=rust`.
- Emits: `version 2` banner; `agent`, `session-id`, `object-format=sha1`, `server-option`, `ls-refs`, and `fetch=shallow|filter|ref-in-want|deepen-since|deepen-not`.
- Default remains `git` (proxy) to avoid CI diffs while we finish Phase A/B.
- Switch on locally for testing: `FORGE_GIT_SMART_V2_ADVERTISE=rust FORGE_GIT_SMART_V2_BACKEND=rust just test-git-http-v2-diff`.

## Error Model (Planned)
- Band 3 errors carry user-facing messages; HTTP status reflects class of failure.
- Map common failures: malformed pkt-line (400), unsupported capability (400), repo not exported (403), repo not found (404), timeout (504), internal (500).
- Acceptance: scripted negative tests assert both sideband error text and HTTP status.

## Security & Hardening (Planned)
- Repository resolution: strict normalization and allowlist under `FORGE_REPOS_PATH`; forbid path traversal and `..`.
- Export gating: honor `git-daemon-export-ok` unless `FORGE_GIT_HTTP_EXPORT_ALL=true`.
- Resource caps: enforce global and per-connection limits; abort cleanly with band 3 + HTTP 429/413 as appropriate.
- Acceptance: dedicated abuse suite verifies graceful handling and no panics.

## Rollout Plan (Planned)
- Flag: `FORGE_GIT_SMART_V2_BACKEND={git|rust}` default `git` initially; canary via env on staging.
- Phased enablement: shallow/standard clones first; enable filters after Phase B; enable thin-pack after Phase C.
- Observability gates: error-rate SLO (<0.1%), timeout rate (<0.5%), median clone time within ±20% of baseline.
- Rollback: single env flip to baseline; traces remain comparable for quick diagnosis.

## Ownership & Tracking
- Tech lead: Git Smart HTTP v2 (Rust path) — TBA.
- Code owners: `server/src/git_http/*` — TBA.
- Labels: `area:git-http`, `proto:v2`, `kind:perf`, `kind:compat`.
- Weekly check-in: publish trace diffs, perf deltas, and open risks; update this document as phases complete.
