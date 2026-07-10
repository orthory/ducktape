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

use crate::config;
use crate::constants::*;
use crate::explorer::heal_index;
use crate::host_reads::read_upgrade_state;
use crate::host_reads::{read_valset_residents, resume_member_keys};
use crate::reachability_plane::wire_reachability_plane;
use crate::sync::catchup::derive_pending_boot;
use crate::sync::serve::{SyncStateRequest, drive_sync_request};
use crate::{blob_fetch, statesync_plane, voice, voice_plane};
use futures::StreamExt as _;
use statesync::SyncServer;

pub(super) struct PreWiring {
    pub(super) initial_member_keys: Vec<ed25519::PublicKey>,
    pub(super) initial_resident_keys: Vec<ed25519::PublicKey>,
    pub(super) mesh_oracle: discovery::Oracle<ed25519::PublicKey>,
    pub(super) bank_base: u64,
    pub(super) channel_bank: super::ChannelBank,
    pub(super) sync_tx: super::MeshSender,
    pub(super) sync_rx: super::MeshReceiver,
    pub(super) lobby_tx: super::MeshSender,
    pub(super) lobby_rx: super::MeshReceiver,
    pub(super) relay_tx: super::MeshSender,
    pub(super) relay_rx: super::MeshReceiver,
    pub(super) media_peers: Option<Arc<voice_plane::MediaPeers>>,
    pub(super) reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>>,
}

pub(super) struct RuntimeWiring {
    pub(super) member_keys: Vec<ed25519::PublicKey>,
    pub(super) participants: Set<ed25519::PublicKey>,
    pub(super) resume_epoch: u64,
    pub(super) pending_boot: Option<u64>,
    pub(super) bank_base: u64,
    pub(super) mesh_oracle: discovery::Oracle<ed25519::PublicKey>,
    pub(super) channel_bank: super::ChannelBank,
    pub(super) sync_plane_book: Option<Arc<statesync_plane::OverlayBook>>,
    pub(super) blob_peers: Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>>,
    pub(super) blob_fetcher: blob_fetch::BlobFetchFn,
    pub(super) sync_state_rx:
        futures::channel::mpsc::Receiver<crate::sync::serve::SyncStateRequest>,
    pub(super) lobby_ingress: futures::channel::mpsc::Receiver<(ed25519::PublicKey, Vec<u8>)>,
    pub(super) relay_ingress: futures::channel::mpsc::Receiver<(ed25519::PublicKey, Vec<u8>)>,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn finish(
    context: &commonware_runtime::tokio::Context,
    index: &indexer::IndexStore,
    host: &Host,
    resumed: Option<&recovery::Recovered>,
    recovery_manifest_for_resume: Option<&recovery::Manifest>,
    boot_fold: crate::explorer::IndexFold<'_>,
    validators: &[ed25519::PublicKey],
    signer: ed25519::PrivateKey,
    label: String,
    peers: Vec<ed25519::PublicKey>,
    namespace: Vec<u8>,
    wireguard_effect: config::WireGuardEffectKind,
    overlay_slot: overlay_net::userspace::StackSlot,
    blobs: noded::blobs::BlobHandle,
    initial_member_keys: Vec<ed25519::PublicKey>,
    initial_resident_keys: Vec<ed25519::PublicKey>,
    mut mesh_oracle: discovery::Oracle<ed25519::PublicKey>,
    bank_base: u64,
    mut channel_bank: super::ChannelBank,
    sync_tx: super::MeshSender,
    sync_rx: super::MeshReceiver,
    lobby_rx: super::MeshReceiver,
    relay_rx: super::MeshReceiver,
) -> RuntimeWiring {
    // the FINAL index heal, at the boot tip every path converged on:
    // whatever the replay/catch-up fold could not reproduce (opaque
    // blocks, a state-sync jump, a stopped fold) re-derives here from
    // state that has verified against the boundary app-hash.
    drop(boot_fold);
    if let Some(boot_height) = resumed.as_ref().and_then(|r| r.height) {
        heal_index(index, host, boot_height, &label).await;
    }

    let member_keys = match resume_member_keys(resumed, validators) {
        Ok(keys) => keys,
        Err(e) => {
            eprintln!("[node {label}] FATAL: {e}");
            std::process::exit(1);
        }
    };
    if !member_keys.contains(&signer.public_key()) {
        println!(
            "[node {label}] this identity is not in the recovered validator set — \
             halting (restart with --sync-only to observe)"
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
    if resume_epoch < bank_base || resume_epoch >= bank_base + EPOCH_CHANNEL_BANK {
        eprintln!(
            "[node {label}] FATAL: recovered epoch {resume_epoch} outside the \
             pre-registered channel bank [{bank_base}, {})",
            bank_base + EPOCH_CHANNEL_BANK
        );
        std::process::exit(1);
    }
    for epoch in bank_base..resume_epoch {
        let Some(slot) = channel_bank
            .get_mut((epoch - bank_base) as usize)
            .and_then(|slot| slot.take())
        else {
            continue;
        };
        let ((_, vote_rx), (_, cert_rx), (_, res_rx), (_, payload_rx), (_, fetch_rx)) = slot;
        for (suffix, mut rx) in [
            ("vote", vote_rx),
            ("cert", cert_rx),
            ("resolver", res_rx),
            ("payload", payload_rx),
            ("fetch", fetch_rx),
        ] {
            let label: &'static str =
                Box::leak(format!("blackhole_e{epoch}_{suffix}").into_boxed_str());
            context
                .child(label)
                .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
        }
    }
    let mut pending_boot = recovery_manifest_for_resume
        .zip(resumed.as_ref())
        .and_then(|(manifest, rec)| derive_pending_boot(manifest, rec));
    // If no membership cutover already claimed the resume slot, re-arm a
    // pending upgrade at the same deterministic activation boundary an
    // uninterrupted node would use. This runs after post-reboot catch-up, so
    // it reads the freshest recovered host/record.
    if pending_boot.is_none()
        && let Some(rec) = resumed.as_ref()
    {
        pending_boot = read_upgrade_state(host).await.pending.and_then(|p| {
            let crossed = rec.height.is_some_and(|h| h >= p.activation_height);
            if crossed {
                None
            } else {
                p.activation_height.checked_sub(rec.view_base)
            }
        });
    }

    // the statesync INGRESS task: owns the channel receiver and loops a
    // clean `recv().await`, forwarding frames into a local bounded queue.
    // the pump then selects on THAT queue — dropping an mpsc `next()`
    // future between ticks is lossless, whereas dropping the p2p receiver's
    // actor-backed `recv()` future mid-flight could eat a delivered
    // message. bounded + drop-on-full: clients time out and retry, so a
    // flood degrades to retries instead of unbounded memory. the queue
    // carries BOTH statesync carriers — mesh rpc frames and data-plane
    // request streams — so one serve task answers both.
    let (bridge_tx, sync_ingress) = futures::channel::mpsc::channel::<statesync_plane::SyncJob>(64);
    // the blob fetch-on-miss lane (the #298 prompt-blob cross-node gap):
    // the oracle pool's resolver asks current peers for a digest its own
    // store lacks, over this same statesync channel. the pending map is
    // the serve loop's demux — frames answering OUR fetches never enter
    // the request path — and the peer set follows every cutover re-track
    // beside the other planes' books.
    let blob_pending: blob_fetch::PendingMap = Default::default();
    let blob_peers: std::sync::Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>> =
        std::sync::Arc::new(std::sync::RwLock::new(
            initial_member_keys
                .iter()
                .chain(initial_resident_keys.iter())
                .cloned()
                .collect(),
        ));
    let blob_fetcher = blob_fetch::MeshBlobFetcher::new(
        sync_tx.clone(),
        blob_pending.clone(),
        std::sync::Arc::clone(&blob_peers),
        signer.public_key(),
    )
    .into_fetch_fn();
    {
        let mut bridge_tx = bridge_tx.clone();
        context.child("sync_ingress").spawn(move |_ctx| {
            let mut receiver = sync_rx;
            async move {
                loop {
                    match receiver.recv().await {
                        Ok((peer, msg)) => {
                            let bytes: Vec<u8> = msg.into();
                            // full bridge = flood pressure: drop; clients retry.
                            let _ = bridge_tx.try_send(statesync_plane::SyncJob::Mesh(peer, bytes));
                        }
                        Err(_) => return, // network shutdown — nothing to serve.
                    }
                }
            }
        });
    }
    // statesync's per-use data plane (env-gated, default off): the same
    // requests over overlay stream sockets, accepted into the same queue.
    // the address book doubles as admission — members + standbys of the
    // tracked view, updated at every cutover re-track below.
    let sync_plane_book = statesync_plane::enabled().then(|| {
        let book = statesync_plane::OverlayBook::new(
            String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
        );
        book.set_peers(
            initial_member_keys
                .iter()
                .chain(initial_resident_keys.iter()),
        );
        statesync_plane::spawn_bring_up(
            label.clone(),
            std::sync::Arc::clone(&book),
            signer.public_key(),
            std::sync::Arc::new(std::sync::OnceLock::new()),
            statesync_plane::socket_factory(wireguard_effect, &overlay_slot),
            Some(bridge_tx.clone()),
        );
        book
    });
    drop(bridge_tx);
    // the statesync SERVE task (the [`SyncStateRequest`] seam): owns the
    // capture cache and both statesync carriers end-to-end — decode,
    // leases, chunk slicing, and the mesh/plane replies — so serving a
    // joiner never occupies the consensus loop. the loop answers only
    // the bounded state touches crossing `sync_state_tx`; when the loop
    // is busy the serve lane backpressures, never the reverse.
    let (sync_state_tx, sync_state_rx) = futures::channel::mpsc::channel::<SyncStateRequest>(8);
    {
        let state_tx = sync_state_tx;
        let mut sync_tx = sync_tx;
        let mut ingress = sync_ingress;
        let blob_pending = blob_pending.clone();
        let sync_blobs = blobs.clone();
        context
            .child("statesync_serve")
            .spawn(move |_ctx| async move {
                let mut server = SyncServer::new();
                while let Some(job) = ingress.next().await {
                    // both carriers land here: mesh frames ride an rpc
                    // envelope (multiplexed channel — the id correlates);
                    // a plane stream IS its own correlation and reply path.
                    let (reply_to, rpc_id, request) = match job {
                        statesync_plane::SyncJob::Mesh(peer, bytes) => {
                            let Ok((rpc_id, body)) = statesync::decode_rpc(&bytes) else {
                                continue; // malformed rpc envelope: drop, never crash.
                            };
                            // the mesh demux: OUR fetch answers are consumed,
                            // stray responses (a blob answer landing after its
                            // fan-out's sweep) and unparseable frames are
                            // DROPPED — answering either is how two serve
                            // loops bounce Error frames forever. only a real
                            // request proceeds; the reply-on-bad-frame lane is
                            // stream-only below.
                            match blob_fetch::classify_mesh_frame(&blob_pending, rpc_id, body) {
                                blob_fetch::MeshFrame::OurResponse
                                | blob_fetch::MeshFrame::StrayResponse
                                | blob_fetch::MeshFrame::Junk => continue,
                                blob_fetch::MeshFrame::Request(req) => {
                                    (statesync_plane::SyncReplyTo::Mesh(peer), rpc_id, Ok(req))
                                }
                            }
                        }
                        statesync_plane::SyncJob::Plane(stream, req) => (
                            statesync_plane::SyncReplyTo::Plane(stream),
                            0,
                            statesync::decode_request(&req),
                        ),
                    };
                    let resp = match request {
                        // blob fetches are host state — answered from the
                        // node-local store, never routed into SyncServer.
                        Ok(statesync::SyncRequest::Blob { digest }) => {
                            blob_fetch::serve_blob(&sync_blobs, &digest)
                        }
                        Ok(req) => drive_sync_request(&mut server, &state_tx, req).await,
                        // stream-only by construction: a plane stream is a
                        // one-shot request/response, so an Error reply here
                        // can never re-enter a serve loop and oscillate.
                        Err(e) => statesync::SyncResponse::Error(format!("bad request frame: {e}")),
                    };
                    let resp = statesync::encode_response(&resp);
                    match reply_to {
                        statesync_plane::SyncReplyTo::Mesh(peer) => {
                            let _ = sync_tx.send(
                                Recipients::One(peer),
                                IoBuf::from(statesync::encode_rpc(rpc_id, &resp)),
                                false,
                            );
                        }
                        statesync_plane::SyncReplyTo::Plane(mut stream) => {
                            // one request per stream: write the response
                            // and drop — the close is the client's
                            // completion.
                            let _ = statesync::dataplane::write_frame(&mut stream, &resp).await;
                        }
                    }
                }
            });
    }
    // the lobby lane rides the same bridge pattern: announces are consumed
    // by the pump between drains. drop-on-full is doubly safe here — a
    // parked joiner re-announces every few seconds anyway.
    let (lobby_bridge_tx, lobby_ingress) =
        futures::channel::mpsc::channel::<(ed25519::PublicKey, Vec<u8>)>(64);
    context.child("lobby_ingress").spawn(move |_ctx| {
        let mut receiver = lobby_rx;
        let mut bridge_tx = lobby_bridge_tx;
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
        bank_base,
        mesh_oracle,
        channel_bank,
        sync_plane_book,
        blob_peers,
        blob_fetcher,
        sync_state_rx,
        lobby_ingress,
        relay_ingress,
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
    wireguard_effect: config::WireGuardEffectKind,
    wireguard_key_file: std::path::PathBuf,
    chain_id: String,
    mesh_state_file: std::path::PathBuf,
    advertised_reach: Ingress,
    primary_coordinator: Option<String>,
    wireguard_advertised: Option<Ingress>,
    invite_listen: Option<std::net::SocketAddr>,
    coord_cap: Option<nat_traversal::CoordCap>,
    voice_requests: tokio::sync::mpsc::Receiver<noded::CallSessionRequest>,
    overlay_slot: overlay_net::userspace::StackSlot,
) -> PreWiring {
    // consensus membership comes from the RECOVERY RECORD: the epoch's
    // ENGINE PARTICIPANT SET (at genesis: exactly the config seed). the
    // recovered valset projection is NOT it — a restart inside a cutover
    // window would read a membership change whose boundary has not been
    // crossed and spawn a different scheme than its peers are running.
    let initial_member_keys = match resume_member_keys(resumed, &validators) {
        Ok(keys) => keys,
        Err(e) => {
            eprintln!("[node {label}] FATAL: {e}");
            std::process::exit(1);
        }
    };
    if !initial_member_keys.contains(&signer.public_key()) {
        println!(
            "[node {label}] this identity is not in the recovered validator set — \
             halting (restart with --sync-only to observe)"
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
    // respawned engine needs fresh channels). bank[i] holds epoch
    // (bank_base + i)'s (vote, certificate, resolver, payload, fetch)
    // pairs until that epoch's engine consumes them. a restart therefore
    // re-arms the full window — EPOCH_CHANNEL_BANK bounds membership
    // changes per process RUN, not per network lifetime.
    let bank_base = initial_resume_epoch;
    let channel_bank: Vec<Option<_>> = (0..EPOCH_CHANNEL_BANK)
        .map(|i| {
            let (vote, cert, res, payload, fetch) = engine_channels(bank_base + i);
            Some((
                network.register(vote, quota, MAX_BACKLOG),
                network.register(cert, quota, MAX_BACKLOG),
                network.register(res, quota, MAX_BACKLOG),
                network.register(payload, quota, MAX_BACKLOG),
                network.register(fetch, quota, MAX_BACKLOG),
            ))
        })
        .collect();
    let (sync_tx, sync_rx) = network.register(CHANNEL_STATE_SYNC, quota, MAX_BACKLOG);
    // the lobby lane: parked joiners announce their keys here (connected
    // as the derived lobby identity); this member verifies each announce
    // against the invite token it carries and RECORDS it for approval.
    let (lobby_tx, lobby_rx) = network.register(CHANNEL_LOBBY, quota, MAX_BACKLOG);
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
        let overlay_capable = wireguard_listen.is_some()
            && !matches!(wireguard_effect, config::WireGuardEffectKind::Fake);
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
                statesync_plane::socket_factory(wireguard_effect, &overlay_slot),
                std::sync::Arc::clone(&peers),
                me,
            );
            Some(peers)
        } else {
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
    let reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>> =
        match wireguard_listen {
            Some(wg_addr) => {
                // rendezvous coordinators = every coordinated-reach hint's
                // coordinator ingress, PLUS the ambient override/default
                // (deduped) — without it an invite-joined member (whose
                // descriptor carries no `coordinated:` hints, stripped at
                // mint time) binds zero coordinators and never registers.
                let mut coordinators: Vec<Ingress> =
                    coordinated.iter().map(|(_, c, _)| c.clone()).collect();
                match config::coordinator_ingress(primary_coordinator.as_deref()) {
                    Ok(Some(ambient)) => {
                        if !coordinators.contains(&ambient) {
                            coordinators.push(ambient);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!(
                        "[node {label}] reachability: ambient coordinator unusable ({e}) — \
                         registering with descriptor-hinted coordinators only"
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
                    wireguard_effect,
                    overlay_slot.clone(),
                    advertised_reach,
                    wireguard_advertised,
                    coordinators,
                    // members serve the invite intro: a fresh joiner's
                    // tunnel comes up against this listener before any p2p.
                    invite_listen,
                    coord_cap.clone(),
                    reach_p2p_tx,
                    reach_p2p_rx,
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
        lobby_tx,
        lobby_rx,
        relay_tx,
        relay_rx,
        media_peers,
        reach_cmd,
    }
}
