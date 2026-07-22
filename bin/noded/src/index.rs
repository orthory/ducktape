//! the derived-index tier: shared store construction, from-state rebuilds,
//! and the `/v1/index/*` + `/v1/blocks` snapshot read lane.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::{NodeHandle, error_response, hex_bytes};

/// how many recent blocks `GET /v1/blocks` serves when the caller names no
/// `limit` — a bounded default page over the durable block index.
const BLOCKS_DEFAULT_LIMIT: usize = 256;

// ---------------------------------------------------------------------------
// the derived-index tier: shared construction + from-state rebuild triggers.
// noded and the consensus validator (bin/node) both reuse this router, so
// they share the store setup too — one construction site, identical
// /v1/index/* behavior, each binary passing its own genesis module list.
// ---------------------------------------------------------------------------

/// open the per-module index store under `<storage>/index` with every view
/// mapper registered. an open failure is fatal-with-remedy for the caller:
/// the tier is rebuildable, so the fix is always "delete the directory".
pub fn open_index_store<S: AsRef<str>>(
    storage: &std::path::Path,
    module_ids: &[S],
) -> Result<Arc<indexer::IndexStore>, String> {
    let index_dir = storage.join("index");
    indexer::IndexStore::open(&index_dir, module_ids)
        .map(|store| {
            Arc::new(
                store
                    .with_indexer(Box::new(chat::index::ChatIndex::new("chat")))
                    .with_indexer(Box::new(tasks::index::TasksIndex::new("tasks")))
                    .with_indexer(Box::new(pages::index::PagesIndex::new("pages")))
                    .with_indexer(Box::new(saga::index::UsageIndex::new("saga"))),
            )
        })
        .map_err(|err| {
            format!(
                "open module index at {}: {err} (derived tier — delete the directory to rebuild)",
                index_dir.display()
            )
        })
}

/// flatten a dispatch origin into the index's plain origin tag: external
/// submitter identities render via [`indexer::user_handle`] — printable claimed
/// names pass through, raw ed25519 pubkeys become hex (never the lossy `�`
/// boxes a plain utf-8 decode leaves). the same convention drives a mapper's
/// from-state rebuild (see `chat::index` `author_from_ref`), so folded and
/// rebuilt rows match byte-for-byte on BOTH lanes. hex-keyed identity belongs
/// to the explorer row's `proposer`, not the index op rows.
pub fn index_origin(origin: &sdk::Origin) -> indexer::OriginTag {
    match origin {
        sdk::Origin::External(id) => indexer::OriginTag::external(indexer::user_handle(id)),
        sdk::Origin::Module(id) => indexer::OriginTag::module(id.clone()),
        sdk::Origin::System => indexer::OriginTag::system(),
    }
}

/// one sealed block's dispatch trace as the indexer's fold input. `time` is
/// the block's consensus time — noded passes its submit context's clock, the
/// consensus validator stamps `consensus_time = height`. an empty trace is a
/// real block (a rejected frame consumed its height): folding it advances
/// every module's watermark so staleness checks stay exact.
///
/// `record` starts [`None`]: a caller holding a block the explorer shows
/// grafts its [`block_row`] on via struct update. the live drain builds it
/// from the decoded frame; the validator's boot folds (journal replay, frame
/// catch-up) rebuild the SAME row from the sealed frame bytes riding the
/// replay observer — the row is not reproducible from the dispatch trace
/// alone, so a fold without frame content leaves it `None`.
pub fn index_block_ops(
    height: u64,
    time: u64,
    dispatches: &[host::DispatchRecord],
) -> indexer::BlockOps {
    indexer::BlockOps {
        height,
        time,
        ops: dispatches
            .iter()
            .map(|d| indexer::AppliedOp {
                origin: index_origin(&d.origin),
                module: d.module.clone(),
                payload: d.payload.clone(),
            })
            .collect(),
        record: None,
    }
}

/// one module's canonical state as the indexer's [`indexer::StateReader`]:
/// [`host::Host::query`] adapted onto the bytes-in/bytes-out rebuild surface,
/// module errors mapped into [`indexer::Error::State`].
struct HostStateReader<'a> {
    host: &'a host::Host,
    module: &'a str,
}

#[async_trait::async_trait(?Send)]
impl indexer::StateReader for HostStateReader<'_> {
    async fn query(&self, req: &[u8]) -> indexer::Result<Vec<u8>> {
        self.host
            .query(self.module, req)
            .await
            .map_err(|err| indexer::Error::State(err.to_string()))
    }
}

/// heal every module whose watermark trails `boundary` from VERIFIED
/// canonical state: a mapper that declares a from-state rebuild re-derives
/// its read model; a module without one is stamped backfilled, its content
/// visibly beginning at the boundary. call wherever canonical state advanced
/// without the op stream — after state-sync installs a boundary, after
/// recovery skipped re-executing durable blocks, or over a wiped index
/// directory. returns `(module, rows)` per re-derived view.
pub async fn rebuild_stale_modules(
    index: &indexer::IndexStore,
    host: &host::Host,
    boundary: indexer::RebuildMeta,
) -> Result<Vec<(String, u64)>, indexer::Error> {
    let modules: Vec<String> = index.module_ids().map(str::to_string).collect();
    let mut rebuilt = Vec::new();
    for module in modules {
        if index.applied_height(&module)? >= boundary.height {
            continue;
        }
        let state = HostStateReader {
            host,
            module: &module,
        };
        match index.rebuild_module(&module, &state, boundary).await {
            Ok(rows) => rebuilt.push((module, rows)),
            Err(indexer::Error::RebuildUnsupported) => index.mark_backfilled(&module, boundary)?,
            Err(err) => return Err(err),
        }
    }
    Ok(rebuilt)
}

// ---------------------------------------------------------------------------
// the derived-index read lane. like the blob lane these
// handlers never cross the actor: the store is `Send + Sync` and every read
// runs at its own MVCC snapshot, concurrent with the actor's block writes.
// ---------------------------------------------------------------------------

/// query params for `GET /v1/index/{module}/scan` and `…/ops`.
#[derive(Debug, Deserialize)]
pub struct IndexScanParams {
    /// key prefix to scan under. ignored by `…/ops` (pinned to the op log).
    pub prefix: Option<String>,
    /// opaque page cursor: the `next_after` of the previous page.
    pub after: Option<String>,
    /// page size; the store clamps oversized asks.
    pub limit: Option<usize>,
}

/// default page size when a client sends no `limit`.
const INDEX_DEFAULT_LIMIT: usize = 100;

/// one scanned entry. values written by this tier are json (`value`); a
/// derived value that is not valid json ships as `value_hex` instead.
#[derive(Serialize)]
struct IndexEntry {
    key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Box<serde_json::value::RawValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value_hex: Option<String>,
}

#[derive(Serialize)]
struct IndexScanResponse {
    entries: Vec<IndexEntry>,
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_after: Option<String>,
}

#[derive(Serialize)]
struct IndexOpsResponse {
    /// stored op-row envelopes, verbatim (height/seq/time/origin + payload).
    ops: Vec<Box<serde_json::value::RawValue>>,
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_after: Option<String>,
}

fn index_store(handle: &NodeHandle) -> Option<&Arc<indexer::IndexStore>> {
    handle.index.as_ref()
}

fn no_index_store_response() -> Response {
    error_response(StatusCode::SERVICE_UNAVAILABLE, "no index store configured")
}

fn index_error(err: indexer::Error) -> Response {
    let status = match err {
        indexer::Error::UnknownModule(_) | indexer::Error::ViewUnsupported => StatusCode::NOT_FOUND,
        indexer::Error::View(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    error_response(status, &err.to_string())
}

/// GET /v1/index/status — each module's applied watermark plus the poison
/// flag. a poisoned index keeps serving (stale but consistent) reads; the
/// remedy is a rebuild, which this surface makes visible. modules re-derived
/// from canonical state also report their backfill floor: content below it
/// was never folded from ops (heights are boundary-stamped, the op log
/// starts above it) — the gap stays visible instead of papered over.
pub(crate) async fn index_status(State(handle): State<NodeHandle>) -> Response {
    let Some(store) = index_store(&handle) else {
        return no_index_store_response();
    };
    let mut modules = serde_json::Map::new();
    let mut backfilled = serde_json::Map::new();
    for id in store.module_ids() {
        match store.applied_height(id) {
            Ok(height) => {
                modules.insert(id.to_string(), height.into());
            }
            Err(err) => return index_error(err),
        }
        match store.backfill_height(id) {
            Ok(Some(floor)) => {
                backfilled.insert(id.to_string(), floor.into());
            }
            Ok(None) => {}
            Err(err) => return index_error(err),
        }
    }
    Json(serde_json::json!({
        "poisoned": store.is_poisoned(),
        "modules": modules,
        "backfilled": backfilled,
    }))
    .into_response()
}

/// GET /v1/index/{module}/ops?after=&limit= — one page of the module's op
/// log, oldest-first. rows are the stored envelopes verbatim; page forward by
/// echoing `next_after` as the next call's `after`.
pub(crate) async fn index_ops(
    State(handle): State<NodeHandle>,
    Path(module): Path<String>,
    Query(params): Query<IndexScanParams>,
) -> Response {
    let Some(store) = index_store(&handle) else {
        return no_index_store_response();
    };
    let page = match store.scan(
        &module,
        indexer::OP_PREFIX.as_bytes(),
        params.after.as_deref().map(str::as_bytes),
        params.limit.unwrap_or(INDEX_DEFAULT_LIMIT),
    ) {
        Ok(page) => page,
        Err(err) => return index_error(err),
    };
    let mut ops = Vec::with_capacity(page.entries.len());
    for (_key, value) in &page.entries {
        match serde_json::from_slice(value) {
            Ok(row) => ops.push(row),
            // rows are written as json by construction; failing one means the
            // store is damaged — say so instead of silently dropping it.
            Err(_) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "stored op row was not json — rebuild the index",
                );
            }
        }
    }
    Json(IndexOpsResponse {
        ops,
        has_more: page.has_more,
        next_after: page.next_after,
    })
    .into_response()
}

/// POST /v1/index/{module}/view — the module's materialized view, served by
/// its registered mapper. request body and reply are module-defined json
/// (chat: `{"search": {…}}` → `{"hits": […]}`), exactly as opaque to the
/// daemon as `/v1/query` payloads are. modules with no view answer 404 —
/// some never will (forge's substrate is already a queryable git repo).
pub(crate) async fn index_view(
    State(handle): State<NodeHandle>,
    Path(module): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Response {
    let Some(store) = index_store(&handle) else {
        return no_index_store_response();
    };
    let req_bytes = serde_json::to_vec(&req).expect("a decoded json value re-serializes");
    match store.view(&module, &req_bytes) {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) => Json(value).into_response(),
            Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR, "view reply was not json"),
        },
        Err(err) => index_error(err),
    }
}

/// GET /v1/index/{module}/scan?prefix=&after=&limit= — one page of raw index
/// keys, for a module's derived read models (everything a registered
/// `ModuleIndexer` materialized outside the reserved op/meta spaces).
pub(crate) async fn index_scan(
    State(handle): State<NodeHandle>,
    Path(module): Path<String>,
    Query(params): Query<IndexScanParams>,
) -> Response {
    let Some(store) = index_store(&handle) else {
        return no_index_store_response();
    };
    let prefix = params.prefix.unwrap_or_default();
    let page = match store.scan(
        &module,
        prefix.as_bytes(),
        params.after.as_deref().map(str::as_bytes),
        params.limit.unwrap_or(INDEX_DEFAULT_LIMIT),
    ) {
        Ok(page) => page,
        Err(err) => return index_error(err),
    };
    let entries = page
        .entries
        .iter()
        .map(|(key, value)| {
            let json: Option<Box<serde_json::value::RawValue>> = serde_json::from_slice(value).ok();
            IndexEntry {
                key: String::from_utf8_lossy(key).into_owned(),
                value_hex: json.is_none().then(|| hex_bytes(value)),
                value: json,
            }
        })
        .collect();
    Json(IndexScanResponse {
        entries,
        has_more: page.has_more,
        next_after: page.next_after,
    })
    .into_response()
}

/// query params for `GET /v1/blocks`.
#[derive(Debug, Deserialize)]
pub struct BlocksParams {
    /// cap the response to the most recent N blocks (default:
    /// [`BLOCKS_DEFAULT_LIMIT`]).
    pub limit: Option<usize>,
}

/// GET /v1/blocks — recent non-empty blocks, oldest-first: `{"blocks":[…]}`.
///
/// reads the index store's durable blocks database directly (no actor
/// round-trip), the same discipline as the other `/v1/index/*` reads — so
/// history survives a restart. heartbeat nops never get a row, so an empty
/// reply means no real ops have finalized, not an idle chain. a handle with
/// no index store configured serves the same "no blocks yet" shape.
pub(crate) async fn blocks(
    State(handle): State<NodeHandle>,
    Query(params): Query<BlocksParams>,
) -> Response {
    let Some(store) = handle.index.as_ref() else {
        return Json(serde_json::json!({ "blocks": [] })).into_response();
    };
    let rows = match store.recent_block_rows(params.limit.unwrap_or(BLOCKS_DEFAULT_LIMIT)) {
        Ok(rows) => rows,
        Err(err) => return index_error(err),
    };
    let mut blocks: Vec<Box<serde_json::value::RawValue>> = Vec::with_capacity(rows.len());
    for row in &rows {
        match serde_json::from_slice(row) {
            Ok(block) => blocks.push(block),
            // rows are written as json by construction; failing one means the
            // store is damaged — say so instead of silently dropping it.
            Err(_) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "stored block row was not json — rebuild the index",
                );
            }
        }
    }
    Json(serde_json::json!({ "blocks": blocks })).into_response()
}
