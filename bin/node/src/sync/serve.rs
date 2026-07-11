use commonware_codec::DecodeExt as _;
use commonware_consensus::simplex::scheme::ed25519 as simplex_ed25519;
use commonware_cryptography::ed25519;
use commonware_runtime::Supervisor;
use commonware_utils::ordered::Set;
use host::Host;
use recovery::{Manifest, Recovery};
use sdk::StateRoot;
use statesync::{SyncServer, fetch_frames};

use crate::constants::CUTOVER_DELAY;
use crate::host_reads::read_upgrade_version_fields;
use crate::util::{diag_log, hex};

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
    let live = host.app_hash();
    if live != expected {
        return Err(format!(
            "preflight app_hash {} != boundary {}",
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
) -> Recovery<commonware_runtime::tokio::Context> {
    *reopens += 1;
    let child: &'static str = Box::leak(format!("recovery_reopen_{reopens}").into_boxed_str());
    match Recovery::open(context.child(child)).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[node {label}] FATAL: cannot reopen the recovery store: {e}");
            std::process::exit(1);
        }
    }
}

/// a backfilled height's served seal, held for the post-fold cross-check:
/// `(disposition, app_hash, per-module roots)` as the quorum sealed them.
pub(crate) type ServedSeal = (
    node::Disposition,
    StateRoot,
    Vec<(sdk::ModuleId, StateRoot)>,
);

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
) -> Result<(), String>
where
    C: statesync::SyncClient,
{
    let (after_view, up_to_view) = views;
    let frames = fetch_frames(client, view_base + after_view, view_base + up_to_view)
        .await
        .map_err(|e| format!("{e}"))?;
    println!(
        "[node {label}] replica: backfilling {} committed frame(s) in views ({after_view}, \
         {up_to_view}]",
        frames.len()
    );
    for f in frames {
        let view = f.height.saturating_sub(view_base);
        seal_checks.insert(
            f.height,
            (
                to_node_disposition(f.disposition),
                f.app_hash,
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
pub(crate) fn replica_verifier(namespace: &[u8], participant_keys: &[Vec<u8>]) -> simplex_ed25519::Scheme {
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
    diag_log(format!(
        "DIAG {diag_tag} checkpoint_height={} checkpoint_hash={} \
         floor_height={} floor_present={}",
        boundary.height,
        hex(&host.app_hash()),
        floor_height,
        floor.is_some()
    ));
    // stamp the real committed version fields so the captured checkpoint
    // carries the same `required_min_version` a live checkpoint would; the
    // next boot then preflights against them like any restart.
    let (cv, pu) = read_upgrade_version_fields(host).await;
    let ckpt = match Manifest::capture(
        host,
        Some(boundary.height),
        boundary.epoch,
        boundary.view_base,
        boundary.participants.clone(),
        boundary.residents.clone(),
        None,
        cv,
        pu,
        pos,
        1,
    ) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[node {label}] FATAL: {diag_tag} capture: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = recovery.write_manifest(&ckpt).await {
        eprintln!("[node {label}] FATAL: {diag_tag} write: {e}");
        std::process::exit(1);
    }
    if let Some(fc) = floor
        && let Err(e) = recovery.write_floor_cert(fc).await
    {
        eprintln!("[node {label}] FATAL: {diag_tag} floor-cert write: {e}");
        std::process::exit(1);
    }
    // this checkpoint IS the journal's new genesis: everything below its
    // oplog position must never roll into a boot at this base — a prior
    // life's replica-folded frames sit at earlier POSITIONS even when their
    // heights exceed the boundary, and recovery would roll a trailing one
    // forward past the checkpoint (observed: a promoted ex-replica booting
    // AHEAD of its source's serving window). the engine floor at `boundary`
    // suppresses replay at or below it, so no pruned frame is needed again.
    if let Err(e) = recovery.prune_oplog(pos).await {
        eprintln!("[node {label}] FATAL: {diag_tag} journal prune: {e}");
        std::process::exit(1);
    }
    // the checkpoint's oplog position — the caller's prune anchor when the
    // NEXT (periodic) checkpoint supersedes this one.
    pos
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
        app_hash: frame.app_hash,
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
    /// checkpoint the derived index databases for the shipped-index lane.
    IndexCut {
        reply: tokio::sync::oneshot::Sender<std::collections::BTreeMap<String, Vec<u8>>>,
    },
    /// read the tip's consensus coordinates — the DETECTION lane: answered
    /// straight from loop-owned state (no capture, no lease, no floor-cert
    /// alignment gate), so a resident fleet's routine polling never rides
    /// the Manifest path.
    TipCoords {
        reply: tokio::sync::oneshot::Sender<Result<statesync::TipCoords, String>>,
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

/// drive one decoded statesync request against the serve-task-owned
/// [`SyncServer`], round-tripping the state touches to the consensus loop.
/// a closed loop (shutdown) answers as a plain serve error — clients retry
/// against the next source.
pub(crate) async fn drive_sync_request(
    server: &mut SyncServer,
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
                Ok(Ok(frames)) => {
                    let mut out = Vec::new();
                    let mut err = None;
                    for frame in frames.into_iter().take(statesync::FRAME_BATCH_LEN) {
                        match recovery_frame_to_sync(frame) {
                            Ok(frame) => out.push(frame),
                            Err(e) => {
                                err = Some(e);
                                break;
                            }
                        }
                    }
                    match err {
                        Some(e) => statesync::SyncResponse::Error(e),
                        None => statesync::SyncResponse::Frames { frames: out },
                    }
                }
                Ok(Err(recovery::Error::RangePruned {
                    after_height,
                    retained_start,
                })) => statesync::SyncResponse::RangePruned {
                    requested_after: after_height,
                    retained_from: retained_start,
                },
                Ok(Err(e)) => statesync::SyncResponse::Error(format!("recovery frame range: {e}")),
                Err(_) => statesync::SyncResponse::Error(CLOSED.into()),
            }
        }
        statesync::ServeStep::NeedIndexCut { boundary } => {
            // the shipped-index lane cuts lazily: the FIRST index request for
            // a boundary checkpoints the derived databases and attaches the
            // archives to that capture, so joiners that never opt in cost
            // nothing. the attach is unconditional, so the re-drive below
            // resolves — it cannot need a second cut.
            let (tx, rx) = tokio::sync::oneshot::channel();
            ask(SyncStateRequest::IndexCut { reply: tx }).await;
            let blobs = match rx.await {
                Ok(blobs) => blobs,
                Err(_) => return statesync::SyncResponse::Error(CLOSED.into()),
            };
            if let Err(e) = server.attach_index(boundary, blobs) {
                return statesync::SyncResponse::Error(e);
            }
            match server.serve(statesync::SyncRequest::IndexModules { boundary }) {
                statesync::ServeStep::Reply(resp) => resp,
                _ => {
                    statesync::SyncResponse::Error("index attach did not settle the request".into())
                }
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
