use commonware_codec::DecodeExt as _;
use commonware_consensus::simplex::scheme::ed25519 as simplex_ed25519;
use commonware_cryptography::ed25519;
use commonware_runtime::Supervisor;
use commonware_utils::ordered::Set;
use host::Host;
use recovery::{Manifest, Recovery};
use sdk::StateRoot;
use statesync::{SyncError, SyncServer, fetch_frames};

use crate::constants::{CUTOVER_DELAY, MAX_MESSAGE_SIZE};
use crate::util::{fatal, hex};

pub(crate) fn assert_floor_binds_view(
    view_base: u64,
    boundary_height: u64,
    cert_view: u64,
) -> Result<(), String> {
    let certified_height = view_base
        .checked_add(cert_view)
        .ok_or_else(|| format!("floor view {cert_view} overflows view_base {view_base}"))?;
    if certified_height != boundary_height {
        return Err(format!(
            "floor certifies height {certified_height}, not boundary {boundary_height}"
        ));
    }
    Ok(())
}

pub(crate) fn reopen_preflight_synced_host(host: &Host, expected: StateRoot) -> Result<(), String> {
    let live = host.root_hash();
    if live != expected {
        return Err(format!(
            "preflight root_hash {} != boundary {}",
            hex(&live),
            hex(&expected)
        ));
    }
    Ok(())
}

pub(crate) fn verify_manifest_floor(
    namespace: &[u8],
    boundary: &statesync::Manifest,
) -> Result<Option<Vec<u8>>, String> {
    if boundary.height <= boundary.view_base {
        return Ok(None);
    }
    let cert = boundary
        .floor_cert
        .clone()
        .ok_or_else(|| "boundary past its epoch base has no finalization floor".to_string())?;
    let mut keys = Vec::with_capacity(boundary.participants.len());
    for k in &boundary.participants {
        let pk = ed25519::PublicKey::decode(k.as_slice())
            .map_err(|e| format!("served participant set holds a non-ed25519 key: {e}"))?;
        keys.push(pk);
    }
    let participants =
        Set::try_from(keys).map_err(|_| "served participant set has duplicates".to_string())?;
    // a VERIFIER-only scheme (V1 ed25519, the only wired one): no signing key,
    // no our-key-is-a-participant requirement — any node (a not-yet-seated
    // joiner included) can check a served floor. and the check is now
    // CRYPTOGRAPHIC (the quorum's signatures), not the former structural
    // decode: a server cannot mint a floor its quorum never signed.
    let scheme = simplex_ed25519::Scheme::verifier(namespace, participants);
    let finalization = consensus::verify_finalization(&mut rand::rngs::OsRng, &scheme, &cert)
        .map_err(|e| {
            format!(
                "served finalization floor does not verify against the epoch's participant set: {e}"
            )
        })?;
    assert_floor_binds_view(
        boundary.view_base,
        boundary.height,
        finalization.proposal.round.view().get(),
    )
    .map_err(|e| format!("served finalization floor is stale: {e}"))?;
    Ok(Some(cert))
}

/// reopen the recovery journal after a replica DESCEND (the node — which
/// owned the journal as its block sink — was dropped for an epoch cutover or
/// a promotion re-sync). a fresh metrics child label per reopen keeps the
/// runtime's registry collision-free. FATAL on failure: a node that lost its
/// journal handle must not continue as if it had one.
pub(crate) async fn reopen_recovery(
    context: &commonware_runtime::tokio::Context,
    reopens: &mut u32,
    label: &str,
    code_source: std::sync::Arc<dyn host::CodeSource>,
) -> Recovery<commonware_runtime::tokio::Context> {
    *reopens += 1;
    let child: &'static str = Box::leak(format!("recovery_reopen_{reopens}").into_boxed_str());
    match Recovery::open(context.child(child)).await {
        Ok(mut r) => {
            // re-wire the code source: a reopened journal must realize
            // code-registry swaps exactly like the instance it replaces.
            r.set_code_source(code_source);
            r
        }
        Err(e) => {
            fatal!(label, "cannot reopen the recovery store: {e}");
        }
    }
}

/// a backfilled height's served seal, held for the post-fold cross-check:
/// `(disposition, root_hash, per-module roots)` as the quorum sealed them.
pub(crate) type ServedSeal = (
    node::Disposition,
    StateRoot,
    Vec<(sdk::ModuleId, StateRoot)>,
);

/// a failed backfill, split by whether retrying the same range can ever
/// succeed. `permanent` means the SOURCE no longer holds the frames — the
/// range fell below its retention floor while this follower was suspended
/// (a slept laptop's signature shape) — so waiting for the next certificate
/// re-plans the same impossible range forever; the only way forward is a
/// fresh boundary sync, the same jump a rebooted node takes when its reboot
/// gap is pruned.
pub(crate) struct BackfillUnavailable {
    pub(crate) permanent: bool,
    pub(crate) detail: String,
}

/// fold the committed views in `(after_view, up_to_view]` that never reached
/// this replica as certificates — lost gossip, or ancestors committed by
/// descent without their own finalization (the parent-linkage gap the fold
/// planner detected). the frames come from the statesync Frames lane (the
/// validators' journal: the authoritative FOLDED sequence) and enter through
/// the follower gate content-addressed; the served seals are stashed for the
/// post-fold cross-check — a mismatch there is divergence and fatal.
pub(crate) async fn replica_backfill<C>(
    client: &C,
    node_r: &mut node::OrderedNode<
        consensus::FollowerOrderer,
        Recovery<commonware_runtime::tokio::Context>,
    >,
    view_base: u64,
    views: (u64, u64),
    watermark: &mut Option<u64>,
    seal_checks: &mut std::collections::HashMap<u64, ServedSeal>,
    label: &str,
) -> Result<(), BackfillUnavailable>
where
    C: statesync::SyncClient,
{
    let (after_view, up_to_view) = views;
    let frames = fetch_frames(client, view_base + after_view, view_base + up_to_view)
        .await
        .map_err(|e| BackfillUnavailable {
            permanent: matches!(e, SyncError::RangePruned { .. }),
            detail: e.to_string(),
        })?;
    tracing::debug!(
        target: "ducktape::statesync",
        node = %label,
        frames = frames.len(),
        after_view,
        up_to_view,
        "replica backfill"
    );
    for f in frames {
        let view = f.height.saturating_sub(view_base);
        seal_checks.insert(
            f.height,
            (
                to_node_disposition(f.disposition),
                f.root_hash,
                f.roots.clone(),
            ),
        );
        if node_r.orderer_mut().admit_backfilled(view, f.frame.clone()) {
            *watermark = Some(view);
        }
    }
    Ok(())
}

/// the verifier-only scheme for a boundary's epoch: what the replica fold
/// driver checks every observed finalization certificate against. mirrors
/// [`verify_manifest_floor`]'s construction. FATAL on undecodable
/// participants — the boundary already passed the floor verify, so garbage
/// here is our own bug, not the wire's.
pub(crate) fn replica_verifier(
    namespace: &[u8],
    participant_keys: &[Vec<u8>],
) -> simplex_ed25519::Scheme {
    let mut keys = Vec::with_capacity(participant_keys.len());
    for k in participant_keys {
        let pk = ed25519::PublicKey::decode(k.as_slice())
            .expect("participants already decoded for the floor verify");
        keys.push(pk);
    }
    let participants =
        Set::try_from(keys).expect("participant set already deduplicated for the floor verify");
    simplex_ed25519::Scheme::verifier(namespace, participants)
}

/// the replica's valset orchestrator at (epoch, base): the same
/// deterministic observe → ceiling → cutover state machine the validator
/// drain runs. the pending-cutover slot resumes empty — the manifest-epoch
/// descend stays as the safety net for a cutover armed before this handle
/// existed (a restart into a pending window).
pub(crate) fn replica_orchestrator_at(
    epoch: u64,
    view_base: u64,
    participants: &[Vec<u8>],
    residents: &[Vec<u8>],
) -> consensus::ValsetOrchestrator<ed25519::PublicKey> {
    let decode = |keys: &[Vec<u8>]| -> Vec<ed25519::PublicKey> {
        keys.iter()
            .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
            .collect()
    };
    consensus::ValsetOrchestrator::resume(
        CUTOVER_DELAY,
        decode(participants),
        decode(residents),
        epoch,
        view_base,
        None,
    )
}

/// capture and persist the checkpoint (+ floor cert) that makes a synced
/// boundary a valid recovery-boot base — the journal's genesis for an
/// identity that never framed ops on this network (`next_seq = 1`). used by
/// the replica's join-time journal init, and (until the promotion collapse
/// lands) by the promotion path's pre-reboot fabrication. FATALs on
/// persistence failure: a node that cannot journal its base must not proceed
/// as if it had.
pub(crate) async fn write_boundary_checkpoint<E>(
    recovery: &mut Recovery<E>,
    host: &Host,
    boundary: &statesync::Manifest,
    floor: &Option<recovery::FloorCert>,
    label: &str,
    diag_tag: &str,
) -> u64
where
    E: recovery::Context + commonware_runtime::BufferPooler + commonware_runtime::Supervisor,
{
    let pos = recovery.oplog_pos().await;
    let floor_height = floor
        .as_ref()
        .map(|floor| floor.height.to_string())
        .unwrap_or_else(|| "none".to_string());
    tracing::debug!(
        target: "ducktape::statesync",
        tag = %diag_tag,
        checkpoint_height = boundary.height,
        checkpoint_hash = %hex(&host.root_hash()),
        floor_height = %floor_height,
        floor_present = floor.is_some(),
        "checkpoint captured"
    );
    let ckpt = match Manifest::capture(
        host,
        Some(boundary.height),
        boundary.epoch,
        boundary.view_base,
        boundary.participants.clone(),
        boundary.residents.clone(),
        None,
        pos,
        1,
    ) {
        Ok(m) => m,
        Err(e) => {
            fatal!(label, "{diag_tag} capture: {e}");
        }
    };
    if let Err(e) = recovery.write_manifest(&ckpt).await {
        fatal!(label, "{diag_tag} write: {e}");
    }
    if let Some(fc) = floor
        && let Err(e) = recovery.write_floor_cert(fc).await
    {
        fatal!(label, "{diag_tag} floor-cert write: {e}");
    }
    // this checkpoint IS the journal's new genesis: everything below its
    // oplog position must never roll into a boot at this base — a prior
    // life's replica-folded frames sit at earlier POSITIONS even when their
    // heights exceed the boundary, and recovery would roll a trailing one
    // forward past the checkpoint (observed: a promoted ex-replica booting
    // AHEAD of its source's serving window). the engine floor at `boundary`
    // suppresses replay at or below it, so no pruned frame is needed again.
    if let Err(e) = recovery.prune_oplog(pos).await {
        fatal!(label, "{diag_tag} journal prune: {e}");
    }
    // the checkpoint's oplog position — the caller's prune anchor when the
    // NEXT (periodic) checkpoint supersedes this one.
    pos
}

/// how long after the last served state-sync request the source keeps
/// deferring oplog pruning (sliding — every request renews it). generous vs
/// the 32-block checkpoint cadence so one slow module fetch cannot lose the
/// race; bounded so a dead syncer cannot wedge retention forever. this is the
/// anti-treadmill: without it a busy chain prunes a slow syncer's boundary out
/// from under it on every attempt, and a rebootstrapping replica can NEVER
/// converge (observed: boundary 297→318→340→… forever).
pub(crate) const SYNC_LEASE_SECS: u64 = 60;

pub(crate) fn unix_now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(crate) fn sync_lease_active(lease: &std::sync::atomic::AtomicU64) -> bool {
    let last_served = lease.load(std::sync::atomic::Ordering::Relaxed);
    unix_now_secs().saturating_sub(last_served) < SYNC_LEASE_SECS
}

pub(crate) fn to_node_disposition(disposition: statesync::FrameDisposition) -> node::Disposition {
    match disposition {
        statesync::FrameDisposition::Applied => node::Disposition::Applied,
        statesync::FrameDisposition::Rejected => node::Disposition::Rejected,
    }
}

pub(crate) fn to_sync_disposition(
    disposition: node::Disposition,
) -> Result<statesync::FrameDisposition, String> {
    match disposition {
        node::Disposition::Applied => Ok(statesync::FrameDisposition::Applied),
        node::Disposition::Rejected => Ok(statesync::FrameDisposition::Rejected),
        node::Disposition::Discarded => Err("discarded frames are not recovery-journaled".into()),
    }
}

pub(crate) fn recovery_frame_to_sync(
    frame: recovery::JournalFrame,
) -> Result<statesync::FinalizedFrame, String> {
    Ok(statesync::FinalizedFrame {
        height: frame.height,
        frame: frame.frame,
        disposition: to_sync_disposition(frame.disposition)?,
        roots: frame.roots,
        root_hash: frame.root_hash,
    })
}

// ---------------------------------------------------------------------------
// the statesync serve seam: serving runs on its OWN task (captures, leases,
// chunk slicing, mesh/plane replies), so a joiner's sync never rides a drain
// beat of the consensus loop. only the four STATE TOUCHES below cross back to
// the loop — the one task that owns the host, the recovery journal, and the
// derived index — as bounded request/reply pairs, so a busy loop backpressures
// the serve lane instead of the reverse.
// ---------------------------------------------------------------------------

/// one state touch the statesync serve task asks of the consensus loop.
pub(crate) enum SyncStateRequest {
    /// capture (or re-coordinate) the current finalized boundary — the
    /// Manifest path. `known` names the boundaries the serve task already
    /// holds, so a known id round-trips fresh coordinates only, never
    /// payload bytes.
    Boundary {
        known: Vec<statesync::BoundaryId>,
        reply: tokio::sync::oneshot::Sender<Result<SyncBoundary, String>>,
    },
    /// route module-defined bytes to the live module's `serve_sync` (the
    /// resolver lanes: qmdb op ranges, duckfs refs/objects).
    ModuleServe {
        module_id: String,
        body: Vec<u8>,
        reply: tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>,
    },
    /// read recovery-equivalent finalized frames in `(after, up_to]`.
    Frames {
        after_height: u64,
        up_to_height: u64,
        reply: tokio::sync::oneshot::Sender<Result<Vec<recovery::JournalFrame>, recovery::Error>>,
    },
    /// read one page of a module's derived index op rows at or below
    /// `up_to_height` — the joiner's backfill lane.
    IndexOps {
        module: String,
        after: Option<(u64, u32)>,
        up_to_height: u64,
        reply: tokio::sync::oneshot::Sender<Result<SyncIndexOps, String>>,
    },
    /// read the tip's consensus coordinates — the DETECTION lane: answered
    /// straight from loop-owned state (no capture, no lease, no floor-cert
    /// alignment gate), so a resident fleet's routine polling never rides
    /// the Manifest path.
    TipCoords {
        reply: tokio::sync::oneshot::Sender<Result<statesync::TipCoords, String>>,
    },
    /// the fail-closed standing check (ADR §5.1): is `requester` in committed
    /// standing (validators ∪ residents)? answered from the loop's own
    /// committed host reads, FRESH per request — a just-committed Redeem grant
    /// is seen immediately (a cached snapshot would starve a fresh resident
    /// between its Redeem block and the later transport cutover).
    Standing {
        requester: [u8; 32],
        reply: tokio::sync::oneshot::Sender<bool>,
    },
}

/// the [`SyncStateRequest::Boundary`] answer: the served boundary's identity
/// and coordinates, with capture payload only when the serve task named the
/// id unknown.
pub(crate) struct SyncBoundary {
    pub(crate) id: statesync::BoundaryId,
    pub(crate) coords: statesync::BoundaryCoords,
    pub(crate) data: Option<statesync::CaptureData>,
}

/// the [`SyncStateRequest::IndexOps`] answer: one page of stored op rows in
/// key order, plus the two facts that let a joiner compose an honest floor —
/// this node's own backfill floor for the module and its feed watermark.
pub(crate) struct SyncIndexOps {
    pub(crate) rows: Vec<(String, Vec<u8>)>,
    /// more rows exist past the last one served.
    pub(crate) has_more: bool,
    pub(crate) source_floor: Option<u64>,
    pub(crate) applied_height: u64,
}

const MAX_SYNC_RESPONSE_BODY_LEN: usize =
    MAX_MESSAGE_SIZE as usize - statesync::RPC_HEADER_LEN;
const _: () = assert!(MAX_SYNC_RESPONSE_BODY_LEN >= 9);

/// Return the largest non-empty prefix that fits the mesh's configured message
/// cap. An available frame that cannot fit alone is an explicit error: an empty
/// successful batch would make suffix catch-up retry without advancing.
fn bounded_frames_response(mut frames: Vec<statesync::FinalizedFrame>) -> statesync::SyncResponse {
    frames.truncate(statesync::FRAME_BATCH_LEN);
    if frames.is_empty() {
        return statesync::SyncResponse::Frames { frames };
    }

    let mut fitting = 0usize;
    let mut excluded = frames.len() + 1;
    while excluded - fitting > 1 {
        let candidate = fitting + (excluded - fitting) / 2;
        let encoded_len = statesync::encoded_frames_response_len(&frames[..candidate]);
        let fits_transport = encoded_len <= MAX_SYNC_RESPONSE_BODY_LEN;
        if fits_transport {
            fitting = candidate;
        } else {
            excluded = candidate;
        }
    }

    if fitting == 0 {
        return statesync::SyncResponse::Error(format!(
            "finalized frame at height {} exceeds the {MAX_MESSAGE_SIZE}-byte statesync mesh message limit",
            frames[0].height
        ));
    }
    frames.truncate(fitting);
    statesync::SyncResponse::Frames { frames }
}

/// how many wire pages ONE consensus-loop touch reads for the joiner backfill
/// lane. every page a joiner walks otherwise costs a full round trip through
/// the loop — the one task that owns the derived index — so a long history
/// paces itself against consensus work, a page per turn.
///
/// CEILING: two, because [`indexer::MAX_SCAN_LIMIT`] bounds one snapshot scan
/// at 1024 rows and a wire page is 512 of them. A deeper budget needs a read
/// LOOP on the state owner (several snapshots per touch) and parks
/// proportionally more of a walking joiner's history in the serve task's
/// read-ahead — raise it there, deliberately, if the round trips ever measure.
pub(crate) const INDEX_OPS_LOOP_PAGES: usize =
    indexer::MAX_SCAN_LIMIT / statesync::INDEX_OPS_BATCH_LEN;
const _: () = assert!(INDEX_OPS_LOOP_PAGES >= 1);

/// how many op rows one consensus-loop touch reads: the budget above, in rows.
pub(crate) const INDEX_OPS_LOOP_ROWS: usize = INDEX_OPS_LOOP_PAGES * statesync::INDEX_OPS_BATCH_LEN;

/// the serve task's READ-AHEAD for the joiner backfill lane: the rows one
/// consensus-loop touch read past the wire page it answered, held for the next
/// request of the same walk.
///
/// ONE SLOT, not a map: a walk is strictly sequential — a page, then the next
/// from the cursor that page handed out — so the slot belongs to whichever
/// joiner is walking now. A second concurrent walk misses it and touches the
/// loop, exactly as every page did before, and a request that does not match
/// what the slot holds drops it: a walk that stops carries nothing.
#[derive(Default)]
pub(crate) struct IndexOpsPager {
    held: Option<ReadAhead>,
}

/// rows already read, and the exact ask they answer.
struct ReadAhead {
    module: String,
    up_to_height: u64,
    /// the cursor the next request must carry for these rows to be its answer.
    after: (u64, u32),
    page: SyncIndexOps,
}

impl IndexOpsPager {
    /// the rows this ask is owed, when the last loop touch already read them.
    fn take(
        &mut self,
        module: &str,
        up_to_height: u64,
        after: Option<(u64, u32)>,
    ) -> Option<SyncIndexOps> {
        let held = self.held.take()?;
        let same_walk =
            held.module == module && held.up_to_height == up_to_height && after == Some(held.after);
        same_walk.then_some(held.page)
    }

    /// hold what the wire page could not carry, keyed by the cursor that page
    /// just handed the joiner. a page whose cursor is absent ends the walk, so
    /// there is nobody to hand the rest to.
    fn keep(
        &mut self,
        module: &str,
        up_to_height: u64,
        after: Option<(u64, u32)>,
        page: SyncIndexOps,
    ) {
        self.held = after.map(|after| ReadAhead {
            module: module.to_string(),
            up_to_height,
            after,
            page,
        });
    }
}

/// Same binary search as [`bounded_frames_response`], over index op rows: keep
/// the largest non-empty prefix that fits the mesh's message cap, and set the
/// cursor whenever anything was left behind. A single row that cannot fit alone
/// is an explicit error — an empty successful page with `next_after` set would
/// make the joiner's walk spin without advancing.
///
/// The rows past that prefix are the loop touch's READ-AHEAD ([`IndexOpsPager`]),
/// returned rather than dropped: the next request of the same walk answers from
/// them without crossing to the consensus loop again.
pub(crate) fn split_index_ops_response(
    page: SyncIndexOps,
) -> (statesync::SyncResponse, Option<SyncIndexOps>) {
    let SyncIndexOps {
        mut rows,
        has_more,
        source_floor,
        applied_height,
    } = page;
    // the wire caps a page; everything past it is read-ahead, never dropped.
    let mut rest = if rows.len() > statesync::INDEX_OPS_BATCH_LEN {
        rows.split_off(statesync::INDEX_OPS_BATCH_LEN)
    } else {
        Vec::new()
    };

    let mut fitting = 0usize;
    let mut excluded = rows.len() + 1;
    while excluded - fitting > 1 {
        let candidate = fitting + (excluded - fitting) / 2;
        let encoded_len = statesync::encoded_index_ops_response_len(&rows[..candidate]);
        let fits_transport = encoded_len <= MAX_SYNC_RESPONSE_BODY_LEN;
        if fits_transport {
            fitting = candidate;
        } else {
            excluded = candidate;
        }
    }

    if fitting == 0 && !rows.is_empty() {
        return (
            statesync::SyncResponse::Error(format!(
                "index op row {} exceeds the {MAX_MESSAGE_SIZE}-byte statesync mesh message limit",
                rows[0].0
            )),
            None,
        );
    }
    // what the byte cap trimmed belongs in FRONT of what the page cap did:
    // read-ahead stays in key order, which is the order the walk resumes in.
    rest.splice(0..0, rows.drain(fitting..));
    let carried = !rest.is_empty();
    let next_after = (has_more || carried)
        .then(|| {
            rows.last()
                .and_then(|(key, _)| indexer::parse_op_key(key.as_bytes()))
        })
        .flatten();
    let read_ahead = carried.then_some(SyncIndexOps {
        rows: rest,
        has_more,
        source_floor,
        applied_height,
    });
    (
        statesync::SyncResponse::IndexOps {
            rows,
            next_after,
            source_floor,
            applied_height,
        },
        read_ahead,
    )
}

/// drive one decoded statesync request against the serve-task-owned
/// [`SyncServer`], round-tripping the state touches to the consensus loop.
/// a closed loop (shutdown) answers as a plain serve error — clients retry
/// against the next source.
pub(crate) async fn drive_sync_request(
    server: &mut SyncServer,
    pager: &mut IndexOpsPager,
    state_tx: &futures::channel::mpsc::Sender<SyncStateRequest>,
    req: statesync::SyncRequest,
) -> statesync::SyncResponse {
    const CLOSED: &str = "statesync state owner is shutting down";
    // a failed send drops the request (and its reply sender) on the floor, so
    // the paired `rx.await` below surfaces it as the CLOSED error — no
    // separate delivered/undelivered bookkeeping.
    let ask = |req: SyncStateRequest| {
        let mut tx = state_tx.clone();
        async move {
            let _ = futures::SinkExt::send(&mut tx, req).await;
        }
    };
    match server.serve(req) {
        statesync::ServeStep::Reply(resp) => resp,
        statesync::ServeStep::NeedBoundary => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            ask(SyncStateRequest::Boundary {
                known: server.known_boundaries(),
                reply: tx,
            })
            .await;
            match rx.await {
                Ok(Ok(SyncBoundary { id, coords, data })) => {
                    match data {
                        Some(data) => server.install_capture(id, data),
                        None => server.refresh_coords(id, coords),
                    }
                    server
                        .manifest_for(id)
                        .unwrap_or_else(statesync::SyncResponse::Error)
                }
                Ok(Err(e)) => statesync::SyncResponse::Error(e),
                Err(_) => statesync::SyncResponse::Error(CLOSED.into()),
            }
        }
        statesync::ServeStep::NeedModuleServe { module_id, body } => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            ask(SyncStateRequest::ModuleServe {
                module_id,
                body,
                reply: tx,
            })
            .await;
            match rx.await {
                Ok(Ok(bytes)) => statesync::SyncResponse::Module(bytes),
                Ok(Err(e)) => statesync::SyncResponse::Error(e),
                Err(_) => statesync::SyncResponse::Error(CLOSED.into()),
            }
        }
        statesync::ServeStep::NeedFrames {
            after_height,
            up_to_height,
        } => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            ask(SyncStateRequest::Frames {
                after_height,
                up_to_height,
                reply: tx,
            })
            .await;
            match rx.await {
                Ok(Ok(frames)) => match frames
                    .into_iter()
                    .take(statesync::FRAME_BATCH_LEN)
                    .map(recovery_frame_to_sync)
                    .collect::<Result<Vec<_>, _>>()
                {
                    Ok(frames) => bounded_frames_response(frames),
                    Err(e) => statesync::SyncResponse::Error(e),
                },
                Ok(Err(recovery::Error::RangePruned {
                    after_height,
                    retained_start,
                })) => {
                    // THE known wedge, and neither side logged it. `checkpoint_blocks`
                    // defaults to 32 and prune trails one checkpoint, so at a 1s block
                    // the retention window is 32-64 SECONDS — a slow bridge or a laptop
                    // wake is outrun, and no node anywhere recorded what the floor was
                    // or who got refused. this is the line that answers "was my follower
                    // too slow, or was the source pruning too aggressively" — the exact
                    // question that ate the 07-14 live-join session.
                    tracing::warn!(
                        target: "ducktape::statesync",
                        requested_after = after_height,
                        retained_from = retained_start,
                        gap_blocks = retained_start.saturating_sub(after_height),
                        reason = "pruned_below_retention_floor",
                        "frame range REFUSED — the requester is below this node's retention \
                         floor and can never catch up from here (it must full-sync; raise \
                         `checkpoint_blocks` if this recurs)"
                    );
                    statesync::SyncResponse::RangePruned {
                        requested_after: after_height,
                        retained_from: retained_start,
                    }
                }
                Ok(Err(e)) => statesync::SyncResponse::Error(format!("recovery frame range: {e}")),
                Err(_) => statesync::SyncResponse::Error(CLOSED.into()),
            }
        }
        statesync::ServeStep::NeedIndexOps {
            boundary,
            module,
            after,
        } => {
            // the loop reads a BUDGET of pages at a time; this ask is either
            // the one that reads, or the one that spends what the last read.
            let read = match pager.take(&module, boundary, after) {
                Some(held) => Ok(held),
                None => {
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    ask(SyncStateRequest::IndexOps {
                        module: module.clone(),
                        after,
                        up_to_height: boundary,
                        reply: tx,
                    })
                    .await;
                    match rx.await {
                        Ok(read) => read.map_err(statesync::SyncResponse::Error),
                        Err(_) => Err(statesync::SyncResponse::Error(CLOSED.into())),
                    }
                }
            };
            match read {
                Ok(page) => {
                    let (response, read_ahead) = split_index_ops_response(page);
                    if let Some(rest) = read_ahead {
                        let statesync::SyncResponse::IndexOps { next_after, .. } = &response else {
                            unreachable!("a split page answers on the index-op lane");
                        };
                        pager.keep(&module, boundary, *next_after, rest);
                    }
                    response
                }
                Err(refusal) => refusal,
            }
        }
        statesync::ServeStep::NeedCoords => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            ask(SyncStateRequest::TipCoords { reply: tx }).await;
            match rx.await {
                Ok(Ok(coords)) => statesync::SyncResponse::TipCoords(coords),
                Ok(Err(e)) => statesync::SyncResponse::Error(e),
                Err(_) => statesync::SyncResponse::Error(CLOSED.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(height: u64, payload_len: usize) -> statesync::FinalizedFrame {
        statesync::FinalizedFrame {
            height,
            frame: vec![0xAB; payload_len],
            disposition: statesync::FrameDisposition::Applied,
            roots: Vec::new(),
            root_hash: StateRoot([height as u8; 32]),
        }
    }

    fn encoded_mesh_len(resp: &statesync::SyncResponse) -> usize {
        let body = statesync::encode_response(resp);
        statesync::encode_rpc(&[0; 32], &[0; 64], 7, &body).len()
    }

    /// N WIRE PAGES MUST NOT COST N CONSENSUS-LOOP ROUND TRIPS. The joiner
    /// backfill lane reads through the loop — the one task that owns the
    /// derived index — so a joiner walking a long history paced itself
    /// against consensus work at one page per round trip. One touch now reads
    /// a whole page BUDGET and the serve task hands the rest out itself, so a
    /// walk of N pages costs ceil(N / budget) touches and serves the same rows
    /// in the same order.
    #[tokio::test]
    async fn a_backfill_walk_touches_the_loop_once_per_page_budget() {
        use futures::StreamExt;
        use std::sync::atomic::{AtomicUsize, Ordering};

        const PAGES: usize = 3;
        let dir = tempfile::tempdir().expect("index dir");
        let index = std::sync::Arc::new(
            indexer::IndexStore::open(dir.path(), &[indexer::IndexModule::bare("chat")])
                .expect("open index"),
        );
        let seeded = (PAGES * statesync::INDEX_OPS_BATCH_LEN) as u64;
        let rows: Vec<(String, Vec<u8>)> = (1..=seeded)
            .map(|height| (indexer::op_key(height, 0), vec![height as u8; 8]))
            .collect();
        index
            .write_backfill_rows("chat", &rows)
            .expect("seed the feed");

        // the consensus loop's side of the seam: answers the state touch off
        // the store it owns, and counts every one it is asked for.
        let (state_tx, mut state_rx) = futures::channel::mpsc::channel::<SyncStateRequest>(8);
        let touches = std::sync::Arc::new(AtomicUsize::new(0));
        let loop_side = {
            let index = index.clone();
            let touches = touches.clone();
            tokio::spawn(async move {
                while let Some(req) = state_rx.next().await {
                    let SyncStateRequest::IndexOps {
                        module,
                        after,
                        up_to_height,
                        reply,
                    } = req
                    else {
                        unreachable!("only the index-op lane is asked here");
                    };
                    touches.fetch_add(1, Ordering::Relaxed);
                    let _ = reply.send(crate::validator::run::sync::read_index_ops(
                        &index,
                        &module,
                        after,
                        up_to_height,
                    ));
                }
            })
        };

        let mut server = SyncServer::new();
        let mut pager = IndexOpsPager::default();
        let mut after = None;
        let mut pages = 0usize;
        let mut served: Vec<(u64, u32)> = Vec::new();
        loop {
            let resp = drive_sync_request(
                &mut server,
                &mut pager,
                &state_tx,
                statesync::SyncRequest::IndexOps {
                    boundary: u64::MAX,
                    module: "chat".into(),
                    after,
                },
            )
            .await;
            let statesync::SyncResponse::IndexOps {
                rows, next_after, ..
            } = resp
            else {
                panic!("the index-op lane answers its own response");
            };
            pages += 1;
            served.extend(
                rows.iter()
                    .filter_map(|(key, _)| indexer::parse_op_key(key.as_bytes())),
            );
            match next_after {
                Some(next) => after = Some(next),
                None => break,
            }
        }
        drop(state_tx);
        loop_side
            .await
            .expect("the loop side ends with its channel");

        assert_eq!(pages, PAGES, "the walk pages at the wire cap");
        assert_eq!(
            served,
            (1..=seeded).map(|height| (height, 0)).collect::<Vec<_>>(),
            "every row crosses exactly once, in key order"
        );
        assert_eq!(
            touches.load(Ordering::Relaxed),
            PAGES.div_ceil(INDEX_OPS_LOOP_PAGES),
            "one loop touch per page budget, not one per wire page"
        );
    }

    #[test]
    fn frames_response_splits_the_observed_oversized_batch() {
        let original = (1..=statesync::FRAME_BATCH_LEN)
            .map(|height| {
                let payload_len = if height < statesync::FRAME_BATCH_LEN {
                    40_554
                } else {
                    40_553
                };
                frame(height as u64, payload_len)
            })
            .collect::<Vec<_>>();
        let original_response = statesync::SyncResponse::Frames {
            frames: original.clone(),
        };
        assert_eq!(encoded_mesh_len(&original_response), 2_599_216);
        assert!(encoded_mesh_len(&original_response) > MAX_MESSAGE_SIZE as usize);

        let bounded = bounded_frames_response(original.clone());
        let statesync::SyncResponse::Frames { frames } = bounded else {
            panic!("a fitting prefix must be served");
        };
        assert!(!frames.is_empty(), "the client must make height progress");
        assert!(frames.len() < statesync::FRAME_BATCH_LEN);
        assert!(
            encoded_mesh_len(&statesync::SyncResponse::Frames {
                frames: frames.clone(),
            }) <= MAX_MESSAGE_SIZE as usize
        );

        let mut one_more = frames;
        one_more.push(original[one_more.len()].clone());
        assert!(
            encoded_mesh_len(&statesync::SyncResponse::Frames { frames: one_more })
                > MAX_MESSAGE_SIZE as usize,
            "the selected prefix is maximal"
        );
    }

    #[test]
    fn frames_response_accepts_exact_limit_and_rejects_one_byte_over() {
        let fixed_len = statesync::RPC_HEADER_LEN
            + statesync::encoded_frames_response_len(&[frame(1, 0)]);
        let exact_payload_len = MAX_MESSAGE_SIZE as usize - fixed_len;

        let exact = bounded_frames_response(vec![frame(1, exact_payload_len)]);
        let statesync::SyncResponse::Frames { frames } = &exact else {
            panic!("a frame at the exact transport limit must fit");
        };
        assert_eq!(frames.len(), 1);
        assert_eq!(encoded_mesh_len(&exact), MAX_MESSAGE_SIZE as usize);

        let oversized = bounded_frames_response(vec![frame(2, exact_payload_len + 1)]);
        let statesync::SyncResponse::Error(message) = &oversized else {
            panic!("a single oversized frame must fail closed");
        };
        assert!(message.contains("exceeds"));
        assert!(
            encoded_mesh_len(&oversized) <= MAX_MESSAGE_SIZE as usize,
            "the fail-closed error must itself fit the transport"
        );
    }
}
