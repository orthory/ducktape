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

/// the heads an advertisement offers — NOT the same set for the two services.
///
/// PUSH advertises forge's COMMITTED heads: the client builds its ref commands
/// against what it is shown and consensus gates each as a CAS against the
/// committed head, so advertising anything else mints a doomed push.
///
/// FETCH advertises what this node can actually SERVE — its ON-DISK refs. the
/// two diverge on a node whose objects have not caught up yet (a resident, or
/// a validator that was down for the push: see `RepoState::materialize`), and
/// there, offering the committed head is worse than offering the older one.
/// `git clone` wants every ref it was shown, the pack builder cannot walk an
/// oid whose objects are missing, and the error takes the WHOLE clone down —
/// not just the branch that lagged. an older head is what any mirror serves,
/// and the node's pack sweep catches it up within a tick.
async fn advertised_refs(
    handle: &NodeHandle,
    repo: &str,
    service: GitService,
) -> Result<Vec<forge::RefHead>, Response> {
    match service {
        GitService::Receive => forge_refs(handle, repo).await,
        GitService::Upload => servable_refs(handle, repo)
            .map_err(|why| error_response(StatusCode::INTERNAL_SERVER_ERROR, &why)),
    }
}

/// the fetch half of [`advertised_refs`], reading the same on-disk repo
/// [`build_upload_pack`] packs from.
fn servable_refs(handle: &NodeHandle, repo: &str) -> Result<Vec<forge::RefHead>, String> {
    let Some(base) = handle.forge_repo.as_deref() else {
        return Err("forge repo path not configured on this node".into());
    };
    on_disk_refs(base, repo).map_err(|e| format!("read forge refs: {e}"))
}

/// this node's on-disk branches for `repo`. a repo dir nothing has
/// materialized here yet is an empty listing, which advertises as an empty
/// repository — the same answer an unborn repo gives.
fn on_disk_refs(base: &std::path::Path, repo: &str) -> Result<Vec<forge::RefHead>, git2::Error> {
    let dir = base.join(repo);
    if !dir.join(".git").exists() {
        return Ok(Vec::new());
    }
    let repo = git2::Repository::open(&dir)?;
    Ok(forge::list_branches(&repo)?
        .into_iter()
        .map(|(name, head)| forge::RefHead {
            name,
            head: head.to_string(),
        })
        .collect())
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

/// a receive-pack command list, decoded.
#[derive(Debug)]
struct PushCommands {
    /// `(old hex, new hex, full refname)` per update — the report-status
    /// keys, in the order the pusher named them.
    cmds: Vec<(String, String, String)>,
    /// `git push --signed`'s certificate, when one rode along.
    cert: Option<forge::PushCert>,
}

const PUSH_CERT_LINE: &str = "push-cert";
const PUSH_CERT_END: &str = "push-cert-end";
const SSHSIG_ARMOR_BEGIN: &str = "-----BEGIN SSH SIGNATURE-----";

/// decode the command list. a stock push sends `<old> <new> <refname>` lines
/// (capabilities after a NUL on the first). a signed push (send-pack.c
/// `generate_push_cert`) sends `push-cert\0<caps>` instead, then every line
/// of the certificate — its text, then the armored signature — one pkt-line
/// each WITH its newline, then `push-cert-end`; the ref updates are inside
/// the certificate, and the plain lines are not sent. `expected_nonce` is
/// what this node advertised: a certificate must echo it (the chain half of
/// the nonce is checked here, the repo half by consensus).
fn parse_push_commands(
    commands: &[Vec<u8>],
    expected_nonce: Option<&str>,
) -> Result<PushCommands, String> {
    let Some((first, rest)) = commands.split_first() else {
        return Err("empty command list".into());
    };
    let signed = command_text(first) == PUSH_CERT_LINE;
    if !signed {
        let cmds = commands
            .iter()
            .map(|raw| command_triple(command_text(raw)))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(PushCommands { cmds, cert: None });
    }
    let Some(expected_nonce) = expected_nonce else {
        return Err("this node offered no push-cert (its chain is not named yet)".into());
    };
    let (text, armor) = certificate_lines(rest)?;
    let sshsig = keyscheme::sshsig::dearmor(&armor)?;
    let certificate = forge::pushcert::parse(text.as_bytes())?;
    if certificate.nonce != expected_nonce {
        return Err(format!(
            "push certificate nonce {:?} is not this node's {expected_nonce:?}",
            certificate.nonce
        ));
    }
    let cmds = certificate
        .updates
        .iter()
        .map(|u| {
            (
                oid_hex(u.prev_oid.as_deref()),
                oid_hex(u.new_oid.as_deref()),
                format!("{GIT_HEADS_PREFIX}{}", u.ref_name),
            )
        })
        .collect();
    Ok(PushCommands {
        cmds,
        cert: Some(forge::PushCert {
            cert: text.into_bytes(),
            sshsig,
        }),
    })
}

/// the certificate's text and its armored signature, reassembled from the
/// pkt-lines between `push-cert` and `push-cert-end` — verbatim, newline for
/// newline, because the signature is over exactly those bytes.
fn certificate_lines(lines: &[Vec<u8>]) -> Result<(String, String), String> {
    let mut text = String::new();
    let mut armor = String::new();
    for raw in lines {
        let line = std::str::from_utf8(raw).map_err(|_| "push certificate is not utf-8")?;
        if line.trim_end() == PUSH_CERT_END {
            return Ok((text, armor));
        }
        let in_armor = !armor.is_empty() || line.starts_with(SSHSIG_ARMOR_BEGIN);
        if in_armor {
            armor.push_str(line);
        } else {
            text.push_str(line);
        }
    }
    Err("push certificate is not terminated by push-cert-end".into())
}

/// a command pkt-line's text: up to the NUL that starts the capability list
/// (first line only), trailing newline dropped.
fn command_text(raw: &[u8]) -> &str {
    let nul = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    std::str::from_utf8(&raw[..nul])
        .map(str::trim_end)
        .unwrap_or("")
}

fn command_triple(line: &str) -> Result<(String, String, String), String> {
    let mut parts = line.split(' ');
    let (Some(old), Some(new), Some(refname)) = (parts.next(), parts.next(), parts.next()) else {
        return Err("malformed ref-update command".into());
    };
    Ok((old.to_string(), new.to_string(), refname.to_string()))
}

fn oid_hex(oid: Option<&[u8]>) -> String {
    match oid {
        Some(bytes) => bytes.iter().map(|b| format!("{b:02x}")).collect(),
        None => GIT_ZERO_OID.to_string(),
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

/// the capability line for `service` on `repo`. receive-pack additionally
/// offers `push-cert=<nonce>` — the invitation `git push --signed` needs
/// (git refuses to sign a push the server did not offer a nonce for) — once
/// this node knows its chain: the nonce is `<chain id>/<repo>`.
fn advertised_caps(handle: &NodeHandle, repo: &str, service: GitService) -> String {
    let base = service.caps();
    let nonce = match service {
        GitService::Receive => push_cert_nonce(handle, repo),
        GitService::Upload => None,
    };
    match nonce {
        Some(nonce) => format!("{base} push-cert={nonce}"),
        None => base.to_string(),
    }
}

/// the push-cert nonce this node offers for `repo`; `None` until the status
/// cell names the chain (a node still booting offers no signed pushes).
fn push_cert_nonce(handle: &NodeHandle, repo: &str) -> Option<String> {
    let chain_id = handle.status.current().chain_id;
    let named = !chain_id.is_empty();
    named.then(|| forge::pushcert::nonce(&chain_id, repo))
}

/// GET /forge/{repo}/info/refs?service=… — the smart-HTTP ref advertisement a
/// `git push`/`git clone` fetches FIRST to learn the remote's current head.
/// which heads those are differs per service (see [`advertised_refs`]). both
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
    let refs = match advertised_refs(handle, repo, service).await {
        Ok(refs) => refs,
        Err(resp) => return resp,
    };
    let caps = advertised_caps(handle, repo, service);

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

    // the command list: plain `<old> <new> <refname>` lines, or — a signed
    // push — the certificate they live in. one push may update several
    // branches, and forge applies them ATOMICALLY.
    let nonce = push_cert_nonce(&handle, &repo);
    let PushCommands { cmds, cert } = match parse_push_commands(&commands, nonce.as_deref()) {
        Ok(parsed) => parsed,
        Err(msg) => return error_response(StatusCode::BAD_REQUEST, &msg),
    };

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

    // a signed push is refused HERE with the reason consensus would give — a
    // clean per-ref `ng` instead of a rejected block. every validator
    // re-verifies; this node is not trusted for it.
    if let Some(cert) = &cert
        && let Err(reason) = forge::pushcert::signer(cert, &repo, &updates)
    {
        let results: Vec<(String, Option<String>)> = cmds
            .into_iter()
            .map(|(_, _, r)| (r, Some(reason.clone())))
            .collect();
        return git_report_status(&results);
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
        cert,
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
/// negotiation (`want <oid>` lines, capabilities on the FIRST; flush-ended
/// `have` rounds receive plain NAK so the client keeps batching), open
/// `<forge_repo>/{repo}` READ-ONLY, and after `done` answer with the pack on
/// side-band-64k band 1: a have-bounded delta behind `ACK <common>` when the
/// repo knows any of the client's haves, or the full closure behind NAK when
/// it knows none (see [`build_upload_pack`]).
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
mod receive_pack_tests {
    use super::*;

    /// the real `ssh-keygen -Y sign -n git` fixture keyscheme and forge pin,
    /// framed exactly as `git push --signed` frames it.
    const CERT: &str = "certificate version 0.1\npusher key::ssh-ed25519 AAAA 1756332000 +0000\npushee http://127.0.0.1:8844/forge/lab\nnonce chain-a/lab\n\n0000000000000000000000000000000000000000 ab5b1f3d5b7e3e0e0d33e2c6d1f6c2a7d3a7f1e2 refs/heads/main\n";
    const ARMORED: &str = "-----BEGIN SSH SIGNATURE-----\n\
U1NIU0lHAAAAAQAAADMAAAALc3NoLWVkMjU1MTkAAAAgJjhQt02r3vG8+pxaBdryKnexRC\n\
cULQqMrrcadzt/2iEAAAADZ2l0AAAAAAAAAAZzaGE1MTIAAABTAAAAC3NzaC1lZDI1NTE5\n\
AAAAQAkqyuC4rshUkBgUVsgAqGxBltLKRLcwdq5LAQn+2lCUmiUJWTsYTykmuaNO+cntB2\n\
ZYBzkWoVNWmNV5YTCuZwE=\n\
-----END SSH SIGNATURE-----\n";

    fn signed_commands() -> Vec<Vec<u8>> {
        let mut lines = vec![b"push-cert\0report-status agent=git/2.43.0\n".to_vec()];
        for line in CERT.split_inclusive('\n') {
            lines.push(line.as_bytes().to_vec());
        }
        for line in ARMORED.split_inclusive('\n') {
            lines.push(line.as_bytes().to_vec());
        }
        lines.push(b"push-cert-end\n".to_vec());
        lines
    }

    #[test]
    fn a_signed_push_yields_its_certificate_and_the_moves_inside_it() {
        let parsed = parse_push_commands(&signed_commands(), Some("chain-a/lab")).unwrap();
        assert_eq!(
            parsed.cmds,
            vec![(
                GIT_ZERO_OID.to_string(),
                "ab5b1f3d5b7e3e0e0d33e2c6d1f6c2a7d3a7f1e2".to_string(),
                "refs/heads/main".to_string()
            )]
        );
        let cert = parsed.cert.expect("a certificate");
        assert_eq!(cert.cert, CERT.as_bytes(), "the signed bytes, verbatim");
        assert_eq!(cert.sshsig, keyscheme::sshsig::dearmor(ARMORED).unwrap());
        // and it is what consensus will accept.
        let updates = vec![forge::RefUpdate {
            ref_name: "main".into(),
            prev_oid: None,
            new_oid: Some(hex_to_bytes("ab5b1f3d5b7e3e0e0d33e2c6d1f6c2a7d3a7f1e2").unwrap()),
        }];
        forge::pushcert::signer(&cert, "lab", &updates).expect("verifies");
    }

    #[test]
    fn a_certificate_must_echo_this_nodes_nonce_and_be_terminated() {
        let wrong = parse_push_commands(&signed_commands(), Some("chain-b/lab")).unwrap_err();
        assert!(wrong.contains("nonce"), "{wrong}");
        let unoffered = parse_push_commands(&signed_commands(), None).unwrap_err();
        assert!(unoffered.contains("offered no push-cert"), "{unoffered}");
        let mut cut = signed_commands();
        cut.pop();
        let cut = parse_push_commands(&cut, Some("chain-a/lab")).unwrap_err();
        assert!(cut.contains("push-cert-end"), "{cut}");
    }

    #[test]
    fn a_stock_push_still_parses_line_by_line() {
        let commands = vec![
            b"0000000000000000000000000000000000000000 ab5b1f3d5b7e3e0e0d33e2c6d1f6c2a7d3a7f1e2 refs/heads/main\0report-status\n".to_vec(),
            b"ab5b1f3d5b7e3e0e0d33e2c6d1f6c2a7d3a7f1e2 0000000000000000000000000000000000000000 refs/heads/old\n".to_vec(),
        ];
        let parsed = parse_push_commands(&commands, None).unwrap();
        assert!(parsed.cert.is_none());
        assert_eq!(parsed.cmds.len(), 2);
        assert_eq!(parsed.cmds[1].2, "refs/heads/old");
        assert!(parse_push_commands(&[b"junk\n".to_vec()], None).is_err());
    }
}

#[cfg(test)]
mod upload_pack_tests {
    use super::*;

    const WANT: &str = "1111111111111111111111111111111111111111";
    const HAVE: &str = "2222222222222222222222222222222222222222";

    fn request_tail(tail: &[u8]) -> Vec<u8> {
        let mut body =
            pkt_line(format!("want {WANT} multi_ack_detailed side-band-64k\n").as_bytes());
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

    /// a fetch advertisement offers exactly what this node can pack: nothing
    /// for a repo it has never materialized, and afterwards the ON-DISK heads
    /// — never a committed head whose objects have not arrived, which would
    /// take the whole clone down instead of just lagging one branch.
    #[test]
    fn on_disk_refs_offer_only_the_branches_this_node_can_pack() {
        let base = tempfile::tempdir().unwrap();
        assert!(
            on_disk_refs(base.path(), "demo").unwrap().is_empty(),
            "a repo nothing materialized here advertises as empty"
        );

        let repo = git2::Repository::init(base.path().join("demo")).unwrap();
        let sig = git2::Signature::now("test", "test@example.com").unwrap();
        let blob = repo.blob(b"one").unwrap();
        let mut tb = repo.treebuilder(None).unwrap();
        tb.insert("a.txt", blob, 0o100644).unwrap();
        let tree = repo.find_tree(tb.write().unwrap()).unwrap();
        let head = repo
            .commit(Some("refs/heads/main"), &sig, &sig, "one", &tree, &[])
            .unwrap();
        repo.reference("refs/heads/feature/x", head, true, "test")
            .unwrap();

        let refs = on_disk_refs(base.path(), "demo").unwrap();

        let names: Vec<&str> = refs.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["feature/x", "main"], "every born branch, sorted");
        for r in &refs {
            assert_eq!(r.head, head.to_string(), "at its on-disk oid");
        }
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
        let (fallback, ack) = build_upload_pack(dir.path(), &want, &[HAVE.to_string()]).unwrap();
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
