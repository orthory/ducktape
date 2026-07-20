use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::Ingress;
use commonware_p2p::authenticated::discovery::{self, Network};
use commonware_runtime::{Quota, Supervisor};
use commonware_utils::{NZU32, ordered::Set};

use crate::config::{self, WireGuardEffectKind, hex_bytes};
use crate::constants::MAX_MESSAGE_SIZE;

/// `run_node`'s shared runtime head (phase P3): the head of the async
/// closure `executor.start(|context| async move { … })` runs on — metrics
/// registration, the tracked mesh set, the statesync source pick,
/// discovery's config, the overlay-net seam, and the real
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
    pub(crate) network:
        Network<overlay_net::OverlayContext<commonware_runtime::tokio::Context>, ed25519::PrivateKey>,
    pub(crate) oracle: discovery::Oracle<ed25519::PublicKey>,
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
    bootstrappers: Vec<(ed25519::PublicKey, Ingress)>,
    wireguard_effect: WireGuardEffectKind,
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
    let sync_monitor = statesync::monitor::ServeMonitor::default();
    let sync_metrics = crate::sync::metrics::SyncServeMetrics::register(&context, &sync_monitor);

    // the authorized MESH set, SORTED — what discovery tracks. the
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
    // retried forever — discovery never connects a node to itself).
    let sync_sources =
        config::sync_source_candidates(&sync_candidates, &validators, &signer.public_key());
    let sync_source = sync_sources.first().cloned();

    // the real encrypted TCP mesh. `local` is the dev preset (allows private
    // ips). MUST be the real tokio runtime — discovery live-locks under the
    // deterministic clock.
    // reachability plane (docs/deploy/sentry-deployment.md): a forward sentry on a
    // private network relies on this `local` preset's allow_private_ips:true;
    // switching to a preset with allow_private_ips:false would reject the
    // forwarded connection from a private source IP — use a public-IP sentry
    // or a reverse tunnel then.
    //
    // TRANSPORT IDENTITY: every node — a parked joiner included — connects
    // under its REAL key (Join v2 §4; the derived lobby identity is retired).
    // a fresh joiner's key is untracked on every member until its `Redeem`
    // grant, when the members' drains re-track it onto the mesh immediately
    // (ahead of the epoch cutover) — pre-admission it needs no mesh at all:
    // the join gate rides the WireGuard-tunnel doorbell, not a channel.
    let p2p_signer = signer.clone();
    // the staged reachability plane derives its advertised control
    // endpoint from the mesh `advertised`; keep a copy — discovery's
    // config consumes the original.
    let advertised_reach = advertised.clone();
    let p2p_cfg = discovery::Config::local(
        p2p_signer,
        &namespace,
        listen,
        advertised,
        bootstrappers,
        MAX_MESSAGE_SIZE,
    );
    // the overlay-net seam (ADR 2026-07-07): the mesh dials/binds through
    // a wrapper context whose Network routes BY ADDRESS — sockets on this
    // chain's ULA /48 go to the active overlay backend (today: the TUN
    // pass-through, i.e. the same OS socket the kernel routes through the
    // wireguard interface), everything else straight to the OS. the p2p
    // dialer never connect()s an overlay ULA on a raw OS socket as an
    // assumption again; the userspace backend lands behind this seam.
    // the prefix derives from the SAME namespace string the per-use planes'
    // OverlayBook and the reachability plane use, so all three agree on
    // what "overlay" means.
    let overlay_router = overlay_net::OverlayRouter::for_prefix48(
        wireguard::ula_v6_prefix(&String::from_utf8_lossy(&namespace)),
    );
    // ADR phase 3: the backend follows `wireguard_effect`. socket mode
    // routes overlay dials/binds into the in-process virtual stack (and
    // gives the wildcard mesh listener its virtual leg); tun AND fake
    // keep the OS pass-through — fake stages no data plane at all, so
    // pass-through preserves its long-standing "overlay dials just fail
    // like a downed interface" behavior.
    //
    // socket mode's wildcard mesh bind normally carries the kernel OS leg
    // beside the virtual one — but a node that advertises ONLY its overlay
    // ULA hands out no underlay address anywhere (no bootstrap hint is
    // minted, gossip carries the ULA), so its kernel leg could never
    // receive a legitimate dial. it would sit unreachable as a wildcard
    // listener the host firewall alarms on (macOS prompts about every
    // wildcard bind) — such a node keeps the virtual leg only.
    let underlay_ingress = match &advertised_reach {
        Ingress::Socket(addr) => !overlay_router.is_overlay(addr),
        // a hostname advertisement is an underlay address by construction.
        Ingress::Dns { .. } => true,
    };
    let overlay_backend = match wireguard_effect {
        WireGuardEffectKind::Socket => overlay_net::OverlayBackend::Userspace {
            slot: overlay_slot.clone(),
            underlay_ingress,
        },
        WireGuardEffectKind::Tun | WireGuardEffectKind::Fake => overlay_net::OverlayBackend::Tun,
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
