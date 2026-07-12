use std::path::PathBuf;

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
        context.child("reachability_out").spawn(move |_ctx| async move {
            while let Some(event) = ev_rx.recv().await {
                match event {
                    reachability::ReachabilityEvent::Send { to, bytes } => {
                        let _ = tx.send(Recipients::One(to), IoBuf::from(bytes), false);
                    }
                    reachability::ReachabilityEvent::MeshReady { epoch, .. } => {
                        println!(
                            "[node {pump_label}] reachability: epoch {epoch} mesh verified"
                        )
                    }
                    reachability::ReachabilityEvent::TunnelsApplied {
                        epoch,
                        interface,
                        peers,
                    } => match wireguard_effect {
                        WireGuardEffectKind::Tun => println!(
                            "[node {pump_label}] reachability: epoch {epoch} tunnels applied \
                             on {interface} ({peers} peer(s))"
                        ),
                        // socket mode has no OS interface: {interface} is the
                        // orchestrator's label for the in-process backend.
                        WireGuardEffectKind::Socket => println!(
                            "[node {pump_label}] reachability: epoch {epoch} tunnels applied \
                             on {interface} ({peers} peer(s); userspace socket backend)"
                        ),
                        WireGuardEffectKind::Fake => println!(
                            "[node {pump_label}] reachability: epoch {epoch} tunnel config \
                             staged on {interface} ({peers} peer(s); fake effect — no real \
                             interface)"
                        ),
                    },
                    reachability::ReachabilityEvent::StandbyTunnelsApplied {
                        epoch,
                        interface,
                        peers,
                    } => println!(
                        "[node {pump_label}] reachability: epoch {epoch} standby pre-warm \
                         tunnels on {interface} ({peers} peer(s))"
                    ),
                    reachability::ReachabilityEvent::InvitePeerInstalled { peer, interface } => {
                        println!(
                            "[node {pump_label}] reachability: invite tunnel to {} on {interface}",
                            hex_bytes(&peer.as_ref()[..4])
                        )
                    }
                    reachability::ReachabilityEvent::PeerFailed { peer, reason } => {
                        println!(
                            "[node {pump_label}] reachability: peer {}: {reason}",
                            hex_bytes(&peer.as_ref()[..4])
                        )
                    }
                    reachability::ReachabilityEvent::EpochFailed { epoch, reason } => println!(
                        "[node {pump_label}] reachability: epoch {epoch} failed: {reason}"
                    ),
                    reachability::ReachabilityEvent::MeshRestored {
                        epoch,
                        interface,
                        peers,
                    } => println!(
                        "[node {pump_label}] reachability: persisted mesh (epoch {epoch}) \
                         restored on {interface} ({peers} peer(s)) — awaiting live assembly"
                    ),
                    reachability::ReachabilityEvent::RestoreFailed { reason } => {
                        println!(
                            "[node {pump_label}] reachability: persisted mesh not restored \
                             ({reason}); continuing on live assembly only"
                        )
                    }
                    reachability::ReachabilityEvent::PersistFailed { reason } => {
                        println!(
                            "[node {pump_label}] reachability: WARNING: mesh state not \
                             persisted ({reason}) — a cold restart will not restore this epoch"
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
        eprintln!(
            "[node {label}] reachability: advertised {advertised:?} did not resolve — plane \
             not started"
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
            eprintln!(
                "[node {label}] reachability: advertised control endpoint rejected ({err:?}) — \
                 set `advertised` to a dialable address; plane not started"
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
    let (invite_intro_tx, mut invite_intro_rx) =
        if socket_underlay.is_some() && intro_listen.is_some() {
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
            if let Err(err) = reachability::run(config, effect, resolver, commands, events).await {
                eprintln!("[node {label}] reachability plane exited: {err}");
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
