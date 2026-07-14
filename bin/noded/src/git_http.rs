//! git smart-HTTP: forge as a full push+fetch remote over `/forge/{repo}/…`.

use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures::channel::oneshot;
use serde::Deserialize;

use crate::{DEFAULT_ORIGIN, NodeCommand, NodeHandle, actor_gone, error_response};

// ============================================================================
// git smart-HTTP: forge is a full push+fetch remote.
//
// this is the ONE forge-specific corner of the surface (every other route is
// module-agnostic opaque json). it speaks the git smart-HTTP protocol on both
// sides so a stock `git` clones, pulls, and pushes `http://<node>/forge/<repo>`:
//   GET  /forge/{repo}/info/refs?service=git-receive-pack — advertise for push
//   GET  /forge/{repo}/info/refs?service=git-upload-pack  — advertise for fetch
//   POST /forge/{repo}/git-receive-pack                   — receive a push
//   POST /forge/{repo}/git-upload-pack                    — serve a fetch/clone
//
// PUSH bridges to forge's consensus `Push` op: the packfile bytes land in the
// node-local blob store (never consensus); only the (prev_oid, new_oid,
// pack_digest) CAS crosses into a block, and forge's in-module `materialize`
// verifies the pack against the repo's objects.
//
// FETCH reads forge's git substrate DIRECTLY — the one route that opens the
// on-disk repo (`<forge_repo>/<name>`, threaded onto the handle) instead of
// talking to the actor. once the client sends `done`, the haves it advertised
// bound the pack: every have this repo knows hides its closure from the walk,
// so an up-to-date-ish client (the remote-view mirror re-syncing per head
// movement) downloads only what moved, ACKed with the common base. a client
// with no usable common base still gets the FULL self-contained closure after
// a NAK. intermediate flush-ended rounds answer plain NAK, so stock git keeps
// batching haves until it sends `done`.
// ============================================================================

/// the capabilities forge's receive-pack advertises. deliberately NO
/// `side-band-64k`, so the client sends the report-status back as plain
/// pkt-lines (not muxed onto a side channel) — the minimal wire this bridge
/// needs to read.
const GIT_RECEIVE_PACK_CAPS: &str =
    "report-status report-status-v2 delete-refs ofs-delta agent=ducktape-forge/0.1";
/// the capabilities forge's upload-pack (fetch/clone) advertises. `side-band-64k`
/// muxes the packfile onto band 1 of the reply — git clients request it by
/// default; `multi_ack_detailed` is the modern negotiation, `thin-pack`/
/// `ofs-delta` are standard pack encodings. no fetch-side extras (shallow /
/// filter): the answer is either the full closure or a have-bounded delta.
const GIT_UPLOAD_PACK_CAPS: &str =
    "multi_ack_detailed side-band-64k thin-pack ofs-delta agent=ducktape-forge/0.1";
/// the body cap for a git packfile POST — push (whole-repo pack) and fetch
/// (want/have negotiation). lifted far above the json/chunk defaults.
pub(crate) const GIT_PACK_BODY_LIMIT: usize = 512 * 1024 * 1024;
/// max PACK bytes per side-band-64k data pkt-line: prefixed with the 1-byte band
/// id, plus the 4-byte pkt length header, this yields a 65520-byte line — git's
/// `LARGE_PACKET_MAX`, the ceiling a side-band-64k client accepts.
const GIT_SIDE_BAND_CHUNK: usize = 65515;
/// the ref namespace pushes may touch: any branch. a command outside
/// `refs/heads/*` (tags, notes) is refused with a per-ref `ng`.
const GIT_HEADS_PREFIX: &str = "refs/heads/";
/// 40 ascii zeros: git's "null" oid — the old value of a ref being created, and
/// the head advertised for an unborn repo.
const GIT_ZERO_OID: &str = "0000000000000000000000000000000000000000";
/// raw sha1 oid length in bytes. git's wire oids are 40 hex chars == 20 bytes;
/// forge's `Push` op wants exactly these raw bytes (it re-length-checks too).
const GIT_OID_RAW_LEN: usize = 20;
/// the flush-pkt: a zero-length pkt that ends a pkt-line stream or section.
const GIT_FLUSH_PKT: &[u8] = b"0000";

/// encode one git pkt-line: a 4-hex length (INCLUDING the 4 length bytes)
/// followed by the payload. every line this bridge emits is tiny, well under
/// the 65516-byte payload cap, so no splitting is needed.
fn pkt_line(payload: &[u8]) -> Vec<u8> {
    let len = payload.len() + 4;
    let mut out = format!("{len:04x}").into_bytes();
    out.extend_from_slice(payload);
    out
}

/// split a leading pkt-line section off `buf`: parse length-framed lines until a
/// flush-pkt (`0000`), returning each payload (WITHOUT its 4-byte length header)
/// and the bytes AFTER the flush (for receive-pack, the raw packfile). a
/// truncated or malformed length is a clean error, never a panic — a corrupt
/// body becomes a 400.
fn parse_pkt_lines(buf: &[u8]) -> Result<(Vec<Vec<u8>>, &[u8]), String> {
    let mut lines = Vec::new();
    let mut rest = buf;
    loop {
        if rest.len() < 4 {
            return Err("truncated pkt-line length header".into());
        }
        let hdr =
            std::str::from_utf8(&rest[..4]).map_err(|_| "non-ascii pkt-line length".to_string())?;
        let len = usize::from_str_radix(hdr, 16)
            .map_err(|_| "invalid pkt-line length hex".to_string())?;
        if len == 0 {
            // flush-pkt terminates the command section; the rest is the pack.
            return Ok((lines, &rest[4..]));
        }
        if len < 4 || len > rest.len() {
            return Err("pkt-line length out of range".into());
        }
        lines.push(rest[4..len].to_vec());
        rest = &rest[len..];
    }
}

/// the parts of a v0 upload-pack request this server needs. haves bound the
/// pack: every have the repo knows hides its closure from the walk, so a
/// remote client refreshing its mirror downloads only what moved — the
/// remote-view lane syncs per head movement, and a full-closure answer there
/// would re-ship the whole repo every time.
struct UploadPackRequest {
    wants: Vec<String>,
    haves: Vec<String>,
    side_band: bool,
    done: bool,
}

/// parse the complete v0 upload-pack request, including the negotiation tail.
/// A stateless smart-HTTP client may end a round with a flush instead of `done`;
/// that round must receive only NAK so it can send another batch of haves.
fn parse_upload_pack_request(body: &[u8]) -> Result<UploadPackRequest, String> {
    let (lines, mut rest) = parse_pkt_lines(body)?;
    let mut wants = Vec::new();
    let mut side_band = false;
    let mut first_want = true;
    for line in &lines {
        let text = std::str::from_utf8(line)
            .map_err(|_| "non-utf8 want line".to_string())?
            .trim_end();
        let Some(want) = text.strip_prefix("want ") else {
            return Err("unexpected line in want section".into());
        };
        let mut toks = want.split(' ');
        let oid = toks
            .next()
            .filter(|oid| !oid.is_empty())
            .ok_or_else(|| "want line carried no oid".to_string())?;
        if git2::Oid::from_str(oid).is_err() {
            return Err("want line carried an invalid oid".into());
        }
        wants.push(oid.to_string());
        if first_want {
            side_band = toks.any(|cap| cap == "side-band-64k");
            first_want = false;
        }
    }
    if wants.is_empty() {
        return Err("request carried no want lines".into());
    }

    let mut haves = Vec::new();
    let mut done = false;
    while !rest.is_empty() {
        if done {
            return Err("upload-pack negotiation continued after done".into());
        }
        if rest.len() < 4 {
            return Err("truncated negotiation pkt-line length header".into());
        }
        let hdr = std::str::from_utf8(&rest[..4])
            .map_err(|_| "non-ascii negotiation pkt-line length".to_string())?;
        let len = usize::from_str_radix(hdr, 16)
            .map_err(|_| "invalid negotiation pkt-line length hex".to_string())?;
        if len == 0 {
            rest = &rest[4..];
            continue;
        }
        if len < 4 || len > rest.len() {
            return Err("negotiation pkt-line length out of range".into());
        }
        let text = std::str::from_utf8(&rest[4..len])
            .map_err(|_| "non-utf8 negotiation line".to_string())?
            .trim_end();
        if text == "done" {
            done = true;
        } else if let Some(oid) = text.strip_prefix("have ") {
            if git2::Oid::from_str(oid).is_err() {
                return Err("have line carried an invalid oid".into());
            }
            haves.push(oid.to_string());
        } else {
            return Err("unexpected upload-pack negotiation line".into());
        }
        rest = &rest[len..];
    }

    Ok(UploadPackRequest {
        wants,
        haves,
        side_band,
        done,
    })
}

/// decode an even-length hex string to raw bytes; `None` on an odd length or any
/// non-hex nibble. turns a git pkt-line oid (40 hex) into raw sha1 bytes.
fn hex_to_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

/// query the forge module for a repo's committed branches (`[]` == unborn).
/// errors surface as an http `Response` so callers can early-return them.
async fn forge_refs(handle: &NodeHandle, repo: &str) -> Result<Vec<forge::RefHead>, Response> {
    let req = forge::encode_query(&forge::ForgeQuery::ListRefs {
        repo: repo.to_string(),
    });
    let (reply, rx) = oneshot::channel();
    handle
        .send(NodeCommand::Query {
            target: "forge".into(),
            req,
            reply,
        })
        .await?;
    let bytes = rx
        .await
        .map_err(|_| actor_gone())?
        .map_err(|err| error_response(StatusCode::INTERNAL_SERVER_ERROR, &err))?;
    match forge::decode_reply(&bytes) {
        Ok(forge::ForgeReply::Refs(refs)) => Ok(refs),
        Ok(_) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected forge reply to ListRefs",
        )),
        Err(err) => Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &err)),
    }
}

/// build a receive-pack `report-status` body: `unpack ok`, one status line per
/// ref, then a flush. each entry is `(full refname, None == ok | Some(reason)
/// == ng)`. forge's PushRefs is ATOMIC, so callers report one shared fate for
/// every ref of a push. the pack is always received by the time we answer, so
/// `unpack ok` is unconditional (we don't verify closure here).
fn git_report_status(results: &[(String, Option<String>)]) -> Response {
    let mut body = Vec::new();
    body.extend_from_slice(&pkt_line(b"unpack ok\n"));
    for (refname, err) in results {
        let status_line = match err {
            None => format!("ok {refname}\n"),
            Some(reason) => format!("ng {refname} {reason}\n"),
        };
        body.extend_from_slice(&pkt_line(status_line.as_bytes()));
    }
    body.extend_from_slice(GIT_FLUSH_PKT);
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/x-git-receive-pack-result",
            ),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

/// query params for the ref advertisement; git always sends `service=`.
#[derive(Debug, Deserialize)]
pub struct InfoRefsParams {
    pub service: Option<String>,
}

/// which smart-HTTP service an info/refs advertisement is for — push
/// (receive-pack) or fetch (upload-pack). the two differ only in the banner,
/// the capability set, the content-type, and whether a `HEAD` line rides along.
#[derive(Clone, Copy)]
enum GitService {
    Receive,
    Upload,
}

impl GitService {
    fn name(self) -> &'static str {
        match self {
            Self::Receive => "git-receive-pack",
            Self::Upload => "git-upload-pack",
        }
    }
    fn caps(self) -> &'static str {
        match self {
            Self::Receive => GIT_RECEIVE_PACK_CAPS,
            Self::Upload => GIT_UPLOAD_PACK_CAPS,
        }
    }
    fn advertisement_content_type(self) -> &'static str {
        match self {
            Self::Receive => "application/x-git-receive-pack-advertisement",
            Self::Upload => "application/x-git-upload-pack-advertisement",
        }
    }
}

/// GET /forge/{repo}/info/refs?service=… — the smart-HTTP ref advertisement a
/// `git push`/`git clone` fetches FIRST to learn the remote's current head. the
/// advertised head is forge's COMMITTED head (from the actor lane, so it matches
/// consensus); the fetch POST then serves the matching objects off disk. both
/// receive-pack (push) and upload-pack (fetch) are served — the v0 banner we
/// send makes git speak the classic protocol for the follow-up POST even when it
/// probed with `Git-Protocol: version=2`.
pub(crate) async fn git_info_refs(
    State(handle): State<NodeHandle>,
    Path(repo): Path<String>,
    Query(params): Query<InfoRefsParams>,
) -> Response {
    let Ok(repo) = forge::norm_repo(&repo) else {
        return error_response(StatusCode::NOT_FOUND, "no such repo");
    };
    let service = match params.service.as_deref() {
        Some("git-receive-pack") => GitService::Receive,
        Some("git-upload-pack") => GitService::Upload,
        _ => {
            return error_response(
                StatusCode::FORBIDDEN,
                "only git-receive-pack and git-upload-pack are served",
            );
        }
    };
    git_advertise_refs(&handle, &repo, service).await
}

/// build the smart-HTTP ref advertisement for `service`: the service banner, a
/// flush, the ref line(s), then a flush. an unborn repo advertises the null oid
/// against the magic `capabilities^{}` ref (so caps ride along with no real ref)
/// — a clone then reports an empty repository. a born repo advertises EVERY
/// committed branch; a fetch advertisement leads with a `HEAD` line at main's
/// oid so `git clone` resolves the default branch to check out. capabilities
/// ride the first emitted line after a NUL, per the v0 protocol.
async fn git_advertise_refs(handle: &NodeHandle, repo: &str, service: GitService) -> Response {
    let refs = match forge_refs(handle, repo).await {
        Ok(refs) => refs,
        Err(resp) => return resp,
    };
    let caps = service.caps();

    let mut body = Vec::new();
    body.extend_from_slice(&pkt_line(
        format!("# service={}\n", service.name()).as_bytes(),
    ));
    body.extend_from_slice(GIT_FLUSH_PKT);
    if refs.is_empty() {
        body.extend_from_slice(&pkt_line(
            format!("{GIT_ZERO_OID} capabilities^{{}}\0{caps}\n").as_bytes(),
        ));
    } else {
        let mut lines: Vec<String> = Vec::new();
        if matches!(service, GitService::Upload)
            && let Some(main) = refs.iter().find(|r| r.name == "main")
        {
            lines.push(format!("{} HEAD", main.head));
        }
        for r in &refs {
            lines.push(format!("{} {GIT_HEADS_PREFIX}{}", r.head, r.name));
        }
        for (i, line) in lines.iter().enumerate() {
            if i == 0 {
                body.extend_from_slice(&pkt_line(format!("{line}\0{caps}\n").as_bytes()));
            } else {
                body.extend_from_slice(&pkt_line(format!("{line}\n").as_bytes()));
            }
        }
    }
    body.extend_from_slice(GIT_FLUSH_PKT);

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, service.advertisement_content_type()),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

/// return the request body, gzip-inflated if `Content-Encoding: gzip`. git may
/// compress a receive-pack request; any other encoding is passed through.
fn decode_git_body(headers: &HeaderMap, body: &[u8]) -> Result<Vec<u8>, String> {
    let gzip = headers
        .get(header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("gzip"));
    if !gzip {
        return Ok(body.to_vec());
    }
    use std::io::Read as _;
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(body)
        .read_to_end(&mut out)
        .map_err(|e| format!("gzip inflate failed: {e}"))?;
    Ok(out)
}

/// POST /forge/{repo}/git-receive-pack — receive a push: parse the ref-update
/// command list + packfile, stash the whole pack in the node-local blob store,
/// and CAS every branch through ONE atomic forge `PushRefs` op (one submit ==
/// one block). branch deletions (`:feature`) ride the same op pack-free. the
/// response is a git `report-status` reflecting the push's shared fate.
pub(crate) async fn git_receive_pack(
    State(handle): State<NodeHandle>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let Ok(repo) = forge::norm_repo(&repo) else {
        return error_response(StatusCode::NOT_FOUND, "no such repo");
    };
    let body = match body {
        Ok(bytes) => bytes,
        // the DefaultBodyLimit layer rejects an oversized pack with 413.
        Err(rejection) => return error_response(rejection.status(), &rejection.body_text()),
    };
    let body = match decode_git_body(&headers, &body) {
        Ok(bytes) => bytes,
        Err(msg) => return error_response(StatusCode::BAD_REQUEST, &msg),
    };

    // the body is a pkt-line command list, a flush-pkt, then the raw packfile.
    let (commands, pack) = match parse_pkt_lines(&body) {
        Ok(parsed) => parsed,
        Err(msg) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("malformed git command stream: {msg}"),
            );
        }
    };
    if commands.is_empty() {
        // a push whose pack exceeds git's `http.postBuffer` (1 MiB default) is
        // preceded by a flush-only PROBE POST (Content-Length: 4, body `0000`,
        // zero commands) before git streams the real chunked request. an empty
        // command list is a valid no-op: answer 200 with an empty result so the
        // probe succeeds and git proceeds with the actual push. 400 here aborts
        // every push larger than the post buffer.
        return (
            StatusCode::OK,
            [
                (
                    header::CONTENT_TYPE,
                    "application/x-git-receive-pack-result",
                ),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            GIT_FLUSH_PKT.to_vec(),
        )
            .into_response();
    }

    // each command line is `<old> <new> <refname>`, with capabilities after a
    // NUL on the FIRST line. parse every command — one push may update several
    // branches, and forge applies them ATOMICALLY.
    let mut cmds: Vec<(String, String, String)> = Vec::new();
    for raw in &commands {
        let nul = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let line = std::str::from_utf8(&raw[..nul])
            .map(str::trim_end)
            .unwrap_or("");
        let mut parts = line.split(' ');
        let (Some(old), Some(new), Some(refname)) = (parts.next(), parts.next(), parts.next())
        else {
            return error_response(StatusCode::BAD_REQUEST, "malformed ref-update command");
        };
        cmds.push((old.to_string(), new.to_string(), refname.to_string()));
    }

    // only branches are pushable (no tags/notes). consume-and-refuse: the pack
    // was fully received; reporting `ng` (not an http error) lets git print a
    // clean per-ref reason.
    if cmds
        .iter()
        .any(|(_, _, r)| !r.starts_with(GIT_HEADS_PREFIX))
    {
        let results: Vec<(String, Option<String>)> = cmds
            .into_iter()
            .map(|(_, _, r)| (r, Some(format!("only {GIT_HEADS_PREFIX}* is supported"))))
            .collect();
        return git_report_status(&results);
    }

    // old/new == the null oid mean "create" (prev_oid None) / "delete" (new_oid
    // None); otherwise 40-hex oids the forge per-branch CAS must match.
    let mut updates = Vec::new();
    for (old, new, refname) in &cmds {
        let prev_oid = if old == GIT_ZERO_OID {
            None
        } else {
            match hex_to_bytes(old).filter(|b| b.len() == GIT_OID_RAW_LEN) {
                Some(bytes) => Some(bytes),
                None => return error_response(StatusCode::BAD_REQUEST, "malformed old oid"),
            }
        };
        let new_oid = if new == GIT_ZERO_OID {
            None
        } else {
            match hex_to_bytes(new).filter(|b| b.len() == GIT_OID_RAW_LEN) {
                Some(bytes) => Some(bytes),
                None => return error_response(StatusCode::BAD_REQUEST, "malformed new oid"),
            }
        };
        updates.push(forge::RefUpdate {
            ref_name: refname[GIT_HEADS_PREFIX.len()..].to_string(),
            prev_oid,
            new_oid,
        });
    }

    // stash the WHOLE packfile as one node-local blob, keyed by its sha256;
    // forge materializes it by this digest (the bytes never cross consensus).
    // a delete-only push carries no objects, so nothing is stashed.
    let pack_digest = if updates.iter().any(|u| u.new_oid.is_some()) {
        Some(handle.blobs.put_chunk(pack.to_vec()).to_vec())
    } else {
        None
    };

    // CAS every branch through ONE atomic PushRefs op and await the block.
    let payload = forge::encode_msg(&forge::ForgeMsg::PushRefs {
        repo,
        updates,
        pack_digest,
    });
    let (reply, rx) = oneshot::channel();
    if let Err(resp) = handle
        .send(NodeCommand::Submit {
            target: "forge".into(),
            payload,
            origin: DEFAULT_ORIGIN.as_bytes().to_vec(),
            reply,
        })
        .await
    {
        return resp;
    }
    let refnames: Vec<String> = cmds.into_iter().map(|(_, _, r)| r).collect();
    match rx.await {
        Ok(Ok(_block)) => {
            let results: Vec<(String, Option<String>)> =
                refnames.into_iter().map(|r| (r, None)).collect();
            git_report_status(&results)
        }
        Ok(Err(reason)) => {
            // a CAS mismatch's rejection carries "non-fast-forward" — surface
            // exactly that token so git prints its standard "fetch first" hint.
            // any other rejection passes through as a single-line reason. the
            // op is atomic, so every ref shares the fate.
            let reason = if reason.contains("non-fast-forward") {
                "non-fast-forward".to_string()
            } else {
                reason.replace('\n', " ")
            };
            let results: Vec<(String, Option<String>)> = refnames
                .into_iter()
                .map(|r| (r, Some(reason.clone())))
                .collect();
            git_report_status(&results)
        }
        Err(_) => actor_gone(),
    }
}

/// POST /forge/{repo}/git-upload-pack — serve a fetch/clone. parse the pkt-line
/// negotiation (`want <oid>` lines, capabilities on the FIRST; `have`s receive
/// NAK until `done` — the MVP serves a full closure), open
/// `<forge_repo>/{repo}` READ-ONLY, build a packfile of the wanted oids' closure,
/// and, after `done`, reply `NAK` then the pack muxed on side-band-64k band 1.
/// incremental (`have`-aware) fetch is future work: a full pack is always
/// correct, just larger, and `git pull` still works (it refetches).
pub(crate) async fn git_upload_pack(
    State(handle): State<NodeHandle>,
    Path(repo): Path<String>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let Ok(repo) = forge::norm_repo(&repo) else {
        return error_response(StatusCode::NOT_FOUND, "no such repo");
    };
    let Some(forge_repo) = handle.forge_repo.clone() else {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "forge repo path not configured on this node",
        );
    };
    let body = match body {
        Ok(bytes) => bytes,
        // the DefaultBodyLimit layer rejects an oversized request with 413.
        Err(rejection) => return error_response(rejection.status(), &rejection.body_text()),
    };
    let body = match decode_git_body(&headers, &body) {
        Ok(bytes) => bytes,
        Err(msg) => return error_response(StatusCode::BAD_REQUEST, &msg),
    };

    let request = match parse_upload_pack_request(&body) {
        Ok(request) => request,
        Err(msg) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("malformed git-upload-pack request: {msg}"),
            );
        }
    };

    // A flush-ended have batch is an intermediate negotiation round. Returning
    // only NAK (and no side-band/PACK bytes) lets stock git send its next batch.
    if !request.done {
        return (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/x-git-upload-pack-result"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            pkt_line(b"NAK\n"),
        )
            .into_response();
    }

    // the pack build is blocking git2 IO over a non-Send `Repository`; run it off
    // the async worker, moving only Send data (the dir + hex oids) across.
    let repo_dir = forge_repo.join(&repo);
    let UploadPackRequest {
        wants,
        haves,
        side_band,
        ..
    } = request;
    let (pack, common) =
        match tokio::task::spawn_blocking(move || build_upload_pack(&repo_dir, &wants, &haves))
            .await
        {
            Ok(Ok(built)) => built,
            Ok(Err(msg)) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &msg),
            Err(_) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "git pack builder task panicked",
                );
            }
        };

    let mut out = Vec::new();
    // the terminal negotiation line, valid in every v0 multi_ack mode: a bare
    // `ACK <oid>` names the common base the pack builds on (the delta answer),
    // NAK means no usable have was found (the pack is then the full closure).
    // either way a PLAIN pkt-line, BEFORE any side-band framing begins.
    match &common {
        Some(oid) => out.extend_from_slice(&pkt_line(format!("ACK {oid}\n").as_bytes())),
        None => out.extend_from_slice(&pkt_line(b"NAK\n")),
    }
    if side_band {
        // band 1 = pack data, chunked to the side-band-64k ceiling.
        for chunk in pack.chunks(GIT_SIDE_BAND_CHUNK) {
            let mut framed = Vec::with_capacity(chunk.len() + 1);
            framed.push(0x01);
            framed.extend_from_slice(chunk);
            out.extend_from_slice(&pkt_line(&framed));
        }
        out.extend_from_slice(GIT_FLUSH_PKT);
    } else {
        // the client didn't request side-band: the raw pack follows NAK directly
        // (no band framing, no trailing flush — the pack trailer ends the stream).
        out.extend_from_slice(&pack);
    }

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/x-git-upload-pack-result"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        out,
    )
        .into_response()
}

/// build the packfile answering `want_hexes`, bounded by the client's haves:
/// every have this repo knows as a commit hides its closure from the walk
/// (forge's `pack_delta`), so a mirror refresh downloads only what moved. a
/// client with NO usable common base still gets the FULL self-contained
/// closure (forge's `pack_closure_many` — ONE packing implementation for the
/// module's snapshot pack and this fetch lane). returns the pack plus the
/// first usable common base, which the handler ACKs. any git2 failure — a
/// missing repo dir, an oid absent from the odb, a pack-write error — is
/// returned as a message the handler surfaces.
fn build_upload_pack(
    repo_dir: &std::path::Path,
    want_hexes: &[String],
    have_hexes: &[String],
) -> Result<(Vec<u8>, Option<String>), String> {
    let repo = git2::Repository::open(repo_dir).map_err(|e| format!("open forge repo: {e}"))?;
    let mut oids = Vec::with_capacity(want_hexes.len());
    for hex in want_hexes {
        oids.push(git2::Oid::from_str(hex).map_err(|e| format!("bad want oid {hex}: {e}"))?);
    }
    // only haves this repo KNOWS as commits can bound the walk — a have from
    // history this node never saw simply doesn't help (and never errors).
    let mut common = Vec::new();
    for hex in have_hexes {
        let Ok(oid) = git2::Oid::from_str(hex) else {
            continue; // parser already validated; belt and braces
        };
        if repo.find_commit(oid).is_ok() {
            common.push(oid);
        }
    }
    if common.is_empty() {
        return forge::pack_closure_many(&repo, &oids)
            .map(|pack| (pack, None))
            .map_err(|e| format!("build pack: {e}"));
    }
    let ack = common[0].to_string();
    forge::pack_delta(&repo, &oids, &common)
        .map(|pack| (pack, Some(ack)))
        .map_err(|e| format!("build delta pack: {e}"))
}

#[cfg(test)]
mod upload_pack_tests {
    use super::*;

    const WANT: &str = "1111111111111111111111111111111111111111";
    const HAVE: &str = "2222222222222222222222222222222222222222";

    fn request_tail(tail: &[u8]) -> Vec<u8> {
        let mut body = pkt_line(format!("want {WANT} multi_ack_detailed side-band-64k\n").as_bytes());
        body.extend_from_slice(GIT_FLUSH_PKT);
        body.extend_from_slice(tail);
        body
    }

    #[test]
    fn flush_ended_have_round_is_not_done() {
        let mut tail = pkt_line(format!("have {HAVE}\n").as_bytes());
        tail.extend_from_slice(GIT_FLUSH_PKT);

        let parsed = parse_upload_pack_request(&request_tail(&tail)).expect("valid request");

        assert_eq!(parsed.wants, vec![WANT.to_string()]);
        assert_eq!(parsed.haves, vec![HAVE.to_string()]);
        assert!(parsed.side_band);
        assert!(!parsed.done, "a have flush must not authorize pack bytes");
    }

    #[test]
    fn explicit_done_completes_negotiation() {
        let mut tail = pkt_line(format!("have {HAVE}\n").as_bytes());
        tail.extend_from_slice(&pkt_line(b"done\n"));

        let parsed = parse_upload_pack_request(&request_tail(&tail)).expect("valid request");

        assert!(parsed.done);
        assert!(parsed.side_band);
        assert_eq!(parsed.haves, vec![HAVE.to_string()]);
    }

    /// two commits at the origin; a client that has the first must get a pack
    /// that (a) is smaller than the full closure and (b) still completes the
    /// second commit when installed next to the objects it already holds.
    #[test]
    fn haves_bound_the_pack_to_what_moved() {
        let dir = tempfile::tempdir().unwrap();
        let origin = git2::Repository::init(dir.path()).unwrap();
        let sig = git2::Signature::now("test", "test@example.com").unwrap();

        let blob_a = origin.blob(b"one").unwrap();
        let mut tb = origin.treebuilder(None).unwrap();
        tb.insert("a.txt", blob_a, 0o100644).unwrap();
        let tree1 = origin.find_tree(tb.write().unwrap()).unwrap();
        let first = origin
            .commit(Some("refs/heads/dev"), &sig, &sig, "one", &tree1, &[])
            .unwrap();

        let blob_b = origin.blob(b"two").unwrap();
        let mut tb = origin.treebuilder(Some(&tree1)).unwrap();
        tb.insert("b.txt", blob_b, 0o100644).unwrap();
        let tree2 = origin.find_tree(tb.write().unwrap()).unwrap();
        let first_commit = origin.find_commit(first).unwrap();
        let second = origin
            .commit(
                Some("refs/heads/dev"),
                &sig,
                &sig,
                "two",
                &tree2,
                &[&first_commit],
            )
            .unwrap();

        let want = vec![second.to_string()];
        let (full, ack) = build_upload_pack(dir.path(), &want, &[]).unwrap();
        assert_eq!(ack, None, "no haves -> full closure after NAK");
        let (delta, ack) = build_upload_pack(dir.path(), &want, &[first.to_string()]).unwrap();
        assert_eq!(ack, Some(first.to_string()), "the common base is ACKed");
        assert!(
            delta.len() < full.len(),
            "delta pack ({}) must be smaller than the closure ({})",
            delta.len(),
            full.len()
        );

        // an unknown have cannot bound the walk — the answer stays full.
        let (fallback, ack) =
            build_upload_pack(dir.path(), &want, &[HAVE.to_string()]).unwrap();
        assert_eq!(ack, None);
        assert_eq!(fallback.len(), full.len());

        // install first's closure, then the delta, into a fresh repo: the
        // second commit and BOTH blobs must be readable — the delta carried
        // everything the client didn't already hold.
        let clone_dir = tempfile::tempdir().unwrap();
        let clone = git2::Repository::init_bare(clone_dir.path()).unwrap();
        let (base_pack, _) = build_upload_pack(dir.path(), &[first.to_string()], &[]).unwrap();
        for pack in [&base_pack, &delta] {
            let odb = clone.odb().unwrap();
            let mut pw = odb.packwriter().unwrap();
            std::io::Write::write_all(&mut pw, pack).unwrap();
            pw.commit().unwrap();
        }
        let landed = clone.find_commit(second).unwrap();
        assert_eq!(landed.tree().unwrap().len(), 2);
        assert!(clone.find_blob(blob_a).is_ok());
        assert!(clone.find_blob(blob_b).is_ok());
    }

    #[test]
    fn malformed_negotiation_tail_is_rejected() {
        let err = parse_upload_pack_request(&request_tail(b"0009wat!\n"))
            .err()
            .expect("unknown negotiation line must fail");

        assert!(err.contains("unexpected upload-pack negotiation line"));
    }
}
