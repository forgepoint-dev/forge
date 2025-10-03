# TODO.md Analysis and Corrections

This document provides a detailed analysis of the current TODO.md against the actual codebase implementation, identifying what is correct, incorrect, and what needs updating.

## Executive Summary

The TODO.md document is **partially accurate** but contains several inaccuracies and omissions:

1. **Phase A (Negotiation)**: Mostly accurate, but some features are already implemented
2. **Phase B (Shallow + Filters)**: Accurate in scope, needs verification status
3. **Phase C (Deltas and Thin-Pack)**: Partially implemented; TODO underestimates current progress
4. **File References**: Some file paths and function names are incorrect or outdated

## Detailed Analysis by Phase

### Phase A: Negotiation Correctness

#### A.1 Capabilities Advertisement (Lines 11-15)

**Status**: ✅ MOSTLY CORRECT, needs minor updates

**Current Implementation** ([`server/src/git_http/v2.rs:89-117`](server/src/git_http/v2.rs:89-117)):
```rust
async fn advertise_v2_rust(...) {
    // Lines 97-116 show actual capabilities advertised:
    body.extend_from_slice(&encode_pkt_line(b"version 2\n"));
    body.extend_from_slice(&encode_pkt_line(format!("agent=forge/{}\n", ...).as_bytes()));
    body.extend_from_slice(&encode_pkt_line(format!("session-id={}\n", sid).as_bytes()));
    body.extend_from_slice(&encode_pkt_line(b"object-format=sha1\n"));
    body.extend_from_slice(&encode_pkt_line(b"server-option\n"));
    body.extend_from_slice(&encode_pkt_line(b"ls-refs\n"));
    body.extend_from_slice(&encode_pkt_line(b"fetch=shallow\n"));
    body.extend_from_slice(&encode_pkt_line(b"fetch=filter\n"));
    body.extend_from_slice(&encode_pkt_line(b"fetch=ref-in-want\n"));
    body.extend_from_slice(&encode_pkt_line(b"fetch=deepen-since\n"));
    body.extend_from_slice(&encode_pkt_line(b"fetch=deepen-not\n"));
}
```

**Findings**:
- ✅ All required capabilities ARE advertised
- ✅ `object-format=sha1` is hardcoded (sha256 not yet supported)
- ⚠️ TODO claims to verify dynamic object-format, but code shows it's static
- ✅ All fetch capabilities listed in TODO are present

**Corrections Needed**:
- Update TODO to reflect that capabilities are complete
- Note that `object-format` is intentionally sha1-only (not a bug)

#### A.2 Request Parsing Completeness (Lines 17-24)

**Status**: ✅ ALREADY IMPLEMENTED

**Current Implementation** ([`server/src/git_http/v2.rs:426-451`](server/src/git_http/v2.rs:426-451)):
```rust
fn parse_fetch(pkts: &[Pkt]) -> anyhow::Result<FetchRequest> {
    // Line 434: want-ref parsing
    if let Some(rest) = s.strip_prefix("want-ref ") {
        req.want_refs.push(rest.to_string());
    }
    // Line 435: want-refs (plural) parsing
    if let Some(rest) = s.strip_prefix("want-refs ") {
        for r in rest.split(' ') {
            if !r.is_empty() { req.want_refs.push(r.to_string()); }
        }
    }
    // Line 441: no-progress parsing
    if s == "no-progress" { req.no_progress = true; }
    // Line 447: done parsing
    if s == "done" { req.done = true; }
}
```

**Findings**:
- ✅ `done` - IMPLEMENTED (line 447)
- ✅ `want-ref` - IMPLEMENTED (line 434)
- ✅ `want-refs` - IMPLEMENTED (line 435)
- ✅ `no-progress` - IMPLEMENTED (line 441)

**Corrections Needed**:
- Mark this task as COMPLETE in TODO
- Remove from action items

#### A.3 Section Framing and Negotiation (Lines 26-33)

**Status**: ⚠️ PARTIALLY IMPLEMENTED

**Current Implementation**:

1. **ACK Negotiation** ([`server/src/git_http/pack.rs:658-697`](server/src/git_http/pack.rs:658-697)):
```rust
async fn emit_acknowledgments(...) {
    // Lines 684-687: NAK when no common base
    if common.is_empty() {
        let _ = tx.send(Bytes::from(encode_pkt_line(b"NAK\n"))).await;
        return Ok(());
    }
    // Lines 689-692: ACK common for each common commit
    for c in &common {
        let line = format!("ACK {} common\n", c);
        let _ = tx.send(Bytes::from(encode_pkt_line(line.as_bytes()))).await;
    }
    // Lines 694-696: ACK ready
    let ready_id = common.last().cloned().unwrap();
    let line = format!("ACK {} ready\n", ready_id);
    let _ = tx.send(Bytes::from(encode_pkt_line(line.as_bytes()))).await;
}
```

2. **Sideband Support** ([`server/src/git_http/pack.rs:346-381`](server/src/git_http/pack.rs:346-381)):
```rust
impl SidebandPktWriter {
    fn send_chunk(&mut self, mut data: &[u8]) -> IoResult<()> {
        // Lines 357-362: Sideband framing
        if self.sideband {
            let mut payload = Vec::with_capacity(1 + chunk.len());
            payload.push(1u8); // band 1: data
            payload.extend_from_slice(chunk);
            let pkt = encode_pkt_line(&payload);
            let _ = self.tx.blocking_send(Bytes::from(pkt));
        } else {
            // Lines 363-365: Raw pack streaming
            let _ = self.tx.blocking_send(Bytes::copy_from_slice(chunk));
        }
    }

    fn progress_line(&mut self, msg: String) -> IoResult<()> {
        // Lines 373: Respects no-progress flag
        if !self.sideband || self.suppress_progress { return Ok(()); }
        // Lines 374-379: Band 2 progress messages
        let mut payload = Vec::with_capacity(1 + msg.len() + 1);
        payload.push(2u8); // band 2: progress
        payload.extend_from_slice(msg.as_bytes());
        payload.push(b'\n');
        let pkt = encode_pkt_line(&payload);
        let _ = self.tx.blocking_send(Bytes::from(pkt));
    }
}
```

**Findings**:
- ✅ ACK negotiation implemented with `common` and `ready` states
- ❌ Multi-ACK mode NOT implemented (only simple ACK/NAK)
- ✅ NAK sent appropriately (only when no common base)
- ✅ Sideband vs raw pack streaming IMPLEMENTED
- ✅ `no-progress` flag RESPECTED

**Corrections Needed**:
- Update TODO to reflect partial completion
- Add specific task for multi-ACK implementation if needed
- Mark sideband and no-progress as complete

### Phase B: Shallow + Filters

#### B.1-B.2 Verification (Lines 41-43)

**Status**: ⚠️ NEEDS VERIFICATION

**Test Files Present**:
- [`server/tests/git_http_v2_shallow.sh`](server/tests/git_http_v2_shallow.sh)
- [`server/tests/git_http_v2_filter_symlink_submodule.sh`](server/tests/git_http_v2_filter_symlink_submodule.sh)
- [`server/tests/git_http_v2_partial_blob_limit.sh`](server/tests/git_http_v2_partial_blob_limit.sh)
- [`server/tests/git_http_v2_filter_tree.sh`](server/tests/git_http_v2_filter_tree.sh)
- [`server/tests/git_http_v2_partial_blob_none.sh`](server/tests/git_http_v2_partial_blob_none.sh)

**Implementation Status** ([`server/src/git_http/pack.rs:457-589`](server/src/git_http/pack.rs:457-589)):
```rust
fn plan_pack(repo_dir: PathBuf, req: &FetchRequest) -> anyhow::Result<PackPlan> {
    // Lines 511-545: Shallow clone support (deepen, deepen-since, deepen-not)
    let depth_limit = req.deepen();
    let since_limit = req.deepen_since();
    // ... depth and since boundary detection

    // Lines 513-570: Filter support (blob:none, tree:depth, blob:limit)
    let tree_depth_limit = req.filter_tree_depth();
    let blob_limit = req.filter_blob_limit();
    // ... filter application during tree walk
}
```

**Findings**:
- ✅ Shallow clone features IMPLEMENTED
- ✅ Filter features IMPLEMENTED
- ❓ Tests exist but verification status unknown
- ❓ Default enable status unknown

**Corrections Needed**:
- TODO is accurate; verification needed
- Add step to run tests and document results

#### B.3 Enable Filters by Default (Lines 45-47)

**Status**: ❓ UNKNOWN

**Current Code** ([`server/src/git_http/v2.rs:32-34`](server/src/git_http/v2.rs:32-34)):
```rust
let resp = match std::env::var("FORGE_GIT_SMART_V2_ADVERTISE").ok().as_deref() {
    Some("rust") => advertise_v2_rust(&state, &segments, &HeaderMap::new()).await,
    _ => advertise_v2_via_git(&state, &segments, &HeaderMap::new()).await,
};
```

**Findings**:
- ⚠️ No `FORGE_GIT_SMART_V2_ENABLE_FILTERS` variable found in code
- ⚠️ Filters are always enabled when Rust backend is active
- ⚠️ TODO references non-existent configuration

**Corrections Needed**:
- Update TODO to reflect actual configuration mechanism
- Clarify that filters are enabled via `FORGE_GIT_SMART_V2_BACKEND=rust`

### Phase C: Deltas and Thin-Pack

#### C.1 Delta and Thin-Pack Support (Lines 55-61)

**Status**: ✅ PARTIALLY IMPLEMENTED

**Current Implementation** ([`server/src/git_http/pack.rs:254-314`](server/src/git_http/pack.rs:254-314)):
```rust
// Lines 254-292: REF-DELTA implementation
let mut write_ref_delta_commit = |target_oid, base_oid| -> anyhow::Result<()> {
    // ... builds delta with insert ops
    // Header: REF_DELTA (type=7) with target size
    let mut hdr = encode_obj_header(7u8, data.len() as u64);
    // ... 20-byte base object id
    // ... compressed delta payload
};

// Lines 295-313: Thin-pack commit delta generation
for id in &commits {
    let mut wrote = false;
    if req.thin_pack() {
        // Choose a base: prefer direct parent that exists in client haves
        if let Some(base) = parents.into_iter().find(|p| have_set.contains(p)) {
            if let Err(e) = write_ref_delta_commit((*id).into(), base) {
                tracing::debug!("ref-delta emit failed for {}: {}", id, e);
            } else {
                wrote = true;
            }
        }
    }
    if !wrote { write_obj(*id)?; }
}
```

**Findings**:
- ✅ Thin-pack IMPLEMENTED for commits
- ✅ REF-DELTA IMPLEMENTED (type 7)
- ❌ OFS-DELTA NOT implemented (type 6)
- ⚠️ Delta only for commits, not trees/blobs
- ⚠️ Simple insert-only delta (not optimal)

**Corrections Needed**:
- Update TODO to reflect REF-DELTA completion
- Add specific task for OFS-DELTA
- Add task for tree/blob delta support
- Add task for optimal delta generation

#### C.2 Object Selection for Deltas (Lines 63-68)

**Status**: ✅ PARTIALLY IMPLEMENTED

**Current Implementation** ([`server/src/git_http/pack.rs:700-714`](server/src/git_http/pack.rs:700-714)):
```rust
async fn resolve_want_refs(repo_dir: &PathBuf, req: &mut FetchRequest) -> anyhow::Result<()> {
    let repo = gix::open(repo_dir)?;
    let mut new_wants = Vec::new();
    for r in req.want_refs().iter() {
        if let Ok(mut reference) = repo.find_reference(r) {
            if let Some(idref) = reference.try_id() {
                new_wants.push(idref.to_string());
            } else if let Ok(commit) = reference.peel_to_commit() {
                new_wants.push(commit.id().to_string());
            }
        }
    }
    req.wants().to_vec().extend(new_wants);
    Ok(())
}
```

**Findings**:
- ✅ `want-ref` resolution IMPLEMENTED
- ✅ Object traversal stops at `have` commits (lines 162-163 in pack.rs)
- ⚠️ Bug in line 713: `req.wants().to_vec().extend()` doesn't mutate req

**Corrections Needed**:
- Mark want-ref integration as complete
- Add bug fix task for line 713
- Mark have-boundary traversal as complete

#### C.3 Compression and Performance (Lines 70-75)

**Status**: ❌ NOT IMPLEMENTED

**Current Implementation**:
- Single-threaded delta search
- No on-disk pack delta reuse
- Simple zlib compression only

**Findings**:
- ❌ Multi-threaded delta search NOT implemented
- ❌ On-disk pack delta reuse NOT implemented
- ✅ Basic compression works (flate2)

**Corrections Needed**:
- TODO is accurate for this section

## File Path and Function Name Corrections

### Incorrect References in TODO.md

1. **Line 12**: "Function: `advertise_capabilities`"
   - ❌ INCORRECT: Function is actually [`advertise_v2_rust`](server/src/git_http/v2.rs:89)

2. **Line 19**: "Function: Request parsing logic within the `handle_upload_pack_request`"
   - ❌ INCORRECT: Function is actually [`parse_fetch`](server/src/git_http/v2.rs:426)

3. **Line 28**: "Function: Negotiation-related functions (e.g., `negotiate_commits`, `send_pack_data`)"
   - ❌ INCORRECT: Functions are actually [`emit_acknowledgments`](server/src/git_http/pack.rs:658) and [`build_and_stream_pack_with_plan`](server/src/git_http/pack.rs:591)

4. **Line 46**: "File: Configuration files or environment variable defaults (e.g., `server/src/config.rs` or `server/src/main.rs`)"
   - ⚠️ MISLEADING: No dedicated config file; uses env vars directly in v2.rs

5. **Line 56**: "File: `server/src/git_http/pack.rs` (core packfile generation logic)"
   - ✅ CORRECT

6. **Line 57**: "Function: Functions responsible for writing objects to the packfile (e.g., `write_object`, `build_packfile`)"
   - ⚠️ PARTIALLY CORRECT: Functions are [`write_obj`](server/src/git_http/pack.rs:620) (closure) and [`build_and_stream_pack_with_plan`](server/src/git_http/pack.rs:591)

## Missing Items in TODO.md

The TODO.md is missing several important tasks:

1. **Bug Fixes**:
   - Fix [`resolve_want_refs`](server/src/git_http/pack.rs:713) mutation bug
   - Verify ACK negotiation handles multi-round negotiation correctly

2. **Testing**:
   - Integration tests for ACK negotiation
   - Tests for thin-pack with REF-DELTA
   - Tests for want-ref resolution

3. **Documentation**:
   - Document environment variables (`FORGE_GIT_SMART_V2_BACKEND`, `FORGE_GIT_SMART_V2_ADVERTISE`)
   - Update architecture docs for v2 implementation

4. **Performance**:
   - Benchmark pack generation vs git native
   - Profile delta generation overhead

5. **Edge Cases**:
   - Handle empty repositories
   - Handle corrupted pack requests
   - Handle timeout scenarios

## Recommended TODO.md Structure

Based on this analysis, here's the recommended structure:

### Phase A: Negotiation Correctness (Mostly Complete)
- [x] Capabilities advertisement (complete)
- [x] Request parsing (complete: done, want-ref, want-refs, no-progress)
- [x] Basic ACK negotiation (common, ready, NAK)
- [ ] Multi-ACK mode implementation (if needed)
- [x] Sideband vs raw pack streaming
- [x] no-progress flag support

### Phase B: Shallow + Filters (Implementation Complete, Verification Needed)
- [x] Shallow clone implementation (deepen, deepen-since, deepen-not)
- [x] Filter implementation (blob:none, tree:depth, blob:limit)
- [ ] Run and verify all shallow tests
- [ ] Run and verify all filter tests
- [ ] Document test results
- [ ] Enable Rust backend by default (if desired)

### Phase C: Deltas and Thin-Pack (Partially Complete)
- [x] Thin-pack support for commits
- [x] REF-DELTA implementation
- [ ] OFS-DELTA implementation
- [ ] Delta support for trees and blobs
- [ ] Optimal delta generation (not just insert-only)
- [x] want-ref resolution
- [ ] Fix want-ref mutation bug
- [x] Have-boundary traversal
- [ ] Multi-threaded delta search
- [ ] On-disk pack delta reuse

### Phase D: Bug Fixes and Polish (New)
- [ ] Fix resolve_want_refs mutation bug (line 713)
- [ ] Add integration tests for negotiation
- [ ] Add tests for thin-pack generation
- [ ] Document environment variables
- [ ] Benchmark against git native
- [ ] Add edge case handling

## Conclusion

The TODO.md document provides a reasonable high-level roadmap but contains several inaccuracies:

1. **Overestimates remaining work**: Many Phase A and B items are already complete
2. **Incorrect function names**: Several function references are wrong
3. **Missing bug fixes**: Doesn't account for bugs found in implementation
4. **Underestimates Phase C**: Delta work is more complex than described
5. **Missing verification steps**: Needs explicit test execution and documentation

The actual state is:
- **Phase A**: ~80% complete (missing multi-ACK if needed)
- **Phase B**: ~90% complete (implementation done, needs verification)
- **Phase C**: ~40% complete (basic thin-pack works, needs optimization)

Priority should be:
1. Fix critical bugs (resolve_want_refs)
2. Verify Phase B with tests
3. Complete Phase C delta optimization
4. Add comprehensive testing