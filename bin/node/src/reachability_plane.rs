use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::{
    AddressableManager as _, Ingress, Receiver as P2pReceiver, Recipients, Sender as P2pSender,
};
use commonware_runtime::{IoBuf, Spawner, Supervisor};

use crate::config::{self, hex_bytes};
use crate::constants::NUDGE_INTERVAL;
use crate::join_gate;

/// Which doorbell an intro arrived on: the DIRECT UDP listener or the
/// COORDINATED (rendezvous-punched, resolver-socket) receiver.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum IntroPath {
    Direct,
    Coordinated,
}

/// One inviter-side intro datagram, shared by BOTH doorbells: OPEN → decode →
/// verify → install → ack, in that order — the ack is only emitted after the
/// `InstallInvitePeer` reply settles, so "acked" can never outrun
/// "installed". The datagram arrives SEALED to this member's WireGuard X25519
/// key (join ADR, item 5): a bearer token never crosses the wire in
/// the clear, so `open` decrypts it with the member's secret before anything
/// else. `ack` abstracts the reply transport (the direct listener answers on
/// its own socket, the coordinated receiver via `SendResolverDatagram`).
/// Returns `false` once the plane's command channel is gone, telling the
/// caller to exit its receive loop.
/// the shared gate-outcome map (joiner key → its resolved [`join_gate::IntroReply`]):
/// the run loop's drain WRITES the settled outcome, the intro doorbell READS it
/// on the joiner's next retransmit and seals it back down the tunnel.
pub(crate) type GateOutcomes =
    std::sync::Arc<std::sync::Mutex<HashMap<Vec<u8>, join_gate::IntroReply>>>;

/// the caller-side halves of the plane's lane-reclaim seam (see
/// `wire_reachability_plane`'s `lane_reclaim`): each resolves with its half
/// of the CHANNEL_REACHABILITY pair once the plane exits.
pub(crate) type ReachLaneHandback = (
    futures::channel::oneshot::Receiver<crate::validator::MeshSender>,
    futures::channel::oneshot::Receiver<crate::validator::MeshReceiver>,
);

/// The member side's link from the intro doorbell (reachability-plane thread)
/// to its validator run loop (§4). The doorbell FORWARDS a verified gate request
/// to the loop, which submits `Redeem` and settles; the loop's drain writes the
/// resolved outcome into `outcomes`, which the doorbell reads on the joiner's
/// next retransmit and seals back. A joiner's own plane carries `None`.
#[derive(Clone)]
pub(crate) struct GateHook {
    pub(crate) forward: tokio::sync::mpsc::Sender<join_gate::GateForward>,
    pub(crate) outcomes: GateOutcomes,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_intro<F, Fut, O>(
    sealed: &[u8],
    src: std::net::SocketAddr,
    binding: &[u8],
    label: &str,
    path: IntroPath,
    cmds: &tokio::sync::mpsc::WeakSender<reachability::ReachabilityCommand>,
    open: O,
    gate: Option<&GateHook>,
    ack: F,
) -> bool
where
    F: FnOnce(Vec<u8>) -> Fut,
    Fut: std::future::Future<Output = ()>,
    O: FnOnce(&[u8]) -> Result<Vec<u8>, String>,
{
    // OPEN the sealed envelope to THIS member's WG key. A datagram that does not
    // open — an observer's junk, or an intro sealed to a different member — is
    // dropped silently: no nonce to echo, no answer earned.
    let Ok(plaintext) = open(sealed) else {
        return true;
    };
    let Ok(msg) = join_gate::decode_intro(&plaintext) else {
        return true;
    };
    let nonce = msg.nonce.clone();
    let verified = match join_gate::verify_intro(&msg, binding) {
        Ok(v) => v,
        Err(_) => return true,
    };
    // past verification we hold the joiner's WG key: every reply from here is
    // SEALED to it, so an `Admitted`'s coordinator capability never crosses the
    // wire in the clear.
    let joiner_wg = verified.wg_public_key;
    let sealed_reply = |reply: join_gate::IntroReply| {
        let bytes = join_gate::encode_intro_ack(&join_gate::IntroAck {
            nonce: nonce.clone(),
            reply,
        });
        reachability::seal(&joiner_wg, &bytes)
    };
    // V4 expiry, on this member's wall clock (signature-covered field).
    if nat_traversal::now_secs() >= msg.expires_unix_secs {
        if path == IntroPath::Direct {
            ack(sealed_reply(join_gate::IntroReply::Refused {
                detail: "invite expired — ask the inviter for a fresh one".into(),
            }))
            .await;
        }
        return true;
    }
    // V6/V7 need committed state — those run at the loop (`on_gate_forward`).
    let Some(cmds) = cmds.upgrade() else {
        return false;
    };
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    let install = reachability::ReachabilityCommand::InstallInvitePeer {
        peer: verified.joiner.clone(),
        wireguard_public_key: wireguard::X25519PublicKey(verified.wg_public_key),
        endpoint: src,
        reply: reachability::InstallReply(reply_tx),
    };
    if cmds.send(install).await.is_err() {
        return false;
    }
    match reply_rx.await {
        Ok(Ok(())) => {
            let via = match path {
                IntroPath::Direct => "direct",
                IntroPath::Coordinated => "coordinated",
            };
            tracing::info!(
                target: "ducktape::join",
                node = %label,
                peer = %config::hex_bytes(&verified.joiner.as_ref()[..4]),
                via,
                "invite intro tunnel peer installed"
            );
            // THE GATE (§4): the sealed intro IS the gate request. Forward it to
            // the run loop and ack the CURRENT outcome — `Installed` while it
            // settles, or the resolved `Admitted`/`Rejected` a later retransmit
            // picks up from the shared map.
            let reply = gate_reply(gate, &verified.joiner, &msg).await;
            ack(sealed_reply(reply)).await;
        }
        Ok(Err(e)) => ack(sealed_reply(join_gate::IntroReply::Refused { detail: e })).await,
        Err(_) => {
            ack(sealed_reply(join_gate::IntroReply::Refused {
                detail: "plane exited".into(),
            }))
            .await;
        }
    }
    true
}

/// Resolve the gate for a just-installed joiner: return the settled outcome if
/// the run loop already wrote one (a later retransmit), else forward the request
/// (the loop dedups per joiner) and report `Installed` while it settles. A
/// `None` gate always reports `Installed`.
///
/// outcome consumption is deliberate: `Admitted` STAYS in the map (idempotent
/// success — a lost ack's retransmit re-reads it for free), everything else is
/// taken ONE-SHOT — a joiner that retries this member later (a failed-over
/// `Busy`, a new attempt) must re-run the gate, not eat a stale refusal forever.
async fn gate_reply(
    gate: Option<&GateHook>,
    joiner: &ed25519::PublicKey,
    msg: &join_gate::IntroRequest,
) -> join_gate::IntroReply {
    let Some(hook) = gate else {
        return join_gate::IntroReply::Installed;
    };
    let joiner_key = joiner.as_ref().to_vec();
    let settled = {
        let mut outcomes = hook.outcomes.lock().expect("gate outcomes lock");
        match outcomes.get(&joiner_key) {
            Some(admitted @ join_gate::IntroReply::Admitted { .. }) => Some(admitted.clone()),
            Some(_) => outcomes.remove(&joiner_key),
            None => None,
        }
    };
    if let Some(outcome) = settled {
        return outcome;
    }
    let _ = hook
        .forward
        .send(join_gate::GateForward {
            issuer: msg.issuer.clone(),
            nonce: msg.nonce.clone(),
            token_sig: msg.token_sig.clone(),
            joiner: joiner_key,
            proof: msg.proof.clone(),
            expires_unix_secs: msg.expires_unix_secs,
        })
        .await;
    join_gate::IntroReply::Installed
}

/// the reachability plane's thread body: derive the plane's endpoints, bind
/// the nat client against the coordinated-reach coordinators, and drive
/// `reachability::run` on the in-process userspace backend. every failure
/// path prints and returns — the plane is an overlay on a working node,
/// never a reason to take the node down.
/// Wire the staged WireGuard reachability plane onto an already-registered
/// mesh channel: the orchestrator runs on its own plain-tokio OS thread (the
/// app-surface split exactly), and two pump tasks bridge it — mesh datagrams
/// in as `Deliver` commands, `Send` events out as mesh datagrams, everything
/// else printed as operator-visible progress. Returns the plane's command
/// sender. Shared by the validator path and the parked standby path (which
/// pre-warms its tunnels ahead of activation); the callers differ only in
/// where their `Retarget`/`ViewTick` commands come from.
#[allow(clippy::too_many_arguments)]
pub(crate) fn wire_reachability_plane<S, R>(
    context: &commonware_runtime::tokio::Context,
    label: &str,
    chain_id: &str,
    signer: &ed25519::PrivateKey,
    wireguard_key_file: &std::path::Path,
    mesh_state_file: &std::path::Path,
    wireguard_listen: std::net::SocketAddr,
    overlay_slot: overlay_net::userspace::StackSlot,
    advertised: Ingress,
    // an explicit WireGuard advertise override (node.toml
    // `wireguard_advertised`); `None` keeps today's derivation from
    // `wireguard_listen` (see `reachability_plane`'s body).
    wireguard_advertised: Option<Ingress>,
    coordinators: Vec<Ingress>,
    intro_listen: Option<std::net::SocketAddr>,
    // the genesis-issued admission capability presented on every coordinator
    // request (private coordination); `None` for a genesis validator, a public
    // coordinator, or the dev shape.
    coord_cap: Option<nat_traversal::CoordCap>,
    // the member side's gate hook (§4): the intro doorbells forward verified
    // gate requests to the validator run loop through it and answer settled
    // outcomes from its shared map. a joiner's own plane passes `None`.
    gate: Option<GateHook>,
    // the mesh ADDRESS seam: an accepted signed advert's control endpoint
    // lands in the book, and — when the effective address changed — is fed
    // to the lookup oracle's `overwrite`, which severs the stale connection
    // and redials at the new address. this replaces discovery's on-wire
    // address gossip outright.
    mesh_book: std::sync::Arc<crate::mesh_book::MeshAddressBook>,
    mesh_oracle: commonware_p2p::authenticated::lookup::Oracle<ed25519::PublicKey>,
    reach_p2p_tx: S,
    mut reach_p2p_rx: R,
    // the promotion seam: when armed, each pump hands its lane half back the
    // moment the plane exits (an orderly `Shutdown`), so a member-flavored
    // plane can be wired over the SAME registered channel in-process. `None`
    // = the lanes die with the process, exactly the pre-promotion validator
    // and sync-only shapes.
    lane_reclaim: Option<(
        futures::channel::oneshot::Sender<S>,
        futures::channel::oneshot::Sender<R>,
    )>,
) -> tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>
where
    S: P2pSender<PublicKey = ed25519::PublicKey> + Send + Sync + 'static,
    R: P2pReceiver<PublicKey = ed25519::PublicKey> + Send + 'static,
{
    let (tx_handback, rx_handback) = match lane_reclaim {
        Some((tx_handback, rx_handback)) => (Some(tx_handback), Some(rx_handback)),
        None => (None, None),
    };
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<reachability::ReachabilityCommand>(256);
    let (ev_tx, mut ev_rx) = tokio::sync::mpsc::channel::<reachability::ReachabilityEvent>(256);

    let thread_label = label.to_string();
    let reach_signer = signer.clone();
    let reach_coord_cap = coord_cap;
    let reach_gate = gate;
    let plane_chain_id = chain_id.to_string();
    let key_file = wireguard_key_file.to_path_buf();
    let state_file = mesh_state_file.to_path_buf();
    let nudge_tx = cmd_tx.clone();
    std::thread::Builder::new()
        .name("reachability".into())
        .spawn(move || {
            // default is one worker per core; this plane pumps a handful of
            // control-plane sockets and never needs that fan-out.
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("reachability tokio runtime")
                .block_on(reachability_plane(
                    thread_label,
                    plane_chain_id,
                    reach_signer,
                    key_file,
                    state_file,
                    wireguard_listen,
                    overlay_slot,
                    advertised,
                    wireguard_advertised,
                    coordinators,
                    intro_listen,
                    reach_coord_cap,
                    reach_gate,
                    cmd_rx,
                    nudge_tx,
                    ev_tx,
                ));
        })
        .expect("spawn reachability thread");

    // pump in: mesh datagrams -> orchestrator commands. exits the moment the
    // plane's command channel closes (an orderly Shutdown) instead of
    // lingering until the next inbound frame notices the dead plane, so an
    // armed handback fires promptly; a frame the exit's select drops was
    // addressed to a plane that no longer exists.
    {
        let cmd = cmd_tx.clone();
        context
            .child("reachability_in")
            .spawn(move |_ctx| async move {
                use futures::FutureExt as _;
                loop {
                    let frame = futures::select_biased! {
                        _ = std::pin::pin!(cmd.closed().fuse()) => None,
                        frame = reach_p2p_rx.recv().fuse() => Some(frame),
                    };
                    let Some(Ok((peer, msg))) = frame else { break };
                    let bytes: Vec<u8> = msg.into();
                    let deliver = reachability::ReachabilityCommand::Deliver { from: peer, bytes };
                    if cmd.send(deliver).await.is_err() {
                        break;
                    }
                }
                if let Some(handback) = rx_handback {
                    let _ = handback.send(reach_p2p_rx);
                }
            });
    }
    // pump out: orchestrator sends -> mesh; everything else is
    // operator-visible progress. the plane's exit closes the event channel,
    // so this pump drains the tail and (when armed) hands the sender back.
    {
        let pump_label = label.to_string();
        let book = mesh_book;
        let mut oracle = mesh_oracle;
        let mut tx = reach_p2p_tx;
        context
            .child("reachability_out")
            .spawn(move |_ctx| async move {
                while let Some(event) = ev_rx.recv().await {
                    match event {
                        reachability::ReachabilityEvent::Send { to, bytes } => {
                            let _ = tx.send(Recipients::One(to), IoBuf::from(bytes), false);
                        }
                        reachability::ReachabilityEvent::MeshReady { epoch, .. } => {
                            tracing::info!(
                                target: "ducktape::reachability",
                                node = %pump_label, epoch,
                                "mesh verified"
                            )
                        }
                        reachability::ReachabilityEvent::TunnelsApplied {
                            epoch,
                            interface,
                            peers,
                        } => {
                            // NB: this proves only that the EFFECT ACCEPTED A CONFIG. The
                            // completed-handshake half — the difference between "the
                            // overlay never came up" and "the overlay is up but the peer
                            // is dark" — is reported separately by `spawn_handshake_sampler`
                            // as `peer handshake COMPLETE` / `peer DARK`. Read them together:
                            // tunnels applied WITHOUT a matching handshake for a peer is
                            // precisely the "peer dark" bug.
                            tracing::info!(
                                target: "ducktape::reachability",
                                node = %pump_label, epoch, %interface, peers,
                                "tunnels applied (config accepted — the handshake is reported \
                                 separately)"
                            )
                        }
                        reachability::ReachabilityEvent::StandbyTunnelsApplied {
                            epoch,
                            interface,
                            peers,
                        } => tracing::info!(
                            target: "ducktape::reachability",
                            node = %pump_label, epoch, %interface, peers,
                            "standby pre-warm tunnels applied"
                        ),
                        reachability::ReachabilityEvent::InvitePeerInstalled {
                            peer,
                            interface,
                        } => {
                            tracing::info!(
                                target: "ducktape::reachability",
                                node = %pump_label,
                                peer = %hex_bytes(&peer.as_ref()[..4]),
                                %interface,
                                "invite tunnel installed"
                            )
                        }
                        reachability::ReachabilityEvent::PeerFailed { peer, reason } => {
                            // the peer is DARK. media to it will silently go nowhere.
                            tracing::warn!(
                                target: "ducktape::reachability",
                                node = %pump_label,
                                peer = %hex_bytes(&peer.as_ref()[..4]),
                                %reason,
                                "peer unreachable — traffic to it will go nowhere"
                            )
                        }
                        reachability::ReachabilityEvent::EpochFailed { epoch, reason } => {
                            tracing::error!(
                                target: "ducktape::reachability",
                                node = %pump_label, epoch, %reason,
                                "epoch FAILED — the mesh did not assemble"
                            )
                        }
                        reachability::ReachabilityEvent::MeshRestored {
                            epoch,
                            interface,
                            peers,
                        } => tracing::info!(
                            target: "ducktape::reachability",
                            node = %pump_label, epoch, %interface, peers,
                            "persisted mesh restored — awaiting live assembly"
                        ),
                        reachability::ReachabilityEvent::RestoreFailed { reason } => {
                            // #471: this was an unlevelled println that read like startup
                            // chatter — which is exactly why it sat there being ignored
                            // while restart-reconnect was dead. it is not chatter: the
                            // persisted mesh is GONE for this whole boot.
                            tracing::error!(
                                target: "ducktape::reachability",
                                node = %pump_label, %reason,
                                consequence = "restart reconnect is dead for this boot; \
                                               live assembly only",
                                "persisted mesh NOT restored"
                            )
                        }
                        reachability::ReachabilityEvent::PersistFailed { reason } => {
                            tracing::warn!(
                                target: "ducktape::reachability",
                                node = %pump_label, %reason,
                                consequence = "a cold restart will not restore this epoch",
                                "mesh state NOT persisted"
                            )
                        }
                        reachability::ReachabilityEvent::ControlEndpointObserved {
                            peer,
                            control_endpoint,
                        } => {
                            let Ok(peer_pk) = <ed25519::PublicKey as commonware_codec::DecodeExt<
                                _,
                            >>::decode(&peer.0[..]) else {
                                continue;
                            };
                            let Some(addr) = book.observe_advert(&peer_pk, control_endpoint) else {
                                // unchanged, or pinned by a DNS hint — silent.
                                continue;
                            };
                            let overwrite = commonware_utils::ordered::Map::from_iter_dedup([(
                                peer_pk.clone(),
                                addr,
                            )]);
                            let _ = oracle.overwrite(overwrite);
                            tracing::info!(
                                target: "ducktape::reachability",
                                node = %pump_label,
                                peer = %hex_bytes(&peer_pk.as_ref()[..4]),
                                "mesh address updated from signed advert"
                            );
                            tracing::debug!(
                                target: "ducktape::reachability",
                                node = %pump_label,
                                endpoint = %control_endpoint,
                                "updated mesh endpoint detail"
                            )
                        }
                    }
                }
                if let Some(handback) = tx_handback {
                    let _ = handback.send(tx);
                }
            });
    }
    cmd_tx
}

#[allow(clippy::too_many_arguments)]
async fn reachability_plane(
    label: String,
    chain_id: String,
    signer: ed25519::PrivateKey,
    wireguard_key_file: PathBuf,
    // where the plane persists each applied epoch's verified mesh and
    // re-applies it from at boot (the cold-restart path).
    mesh_state_file: PathBuf,
    wireguard_listen: std::net::SocketAddr,
    // the seam's stack handle (socket mode): created by the node so the mesh
    // context and the data-plane factory hold it BEFORE this thread exists;
    // the socket-mode effect publishes/clears the live stack through it.
    overlay_slot: overlay_net::userspace::StackSlot,
    advertised: Ingress,
    // an explicit WireGuard advertise override; see `wire_reachability_plane`.
    wireguard_advertised_override: Option<Ingress>,
    coordinators: Vec<Ingress>,
    // the invite intro listener: where a fresh joiner announces its keys
    // (token-authenticated) so its tunnel exists before any p2p.
    intro_listen: Option<std::net::SocketAddr>,
    // the genesis-issued admission capability presented on every coordinator
    // request (private coordination); `None` for a genesis validator, a public
    // coordinator, or the dev shape.
    coord_cap: Option<nat_traversal::CoordCap>,
    // the member side's gate hook (§4), cloned into both intro doorbells;
    // `None` on a joiner's plane.
    gate: Option<GateHook>,
    commands: tokio::sync::mpsc::Receiver<reachability::ReachabilityCommand>,
    // a clone of the `commands` sender, for the plane's own nudge ticker.
    nudges: tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>,
    events: tokio::sync::mpsc::Sender<reachability::ReachabilityEvent>,
) {
    use std::net::ToSocketAddrs as _;
    let policy = reachability::open_port_policy();
    // the plane's records carry IP literals only (the endpoint parser
    // rejects DNS); a hostname ingress resolves ONCE at plane start.
    let resolve_ingress = |ingress: &Ingress| match ingress {
        Ingress::Socket(addr) => Some(*addr),
        Ingress::Dns { host, port } => (host.as_str(), *port)
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.next()),
    };
    let Some(control_addr) = resolve_ingress(&advertised) else {
        // the plane never starts and the node runs on forever with NO overlay:
        // no tunnels, no hub, every huddle failing with a string that names none
        // of this. it does not self-heal and nothing else reports it.
        tracing::error!(
            target: "ducktape::reachability",
            node = %label,
            advertised = ?advertised,
            reason = "advertised_unresolvable",
            "reachability plane NOT started — this node has no overlay for the rest of \
             this boot (set `advertised` to a resolvable address)"
        );
        return;
    };
    let control_endpoint = match wireguard::Endpoint::new(
        control_addr.ip(),
        control_addr.port(),
        wireguard::Transport::Tcp,
        &policy,
    ) {
        Ok(endpoint) => endpoint,
        Err(err) => {
            tracing::error!(
                target: "ducktape::reachability",
                node = %label,
                error = ?err,
                reason = "control_endpoint_rejected",
                "reachability plane NOT started — this node has no overlay for the rest of \
                 this boot (set `advertised` to a dialable address)"
            );
            return;
        }
    };
    // an UNSPECIFIED wireguard_listen address (0.0.0.0/[::], cmd_join's
    // NAT'd-joiner default) means "bind the port, advertise NO endpoint":
    // the plane runs endpoint-less — peers install this node's tunnel
    // without an endpoint and this node's own initiations complete it
    // (WireGuard roams to the authenticated source). A concrete address
    // advertises exactly as before.
    if wireguard_listen.port() == 0 {
        tracing::error!(
            target: "ducktape::reachability",
            node = %label,
            reason = "wireguard_port_zero",
            "reachability plane NOT started; wireguard_listen needs a concrete UDP port"
        );
        return;
    }
    let wireguard_advertised = match &wireguard_advertised_override {
        // an explicit `wireguard_advertised` wins outright — the bind/
        // advertise split (change 3): resolved ONCE here, same discipline as
        // `advertised` above, independent of whether `wireguard_listen` is
        // itself unspecified.
        Some(ingress) => match resolve_ingress(ingress) {
            Some(addr) => match wireguard::Endpoint::new(
                addr.ip(),
                addr.port(),
                wireguard::Transport::Udp,
                &policy,
            ) {
                Ok(endpoint) => Some(endpoint),
                Err(err) => {
                    tracing::error!(
                        target: "ducktape::reachability",
                        node = %label,
                        error = ?err,
                        reason = "wireguard_advertised_rejected",
                        "reachability plane NOT started"
                    );
                    return;
                }
            },
            None => {
                tracing::error!(
                    target: "ducktape::reachability",
                    node = %label,
                    advertised = ?ingress,
                    reason = "wireguard_advertised_unresolvable",
                    "reachability plane NOT started"
                );
                return;
            }
        },
        // no override: derive from the bind address exactly like today —
        // unspecified means endpoint-less/roaming, concrete advertises itself.
        None if wireguard_listen.ip().is_unspecified() => None,
        None => match wireguard::Endpoint::new(
            wireguard_listen.ip(),
            wireguard_listen.port(),
            wireguard::Transport::Udp,
            &policy,
        ) {
            Ok(endpoint) => Some(endpoint),
            Err(err) => {
                tracing::error!(
                    target: "ducktape::reachability",
                    node = %label,
                    error = ?err,
                    reason = "wireguard_listen_rejected",
                    "reachability plane NOT started"
                );
                return;
            }
        },
    };
    let mut coords: Vec<std::net::SocketAddr> = Vec::new();
    for ingress in &coordinators {
        match resolve_ingress(ingress) {
            Some(addr) if !coords.contains(&addr) => coords.push(addr),
            Some(_) => {}
            None => tracing::warn!(
                target: "ducktape::reachability",
                node = %label,
                coordinator = ?ingress,
                reason = "coordinator_unresolvable",
                "coordinator skipped"
            ),
        }
    }
    let me = reachability::node_key(reachability::identity_of(&signer.public_key()));
    // the plane owns the underlay socket from PLANE START, not first
    // apply: the NAT client below rides it (reflexive discovery,
    // registration, keepalives, and the punch all originate from the
    // tunnel's own 5-tuple — the pinhole a punch opens is only good for the
    // socket it originated from), and it survives interface rebuilds so the
    // coordinator mapping stays warm while a tunnel is torn down/re-applied.
    let socket_underlay = match overlay_net::userspace::UnderlaySocket::bind(
        &tokio::runtime::Handle::current(),
        wireguard_listen.port(),
    ) {
        Ok(underlay) => underlay,
        Err(err) => {
            tracing::error!(
                target: "ducktape::reachability",
                node = %label,
                port = wireguard_listen.port(),
                error = %err,
                reason = "underlay_bind_failed",
                "reachability plane NOT started"
            );
            return;
        }
    };
    // the coordinated intro lane rides the shared underlay socket —
    // INCLUDING on a node that binds no direct intro listener below (a
    // NAT'd desktop's only join door is this lane).
    let (invite_intro_tx, invite_intro_rx) = tokio::sync::mpsc::channel(32);
    let (invite_intro_tx, mut invite_intro_rx) = (Some(invite_intro_tx), Some(invite_intro_rx));
    // authenticate every coordinator request: the node signs a
    // proof-of-possession with its identity key and, in private coordination,
    // carries the genesis-issued cap. A fully-open coordinator ignores the
    // authenticator; a public/private one requires it. With no coordinators
    // configured `bind` short-circuits to pass-through and never touches this.
    let resolver = match &socket_underlay {
        underlay if !coords.is_empty() => {
            let bypass = underlay
                .take_bypass()
                .expect("a fresh underlay socket still holds its bypass lane");
            let client =
                nat_traversal::NatSocket::shared(underlay.sender(), bypass).and_then(|sock| {
                    nat_traversal::NatClient::with_socket(
                        sock,
                        me,
                        coords.clone(),
                        signer.clone(),
                        coord_cap.clone(),
                    )
                });
            match client {
                // Establishment (reflexive discovery + registration) happens
                // inside the resolver's own task, retried with backoff — a
                // coordinator that is dark AT BOOT (machine woke before its
                // network, coordinator restarting) no longer costs this
                // process its rendezvous for life.
                Ok(client) => reachability::NatResolver::from_client_with_datagram_sink(
                    client,
                    reachability::RENDEZVOUS_KEEPALIVE,
                    invite_intro_tx,
                ),
                // A LOCAL wiring failure of the shared-socket seam itself —
                // not a network condition. Rendezvous cannot exist on this
                // socket, so degrade to pass-through: DIRECT / front
                // candidates (InstallInvitePeer + this node's own
                // initiations) need no rendezvous at all.
                Err(err) => {
                    tracing::warn!(
                        target: "ducktape::reachability",
                        node = %label,
                        error = %err,
                        reason = "rendezvous_socket_unusable",
                        "continuing without rendezvous; direct/front paths still work"
                    );
                    reachability::NatResolver::bind(me, Vec::new(), (signer.clone(), coord_cap))
                        .await
                        .expect("empty-coordinator pass-through resolver is infallible")
                }
            }
        }
        _ => {
            let auth = (signer.clone(), coord_cap.clone());
            match reachability::NatResolver::bind(me, coords.clone(), auth).await {
                Ok(resolver) => resolver,
                // bind can only fail LOCALLY now (its own UDP socket); an
                // unreachable coordinator is retried inside the resolver.
                Err(err) => {
                    tracing::warn!(
                        target: "ducktape::reachability",
                        node = %label,
                        error = %err,
                        reason = "rendezvous_socket_unusable",
                        "continuing without rendezvous; direct/front paths still work"
                    );
                    reachability::NatResolver::bind(me, Vec::new(), (signer.clone(), coord_cap))
                        .await
                        .expect("empty-coordinator pass-through resolver is infallible")
                }
            }
        }
    };
    // Establishment is asynchronous: narrate its transitions — one loud line
    // if the coordinator is dark at boot (so a self-healing plane is never
    // mistaken for a silently degraded one), then the reflexive when it
    // lands, however late.
    if let Some(mut status) = resolver.status() {
        let status_label = label.clone();
        tokio::spawn(async move {
            loop {
                let current = *status.borrow_and_update();
                match current {
                    reachability::RendezvousStatus::Ready { reflexive } => {
                        tracing::info!(
                            target: "ducktape::reachability",
                            node = %status_label,
                            %reflexive,
                            "coordinator-observed reflexive"
                        );
                        return;
                    }
                    reachability::RendezvousStatus::Unavailable { attempts: 1 } => {
                        tracing::warn!(
                            target: "ducktape::reachability",
                            node = %status_label,
                            attempts = 1,
                            reason = "coordinator_unavailable",
                            "coordinator rendezvous unavailable; retrying in the background"
                        );
                    }
                    _ => {}
                }
                if status.changed().await.is_err() {
                    return;
                }
            }
        });
    }
    // this member's WG keypair, shared into the intro doorbells so they can
    // OPEN a joiner's sealed first-contact intro (item 5). `load_or_generate`
    // is idempotent — the orchestrator below loads the same file, so a failure
    // here means the plane is unusable for inbound joins; log and disable the
    // listeners rather than take the node down (the plane is an overlay).
    let intro_keypair = match reachability::WireGuardKeypair::load_or_generate(&wireguard_key_file)
    {
        Ok((keypair, _)) => Some(std::sync::Arc::new(keypair)),
        Err(e) => {
            tracing::warn!(
                target: "ducktape::join",
                node = %label,
                path = %wireguard_key_file.display(),
                error = %e,
                reason = "wireguard_key_unreadable",
                "inbound joins via this node's invites are disabled"
            );
            None
        }
    };
    let config = reachability::ReachabilityConfig {
        chain_id,
        signer,
        wireguard_key_file,
        wireguard_port: wireguard_listen.port(),
        wireguard_advertised,
        control_endpoint,
        coordinators: coords,
        port_policy: policy,
        persist_file: Some(mesh_state_file),
        // the derived lobby transport identity is RETIRED (join ADR §4): a
        // joiner's gossip arrives under its REAL key — the mesh re-track at
        // its Redeem grant is what admits it.
        gossip_ingress: None,
    };
    // the invite intro listener: a fresh joiner's first contact. one
    // datagram carries the token, the joiner's identity + proof, and its
    // WireGuard key (identity-bound); a verified intro installs the
    // join-window tunnel peer (endpoint = the datagram's observed source —
    // WireGuard roams to the joiner's authenticated initiation anyway) and
    // the ack goes back only after the interface really carries it.
    // membership is NOT checked here (this task has no state access) — the
    // in-consensus redemption enforces it; a revoked member's token can at
    // worst open a tunnel that admits nothing.
    if intro_listen.is_none() {
        // resolve.rs already decided this config can never mint a direct
        // intro endpoint — say so once at boot instead of binding a
        // wildcard socket no joiner could ever reach.
        tracing::info!(
            target: "ducktape::join",
            node = %label,
            "no direct invite intro listener; intros arrive via the coordinated path"
        );
    }
    if let (Some(intro_addr), Some(intro_keypair)) = (intro_listen, intro_keypair.clone()) {
        let intro_cmds = nudges.clone().downgrade();
        let intro_label = label.clone();
        let intro_gate = gate.clone();
        // `chain_id` (the namespace string) moved into the plane config
        // above; the binding tokens sign over is those same bytes.
        let binding = config.chain_id.clone().into_bytes();
        tokio::spawn(async move {
            let socket = match tokio::net::UdpSocket::bind(intro_addr).await {
                Ok(socket) => socket,
                Err(err) => {
                    tracing::error!(
                        target: "ducktape::join",
                        node = %intro_label,
                        listen = %intro_addr,
                        error = %err,
                        reason = "intro_bind_failed",
                        "invite intro listener stopped; joins need another member"
                    );
                    return;
                }
            };
            tracing::info!(
                target: "ducktape::join",
                node = %intro_label,
                listen = %intro_addr,
                "invite intro listening"
            );
            let mut buf = vec![0u8; 4096];
            loop {
                let Ok((n, src)) = socket.recv_from(&mut buf).await else {
                    continue;
                };
                let ack = |bytes: Vec<u8>| {
                    let socket = &socket;
                    async move {
                        let _ = socket.send_to(&bytes, src).await;
                    }
                };
                if !handle_intro(
                    &buf[..n],
                    src,
                    &binding,
                    &intro_label,
                    IntroPath::Direct,
                    &intro_cmds,
                    |sealed| intro_keypair.open_sealed(sealed),
                    intro_gate.as_ref(),
                    ack,
                )
                .await
                {
                    break;
                }
            }
        });
    }
    if let (Some(mut invite_intro_rx), Some(intro_keypair)) =
        (invite_intro_rx.take(), intro_keypair.clone())
    {
        let intro_cmds = nudges.clone().downgrade();
        let intro_label = label.clone();
        let intro_gate = gate.clone();
        let binding = config.chain_id.clone().into_bytes();
        tokio::spawn(async move {
            while let Some((src, bytes)) = invite_intro_rx.recv().await {
                let ack = |ack_bytes: Vec<u8>| {
                    let cmds = intro_cmds.clone();
                    async move {
                        if let Some(cmds) = cmds.upgrade() {
                            let _ = cmds
                                .send(reachability::ReachabilityCommand::SendResolverDatagram {
                                    endpoint: src,
                                    bytes: ack_bytes,
                                })
                                .await;
                        }
                    }
                };
                if !handle_intro(
                    &bytes,
                    src,
                    &binding,
                    &intro_label,
                    IntroPath::Coordinated,
                    &intro_cmds,
                    |sealed| intro_keypair.open_sealed(sealed),
                    intro_gate.as_ref(),
                    ack,
                )
                .await
                {
                    break;
                }
            }
        });
    }

    // the boot `Retarget`'s record fan-out fires before the p2p actors have
    // a single live connection, and mesh sends are best-effort — when both
    // sides of a link lose that first datagram the plane deadlocks in record
    // gossip. the nudge re-offers un-acked gossip until the epoch assembles
    // (a no-op afterwards). the ticker holds only a WEAK sender: the plane's
    // exit is "every command sender dropped", and a strong clone here would
    // keep its own channel alive forever.
    let nudges = {
        let weak = nudges.downgrade();
        // the strong param must die NOW — holding it for the plane's
        // lifetime would itself keep the channel open.
        drop(nudges);
        weak
    };
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(NUDGE_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let Some(tx) = nudges.upgrade() else { break };
            if tx
                .send(reachability::ReachabilityCommand::Nudge)
                .await
                .is_err()
            {
                break;
            }
        }
    });
    let underlay = socket_underlay;
    tracing::info!(
        target: "ducktape::reachability",
        node = %label,
        backend = "userspace_socket",
        "reachability backend ready"
    );
    let effect = overlay_net::userspace::UserspaceWireGuardEffect::with_shared_underlay(
        tokio::runtime::Handle::current(),
        overlay_slot,
        underlay,
    );
    // take the probe BEFORE the effect is moved into the orchestrator.
    spawn_handshake_sampler(effect.probe_slot(), label.clone());
    if let Err(err) = reachability::run(config, effect, resolver, commands, events).await {
        tracing::error!(
            target: "ducktape::reachability",
            node = %label,
            error = %err,
            "reachability plane EXITED — this node has no overlay for the rest of \
             this boot"
        );
    }
}

/// how long without a completed handshake before a peer is called DARK.
///
/// WireGuard rekeys well inside this (REKEY_AFTER_TIME is 120s and the timer
/// pump drives retransmits), so exceeding it means the crypto handshake is not
/// completing at all — not that traffic is merely idle.
const HANDSHAKE_DARK_AFTER: Duration = Duration::from_secs(180);

/// Watch whether WireGuard handshakes actually COMPLETE, and say so on transition.
///
/// `TunnelsApplied` proves only that the effect ACCEPTED A CONFIG. Nothing in this
/// system proved a handshake ever completed — which is precisely the difference
/// between "the overlay never came up" and "the overlay is up but the peer is
/// dark". Those are two different bugs, and they presented as one string
/// ("Voice connection failed.") for days.
///
/// `WgDevice::time_since_last_handshake` existed the whole time — its doc even says
/// "for handshake probes" — but it was only ever called from tests, because the
/// device is owned by the effect and the effect is moved into the orchestrator.
/// `ProbeSlot` is the seam that fixes that (it mirrors the existing `StackSlot`).
///
/// Cost: this rides the EXISTING nudge tick and emits ONLY on a state transition.
/// Nothing is logged per packet, per handshake, or per tick.
fn spawn_handshake_sampler(probes: overlay_net::userspace::ProbeSlot, label: String) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(NUDGE_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // peer -> was it live at the last sample? the transition IS the event.
        let mut live: HashMap<std::net::Ipv6Addr, bool> = HashMap::new();
        loop {
            tick.tick().await;
            let Some(probe) = probes.get() else {
                // no backend yet: the overlay has not come up at all. that absence
                // is already reported by the plane's own bind/apply events; do not
                // duplicate it here every tick.
                continue;
            };
            let peers = probe.peers();
            for (ip, since) in &peers {
                let is_live = since.is_some_and(|elapsed| elapsed < HANDSHAKE_DARK_AFTER);
                match live.insert(*ip, is_live) {
                    Some(was) if was == is_live => {}
                    // first sight of a peer that is already handshaking, or a peer
                    // that recovered.
                    _ if is_live => tracing::info!(
                        target: "ducktape::reachability",
                        node = %label,
                        peer_ula = %ip,
                        since_handshake_s = since.map(|d| d.as_secs()),
                        "peer handshake COMPLETE — the tunnel is actually carrying traffic"
                    ),
                    // first sight of a peer that has never handshaked, or one that
                    // went dark. THIS is the line that was missing: config applied,
                    // crypto never completed, media silently going nowhere.
                    _ => tracing::warn!(
                        target: "ducktape::reachability",
                        node = %label,
                        peer_ula = %ip,
                        since_handshake_s = since.map(|d| d.as_secs()),
                        ever_handshaked = since.is_some(),
                        "peer DARK — its tunnel config is applied but no WireGuard \
                         handshake has completed; traffic to it is going nowhere"
                    ),
                }
            }
            // a peer removed from the table (epoch change) is not a transition.
            live.retain(|ip, _| peers.iter().any(|(seen, _)| seen == ip));
        }
    });
}
