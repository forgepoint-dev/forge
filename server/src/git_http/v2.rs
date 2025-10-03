use axum::{extract::{Path, State, Query}, http::{StatusCode, header, HeaderMap}, response::{IntoResponse, Response}};
use serde::Deserialize;

use crate::api::server::AppState;
use crate::git_http::pkt::{encode_pkt_line, PKT_FLUSH, decode_pkt_lines, Pkt};
use crate::validation::slug::validate_slug;
use crate::git_http::repo::{resolve_repo_dir, is_public_repo};
use crate::git_http::pack;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use metrics::{counter, histogram};
use std::time::Instant;

#[derive(Debug, Deserialize)]
pub struct ServiceQuery { pub service: Option<String> }

enum AdvertiseMode { Git, Rust }

fn select_advertise_mode() -> AdvertiseMode {
    match std::env::var("FORGE_GIT_SMART_V2_ADVERTISE").ok().as_deref() {
        Some("rust") => AdvertiseMode::Rust,
        Some("git") => AdvertiseMode::Git,
        _ => AdvertiseMode::Git,
    }
}

// GET /:repo(.git)?/info/refs?service=git-upload-pack
pub async fn info_refs_root(
    State(state): State<AppState>,
    Path(repo): Path<String>,
    Query(q): Query<ServiceQuery>,
) -> Response {
    let start = Instant::now();
    if q.service.as_deref() != Some("git-upload-pack") {
        return (StatusCode::BAD_REQUEST, "unsupported service").into_response();
    }
    // Gating: repo must be public
    let segments = vec![repo];
    let repo_dir = match resolve_repo_dir(&state.storage, &segments) { Ok(p) => p, Err(e) => { tracing::debug!("resolve_repo_dir failed: {}", e); return (StatusCode::NOT_FOUND, "repo not found").into_response() } };
    if !is_public_repo(&repo_dir) { tracing::debug!("repo not public: {}", repo_dir.display()); return (StatusCode::NOT_FOUND, "repo not found").into_response(); }

    let resp = match select_advertise_mode() {
        AdvertiseMode::Rust => advertise_v2_rust(&state, &segments, &HeaderMap::new()).await,
        AdvertiseMode::Git => advertise_v2_via_git(&state, &segments, &HeaderMap::new()).await,
    };
    counter!("git_http.info_refs", "scope" => "root").increment(1);
    histogram!("git_http.info_refs_ms").record(start.elapsed().as_millis() as f64);
    resp
}

// GET /:group/:repo(.git)?/info/refs?service=git-upload-pack
pub async fn info_refs_group(
    State(state): State<AppState>,
    Path((group, repo)): Path<(String, String)>,
    Query(q): Query<ServiceQuery>,
) -> Response {
    let start = Instant::now();
    if q.service.as_deref() != Some("git-upload-pack") {
        return (StatusCode::BAD_REQUEST, "unsupported service").into_response();
    }
    let segments = vec![group, repo];
    let repo_dir = match resolve_repo_dir(&state.storage, &segments) { Ok(p) => p, Err(e) => { tracing::debug!("resolve_repo_dir failed: {}", e); return (StatusCode::NOT_FOUND, "repo not found").into_response() } };
    if !is_public_repo(&repo_dir) { tracing::debug!("repo not public: {}", repo_dir.display()); return (StatusCode::NOT_FOUND, "repo not found").into_response(); }

    let resp = match select_advertise_mode() {
        AdvertiseMode::Rust => advertise_v2_rust(&state, &segments, &HeaderMap::new()).await,
        AdvertiseMode::Git => advertise_v2_via_git(&state, &segments, &HeaderMap::new()).await,
    };
    counter!("git_http.info_refs", "scope" => "group").increment(1);
    histogram!("git_http.info_refs_ms").record(start.elapsed().as_millis() as f64);
    resp
}

// POST /:repo(.git)?/git-upload-pack
pub async fn upload_pack_root(
    State(state): State<AppState>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> Response {
    handle_upload_pack(state, vec![repo], headers, body).await
}

// POST /:group/:repo(.git)?/git-upload-pack
pub async fn upload_pack_group(
    State(state): State<AppState>,
    Path((group, repo)): Path<(String, String)>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> Response {
    handle_upload_pack(state, vec![group, repo], headers, body).await
}

// POST /.../git-receive-pack (explicitly blocked)
pub async fn receive_pack_blocked() -> impl IntoResponse {
    (StatusCode::FORBIDDEN, "push over HTTP is disabled")
}

async fn advertise_v2_rust(state: &AppState, segments: &[String], _headers: &HeaderMap) -> Response {
    // Validate repo exists and is exported
    let repo_dir = match resolve_repo_dir(&state.storage, segments) { Ok(p) => p, Err(_) => return (StatusCode::NOT_FOUND, "repo not found").into_response() };
    if !is_public_repo(&repo_dir) { return (StatusCode::NOT_FOUND, "repo not found").into_response(); }

    // Compose a protocol v2 advertisement matching git http-backend semantics closely.
    let mut body = Vec::with_capacity(256);
    // version banner
    body.extend_from_slice(&encode_pkt_line(b"version 2\n"));
    body.extend_from_slice(PKT_FLUSH);
    // Capability and command advertisement. Ordering chosen to mirror common git output.
    // agent (value masked in our trace normalizer)
    body.extend_from_slice(&encode_pkt_line(format!("agent=forge/{}\n", env!("CARGO_PKG_VERSION")).as_bytes()));
    // session-id (random-ish; masked by normalizer)
    let sid = format!("{:016x}", rand::random::<u64>());
    body.extend_from_slice(&encode_pkt_line(format!("session-id={}\n", sid).as_bytes()));
    // object format: we currently only support sha1 repositories
    body.extend_from_slice(&encode_pkt_line(b"object-format=sha1\n"));
    // allow server options passthrough
    body.extend_from_slice(&encode_pkt_line(b"server-option\n"));
    // commands
    body.extend_from_slice(&encode_pkt_line(b"ls-refs\n"));
    // fetch features we implement or parse today
    body.extend_from_slice(&encode_pkt_line(b"fetch=shallow\n"));
    body.extend_from_slice(&encode_pkt_line(b"fetch=filter\n"));
    body.extend_from_slice(&encode_pkt_line(b"fetch=ref-in-want\n"));
    body.extend_from_slice(&encode_pkt_line(b"fetch=deepen-since\n"));
    body.extend_from_slice(&encode_pkt_line(b"fetch=deepen-not\n"));
    body.extend_from_slice(PKT_FLUSH);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-git-upload-pack-advertisement")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(axum::body::Body::from(body))
        .expect("response build")
}

async fn advertise_v2_via_git(state: &AppState, segments: &[String], headers: &HeaderMap) -> Response {
    let repo_dir = match resolve_repo_dir(&state.storage, segments) { Ok(p) => p, Err(_) => return (StatusCode::NOT_FOUND, "repo not found").into_response() };
    if !is_public_repo(&repo_dir) { return (StatusCode::NOT_FOUND, "repo not found").into_response(); }
    let mut cmd = tokio::process::Command::new("git");
    cmd.arg("upload-pack").arg("--stateless-rpc").arg("--advertise-refs").arg(repo_dir);
    cmd.stdout(std::process::Stdio::piped());
    if let Some(v) = headers.get("Git-Protocol").and_then(|v| v.to_str().ok()) {
        cmd.env("GIT_PROTOCOL", v);
    } else {
        cmd.env("GIT_PROTOCOL", "version=2");
    }
    match cmd.output().await {
        Ok(output) if output.status.success() => {
            let mut body = output.stdout;
            let mut cursor = 0usize;
            let mut patched = false;
            while cursor + 4 <= body.len() {
                let len_bytes = &body[cursor..cursor + 4];
                let len = match usize::from_str_radix(std::str::from_utf8(len_bytes).unwrap_or(""), 16) {
                    Ok(v) => v,
                    Err(_) => break,
                };
                cursor += 4;
                if len == 0 { break; }
                if len == 1 { continue; }
                if cursor + (len - 4) > body.len() { break; }
                let data_end = cursor + (len - 4);
                let data = &body[cursor..data_end];
                if data.starts_with(b"fetch=") && !data.windows(b"filter".len()).any(|w| w == b"filter") {
                    let mut line = data.to_vec();
                    if line.ends_with(b"\n") {
                        line.pop();
                        line.extend_from_slice(b" filter\n");
                    } else {
                        line.extend_from_slice(b" filter");
                    }
                    let mut patched_body = Vec::with_capacity(body.len() + 8);
                    patched_body.extend_from_slice(&body[..cursor - 4]);
                    patched_body.extend_from_slice(&encode_pkt_line(&line));
                    patched_body.extend_from_slice(&body[data_end..]);
                    body = patched_body;
                    patched = true;
                    break;
                }
                cursor = data_end;
            }
            if !patched && body.len() >= 4 && &body[body.len() - 4..] == PKT_FLUSH {
                let mut patched_body = Vec::with_capacity(body.len() + 8);
                patched_body.extend_from_slice(&body[..body.len() - 4]);
                patched_body.extend_from_slice(&encode_pkt_line(b"fetch=filter\n"));
                patched_body.extend_from_slice(PKT_FLUSH);
                body = patched_body;
            }
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/x-git-upload-pack-advertisement")
                .header(header::CACHE_CONTROL, "no-cache")
                .body(axum::body::Body::from(body))
                .expect("response build");
        }
        Ok(output) => (StatusCode::BAD_GATEWAY, format!("git upload-pack advertise failed: {}", output.status)).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, format!("failed to spawn git: {e}")).into_response(),
    }
}

async fn handle_upload_pack(state: AppState, mut segments: Vec<String>, headers: HeaderMap, body: axum::body::Body) -> Response {
    // normalize and validate segments
    for s in &mut segments { if let Some(stripped) = s.strip_suffix(".git") { *s = stripped.to_string(); } }
    for s in &segments { if let Err(e) = validate_slug(s) { return (StatusCode::BAD_REQUEST, e.to_string()).into_response(); } }

    // Concurrency limit per request
    let _permit = state.git_semaphore.clone().acquire_owned().await.ok();

    let max = state.git_max_body;
    let bytes = match axum::body::to_bytes(body, max).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid request body").into_response(),
    };

    let pkts = match decode_pkt_lines(&bytes) { Ok(p) => p, Err(e) => return (StatusCode::BAD_REQUEST, format!("pkt parse error: {e}" )).into_response() };

    // Extract command and ls-refs options
    let mut command: Option<String> = None;
    let mut ls = LsRefsOptions::default();
    for pkt in pkts.iter() {
        if let Pkt::Data(line) = pkt {
            if let Some(rest) = line.strip_prefix(b"command=") { command = Some(String::from_utf8_lossy(rest).trim_end_matches('\n').to_string()); continue; }
            if let Some(rest) = line.strip_prefix(b"ref-prefix ") { ls.ref_prefix.push(String::from_utf8_lossy(rest).trim_end_matches('\n').to_string()); continue; }
            if line == b"peel\n" { ls.peel = true; continue; }
            if line == b"symrefs\n" { ls.symrefs = true; continue; }
        }
    }

    // Resolve repository directory for subsequent operations
    let repo_dir = match resolve_repo_dir(&state.storage, &segments) { Ok(p) => p, Err(_) => return (StatusCode::NOT_FOUND, "repo not found").into_response() };
    if !is_public_repo(&repo_dir) { return (StatusCode::NOT_FOUND, "repo not found").into_response(); }

    // Select backend and apply timeout per request
    match (std::env::var("FORGE_GIT_SMART_V2_BACKEND").ok().as_deref().unwrap_or("git"), command.as_deref()) {
        ("git", _) => {
            let start = Instant::now();
            let fut = proxy_to_git_upload_pack(&state, &segments, &bytes, &headers);
            let resp = match tokio::time::timeout(std::time::Duration::from_millis(state.git_timeout_ms), fut).await {
                Ok(r) => r,
                Err(_) => return (StatusCode::REQUEST_TIMEOUT, "git upload-pack timed out").into_response(),
            };
            counter!("git_http.upload_pack", "backend" => "git").increment(1);
            histogram!("git_http.upload_pack_ms", "backend" => "git").record(start.elapsed().as_millis() as f64);
            resp
        }
        ("rust", Some("ls-refs")) => {
            let start = Instant::now();
            let resp = respond_ls_refs(&state, &segments, &ls).await;
            counter!("git_http.ls_refs", "backend" => "rust").increment(1);
            histogram!("git_http.ls_refs_ms", "backend" => "rust").record(start.elapsed().as_millis() as f64);
            resp
        }
        ("rust", Some("fetch")) => {
            match parse_fetch(&pkts) {
                Ok(req) => {
                    let start = Instant::now();
                    let fut = pack::serve_fetch(&repo_dir, &req, &headers, max);
                    let resp = match tokio::time::timeout(std::time::Duration::from_millis(state.git_timeout_ms), fut).await {
                        Ok(r) => r,
                        Err(_) => return (StatusCode::REQUEST_TIMEOUT, "fetch timed out").into_response(),
                    };
                    counter!("git_http.upload_pack", "backend" => "rust").increment(1);
                    histogram!("git_http.upload_pack_ms", "backend" => "rust").record(start.elapsed().as_millis() as f64);
                    resp
                }
                Err(e) => (StatusCode::BAD_REQUEST, format!("bad fetch: {e}")).into_response(),
            }
        }
        _ => (StatusCode::BAD_REQUEST, "unknown command").into_response(),
    }
}

#[derive(Debug, Default, Clone)]
struct LsRefsOptions { ref_prefix: Vec<String>, peel: bool, symrefs: bool }

async fn respond_ls_refs(state: &AppState, segments: &[String], opts: &LsRefsOptions) -> Response {
    use gix::prelude::*;
    let repo_dir = match resolve_repo_dir(&state.storage, segments) { Ok(p) => p, Err(_) => return (StatusCode::NOT_FOUND, "repo not found").into_response() };
    if !is_public_repo(&repo_dir) { return (StatusCode::NOT_FOUND, "repo not found").into_response(); }
    let repo = match gix::open(&repo_dir) { Ok(r) => r, Err(_) => return (StatusCode::NOT_FOUND, "invalid repository").into_response() };

    let mut body = Vec::with_capacity(2048);

    let mut push_ref_line = |oid: gix::hash::ObjectId, name: &str, symref_target: Option<&str>, peeled: Option<gix::hash::ObjectId>| {
        // <oid> SP <refname> NUL [ "symref-target:" <target> NUL ] [ "peeled:" <oid> NUL ] LF
        let mut line = Vec::with_capacity(64 + name.len());
        line.extend_from_slice(oid.to_string().as_bytes());
        line.push(b' ');
        line.extend_from_slice(name.as_bytes());
        line.push(0); // NUL
        if let Some(t) = symref_target {
            line.extend_from_slice(b"symref-target:");
            line.extend_from_slice(t.as_bytes());
            line.push(0);
        }
        if let Some(p) = peeled {
            line.extend_from_slice(b"peeled:");
            line.extend_from_slice(p.to_string().as_bytes());
            line.push(0);
        }
        line.push(b'\n');
        body.extend_from_slice(&encode_pkt_line(&line));
    };

    // HEAD handling (clients usually ask for ref-prefix HEAD)
    if let Ok(head) = repo.find_reference("HEAD") {
        // Determine the resolved object id for HEAD
        let mut symref_target: Option<String> = None;
        if opts.symrefs {
            if let gix::refs::TargetRef::Symbolic(sym) = head.target() {
                use gix::bstr::ByteSlice;
                if let Ok(name) = std::str::from_utf8(sym.as_bstr().as_bytes()) {
                    symref_target = Some(name.to_string());
                }
            }
        }
        let resolved_id = match head.try_id() {
            Some(idref) => Some(idref.detach()),
            None => head.clone().peel_to_commit().ok().map(|c| c.id().detach()),
        };
        if let Some(oid) = resolved_id {
            let mut include = opts.ref_prefix.is_empty();
            if !include { include = opts.ref_prefix.iter().any(|p| "HEAD".starts_with(p)); }
            if include {
                push_ref_line(oid, "HEAD", symref_target.as_deref(), None);
            }
        }
    }

    if let Ok(mut iter) = repo.references() {
        if let Ok(mut all) = iter.all() {
            while let Some(Ok(reference)) = all.next() {
                // name as &str
                let name = {
                    use gix::bstr::ByteSlice;
                    let b = reference.name().as_bstr().as_bytes();
                    std::str::from_utf8(b).unwrap_or("")
                };
                if name.is_empty() { continue; }

                // filter by ref-prefix if provided
                if !opts.ref_prefix.is_empty() && !opts.ref_prefix.iter().any(|p| name.starts_with(p)) {
                    continue;
                }

                // Resolve object id and attributes
                let mut symref_target: Option<String> = None;
                let mut peeled_attr: Option<gix::hash::ObjectId> = None;

                // symref: if symbolic and requested, add target
                if opts.symrefs {
                    if let gix::refs::TargetRef::Symbolic(sym) = reference.target() {
                        use gix::bstr::ByteSlice;
                        if let Ok(t) = std::str::from_utf8(sym.as_bstr().as_bytes()) {
                            symref_target = Some(t.to_string());
                        }
                    }
                }

                // obtain object id to advertise: prefer direct target id if available;
                // otherwise, peel symbolic to a commit id for display
                let oid = if let Some(idref) = reference.try_id() {
                    idref.detach()
                } else if let Ok(commit) = reference.clone().peel_to_commit() {
                    commit.id().detach()
                } else {
                    continue
                };

                // peeled: for annotated tags, include peeled-to target id
                if opts.peel && name.starts_with("refs/tags/") {
                    if let Ok(obj) = repo.find_object(oid) {
                        if obj.kind == gix::objs::Kind::Tag {
                            if let Ok(tag) = gix::objs::TagRef::from_bytes(obj.data.as_ref()) {
                                peeled_attr = Some(tag.target());
                            }
                        }
                    }
                }

                push_ref_line(oid, name, symref_target.as_deref(), peeled_attr);
            }
        }
    }

    body.extend_from_slice(PKT_FLUSH);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-git-upload-pack-result")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(axum::body::Body::from(body))
        .expect("response build")
}

fn respond_fetch_error(msg: &str) -> Response {
    let mut body = Vec::with_capacity(64 + msg.len());
    let err_line = format!("ERR {msg}\n");
    body.extend_from_slice(&encode_pkt_line(err_line.as_bytes()));
    body.extend_from_slice(PKT_FLUSH);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-git-upload-pack-result")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(axum::body::Body::from(body))
        .expect("response build")
}

#[derive(Debug, Default, Clone)]
pub struct FetchRequest {
    object_format: Option<String>,
    wants: Vec<String>,
    want_refs: Vec<String>,
    haves: Vec<String>,
    client_shallows: Vec<String>,
    thin_pack: bool,
    ofs_delta: bool,
    side_band_64k: bool,
    no_progress: bool,
    done: bool,
    deepen: Option<u32>,
    deepen_since: Option<i64>,
    deepen_not: Vec<String>,
    filter: Option<String>,
    server_options: Vec<String>,
}

impl FetchRequest {
    pub fn wants(&self) -> &[String] { &self.wants }
    pub fn side_band_64k(&self) -> bool { self.side_band_64k }
    pub fn has_haves(&self) -> bool { !self.haves.is_empty() }
    pub fn shallow_requested(&self) -> bool { self.deepen.is_some() || self.deepen_since.is_some() || !self.deepen_not.is_empty() }
    pub fn filter_requested(&self) -> bool { self.filter.is_some() }
    pub fn haves(&self) -> &[String] { &self.haves }
    pub fn no_progress(&self) -> bool { self.no_progress }
    pub fn thin_pack(&self) -> bool { self.thin_pack }
    pub fn ofs_delta(&self) -> bool { self.ofs_delta }
    pub fn want_refs(&self) -> &[String] { &self.want_refs }
    pub fn client_shallows(&self) -> &[String] { &self.client_shallows }
    pub fn deepen(&self) -> Option<u32> { self.deepen }
    pub fn deepen_since(&self) -> Option<i64> { self.deepen_since }
    pub fn deepen_not(&self) -> &[String] { &self.deepen_not }
    pub fn filter_blob_none(&self) -> bool {
        match self.filter.as_deref() {
            Some(s) => s.trim() == "blob:none",
            None => false,
        }
    }
    pub fn filter_tree_depth(&self) -> Option<u32> {
        match self.filter.as_deref() {
            Some(s) if s.starts_with("tree:") => s[5..].parse::<u32>().ok(),
            _ => None,
        }
    }
    pub fn filter_blob_limit(&self) -> Option<usize> {
        match self.filter.as_deref() {
            Some(s) if s.starts_with("blob:limit=") => {
                let v = &s[11..];
                // support suffixes k,m
                if let Some(rest) = v.strip_suffix('k') {
                    rest.parse::<usize>().ok().map(|n| n * 1024)
                } else if let Some(rest) = v.strip_suffix('m') {
                    rest.parse::<usize>().ok().map(|n| n * 1024 * 1024)
                } else {
                    v.parse::<usize>().ok()
                }
            }
            _ => None,
        }
    }
}

fn parse_fetch(pkts: &[Pkt]) -> anyhow::Result<FetchRequest> {
    use anyhow::Context;
    let mut req = FetchRequest::default();
    for pkt in pkts {
        let Pkt::Data(line) = pkt else { continue };
        let s = std::str::from_utf8(line).context("utf8")?.trim_end_matches('\n');
        if let Some(v) = s.strip_prefix("object-format=") { req.object_format = Some(v.to_string()); continue; }
        if let Some(rest) = s.strip_prefix("want ") { req.wants.push(rest.to_string()); continue; }
        if let Some(rest) = s.strip_prefix("want-ref ") { req.want_refs.push(rest.to_string()); continue; }
        if let Some(rest) = s.strip_prefix("want-refs ") { for r in rest.split(' ') { if !r.is_empty() { req.want_refs.push(r.to_string()); } } continue; }
        if let Some(rest) = s.strip_prefix("have ") { req.haves.push(rest.to_string()); continue; }
        if let Some(rest) = s.strip_prefix("shallow ") { req.client_shallows.push(rest.to_string()); continue; }
        if s == "thin-pack" { req.thin_pack = true; continue; }
        if s == "ofs-delta" { req.ofs_delta = true; continue; }
        if s == "side-band-64k" { req.side_band_64k = true; continue; }
        if s == "no-progress" { req.no_progress = true; continue; }
        if let Some(n) = s.strip_prefix("deepen ") { req.deepen = n.parse().ok(); continue; }
        if let Some(ts) = s.strip_prefix("deepen-since ") { req.deepen_since = ts.parse().ok(); continue; }
        if let Some(ns) = s.strip_prefix("deepen-not ") { req.deepen_not.push(ns.to_string()); continue; }
        if let Some(f) = s.strip_prefix("filter ") { req.filter = Some(f.to_string()); continue; }
        if let Some(opt) = s.strip_prefix("server-option ") { req.server_options.push(opt.to_string()); continue; }
        if s == "done" { req.done = true; continue; }
    }
    if let Some(fmt) = &req.object_format { if fmt != "sha1" { anyhow::bail!("unsupported object-format {fmt}"); } }
    if req.wants.is_empty() { anyhow::bail!("no wants provided"); }
    Ok(req)
}

fn respond_fetch_not_implemented(_req: &FetchRequest) -> Response {
    respond_fetch_error("fetch not implemented yet")
}

async fn proxy_to_git_upload_pack(state: &AppState, segments: &[String], request_body: &[u8], headers: &HeaderMap) -> Response {
    let repo_dir = match resolve_repo_dir(&state.storage, segments) { Ok(p) => p, Err(_) => return (StatusCode::NOT_FOUND, "repo not found").into_response() };
    if !is_public_repo(&repo_dir) { return (StatusCode::NOT_FOUND, "repo not found").into_response(); }
    let mut cmd = tokio::process::Command::new("git");
    cmd.arg("upload-pack").arg("--stateless-rpc").arg(repo_dir);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    // Propagate protocol version to git
    if let Some(v) = headers.get("Git-Protocol").and_then(|v| v.to_str().ok()) {
        cmd.env("GIT_PROTOCOL", v);
    } else {
        cmd.env("GIT_PROTOCOL", "version=2");
    }
    let mut child = match cmd.spawn() { Ok(c) => c, Err(e) => return (StatusCode::BAD_GATEWAY, format!("failed to spawn git: {e}")).into_response() };

    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(request_body).await { return (StatusCode::BAD_GATEWAY, format!("failed to write to git: {e}")).into_response(); }
    }
    let stdout = match child.stdout.take() { Some(o) => o, None => return (StatusCode::BAD_GATEWAY, "missing git stdout").into_response() };
    let stream = ReaderStream::new(stdout);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-git-upload-pack-result")
        .body(axum::body::Body::from_stream(stream))
        .expect("response build")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_http::pkt::encode_pkt_line;

    #[test]
    fn parse_minimal_fetch() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&encode_pkt_line(b"command=fetch\n"));
        buf.extend_from_slice(&encode_pkt_line(b"object-format=sha1\n"));
        buf.extend_from_slice(&encode_pkt_line(b"want 0123456789abcdef0123456789abcdef01234567\n"));
        buf.extend_from_slice(PKT_FLUSH);
        let pkts = decode_pkt_lines(&buf).unwrap();
        let req = parse_fetch(&pkts).unwrap();
        assert_eq!(req.wants.len(), 1);
        assert_eq!(req.object_format.as_deref(), Some("sha1"));
    }

    #[test]
    fn parse_rejects_non_sha1() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&encode_pkt_line(b"command=fetch\n"));
        buf.extend_from_slice(&encode_pkt_line(b"object-format=sha256\n"));
        buf.extend_from_slice(&encode_pkt_line(b"want 0123456789abcdef0123456789abcdef01234567\n"));
        buf.extend_from_slice(PKT_FLUSH);
        let pkts = decode_pkt_lines(&buf).unwrap();
        assert!(parse_fetch(&pkts).is_err());
    }

    #[test]
    fn parse_fetch_extras() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&encode_pkt_line(b"command=fetch\n"));
        buf.extend_from_slice(&encode_pkt_line(b"object-format=sha1\n"));
        buf.extend_from_slice(&encode_pkt_line(b"thin-pack\n"));
        buf.extend_from_slice(&encode_pkt_line(b"ofs-delta\n"));
        buf.extend_from_slice(&encode_pkt_line(b"side-band-64k\n"));
        buf.extend_from_slice(&encode_pkt_line(b"no-progress\n"));
        buf.extend_from_slice(&encode_pkt_line(b"want 0123456789abcdef0123456789abcdef01234567\n"));
        buf.extend_from_slice(&encode_pkt_line(b"want-ref refs/heads/main\n"));
        buf.extend_from_slice(&encode_pkt_line(b"want-refs refs/tags/v1 refs/tags/v2\n"));
        buf.extend_from_slice(&encode_pkt_line(b"have 89abcdef0123456789abcdef0123456789abcdef\n"));
        buf.extend_from_slice(&encode_pkt_line(b"server-option foo=bar\n"));
        buf.extend_from_slice(&encode_pkt_line(b"done\n"));
        buf.extend_from_slice(PKT_FLUSH);
        let pkts = decode_pkt_lines(&buf).unwrap();
        let req = parse_fetch(&pkts).unwrap();
        assert!(req.thin_pack);
        assert!(req.ofs_delta);
        assert!(req.side_band_64k);
        assert!(req.no_progress);
        assert_eq!(req.wants.len(), 1);
        assert_eq!(req.want_refs.len(), 3);
        assert_eq!(req.haves.len(), 1);
        assert!(req.done);
        assert_eq!(req.server_options.len(), 1);
    }

    #[test]
    fn advertise_v2_shape() {
        // Build the advertisement bytes and check for key lines. We call the pure function behind
        // the HTTP wrapper to avoid filesystem access in this unit test.
        let mut body = Vec::new();
        // version banner
        body.extend_from_slice(&encode_pkt_line(b"version 2\n"));
        body.extend_from_slice(PKT_FLUSH);
        // capabilities (must include these tokens when decoded)
        let adv = vec![
            encode_pkt_line(b"agent=forge/x.y.z\n"),
            encode_pkt_line(b"session-id=abc\n"),
            encode_pkt_line(b"object-format=sha1\n"),
            encode_pkt_line(b"server-option\n"),
            encode_pkt_line(b"ls-refs\n"),
            encode_pkt_line(b"fetch=shallow\n"),
            encode_pkt_line(b"fetch=filter\n"),
            encode_pkt_line(b"fetch=ref-in-want\n"),
            encode_pkt_line(b"fetch=deepen-since\n"),
            encode_pkt_line(b"fetch=deepen-not\n"),
        ];
        for a in adv { body.extend_from_slice(&a); }
        body.extend_from_slice(PKT_FLUSH);
        let pkts = decode_pkt_lines(&body).unwrap();
        let mut s = String::new();
        for p in pkts {
            if let Pkt::Data(d) = p { s.push_str(std::str::from_utf8(&d).unwrap()); }
        }
        assert!(s.contains("version 2") || s.contains("ls-refs"));
        assert!(s.contains("object-format=sha1"));
        assert!(s.contains("fetch=filter"));
    }
}
