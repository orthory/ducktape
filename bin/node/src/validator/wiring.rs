//! Validator mesh wiring.
//!
//! All discovery channels are registered before the one legal
//! `network.start()` call; post-catch-up ingress bridges then hand bounded
//! local receivers to the consensus pump.

use std::sync::Arc;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::authenticated::discovery::{self, Network};
use commonware_p2p::{Ingress, Manager, Receiver as P2pReceiver, Recipients, Sender as P2pSender};
use commonware_runtime::{IoBuf, Quota, Spawner, Supervisor};
use commonware_utils::ordered::Set;

use host::Host;

use crate::blob_fetch;
use crate::config;
use crate::constants::*;
use crate::explorer::heal_index;
use crate::host_reads::{read_valset_residents, resume_member_keys};
use crate::lobby;
use crate::reachability_plane::{GateHook, GateOutcomes, wire_reachability_plane};
use crate::sync::catchup::derive_pending_boot;
use crate::sync::serve::{SyncStateRequest, drive_sync_request};
use crate::{voice, voice_plane};
use futures::StreamExt as _;
use statesync::SyncServer;

pub(super) struct PreWiring {
    pub(super) initial_member_keys: Vec<ed25519::PublicKey>,
    pub(super) initial_resident_keys: Vec<ed25519::PublicKey>,
    pub(super) mesh_oracle: discovery::Oracle<ed25519::PublicKey>,
    pub(super) bank_base: u64,
    pub(super) channel_bank: super::LaneBank,
    pub(super) sync_tx: super::MeshSender,
    pub(super) sync_rx: super::MeshReceiver,
    pub(super) relay_tx: super::MeshSender,
    pub(super) relay_rx: super::MeshReceiver,
    pub(super) media_peers: Option<Arc<voice_plane::MediaPeers>>,
    pub(super) reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>>,
    /// the join GATE's loop end (join ADR §4): forwarded requests arrive here…
    pub(super) gate_fwd_rx: tokio::sync::mpsc::Receiver<lobby::GateForward>,
    /// …kept open by this never-sending clone even when no plane was wired…
    pub(super) gate_fwd_keepalive: tokio::sync::mpsc::Sender<lobby::GateForward>,
    /// …and settled outcomes go back through this shared map.
    pub(super) gate_outcomes: GateOutcomes,
}

pub(super) struct RuntimeWiring {
    pub(super) member_keys: Vec<ed25519::PublicKey>,
    pub(super) participants: Set<ed25519::PublicKey>,
    pub(super) resume_epoch: u64,
    pub(super) pending_boot: Option<u64>,
    pub(super) mesh_oracle: discovery::Oracle<ed25519::PublicKey>,
    pub(super) channel_bank: super::LaneBank,
    pub(super) gateway_book: Option<Arc<crate::gateway_plane::OverlayBook>>,
    pub(super) blob_peers: Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>>,
    pub(super) blob_client: blob_fetch::ServeLaneBlobClient<super::MeshSender>,
    pub(super) sync_state_rx:
        futures::channel::mpsc::Receiver<crate::sync::serve::SyncStateRequest>,
    /// unix seconds of the last served state-sync request — the drain reads it
    /// to defer oplog pruning while a syncer is actively pulling (the sync
    /// retention lease, see sync/serve.rs).
    pub(super) sync_lease: Arc<std::sync::atomic::AtomicU64>,
    pub(super) relay_ingress: futures::channel::mpsc::Receiver<(ed25519::PublicKey, Vec<u8>)>,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn finish(
    context: &commonware_runtime::tokio::Context,
    index: &indexer::IndexStore,
    resumed: Option<&recovery::Recovered>,
    recovery_manifest_for_resume: Option<&recovery::Manifest>,
    boot_fold: crate::explorer::IndexFold<'_>,
    validators: &[ed25519::PublicKey],
    signer: ed25519::PrivateKey,
    label: String,
    peers: Vec<ed25519::PublicKey>,
    namespace: Vec<u8>,
    overlay_enabled: bool,
    overlay_slot: overlay_net::userspace::StackSlot,
    bulk_pacer: data_plane::BulkPacer,
    planes: data_plane::PlaneMonitor,
    sync_monitor: statesync::monitor::ServeMonitor,
    gateway_requests: Option<tokio::sync::mpsc::Receiver<noded::GatewayJob>>,
    gateway_commands: futures::channel::mpsc::Sender<noded::NodeCommand>,
    gateway_workspace: std::path::PathBuf,
    blobs: noded::blobs::BlobHandle,
    initial_member_keys: Vec<ed25519::PublicKey>,
    initial_resident_keys: Vec<ed25519::PublicKey>,
    mut mesh_oracle: discovery::Oracle<ed25519::PublicKey>,
    bank_base: u64,
    mut channel_bank: super::LaneBank,
    sync_tx: super::MeshSender,
    sync_rx: super::MeshReceiver,
    relay_rx: super::MeshReceiver,
) -> RuntimeWiring {
    // the FINAL index heal, at the boot tip every path converged on:
    // whatever the replay/catch-up fold could not reproduce (opaque
    // blocks, a state-sync jump, a stopped fold) re-derives here from
    // state that has verified against the boundary root-hash.
    drop(boot_fold);
    if let Some(boot_height) = resumed.as_ref().and_then(|r| r.height) {
        heal_index(index, boot_height, &label);
    }

    let member_keys = match resume_member_keys(resumed, validators) {
        Ok(keys) => keys,
        Err(e) => {
            tracing::error!(
                target: "ducktape::node",
                node = %label,
                error = %e,
                "FATAL: recovered validator set is invalid"
            );
            std::process::exit(1);
        }
    };
    if !member_keys.contains(&signer.public_key()) {
        tracing::info!(
            target: "ducktape::node",
            node = %label,
            reason = "not_in_recovered_validator_set",
            "halting; restart with --sync-only to observe"
        );
        std::process::exit(0);
    }
    let participants: Set<ed25519::PublicKey> =
        Set::try_from(member_keys.clone()).expect("valset membership has no duplicates");
    let resume_epoch = resumed.as_ref().map(|r| r.epoch).unwrap_or(0);
    mesh_oracle.track(
        resume_epoch,
        super::wiring::mesh_at(&peers, &member_keys.iter().cloned().collect()),
    );
    if !channel_bank.covers(resume_epoch) {
        tracing::error!(
            target: "ducktape::node",
            node = %label,
            epoch = resume_epoch,
            bank_base,
            bank_end = bank_base + EPOCH_CHANNEL_BANK,
            "FATAL: recovered epoch outside the pre-registered channel bank"
        );
        std::process::exit(1);
    }
    channel_bank.blackhole_below(resume_epoch, context);
    let pending_boot = recovery_manifest_for_resume
        .zip(resumed.as_ref())
        .and_then(|(manifest, rec)| derive_pending_boot(manifest, rec));

    // Gateway has its own flow, socket, queue, and admission policy. It shares
    // only the process-wide bulk pacer with state sync and follows the same
    // finalized transport-member cut at boot and every epoch transition.
    let gateway_book = gateway_requests.map(|requests| {
        let book = crate::gateway_plane::OverlayBook::new(
            String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
        );
        book.set_peers(
            initial_member_keys
                .iter()
                .chain(initial_resident_keys.iter()),
        );
        crate::gateway_plane::spawn(
            crate::gateway_plane::SpawnConfig {
                label: label.clone(),
                book: std::sync::Arc::clone(&book),
                me: signer.public_key(),
                factory: crate::overlay_book::socket_factory(overlay_enabled, &overlay_slot),
                pacer: bulk_pacer,
                planes,
                commands: gateway_commands,
                workspace: gateway_workspace,
            },
            requests,
        );
        book
    });
    let ServeLanes {
        blob_peers,
        blob_client,
        sync_state_rx,
        sync_lease,
    } = wire_serve_lanes(
        context,
        &signer,
        &namespace,
        initial_member_keys
            .iter()
            .chain(initial_resident_keys.iter())
            .cloned()
            .collect(),
        blobs,
        sync_monitor,
        sync_tx,
        sync_rx,
    );
    // the submit-relay lane rides the same bounded drop-on-full bridge: a
    // dropped relay degrades to the resident client's honest timeout +
    // re-submit, so flood pressure never blocks the pump.
    let (relay_bridge_tx, relay_ingress) =
        futures::channel::mpsc::channel::<(ed25519::PublicKey, Vec<u8>)>(64);
    context.child("relay_ingress").spawn(move |_ctx| {
        let mut receiver = relay_rx;
        let mut bridge_tx = relay_bridge_tx;
        async move {
            loop {
                match receiver.recv().await {
                    Ok((peer, msg)) => {
                        let bytes: Vec<u8> = msg.into();
                        let _ = bridge_tx.try_send((peer, bytes));
                    }
                    Err(_) => return,
                }
            }
        }
    });
    RuntimeWiring {
        member_keys,
        participants,
        resume_epoch,
        pending_boot,
        mesh_oracle,
        channel_bank,
        gateway_book,
        blob_peers,
        blob_client,
        sync_state_rx,
        sync_lease,
        relay_ingress,
    }
}


/// the statesync serve lanes, shared by the fresh-boot wiring and the
/// in-process promotion seat: the ingress bridge, the SERVE task (capture
/// cache + authed envelope + standing gate + blob demux), and the blob
/// co-client this node fetches committed code through.
pub(super) struct ServeLanes {
    pub(super) blob_peers: Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>>,
    pub(super) blob_client: blob_fetch::ServeLaneBlobClient<super::MeshSender>,
    pub(super) sync_state_rx: futures::channel::mpsc::Receiver<SyncStateRequest>,
    pub(super) sync_lease: Arc<std::sync::atomic::AtomicU64>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn wire_serve_lanes(
    context: &commonware_runtime::tokio::Context,
    signer: &ed25519::PrivateKey,
    namespace: &[u8],
    initial_transport: Vec<ed25519::PublicKey>,
    blobs: noded::blobs::BlobHandle,
    sync_monitor: statesync::monitor::ServeMonitor,
    sync_tx: super::MeshSender,
    sync_rx: super::MeshReceiver,
) -> ServeLanes {
    // the statesync INGRESS task: owns the channel receiver and loops a
    // clean `recv().await`, forwarding frames into a local bounded queue.
    // the pump then selects on THAT queue — dropping an mpsc `next()`
    // future between ticks is lossless, whereas dropping the p2p receiver's
    // actor-backed `recv()` future mid-flight could eat a delivered
    // message. bounded + drop-on-full: clients time out and retry, so a
    // flood degrades to retries instead of unbounded memory.
    let (bridge_tx, sync_ingress) =
        futures::channel::mpsc::channel::<(ed25519::PublicKey, Vec<u8>)>(64);
    context.child("sync_ingress").spawn(move |_ctx| {
        let mut receiver = sync_rx;
        let mut bridge_tx = bridge_tx;
        async move {
            loop {
                match receiver.recv().await {
                    Ok((peer, msg)) => {
                        let bytes: Vec<u8> = msg.into();
                        // full bridge = flood pressure: drop; clients retry.
                        let _ = bridge_tx.try_send((peer, bytes));
                    }
                    Err(_) => return, // network shutdown — nothing to serve.
                }
            }
        }
    });
    // the statesync SERVE task (the [`SyncStateRequest`] seam): owns the
    // capture cache and the mesh statesync carrier end-to-end — decode,
    // leases, chunk slicing, and the mesh replies — so serving a joiner
    // never occupies the consensus loop. the loop answers only the
    // bounded state touches crossing `sync_state_tx`; when the loop is
    // busy the serve lane backpressures, never the reverse.
    let (sync_state_tx, sync_state_rx) = futures::channel::mpsc::channel::<SyncStateRequest>(8);
    // the blob code lane (wasm code distribution): the pending map is the
    // serve loop's demux for THIS validator's own fetches, and the peer book
    // follows every cutover re-track beside the other planes' books.
    let blob_pending: blob_fetch::PendingMap = Default::default();
    let blob_peers: Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>> =
        Arc::new(std::sync::RwLock::new(initial_transport));
    let sync_blobs = blobs;
    // the serve-lane blob co-client: this validator's own fetch side of the
    // blob lane. sends ride a sender clone under this node's OWN standing
    // proof (a validator's key is in the committed valset); answers route
    // back through the pending-map demux the serve loop below runs.
    let (blob_requester, blob_proof) = statesync::sign_sync_proof(signer, namespace);
    let blob_client = blob_fetch::ServeLaneBlobClient::new(
        sync_tx.clone(),
        blob_pending.clone(),
        blob_peers.clone(),
        blob_requester,
        blob_proof,
    );
    let sync_lease = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let state_tx = sync_state_tx;
    let sync_lease_serve = sync_lease.clone();
    let mut sync_tx = sync_tx;
    let mut ingress = sync_ingress;
    // the genesis namespace the standing proof is bound to (ADR §5.1).
    let serve_namespace = namespace.to_vec();
    context
        .child("statesync_serve")
        .spawn(move |_ctx| async move {
            let mut server = SyncServer::new();
            // every refusal below is a SILENT DROP: "why is this joiner never
            // syncing?" is unanswerable from the serving side, because
            // standing-refused, proof-invalid and malformed all look identical
            // (nothing) to both parties. these paths are peer-drivable and a
            // blocked joiner retries forever, so they latch instead of flooding.
            static REFUSED: noded::log::Latch = noded::log::Latch::new(100);
            while let Some((peer, bytes)) = ingress.next().await {
                // mesh frames ride the AUTHENTICATED rpc envelope
                // (requester ‖ proof ‖ id ‖ body — the id correlates).
                let Ok((requester, proof, rpc_id, body)) =
                    statesync::decode_rpc_authed(&bytes)
                else {
                    if let Some(attempts) = REFUSED.hit("malformed_rpc_envelope") {
                        tracing::debug!(
                            target: "ducktape::statesync",
                            peer = %noded::hex_bytes(&peer.as_ref()[..4]),
                            reason = "malformed_rpc_envelope",
                            attempts,
                            "statesync request dropped"
                        );
                    }
                    continue; // malformed rpc envelope: drop, never crash.
                };
                // OUR blob-fetch answers ride the same authed envelope with
                // ZEROED auth fields (the transport authenticates replies):
                // complete the pending waiter by id BEFORE the proof gate
                // below, which would otherwise drop them. a malformed body
                // on a matched id drops the waiter — that fetch times out
                // and rotates, never misreads as a peer's request.
                if let Some(waiter) = blob_pending
                    .lock()
                    .expect("pending blob lock")
                    .remove(&rpc_id)
                {
                    if let Ok(resp) = statesync::decode_response(body) {
                        let _ = waiter.send(resp);
                    }
                    continue; // ours — never a request to serve.
                }
                // FAIL-CLOSED (ADR §5.1). a transport-key standing gate is
                // IMPOSSIBLE at this seam: a pre-admission joiner and an
                // admitted resident share the derived LOBBY key on this
                // channel (boot/mesh.rs), so their peer identity is the
                // SAME. enforcement is a REQUEST-LEVEL real-key proof:
                //  (1) the proof must verify — the requester signed
                //      SYNC_AUTH_NAMESPACE over the genesis namespace with a
                //      key it holds. sound as a STATIC per-session proof: the
                //      mesh transport is authenticated+encrypted, so the
                //      proof is not wire-capturable, and a pre-admission
                //      joiner can only sign for its own non-standing key.
                //  (2) that key must be in COMMITTED standing (validators ∪
                //      residents), read fresh per request through the loop
                //      seam. a valid targeted invite alone yields no standing
                //      key ⇒ leaks ZERO chain state (R4). the restore path
                //      and validator backfill dial under their real keys —
                //      which ARE in the valset — so they still sync; an
                //      admitted resident's key enters residents at its Redeem
                //      block, so it syncs the instant it is admitted, still
                //      under the shared lobby transport key.
                // a failed check DROPS the request (deny-by-default, like the
                // malformed/non-request drops), never a reply.
                if !statesync::verify_sync_proof(requester, proof, &serve_namespace) {
                    if let Some(attempts) = REFUSED.hit("sync_proof_invalid") {
                        tracing::warn!(
                            target: "ducktape::statesync",
                            peer = %noded::hex_bytes(&peer.as_ref()[..4]),
                            requester = %noded::hex_bytes(&requester.as_ref()[..4]),
                            reason = "sync_proof_invalid",
                            attempts,
                            "statesync request REFUSED — the requester's standing proof \
                             did not verify against this genesis namespace"
                        );
                    }
                    continue;
                }
                let requester = *requester;
                // only a decodable REQUEST proceeds. everything else —
                // a stray response, version skew, junk — is DROPPED,
                // never answered: answering non-requests is how two
                // serve loops bounce Error frames forever.
                let Ok(req) = statesync::decode_request(body) else {
                    continue;
                };
                // the COMMITTED-standing check gates the STATE-BEARING lanes
                // (Manifest/Chunk/Module/Frames/Index*), fresh per request via
                // the loop-owned seam (a just-Redeemed resident is admitted
                // immediately; see SyncStateRequest::Standing). the TipCoords
                // DETECTION lane is EXEMPT: it carries coordinates (height,
                // root_hash, epoch, membership), never state bytes, and a node
                // that has LOST standing (a revoked resident) or awaits an
                // out-of-band grant needs it to detect its own transition — a
                // poll its own revocation would otherwise refuse, wedging it
                // forever (it never learns to fall back to a parked joiner).
                // the PoP above still gates it (only a real key-holder polls),
                // and every STATE lane stays refused, so ZERO chain state
                // crosses to a standing-less key.
                if !matches!(req, statesync::SyncRequest::TipCoords) {
                    let (standing_tx, standing_rx) = tokio::sync::oneshot::channel();
                    let mut probe = state_tx.clone();
                    if futures::SinkExt::send(
                        &mut probe,
                        SyncStateRequest::Standing {
                            requester,
                            reply: standing_tx,
                        },
                    )
                    .await
                    .is_err()
                    {
                        continue; // state owner shutting down.
                    }
                    if !standing_rx.await.unwrap_or(false) {
                        // THE one that makes a joiner sync forever in silence.
                        // Both sides see nothing: the joiner just never converges,
                        // and this node never says it was the one refusing.
                        if let Some(attempts) = REFUSED.hit("not_in_committed_standing") {
                            tracing::warn!(
                                target: "ducktape::statesync",
                                requester = %noded::hex_bytes(&requester.as_ref()[..4]),
                                reason = "not_in_committed_standing",
                                attempts,
                                "statesync REFUSED — the requester is not in committed \
                                 standing (it must be admitted before it can sync state)"
                            );
                        }
                        continue; // not in committed standing: refuse (drop).
                    }
                }
                let req_kind = req.kind_name();
                let resp = match req {
                    // blob fetches are host state — answered from the
                    // node-local store, never routed into SyncServer.
                    // standing-gated above like every state lane (code
                    // components are consensus-pinned content).
                    statesync::SyncRequest::Blob { digest } => {
                        blob_fetch::serve_blob(&sync_blobs, &digest)
                    }
                    statesync::SyncRequest::BlobInfo { digest } => {
                        blob_fetch::serve_blob_info(&sync_blobs, &digest)
                    }
                    statesync::SyncRequest::BlobRange {
                        digest,
                        offset,
                        len,
                    } => blob_fetch::serve_blob_range(&sync_blobs, &digest, offset, len),
                    req => {
                        // renew the sync retention lease: this node is
                        // actively serving a syncer, so the drain defers
                        // oplog pruning until the lease lapses.
                        sync_lease_serve.store(
                            crate::sync::serve::unix_now_secs(),
                            std::sync::atomic::Ordering::Relaxed,
                        );
                        drive_sync_request(&mut server, &state_tx, req).await
                    }
                };
                let framed = statesync::encode_rpc_authed(
                    &[0u8; 32],
                    &[0u8; 64],
                    rpc_id,
                    &statesync::encode_response(&resp),
                );
                // the serve-lane observation (`ducktape_statesync_serve_*`):
                // who pulled what, and the progression the response
                // itself proves (served boundary / frame heights).
                sync_monitor.record(
                    &config::hex_bytes(peer.as_ref()),
                    req_kind,
                    &resp,
                    framed.len() as u64,
                );
                let _ = sync_tx.send(Recipients::One(peer), IoBuf::from(framed), false);
            }
        });
    ServeLanes {
        blob_peers,
        blob_client,
        sync_state_rx,
        sync_lease,
    }
}

pub(super) fn mesh_at(
    descriptor_mesh: &[ed25519::PublicKey],
    epoch_members: &std::collections::BTreeSet<ed25519::PublicKey>,
) -> Set<ed25519::PublicKey> {
    let mut union: std::collections::BTreeSet<ed25519::PublicKey> =
        descriptor_mesh.iter().cloned().collect();
    union.extend(epoch_members.iter().cloned());
    Set::try_from(union.into_iter().collect::<Vec<_>>())
        .expect("a btree-set union has no duplicates")
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn wire(
    context: &commonware_runtime::tokio::Context,
    mut network: Network<super::OverlayCtx, ed25519::PrivateKey>,
    oracle: &discovery::Oracle<ed25519::PublicKey>,
    quota: Quota,
    host: &Host,
    resumed: Option<&recovery::Recovered>,
    validators: Vec<ed25519::PublicKey>,
    signer: ed25519::PrivateKey,
    peers: Vec<ed25519::PublicKey>,
    namespace: Vec<u8>,
    label: String,
    coordinated: Vec<(ed25519::PublicKey, Ingress, ed25519::PublicKey)>,
    wireguard_listen: Option<std::net::SocketAddr>,
    wireguard_key_file: std::path::PathBuf,
    chain_id: String,
    mesh_state_file: std::path::PathBuf,
    advertised_reach: Ingress,
    primary_coordinator: Option<String>,
    wireguard_advertised: Option<Ingress>,
    invite_listen: Option<std::net::SocketAddr>,
    coord_cap: Option<nat_traversal::CoordCap>,
    voice_requests: tokio::sync::mpsc::Receiver<noded::RealtimeSessionRequest>,
    overlay_slot: overlay_net::userspace::StackSlot,
    planes: data_plane::PlaneMonitor,
) -> PreWiring {
    // consensus membership comes from the RECOVERY RECORD: the epoch's
    // ENGINE PARTICIPANT SET (at genesis: exactly the config seed). the
    // recovered valset projection is NOT it — a restart inside a cutover
    // window would read a membership change whose boundary has not been
    // crossed and spawn a different scheme than its peers are running.
    let initial_member_keys = match resume_member_keys(resumed, &validators) {
        Ok(keys) => keys,
        Err(e) => {
            tracing::error!(
                target: "ducktape::node",
                node = %label,
                error = %e,
                "FATAL: recovered validator set is invalid"
            );
            std::process::exit(1);
        }
    };
    if !initial_member_keys.contains(&signer.public_key()) {
        tracing::info!(
            target: "ducktape::node",
            node = %label,
            reason = "not_in_recovered_validator_set",
            "halting; restart with --sync-only to observe"
        );
        std::process::exit(0);
    }
    let initial_resume_epoch = resumed.map(|r| r.epoch).unwrap_or(0);

    // the TRANSPORT baseline adds the committed RESIDENT set (granted,
    // quorum-exempt keys the mesh must admit so they can sync). read
    // LIVE from the recovered host, unlike the frozen participant set
    // above: a resident grant arms its own cutover, so within any epoch
    // the resident set is constant — except a reboot inside that cutover
    // window, where this node briefly tracks the wider set alone; the
    // boundary re-tracks identically a few views later.
    let initial_resident_keys: Vec<ed25519::PublicKey> = read_valset_residents(host)
        .await
        .iter()
        .filter_map(|key| ed25519::PublicKey::decode(key.as_slice()).ok())
        .collect();

    // the validator-owned transport mesh, tracked at index = epoch: the
    // epoch's TRANSPORT members (participants ∪ standby registrants) ∪
    // the descriptor mesh (genesis members + [dev] extras — kept
    // authorized so demoted members and pre-genesis peers can still
    // reach the statesync service). the SAME set on every node at this
    // index: discovery kills peers whose bit-vector length disagrees at
    // a shared index, and boundary-read membership is the only set every
    // node agrees on epoch-for-epoch.
    let mut mesh_oracle = (*oracle).clone();
    mesh_oracle.track(
        initial_resume_epoch,
        mesh_at(
            &peers,
            &initial_member_keys
                .iter()
                .chain(initial_resident_keys.iter())
                .cloned()
                .collect(),
        ),
    );

    // lanes for epochs BELOW the resume epoch are registered and
    // black-holed (the sync-only arm's exact trick): a lagging peer still
    // gossips there, and an unregistered channel is a protocol violation
    // that would kill its connection — cutting off the very fetch lane it
    // needs to catch up.
    for epoch in 0..initial_resume_epoch {
        let (vote, cert, res, payload, fetch) = engine_channels(epoch);
        for ch in [vote, cert, res, payload, fetch] {
            let (_tx, mut rx) = network.register(ch, quota, MAX_BACKLOG);
            let label: &'static str = Box::leak(format!("blackhole_{ch}").into_boxed_str());
            context
                .child(label)
                .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
        }
    }

    // pre-register the epoch channel bank from the RESUME epoch up
    // (registration is only possible before network.start(); every
    // respawned engine needs fresh channels). each slot holds epoch
    // (bank_base + i)'s (vote, certificate, resolver, payload, fetch)
    // pairs until that epoch's engine claims them. a restart therefore
    // re-arms the full window — EPOCH_CHANNEL_BANK bounds membership
    // changes per process RUN, not per network lifetime.
    let bank_base = initial_resume_epoch;
    let channel_bank = super::LaneBank::new(
        bank_base,
        (0..EPOCH_CHANNEL_BANK)
            .map(|i| {
                let (vote, cert, res, payload, fetch) = engine_channels(bank_base + i);
                super::LaneSlot::Banked((
                    network.register(vote, quota, MAX_BACKLOG),
                    network.register(cert, quota, MAX_BACKLOG),
                    network.register(res, quota, MAX_BACKLOG),
                    network.register(payload, quota, MAX_BACKLOG),
                    network.register(fetch, quota, MAX_BACKLOG),
                ))
            })
            .collect(),
    );
    let (sync_tx, sync_rx) = network.register(CHANNEL_STATE_SYNC, quota, MAX_BACKLOG);
    // the submit-relay lane: a resident-standing node ships its own
    // signed frame here; this validator takes custody and answers on
    // drain/expiry. bound `mut` because the pump uses `relay_tx` from BOTH
    // the ingress select arm and the drain-resolution/expiry code.
    let (relay_tx, relay_rx) = network.register(CHANNEL_SUBMIT_RELAY, quota, MAX_BACKLOG);

    // the voice + video hub: huddle media between members. per the
    // per-use data-plane ADR (docs/adr/2026-07-07-per-use-data-plane.mdx),
    // media rides the OVERLAY — audio+control on Service::Voice's overlay
    // socket (45902), camera on Service::Video's (45903) — never the mesh.
    let media_peers = {
        // media needs the overlay: with no overlay (fake effect, or the
        // reachability plane unconfigured) there is no media transport at
        // all (the overlay-only cutover — no mesh fallback), so drop the
        // session lane and huddle joins refuse fast instead of hanging.
        let overlay_capable = wireguard_listen.is_some();
        if overlay_capable {
            // tracked media set = transport members ∪ residents, refreshed
            // on every valset cutover (below, beside the statesync book).
            let peers = voice_plane::MediaPeers::new(
                String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
            );
            peers.set_peers(
                initial_member_keys
                    .iter()
                    .chain(initial_resident_keys.iter()),
            );
            let me: [u8; 32] = signer
                .public_key()
                .as_ref()
                .try_into()
                .expect("ed25519 keys are 32 bytes");
            voice::spawn_hub(
                voice_requests,
                crate::overlay_book::socket_factory(overlay_capable, &overlay_slot),
                std::sync::Arc::clone(&peers),
                me,
                planes,
            );
            Some(peers)
        } else {
            // Say it at boot: an operator whose node can never host a huddle
            // otherwise learns it one failed join at a time, from the webview.
            tracing::warn!(
                target: "ducktape::voice",
                node = %label,
                reason = "overlay_unavailable",
                "calls disabled; set wireguard_listen to enable huddles"
            );
            drop(voice_requests);
            None
        }
    };

    // the reachability lane + the staged WireGuard plane. the channel is
    // registered unconditionally (an unregistered channel is a protocol
    // violation that kills the sender's connection); the plane itself
    // runs only when `wireguard_listen` is configured, on its OWN
    // plain-tokio OS thread (the app-surface split exactly), talking to
    // the mesh through the two pump tasks below.
    let (reach_p2p_tx, mut reach_p2p_rx) =
        network.register(CHANNEL_REACHABILITY, quota, MAX_BACKLOG);
    // the join GATE's two connectors between the intro doorbell (the plane's
    // thread) and the validator run loop (join ADR §4): verified gate requests
    // forward in over the channel; resolved outcomes ride back through the
    // shared map. created whether or not the plane runs — the loop's select
    // arm stays wired either way (the keepalive sender keeps it pending, not
    // None-spinning, when no doorbell exists to ring it).
    let (gate_fwd_tx, gate_fwd_rx) = tokio::sync::mpsc::channel::<lobby::GateForward>(256);
    let gate_outcomes = GateOutcomes::default();
    let reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>> =
        match wireguard_listen {
            Some(wg_addr) => {
                // rendezvous coordinators = every coordinated-reach hint's
                // coordinator ingress, PLUS the node-local ambient one
                // (deduped). An invite-joined member's descriptor carries no
                // `coordinated:` hints (stripped at mint time), so WITHOUT a
                // configured `primary_coordinator` it binds zero coordinators
                // and never registers — that is the direct-only posture, and
                // it is why rendezvous needs an explicit node.toml value.
                let mut coordinators: Vec<Ingress> =
                    coordinated.iter().map(|(_, c, _)| c.clone()).collect();
                match config::coordinator_ingress(primary_coordinator.as_deref()) {
                    Ok(Some(ambient)) => {
                        if !coordinators.contains(&ambient) {
                            coordinators.push(ambient);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => tracing::warn!(
                        target: "ducktape::reachability",
                        node = %label,
                        error = %e,
                        reason = "ambient_coordinator_unusable",
                        "registering with descriptor-hinted coordinators only"
                    ),
                }
                Some(wire_reachability_plane(
                    context,
                    &label,
                    &chain_id,
                    &signer,
                    &wireguard_key_file,
                    &mesh_state_file,
                    wg_addr,
                    overlay_slot.clone(),
                    advertised_reach,
                    wireguard_advertised,
                    coordinators,
                    // members serve the invite intro: a fresh joiner's
                    // tunnel comes up against this listener before any p2p.
                    // None when this config mints no direct intro endpoint —
                    // coordinated intros ride the plane's shared socket.
                    invite_listen,
                    coord_cap.clone(),
                    // MEMBER side: the doorbells ring the join gate through
                    // to this validator's run loop (§4).
                    Some(GateHook {
                        forward: gate_fwd_tx.clone(),
                        outcomes: gate_outcomes.clone(),
                    }),
                    reach_p2p_tx,
                    reach_p2p_rx,
                    // a validator never hands this plane off in-process:
                    // demotion exits, promotion already happened.
                    None,
                ))
            }
            None => {
                context
                    .child("blackhole_reachability")
                    .spawn(move |_ctx| async move { while reach_p2p_rx.recv().await.is_ok() {} });
                drop(reach_p2p_tx);
                None
            }
        };
    // boot: target the resume epoch's member set immediately (with the
    // committed resident set as the pre-warm standbys); cutovers
    // retarget from the orchestrator loop below. the recovered view base
    // keeps advert expiries in the same view regime as live peers.
    if let Some(cmd) = &reach_cmd {
        let _ = cmd
            .send(reachability::ReachabilityCommand::Retarget(
                reachability::MeshEpochEvent {
                    epoch: initial_resume_epoch,
                    members: initial_member_keys.clone(),
                    standbys: initial_resident_keys.clone(),
                    current_view: resumed.map(|r| r.view_base).unwrap_or(0),
                },
            ))
            .await;
    }

    // start the network actors (dialer/listener/router/tracker). registered
    // receivers buffer regardless, so starting before the engine is fine.
    network.start();

    PreWiring {
        initial_member_keys,
        initial_resident_keys,
        mesh_oracle,
        bank_base,
        channel_bank,
        sync_tx,
        sync_rx,
        relay_tx,
        relay_rx,
        media_peers,
        reach_cmd,
        gate_fwd_rx,
        gate_fwd_keepalive: gate_fwd_tx,
        gate_outcomes,
    }
}
