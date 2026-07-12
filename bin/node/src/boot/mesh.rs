use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::Ingress;
use commonware_p2p::authenticated::discovery::{self, Network};
use commonware_runtime::{Quota, Supervisor};
use commonware_utils::{NZU32, ordered::Set};

use crate::config::{self, WireGuardEffectKind, hex_bytes};
use crate::constants::MAX_MESSAGE_SIZE;

/// `run_node`'s shared runtime head (phase P3): the head of the async
/// closure `executor.start(|context| async move { … })` runs on — metrics
/// registration, the tracked mesh set, the statesync source pick, the lobby
/// transport identity, discovery's config, the overlay-net seam, and the
/// real `Network`/`Oracle` pair. Ends before the `if sync_only {` branch,
/// which stays in `run_node` (that's the sync-only-vs-validator fork).
pub(crate) struct MeshHead {
    /// the closure's own root context, round-tripped: `commonware_runtime`'s
    /// `Context` has no `Clone`, and `NodeMetrics::register` must run on the
    /// SAME root context the closure was handed (child contexts prefix
    /// metric names) — so `build` only ever *borrows* it (via `&self`
    /// methods) and hands the identical value back rather than consuming it.
    pub(crate) context: commonware_runtime::tokio::Context,
    pub(crate) metrics: noded::NodeMetrics,
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
    joiner: bool,
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
    // TRANSPORT IDENTITY: a parked joiner's own key is usually untracked
    // on every member (that is what admission changes), so it would be
    // bounced at the handshake and could neither announce itself nor poll
    // the statesync manifest. such a joiner connects AS the network's
    // derived LOBBY identity — the one key every member tracks that any
    // invite holder can derive. its REAL key still signs everything that
    // matters (the join proof, and consensus after the promotion reboot).
    // the door only exists where the mesh tracks it: the network shape
    // folds the lobby key into every member's mesh, so `peers` carries it
    // here too (both sides derive the same mesh). a mesh WITHOUT a lobby
    // key (the dev-seed shape) keeps the old behavior — the joiner parks
    // under its real identity, refused until the cutover re-tracks it.
    let lobby = config::lobby_identity(&namespace);
    let p2p_signer = if joiner && peers.contains(&lobby.public_key()) {
        lobby
    } else {
        signer.clone()
    };
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
    let overlay_backend = match wireguard_effect {
        WireGuardEffectKind::Socket => {
            overlay_net::OverlayBackend::Userspace(overlay_slot.clone())
        }
        WireGuardEffectKind::Tun | WireGuardEffectKind::Fake => {
            overlay_net::OverlayBackend::Tun
        }
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
