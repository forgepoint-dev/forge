# Forgepoint Server & Git HTTP Migration Guide

This document walks through the repository changes introduced when the Forgepoint
server crate moved under `crates/server` and the Smart HTTP implementation was
extracted into a reusable `crates/git-http` crate. Use it when rebasing work in
progress, updating tooling, or backporting fixes from the legacy layout.

## Snapshot: Before vs. After

| Area | Legacy path/name | New path/name |
| --- | --- | --- |
| Server crate | `server/` | `crates/server/` |
| Git Smart HTTP modules | `server/src/git_http` | `crates/git-http/src` |
| Server Cargo manifest | `server/Cargo.toml` | `crates/server/Cargo.toml` |
| Workspace membership | `Cargo.toml` listed `server` | `Cargo.toml` lists `crates/server` + `crates/git-http` |
| Integration tests & scripts | `server/tests/*.sh` | `crates/server/tests/*.sh` |
| Extension artifacts | `server/extensions/*.wasm` | `crates/server/extensions/*.wasm` (temporary) |
| Start scripts & Just recipes | referenced `server/...` | now reference `crates/server/...` |

Keep the table handy while migrating or reviewing feature branches; if a file in
your diff still points at the legacy column, it almost certainly needs an update.

## Audience & Prerequisites

- Engineers maintaining the Forgepoint backend or automation that touches the
  server crate.
- Git + Rust toolchain available. The project expects commands to run inside the
  Nix shell (`nix develop --impure -c ...`), but the steps below mention
  non-Nix fallbacks where relevant.
- Working tree clean before you begin.

## High-Level Migration Steps

1. Sync to the commit that contains this guide.
2. Update workspace metadata to point at `crates/server` and `crates/git-http`.
3. Move code and assets out of the legacy `server/` directory.
4. Replace in-crate Git HTTP imports with the new library crate.
5. Refresh scripts, docs, and tooling that referenced the old paths.
6. Re-run checks/tests inside the Nix shell to ensure everything still builds.

Each step is detailed below.

## Step-by-Step Instructions

### 1. Sync & Branch

```bash
git fetch origin
git checkout -b migrate-server-refactor origin/main
```

If you have outstanding work that still references `server/`, rebase it onto
this branch after completing the migration.

### 2. Workspace Metadata

- `Cargo.toml` now lists `crates/server` and `crates/git-http` under
  `[workspace].members`.
- `Cargo.lock` includes `git-http` as a package entry.

No further action is required unless you maintain tooling that parsed the old
member list.

### 3. Directory Layout

- The former `server/` root has been removed from version control.
- The server crate now lives at `crates/server/` with the same source tree.
- Git HTTP modules (`git_http/{mod.rs,v2.rs,pack.rs,...}`) were moved into
  `crates/git-http/` as a standalone library.

If you have unmerged branches: use `git mv` to relocate files into the new
folders before rebasing to minimize conflicts.

#### Handling In-Flight Branches

Stage the obvious renames locally *before* you rebase so Git can detect them and
carry history across:

```bash
# Inside your feature branch (based on pre-migration main)
mkdir -p crates
git mv server crates/server
mkdir -p crates/git-http
git mv crates/server/src/git_http crates/git-http/src
```

Commit (or at least stage) those moves, then rebase onto `origin/main`. Git will
now treat your feature edits in `server/src/git_http/*` as touching
`crates/git-http/src/*`, avoiding wholesale deletions/re-additions. After the
rebase finishes, drop the temporary commit if you created one and refresh your
workspace from `main` for the new `crates/git-http/Cargo.toml`, README updates,
and other files that were added as part of the migration.

### 4. Code Changes in Server Crate

- Imports of `crate::git_http::*` should be replaced with `git_http::*` via the
  new dependency added in `crates/server/Cargo.toml`.
- `AppState` in `crates/server/src/api/server.rs` implements the new
  `git_http::GitHttpState` trait, while `RepositoryStorage` implements
  `git_http::RepositoryProvider`.
- Any direct module references (e.g., `crate::git_http::pack`) need updates to
  point at the external crate.

Review your downstream code for similar adjustments.

### 5. Scripts, Docs, and Tooling

- `Justfile`, `start-with-extensions.sh`, and integration test scripts now use
  `crates/server/...` paths.
- Documentation (README, `docs/guides/smart-http.md`, `docs/guides/oci-extensions.md`,
  ADRs, RFCs, TODOs) that pointed at `server/...` should be updated to the new
  locations. Several files were already migrated; use `rg "server/"` to spot any
  remaining references you own.
- CI workflows and automation that shell out to scripts must be pointed at the
  new paths. Known touch points:
  - `.github/workflows/git-smart-http-v2-parity.yml`
  - Any bootstrap scripts in `scripts/` or neighboring infra repos that expect
    `server/tests/*` or `server/run-dev.sh`
- Audit README snippets, `TODO.md`, `TODO_ANALYSIS.md`, and
  `start-with-extensions.sh`. Anywhere you see `server/tests/...` or
  `server/extensions/...`, replace it with the new `crates/server/...` paths so
  local docs stay accurate.
- When in doubt, run both `rg "server/tests"` and `rg "server/src/git_http"`.

### 6. Validation

Inside the repo root:

```bash
nix develop --impure -c cargo check
nix develop --impure -c cargo test

# optional: run git HTTP smoke tests
nix develop --impure -c just test-git-http-v2
```

If you are outside the Nix environment, ensure `cc`, `pkg-config`, and OpenSSL
development headers are available; otherwise the build may fail.

## Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `error: linker cc not found` when running `cargo check` outside Nix | Missing system compiler | Use the Nix shell or install a C toolchain locally. |
| `failed to resolve address for github.com` during `cargo generate-lockfile` | Network sandbox in CI or restricted environment | Retry inside the allowed environment or set `CARGO_NET_GIT_FETCH_WITH_CLI=true`. |
| Scripts still referencing `server/` fail | Hard-coded paths | Update them to `crates/server/` equivalents. |
| `unresolved import git_http` or `cannot find crate` errors | `crates/git-http` not added to workspace or dependency not declared | Ensure `Cargo.toml` lists `crates/git-http` in `[workspace].members` and add `git-http = { path = "../git-http" }` to `crates/server/Cargo.toml`. |

## Verification Checklist

- [ ] `nix develop --impure -c cargo check` / `cargo test` succeed from the repo root.
- [ ] `rg "server/"` and `rg "server/tests"` only return expected hits inside `target/` or other generated artifacts.
- [ ] New `git-http` crate builds and is used by the server (`cargo check -p server` pulls it in).
- [ ] Tooling (Justfile targets, CI jobs, start scripts) points at `crates/server/...` paths.

## Rollback

If you need to revert locally, run:

```bash
git restore --staged --worktree --source=HEAD^ .
```

Or reset to a commit prior to the migration. Pushes to shared branches should be
coordinated with the backend team.

## Questions?

Reach out in the Forgepoint backend channel or mention the maintainers listed in
`CODEOWNERS` for `crates/server/` and `crates/git-http/`.
