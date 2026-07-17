//! resolution of both config shapes into the one runnable form
//! (`Resolved`), plus the wireguard/advertised endpoint derivations.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use capability_host::SandboxBackend;
use commonware_cryptography::{Signer as _, ed25519};
use commonware_p2p::Ingress;

use super::identity::load_identity;
use super::node_toml::NodeToml;
use super::{
    Coordination, Front, InviteToken, NetworkDescriptor, ReachDial, SCHEME_ED25519,
    StoredInviteWireGuard, dialable, hex_bytes, ingress_of, load_coord_cap,
    load_invite_fronts, load_invite_token, load_invite_wireguard,
};

/// everything `run_node` needs, shape-independent.
#[derive(Debug)]
pub struct Resolved {
    pub signer: ed25519::PrivateKey,
    /// log prefix: "#<id>" for the dev shape, the identity's short hex
    /// otherwise.
    pub label: String,
    /// the chain-id (network shape) or legacy namespace bytes.
    pub namespace: Vec<u8>,
    /// this network's chain id — the descriptor's own `chain_id` field (network
    /// shape) or the raw configured namespace (dev shape, which has no
    /// fingerprint appended). NOT `namespace`: the network shape's `namespace`
    /// is `genesis_namespace()`, i.e. `chain_id@fingerprint` — a DIFFERENT
    /// string. This is the exact string the desktop app records as
    /// `Workspace.chain_id` (the `init` verb's last stdout line), so modules
    /// that must agree with the app on "this network's id" (e.g. `identity`'s
    /// certificate domain separation) use this field, never `namespace`.
    pub chain_id: String,
    /// the authorized mesh set (unsorted here; the caller builds the ordered
    /// Set discovery tracks).
    pub mesh: Vec<ed25519::PublicKey>,
    /// the genesis consensus participant subset.
    pub validators: Vec<ed25519::PublicKey>,
    /// (identity, dial ingress) pairs to dial at startup; never contains
    /// self. hostname ingresses stay hostnames — dialers re-resolve them.
    pub bootstrappers: Vec<(ed25519::PublicKey, Ingress)>,
    /// reach targets that need the nat client: (target key, coordinator
    /// ingress, coordinator key). empty unless a v3 invite carried Coordinated
    /// hints. the runtime rendezvous/hole-punches through the coordinator to
    /// each target, then authenticates the target's own key end-to-end.
    pub coordinated: Vec<(ed25519::PublicKey, Ingress, ed25519::PublicKey)>,
    pub listen: SocketAddr,
    /// this node's self-announced dial address. a HOSTNAME advertised stays a
    /// hostname all the way into the signed peer record, so a node behind a
    /// tunnel with a stable name never needs an address update — and it BOOTS
    /// even while its own name does not resolve.
    pub advertised: Ingress,
    pub storage_dir: PathBuf,
    pub rpc_listen: Option<String>,
    pub http_listen: Option<String>,
    pub gateway_listen: Option<String>,
    /// the staged WireGuard reachability plane's advertised UDP endpoint;
    /// None = plane off.
    pub wireguard_listen: Option<SocketAddr>,
    /// which `WireGuardEffect` the plane drives when it is on.
    pub wireguard_effect: WireGuardEffectKind,
    /// the DIRECT invite intro listener endpoint (`invite_listen`, defaulted
    /// from `wireguard_listen` + 1); `None` when the plane is off — or when
    /// this config can never mint a direct intro endpoint (no dialable
    /// underlay host), where binding it would only leave an unreachable
    /// wildcard socket. coordinated intros ride the plane's shared underlay
    /// socket regardless (see `reachability_plane.rs`).
    pub invite_listen: Option<SocketAddr>,
    /// where the node's X25519 WireGuard keypair persists (beside
    /// identity.key in the network shape).
    pub wireguard_key_file: PathBuf,
    /// dev-seed shape marker: gates the boot-time demo op + converged print
    /// (scaffolding a REAL network must not write into its genesis).
    pub dev_demo: bool,
    /// sealed blocks between recovery checkpoints.
    pub checkpoint_blocks: u64,
    /// the invite token a `join` stored beside the descriptor, if any — what a
    /// parked joiner announces over the lobby channel. always `None` for the
    /// dev shape and for manual (token-less) joins.
    pub invite_token: Option<InviteToken>,
    /// the inviter's WireGuard bootstrap a `join` stored, if any — the tunnel
    /// the joining node brings up BEFORE any p2p. always `None` for the dev
    /// shape and for members.
    pub invite_wireguard: Option<StoredInviteWireGuard>,
    /// the inviter's offered member fronts a `join` stored, if any — the
    /// ADDITIONAL first-contact paths the joiner races alongside the inviter.
    /// Empty for the dev shape, for members, and for pre-feature invites.
    pub invite_fronts: Vec<Front>,
    /// opt-in shipped-index warm start when joining; see `NodeToml::sync_index`.
    pub sync_index: bool,
    /// publish the discovered provider set into the capability registry; see
    /// `NodeToml::announce_capabilities`.
    pub announce_capabilities: bool,
    /// the reachability plane's coordination privacy (per-network operational
    /// policy). `Private` (the default) requires a genesis-issued `CoordCap`
    /// for a node outside the genesis validator set; `Public` accepts any
    /// proof-of-possession. The dev shape is always `Private` (it never uses a
    /// real coordinator).
    pub coordination: Coordination,
    /// the genesis-issued admission capability this node presents on every
    /// coordinator request (loaded from `coord.cap` beside the identity).
    /// `None` for a genesis validator (admitted by membership), the dev shape,
    /// or a node that has not been issued one.
    pub coord_cap: Option<nat_traversal::CoordCap>,
    /// the AMBIENT coordinator override (`NodeToml::primary_coordinator`),
    /// raw and unvalidated — resolved through `coordinator_ingress` at the
    /// point of use so a bad value DEGRADES (coordinated paths dark, loud
    /// log) rather than aborting boot. `None` = re-derive the compiled
    /// default, exactly like today.
    pub primary_coordinator: Option<String>,
    /// the WireGuard endpoint this node advertises, resolved once
    /// (`NodeToml::wireguard_advertised`); `None` = derive it from
    /// `wireguard_listen` exactly like today (see `reachability_plane.rs`).
    pub wireguard_advertised: Option<Ingress>,
    /// the workspace base directory — where `identity.key`, `network.toml`,
    /// `wireguard.key` and `coord.cap` live (the network shape's config
    /// directory; the dev shape's `storage_dir`). Threaded so a parked
    /// joiner's gate phase can persist a `coord.cap` delivered over its
    /// sealed `IntroReply::Admitted` ack via `save_coord_cap`.
    pub workspace: PathBuf,
    /// how provider runs are spawned (`NodeToml::sandbox`). `Direct` (the
    /// default) is the plain host spawn; `Podman` sandboxes every run AND
    /// makes this node announce `sandbox_capacity`.
    pub sandbox: SandboxBackend,
    /// the numeric capacity a sandboxed node announces alongside its tags
    /// (probed host totals, per-key overrides winning). EMPTY for a `Direct`
    /// node — a direct spawn makes no capacity promise. This one value is both
    /// the dispatch pool's ledger and the capability announce's resources.
    pub sandbox_capacity: BTreeMap<String, u64>,
}

/// resolve the operator's sandbox choice into a spawn backend plus the numeric
/// capacity a sandboxed node announces. absent/`"direct"` → `Direct` (no
/// capacity — a direct spawn makes no promise); `"podman"` → `Podman` with the
/// probed host totals, per-key overrides winning; `"tart"` → `Tart` (ephemeral
/// macOS VMs, same capacity model); anything else is a loud config error.
fn resolve_sandbox(raw: &NodeToml) -> Result<(SandboxBackend, BTreeMap<String, u64>), String> {
    // podman and tart share the capacity derivation: probed totals with the
    // operator's per-key overrides winning. the map is validated through THE
    // consensus rule (capability::validate_resources) before it leaves this
    // boundary: a zero override would otherwise pass boot, get announced,
    // and be rejected by the module — wedging the announcer's in-flight
    // latch with a false success log instead of erroring here, loudly.
    let probed = |raw: &NodeToml| -> Result<BTreeMap<String, u64>, String> {
        let mut capacity = crate::host_resources::probe();
        if let Some(cores) = raw.sandbox_cores {
            capacity.insert("cores".into(), cores);
        }
        if let Some(mem_gb) = raw.sandbox_mem_gb {
            capacity.insert("mem_gb".into(), mem_gb);
        }
        capability::validate_resources(&capacity)
            .map_err(|e| format!("sandbox capacity: {e}"))?;
        for dimension in ["cores", "mem_gb"] {
            if !capacity.contains_key(dimension) {
                return Err(format!(
                    "sandbox capacity: could not determine {dimension}; set sandbox_{dimension} explicitly"
                ));
            }
        }
        Ok(capacity)
    };
    match raw.sandbox.as_deref() {
        None | Some("direct") => Ok((SandboxBackend::Direct, BTreeMap::new())),
        Some("podman") => {
            let image = raw
                .sandbox_image
                .clone()
                .unwrap_or_else(|| "docker.io/library/node:22-slim".into());
            Ok((SandboxBackend::Podman { image }, probed(raw)?))
        }
        Some("tart") => {
            let image = raw
                .sandbox_image
                .clone()
                .unwrap_or_else(|| "ghcr.io/cirruslabs/macos-sonoma-base:latest".into());
            let capacity = probed(raw)?;
            if capacity.get("cores").copied().unwrap_or(0) < capability_host::TART_MIN_CORES {
                return Err(format!(
                    "sandbox capacity: Tart requires at least {} cores",
                    capability_host::TART_MIN_CORES
                ));
            }
            Ok((SandboxBackend::Tart { image }, capacity))
        }
        Some(other) => Err(format!(
            "sandbox: {other:?} is not \"direct\", \"podman\", or \"tart\""
        )),
    }
}

/// default recovery checkpoint cadence: small enough that boot replay stays
/// cheap, large enough that snapshotting the in-memory cohort is amortized.
pub const DEFAULT_CHECKPOINT_BLOCKS: u64 = 32;

fn absolute_runtime_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|e| format!("current directory: {e}"))
}

/// read + resolve a config file into its runnable form. paths inside the file
/// (network, key_file, storage_dir) resolve relative to the file's directory,
/// so a workspace directory is relocatable.
pub fn resolve(cfg_path: &Path) -> Result<Resolved, String> {
    let text = std::fs::read_to_string(cfg_path).map_err(|e| format!("read {cfg_path:?}: {e}"))?;
    let raw: NodeToml = toml::from_str(&text).map_err(|e| format!("{cfg_path:?}: {e}"))?;
    if raw.network.is_some() {
        let base = absolute_runtime_path(cfg_path.parent().unwrap_or_else(|| Path::new(".")))?;
        resolve_network_shape(&base, raw)
    } else {
        resolve_dev_shape(raw)
    }
}

fn resolve_network_shape(base: &Path, raw: NodeToml) -> Result<Resolved, String> {
    let descriptor_path = base.join(raw.network.as_deref().expect("checked by caller"));
    let descriptor = NetworkDescriptor::load(&descriptor_path)?;
    if descriptor.scheme != SCHEME_ED25519 {
        return Err(format!(
            "network {} uses scheme {:?}; this build runs {SCHEME_ED25519:?}",
            descriptor.chain_id, descriptor.scheme
        ));
    }
    let key_path = base.join(raw.key_file.as_deref().unwrap_or("identity.key"));
    let signer = load_identity(&key_path).map_err(|e| {
        format!("{e} — run `ducktape-node init` or `ducktape-node join <invite>` first")
    })?;
    let me = signer.public_key();

    let validators = descriptor.validator_keys()?;
    if validators.is_empty() {
        return Err(format!("network {} has no validators", descriptor.chain_id));
    }
    // one dial source of truth: reach_entries() folds bootstrap-synthesised
    // Direct hints in with the typed `reach` hints (their union). Direct/Fronted
    // resolve to a mesh Ingress dialed directly; Coordinated routes are handed
    // to the nat client, which rendezvouses through the coordinator and
    // hole-punches to the target — but the target is still authenticated
    // end-to-end by its own key, so a coordinated peer is a real mesh member
    // either way.
    let mut bootstrap: Vec<(ed25519::PublicKey, Ingress)> = Vec::new();
    let mut coordinated: Vec<(ed25519::PublicKey, Ingress, ed25519::PublicKey)> = Vec::new();
    for (key, dial) in descriptor.reach_entries()? {
        match dial {
            ReachDial::Direct(ingress) => bootstrap.push((key, ingress)),
            ReachDial::Coordinated { coord, coord_key } => coordinated.push((key, coord, coord_key)),
        }
    }
    // mesh = validators ∪ every reach identity (direct + coordinated). A
    // fresh network-shape joiner may be outside this set at genesis; it parks
    // until governance admits it (Join v2 §4: the gate rides the WireGuard-
    // tunnel doorbell, so a pre-admission joiner needs no mesh door — its
    // REAL key is re-tracked onto every member's mesh at its Redeem grant).
    let mut mesh = validators.clone();
    for (k, _) in &bootstrap {
        if !mesh.contains(k) {
            mesh.push(k.clone());
        }
    }
    for (k, _, _) in &coordinated {
        if !mesh.contains(k) {
            mesh.push(k.clone());
        }
    }

    let listen: SocketAddr = raw.listen.parse().map_err(|e| format!("listen: {e}"))?;
    let advertised = resolve_advertised(
        raw.advertised.as_deref(),
        listen,
        &descriptor.genesis_namespace(),
        &me,
    )?;
    let bootstrappers = bootstrap.into_iter().filter(|(k, _)| *k != me).collect();
    let wireguard_listen = parse_wireguard_listen(raw.wireguard_listen.as_deref())?;
    let wireguard_effect = parse_wireguard_effect(raw.wireguard_effect.as_deref())?;
    let invite_listen = resolved_intro_listener(&raw, wireguard_listen)?;
    let wireguard_advertised = parse_wireguard_advertised(raw.wireguard_advertised.as_deref())?;
    let (sandbox, sandbox_capacity) = resolve_sandbox(&raw)?;
    // Existing workspaces predate `gateway_listen`. Any node already exposing
    // the app surface gets the safe loopback/ephemeral gateway automatically;
    // no registry or node.toml migration (and no stale port) is required.
    let gateway_listen = raw
        .gateway_listen
        .clone()
        .or_else(|| raw.http_listen.as_ref().map(|_| "127.0.0.1:0".to_string()));

    Ok(Resolved {
        label: hex_bytes(&me.as_ref()[..4]),
        namespace: descriptor.genesis_namespace().into_bytes(),
        chain_id: descriptor.chain_id.clone(),
        signer,
        mesh,
        validators,
        bootstrappers,
        coordinated,
        listen,
        advertised,
        storage_dir: base.join(raw.storage_dir.as_deref().unwrap_or("storage")),
        rpc_listen: raw.rpc_listen,
        http_listen: raw.http_listen,
        gateway_listen,
        wireguard_listen,
        wireguard_effect,
        wireguard_key_file: base.join("wireguard.key"),
        invite_listen,
        dev_demo: false,
        checkpoint_blocks: raw.checkpoint_blocks.unwrap_or(DEFAULT_CHECKPOINT_BLOCKS),
        invite_token: load_invite_token(base)?,
        invite_wireguard: load_invite_wireguard(base)?,
        invite_fronts: load_invite_fronts(base)?,
        sync_index: raw.sync_index.unwrap_or(false),
        announce_capabilities: raw.announce_capabilities.unwrap_or(false),
        coordination: descriptor.coordination(),
        // the reachability plane presents this on every coordinator request; a
        // genesis validator needs none (admitted by membership), a joiner is
        // issued one beside its identity.
        coord_cap: load_coord_cap(base),
        // the config directory: identity.key / network.toml / coord.cap live
        // here, so a joiner persists a delivered cap into it.
        workspace: base.to_path_buf(),
        primary_coordinator: raw.primary_coordinator,
        wireguard_advertised,
        sandbox,
        sandbox_capacity,
    })
}

fn parse_wireguard_listen(raw: Option<&str>) -> Result<Option<SocketAddr>, String> {
    raw.map(|a| {
        a.parse::<SocketAddr>()
            .map_err(|e| format!("wireguard_listen: {e}"))
    })
    .transpose()
}

/// the parsed `wireguard_listen`, for callers working off a raw `NodeToml`
/// (the CLI verbs) rather than a full `resolve`.
pub fn resolved_wireguard_listen(raw: Option<&str>) -> Result<Option<SocketAddr>, String> {
    parse_wireguard_listen(raw)
}

/// resolve `wireguard_advertised` into a dial ingress: absent = "derive from
/// `wireguard_listen`" (the caller's job — see `reachability_plane.rs`), an
/// explicit value must be dialable (a hostname is kept VERBATIM, resolved
/// once at plane start, same discipline as the mesh `advertised`).
fn parse_wireguard_advertised(raw: Option<&str>) -> Result<Option<Ingress>, String> {
    match raw {
        None => Ok(None),
        Some(a) => ingress_of(a)
            .map_err(|e| format!("wireguard_advertised: {e}"))?
            .map(Some)
            .ok_or_else(|| format!("wireguard_advertised addr {a:?} is not dialable")),
    }
}

/// the DIRECT invite intro listener the plane binds: [`resolved_invite_listen`],
/// but only when this config can mint an invite that carries a direct intro
/// endpoint ([`endpoint_host`] — the exact predicate the minting side uses).
/// a node with no dialable underlay host (the desktop shape: `advertised =
/// "overlay"`, unspecified binds) hands joiners only the coordinated path, so
/// a kernel intro listener would sit unreachable by construction — while
/// tripping host firewall prompts (macOS asks about every wildcard bind).
fn resolved_intro_listener(
    raw: &NodeToml,
    wireguard_listen: Option<SocketAddr>,
) -> Result<Option<SocketAddr>, String> {
    let Some(wg) = wireguard_listen else {
        return Ok(None);
    };
    if endpoint_host(
        raw.advertised.as_deref(),
        &raw.listen,
        wg,
        raw.wireguard_advertised.as_deref(),
    )
    .is_err()
    {
        return Ok(None);
    }
    resolved_invite_listen(raw.invite_listen.as_deref(), wg).map(Some)
}

/// the invite intro listener endpoint: explicit `invite_listen`, else the
/// WireGuard listen address with the next port — one convention both the
/// minting side (what lands in the blob) and the serving side (what the
/// plane binds) derive identically.
pub fn resolved_invite_listen(
    raw: Option<&str>,
    wireguard_listen: SocketAddr,
) -> Result<SocketAddr, String> {
    match raw {
        Some(a) => a.parse().map_err(|e| format!("invite_listen: {e}")),
        None => {
            let port = wireguard_listen
                .port()
                .checked_add(1)
                .ok_or("wireguard_listen port has no successor for the intro default")?;
            Ok(SocketAddr::new(wireguard_listen.ip(), port))
        }
    }
}

/// the HOST a minted invite's UDP endpoints carry: an explicit
/// `wireguard_advertised` wins outright (it IS the truth once configured),
/// else the WireGuard listen IP when it is concrete, else the advertised
/// host (an invite must hand the joiner an underlay address that reaches
/// this machine — the usual listen is unspecified, so `advertised` is the
/// truth).
pub fn endpoint_host(
    advertised: Option<&str>,
    listen: &str,
    wireguard_listen: SocketAddr,
    wireguard_advertised: Option<&str>,
) -> Result<String, String> {
    if let Some(ingress) = parse_wireguard_advertised(wireguard_advertised)? {
        return Ok(match ingress {
            Ingress::Socket(addr) => addr.ip().to_string(),
            Ingress::Dns { host, .. } => host.to_string(),
        });
    }
    if !wireguard_listen.ip().is_unspecified() {
        return Ok(wireguard_listen.ip().to_string());
    }
    let dial = dialable(advertised, listen)?.ok_or(
        "no dialable host for the WireGuard invite endpoints: set `advertised` (or a \
         concrete wireguard_listen IP) so a joiner can reach this node's tunnel",
    )?;
    // strip the port: `host:port` or `[v6]:port`.
    match dial.rsplit_once(':') {
        Some((host, _)) => Ok(host.trim_matches(['[', ']']).to_string()),
        None => Ok(dial),
    }
}

/// the FULL `host:port` a minted invite's WireGuard `endpoint` carries: an
/// explicit `wireguard_advertised` is used VERBATIM — host AND port — because
/// in the port-forwarded setup the key exists for, the externally reachable
/// port can differ from the local bind port (`wireguard_listen`); baking the
/// advertised host with the bind port would mint an invite whose endpoint is
/// silently wrong. Absent, the endpoint is today's derivation exactly:
/// [`endpoint_host`]'s host at the bind port.
pub fn invite_wireguard_endpoint(
    advertised: Option<&str>,
    listen: &str,
    wireguard_listen: SocketAddr,
    wireguard_advertised: Option<&str>,
) -> Result<String, String> {
    if let Some(ingress) = parse_wireguard_advertised(wireguard_advertised)? {
        return Ok(match ingress {
            Ingress::Socket(addr) => addr.to_string(),
            Ingress::Dns { host, port } => format!("{host}:{port}"),
        });
    }
    let host = endpoint_host(advertised, listen, wireguard_listen, None)?;
    Ok(format!("{host}:{}", wireguard_listen.port()))
}

/// which `WireGuardEffect` implementation the reachability plane drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireGuardEffectKind {
    /// the TUN-less in-process backend (overlay-net ADR): BoringTun `Tunn`s
    /// + smoltcp behind the overlay seam, no privilege, no host mutation.
    Socket,
    /// configure an actual interface through the userspace WireGuard runtime.
    Tun,
    /// record configurations in memory without touching the network stack.
    Fake,
}

pub(super) fn parse_wireguard_effect(raw: Option<&str>) -> Result<WireGuardEffectKind, String> {
    match raw {
        Some("socket") => Ok(WireGuardEffectKind::Socket),
        // "real" predates the socket backend and stays as an alias for the
        // interface-backed path it always meant.
        None | Some("tun") | Some("real") => Ok(WireGuardEffectKind::Tun),
        Some("fake") => Ok(WireGuardEffectKind::Fake),
        Some(other) => Err(format!(
            "wireguard_effect: {other:?} is not \"socket\", \"tun\" (alias \"real\"), or \"fake\""
        )),
    }
}

/// resolve the `advertised` config value into a dial ingress. the sentinel
/// `"overlay"` advertises this node's chain-derived WireGuard overlay address
/// (`ula_v6_member_addr(namespace, identity)`) at the mesh listen port — the
/// address peers can dial once a tunnel to this node is up, and the RIGHT
/// advertisement for a member with no dialable underlay address (NAT, zero
/// exposed ports). the overlay is IPv6, so it requires an IPv6 mesh listener
/// (`listen = "[::]:port"` accepts both families on a default dual-stack
/// host); a v4-only listener would never see the tunnel's SYNs.
fn resolve_advertised(
    raw: Option<&str>,
    listen: SocketAddr,
    namespace: &str,
    me: &ed25519::PublicKey,
) -> Result<Ingress, String> {
    match raw {
        Some("overlay") => {
            if !listen.is_ipv6() {
                return Err(format!(
                    "advertised = \"overlay\" needs an IPv6 mesh listener to accept tunnel \
                     traffic — set listen = \"[::]:{}\"",
                    listen.port()
                ));
            }
            let identity = wireguard::ValidatorIdentity::try_from(me.as_ref())
                .map_err(|e| format!("advertised: {e:?}"))?;
            let ula = wireguard::ula_v6_member_addr(namespace, identity);
            Ok(Ingress::Socket(SocketAddr::new(
                std::net::IpAddr::V6(ula),
                listen.port(),
            )))
        }
        // an explicitly-configured advertised that can never be dialed is a
        // config error; a hostname is kept VERBATIM (no boot-time DNS).
        Some(a) => ingress_of(a)
            .map_err(|e| format!("advertised: {e}"))?
            .ok_or_else(|| format!("advertised addr {a:?} is not dialable")),
        None => Ok(Ingress::Socket(listen)),
    }
}

/// the dev-seed shape, replicating the historical semantics exactly: node 0
/// bootstraps nobody; everyone else dials peer_seeds[0] at bootstrapper_addr.
fn resolve_dev_shape(raw: NodeToml) -> Result<Resolved, String> {
    // resolved before any field of `raw` is moved out below (they borrow the
    // whole struct).
    let (sandbox, sandbox_capacity) = resolve_sandbox(&raw)?;
    let wireguard_listen = parse_wireguard_listen(raw.wireguard_listen.as_deref())?;
    let invite_listen = resolved_intro_listener(&raw, wireguard_listen)?;
    let id = raw
        .id
        .ok_or("a dev-shape config needs `id` (or add `network = ...`)")?;
    let namespace = raw
        .namespace
        .ok_or("a dev-shape config needs `namespace`")?;
    let peer_seeds = raw
        .peer_seeds
        .ok_or("a dev-shape config needs `peer_seeds`")?;
    let validator_seeds = raw
        .validator_seeds
        .clone()
        .unwrap_or_else(|| peer_seeds.clone());
    // duplicates would otherwise panic much later at run_node's Set::try_from.
    for (kind, seeds) in [
        ("peer_seeds", &peer_seeds),
        ("validator_seeds", &validator_seeds),
    ] {
        let mut seen = std::collections::BTreeSet::new();
        for s in seeds {
            if !seen.insert(*s) {
                return Err(format!("duplicate seed {s} in {kind}"));
            }
        }
    }

    let key_of = |s: u64| ed25519::PrivateKey::from_seed(s).public_key();
    let mesh: Vec<_> = peer_seeds.iter().map(|s| key_of(*s)).collect();
    let validators: Vec<_> = validator_seeds.iter().map(|s| key_of(*s)).collect();

    let bootstrappers = if id == 0 {
        Vec::new()
    } else {
        let boot_seed = *peer_seeds
            .first()
            .ok_or("a bootstrapping node needs peer_seeds[0] = node 0")?;
        let boot_addr: SocketAddr = raw
            .bootstrapper_addr
            .as_deref()
            .ok_or("a non-zero node needs bootstrapper_addr set")?
            .parse()
            .map_err(|e| format!("bootstrapper_addr: {e}"))?;
        // self-filter matches the Resolved.bootstrappers contract: a config
        // with peer_seeds[0] == id would otherwise dial (and statesync) itself.
        vec![(key_of(boot_seed), Ingress::Socket(boot_addr))]
            .into_iter()
            .filter(|(k, _)| *k != ed25519::PrivateKey::from_seed(id).public_key())
            .collect()
    };

    let listen: SocketAddr = raw.listen.parse().map_err(|e| format!("listen: {e}"))?;
    let advertised = resolve_advertised(
        raw.advertised.as_deref(),
        listen,
        &namespace,
        &ed25519::PrivateKey::from_seed(id).public_key(),
    )?;

    let storage_dir = match raw.storage_dir {
        Some(path) => absolute_runtime_path(Path::new(&path))?,
        None => std::env::temp_dir().join(format!("ducktape-node-{id}")),
    };
    let wireguard_effect = parse_wireguard_effect(raw.wireguard_effect.as_deref())?;
    let wireguard_advertised = parse_wireguard_advertised(raw.wireguard_advertised.as_deref())?;
    let gateway_listen = raw
        .gateway_listen
        .clone()
        .or_else(|| raw.http_listen.as_ref().map(|_| "127.0.0.1:0".to_string()));
    Ok(Resolved {
        signer: ed25519::PrivateKey::from_seed(id),
        label: format!("#{id}"),
        // the dev shape's namespace carries no fingerprint suffix (unlike the
        // network shape's `genesis_namespace()`), so it IS the chain id here.
        chain_id: namespace.clone(),
        namespace: namespace.into_bytes(),
        mesh,
        validators,
        bootstrappers,
        // the dev-seed shape never uses coordinated reach — direct sockets only.
        coordinated: Vec::new(),
        listen,
        advertised,
        // the dev shape has no identity.key directory; the wireguard key
        // lives with the node's other per-process state.
        wireguard_key_file: storage_dir.join("wireguard.key"),
        // the dev shape has no config directory; its per-process state dir
        // stands in as the workspace base (it never delivers a real cap).
        workspace: storage_dir.clone(),
        storage_dir,
        rpc_listen: raw.rpc_listen,
        http_listen: raw.http_listen,
        gateway_listen,
        wireguard_listen,
        wireguard_effect,
        invite_listen,
        dev_demo: true,
        checkpoint_blocks: raw.checkpoint_blocks.unwrap_or(DEFAULT_CHECKPOINT_BLOCKS),
        invite_token: None,
        invite_wireguard: None,
        invite_fronts: Vec::new(),
        sync_index: raw.sync_index.unwrap_or(false),
        announce_capabilities: raw.announce_capabilities.unwrap_or(false),
        // the dev shape wires direct sockets only — no real coordinator, so
        // the coordination mode defaults to Private and no cap is presented.
        coordination: Coordination::Private,
        coord_cap: None,
        primary_coordinator: raw.primary_coordinator,
        wireguard_advertised,
        sandbox,
        sandbox_capacity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{coordinator_ingress, load_or_generate_identity};

    const DELETED_CWD_CONFIGS: &str = "DUCKTAPE_TEST_DELETED_CWD_CONFIGS";

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ducktape-config-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn a_hostname_advertised_boots_without_dns_and_stays_a_hostname() {
        // the tunnel case: a stable name whose IP moves (or does not resolve
        // right now) must neither block boot nor be frozen to one lookup —
        // it stays a DNS ingress that dialing peers re-resolve every attempt.
        let dir = tmp("dnsadvertised");
        let (me, _) = load_or_generate_identity(&dir.join("identity.key")).expect("keygen");
        let d = NetworkDescriptor {
            chain_id: "net#44444444".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.public_key().as_ref())],
            bootstrap: vec![format!(
                "{}@definitely-not-resolvable.ducktape.invalid:443",
                hex_bytes(me.public_key().as_ref())
            )],
            reach: vec![],
            coordination: None,
        };
        d.save(&dir.join("network.toml")).expect("save");
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"127.0.0.1:52250\"\n\
             advertised = \"my-tunnel.example.com:443\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("hostnames never block boot");
        assert!(
            matches!(&r.advertised, Ingress::Dns { port: 443, .. }),
            "advertised stays a hostname: {:?}",
            r.advertised
        );
        // the unresolvable bootstrap hint is KEPT as a hostname too (self is
        // filtered from bootstrappers, so check via the descriptor directly).
        let entries = d.bootstrap_entries().expect("hints parse");
        assert!(
            matches!(&entries[0].1, Ingress::Dns { port: 443, .. }),
            "hint stays a hostname: {:?}",
            entries[0].1
        );
    }

    #[test]
    fn the_mesh_carries_no_derived_lobby_identity() {
        // Join v2 §4: the derived lobby transport identity is RETIRED — the
        // tracked mesh is exactly the descriptor's real identities, nothing
        // derivable from the namespace alone.
        let dir = tmp("lobbymesh");
        let (me, _) = load_or_generate_identity(&dir.join("identity.key")).expect("keygen");
        let d = NetworkDescriptor {
            chain_id: "net#33333333".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.save(&dir.join("network.toml")).expect("save");
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"127.0.0.1:52240\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        assert_eq!(
            r.mesh,
            vec![me.public_key()],
            "mesh = the descriptor's real identities only"
        );
    }

    #[test]
    fn a_non_wg_joiner_resolves_with_zero_reachability_config() {
        // the zero-config joiner contract, non-WG shape: a network-shape
        // config with NO `advertised` and a listen that is not dialable
        // (loopback-ephemeral — cmd_join's non-WG default plumbing) must
        // resolve: the joiner only ever dials OUT to the descriptor's reach
        // hints, so nothing may demand it be reachable itself.
        let dir = tmp("nonwgjoin");
        let (me, _) = load_or_generate_identity(&dir.join("identity.key")).expect("keygen");
        let founder = ed25519::PrivateKey::from_seed(7).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "net#44444444".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(founder.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.add_bootstrap(&founder, "203.0.113.7:41000");
        d.save(&dir.join("network.toml")).expect("save");
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"127.0.0.1:0\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect(
            "a joiner with no advertised and a non-dialable listen must resolve",
        );
        assert_eq!(r.signer.public_key(), me.public_key());
        assert_eq!(r.bootstrappers.len(), 1, "it dials the founder's hint");
    }

    #[test]
    fn network_shape_resolves_membership_and_bootstrap() {
        let dir = tmp("resolve");
        let key_path = dir.join("identity.key");
        let (me, _) = load_or_generate_identity(&key_path).expect("keygen");
        let other = ed25519::PrivateKey::from_seed(9).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "net#11223344".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![
                hex_bytes(me.public_key().as_ref()),
                hex_bytes(other.as_ref()),
            ],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.add_bootstrap(&other, "127.0.0.1:52200");
        d.save(&dir.join("network.toml")).expect("save descriptor");
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"127.0.0.1:52201\"\n",
        )
        .expect("write node.toml");

        let r = resolve(&dir.join("node.toml")).expect("resolve");
        // the running namespace is the chain-id plus the genesis fingerprint.
        assert_eq!(r.namespace, d.genesis_namespace().into_bytes());
        assert!(String::from_utf8_lossy(&r.namespace).starts_with("net#11223344@"));
        assert_eq!(r.validators.len(), 2);
        // exactly the validators — no derived lobby identity any more (the
        // join gate rides the tunnel doorbell, Join v2 §4).
        assert_eq!(r.mesh.len(), 2);
        // self never appears in bootstrappers; the other member does.
        assert_eq!(r.bootstrappers.len(), 1);
        assert_eq!(r.bootstrappers[0].0, other);
        assert!(!r.dev_demo);
        assert_eq!(r.signer.public_key(), me.public_key());
        assert_eq!(r.storage_dir, dir.join("storage"));
        // the workspace base is the config directory — where a joiner would
        // persist a `coord.cap` delivered over its Admitted gate reply.
        assert_eq!(r.workspace, dir);
    }

    #[test]
    fn relative_network_config_yields_absolute_runtime_paths() {
        let cwd = std::env::current_dir().expect("current directory");
        let workspace = tempfile::Builder::new()
            .prefix("ducktape-relative-config-")
            .tempdir_in(&cwd)
            .expect("workspace in current directory");
        let dir = workspace.path();
        let (me, _) = load_or_generate_identity(&dir.join("identity.key")).expect("keygen");
        NetworkDescriptor {
            chain_id: "relative#11223344".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        }
        .save(&dir.join("network.toml"))
        .expect("save descriptor");
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"127.0.0.1:52201\"\n\
             storage_dir = \"storage\"\n",
        )
        .expect("write node.toml");

        let relative_config = dir
            .strip_prefix(&cwd)
            .expect("workspace is below cwd")
            .join("node.toml");
        let resolved = resolve(&relative_config).expect("resolve relative config");

        assert_eq!(resolved.storage_dir, dir.join("storage"));
        assert!(resolved.storage_dir.is_absolute());
        assert_eq!(resolved.workspace, dir);
        assert!(resolved.workspace.is_absolute());
    }

    #[test]
    fn dev_shape_relative_storage_is_absolute_from_launch_cwd() {
        let launch_cwd = std::env::current_dir().expect("current directory");
        let raw: NodeToml = toml::from_str(
            "id = 7\nlisten = \"127.0.0.1:52220\"\nnamespace = \"demo\"\n\
             peer_seeds = [7]\nbootstrapper_addr = \"127.0.0.1:52220\"\n\
             storage_dir = \"relative/storage\"\n",
        )
        .expect("parse dev config");
        let resolved = resolve_dev_shape(raw).expect("resolve relative storage");
        assert_eq!(resolved.storage_dir, launch_cwd.join("relative/storage"));
        assert!(resolved.storage_dir.is_absolute());

        let default_raw: NodeToml = toml::from_str(
            "id = 8\nlisten = \"127.0.0.1:52220\"\nnamespace = \"demo\"\n\
             peer_seeds = [8]\nbootstrapper_addr = \"127.0.0.1:52220\"\n",
        )
        .expect("parse default dev config");
        let default = resolve_dev_shape(default_raw).expect("resolve default storage");
        assert_eq!(
            default.storage_dir,
            std::env::temp_dir().join("ducktape-node-8")
        );
        assert!(default.storage_dir.is_absolute());
    }

    #[cfg(unix)]
    #[test]
    fn absolute_configs_resolve_after_launch_cwd_is_deleted() {
        if let Ok(paths) = std::env::var(DELETED_CWD_CONFIGS) {
            let mut paths = paths.lines();
            let network_config = paths.next().expect("network config");
            let dev_config = paths.next().expect("dev config");
            let doomed_cwd = paths.next().expect("doomed cwd");

            std::env::set_current_dir(doomed_cwd).expect("enter doomed cwd");
            std::fs::remove_dir(doomed_cwd).expect("remove launch cwd");
            assert!(std::env::current_dir().is_err(), "cwd is genuinely unavailable");

            let network = resolve(Path::new(network_config)).expect("absolute network config");
            assert!(network.storage_dir.is_absolute());
            let dev = resolve(Path::new(dev_config)).expect("absolute dev config");
            assert!(dev.storage_dir.is_absolute());
            return;
        }

        let network_dir = tmp("deleted-cwd-network");
        let (me, _) =
            load_or_generate_identity(&network_dir.join("identity.key")).expect("keygen");
        NetworkDescriptor {
            chain_id: "deleted-cwd#11223344".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        }
        .save(&network_dir.join("network.toml"))
        .expect("save descriptor");
        let network_config = network_dir.join("node.toml");
        std::fs::write(
            &network_config,
            "network = \"network.toml\"\nlisten = \"127.0.0.1:52201\"\n",
        )
        .expect("write network config");

        let dev_dir = tmp("deleted-cwd-dev");
        let dev_config = dev_dir.join("node.toml");
        std::fs::write(
            &dev_config,
            "id = 0\nlisten = \"127.0.0.1:52220\"\nnamespace = \"demo\"\npeer_seeds = [0]\n",
        )
        .expect("write dev config");
        let doomed_cwd = tmp("deleted-cwd-launch");

        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "config::resolve::tests::absolute_configs_resolve_after_launch_cwd_is_deleted",
                "--nocapture",
            ])
            .env(
                DELETED_CWD_CONFIGS,
                format!(
                    "{}\n{}\n{}",
                    network_config.display(),
                    dev_config.display(),
                    doomed_cwd.display()
                ),
            )
            .output()
            .expect("run isolated deleted-cwd test");
        assert!(
            output.status.success(),
            "child failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn a_non_member_identity_resolves_as_a_pending_joiner() {
        let dir = tmp("nonmember");
        let (me, _) = load_or_generate_identity(&dir.join("identity.key")).expect("keygen");
        let other = ed25519::PrivateKey::from_seed(3).public_key();
        let d = NetworkDescriptor {
            chain_id: "closed#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(other.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.save(&dir.join("network.toml")).expect("save");
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"127.0.0.1:52202\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("non-member resolves as a joiner");
        assert_eq!(r.signer.public_key(), me.public_key());
        assert!(!r.validators.contains(&me.public_key()));
        assert_eq!(r.validators, vec![other.clone()]);
        // no derived lobby door any more (Join v2 §4): the joiner's own key
        // enters the mesh at its Redeem grant, not at resolve time.
        assert_eq!(r.mesh, vec![other]);
    }

    #[test]
    fn dev_shape_duplicate_seeds_are_a_config_error() {
        let dir = tmp("devdups");
        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52220\"\nnamespace = \"demo\"\npeer_seeds = [0, 1, 1]\n",
        )
        .expect("write");
        let err = resolve(&dir.join("node.toml")).expect_err("dup seeds refused");
        assert!(err.contains("duplicate seed"), "{err}");
    }

    #[test]
    fn announce_capabilities_defaults_off_and_parses_on() {
        let dir = tmp("announce");
        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52221\"\nnamespace = \"demo\"\npeer_seeds = [0]\n",
        )
        .expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("resolve default");
        assert!(
            !resolved.announce_capabilities,
            "serving is now opt-in: the default posture stays out of every rendezvous pool"
        );

        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52221\"\nnamespace = \"demo\"\npeer_seeds = [0]\n\
             announce_capabilities = true\n",
        )
        .expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("resolve opted-in");
        assert!(
            resolved.announce_capabilities,
            "true opts this node into serving its discovered providers"
        );
    }

    #[test]
    fn sandbox_parses_and_defaults_direct() {
        let dir = tmp("sandbox");
        let base = "id = 0\nlisten = \"127.0.0.1:52222\"\nnamespace = \"demo\"\npeer_seeds = [0]\n";

        // absent ⇒ Direct, and a direct node announces no capacity.
        std::fs::write(dir.join("node.toml"), base).expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("resolve default");
        assert_eq!(resolved.sandbox, SandboxBackend::Direct);
        assert!(
            resolved.sandbox_capacity.is_empty(),
            "a direct spawn makes no capacity promise"
        );

        // "direct" is the explicit spelling of the default.
        std::fs::write(
            dir.join("node.toml"),
            format!("{base}sandbox = \"direct\"\n"),
        )
        .expect("write");
        assert_eq!(
            resolve(&dir.join("node.toml")).expect("resolve").sandbox,
            SandboxBackend::Direct
        );

        // "podman" ⇒ Podman with the default image + probed capacity.
        std::fs::write(
            dir.join("node.toml"),
            format!("{base}sandbox = \"podman\"\n"),
        )
        .expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("resolve podman");
        assert_eq!(
            resolved.sandbox,
            SandboxBackend::Podman {
                image: "docker.io/library/node:22-slim".into()
            }
        );
        assert!(
            resolved.sandbox_capacity.get("cores").copied().unwrap_or(0) >= 1,
            "a podman node announces its probed capacity"
        );

        // an override wins over the probe; a custom image is honored.
        std::fs::write(
            dir.join("node.toml"),
            format!(
                "{base}sandbox = \"podman\"\nsandbox_image = \"docker.io/library/rust:1\"\n\
                 sandbox_cores = 99\nsandbox_mem_gb = 128\n"
            ),
        )
        .expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("resolve overrides");
        assert_eq!(
            resolved.sandbox,
            SandboxBackend::Podman {
                image: "docker.io/library/rust:1".into()
            }
        );
        assert_eq!(resolved.sandbox_capacity.get("cores"), Some(&99));
        assert_eq!(resolved.sandbox_capacity.get("mem_gb"), Some(&128));

        // "tart" resolves to the Tart backend with the default macOS image and
        // probed capacity (overrides win, same as podman).
        std::fs::write(dir.join("node.toml"), format!("{base}sandbox = \"tart\"\n")).expect("write");
        let tart = resolve(&dir.join("node.toml")).expect("tart accepted");
        assert_eq!(
            tart.sandbox,
            SandboxBackend::Tart {
                image: "ghcr.io/cirruslabs/macos-sonoma-base:latest".into()
            }
        );
        assert!(
            ["cores", "mem_gb"]
                .iter()
                .all(|dimension| tart.sandbox_capacity.contains_key(*dimension)),
            "both enforceable capacity dimensions ride Tart"
        );
        std::fs::write(
            dir.join("node.toml"),
            format!("{base}sandbox = \"tart\"\nsandbox_cores = 1\n"),
        )
        .expect("write");
        let err = resolve(&dir.join("node.toml")).expect_err("undersized Tart capacity refused");
        assert!(err.contains("requires at least 2 cores"), "{err}");

        // a zero capacity override is a loud BOOT error, not an announce-time
        // module reject: the consensus rule (validate_resources) runs at this
        // trust boundary too, so the announcer can never latch an unannouncable
        // set (the silent-wedge failure the review found).
        for zero in ["sandbox_cores = 0", "sandbox_mem_gb = 0"] {
            std::fs::write(
                dir.join("node.toml"),
                format!("{base}sandbox = \"podman\"\n{zero}\n"),
            )
            .expect("write");
            let err = resolve(&dir.join("node.toml")).expect_err("zero capacity refused");
            assert!(err.contains("sandbox capacity"), "{err}");
        }

        // any other value is a loud config error.
        std::fs::write(dir.join("node.toml"), format!("{base}sandbox = \"gvisor\"\n")).expect("write");
        let err = resolve(&dir.join("node.toml")).expect_err("unknown sandbox refused");
        assert!(err.contains("sandbox"), "{err}");
    }

    /// `primary_coordinator` (change 1, issue #331): the key ABSENT resolves
    /// to `None` — bit-identical to today, since `main.rs` re-derives the
    /// compiled default from `coordinator_ingress(None)` either way; the
    /// disable sentinel and an explicit override both ride the raw string
    /// through, unvalidated at resolve time (validated lazily at the point
    /// of use so a bad value degrades rather than aborting boot — inv 12).
    #[test]
    fn primary_coordinator_key_survives_resolve_default_absent_and_explicit() {
        let dir = tmp("primary-coordinator-key");
        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52260\"\nnamespace = \"demo\"\npeer_seeds = [0]\n",
        )
        .expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("resolve absent");
        assert_eq!(
            resolved.primary_coordinator, None,
            "absent key: re-derive the compiled default at the point of use"
        );

        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52260\"\nnamespace = \"demo\"\npeer_seeds = [0]\n\
             primary_coordinator = \"none\"\n",
        )
        .expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("resolve none");
        assert_eq!(resolved.primary_coordinator.as_deref(), Some("none"));
        assert_eq!(
            coordinator_ingress(resolved.primary_coordinator.as_deref()).expect("resolves"),
            None,
            "the persisted sentinel disables coordination on every future read"
        );

        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52260\"\nnamespace = \"demo\"\npeer_seeds = [0]\n\
             primary_coordinator = \"203.0.113.9:3478\"\n",
        )
        .expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("resolve override");
        assert_eq!(
            resolved.primary_coordinator.as_deref(),
            Some("203.0.113.9:3478")
        );
    }

    /// `wireguard_advertised` (change 3, issue #331): the key ABSENT resolves
    /// to `None` — `reachability_plane.rs` then derives it from
    /// `wireguard_listen` exactly like today; an explicit concrete override
    /// parses to a socket ingress, and a hostname stays a hostname (DNS
    /// deferred to plane start, same discipline as the mesh `advertised`).
    #[test]
    fn wireguard_advertised_key_absent_defaults_and_explicit_value_parses() {
        let dir = tmp("wg-advertised-key");
        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52270\"\nnamespace = \"demo\"\npeer_seeds = [0]\n\
             wireguard_listen = \"0.0.0.0:51820\"\n",
        )
        .expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("resolve absent");
        assert_eq!(
            resolved.wireguard_advertised, None,
            "absent key: reachability_plane.rs derives it from wireguard_listen, unchanged"
        );

        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52270\"\nnamespace = \"demo\"\npeer_seeds = [0]\n\
             wireguard_listen = \"0.0.0.0:51820\"\nwireguard_advertised = \"203.0.113.9:41820\"\n",
        )
        .expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("resolve override");
        assert_eq!(
            resolved.wireguard_advertised,
            Some(Ingress::Socket("203.0.113.9:41820".parse().unwrap()))
        );

        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52270\"\nnamespace = \"demo\"\npeer_seeds = [0]\n\
             wireguard_listen = \"0.0.0.0:51820\"\n\
             wireguard_advertised = \"definitely-not-resolvable.ducktape.invalid:41820\"\n",
        )
        .expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("hostnames never block boot");
        assert!(
            matches!(
                &resolved.wireguard_advertised,
                Some(Ingress::Dns { port: 41820, .. })
            ),
            "wireguard_advertised stays a hostname: {:?}",
            resolved.wireguard_advertised
        );
    }

    /// the desktop shape's kernel-socket posture: a config with no dialable
    /// underlay host (advertised = "overlay", unspecified binds) mints no
    /// direct intro endpoint (`cmd_invite`), so the resolved DIRECT intro
    /// listener is None — the plane binds no wildcard UDP socket (a macOS
    /// firewall prompt trigger) and joins ride the coordinated path. any
    /// mintable host keeps the listener.
    #[test]
    fn intro_listener_resolves_only_when_a_direct_endpoint_is_mintable() {
        let dir = tmp("intro-listener");
        let (me, _) = load_or_generate_identity(&dir.join("identity.key")).expect("keygen");
        let d = NetworkDescriptor {
            chain_id: "net#55555555".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.save(&dir.join("network.toml")).expect("save");

        // the desktop shape: overlay-advertised, unspecified binds.
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"[::]:52320\"\nadvertised = \"overlay\"\n\
             wireguard_listen = \"0.0.0.0:52323\"\ninvite_listen = \"0.0.0.0:52324\"\n\
             wireguard_effect = \"socket\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve desktop shape");
        assert_eq!(
            r.invite_listen, None,
            "no dialable underlay host → no direct intro listener"
        );

        // the port-forwarded NAT shape: wireguard_advertised keeps it.
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"[::]:52320\"\nadvertised = \"overlay\"\n\
             wireguard_listen = \"0.0.0.0:52323\"\ninvite_listen = \"0.0.0.0:52324\"\n\
             wireguard_effect = \"socket\"\nwireguard_advertised = \"203.0.113.9:41820\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve port-forwarded shape");
        assert_eq!(
            r.invite_listen,
            Some("0.0.0.0:52324".parse().unwrap()),
            "a dialable WG endpoint keeps the direct intro listener"
        );

        // the server shape: a concrete advertised keeps it, with the
        // wireguard_listen + 1 default.
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"[::]:52320\"\n\
             advertised = \"203.0.113.9:52320\"\nwireguard_listen = \"0.0.0.0:52323\"\n\
             wireguard_effect = \"socket\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve server shape");
        assert_eq!(
            r.invite_listen,
            Some("0.0.0.0:52324".parse().unwrap()),
            "a dialable advertised keeps the intro default (wireguard_listen + 1)"
        );
    }

    #[test]
    fn wireguard_advertised_rejects_an_unspecified_or_port_zero_value() {
        let dir = tmp("wg-advertised-bad");
        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52280\"\nnamespace = \"demo\"\npeer_seeds = [0]\n\
             wireguard_listen = \"0.0.0.0:51820\"\nwireguard_advertised = \"0.0.0.0:0\"\n",
        )
        .expect("write");
        let err = resolve(&dir.join("node.toml"))
            .expect_err("an explicit unspecified/port0 value is a config error, not a silent \
                          fallback to endpoint-less");
        assert!(err.contains("wireguard_advertised"), "{err}");
    }

    /// `endpoint_host` (change 3's invite-mint analog, config.rs ~2068): an
    /// explicit `wireguard_advertised` wins outright, including over a
    /// concrete `wireguard_listen` IP; absent falls back to today's
    /// derivation exactly (wireguard_listen IP, else `advertised`/`listen`).
    #[test]
    fn endpoint_host_prefers_wireguard_advertised_over_the_listen_derivation() {
        let unspecified: SocketAddr = "0.0.0.0:51820".parse().unwrap();
        let concrete: SocketAddr = "10.0.0.5:51820".parse().unwrap();

        assert_eq!(
            endpoint_host(None, "127.0.0.1:0", concrete, Some("203.0.113.9:9999")).unwrap(),
            "203.0.113.9",
            "wireguard_advertised wins even over a concrete wireguard_listen IP"
        );
        assert_eq!(
            endpoint_host(None, "127.0.0.1:0", unspecified, Some("tunnel.example.com:9999"))
                .unwrap(),
            "tunnel.example.com",
            "a hostname override stays a hostname"
        );
        assert_eq!(
            endpoint_host(None, "127.0.0.1:0", concrete, None).unwrap(),
            "10.0.0.5",
            "absent: today's derivation — the concrete wireguard_listen IP wins"
        );
        assert_eq!(
            endpoint_host(Some("203.0.113.1:443"), "127.0.0.1:0", unspecified, None).unwrap(),
            "203.0.113.1",
            "absent + unspecified listen: falls back to `advertised`/`listen`, unchanged"
        );
    }

    /// The invite-mint endpoint (review fix, change 3): with
    /// `wireguard_advertised` set the minted `endpoint` is the advertised
    /// value VERBATIM — host AND port. The port-forwarded scenario the key
    /// exists for: external 41820 forwarded to bind 51820 — baking the
    /// advertised host with the BIND port would silently mint a wrong
    /// endpoint. Absent, the derivation is bit-identical to before: the
    /// `endpoint_host` host at the bind port.
    #[test]
    fn invite_endpoint_uses_the_advertised_value_verbatim_including_its_port() {
        let unspecified: SocketAddr = "0.0.0.0:51820".parse().unwrap();
        let concrete: SocketAddr = "10.0.0.5:51820".parse().unwrap();

        assert_eq!(
            invite_wireguard_endpoint(None, "127.0.0.1:0", unspecified, Some("203.0.113.9:41820"))
                .unwrap(),
            "203.0.113.9:41820",
            "the advertised endpoint rides verbatim — 41820, never the bind port 51820"
        );
        assert_eq!(
            invite_wireguard_endpoint(
                None,
                "127.0.0.1:0",
                concrete,
                Some("tunnel.example.com:41820")
            )
            .unwrap(),
            "tunnel.example.com:41820",
            "a hostname override stays a hostname, with ITS port — even over a concrete bind IP"
        );
        assert_eq!(
            invite_wireguard_endpoint(None, "127.0.0.1:0", concrete, None).unwrap(),
            "10.0.0.5:51820",
            "absent: today's derivation exactly — the wireguard_listen IP at the bind port"
        );
        assert_eq!(
            invite_wireguard_endpoint(Some("203.0.113.1:443"), "127.0.0.1:0", unspecified, None)
                .unwrap(),
            "203.0.113.1:51820",
            "absent + unspecified listen: the advertised HOST at the WG bind port, unchanged"
        );
    }

    #[test]
    fn dev_shape_never_bootstraps_itself() {
        let dir = tmp("devself");
        std::fs::write(
            dir.join("node.toml"),
            "id = 1\nlisten = \"127.0.0.1:52230\"\nnamespace = \"demo\"\npeer_seeds = [1, 0]\nbootstrapper_addr = \"127.0.0.1:52231\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        assert!(
            r.bootstrappers.is_empty(),
            "peer_seeds[0] == id must not dial itself"
        );
    }

    #[test]
    fn dev_shape_matches_historical_semantics() {
        let toml = r#"
id = 1
listen = "127.0.0.1:52210"
namespace = "demo"
peer_seeds = [0, 1, 2]
validator_seeds = [0, 1]
bootstrapper_addr = "127.0.0.1:52200"
"#;
        let dir = tmp("dev");
        std::fs::write(dir.join("node.toml"), toml).expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        assert!(r.dev_demo);
        assert_eq!(r.label, "#1");
        assert_eq!(r.mesh.len(), 3);
        assert_eq!(r.validators.len(), 2);
        assert_eq!(r.bootstrappers.len(), 1);
        assert_eq!(
            r.bootstrappers[0].0,
            ed25519::PrivateKey::from_seed(0).public_key(),
            "non-zero nodes dial peer_seeds[0]"
        );
        assert_eq!(
            r.signer.public_key(),
            ed25519::PrivateKey::from_seed(1).public_key()
        );
    }

    #[test]
    fn wireguard_effect_defaults_tun_and_rejects_unknown_values() {
        let dir = tmp("wgeffect");
        let base = "id = 0\nlisten = \"127.0.0.1:52230\"\nnamespace = \"demo\"\npeer_seeds = [0]\n";
        std::fs::write(dir.join("node.toml"), base).expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        assert_eq!(r.wireguard_effect, WireGuardEffectKind::Tun);

        // "real" is the legacy alias for the interface-backed path.
        for spelled in ["tun", "real"] {
            std::fs::write(
                dir.join("node.toml"),
                format!("{base}wireguard_effect = \"{spelled}\"\n"),
            )
            .expect("write");
            let r = resolve(&dir.join("node.toml")).expect("resolve");
            assert_eq!(r.wireguard_effect, WireGuardEffectKind::Tun, "{spelled}");
        }

        std::fs::write(
            dir.join("node.toml"),
            format!("{base}wireguard_effect = \"socket\"\n"),
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        assert_eq!(r.wireguard_effect, WireGuardEffectKind::Socket);

        std::fs::write(
            dir.join("node.toml"),
            format!("{base}wireguard_effect = \"fake\"\n"),
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        assert_eq!(r.wireguard_effect, WireGuardEffectKind::Fake);

        std::fs::write(
            dir.join("node.toml"),
            format!("{base}wireguard_effect = \"simulated\"\n"),
        )
        .expect("write");
        let err = resolve(&dir.join("node.toml")).expect_err("unknown effect refused");
        assert!(err.contains("wireguard_effect"), "{err}");
    }

    #[test]
    fn overlay_advertised_derives_the_ula_and_requires_v6_listen() {
        let dir = tmp("overlay-advertised");
        let base = "id = 1\nnamespace = \"demo\"\npeer_seeds = [0, 1]\n\
                    bootstrapper_addr = \"127.0.0.1:52240\"\n";
        std::fs::write(
            dir.join("node.toml"),
            format!("{base}listen = \"[::]:52241\"\nadvertised = \"overlay\"\n"),
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        let identity = wireguard::ValidatorIdentity::try_from(
            ed25519::PrivateKey::from_seed(1).public_key().as_ref(),
        )
        .unwrap();
        let ula = wireguard::ula_v6_member_addr("demo", identity);
        assert_eq!(
            r.advertised,
            Ingress::Socket(SocketAddr::new(std::net::IpAddr::V6(ula), 52241)),
            "the overlay sentinel advertises the chain-derived ULA at the listen port"
        );

        // the overlay is v6: a v4-only listener would never see tunnel SYNs.
        std::fs::write(
            dir.join("node.toml"),
            format!("{base}listen = \"0.0.0.0:52241\"\nadvertised = \"overlay\"\n"),
        )
        .expect("write");
        let err = resolve(&dir.join("node.toml")).expect_err("v4 listener refused");
        assert!(err.contains("IPv6"), "{err}");
    }
}
