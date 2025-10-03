# Smart HTTP v2 — Merge Readiness

Updated: 2025-10-03

## Completed

- [x] Remove stray planning artifacts
  - Deleted `file` (root)
  - Deleted `repeat.fish` (root)
- [x] Align Smart HTTP v2 docs with code
  - `docs/guides/smart-http.md` now documents:
    - `FORGE_GIT_HTTP_MODE=smart|smart-v2`
    - `FORGE_GIT_SMART_V2_BACKEND=git|rust`
    - `FORGE_GIT_SMART_V2_ADVERTISE=git|rust`
    - Export gating and `FORGE_GIT_HTTP_EXPORT_ALL`
- [x] Add spec-focused HTTP tests (unit)
  - `server/src/git_http/v2.rs` tests:
    - `info_refs_requires_service_param` → 400 when service ≠ `git-upload-pack`
    - `info_refs_gated_and_content_type` → 404 without export-ok; 200 with; correct content-type and v2 banner
    - `receive_pack_is_forbidden` → 403 on `/git-receive-pack`
    - `ls_refs_supports_ref_prefix_peel_and_symrefs` (rust backend) → returns refs (incl. `refs/heads/main`)
    - `upload_pack_unknown_command_400` → 400 on unknown command
    - `fetch_with_bad_object_format_is_400` → rejects non-`sha1` object-format

## Next

- [x] Run tests locally (completed 2025-10-03T10:07:39Z) — `cargo test -p server` passed: 102 unit/integration tests ok
  - `nix develop --impure -c cargo test -p server`
- [x] (Optional) Run parity scripts locally for extra confidence (completed 2025-10-03T10:07:39Z)
  - Clone: `nix develop --impure -c bash server/tests/git_http_v2_clone.sh` → ok
  - Shallow: `nix develop --impure -c bash server/tests/git_http_v2_shallow.sh` → ok
  - Fetch update: `nix develop --impure -c bash server/tests/git_http_v2_fetch_update.sh` → ok
- [ ] Open PR and request review
  - Summarize the above and link CI `git-smart-http-v2-parity.yml` run
