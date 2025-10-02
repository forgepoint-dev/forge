# Smart HTTP v2 for Forge

This document explains how to serve Git Smart HTTP v2 (read-only) from the Forge server and how to test it.

## Modes

- `FORGE_GIT_HTTP_MODE=smart` enables Smart HTTP routes.
- `FORGE_GIT_SMART_V2_BACKEND=git` uses `git upload-pack --stateless-rpc` under the hood for fetch (default today).
- `FORGE_GIT_SMART_V2_BACKEND=rust` uses the pure-Rust packer (WIP).

Push over HTTP is disabled. The server always returns `403` on `/git-receive-pack`.

## Endpoints

- `GET /:repo/info/refs?service=git-upload-pack` → advertise v2 capabilities.
- `POST /:repo/git-upload-pack` → protocol v2 commands:
  - `ls-refs` — implemented in Rust; supports `ref-prefix`, `peel`, `symrefs`.
  - `fetch` — proxied to Git until pure-Rust pack is finished.

Group routes `/:group/:repo/...` are also supported. The `.git` suffix is optional.

Note: `info/refs` is gated by public visibility as well. Repos without `git-daemon-export-ok` return 404.

## Quickstart

```
FORGE_GIT_HTTP_MODE=smart FORGE_GIT_SMART_V2_BACKEND=git \
FORGE_IN_MEMORY_DB=true cargo run --bin server

# Create a bare repo under FORGE_REPOS_PATH and seed a commit, then:
git -c protocol.version=2 ls-remote http://localhost:8000/alpha
git -c protocol.version=2 clone http://localhost:8000/alpha
```

## Tests

- `just test-git-http-v2` — ls-remote + clone e2e.
- `just test-git-http-v2-shallow` — shallow clone.

Unit tests cover pkt-line encode/decode and fetch parser.

## Roadmap (Pure Rust)

1. Build pack from wants via `gix` and stream over side-band-64k.
2. Add have negotiation for minimal packs.
3. Support shallow clones and (optionally) partial clone filters.

## Security and Limits

- Public gating: create `git-daemon-export-ok` in a repo to allow anonymous HTTP. Or set `FORGE_GIT_HTTP_EXPORT_ALL=true` to allow all (not recommended for multi-tenant).
- Limits (env vars):
  - `FORGE_GIT_MAX_REQUEST_BYTES` (default 67108864)
  - `FORGE_GIT_MAX_CONCURRENCY` (default 64)
  - `FORGE_GIT_REQUEST_TIMEOUT_MS` (default 120000)
  - These are enforced via Axum/Tower layers on Smart HTTP routes.

## Observability

- Metrics endpoint: `GET /metrics` (Prometheus text format)
  - Counters and histograms for advertise, ls-refs, and upload-pack (backend label).
- Health check: `GET /healthz` returns 204.
