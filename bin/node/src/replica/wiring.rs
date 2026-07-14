//! phase 6a of the joiner/replica role: everything that must register with
//! the mesh BEFORE `network.start()` — the per-epoch engine-channel bank
//! (cert/payload/black-holed lanes), the statesync channel pair, the
//! reachability plane's STANDBY wiring (+ the tunnel-first join race), and
//! the relay/lobby lanes, ending with the lobby-reply printer task and the
//! `network.start()` call itself. `park` (phase 6b–6d) picks up everything
//! this phase produced via [`ReplicaChannels`].

use commonware_cryptography::ed25519;
use commonware_p2p::authenticated::discovery::{self, Network};
use commonware_p2p::{Ingress, Manager, Receiver as P2pReceiver};
use commonware_runtime::{Quota, Spawner, Supervisor};
use commonware_utils::ordered::Set;
use consensus::ContentStore;
use recovery::{Manifest, Recovery};

use crate::config::{self, hex_bytes};
use crate::constants::*;
use crate::first_contact_join;
use crate::lobby;
use crate::reachability_plane::wire_reachability_plane;
use crate::util::fatal;

use super::OverlayCtx;

/// phase 6a's output: every channel/handle phase 6b–6d needs, handed to
/// [`super::park::park`] as one bundle. `network.start()` has already run by
/// the time this exists — no further registration is legal on this mesh.
///
/// `context` rides along too: `commonware_runtime::tokio::Context` has no
/// `Clone` (see `boot::mesh::MeshHead`'s doc comment), so it cannot be a
/// pass-through param like `signer`/`label`/etc — `wire` is hosting it
/// between the two calls, not really "producing" it.
pub(super) struct ReplicaChannels {
    pub(super) context: commonware_runtime::tokio::Context,
    pub(super) replica_store: ContentStore,
    pub(super) head_wake: futures::channel::mpsc::Receiver<()>,
    pub(super) cert_bridge: futures::channel::mpsc::Receiver<Vec<u8>>,
    pub(super) sync_tx: discovery::Sender<ed25519::PublicKey, OverlayCtx>,
    pub(super) sync_rx: discovery::Receiver<ed25519::PublicKey>,
    pub(super) reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>>,
    pub(super) relay_tx: discovery::Sender<ed25519::PublicKey, OverlayCtx>,
    pub(super) relay_rx: discovery::Receiver<ed25519::PublicKey>,
    pub(super) lobby_tx: discovery::Sender<ed25519::PublicKey, OverlayCtx>,
    /// the joiner's gate-reply lane: the park loop's gate phase blocks on this
    /// for the authoritative `GateMsg::Admitted`/`Rejected` (ADR §3.3). NOT a
    /// side printer any more — the reply IS the admission signal.
    pub(super) lobby_rx: discovery::Receiver<ed25519::PublicKey>,
    pub(super) voice_requests: tokio::sync::mpsc::Receiver<noded::RealtimeSessionRequest>,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn wire(
    context: commonware_runtime::tokio::Context,
    mut network: Network<OverlayCtx, ed25519::PrivateKey>,
    oracle: &mut discovery::Oracle<ed25519::PublicKey>,
    quota: Quota,
    mesh_participants: &Set<ed25519::PublicKey>,
    recovery: &Recovery<commonware_runtime::tokio::Context>,
    manifest: &Option<Manifest>,
    signer: ed25519::PrivateKey,
    label: String,
    namespace: Vec<u8>,
    wireguard_listen: Option<std::net::SocketAddr>,
    wireguard_effect: config::WireGuardEffectKind,
    wireguard_key_file: std::path::PathBuf,
    chain_id: String,
    mesh_state_file: std::path::PathBuf,
    advertised_reach: Ingress,
    // the AMBIENT coordinator override (node.toml `primary_coordinator`);
    // `None` = the compiled-in default, exactly as before the key existed.
    primary_coordinator: Option<String>,
    // the WireGuard bind/advertise split (node.toml `wireguard_advertised`).
    wireguard_advertised: Option<Ingress>,
    coord_cap: &Option<nat_traversal::CoordCap>,
    invite_token: &Option<config::InviteToken>,
    invite_wireguard: &Option<config::StoredInviteWireGuard>,
    invite_fronts: Vec<config::Front>,
    voice_requests: tokio::sync::mpsc::Receiver<noded::RealtimeSessionRequest>,
    overlay_slot: overlay_net::userspace::StackSlot,
) -> ReplicaChannels {
    if manifest.is_none() && !recovery.journal_is_empty().await {
        fatal!(label, "recovery journal exists but the checkpoint is \
             missing — wipe the app state and re-join (KEEP any consensus journal \
             partitions: they are what prevents this key from double-voting)");
    }
    // the parked mesh identity: genesis set at the base index (no
    // consensus coordinates yet). engine lanes are NOT black-holed
    // like the sync-only resident — the replica pipeline (phase 2)
    // consumes them:
    // - CERT lanes bridge their raw bytes to the fold driver, which
    //   decodes finalizations and verifies them against the epoch's
    //   quorum (the phase-1 gate). pre-standing, the same bytes fire
    //   the park loop's wake (a byte's arrival is the old nudge).
    // - PAYLOAD lanes drain store-only into the shared content store
    //   (content-addressing is the verification), so a finalization's
    //   bytes are usually already local when its certificate lands.
    // - vote/resolver/fetch lanes stay black-holed. the follower runs
    //   WITHOUT a payload resolver: a gossip-missed payload surfaces
    //   as Unresolvable and backfills over the Frames lane (the
    //   backstop that must exist anyway). a banked-but-unread lane is
    //   NOT an option — validators' resolvers send fetch requests to
    //   every tracked peer, and an unread backlog jams the very
    //   connection the sync client rides.
    oracle.track(PEER_SET, mesh_participants.clone());
    let replica_store = ContentStore::new();
    let (head_wake_tx, head_wake) = futures::channel::mpsc::channel::<()>(1);
    // raw cert-lane bytes for the fold driver: bounded, drop-on-full —
    // a shed certificate is re-anchored by the next one's parent
    // linkage (the planner backfills the gap), so the drain never
    // blocks the peer connection.
    let (cert_bridge_tx, cert_bridge) =
        futures::channel::mpsc::channel::<Vec<u8>>(256);
    for epoch in 0..EPOCH_CHANNEL_BANK {
        let (vote, cert, res, payload, fetch) = engine_channels(epoch);
        for ch in [vote, res, fetch] {
            let (_tx, mut rx) = network.register(ch, quota, MAX_BACKLOG);
            let label: &'static str =
                Box::leak(format!("blackhole_{ch}").into_boxed_str());
            context.child(label).spawn(move |_ctx| async move {
                while rx.recv().await.is_ok() {}
            });
        }
        {
            let (_tx, mut payload_rx) = network.register(payload, quota, MAX_BACKLOG);
            let store = replica_store.clone();
            let label: &'static str =
                Box::leak(format!("payload_store_{payload}").into_boxed_str());
            context.child(label).spawn(move |_ctx| async move {
                while let Ok((_peer, msg)) = payload_rx.recv().await {
                    let bytes: Vec<u8> = msg.into();
                    // store-ONLY, never delivered: delivery is the
                    // fold driver's verified-finalization arm.
                    store.put(bytes);
                }
            });
        }
        let (_tx, mut cert_rx) = network.register(cert, quota, MAX_BACKLOG);
        let label: &'static str =
            Box::leak(format!("certbridge_{cert}").into_boxed_str());
        let mut wake = head_wake_tx.clone();
        let mut bridge = cert_bridge_tx.clone();
        context.child(label).spawn(move |_ctx| async move {
            while let Ok((_peer, msg)) = cert_rx.recv().await {
                let bytes: Vec<u8> = msg.into();
                // full == a wake is already pending: coalesce, never
                // block the drain (an unread lane kills the peer).
                let _ = wake.try_send(());
                // drop-on-full: parent linkage re-covers shed certs.
                let _ = bridge.try_send(bytes);
            }
        });
    }
    let (sync_tx, sync_rx) = network.register(CHANNEL_STATE_SYNC, quota, MAX_BACKLOG);
    // the reachability lane: a parked joiner with a WireGuard config
    // runs the plane in its STANDBY role — once resident standing
    // lands (the park loop below drives Retargets off the manifest),
    // it pre-warms tunnels with every member so activation, and the
    // promotion reboot via the persisted mesh, start connected
    // instead of assembling. Without `wireguard_listen` the channel
    // just stays legal — black-hole.
    let reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>> = {
        let (reach_tx, mut reach_rx) =
            network.register(CHANNEL_REACHABILITY, quota, MAX_BACKLOG);
        match wireguard_listen {
            Some(wg_addr) => {
                // AMBIENT coordinator: the joiner resolves coordinated
                // rendezvous through its OWN configured/default
                // coordinator, NEVER one baked into the invite (the
                // unified invite carries no coordinator address). See
                // docs/superpowers/specs/2026-07-08-fully-nated-inviter-design.md.
                let coordinators: Vec<Ingress> =
                    match config::coordinator_ingress(primary_coordinator.as_deref()) {
                        Ok(Some(ingress)) => vec![ingress],
                        Ok(None) => Vec::new(),
                        Err(e) => {
                            eprintln!(
                                "[node {label}] invite: ambient coordinator unusable ({e}) — \
                                 coordinated first-contact paths disabled"
                            );
                            Vec::new()
                        }
                    };
                Some(wire_reachability_plane(
                    &context,
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
                    // a joiner serves no intros — only members mint
                    // redeemable invites.
                    None,
                    coord_cap.clone(),
                    reach_tx,
                    reach_rx,
                ))
            }
            None => {
                context
                    .child("blackhole_reachability")
                    .spawn(move |_ctx| async move { while reach_rx.recv().await.is_ok() {} });
                drop(reach_tx);
                None
            }
        }
    };
    // the TUNNEL-FIRST join window: an invite that carried a WireGuard
    // bootstrap makes the tunnel the join's carrier — before any p2p,
    // (a) this node's interface gains the INVITER as a peer (endpoint
    // straight from the blob), and (b) an intro announcer delivers
    // this node's identity + WireGuard key to the inviter's intro
    // listener until acked, at which point the inviter's side of the
    // tunnel exists too. the mesh dialer below then reaches the
    // inviter's overlay ULA (the join-minted Direct hint) the moment
    // the tunnel routes, and everything else — lobby announce,
    // redemption, statesync — rides it.
    // the TUNNEL-FIRST join window races the invite's UNIFIED path
    // set: the inviter PLUS every offered front, in one candidate list.
    // The first candidate to install this joiner's token-signed intro
    // wins and the rest are cancelled; the mesh dialer below then
    // reaches that member's overlay ULA (the join-minted Direct hints)
    // the moment the tunnel routes, and everything else — lobby
    // announce, redemption, statesync — rides it. If every offered path
    // is exhausted the race is HONEST-terminal (a distinct exit, never
    // a silent success). The mechanics live in `first_contact_join`;
    // this is just the glue.
    if let (Some(reach), Some(token)) = (&reach_cmd, &invite_token) {
        let inviter = invite_wireguard.as_ref().and_then(|wg| {
            match (wg.issuer_key(), wg.public_key_bytes()) {
                (Ok(key), Ok(wg_key)) => Some(first_contact_join::InviterContact {
                    key,
                    wg: wg_key,
                    mesh_port: wg.mesh_port,
                    // the inviter's underlay endpoint; `None` => the
                    // inviter is coordinated-only (reached by identity).
                    endpoint: wg.endpoint.clone(),
                    // the inviter's explicitly-advertised intro listener
                    // (honors a custom `invite_listen`); the direct path
                    // uses it verbatim instead of re-deriving wg_port+1.
                    intro: wg.intro.clone(),
                }),
                _ => {
                    eprintln!(
                        "[node {label}] invite: inviter wireguard bootstrap is malformed \
                         — racing the offered fronts alone"
                    );
                    None
                }
            }
        });
        let raw = first_contact_join::build_candidates(inviter, &invite_fronts);
        if raw.is_empty() {
            // the invite offered no wireguard/front bootstrap — the join
            // rides the descriptor's reach hints, exactly as before.
        } else {
            let candidates = first_contact_join::plan_race(
                raw,
                matches!(wireguard_effect, config::WireGuardEffectKind::Tun),
            );
            match reachability::WireGuardKeypair::load_or_generate(&wireguard_key_file) {
                Ok((keypair, _)) => {
                    // this joiner's own token-signed intro, built once
                    // and reused across every candidate in the race.
                    let intro = lobby::encode_intro(&lobby::intro_request(
                        &signer,
                        &namespace,
                        token,
                        keypair.public_key().0,
                    ));
                    let token_nonce = token.nonce.to_vec();
                    let reach = reach.clone();
                    let race_label = label.clone();
                    // an exhausted race is honest-terminal ONLY for a fresh
                    // join (no checkpoint): a bad invite must exit loudly,
                    // never spin silently. a RESTART with standing (the
                    // checkpoint exists) is a different animal — its paths
                    // are known-good and merely dark (machine woke before
                    // its network, coordinator briefly down), so dying at
                    // 90s turns every offline boot into a dead node the
                    // operator must resurrect by hand. it re-races forever,
                    // loudly, instead.
                    let restart_with_standing = manifest.is_some();
                    context.child("first_contact").spawn(move |_ctx| async move {
                        let mut round = 0u32;
                        loop {
                            round += 1;
                            let outcome = first_contact_join::drive_first_contact(
                                reach.clone(),
                                candidates.clone(),
                                intro.clone(),
                                token_nonce.clone(),
                                race_label.clone(),
                                std::time::Duration::from_secs(90),
                            )
                            .await;
                            match outcome {
                                first_contact_join::FirstContactOutcome::Installed {
                                    key,
                                    via,
                                } => {
                                    println!(
                                        "[node {race_label}] invite: first contact via \
                                         {via} to {} — join rides the overlay",
                                        hex_bytes(&key.as_ref()[..4])
                                    );
                                    return;
                                }
                                first_contact_join::FirstContactOutcome::Terminal {
                                    tried,
                                    reason,
                                } => {
                                    if !restart_with_standing {
                                        // NOT `fatal!`: this path exits 3, not 1 — a
                                        // join that ran out of paths is distinct from a
                                        // node that broke, and callers read the code.
                                        tracing::error!(
                                            target: "ducktape::join",
                                            node = %race_label,
                                            tried,
                                            reason = %reason,
                                            "FATAL: first contact failed across all offered \
                                             path(s) — ask the inviter for a fresh invite once \
                                             the mesh is reachable"
                                        );
                                        std::process::exit(3);
                                    }
                                    eprintln!(
                                        "[node {race_label}] first contact round {round} \
                                         failed across all {tried} offered path(s) — \
                                         {reason}; this node holds a synced checkpoint, \
                                         so it keeps retrying (next round in 30s) instead \
                                         of exiting"
                                    );
                                    tokio::time::sleep(std::time::Duration::from_secs(30))
                                        .await;
                                }
                            }
                        }
                    });
                }
                Err(e) => eprintln!(
                    "[node {label}] invite: wireguard key unreadable ({e}) — first \
                     contact not started; falling back to the descriptor's reach hints"
                ),
            }
        }
    }
    // the submit-relay lane: once resident standing lands, writes leave
    // here — this node signs its own frames and a validator takes
    // custody. replies (the frame's consensus fate) come back on the
    // same lane. `relay_rx` is bridged into the serve window below (a
    // torn-down select must never drop its `recv()` mid-flight).
    let (relay_tx, relay_rx) = network.register(CHANNEL_SUBMIT_RELAY, quota, MAX_BACKLOG);
    // the lobby lane: the joiner speaks `GateMsg::Request` here and the park
    // loop's gate phase (ADR §3.3) blocks on the authoritative reply — no side
    // printer, the reply IS the admission signal, so `lobby_rx` is handed to
    // `park` verbatim.
    let (lobby_tx, lobby_rx) = network.register(CHANNEL_LOBBY, quota, MAX_BACKLOG);
    network.start();

    ReplicaChannels {
        context,
        replica_store,
        head_wake,
        cert_bridge,
        sync_tx,
        sync_rx,
        reach_cmd,
        relay_tx,
        relay_rx,
        lobby_tx,
        lobby_rx,
        voice_requests,
    }
}
