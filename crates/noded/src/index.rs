//! the derived-index tier: shared store construction (each module's index
//! guest installed from the network's genesis, or from a founding set for a
//! daemon that runs no network), boundary stamping, and the `/v1/index/*` +
//! `/v1/blocks` snapshot read lane.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::{NodeHandle, error_response, hex_bytes};

/// how many recent blocks `GET /v1/blocks` serves when the caller names no
/// `limit` — a bounded default page over the durable block index.
const BLOCKS_DEFAULT_LIMIT: usize = 256;

// ---------------------------------------------------------------------------
// the derived-index tier: shared construction. noded and the consensus
// validator (bin/node) both reuse this router, so they share the store setup
// too — one construction site, identical /v1/index/* behavior, each binary
// passing its own genesis module list.
// ---------------------------------------------------------------------------

/// the index guests a node installs: id → mapper bytes for every module that
/// ships one. Each module's fluentabi mapper is built by `guest-builder
/// --index` and committed beside its crate (`make wasm-modules` refreshes the
/// set); the build stages it into the founding set as `<id>.index.wasm` iff
/// the crate declares the guest (`src/index_guest.rs`), and a network's
/// genesis carries the set its founder built, so every node on that network
/// folds with the same mapper. the artifact's presence IS the declaration:
/// a module with no `<id>.index.wasm` ships no guest, and a database with
/// no guest serves a bare feed.
pub struct IndexGuests(BTreeMap<String, Vec<u8>>);

impl IndexGuests {
    /// the guests a founding set holds for `module_ids` — the daemons that
    /// run no network (noded, simnode, the dev shape) install from here. an
    /// absent file is a module that ships no guest; any other read failure
    /// names its path.
    pub fn from_dir<S: AsRef<str>>(dir: &std::path::Path, module_ids: &[S]) -> Result<Self, String> {
        let mut guests = BTreeMap::new();
        for id in module_ids {
            let id = id.as_ref();
            let path = workspace_config::index_guest_path(dir, id);
            match std::fs::read(&path) {
                Ok(bytes) => {
                    guests.insert(id.to_string(), bytes);
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(format!(
                        "module {id}: {} is unreadable: {err} \
                         (run `make wasm-modules`, or `cargo build` to stage the founding set)",
                        path.display()
                    ));
                }
            }
        }
        Ok(Self(guests))
    }

    /// the guests a genesis carries — a node on a network installs from its
    /// workspace genesis, never from a directory.
    pub fn from_genesis(genesis: &workspace_config::Genesis) -> Self {
        Self(
            genesis
                .index_guests
                .iter()
                .map(|a| (a.id.clone(), a.bytes.clone()))
                .collect(),
        )
    }

    fn get(&self, id: &str) -> Option<&[u8]> {
        self.0.get(id).map(Vec::as_slice)
    }
}

/// open the per-module index store under `<storage>/index`, deciding nothing
/// about guests yet: each database keeps serving the guest its last converge
/// installed until [`converge_index_guests`] runs. The split is what lets a
/// node bring its index routes up before it holds a genesis (a joiner
/// fetches its genesis off the mesh after the surfaces start). an open
/// failure is fatal-with-remedy for the caller: the tier is rebuildable, so
/// the fix is always "delete the directory".
pub fn open_index_store<S: AsRef<str>>(
    storage: &std::path::Path,
    module_ids: &[S],
) -> Result<Arc<indexer::IndexStore>, String> {
    let index_dir = storage.join("index");
    let ids: Vec<&str> = module_ids.iter().map(AsRef::as_ref).collect();
    indexer::IndexStore::open_bare(&index_dir, &ids)
        .map(Arc::new)
        .map_err(|err| {
            format!(
                "open module index at {}: {err} (derived tier — delete the directory to rebuild)",
                index_dir.display()
            )
        })
}

/// converge every module's database onto its guest in `guests` (or onto no
/// guest, for a module that ships none): the install half of
/// [`open_index_store`], run once the guests are known.
pub fn converge_index_guests(
    index: &indexer::IndexStore,
    guests: &IndexGuests,
) -> Result<(), String> {
    let ids = index.module_ids();
    let modules: Vec<indexer::IndexModule> = ids
        .iter()
        .map(|id| indexer::IndexModule {
            id,
            guest: guests.get(id),
        })
        .collect();
    index.converge(&modules).map_err(|err| {
        format!(
            "install index guests at {}: {err} (derived tier — delete the directory to rebuild)",
            index.base().display()
        )
    })
}

/// the index covers every module the host runs: open a database for each id
/// of `modules` the store lacks — a module the modules registry admitted
/// after the store opened — leaving the ones it holds untouched. idempotent,
/// and free when nothing was admitted. called after every compose and before
/// every block fold, so an admitted module's feed begins at the block that
/// seated it. a database opened here holds no guest (an admission ships
/// none through the genesis) and serves a bare feed.
pub fn index_host_modules<'a>(
    index: &indexer::IndexStore,
    modules: impl IntoIterator<Item = &'a str>,
) -> Result<(), String> {
    for id in modules {
        index.open_module(id).map_err(|err| {
            format!(
                "open index database for module {id} at {}: {err} \
                 (derived tier — delete the directory to rebuild)",
                index.base().display()
            )
        })?;
    }
    Ok(())
}

/// flatten a dispatch origin into the index's plain origin tag: external
/// submitter identities render via [`indexer::user_handle`] — printable claimed
/// names pass through, raw ed25519 pubkeys become hex (never the lossy `�`
/// boxes a plain utf-8 decode leaves). the feed carries origins pre-rendered,
/// so index guests never see raw key bytes. hex-keyed identity belongs to the
/// explorer row's `proposer`, not the index op rows.
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
                assigned: d.assigned.clone(),
            })
            .collect(),
        record: None,
    }
}

/// stamp every module whose watermark trails `boundary` as backfilled: its
/// feed and views honestly BEGIN at the boundary, visibly via the floor
/// (`/v1/index/status` reports it). call wherever canonical state advanced
/// without the op stream — after state-sync installs a boundary, after
/// recovery skipped re-executing durable blocks, or over a wiped index
/// directory. history below a boundary re-enters only by replaying blocks
/// through the feed, or by the joiner's op-row backfill pulling the source's
/// stored rows in below the stamp (indexable spec §7). returns the stamped ids.
pub fn stamp_stale_modules(
    index: &indexer::IndexStore,
    boundary: u64,
) -> Result<Vec<String>, indexer::Error> {
    let stale = stale_modules(index, boundary)?;
    for module in &stale {
        index.mark_backfilled(module, boundary)?;
    }
    Ok(stale)
}

/// every module whose op feed trails `boundary` — the stamp's candidates,
/// listed WITHOUT stamping them. a caller that can pull the missing rows
/// decides per module whether the feed is resumable (extend it and keep the
/// views under it) or has to be stamped and rebuilt from the boundary down.
pub fn stale_modules(
    index: &indexer::IndexStore,
    boundary: u64,
) -> Result<Vec<String>, indexer::Error> {
    let modules = index.module_ids();
    let mut stale = Vec::new();
    for module in modules {
        if index.applied_height(&module)? >= boundary {
            continue;
        }
        stale.push(module);
    }
    Ok(stale)
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
    /// op rows projected to json (height/seq/time/origin + payload). the
    /// stored envelope is borsh (the guest feed); this lane re-presents it
    /// in the row shape the tier always served: a payload that is valid
    /// json embeds verbatim as `payload`, anything else ships as
    /// `payload_hex` — the codebase's hex-not-base64 convention.
    ops: Vec<serde_json::Value>,
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_after: Option<String>,
}

/// one borsh op row as this lane's json projection.
fn op_row_json(row: &indexer::OpRow) -> serde_json::Value {
    let mut out = serde_json::json!({
        "height": row.height,
        "seq": row.seq,
        "time": row.time,
        "origin": row.origin,
    });
    let payload_json: Option<serde_json::Value> = serde_json::from_slice(&row.payload).ok();
    match payload_json {
        Some(value) => out["payload"] = value,
        None => out["payload_hex"] = serde_json::Value::String(hex_bytes(&row.payload)),
    }
    if !row.assigned.is_empty() {
        let assigned_json: Option<serde_json::Value> = serde_json::from_slice(&row.assigned).ok();
        match assigned_json {
            Some(value) => out["assigned"] = value,
            None => out["assigned_hex"] = serde_json::Value::String(hex_bytes(&row.assigned)),
        }
    }
    out
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

/// GET /v1/index/status — each module's applied watermark (the op FEED, not
/// the derived view), the poison flag, backfill floors, and every fold
/// trigger's health (`pending` backlog + last drain error): the watermark
/// vouches for the feed, the fold trails it observably. a poisoned index
/// keeps serving (stale but consistent) reads; the remedy is a rebuild,
/// which this surface makes visible. boundary-stamped modules also report
/// their backfill floor: content below it was never in the feed — the gap
/// stays visible instead of papered over.
pub(crate) async fn index_status(State(handle): State<NodeHandle>) -> Response {
    let Some(store) = index_store(&handle) else {
        return no_index_store_response();
    };
    let mut modules = serde_json::Map::new();
    let mut backfilled = serde_json::Map::new();
    let mut fold = serde_json::Map::new();
    for id in store.module_ids() {
        match store.applied_height(&id) {
            Ok(height) => {
                modules.insert(id.to_string(), height.into());
            }
            Err(err) => return index_error(err),
        }
        match store.backfill_height(&id) {
            Ok(Some(floor)) => {
                backfilled.insert(id.to_string(), floor.into());
            }
            Ok(None) => {}
            Err(err) => return index_error(err),
        }
        match store.fold_status(&id) {
            Ok(Some(status)) => match serde_json::to_value(&status) {
                Ok(value) => {
                    fold.insert(id.to_string(), value);
                }
                Err(_) => {
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "fold status did not serialize",
                    );
                }
            },
            Ok(None) => {}
            Err(err) => return index_error(err),
        }
    }
    Json(serde_json::json!({
        "poisoned": store.is_poisoned(),
        "modules": modules,
        "backfilled": backfilled,
        "fold": fold,
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
        match borsh::from_slice::<indexer::OpRow>(value) {
            Ok(row) => ops.push(op_row_json(&row)),
            // rows are written as borsh by construction; failing one means the
            // store is damaged — say so instead of silently dropping it.
            Err(_) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "stored op row was not a borsh envelope — rebuild the index",
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

/// the fold watermark a view reply carries: `"{height}:{seq}"`, the op row the
/// module's fold had consumed when the reply was served. ABSENT when the
/// module has no tip (fresh database, boundary stamp, a guest reinstalled
/// without a refold) — absent means unknown, never zero.
///
/// a HEADER, not an envelope: the module reply enums are the modules' own wire
/// and stay untouched, and a caller that does not care never sees it.
pub const FOLDED_HEADER: &str = "x-ducktape-folded";

/// how many `POST /v1/index/{module}/view` calls may run their wasm query
/// concurrently. The route is `Lane::Open` — any caller that can dial the
/// HTTP port reaches it, no PoP or workspace secret required — and each call
/// runs fluent31's `Db::query` SYNCHRONOUSLY against ~1e9 fuel
/// (`fluent31::Options::wasm_fuel`, the only per-call budget fluent31
/// exposes: no separate wall-clock/epoch deadline exists to set alongside
/// it). `index_view` now runs the fold-tip-then-view pair on
/// [`tokio::task::spawn_blocking`]'s own pool so it can no longer pin an
/// axum worker outright, but that pool is still this same process's CPU —
/// unbounded fan-in there would let N unauthenticated callers burn every
/// core the process has, including bin/node's consensus thread. Must stay
/// small: a value near or above the runtime's worker-thread count buys
/// nothing over no cap at all.
pub(crate) const MAX_CONCURRENT_INDEX_VIEWS: usize = 4;

/// the refusal body when the concurrency gate above is already full — a
/// stable, greppable token, not prose, so an operator can tell "the node is
/// out of index-view slots" apart from every other 429 on this surface.
const INDEX_VIEW_AT_CAPACITY: &str = "index view refused: reason=index_view_at_capacity";

/// POST /v1/index/{module}/view — the module's materialized view, served by
/// its registered mapper. request body and reply are module-defined json
/// (chat: `{"search": {…}}` → `{"hits": […]}`), exactly as opaque to the
/// daemon as `/v1/query` payloads are. modules with no view answer 404 —
/// some never will (forge's substrate is already a queryable git repo).
///
/// the reply carries [`FOLDED_HEADER`]: how far the fold had consumed the op
/// feed, so a caller that just wrote can tell whether this snapshot contains
/// its own op. it answers read-after-YOUR-OWN-WRITE and nothing else — the
/// fold advances only on op traffic, so a quiet module's tip is arbitrarily
/// old while its view is perfectly current.
pub(crate) async fn index_view(
    State(handle): State<NodeHandle>,
    Path(module): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Response {
    let Some(store) = index_store(&handle).cloned() else {
        return no_index_store_response();
    };
    // `try_acquire_owned` refuses immediately once the gate is full — an
    // unauthenticated caller's Nth request never queues behind the first
    // `MAX_CONCURRENT_INDEX_VIEWS`, it just 429s.
    let Ok(_permit) = handle.index_view_gate.clone().try_acquire_owned() else {
        return error_response(StatusCode::TOO_MANY_REQUESTS, INDEX_VIEW_AT_CAPACITY);
    };
    let req_bytes = serde_json::to_vec(&req).expect("a decoded json value re-serializes");
    let query_module = module.clone();
    // BEFORE the view, deliberately: the two reads take two MVCC snapshots, and
    // the order decides which way the mismatch falls. read first and the tip
    // can only be OLDER than the rows served — the caller waits one more round
    // trip. read after and it can be NEWER, claiming a row this reply does not
    // contain, which is the exact bug the tip exists to close.
    //
    // and ADVISORY, never a precondition: a read error here degrades to "no
    // header", the same answer an unstamped module gives. everything about
    // this key is best-effort (absent = unknown, a malformed value = unknown),
    // and `IndexStore::view` promises a poisoned store still serves views —
    // stale but consistent. failing the whole read because one bookkeeping key
    // would not load empties the caller's sidebar over nothing. an unknown
    // module still 404s: `store.view` below answers that.
    //
    // both reads run off the axum worker on `spawn_blocking`'s pool: fluent31's
    // `query` is synchronous wasm with no `.await` inside it, so awaiting it
    // directly here would hold this task's worker thread for the whole call.
    let outcome = tokio::task::spawn_blocking(move || {
        let folded = store.fold_tip(&query_module).ok().flatten();
        let view = store.view(&query_module, &req_bytes);
        (folded, view)
    })
    .await;
    let (folded, view) = match outcome {
        Ok(pair) => pair,
        Err(_) => {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "index view task panicked");
        }
    };
    let mut response = match view {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) => Json(value).into_response(),
            Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR, "view reply was not json"),
        },
        Err(err) => index_error(err),
    };
    if let Some((height, seq)) = folded {
        // infallible on purpose: ABSENT is the one honest way to say "no tip"
        // (§3.2.4), so a header dropped for any other reason would read to the
        // caller as an unstamped module. two integers and a colon are visible
        // ascii, which is exactly what a header value may hold.
        let watermark = HeaderValue::from_str(&format!("{height}:{seq}"))
            .expect("a numeric watermark is a valid header value");
        response.headers_mut().insert(FOLDED_HEADER, watermark);
    }
    response
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

/// GET /v1/blocks — the recent rows this node kept, oldest-first:
/// `{"blocks":[…]}`.
///
/// reads the index store's durable blocks database directly (no actor
/// round-trip), the same discipline as the other `/v1/index/*` reads — so
/// history survives a restart. heartbeat nops never get a row, so an empty
/// reply means no real ops have finalized, not an idle chain. a handle with
/// no index store configured serves the same "no blocks yet" shape.
///
/// NOT uniformly non-empty, and a reader that assumes so is wrong: the block
/// writers drop an op-less block, but [`indexer::IndexStore::apply_block_record`]
/// exists for a follower that observes BOUNDARIES rather than sealed frames,
/// and `bin/node`'s `boundary_block_row` writes one such row (empty `hash`,
/// empty `ops`) at each ascension tip. it is a truthful record of what that
/// node saw; a consumer that presents these rows as blocks-with-content owes
/// its own filter.
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

#[cfg(test)]
mod tests {
    /// THE WATERMARK MUST BE READ BEFORE THE VIEW. The two reads take two MVCC
    /// snapshots, and the ORDER is the whole correctness argument: read the tip
    /// first and it can only be older than the rows served (the caller waits
    /// one more round trip — safe); read it after and it can be newer, vouching
    /// for a row this very reply does not contain — which is exactly the
    /// acceptance-vs-application bug the header exists to close.
    ///
    /// Pinned as a source shape because no behavioural test can see it: both
    /// orders answer identically except in the interleaving that makes the
    /// wrong one wrong.
    #[test]
    fn the_view_reads_its_fold_watermark_before_the_snapshot() {
        const SRC: &str = include_str!("index.rs");
        let body = SRC
            .split("pub(crate) async fn index_view(")
            .nth(1)
            .expect("index_view is declared")
            .split("\n/// ")
            .next()
            .expect("index_view body");
        let tip = body
            .find("store.fold_tip(")
            .expect("the view reads the tip");
        let view = body.find("store.view(").expect("the view serves the view");
        assert!(
            tip < view,
            "the fold watermark must be read BEFORE the view snapshot"
        );
        // AND IT MUST NOT BE ABLE TO REFUSE THE READ. The tip is advisory —
        // absent is unknown, malformed is unknown — so an engine error on it
        // has to degrade to "no header" too. Turning it into a precondition
        // would answer 500 to a request that could have served the rows fine,
        // against `IndexStore::view`'s own promise that a poisoned store still
        // serves views. Nothing between the two reads may leave the function.
        assert!(
            !body[tip..view].contains("return"),
            "an unreadable fold watermark must not refuse the view"
        );
    }

    /// #1717: once `MAX_CONCURRENT_INDEX_VIEWS` callers are already "running"
    /// (holding a permit, as the wasm query would while it runs), the NEXT
    /// `index_view` call refuses with 429 rather than queuing behind them —
    /// this is the whole point of `try_acquire_owned` over `acquire_owned`.
    #[tokio::test]
    async fn index_view_refuses_once_the_concurrency_gate_is_full() {
        let dir = tempfile::TempDir::new().expect("temp index dir");
        let modules = vec![indexer::IndexModule::bare("chat")];
        let store = std::sync::Arc::new(
            indexer::IndexStore::open(dir.path(), &modules).expect("open index"),
        );
        let (handle, _cmds, _hub) = crate::NodeHandle::channel();
        let handle = handle.with_index_store(store);

        // saturate the gate exactly as N concurrent in-flight views would.
        let held: Vec<_> = (0..super::MAX_CONCURRENT_INDEX_VIEWS)
            .map(|_| {
                handle
                    .index_view_gate
                    .clone()
                    .try_acquire_owned()
                    .expect("gate starts with MAX_CONCURRENT_INDEX_VIEWS permits")
            })
            .collect();

        let refused = super::index_view(
            axum::extract::State(handle.clone()),
            axum::extract::Path("chat".to_string()),
            axum::Json(serde_json::json!({})),
        )
        .await;
        assert_eq!(refused.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);

        // releasing a permit reopens the gate for the next caller.
        drop(held);
        let admitted = super::index_view(
            axum::extract::State(handle),
            axum::extract::Path("chat".to_string()),
            axum::Json(serde_json::json!({})),
        )
        .await;
        assert_ne!(admitted.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
    }
}
