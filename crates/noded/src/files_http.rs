//! duckfs product surface: thin wrappers over the files module's ops/queries.
//!
//! every handler is encode → existing-seam → decode → json: it builds the duckfs
//! wire (the binary putblob frame or a `FilesMsg`/`FilesQuery` json) and threads
//! it through the SAME `NodeCommand::Submit`/`Query` lane the generic /v1/submit
//! and /v1/query use, so there is no new consensus path and no per-module
//! plumbing beyond the wire encode. all writes ride the daemon's own external
//! origin (like an unnamed /v1/submit) — this is the LOCAL-TRUST convenience
//! lane (the CLI's write path, origin-guarded, loopback). authenticated
//! per-user authorship does not thread through here: the desktop app signs a
//! `FilesMsg` into an op frame with the user key (`user-sign-frame`) and
//! POSTs it to `/v1/submit/frame`, whose verified signer becomes the commit's
//! author. a public deployment gates or disables THIS lane; the frame lane is
//! the one that survives untrusted callers.
//!
//! extracted from `lib.rs` (which is already over the file-size cap) so the
//! duckfs surface grows in its own module; the workspace rpc (task 9) reuses the
//! `pub(crate)` submit/query helpers here rather than re-plumbing the actor lane.

use axum::Json;
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use duckfs_core::objects::object_id;
use duckfs_core::{
    Change, FilesMsg, FilesQuery, FilesReply, Kind, MAX_PAGE, MAX_READ_BYTES, decode_reply,
    encode_msg, encode_putblob, encode_query, to_hex,
};
use futures::channel::oneshot;
use serde::Deserialize;

use crate::{BlockSummary, DEFAULT_ORIGIN, NodeCommand, NodeHandle, actor_gone, error_response};

/// the target module every duckfs endpoint encodes for.
pub(crate) const FILES_MODULE: &str = "files";

/// submit raw op bytes to the files module over the actor seam, returning the
/// committed block or the module's rejection as a 400. the ONE submit path —
/// the duckfs write endpoints just encode their wire (putblob frame or
/// `FilesMsg` json) first, so nothing here touches consensus differently from
/// /v1/submit.
pub(crate) async fn files_submit(
    handle: &NodeHandle,
    payload: Vec<u8>,
) -> Result<BlockSummary, Response> {
    let (reply, rx) = oneshot::channel();
    handle
        .send(NodeCommand::Submit {
            target: FILES_MODULE.into(),
            payload,
            origin: DEFAULT_ORIGIN.as_bytes().to_vec(),
            reply,
        })
        .await?;
    match rx.await {
        Ok(Ok(block)) => Ok(block),
        Ok(Err(err)) => Err(error_response(StatusCode::BAD_REQUEST, &err)),
        Err(_) => Err(actor_gone()),
    }
}

/// run a files query over the actor seam and decode the typed reply. a module
/// rejection maps through [`files_query_error`]; a reply the codec cannot decode
/// is a 500 (the module and daemon share the wire, so it never should).
pub(crate) async fn files_query(
    handle: &NodeHandle,
    q: &FilesQuery,
) -> Result<FilesReply, Response> {
    let (reply, rx) = oneshot::channel();
    handle
        .send(NodeCommand::Query {
            target: FILES_MODULE.into(),
            req: encode_query(q),
            reply,
        })
        .await?;
    let bytes = match rx.await {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(err)) => return Err(files_query_error(&err)),
        Err(_) => return Err(actor_gone()),
    };
    decode_reply(&bytes).map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))
}

/// map a files query rejection to an http status: an absent path or an
/// unresolvable snapshot is the natural 404; every other rejection (not a
/// directory, empty grep pattern, oversized diff) is a 400.
pub(crate) fn files_query_error(err: &str) -> Response {
    let status = if err.contains("not found") || err.contains("not resolvable") {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::BAD_REQUEST
    };
    error_response(status, err)
}

/// the module always answers a query with the matching reply variant, so any
/// other variant is a daemon/module wire drift — a 500, never silently coerced.
pub(crate) fn wrong_reply() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "unexpected files reply variant",
    )
}

/// POST /v1/files/stage — raw chunk bytes in, `{"digest":"<64-hex>"}` out.
///
/// wraps the body in the binary putblob frame and submits it as a files op: the
/// chunk lands in the odb and the staging table (staging IS consensus state, so
/// a stage moves the module root), and a later /v1/files/commit references the
/// digest. the digest is the chunk's object id — sha256 over the chunk kind tag
/// followed by the bytes — computed here so a caller can name it in a commit
/// without a round-trip, and byte-identical to what the module stages under.
pub(crate) async fn files_stage(
    State(handle): State<NodeHandle>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let bytes = match body {
        Ok(bytes) => bytes,
        // the DefaultBodyLimit layer stops reading past CHUNK_SIZE and rejects
        // with 413 — re-wrap it in the json envelope.
        Err(rejection) => return error_response(rejection.status(), &rejection.body_text()),
    };
    let digest = to_hex(&object_id(Kind::Chunk, &bytes));
    match files_submit(&handle, encode_putblob(&bytes)).await {
        Ok(_) => Json(serde_json::json!({ "digest": digest })).into_response(),
        Err(resp) => resp,
    }
}

/// the json body of POST /v1/files/commit — a `FilesMsg::Commit` spec. snake_case
/// like the module wire (`changes` carries `Change`/`Content`, both snake), so
/// the whole body reads as one duckfs document.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitBody {
    /// the base snapshot the per-path CAS checks against; omitted/`null` means
    /// the empty tree (a first commit).
    #[serde(default)]
    pub base_snapshot: Option<String>,
    #[serde(default)]
    pub message: String,
    pub changes: Vec<Change>,
}

/// POST /v1/files/commit — an atomic multi-path commit. encodes `FilesMsg::Commit`
/// and submits it; the reply is the block that included it.
pub(crate) async fn files_commit(
    State(handle): State<NodeHandle>,
    Json(body): Json<CommitBody>,
) -> Response {
    let payload = encode_msg(&FilesMsg::Commit {
        base_snapshot: body.base_snapshot,
        message: body.message,
        changes: body.changes,
    });
    match files_submit(&handle, payload).await {
        Ok(block) => Json(block).into_response(),
        Err(resp) => resp,
    }
}

/// the json body of POST /v1/files/pin.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinBody {
    pub snapshot: String,
    pub name: String,
}

/// POST /v1/files/pin — pin a snapshot by name so gc keeps it reachable.
pub(crate) async fn files_pin(
    State(handle): State<NodeHandle>,
    Json(body): Json<PinBody>,
) -> Response {
    let payload = encode_msg(&FilesMsg::Pin {
        snapshot: body.snapshot,
        name: body.name,
    });
    match files_submit(&handle, payload).await {
        Ok(block) => Json(block).into_response(),
        Err(resp) => resp,
    }
}

/// the json body of POST /v1/files/watch.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchBody {
    pub prefix: String,
    pub module_id: String,
}

/// POST /v1/files/watch — subscribe a module to a subtree. the module gates
/// watch registration to a MODULE origin, so an external submit here is a clean
/// 400 (this lane authenticates nothing); a module registers its own watch by
/// emitting the op inside a block instead.
pub(crate) async fn files_watch(
    State(handle): State<NodeHandle>,
    Json(body): Json<WatchBody>,
) -> Response {
    let payload = encode_msg(&FilesMsg::Watch {
        prefix: body.prefix,
        module_id: body.module_id,
    });
    match files_submit(&handle, payload).await {
        Ok(block) => Json(block).into_response(),
        Err(resp) => resp,
    }
}

/// query params for GET /v1/files/stat.
#[derive(Debug, Deserialize)]
pub struct StatParams {
    pub path: String,
    #[serde(default)]
    pub snapshot: Option<String>,
}

/// GET /v1/files/stat?path=&snapshot= — the entry at `path` (kind/size/exec/
/// object/meta), or a 404 when nothing is there.
pub(crate) async fn files_stat(
    State(handle): State<NodeHandle>,
    Query(p): Query<StatParams>,
) -> Response {
    match files_query(
        &handle,
        &FilesQuery::Stat {
            path: p.path,
            snapshot: p.snapshot,
        },
    )
    .await
    {
        Ok(FilesReply::Stat(Some(entry))) => Json(entry).into_response(),
        Ok(FilesReply::Stat(None)) => {
            error_response(StatusCode::NOT_FOUND, "no entry at that path")
        }
        Ok(_) => wrong_reply(),
        Err(resp) => resp,
    }
}

/// query params for GET /v1/files/ls.
#[derive(Debug, Deserialize)]
pub struct LsParams {
    pub path: String,
    #[serde(default)]
    pub snapshot: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default)]
    pub limit: Option<u64>,
}

/// GET /v1/files/ls?path=&after=&limit=&snapshot= — one page of a directory's
/// entries in name order, with a `next` cursor to echo as the following `after`.
pub(crate) async fn files_ls(
    State(handle): State<NodeHandle>,
    Query(p): Query<LsParams>,
) -> Response {
    let q = FilesQuery::Ls {
        path: p.path,
        snapshot: p.snapshot,
        after: p.after,
        limit: p.limit.unwrap_or(MAX_PAGE),
    };
    match files_query(&handle, &q).await {
        Ok(FilesReply::Ls { entries, next }) => {
            Json(serde_json::json!({ "entries": entries, "next": next })).into_response()
        }
        Ok(_) => wrong_reply(),
        Err(resp) => resp,
    }
}

/// query params for GET /v1/files/read.
#[derive(Debug, Deserialize)]
pub struct ReadParams {
    pub path: String,
    #[serde(default)]
    pub snapshot: Option<String>,
    #[serde(default)]
    pub offset: Option<u64>,
    #[serde(default)]
    pub len: Option<u64>,
}

/// GET /v1/files/read?path=&offset=&len=&snapshot= — a byte range of a file,
/// base64 in `b64` with `eof` set when the range reached end-of-file. `len` is
/// clamped by the module to its read cap.
pub(crate) async fn files_read(
    State(handle): State<NodeHandle>,
    Query(p): Query<ReadParams>,
) -> Response {
    let q = FilesQuery::Read {
        path: p.path,
        snapshot: p.snapshot,
        offset: p.offset.unwrap_or(0),
        len: p.len.unwrap_or(MAX_READ_BYTES),
    };
    match files_query(&handle, &q).await {
        Ok(FilesReply::Read { b64, eof }) => {
            Json(serde_json::json!({ "b64": b64, "eof": eof })).into_response()
        }
        Ok(_) => wrong_reply(),
        Err(resp) => resp,
    }
}

/// query params for GET /v1/files/find.
#[derive(Debug, Deserialize)]
pub struct FindParams {
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub snapshot: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default)]
    pub limit: Option<u64>,
}

/// GET /v1/files/find?prefix=&after=&limit=&snapshot= — paths under a raw path
/// prefix in full-path order, paged by a `next` cursor.
pub(crate) async fn files_find(
    State(handle): State<NodeHandle>,
    Query(p): Query<FindParams>,
) -> Response {
    let q = FilesQuery::Find {
        prefix: p.prefix.unwrap_or_default(),
        snapshot: p.snapshot,
        after: p.after,
        limit: p.limit.unwrap_or(MAX_PAGE),
    };
    match files_query(&handle, &q).await {
        Ok(FilesReply::Find { entries, next }) => {
            Json(serde_json::json!({ "entries": entries, "next": next })).into_response()
        }
        Ok(_) => wrong_reply(),
        Err(resp) => resp,
    }
}

/// query params for GET /v1/files/grep.
#[derive(Debug, Deserialize)]
pub struct GrepParams {
    pub pattern: String,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub snapshot: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<u64>,
}

/// GET /v1/files/grep?pattern=&prefix=&cursor=&limit=&snapshot= — matching lines
/// under a prefix, each with an evidence uri, paged by a `next` cursor. an empty
/// pattern is a 400 (a module rejection).
pub(crate) async fn files_grep(
    State(handle): State<NodeHandle>,
    Query(p): Query<GrepParams>,
) -> Response {
    let q = FilesQuery::Grep {
        pattern: p.pattern,
        prefix: p.prefix.unwrap_or_default(),
        snapshot: p.snapshot,
        cursor: p.cursor,
        limit: p.limit.unwrap_or(MAX_PAGE),
    };
    match files_query(&handle, &q).await {
        Ok(FilesReply::Grep { hits, next }) => {
            Json(serde_json::json!({ "hits": hits, "next": next })).into_response()
        }
        Ok(_) => wrong_reply(),
        Err(resp) => resp,
    }
}

/// query params for GET /v1/files/history.
#[derive(Debug, Deserialize)]
pub struct HistoryParams {
    #[serde(default)]
    pub limit: Option<u64>,
}

/// GET /v1/files/history?limit= — the bounded commit window, newest-first.
pub(crate) async fn files_history(
    State(handle): State<NodeHandle>,
    Query(p): Query<HistoryParams>,
) -> Response {
    let q = FilesQuery::History {
        limit: p.limit.unwrap_or(MAX_PAGE),
    };
    match files_query(&handle, &q).await {
        Ok(FilesReply::History(snapshots)) => {
            Json(serde_json::json!({ "snapshots": snapshots })).into_response()
        }
        Ok(_) => wrong_reply(),
        Err(resp) => resp,
    }
}

/// GET /v1/files/refs — the committed refs summary: `{head, pins, window_len}`.
/// the checkout/commit engine reads this to resolve the head snapshot and drive
/// per-path CAS against it.
pub(crate) async fn files_refs(State(handle): State<NodeHandle>) -> Response {
    match files_query(&handle, &FilesQuery::Refs {}).await {
        Ok(FilesReply::Refs(info)) => Json(info).into_response(),
        Ok(_) => wrong_reply(),
        Err(resp) => resp,
    }
}

/// query params for GET /v1/files/diff. `from`/`to` are snapshot ids the module
/// resolves against its committed window; `prefix` narrows the walk.
#[derive(Debug, Deserialize)]
pub struct DiffParams {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub prefix: Option<String>,
}

/// GET /v1/files/diff?from=&to=&prefix= — the Added/Removed/Modified leaves
/// between two committed snapshots, as `{"entries": [DiffEntry]}`. the engine's
/// auto-rebase reads this to decide whether upstream touched our paths.
pub(crate) async fn files_diff(
    State(handle): State<NodeHandle>,
    Query(p): Query<DiffParams>,
) -> Response {
    let q = FilesQuery::Diff {
        from: p.from,
        to: p.to,
        prefix: p.prefix.unwrap_or_default(),
    };
    match files_query(&handle, &q).await {
        Ok(FilesReply::Diff(entries)) => {
            Json(serde_json::json!({ "entries": entries })).into_response()
        }
        Ok(_) => wrong_reply(),
        Err(resp) => resp,
    }
}

/// query params for GET /v1/files/has-chunks.
#[derive(Debug, Deserialize)]
pub struct HasChunksParams {
    /// comma-separated 64-hex chunk ids; empty means an empty batch.
    #[serde(default)]
    pub ids: String,
}

/// GET /v1/files/has-chunks?ids=<comma-separated hex> — the client staging probe:
/// `{"present": [bool]}` in request order. an over-cap batch (>256) or a non-hex
/// id passes the module's rejection straight through as a 400 — the engine treats
/// the reply as advisory and the commit re-validates regardless.
pub(crate) async fn files_has_chunks(
    State(handle): State<NodeHandle>,
    Query(p): Query<HasChunksParams>,
) -> Response {
    let ids: Vec<String> = if p.ids.is_empty() {
        Vec::new()
    } else {
        p.ids.split(',').map(|s| s.to_string()).collect()
    };
    match files_query(&handle, &FilesQuery::HasChunks { ids }).await {
        Ok(FilesReply::HasChunks { present }) => {
            Json(serde_json::json!({ "present": present })).into_response()
        }
        Ok(_) => wrong_reply(),
        Err(resp) => resp,
    }
}

// ---- the S3-shaped facade (spec: Revision 2026-07-13) --------------------
//
// one url = one object: PUT is a single-change commit (stage + put), GET
// streams the whole file back, DELETE is a single-change rm. listing is the
// existing /v1/files/ls — a flat, cursor-paged dir page IS the sanctioned
// LIST. same trust story as every handler in this file: the local-trust
// convenience lane (ext:noded); authenticated authorship rides the signed
// frame lane instead. NOTE the lane's actor makes the usable keyspace
// effectively /shared/** — check_authority admits ext:noded nowhere else
// a caller would name.

/// one PUT is buffered whole, then staged in 1 MiB chunks — every byte rides
/// consensus blocks, so this bounds a single request's block fan-out.
/// ponytail: whole-body buffering; stream into staging if objects outgrow RAM.
pub(crate) const MAX_OBJECT_BYTES: usize = 64 * 1024 * 1024;

/// query params for the object facade's PUT.
#[derive(Debug, Deserialize)]
pub struct ObjectPutParams {
    /// optional commit message; defaults to `put <path>`.
    #[serde(default)]
    pub message: Option<String>,
}

/// the wildcard tail arrives without its leading slash; duckfs paths are
/// absolute. (axum already percent-decodes the tail.)
fn object_path(rest: &str) -> String {
    format!("/{rest}")
}

/// the current head snapshot, as a commit base — sequential overwrites are
/// last-writer-wins like S3 (the base is "the head as of this request").
/// CONCURRENT same-path writers diverge from S3: the loser gets the module's
/// per-path CAS rejection (400) where S3 would silently order them — an
/// honest conflict beats a silent overwrite on a consensus filesystem.
async fn head_snapshot(handle: &NodeHandle) -> Result<Option<String>, Response> {
    match files_query(handle, &FilesQuery::Refs {}).await {
        Ok(FilesReply::Refs(info)) => Ok(info.head),
        Ok(_) => Err(wrong_reply()),
        Err(resp) => Err(resp),
    }
}

/// PUT /v1/files/object/{*path}?message= — body is the object's raw bytes.
/// small bodies ride inline in the commit; larger ones stage 1 MiB chunks
/// first (one submit per chunk, exactly like /v1/files/stage) and the commit
/// references the digests. replies with the committing block.
pub(crate) async fn object_put(
    State(handle): State<NodeHandle>,
    axum::extract::Path(rest): axum::extract::Path<String>,
    Query(p): Query<ObjectPutParams>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let bytes = match body {
        Ok(bytes) => bytes,
        Err(rejection) => return error_response(rejection.status(), &rejection.body_text()),
    };
    let path = object_path(&rest);
    let base_snapshot = match head_snapshot(&handle).await {
        Ok(head) => head,
        Err(resp) => return resp,
    };

    use base64::Engine as _;
    let content = if bytes.len() <= duckfs_core::MAX_INLINE_COMMIT_BYTES {
        duckfs_core::Content::Inline {
            b64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        }
    } else {
        let mut chunks = Vec::with_capacity(bytes.len().div_ceil(duckfs_core::CHUNK_SIZE as usize));
        for chunk in bytes.chunks(duckfs_core::CHUNK_SIZE as usize) {
            let digest = to_hex(&object_id(Kind::Chunk, chunk));
            if let Err(resp) = files_submit(&handle, encode_putblob(chunk)).await {
                return resp;
            }
            chunks.push(digest);
        }
        duckfs_core::Content::Chunks {
            size: bytes.len() as u64,
            chunks,
        }
    };

    let payload = encode_msg(&FilesMsg::Commit {
        base_snapshot,
        message: p.message.unwrap_or_else(|| format!("put {path}")),
        changes: vec![Change::Put {
            path,
            exec: false,
            meta: Default::default(),
            content,
        }],
    });
    match files_submit(&handle, payload).await {
        Ok(block) => Json(block).into_response(),
        Err(resp) => resp,
    }
}

/// query params for the object facade's GET.
#[derive(Debug, Deserialize)]
pub struct ObjectGetParams {
    /// read a historical snapshot; omitted reads head.
    #[serde(default)]
    pub snapshot: Option<String>,
}

/// GET /v1/files/object/{*path}?snapshot= — the object's raw bytes, whole
/// (application/octet-stream). pages the module's byte-range Read to eof.
/// an absent path is 404, a directory or symlink 400 (this surface serves
/// FILE bytes, never a link target), and an object over the facade cap is
/// 413 — the whole body is buffered here, symmetric with PUT, so oversized
/// objects belong on the ranged /v1/files/read instead.
pub(crate) async fn object_get(
    State(handle): State<NodeHandle>,
    axum::extract::Path(rest): axum::extract::Path<String>,
    Query(p): Query<ObjectGetParams>,
) -> Response {
    use base64::Engine as _;
    let path = object_path(&rest);
    match files_query(
        &handle,
        &FilesQuery::Stat {
            path: path.clone(),
            snapshot: p.snapshot.clone(),
        },
    )
    .await
    {
        Ok(FilesReply::Stat(None)) => {
            return error_response(StatusCode::NOT_FOUND, "no object at that path");
        }
        Ok(FilesReply::Stat(Some(entry))) => {
            if entry.kind != duckfs_core::EntryKindWire::File {
                return error_response(StatusCode::BAD_REQUEST, "not an object (not a file)");
            }
            if entry.size > MAX_OBJECT_BYTES as u64 {
                return error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "object exceeds the facade cap; read it ranged via /v1/files/read",
                );
            }
        }
        Ok(_) => return wrong_reply(),
        Err(resp) => return resp,
    }
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        let q = FilesQuery::Read {
            path: path.clone(),
            snapshot: p.snapshot.clone(),
            offset: bytes.len() as u64,
            len: MAX_READ_BYTES,
        };
        match files_query(&handle, &q).await {
            Ok(FilesReply::Read { b64, eof }) => {
                match base64::engine::general_purpose::STANDARD.decode(&b64) {
                    Ok(page) => bytes.extend_from_slice(&page),
                    Err(_) => return wrong_reply(),
                }
                if eof {
                    break;
                }
            }
            Ok(_) => return wrong_reply(),
            Err(resp) => return resp,
        }
    }
    (
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        bytes,
    )
        .into_response()
}

/// DELETE /v1/files/object/{*path} — a single-change rm commit. idempotent
/// like S3 for the common repeat-delete: an absent object is a 200 no-op.
/// (the gate is stat-then-commit, so a same-path delete/delete RACE can
/// still surface the module's 400 — narrow, and honest about the conflict.)
pub(crate) async fn object_delete(
    State(handle): State<NodeHandle>,
    axum::extract::Path(rest): axum::extract::Path<String>,
) -> Response {
    let path = object_path(&rest);
    match files_query(
        &handle,
        &FilesQuery::Stat {
            path: path.clone(),
            snapshot: None,
        },
    )
    .await
    {
        Ok(FilesReply::Stat(None)) => {
            return Json(serde_json::json!({ "deleted": false })).into_response();
        }
        Ok(FilesReply::Stat(Some(_))) => {}
        Ok(_) => return wrong_reply(),
        Err(resp) => return resp,
    }
    let base_snapshot = match head_snapshot(&handle).await {
        Ok(head) => head,
        Err(resp) => return resp,
    };
    let payload = encode_msg(&FilesMsg::Commit {
        base_snapshot,
        message: format!("rm {path}"),
        changes: vec![Change::Rm { path }],
    });
    match files_submit(&handle, payload).await {
        Ok(block) => Json(block).into_response(),
        Err(resp) => resp,
    }
}
