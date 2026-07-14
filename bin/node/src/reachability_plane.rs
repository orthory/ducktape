use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::{Ingress, Receiver as P2pReceiver, Recipients, Sender as P2pSender};
use commonware_runtime::{IoBuf, Spawner, Supervisor};

use crate::config::{self, WireGuardEffectKind, hex_bytes};
use crate::constants::NUDGE_INTERVAL;
use crate::lobby;

/// Which doorbell an intro arrived on: the DIRECT UDP listener or the
/// COORDINATED (rendezvous-punched, resolver-socket) receiver.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum IntroPath {
    Direct,
    Coordinated,
}

/// One inviter-side intro datagram, shared by BOTH doorbells: decode →
/// verify → install → ack, in that order — the ack is only emitted after the
/// `InstallInvitePeer` reply settles, so "acked" can never outrun
/// "installed". `ack` abstracts the reply transport (the direct listener
/// answers on its own socket, the coordinated receiver via
/// `SendResolverDatagram`). Returns `false` once the plane's command channel
/// is gone, telling the caller to exit its receive loop.
pub(crate) async fn handle_intro<F, Fut>(
    bytes: &[u8],
    src: std::net::SocketAddr,
    binding: &[u8],
    label: &str,
    path: IntroPath,
    cmds: &tokio::sync::mpsc::WeakSender<reachability::ReachabilityCommand>,
    ack: F,
) -> bool
where
    F: FnOnce(Vec<u8>) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let Ok(msg) = lobby::decode_intro(bytes) else {
        return true; // junk on the doorbell — drop.
    };
    let ack_bytes = |installed: bool, detail: String| {
        lobby::encode_intro_ack(&lobby::IntroAck {
            nonce: msg.nonce.clone(),
            installed,
            detail,
        })
    };
    let verified = match lobby::verify_intro(&msg, binding) {
        Ok(v) => v,
        Err(e) => {
            // the direct doorbell answers a failed verification; the
            // coordinated path drops it silently (preserved pre-extraction
            // behavior — an unverified src has earned no resolver datagram).
            if path == IntroPath::Direct {
                ack(ack_bytes(false, e)).await;
            }
            return true;
        }
    };
    // expiry, on this member's wall clock: an expired token must not obtain a
    // tunnel either. `msg.expires_unix_secs` is signature-covered (verify just
    // proved it), so trusting the wire field here is trusting the token.
    if nat_traversal::now_secs() >= msg.expires_unix_secs {
        if path == IntroPath::Direct {
            ack(ack_bytes(
                false,
                "invite expired — ask the inviter for a fresh one".into(),
            ))
            .await;
        }
        return true;
    }
    // V8 (ADR §3.1): role supported. only `Resident` is redeemable this
    // generation; a `Client` token must not obtain a tunnel it can never
    // redeem (the lobby gate would refuse it terminally at Phase B anyway —
    // refuse here so a doomed join never gets a tunnel at all, R2). the raw
    // role byte is signature-covered (verify proved it) and any INVALID byte
    // was already rejected by `verify_intro`, so this only splits Resident
    // from Client. spent (V6) and issuer-in-valset (V7) need committed state
    // the transport plane does not hold — those stay enforced at Phase B.
    if msg.role != config::InviteRole::Resident.as_u8() {
        if path == IntroPath::Direct {
            ack(ack_bytes(
                false,
                "this invite role is not redeemable yet — the thin-client plane lands separately"
                    .into(),
            ))
            .await;
        }
        return true;
    }
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
            let flavor = match path {
                IntroPath::Direct => "",
                IntroPath::Coordinated => "coordinated ",
            };
            println!(
                "[node {label}] invite intro: {flavor}tunnel peer installed for {}",
                config::hex_bytes(&verified.joiner.as_ref()[..4])
            );
            ack(ack_bytes(true, "tunnel installed".into())).await;
        }
        Ok(Err(e)) => ack(ack_bytes(false, e)).await,
        Err(_) => ack(ack_bytes(false, "plane exited".into())).await,
    }
    true
}

/// the reachability plane's thread body: derive the plane's endpoints, bind
/// the nat client against the coordinated-reach coordinators, and drive
/// `reachability::run` with the configured WireGuard effect — real (an
/// actual interface via the userspace WireGuard runtime) by default,
/// in-memory fake when `wireguard_effect = "fake"` opts out. every failure
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
    wireguard_effect: WireGuardEffectKind,
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
    reach_p2p_tx: S,
    mut reach_p2p_rx: R,
) -> tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>
where
    S: P2pSender<PublicKey = ed25519::PublicKey> + Send + Sync + 'static,
    R: P2pReceiver<PublicKey = ed25519::PublicKey> + Send + 'static,
{
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<reachability::ReachabilityCommand>(256);
    let (ev_tx, mut ev_rx) = tokio::sync::mpsc::channel::<reachability::ReachabilityEvent>(256);

    let thread_label = label.to_string();
    let reach_signer = signer.clone();
    let reach_coord_cap = coord_cap;
    let plane_chain_id = chain_id.to_string();
    let key_file = wireguard_key_file.to_path_buf();
    let state_file = mesh_state_file.to_path_buf();
    let nudge_tx = cmd_tx.clone();
    std::thread::Builder::new()
        .name("reachability".into())
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
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
                    wireguard_effect,
                    overlay_slot,
                    advertised,
                    wireguard_advertised,
                    coordinators,
                    intro_listen,
                    reach_coord_cap,
                    cmd_rx,
                    nudge_tx,
                    ev_tx,
                ));
        })
        .expect("spawn reachability thread");

    // pump in: mesh datagrams -> orchestrator commands.
    {
        let cmd = cmd_tx.clone();
        context
            .child("reachability_in")
            .spawn(move |_ctx| async move {
                while let Ok((peer, msg)) = reach_p2p_rx.recv().await {
                    let bytes: Vec<u8> = msg.into();
                    let deliver = reachability::ReachabilityCommand::Deliver { from: peer, bytes };
                    if cmd.send(deliver).await.is_err() {
                        break;
                    }
                }
            });
    }
    // pump out: orchestrator sends -> mesh; everything else is
    // operator-visible progress.
    {
        let pump_label = label.to_string();
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
                                backend = ?wireguard_effect,
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
                    }
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
    effect_kind: WireGuardEffectKind,
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
        eprintln!(
            "[node {label}] reachability: wireguard_listen needs a concrete UDP port — plane \
             not started"
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
                    eprintln!(
                        "[node {label}] reachability: wireguard_advertised rejected ({err:?}) — \
                         plane not started"
                    );
                    return;
                }
            },
            None => {
                eprintln!(
                    "[node {label}] reachability: wireguard_advertised {ingress:?} did not \
                     resolve — plane not started"
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
                eprintln!(
                    "[node {label}] reachability: wireguard_listen rejected ({err:?}) — plane \
                     not started"
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
            None => eprintln!(
                "[node {label}] reachability: coordinator {ingress:?} did not resolve — skipped"
            ),
        }
    }
    let me = reachability::node_key(reachability::identity_of(&signer.public_key()));
    // socket mode owns the underlay socket from PLANE START, not first
    // apply: the NAT client below rides it (reflexive discovery,
    // registration, keepalives, and the punch all originate from the
    // tunnel's own 5-tuple — the pinhole a punch opens is only good for the
    // socket it originated from), and it survives interface rebuilds so the
    // coordinator mapping stays warm while a tunnel is torn down/re-applied.
    let socket_underlay = match effect_kind {
        WireGuardEffectKind::Socket => {
            match overlay_net::userspace::UnderlaySocket::bind(
                &tokio::runtime::Handle::current(),
                wireguard_listen.port(),
            ) {
                Ok(underlay) => Some(underlay),
                Err(err) => {
                    eprintln!(
                        "[node {label}] reachability: underlay udp/{} bind failed: {err} — \
                         plane not started",
                        wireguard_listen.port()
                    );
                    return;
                }
            }
        }
        WireGuardEffectKind::Tun | WireGuardEffectKind::Fake => None,
    };
    // the coordinated intro lane rides the shared underlay socket, so it
    // exists whenever that socket does — INCLUDING on a node that binds no
    // direct intro listener below (a NAT'd desktop's only join door is
    // this lane).
    let (invite_intro_tx, mut invite_intro_rx) = if socket_underlay.is_some() {
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };
    // authenticate every coordinator request: the node signs a
    // proof-of-possession with its identity key and, in private coordination,
    // carries the genesis-issued cap. A fully-open coordinator ignores the
    // authenticator; a public/private one requires it. With no coordinators
    // configured `bind` short-circuits to pass-through and never touches this.
    let resolver = match &socket_underlay {
        Some(underlay) if !coords.is_empty() => {
            let bypass = underlay
                .take_bypass()
                .expect("a fresh underlay socket still holds its bypass lane");
            let client =
                nat_traversal::NatSocket::shared(underlay.sender(), bypass).and_then(|sock| {
                    nat_traversal::NatClient::with_socket(
                        sock,
                        me,
                        coords.clone(),
                        Some(signer.clone()),
                        coord_cap,
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
                    eprintln!(
                        "[node {label}] reachability: rendezvous socket unusable ({err}) — \
                         continuing WITHOUT rendezvous (direct/front paths still work)"
                    );
                    reachability::NatResolver::bind(me, Vec::new(), None)
                        .await
                        .expect("empty-coordinator pass-through resolver is infallible")
                }
            }
        }
        _ => {
            let auth = Some((signer.clone(), coord_cap));
            match reachability::NatResolver::bind(me, coords.clone(), auth).await {
                Ok(resolver) => resolver,
                // bind can only fail LOCALLY now (its own UDP socket); an
                // unreachable coordinator is retried inside the resolver.
                Err(err) => {
                    eprintln!(
                        "[node {label}] reachability: rendezvous socket unusable ({err}) — \
                         continuing WITHOUT rendezvous (direct/front paths still work)"
                    );
                    reachability::NatResolver::bind(me, Vec::new(), None)
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
                        println!(
                            "[node {status_label}] reachability: coordinator-observed \
                             reflexive {reflexive}"
                        );
                        return;
                    }
                    reachability::RendezvousStatus::Unavailable { attempts: 1 } => {
                        eprintln!(
                            "[node {status_label}] reachability: coordinator rendezvous \
                             unavailable — retrying in the background (direct/front paths \
                             still work; coordinated-by-identity paths wake once a \
                             coordinator responds)"
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
    // a parked standby's gossip arrives under the network's derived lobby
    // identity (its own key is untracked until the grant cutover) — admit
    // that ingress; content signatures still authenticate every message.
    // the namespace is a TOML-sourced string, so `as_bytes` reproduces the
    // exact bytes the transport derived the lobby key from.
    let gossip_ingress = Some(config::lobby_identity(chain_id.as_bytes()).public_key());
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
        gossip_ingress,
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
        println!(
            "[node {label}] no direct invite intro listener (this config mints no direct \
             intro endpoint) — intros arrive via the coordinated path"
        );
    }
    if let Some(intro_addr) = intro_listen {
        let intro_cmds = nudges.clone().downgrade();
        let intro_label = label.clone();
        // `chain_id` (the namespace string) moved into the plane config
        // above; the binding tokens sign over is those same bytes.
        let binding = config.chain_id.clone().into_bytes();
        tokio::spawn(async move {
            let socket = match tokio::net::UdpSocket::bind(intro_addr).await {
                Ok(socket) => socket,
                Err(err) => {
                    eprintln!(
                        "[node {intro_label}] invite intro listener bind {intro_addr} failed: \
                         {err} — joins via this node's invites need another member"
                    );
                    return;
                }
            };
            println!("[node {intro_label}] invite intro listening on udp/{intro_addr}");
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
                    ack,
                )
                .await
                {
                    break;
                }
            }
        });
    }
    if let Some(mut invite_intro_rx) = invite_intro_rx.take() {
        let intro_cmds = nudges.clone().downgrade();
        let intro_label = label.clone();
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
    match effect_kind {
        WireGuardEffectKind::Fake => {
            println!(
                "[node {label}] reachability: wireguard_effect = \"fake\" — tunnel configs are \
                 recorded in memory; no real interface is touched"
            );
            if let Err(err) = reachability::run(
                config,
                wireguard::effect::FakeWireGuardEffect::default(),
                resolver,
                commands,
                events,
            )
            .await
            {
                eprintln!("[node {label}] reachability plane exited: {err}");
            }
        }
        WireGuardEffectKind::Socket => {
            let underlay = socket_underlay.expect("bound above for socket mode");
            println!(
                "[node {label}] reachability: driving the userspace socket backend (TUN-less; \
                 no interface, no privilege — overlay reachability lives inside this process)"
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
        WireGuardEffectKind::Tun => {
            #[cfg(unix)]
            {
                // same name the orchestrator writes into every
                // InterfaceConfiguration it applies — the WGApi handle and
                // the configs it receives must agree on the interface.
                let ifname = reachability::interface_name(&config.chain_id);
                let effect = match wireguard::effect::DefguardWireGuardEffect::new(&ifname) {
                    Ok(effect) => effect,
                    Err(err) => {
                        eprintln!(
                            "[node {label}] reachability: wireguard api handle for {ifname:?} \
                             failed ({err}) — plane not started; set wireguard_effect = \
                             \"fake\" to run without a real interface"
                        );
                        return;
                    }
                };
                println!("[node {label}] reachability: driving wireguard interface {ifname}");
                if let Err(err) =
                    reachability::run(config, effect, resolver, commands, events).await
                {
                    eprintln!("[node {label}] reachability plane exited: {err}");
                }
            }
            #[cfg(not(unix))]
            {
                eprintln!(
                    "[node {label}] reachability: the real wireguard effect needs a unix host — \
                     plane not started; set wireguard_effect = \"fake\" to run without a real \
                     interface"
                );
            }
        }
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
