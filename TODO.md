# Git Smart HTTP v2 Parity – Next Steps

This tracker captures only the remaining work for Git Smart HTTP v2 parity. See `TODO_ANALYSIS.md` for background and status notes.

## Phase B – Shallow & Filters
- Run and record results for:
  - `server/tests/git_http_v2_partial_blob_none.sh`
  - `server/tests/git_http_v2_partial_blob_limit.sh`
  - `server/tests/git_http_v2_filter_symlink_submodule.sh`
- Publish a consolidated test-results summary once the above pass (include pack/object deltas and timing where relevant).
- Revisit `FORGE_GIT_SMART_V2_BACKEND` default after verification; decide if the Rust backend should become the default path.

## Phase C – Deltas & Thin-Pack
- Implement OFS-DELTA support and choose between OFS/REF fallback heuristics.
- Extend delta compression beyond commits to trees and blobs, with reasonable size thresholds.
- Improve delta generation quality (copy ops, similarity heuristics, multi-threaded search, reuse of on-disk deltas when possible).

## Cross-Cutting Testing & Tooling
- Add unit coverage for delta encoding/decoding, negotiation state machine edge cases, and filter handling (`blob:none`, `tree:<depth>`, `blob:limit=<n>`).
- Create end-to-end scenarios exercising thin-pack fetches, want-ref resolution, shallow/partial fetch combinations, and failure handling (timeouts, corrupted input, very large repos).
- Run compatibility checks against multiple Git client versions once the above work lands.
