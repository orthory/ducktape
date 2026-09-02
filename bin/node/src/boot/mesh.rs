use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::Ingress;
use commonware_p2p::authenticated::lookup::{self, Network};
use commonware_runtime::{Quota, Supervisor};
use commonware_utils::{NZU32, ordered::Set};

use crate::config::{self, hex_bytes};
use crate::constants::MAX_MESSAGE_SIZE;

/// The chain's overlay prefix, spelled the ONE way every reader of it must:
/// derived from the same namespace string the per-use planes' `OverlayBook`
/// and the reachability plane use.
fn overlay_router_for(namespace: &[u8]) -> overlay_net::OverlayRouter {
    overlay_net::OverlayRouter::for_prefix48(wireguard::ula_v6_prefix(&String::from_utf8_lossy(
        namespace,
    )))
}

/// Does this node hand out an address on the real network, or only its overlay
/// ULA? The answer decides whether the mesh listener carries a KERNEL leg
/// beside its virtual one — a node advertising only its ULA keeps the virtual
/// leg alone, because no legitimate dial could ever reach the other.
fn advertises_an_underlay_address(
    overlay_router: &overlay_net::OverlayRouter,
    advertised: &Ingress,
) -> bool {
    match advertised {
        Ingress::Socket(addr) => !overlay_router.is_overlay(addr),
        // a hostname advertisement is an underlay address by construction.
        Ingress::Dns { .. } => true,
    }
}

/// Whether `run_node` will really open an OS socket on `listen` — the
/// precondition for [`preflight_mesh_listen`] to mean anything.
///
/// The two inputs are exactly the ones `build` decides the backend from, and
/// this is why the predicate is a function rather than an inline expression:
/// a preflight that guessed differently would refuse a perfectly good
/// overlay-only node whose port happens to be busy for something else.
pub(crate) fn binds_an_os_mesh_socket(
    namespace: &[u8],
    advertised: &Ingress,
    overlay_enabled: bool,
) -> bool {
    !overlay_enabled || advertises_an_underlay_address(&overlay_router_for(namespace), advertised)
}

/// Take `listen` for a moment before the runtime does, so a port that is
/// already spoken for is a clean startup error instead of
/// `thread 'tokio-rt-worker' panicked … failed to bind listener: BindFailed`.
///
/// commonware's mesh listener binds INSIDE the runtime and `expect`s the
/// result, ~10 seconds into boot — after every other surface has logged itself
/// up — so the operator's reward for a taken port was a wall of healthy INFO
/// and then a raw Rust panic with a crates.io path in it.
///
/// ponytail: a preflight, so a socket stolen between this bind and the real one
/// still panics. That window is microseconds against the seconds-long boot it
/// replaces, and closing it properly means handing commonware a pre-bound
/// listener — a change to its API, not ours.
pub(crate) fn preflight_mesh_listen(listen: std::net::SocketAddr) -> Result<(), String> {
    crate::boot::surfaces::bind_listener("p2p mesh listener", "listen", &listen.to_string())
        .map(drop)
}

/// `run_node`'s shared runtime head (phase P3): the head of the async
/// closure `executor.start(|context| async move { … })` runs on — metrics
/// registration, the tracked mesh set, the statesync source pick,
/// lookup's config, the overlay-net seam, and the real
/// `Network`/`Oracle` pair. Ends before the `if sync_only {` branch,
/// which stays in `run_node` (that's the sync-only-vs-validator fork).
pub(crate) struct MeshHead {
    /// the closure's own root context, round-tripped: `commonware_runtime`'s
    /// `Context` has no `Clone`, and `NodeMetrics::register` must run on the
    /// SAME root context the closure was handed (child contexts prefix
    /// metric names) — so `build` only ever *borrows* it (via `&self`
    /// methods) and hands the identical value back rather than consuming it.
    pub(crate) context: commonware_runtime::tokio::Context,
    pub(crate) metrics: noded::NodeMetrics,
    /// The open-plane registry every per-use plane creator registers into;
    /// `plane_metrics` reads it at scrape time.
    pub(crate) plane_monitor: data_plane::PlaneMonitor,
    /// The registered `ducktape_dataplane_*` series — dropping this
    /// unregisters them, so it must live as long as the node runs.
    pub(crate) plane_metrics: crate::plane_metrics::PlaneMetrics,
    /// The statesync serve-lane registry the validator's serve task records
    /// every answered request into; `sync_metrics` reads it at scrape time.
    pub(crate) sync_monitor: statesync::monitor::ServeMonitor,
    /// The registered `ducktape_statesync_serve_*` series — same lifetime
    /// contract as `plane_metrics`.
    pub(crate) sync_metrics: crate::sync::metrics::SyncServeMetrics,
    pub(crate) mesh_participants: Set<ed25519::PublicKey>,
    pub(crate) status_public_key: String,
    pub(crate) sync_sources: Vec<ed25519::PublicKey>,
    pub(crate) sync_source: Option<ed25519::PublicKey>,
    pub(crate) advertised_reach: Ingress,
    pub(crate) network: Network<
        overlay_net::OverlayContext<commonware_runtime::tokio::Context>,
        ed25519::PrivateKey,
    >,
    pub(crate) oracle: lookup::Oracle<ed25519::PublicKey>,
    pub(crate) quota: Quota,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    context: commonware_runtime::tokio::Context,
    signer: ed25519::PrivateKey,
    namespace: Vec<u8>,
    peers: Vec<ed25519::PublicKey>,
    validators: Vec<ed25519::PublicKey>,
    sync_candidates: Vec<(ed25519::PublicKey, Ingress)>,
    listen: std::net::SocketAddr,
    advertised: Ingress,
    overlay_enabled: bool,
    overlay_slot: overlay_net::userspace::StackSlot,
) -> MeshHead {
    // the validator's own `ducktape_*` Prometheus series, registered on the
    // SAME runtime registry `context.encode()` (GET /metrics) serves — the
    // drain loop below folds each applied block in (height, count, apply
    // latency, per-module dispatch counters), so the networked node reports
    // the series the local daemon does and one Grafana board reads both.
    let metrics = noded::NodeMetrics::register(&context);

    // the open-plane registry + its `ducktape_dataplane_*` series, on the
    // same registry: every per-use plane (gateway, voice/video, agent
    // telemetry) registers itself here at bring-up, and each scrape reads
    // the live counters straight off the monitor.
    let plane_monitor = data_plane::PlaneMonitor::default();
    let plane_metrics = crate::plane_metrics::PlaneMetrics::register(&context, &plane_monitor);

    // the statesync serve-lane registry + its `ducktape_statesync_serve_*`
    // series: statesync rides the mesh carrier (never a data plane), so the
    // monitor above can't see it — the validator's serve task records every
    // answered request here instead, per requesting peer.
    let sync_monitor = statesync::monitor::ServeMonitor::new(context.child("statesync_monitor"));
    let sync_metrics = crate::sync::metrics::SyncServeMetrics::register(&context, &sync_monitor);

    // the authorized MESH set, SORTED — what the tracker windows ride on. the
    // consensus scheme uses the (possibly smaller) validator set derived
    // from committed valset state after the recovery boot below.
    let mesh_participants: Set<ed25519::PublicKey> =
        Set::try_from(peers.clone()).expect("authorized peer set has no duplicates");

    // this node's mesh identity, as served on /v1/status — clients stamp
    // it into peer-routed ops (chat's JoinHuddle.node).
    let status_public_key = hex_bytes(signer.public_key().as_ref());

    // the statesync source a --sync-only joiner pulls from: only
    // validators serve the channel, so the candidate must be a validator
    // that is not us (a non-validator hint or our own key would be
    // retried forever — the mesh never connects a node to itself).
    let sync_sources =
        config::sync_source_candidates(&sync_candidates, &validators, &signer.public_key());
    let sync_source = sync_sources.first().cloned();

    // the real encrypted TCP mesh, on `authenticated::lookup`: the
    // application supplies every peer's address (the MeshAddressBook), no
    // wire address gossip. `local` is the dev preset (allows private ips).
    // MUST be the real tokio runtime — the p2p actors live-lock under the
    // deterministic clock.
    //
    // TRANSPORT IDENTITY: every node — a parked joiner included — connects
    // under its REAL key (the derived lobby identity is retired).
    // a fresh joiner's key is untracked on every member until its `Redeem`
    // grant advances the membership generation, which the members' drains
    // track immediately — pre-admission it needs no mesh at all: the join
    // gate rides the WireGuard-tunnel doorbell, not a channel.
    let p2p_signer = signer.clone();
    // the staged reachability plane derives its advertised control endpoint
    // from the mesh `advertised`; lookup's config carries no self-address,
    // so the value survives whole as `advertised_reach`.
    let advertised_reach = advertised;
    let mut p2p_cfg = lookup::Config::local(p2p_signer, &namespace, listen, MAX_MESSAGE_SIZE);
    // EXPLICIT decision — authorization parity with the retired discovery
    // dialect: admission is the cryptographic handshake plus
    // key-in-a-tracked-set, source IP ignored. lookup's default source-IP
    // pinning would reject (a) the sentry forward-splice, which arrives
    // from the validator's real IP, not the advertised sentry address
    // (docs/deploy/sentry-deployment.md), (b) NAT'd members whose egress
    // differs from their advertised ingress, and (c) any DNS-hinted peer,
    // whose egress IP is unknowable pre-resolution. we forfeit the
    // pre-handshake IP allowlist (a cheap DoS filter); the handshake rate
    // limits stay on.
    p2p_cfg.bypass_ip_check = true;
    // the overlay-net seam: the mesh dials/binds through
    // a wrapper context whose Network routes BY ADDRESS — sockets on this
    // chain's ULA /48 go to the active overlay backend (today: the TUN
    // pass-through, i.e. the same OS socket the kernel routes through the
    // wireguard interface), everything else straight to the OS. the p2p
    // dialer never connect()s an overlay ULA on a raw OS socket as an
    // assumption again; the userspace backend lands behind this seam.
    // the prefix derives from the SAME namespace string the per-use planes'
    // OverlayBook and the reachability plane use, so all three agree on
    // what "overlay" means.
    let overlay_router = overlay_router_for(&namespace);
    // the backend follows the reachability plane. a configured plane routes
    // overlay dials/binds into the in-process virtual stack (and gives the
    // wildcard mesh listener its virtual leg); no plane keeps the OS
    // pass-through, so overlay dials just fail like a downed interface.
    //
    // socket mode's wildcard mesh bind normally carries the kernel OS leg
    // beside the virtual one — but a node that advertises ONLY its overlay
    // ULA hands out no underlay address anywhere (no bootstrap hint is
    // minted, gossip carries the ULA), so its kernel leg could never
    // receive a legitimate dial. it would sit unreachable as a wildcard
    // listener the host firewall alarms on (macOS prompts about every
    // wildcard bind) — such a node keeps the virtual leg only.
    let underlay_ingress = advertises_an_underlay_address(&overlay_router, &advertised_reach);
    let overlay_backend = if overlay_enabled {
        overlay_net::OverlayBackend::Userspace {
            slot: overlay_slot.clone(),
            underlay_ingress,
        }
    } else {
        overlay_net::OverlayBackend::Passthrough
    };
    let (network, oracle) = Network::new(
        overlay_net::OverlayContext::with_backend(
            context.child("network"),
            overlay_router,
            overlay_backend,
        ),
        p2p_cfg,
    );

    let quota = Quota::per_second(NZU32!(128));

    MeshHead {
        context,
        metrics,
        plane_monitor,
        plane_metrics,
        sync_monitor,
        sync_metrics,
        mesh_participants,
        status_public_key,
        sync_sources,
        sync_source,
        advertised_reach,
        network,
        oracle,
        quota,
    }
}
