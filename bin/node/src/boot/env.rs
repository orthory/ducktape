use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::Ingress;
use provider_host::SandboxBackend;

use crate::config::{self, Resolved, hex_bytes};

/// `run_node`'s boot-time config derivation (phase P0): the `Resolved`
/// destructure plus everything derived from it before the first listener
/// bind — the chain-id string, the mesh-state path, and the mesh ADDRESS
/// BOOK seeded from the config dial hints and the persisted mesh state.
pub(crate) struct BootEnv {
    pub(crate) signer: ed25519::PrivateKey,
    pub(crate) label: String,
    pub(crate) namespace: Vec<u8>,
    pub(crate) identity_chain_id: String,
    pub(crate) peers: Vec<ed25519::PublicKey>,
    pub(crate) validators: Vec<ed25519::PublicKey>,
    /// the mesh transport's address authority: hints + persisted adverts
    /// seeded here, live adverts observed by the reachability pump.
    /// (`sync_candidates` below carries the raw hint pairs onward.)
    pub(crate) mesh_book: std::sync::Arc<crate::mesh_book::MeshAddressBook>,
    pub(crate) coordinated: Vec<(ed25519::PublicKey, Ingress, ed25519::PublicKey)>,
    pub(crate) listen: SocketAddr,
    pub(crate) advertised: Ingress,
    pub(crate) storage: PathBuf,
    pub(crate) rpc_listen: Option<String>,
    pub(crate) http_listen: Option<String>,
    pub(crate) gateway_listen: Option<String>,
    pub(crate) wireguard_listen: Option<SocketAddr>,
    pub(crate) wireguard_key_file: PathBuf,
    pub(crate) invite_listen: Option<SocketAddr>,
    pub(crate) invite_token: Option<config::InviteToken>,
    pub(crate) invite_wireguard: Option<config::StoredInviteWireGuard>,
    pub(crate) invite_fronts: Vec<config::Front>,
    pub(crate) coordination: config::Coordination,
    pub(crate) coord_cap: Option<nat_traversal::CoordCap>,
    pub(crate) workspace: PathBuf,
    /// the AMBIENT coordinator override (`node.toml primary_coordinator`),
    /// raw — resolved via `config::coordinator_ingress` at each plane-wiring
    /// site so a bad value degrades there instead of aborting boot.
    pub(crate) primary_coordinator: Option<String>,
    /// the TCP relay override (`node.toml coordinator_relay`), raw — consumed
    /// by the joiner's first-contact wiring (join ADR item 2); `None` derives
    /// the relay from the ambient coordinator there.
    pub(crate) coordinator_relay: Option<String>,
    /// the WireGuard bind/advertise split (`node.toml wireguard_advertised`),
    /// threaded into `wire_reachability_plane` at both plane-wiring sites.
    pub(crate) wireguard_advertised: Option<Ingress>,
    pub(crate) sync_candidates: Vec<(ed25519::PublicKey, Ingress)>,
    pub(crate) chain_id: String,
    pub(crate) mesh_state_file: PathBuf,
    pub(crate) checkpoint_blocks: u64,
    pub(crate) dev_demo: bool,
    /// the operator's `[sandbox]` table — HOW a run is isolated on this host.
    /// `None` = consensus-only: no terminal plane, no runs. The node itself no
    /// longer executes provider work, so this drives the pty plane; the compute
    /// daemon resolves the same table for its own provider set.
    pub(crate) sandbox: Option<SandboxBackend>,
    /// the same table, gated on the user's `services.toml` compute grant —
    /// `None` when a sandbox is configured but compute was never enabled,
    /// which is the one thing the boot warning needs to know.
    pub(crate) compute_backend: Option<SandboxBackend>,
    /// the capacity a compute node announces: the same map the daemon resolves
    /// for its pool's ledger, so the scheduler can never be promised what this
    /// host cannot seat. EMPTY for a consensus-only node.
    pub(crate) sandbox_capacity: BTreeMap<String, u64>,
}

pub(crate) fn derive(resolved: Resolved, sync_only: bool) -> BootEnv {
    let Resolved {
        // the keyless half this node shares VERBATIM with its service daemons.
        // Destructured exhaustively (no `..`) on purpose: a new shared fact must
        // be routed here rather than silently ignored.
        service:
            config::ServiceConfig {
                workspace,
                storage_dir: storage,
                // the descriptor's own chain-id (network shape) or the raw
                // dev-shape namespace — NOT `namespace` below, which is
                // `genesis_namespace()` (chain_id@fingerprint) on the network
                // shape. this is the string the desktop app records as
                // `Workspace.chain_id`; threaded into `identity`'s certificate
                // domain separation.
                chain_id: identity_chain_id,
                http_listen,
                sandbox,
                sandbox_capacity,
            },
        signer,
        label,
        namespace,
        mesh: peers,
        validators,
        dial_hints,
        coordinated,
        listen,
        advertised,
        rpc_listen,
        gateway_listen,
        wireguard_listen,
        wireguard_key_file,
        invite_listen,
        dev_demo,
        checkpoint_blocks,
        invite_token,
        invite_wireguard,
        invite_fronts,
        coordination,
        coord_cap,
        primary_coordinator,
        coordinator_relay,
        wireguard_advertised,
        compute_backend,
        // not an env fact: the genesis wasm set is read off `Resolved` where
        // the module set is composed, never copied into the boot environment.
        genesis: _,
    } = resolved;
    // A key outside the GENESIS validator set is not an error: post-genesis
    // standing is admitted via governance and resolved from the latest
    // committed manifest at runtime. This cheap filesystem probe only decides
    // whether the fresh-joiner banner is truthful; the real recovery store is
    // opened later.
    let has_recovery_manifest = storage
        .join("recovery-manifest")
        .read_dir()
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    let fresh_joiner =
        !sync_only && !validators.contains(&signer.public_key()) && !has_recovery_manifest;
    if fresh_joiner {
        if invite_token.is_some() {
            tracing::info!(
                target: "ducktape::join",
                node = %label,
                invited = true,
                "joiner mode: announcing this key for automatic invite redemption"
            );
        } else {
            tracing::info!(
                target: "ducktape::join",
                node = %label,
                invited = false,
                "joiner mode: no invite token on disk; a member must grant standing manually"
            );
        }
    }

    // keep the raw (key, addr) pairs for statesync source selection; the
    // mesh reads the same hints through the address book below.
    let sync_candidates = dial_hints.clone();
    let chain_id = String::from_utf8_lossy(&namespace).to_string();
    let mesh_state_file = storage.join("mesh-state.json");
    // fail-closed check for private coordination: a node that is neither a
    // genesis validator (admitted by membership) nor holding a `coord.cap`
    // will have every rendezvous request silently dropped by a private
    // coordinator. Surface that loudly instead of pretending the plane is
    // healthy — the tunnels never come up, and the operator needs to know it
    // is a missing credential, not a network fault.
    if wireguard_listen.is_some()
        && !coordinated.is_empty()
        && coordination == config::Coordination::Private
        && coord_cap.is_none()
        && !validators.contains(&signer.public_key())
    {
        tracing::warn!(
            target: "ducktape::reachability",
            node = %label,
            reason = "coord_cap_missing",
            "private coordinator rendezvous will be denied; provide coord.cap or use a \
             fronted/direct reach hint"
        );
    }
    // the mesh ADDRESS BOOK: config dial hints as the operator tier, then
    // the persisted mesh's control endpoints (member adverts AND standby
    // records — a parked resident's endpoint is exactly what a member needs
    // at its grant re-track) as the weakest tier, so a member restarting
    // with every configured ingress gone still dials its peers. tier
    // precedence inside the book keeps a possibly-dead persisted address
    // from ever displacing a live operator-provided one. Load refusals
    // (tamper, format, chain) are the plane's to surface at its restore;
    // here they just mean no seeds.
    let default_mesh_port = config::DEFAULT_MESH_LISTEN
        .parse::<SocketAddr>()
        .expect("the default mesh listen parses")
        .port();
    let mesh_book = std::sync::Arc::new(crate::mesh_book::MeshAddressBook::new(
        chain_id.clone(),
        default_mesh_port,
    ));
    for (peer, ingress) in &dial_hints {
        mesh_book.seed_hint(peer.clone(), ingress.clone());
    }
    if let Ok(Some(mesh)) = reachability::store::load(&mesh_state_file, &chain_id) {
        let me = reachability::identity_of(&signer.public_key());
        let persisted_records = mesh
            .adverts
            .iter()
            .map(|advert| &advert.record)
            .chain(mesh.standby_records.iter().map(|signed| &signed.record))
            .filter(|record| record.validator_identity != me);
        let mut seeded = 0usize;
        for record in persisted_records {
            let Ok(pk) = ed25519::PublicKey::decode(&record.validator_identity.0[..]) else {
                continue;
            };
            mesh_book.seed_persisted(pk, record.control_endpoint.socket_addr());
            seeded += 1;
        }
        if seeded > 0 {
            tracing::info!(
                target: "ducktape::reachability",
                node = %label,
                seeds = seeded,
                epoch = mesh.epoch,
                "persisted mesh dial seeds restored"
            );
        }
    }

    for (i, pk) in peers.iter().enumerate() {
        tracing::debug!(
            target: "ducktape::node",
            node = %label,
            index = i,
            peer = %hex_bytes(pk.as_ref()),
            "mesh peer"
        );
    }
    // the first line of every node's life, and the one that ends a whole incident
    // class: a stale uplifted binary has faked at least four "regressions" here,
    // and the standing workaround was `strings <bin> | grep <a symbol you just
    // added>` — binaries were being dated by which bug they exhibited.
    //
    // deliberately NOT a build.rs git sha: cargo will not re-run a build script on
    // a commit, so the sha bakes in and goes STALE — it would lie during exactly
    // the stale-binary incident it exists to prevent (and `.git` is a FILE in every
    // worktree here, so the usual rerun-if-changed fix is fragile too). the exe's
    // own path + mtime is the mechanical equivalent of the manual workaround, and
    // it cannot go stale.
    let exe = std::env::current_exe().unwrap_or_default();
    let built_unix = std::fs::metadata(&exe)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_secs());
    tracing::info!(
        target: "ducktape::node",
        node = %label,
        version = env!("CARGO_PKG_VERSION"),
        profile = if cfg!(debug_assertions) { "debug" } else { "release" },
        binary = %exe.display(),
        built_unix,
        pid = std::process::id(),
        listen = %listen,
        namespace = %String::from_utf8_lossy(&namespace),
        storage = %storage.display(),
        peers = peers.len(),
        validators = validators.len(),
        sync_only,
        "node boot"
    );
    // coordinated reach targets are split OUT of the TCP mesh dialer (a
    // coordinator's UDP rendezvous port is not a TCP mesh peer — dialing it
    // there was a silent no-op). reaching them is the reachability plane's
    // job: gossip relays through whatever mesh links exist, the nat client
    // rendezvouses via the coordinator and hole-punches the WireGuard path, and
    // once tunnels apply the mesh dials the target's advertised overlay
    // address over the tunnel (the target sets `advertised = "overlay"`).
    // what still needs a TCP foothold is the gossip itself: with ZERO
    // bootstrap links nothing carries this node's records anywhere. a
    // RESTART has one without config: the persisted mesh re-applies at boot
    // and its control endpoints seeded the address book above. what remains
    // is the FIRST join (nothing persisted yet) on a coordinated-only
    // config — surface that loudly rather than park silently.
    if !coordinated.is_empty() {
        if dial_hints.is_empty() {
            tracing::warn!(
                target: "ducktape::reachability",
                node = %label,
                targets = coordinated.len(),
                reason = "no_bootstrap_link",
                "coordinated targets UNREACHABLE — add a direct/fronted bootstrap hint for the \
                 first join"
            );
        } else {
            tracing::info!(
                target: "ducktape::reachability",
                node = %label,
                targets = coordinated.len(),
                "coordinated reach configured"
            );
        }
        for (target, coord, _coord_key) in &coordinated {
            tracing::debug!(
                target: "ducktape::reachability",
                node = %label,
                peer = %hex_bytes(&target.as_ref()[..4]),
                coordinator = ?coord,
                "coordinated target"
            );
        }
    }
    if let Some(wg) = &wireguard_listen {
        // the backend is always the userspace socket stack now — no field.
        tracing::info!(
            target: "ducktape::reachability",
            node = %label,
            listen = %wg,
            endpoint_less = wg.ip().is_unspecified(),
            "reachability plane configured"
        );
    }

    BootEnv {
        signer,
        label,
        namespace,
        identity_chain_id,
        peers,
        validators,
        mesh_book,
        coordinated,
        listen,
        advertised,
        storage,
        rpc_listen,
        http_listen,
        gateway_listen,
        wireguard_listen,
        wireguard_key_file,
        invite_listen,
        invite_token,
        invite_wireguard,
        invite_fronts,
        coordination,
        coord_cap,
        workspace,
        primary_coordinator,
        coordinator_relay,
        wireguard_advertised,
        sync_candidates,
        chain_id,
        mesh_state_file,
        checkpoint_blocks,
        dev_demo,
        sandbox,
        compute_backend,
        sandbox_capacity,
    }
}
