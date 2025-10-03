//! Packfile assembly and streaming via gix (pure-Rust backend).

use axum::body::Body;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use bytes::Bytes;
use std::collections::{HashSet, VecDeque};
use std::io::{Result as IoResult, Write};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use futures::StreamExt;
use sha1::Digest;
use metrics::{counter, histogram};

use crate::git_http::pkt::{encode_pkt_line, PKT_FLUSH, PKT_DELIM};
use crate::git_http::v2::FetchRequest;

#[allow(dead_code)]
pub struct PackBuildStats {
    pub objects: usize,
    pub bytes: u64,
}

#[derive(Clone)]
struct PackPlan {
    commits: Vec<gix::hash::ObjectId>,
    trees: Vec<gix::hash::ObjectId>,
    blobs: Vec<gix::hash::ObjectId>,
    shallows: Vec<gix::hash::ObjectId>,
}

/// Serve a Smart HTTP v2 `fetch` response by building a pack in-process and streaming it.
///
/// Response section framing (protocol v2):
/// - optional "acknowledgments" section (ACK/NAK) if client sent `have` lines, then a pkt-delim (0001)
/// - optional "shallow-info" section if deepen/filter imply shallows, then a pkt-delim (0001).
/// - required  "packfile" section header followed by sideband(1) framed pack bytes; final pkt-flush (0000)
pub async fn serve_fetch(repo_dir: &PathBuf, req: &FetchRequest, _headers: &HeaderMap, _body_limit: usize) -> Response {
    // Channel to stream pkt-line framed bytes out to the client
    let (tx, rx) = mpsc::channel::<Bytes>(16);

    // Resolve want-ref(s) into object ids and augment wants list
    let mut req_effective = req.clone();
    if !req.want_refs().is_empty() {
        if let Err(e) = resolve_want_refs(repo_dir, &mut req_effective).await {
            tracing::debug!("resolve_want_refs failed: {}", e);
        }
    }

    // If client sent haves and did not also send 'done', emit an acknowledgments section.
    // Per protocol v2, if the client sends 'done', the acknowledgments section MUST be omitted.
    tracing::info!(has_haves = %req_effective.has_haves(), done = %req_effective.done(), "fetch negotiation flags");
    let mut ack_ready = true;
    if req_effective.has_haves() && !req_effective.done() {
        let _ = tx.send(Bytes::from(encode_pkt_line(b"acknowledgments\n"))).await;
        // Compute simple ACK set by intersecting haves with reachable commits from wants.
        match emit_acknowledgments(repo_dir, &req_effective, &tx).await {
            Ok(rdy) => { ack_ready = rdy; }
            Err(e) => {
                tracing::debug!("acknowledgments generation failed: {}", e);
                // As a fallback, emit a NAK so the client proceeds.
                let _ = tx.send(Bytes::from(encode_pkt_line(b"NAK\n"))).await;
                ack_ready = false;
            }
        }
        tracing::info!(ack_ready = ack_ready, "acknowledgments section done");
        if ack_ready {
            // When we send 'ready', we should NOT send any other sections according to the protocol
            // The client expects the response to end after the 'ready' ACK
            // Send flush to end the response immediately, not a section delimiter
            let _ = tx.send(Bytes::from_static(PKT_FLUSH)).await;
            let stream = ReceiverStream::new(rx).map(Ok::<Bytes, std::convert::Infallible>);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/x-git-upload-pack-result")
                .header(header::CACHE_CONTROL, "no-cache")
                .header(header::PRAGMA, "no-cache")
                .body(Body::from_stream(stream))
                .expect("response");
        } else {
            // Per protocol v2: without 'ready', end response with FLUSH and do not send packfile
            // Send section delimiter and then flush
            let _ = tx.send(Bytes::from_static(PKT_DELIM)).await;
            let _ = tx.send(Bytes::from_static(PKT_FLUSH)).await;
            let stream = ReceiverStream::new(rx).map(Ok::<Bytes, std::convert::Infallible>);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/x-git-upload-pack-result")
                .header(header::CACHE_CONTROL, "no-cache")
                .header(header::PRAGMA, "no-cache")
                .body(Body::from_stream(stream))
                .expect("response");
        }
    }

    // Compute traversal plan (objects + shallow boundaries)
    // This is now executed for all cases including when ack_ready is true
    let repo_path_for_plan = repo_dir.clone();
    let req_for_plan = req_effective.clone();
    let plan = match tokio::task::spawn_blocking(move || plan_pack(repo_path_for_plan, &req_for_plan)).await {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            tracing::warn!("plan_pack failed: {}", e);
            let mut body = Vec::new();
            body.extend_from_slice(&encode_pkt_line(b"ERR planning failed\n"));
            body.extend_from_slice(PKT_FLUSH);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/x-git-upload-pack-result")
                .header(header::CACHE_CONTROL, "no-cache")
                .header(header::PRAGMA, "no-cache")
                .body(Body::from(body))
                .unwrap();
        }
        Err(e) => {
            tracing::warn!("plan_pack join error: {}", e);
            let mut body = Vec::new();
            body.extend_from_slice(&encode_pkt_line(b"ERR internal error\n"));
            body.extend_from_slice(PKT_FLUSH);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/x-git-upload-pack-result")
                .header(header::CACHE_CONTROL, "no-cache")
                .header(header::PRAGMA, "no-cache")
                .body(Body::from(body))
                .unwrap();
        }
    };

    // Optional shallow-info section if client requested shallow/deepen semantics.
    // When ack_ready is true, we skip this section entirely
    if req_effective.shallow_requested() {
        let _ = tx.send(Bytes::from(encode_pkt_line(b"shallow-info\n"))).await;
        // New shallow tips after this fetch
        for oid in &plan.shallows {
            let line = format!("shallow {}\n", oid);
            let _ = tx.send(Bytes::from(encode_pkt_line(line.as_bytes()))).await;
        }
        // Commits that were previously shallow on the client but are no longer shallow
        use std::collections::HashSet as HS;
        let new_set: HS<String> = plan.shallows.iter().map(|o| o.to_string()).collect();
        for s in req_effective.client_shallows() {
            if !new_set.contains(s) {
                let line = format!("unshallow {}\n", s);
                let _ = tx.send(Bytes::from(encode_pkt_line(line.as_bytes()))).await;
            }
        }
        let _ = tx.send(Bytes::from_static(PKT_DELIM)).await;
    }

    // Start the packfile section
    tracing::info!("sending packfile header");
    let _ = tx.send(Bytes::from(encode_pkt_line(b"packfile\n"))).await;

    let repo_path = repo_dir.clone();
    // In protocol v2 over HTTP, packfile bytes are sent using side-band-64k framing.
    // Always enable sideband to match git-http-backend behavior and client expectations.
    let sideband_64k = true;
    let req_clone = req_effective.clone();
    let plan_clone = plan.clone();

    // Spawn blocking task to build and stream the packfile
    let pack_task = tokio::task::spawn_blocking(move || {
        if let Err(err) = build_and_stream_pack_with_plan(repo_path, &req_clone, sideband_64k, plan_clone, tx) {
            tracing::warn!("pack streaming failed: {}", err);
        }
    });

    // Wait for the packfile streaming to complete before returning the response
    let _ = pack_task.await;

    let stream = ReceiverStream::new(rx).map(Ok::<Bytes, std::convert::Infallible>);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-git-upload-pack-result")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::PRAGMA, "no-cache")
        .body(Body::from_stream(stream))
        .expect("response")
}

fn build_and_stream_pack(
    repo_dir: PathBuf,
    req: &FetchRequest,
    sideband_64k: bool,
    tx: mpsc::Sender<Bytes>,
) -> anyhow::Result<()> {
    let repo = gix::open(repo_dir)?;

    // Collect reachable objects starting from wants (commits), stop at direct 'have' commits.
    let mut want_commits: VecDeque<gix::hash::ObjectId> = VecDeque::new();
    let mut seen: HashSet<gix::hash::ObjectId> = HashSet::new();
    for w in req.wants() {
        if let Ok(oid) = gix::hash::ObjectId::from_hex(w.as_bytes()) { want_commits.push_back(oid); }
    }
    let mut have_set: HashSet<gix::hash::ObjectId> = HashSet::new();
    for h in req.haves() {
        if let Ok(oid) = gix::hash::ObjectId::from_hex(h.as_bytes()) { have_set.insert(oid); }
    }

    let mut commits = Vec::new();
    let mut trees = VecDeque::new();
    let mut blobs: Vec<gix::hash::ObjectId> = Vec::new();

    while let Some(cid) = want_commits.pop_front() {
        if have_set.contains(&cid) { continue; } // boundary: client already has this commit
        if !seen.insert(cid) { continue; }
        let commit = match repo.find_object(cid) { Ok(o) => o, Err(_) => continue };
        if commit.kind != gix::objs::Kind::Commit { continue; }
        let (tree_id, parents) = parse_commit_raw(commit.data.as_ref())?;
        commits.push(cid);
        trees.push_back(tree_id);
        for parent in parents { want_commits.push_back(parent); }
    }

    // walk tree recursively to collect all trees and blobs
    let mut seen_tree = HashSet::new();
    while let Some(tid) = trees.pop_front() {
        if !seen_tree.insert(tid) { continue; }
        let tree = repo.find_object(tid)?;
        let t = gix::objs::TreeRef::from_bytes(tree.data.as_ref())?;
        for entry in t.entries.iter() {
            if entry.mode.is_tree() {
                trees.push_back(entry.oid.into());
            } else if entry.mode.is_blob() || entry.mode.is_link() {
                if !req.filter_blob_none() {
                    blobs.push(entry.oid.into());
                }
            }
        }
    }

    // Prepare pack writer with header and SHA1 trailer
    let mut out = SidebandPktWriter::new(tx.clone(), sideband_64k, req.no_progress());
    let start = std::time::Instant::now();
    let mut hasher = sha1::Sha1::new();

    // Minimal progress (band 2) for client UX when no-progress is off
    let total_objects = (commits.len() + seen_tree.len() + blobs.len()) as u32;
    if sideband_64k {
        let _ = out.progress_line(format!("Enumerating objects: {}", total_objects));
        if req.filter_requested() {
            let _ = out.progress_line("Filter requested; sending full objects".to_string());
        }
    }

    // Write pack header: 'PACK' + version(2) + num_objects
    let mut header = Vec::with_capacity(12);
    header.extend_from_slice(b"PACK");
    header.extend_from_slice(&2u32.to_be_bytes());
    header.extend_from_slice(&total_objects.to_be_bytes());
    hasher.update(&header);
    out.send_chunk(&header)?;

    fn encode_varint(mut n: u64) -> Vec<u8> {
        let mut v = Vec::new();
        loop {
            let mut byte = (n & 0x7f) as u8;
            n >>= 7;
            if n != 0 { byte |= 0x80; }
            v.push(byte);
            if n == 0 { break; }
        }
        v
    }

    fn write_full_object(
        repo: &gix::Repository,
        out: &mut SidebandPktWriter,
        hasher: &mut sha1::Sha1,
        oid: gix::hash::ObjectId,
    ) -> anyhow::Result<()> {
        let obj = repo.find_object(oid)?;
        let (kind, data) = match obj.kind {
            gix::objs::Kind::Commit => (1u8, obj.data.to_vec()),
            gix::objs::Kind::Tree => (2u8, obj.data.to_vec()),
            gix::objs::Kind::Blob => (3u8, obj.data.to_vec()),
            gix::objs::Kind::Tag => (4u8, obj.data.to_vec()),
        };
        let hdr = encode_obj_header(kind, data.len() as u64);
        hasher.update(&hdr);
        out.send_chunk(&hdr)?;
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&data)?;
        let compressed = encoder.finish()?;
        hasher.update(&compressed);
        out.send_chunk(&compressed)?;
        Ok(())
    }

    fn write_commit_ref_delta(
        repo: &gix::Repository,
        out: &mut SidebandPktWriter,
        hasher: &mut sha1::Sha1,
        target_oid: gix::hash::ObjectId,
        base_oid: gix::hash::ObjectId,
    ) -> anyhow::Result<()> {
        let base_obj = repo.find_object(base_oid)?;
        if base_obj.kind != gix::objs::Kind::Commit { anyhow::bail!("delta base is not a commit"); }
        let base_size = base_obj.data.len() as u64;

        let targ_obj = repo.find_object(target_oid)?;
        if targ_obj.kind != gix::objs::Kind::Commit { anyhow::bail!("target is not a commit"); }
        let data = targ_obj.data.to_vec();

        let mut delta = Vec::with_capacity(16 + data.len());
        delta.extend_from_slice(&encode_varint(base_size));
        delta.extend_from_slice(&encode_varint(data.len() as u64));
        let mut i = 0;
        while i < data.len() {
            let take = (data.len() - i).min(127);
            delta.push(take as u8);
            delta.extend_from_slice(&data[i..i + take]);
            i += take;
        }

        let hdr = encode_obj_header(7u8, data.len() as u64);
        hasher.update(&hdr);
        out.send_chunk(&hdr)?;

        let base_raw = base_oid.as_bytes();
        hasher.update(base_raw);
        out.send_chunk(base_raw)?;

        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&delta)?;
        let compressed = encoder.finish()?;
        hasher.update(&compressed);
        out.send_chunk(&compressed)?;
        Ok(())
    }

    let mut have_set: HashSet<gix::hash::ObjectId> = HashSet::new();
    for h in req.haves() {
        if let Ok(oid) = gix::hash::ObjectId::from_hex(h.as_bytes()) { have_set.insert(oid); }
    }

    for id in &commits {
        // For correctness first: emit full objects even if the client requested thin-pack.
        // Proper delta encoding (copy/insert ops) can be added later.
        write_full_object(&repo, &mut out, &mut hasher, (*id).into())?;
    }

    for id in &seen_tree {
        write_full_object(&repo, &mut out, &mut hasher, (*id).into())?;
    }
    for id in &blobs {
        write_full_object(&repo, &mut out, &mut hasher, (*id).into())?;
    }

    let trailer = hasher.finalize();
    out.send_chunk(trailer.as_slice())?;

    if sideband_64k && !req.no_progress() {
        let _ = out.progress_line("Done".to_string());
    }

    // metrics
    counter!("git_http.pack.objects").increment(total_objects as u64);
    // best-effort byte count: header + objects + trailer were all sent through SidebandPktWriter; we can't know exact framed bytes here.
    // Still, record logical pack bytes: header.len() + sum(compressed) + 20 trailer
    // Not tracked precisely; emit 0 to ensure metric exists
    histogram!("git_http.pack.logical_bytes").record(0.0);
    histogram!("git_http.pack.build_ms").record(start.elapsed().as_millis() as f64);

    // Final flush for the whole fetch response
    tx.blocking_send(Bytes::from_static(PKT_FLUSH)).ok();
    Ok(())
}

struct SidebandPktWriter {
    tx: mpsc::Sender<Bytes>,
    max_payload: usize,
    sideband: bool,
    suppress_progress: bool,
}

impl SidebandPktWriter {
    fn new(tx: mpsc::Sender<Bytes>, sideband_64k: bool, suppress_progress: bool) -> Self {
        // payload length excluding 4-byte length prefix; reserve 1 byte for the band id
        let max_payload = if sideband_64k { 65520 - 4 - 1 } else { 32768 };
        Self { tx, max_payload, sideband: sideband_64k, suppress_progress }
    }

    fn send_chunk(&mut self, mut data: &[u8]) -> IoResult<()> {
        while !data.is_empty() {
            let take = data.len().min(self.max_payload);
            let chunk = &data[..take];
            if self.sideband {
                let mut payload = Vec::with_capacity(1 + chunk.len());
                payload.push(1u8); // band 1: data
                payload.extend_from_slice(chunk);
                let pkt = encode_pkt_line(&payload);
                let _ = self.tx.blocking_send(Bytes::from(pkt));
            } else {
                // Raw pack bytes (no pkt-line framing) when sideband not negotiated
                let _ = self.tx.blocking_send(Bytes::copy_from_slice(chunk));
            }
            data = &data[take..];
        }
        Ok(())
    }

    fn progress_line(&mut self, msg: String) -> IoResult<()> {
        if !self.sideband || self.suppress_progress { return Ok(()); }
        let mut payload = Vec::with_capacity(1 + msg.len() + 1);
        payload.push(2u8); // band 2: progress
        payload.extend_from_slice(msg.as_bytes());
        payload.push(b'\n');
        let pkt = encode_pkt_line(&payload);
        let _ = self.tx.blocking_send(Bytes::from(pkt));
        Ok(())
    }
}

impl Write for SidebandPktWriter {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.send_chunk(buf)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

fn encode_obj_header(kind: u8, size: u64) -> Vec<u8> {
    // See git pack v2: first byte has lower 4 bits of size, bits 4-6 type, bit7 continuation
    let mut n = size;
    let mut first = (n & 0x0f) as u8 | (kind << 4);
    n >>= 4;
    let mut out = Vec::new();
    if n != 0 {
        first |= 0x80;
    }
    out.push(first);
    while n != 0 {
        let mut byte = (n & 0x7f) as u8;
        n >>= 7;
        if n != 0 {
            byte |= 0x80;
        }
        out.push(byte);
    }
    out
}

fn parse_commit_raw(data: &[u8]) -> anyhow::Result<(gix::hash::ObjectId, Vec<gix::hash::ObjectId>)> {
    use anyhow::Context as _;
    let mut tree: Option<gix::hash::ObjectId> = None;
    let mut parents = Vec::new();
    for line in data.split(|b| *b == b'\n') {
        if line.is_empty() { break; }
        if let Some(rest) = line.strip_prefix(b"tree ") {
            let oid = gix::hash::ObjectId::from_hex(rest).context("parse commit tree")?;
            tree = Some(oid);
        } else if let Some(rest) = line.strip_prefix(b"parent ") {
            let oid = gix::hash::ObjectId::from_hex(rest).context("parse commit parent")?;
            parents.push(oid);
        }
    }
    Ok((tree.context("commit missing tree")?, parents))
}

fn parse_commit_meta(data: &[u8]) -> anyhow::Result<(gix::hash::ObjectId, Vec<gix::hash::ObjectId>, i64)> {
    use anyhow::Context as _;
    let mut tree: Option<gix::hash::ObjectId> = None;
    let mut parents = Vec::new();
    let mut ts: Option<i64> = None;
    for line in data.split(|b| *b == b'\n') {
        if line.is_empty() { break; }
        if let Some(rest) = line.strip_prefix(b"tree ") {
            let oid = gix::hash::ObjectId::from_hex(rest).context("parse commit tree")?;
            tree = Some(oid);
        } else if let Some(rest) = line.strip_prefix(b"parent ") {
            let oid = gix::hash::ObjectId::from_hex(rest).context("parse commit parent")?;
            parents.push(oid);
        } else if let Some(rest) = line.strip_prefix(b"committer ") {
            // committer Name <email> <timestamp> <tz>
            // We find the penultimate space-separated token as timestamp
            let parts: Vec<&[u8]> = rest.split(|b| *b == b' ').collect();
            if parts.len() >= 2 {
                if let Some(val) = std::str::from_utf8(parts[parts.len()-2]).ok().and_then(|s| s.parse::<i64>().ok()) { ts = Some(val); }
            }
        }
    }
    Ok((tree.context("commit missing tree")?, parents, ts.unwrap_or(0)))
}

fn plan_pack(repo_dir: PathBuf, req: &FetchRequest) -> anyhow::Result<PackPlan> {
    let repo = gix::open(repo_dir)?;

    // Start from wants
    let mut want_q: VecDeque<(gix::hash::ObjectId, u32)> = VecDeque::new();
    let mut seen: HashSet<gix::hash::ObjectId> = HashSet::new();
    let mut direct_blobs: Vec<gix::hash::ObjectId> = Vec::new();
    let mut tree_queue: VecDeque<(gix::hash::ObjectId, u32)> = VecDeque::new();
    for w in req.wants() {
        if let Ok(oid) = gix::hash::ObjectId::from_hex(w.as_bytes()) {
            if let Ok(obj) = repo.find_object(oid) {
                match obj.kind {
                    gix::objs::Kind::Commit => want_q.push_back((oid, 0)),
                    gix::objs::Kind::Tree => tree_queue.push_back((oid, 0)),
                    gix::objs::Kind::Blob => direct_blobs.push(oid),
                    gix::objs::Kind::Tag => (),
                }
            } else {
                want_q.push_back((oid, 0));
            }
        }
    }

    // Client haves (stop traversal at these)
    let mut have_set: HashSet<gix::hash::ObjectId> = HashSet::new();
    for h in req.haves() {
        if let Ok(oid) = gix::hash::ObjectId::from_hex(h.as_bytes()) { have_set.insert(oid); }
    }

    // Exclusions from deepen-not: build full reachable set from each excluded ref tip
    let mut exclude: HashSet<gix::hash::ObjectId> = HashSet::new();
    for r in req.deepen_not().iter() {
        if let Ok(reference) = repo.find_reference(r) {
            if let Some(idref) = reference.try_id() {
                let tip = idref.detach();
                let mut q: VecDeque<gix::hash::ObjectId> = VecDeque::new();
                q.push_back(tip);
                while let Some(id) = q.pop_front() {
                    if !exclude.insert(id) { continue; }
                    if let Ok(obj) = repo.find_object(id) {
                        if obj.kind == gix::objs::Kind::Commit {
                            let (_, parents) = parse_commit_raw(obj.data.as_ref())?;
                            for p in parents { q.push_back(p); }
                        }
                    }
                }
            }
        }
    }

    let mut commits: Vec<gix::hash::ObjectId> = Vec::new();
    let mut blobs: Vec<gix::hash::ObjectId> = Vec::new();
    let mut shallows: HashSet<gix::hash::ObjectId> = HashSet::new();

    let depth_limit = req.deepen();
    let since_limit = req.deepen_since();
    let tree_depth_limit = req.filter_tree_depth();
    let blob_limit = req.filter_blob_limit();

    while let Some((cid, d)) = want_q.pop_front() {
        if have_set.contains(&cid) { continue; }
        if !seen.insert(cid) { continue; }
        let commit = match repo.find_object(cid) { Ok(o) => o, Err(_) => continue };
        if commit.kind != gix::objs::Kind::Commit { continue; }
        let (tree_id, parents, _ts) = parse_commit_meta(commit.data.as_ref())?;
        commits.push(cid);
        tree_queue.push_back((tree_id, 0));

        // Traverse parents with constraints
        for p in parents {
            if have_set.contains(&p) { continue; }
            // Depth: do not cross if next depth would exceed limit
            if let Some(maxd) = depth_limit {
                let nd = d + 1;
                if nd > maxd { shallows.insert(cid); continue; }
            }
            // Exclude set boundary
            if exclude.contains(&p) { shallows.insert(cid); continue; }
            // Since limit boundary
            if let Some(since) = since_limit {
                if let Ok(obj) = repo.find_object(p) {
                    if obj.kind == gix::objs::Kind::Commit {
                        let (_, _, pts) = parse_commit_meta(obj.data.as_ref())?;
                        if pts < since { shallows.insert(cid); continue; }
                    }
                }
            }
            want_q.push_back((p, d + 1));
        }
    }

    // Walk trees to collect all referenced trees and blobs
    let mut seen_tree = HashSet::new();
    while let Some((tid, depth)) = tree_queue.pop_front() {
        if !seen_tree.insert(tid) { continue; }
        let tree = repo.find_object(tid)?;
        let t = gix::objs::TreeRef::from_bytes(tree.data.as_ref())?;
        for entry in t.entries.iter() {
            if entry.mode.is_tree() {
                if tree_depth_limit.map(|lim| depth < lim).unwrap_or(true) {
                    tree_queue.push_back((entry.oid.into(), depth + 1));
                }
            } else if entry.mode.is_blob() || entry.mode.is_link() {
                if req.filter_blob_none() { continue; }
                if let Some(limit) = blob_limit {
                    // Look up blob size and include only if <= limit
                    if let Ok(obj) = repo.find_object(entry.oid) {
                        if obj.kind == gix::objs::Kind::Blob && obj.data.len() > limit { continue; }
                    }
                }
                blobs.push(entry.oid.into());
            }
        }
    }

    // Include any direct blob wants (lazy fetches)
    for oid in direct_blobs {
        if let Some(limit) = blob_limit {
            if let Ok(obj) = repo.find_object(oid) {
                if obj.kind == gix::objs::Kind::Blob && obj.data.len() > limit { continue; }
            }
        }
        blobs.push(oid);
    }

    Ok(PackPlan {
        commits,
        trees: seen_tree.iter().cloned().collect(),
        blobs,
        shallows: shallows.iter().cloned().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_http::pkt::{decode_pkt_lines, Pkt};

    #[test]
    fn encode_obj_header_small_sizes() {
        // kind=commit(1), size=0 -> 0x10
        assert_eq!(encode_obj_header(1, 0), vec![0x10]);
        // size=15 -> lower 4 bits set, no continuation
        assert_eq!(encode_obj_header(1, 15), vec![0x1f]);
    }

    #[test]
    fn encode_obj_header_multibyte() {
        // size=16 -> continuation in first byte, then 0x01
        assert_eq!(encode_obj_header(1, 16), vec![0x90, 0x01]);
        // size spans multiple varint bytes
        let v = encode_obj_header(3, 0x1fff); // blob, 8191
        // First byte: kind=3 (0x30) | size low 4 bits (0x0f) | cont (0x80) = 0xbf
        assert_eq!(v[0], 0xbf);
        assert!(v.len() >= 2);
    }

    #[test]
    fn sideband_pkt_writer_frames_data_and_progress() {
        let (tx, mut rx) = mpsc::channel::<Bytes>(4);
        let mut w = SidebandPktWriter::new(tx, true, false);
        // Send a small chunk; expect one pkt with band=1 and payload 'abc'
        w.send_chunk(b"abc").unwrap();
        // Progress line should be band=2 and newline-terminated
        w.progress_line("Counting objects".to_string()).unwrap();
        // Drain two messages
        let first = rx.try_recv().expect("first pkt");
        let pkts = decode_pkt_lines(&first).unwrap();
        match &pkts[0] { Pkt::Data(d) => { assert_eq!(d[0], 1); assert_eq!(&d[1..], b"abc"); }, _ => panic!("expected data pkt") }

        let second = rx.try_recv().expect("second pkt");
        let pkts2 = decode_pkt_lines(&second).unwrap();
        match &pkts2[0] { Pkt::Data(d) => { assert_eq!(d[0], 2); assert!(std::str::from_utf8(&d[1..]).unwrap().starts_with("Counting objects")); }, _ => panic!("expected progress pkt") }
    }

    #[test]
    fn sideband_pkt_writer_respects_no_sideband_or_no_progress() {
        // No sideband: raw bytes, not pkt-framed
        let (tx, mut rx) = mpsc::channel::<Bytes>(1);
        let mut w = SidebandPktWriter::new(tx, false, false);
        w.send_chunk(b"PACK").unwrap();
        let raw = rx.try_recv().unwrap();
        assert_eq!(&raw[..], b"PACK");

        // Suppress progress
        let (tx2, mut rx2) = mpsc::channel::<Bytes>(1);
        let mut w2 = SidebandPktWriter::new(tx2, true, true);
        w2.progress_line("message".to_string()).unwrap();
        assert!(rx2.try_recv().is_err());
    }
}

fn build_and_stream_pack_with_plan(
    repo_dir: PathBuf,
    req: &FetchRequest,
    sideband_64k: bool,
    plan: PackPlan,
    tx: mpsc::Sender<Bytes>,
) -> anyhow::Result<()> {
    let repo = gix::open(repo_dir)?;

    let mut out = SidebandPktWriter::new(tx.clone(), sideband_64k, req.no_progress());
    let start = std::time::Instant::now();
    let mut hasher = sha1::Sha1::new();

    // Pack header
    let mut header = Vec::with_capacity(12);
    header.extend_from_slice(b"PACK");
    header.extend_from_slice(&2u32.to_be_bytes());
    header.extend_from_slice(&(
        (plan.commits.len() + plan.trees.len() + plan.blobs.len()) as u32
    ).to_be_bytes());
    hasher.update(&header);
    out.send_chunk(&header)?;

    let mut write_obj = |oid: gix::hash::ObjectId| -> anyhow::Result<()> {
        let obj = repo.find_object(oid)?;
        let (kind, data) = match obj.kind {
            gix::objs::Kind::Commit => (1u8, obj.data.to_vec()),
            gix::objs::Kind::Tree => (2u8, obj.data.to_vec()),
            gix::objs::Kind::Blob => (3u8, obj.data.to_vec()),
            gix::objs::Kind::Tag => (4u8, obj.data.to_vec()),
        };
        let mut hdr = encode_obj_header(kind, data.len() as u64);
        hasher.update(&hdr);
        out.send_chunk(&hdr)?;
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&data)?;
        let compressed = encoder.finish()?;
        hasher.update(&compressed);
        out.send_chunk(&compressed)?;
        Ok(())
    };

    for id in &plan.commits { write_obj(*id)?; }
    for id in &plan.trees { write_obj(*id)?; }
    for id in &plan.blobs { write_obj(*id)?; }

    let trailer = hasher.finalize();
    out.send_chunk(trailer.as_slice())?;

    if sideband_64k && !req.no_progress() {
        let _ = out.progress_line("Done".to_string());
    }

    counter!("git_http.pack.objects").increment((plan.commits.len() + plan.trees.len() + plan.blobs.len()) as u64);
    histogram!("git_http.pack.logical_bytes").record(0.0);
    histogram!("git_http.pack.build_ms").record(start.elapsed().as_millis() as f64);

    tx.blocking_send(Bytes::from_static(PKT_FLUSH)).ok();
    Ok(())
}

// Returns true if an 'ACK <oid> ready' was emitted, false otherwise
async fn emit_acknowledgments(repo_dir: &PathBuf, req: &FetchRequest, tx: &mpsc::Sender<Bytes>) -> anyhow::Result<bool> {
    use gix::prelude::*;
    let repo = gix::open(repo_dir)?;
    // Build a reachable set from wants (commits only)
    let mut reachable: std::collections::HashSet<gix::hash::ObjectId> = std::collections::HashSet::new();
    let mut queue: VecDeque<gix::hash::ObjectId> = VecDeque::new();
    for w in req.wants() {
        if let Ok(oid) = gix::hash::ObjectId::from_hex(w.as_bytes()) { queue.push_back(oid); }
    }
    while let Some(cid) = queue.pop_front() {
        if !reachable.insert(cid) { continue; }
        if let Ok(obj) = repo.find_object(cid) {
            if obj.kind == gix::objs::Kind::Commit {
                let (_, parents) = parse_commit_raw(obj.data.as_ref())?;
                for p in parents { queue.push_back(p); }
            }
        }
    }
    let mut common: Vec<gix::hash::ObjectId> = Vec::new();
    for h in req.haves() {
        if let Ok(oid) = gix::hash::ObjectId::from_hex(h.as_bytes()) {
            if reachable.contains(&oid) {
                common.push(oid);
            }
        }
    }
    if common.is_empty() {
        // No common base found; reply with NAK per v2 semantics
        let _ = tx.send(Bytes::from(encode_pkt_line(b"NAK\n"))).await;
        return Ok(false);
    }
    for c in &common {
        let line = format!("ACK {} common\n", c);
        let _ = tx.send(Bytes::from(encode_pkt_line(line.as_bytes()))).await;
    }
    // Indicate we are ready to send a pack
    let ready_id = common.last().cloned().unwrap();
    let line = format!("ACK {} ready\n", ready_id);
    let _ = tx.send(Bytes::from(encode_pkt_line(line.as_bytes()))).await;
    Ok(true)
}

async fn resolve_want_refs(repo_dir: &PathBuf, req: &mut FetchRequest) -> anyhow::Result<()> {
    use gix::prelude::*;
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
    // Fix: Actually mutate the request's wants vector via public API
    req.extend_wants(new_wants);
    Ok(())
}
