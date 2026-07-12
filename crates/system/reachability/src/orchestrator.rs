//! The reachability orchestrator: the driver that takes epoch cutover events
//! from the node and turns them into a live WireGuard mesh — record gossip ->
//! signed advertisements -> `MeshView::verify` -> pairwise tunnel handshakes
//! -> ONE `apply_tunnel_plans` call per epoch through a `WireGuardEffect`.
//!
//! Runtime contract: the node runs [`run`] as the ROOT future of a dedicated
//! plain-tokio runtime on its own OS thread (the same split as the node's
//! app-surface thread), talking to the commonware runner over the two mpsc
//! channels. The future is not required to be `Send` — nothing here may
//! assume `tokio::spawn` onto a shared runtime.
//!
//! Phase-A scope, deliberately: the MEMBER record/advert set LOCKS once the
//! epoch's mesh version is computed — a member's mid-epoch re-advertisement
//! (NAT rebind, key rotation) retunnels at the NEXT cutover, not live. And a
//! member that never shows up stalls its epoch's bring-up exactly like
//! `MeshView::verify`'s all-members rule says it must; the previous epoch's
//! tunnels stay up meanwhile.
//!
//! STANDBY identities (the valset resident tier — registered, quorum-exempt
//! keys awaiting activation) ride a separate pre-warm layer with the
//! opposite trade: never versioned, never handshaked, applied LIVE. A
//! standby's owner-signed `EndpointRecord` for the current epoch installs a
//! tunnel by re-applying the full interface config mid-epoch (the same
//! record-derived trust model as the cold-restart restore: WireGuard key and
//! endpoint pinned under the owner's ed25519 signature, overlay routes
//! derived from `(chain_id, identity)`), and a higher-nonce
//! re-advertisement updates that peer in place. So the tunnels to a joining
//! node exist BEFORE its activation cutover — the new epoch's phase-A
//! assembly then replaces them with the verified, versioned mesh.
//!
//! Transport reach is NOT assumed pairwise: two members with no direct mesh
//! link (a coordinated-only joiner parked through one ingress) still
//! assemble, because every message is relayed by the members that do have
//! links — records/adverts flood with nonce dedup, handshake messages ride
//! per-pair relay slots — and every message authenticates by its OWNER's
//! content signature, never by the link it arrived on.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use commonware_cryptography::{Signer as _, ed25519};
use nat_traversal::{ClientEvent, NatClient, NodeKey, SocketEvent};
use tokio::sync::mpsc;
use wireguard::effect::{
    PeerTunnelConfig, WireGuardEffect, apply_peer_tunnels, plan_peer_configs, update_peer_tunnels,
};
use wireguard::{
    ActiveValidatorSet, Endpoint, EndpointAdvertisement, EndpointRecord, MeshVersion, MeshView,
    OverlayPolicy, Perspective, PortPolicy, ReplayCache, SignedEndpointRecord, TunnelInstallPlan,
    TunnelUpgradeAck, TunnelUpgradeAckFields, TunnelUpgradeRequest, TunnelUpgradeRequestFields,
    TunnelUpgradeResponse, TunnelUpgradeResponseFields, UpgradeError, ValidatorIdentity,
    compute_mesh_version,
};

use crate::binding;
use crate::keys::{KeyError, WireGuardKeypair};
use crate::msg::{MsgError, ReachabilityMsg};
use crate::store::{self, PersistedMesh};

/// Views an advertisement stays valid for past the cutover view that minted
/// it. Generous: a re-advertisement (NAT rebind, key rotation) supersedes by
/// nonce long before expiry matters.
pub const ADVERT_TTL_VIEWS: u64 = 10_000;

/// Views a handshake message stays valid for. Tight relative to
/// [`ADVERT_TTL_VIEWS`]: a handshake is a live conversation, not a standing
/// record.
pub const HANDSHAKE_TTL_VIEWS: u64 = 500;

/// WireGuard persistent keepalive for every mesh peer: NAT mappings on the
/// punched path die in tens of seconds of silence, and a consensus mesh can
/// legitimately idle a data tunnel that long.
pub const KEEPALIVE_SECONDS: u16 = 25;

/// For each unordered member pair exactly ONE side runs the handshake, and
/// both sides agree which from public data alone: the lexicographically
/// lower identity initiates.
pub fn initiates(local: ValidatorIdentity, peer: ValidatorIdentity) -> bool {
    local.0 < peer.0
}

/// Everything the node resolves ONCE at boot and hands the orchestrator.
pub struct ReachabilityConfig {
    /// The chain id — doubles as the advertisement namespace and the ULA
    /// derivation input, exactly as it does for the commonware mesh.
    pub chain_id: String,
    /// The node's ed25519 identity: signs records, advertisements, and
    /// handshake messages. Its public key IS the member identity.
    pub signer: ed25519::PrivateKey,
    /// Where the X25519 keypair lives (beside `identity.key`);
    /// `keys::WireGuardKeypair::load_or_generate` runs against this path.
    pub wireguard_key_file: PathBuf,
    /// The local WireGuard UDP bind port — always needed to bring the
    /// interface up, independent of whether an endpoint is advertised.
    pub wireguard_port: u16,
    /// The node's own advertised WireGuard UDP endpoint — `None` for an
    /// endpoint-less (NAT'd) node: it advertises no address, installs every
    /// peer FROM the records, and initiates; peers install it without an
    /// endpoint and WireGuard roams to its authenticated initiation.
    pub wireguard_advertised: Option<Endpoint>,
    /// The node's own advertised control-mesh endpoint.
    pub control_endpoint: Endpoint,
    /// Rendezvous coordinators (from `Resolved.coordinated`), possibly none:
    /// with an empty list every peer resolves to its advertised endpoint.
    pub coordinators: Vec<SocketAddr>,
    /// The endpoint policy advertisements and handshakes validate against.
    pub port_policy: PortPolicy,
    /// Where the last applied epoch's verified mesh is persisted (and read
    /// back for the cold-restart re-apply). `None` disables persistence —
    /// the plane then only ever assembles from live gossip.
    pub persist_file: Option<PathBuf>,
    /// A transport identity whose DELIVERIES are admitted even though it is
    /// no plane participant: the mesh's derived lobby key, which a parked
    /// standby connects under while its own key is still untracked. Purely
    /// an ingress allowance — every message still authenticates by its
    /// owner's content signature, and standby-directed replies route back
    /// over whichever transport identity delivered the standby's record.
    pub gossip_ingress: Option<ed25519::PublicKey>,
}

/// A valset cutover (or boot) the orchestrator must retarget to.
#[derive(Clone, Debug)]
pub struct MeshEpochEvent {
    pub epoch: u64,
    /// The epoch's consensus members' ed25519 public keys, this node
    /// included. Order is irrelevant — every derived commitment sorts.
    pub members: Vec<ed25519::PublicKey>,
    /// The epoch's STANDBY identities (the valset resident tier): registered
    /// keys the pre-warm layer tunnels toward ahead of their activation.
    /// Never part of the epoch's `ActiveValidatorSet` — a standby that never
    /// shows up costs the epoch nothing.
    pub standbys: Vec<ed25519::PublicKey>,
    /// The consensus view at the cutover; the freshness clock for expiries.
    pub current_view: u64,
}

/// The apply outcome an [`ReachabilityCommand::InstallInvitePeer`] caller
/// awaits — wrapped so the command enum keeps its `Debug`.
pub struct InstallReply(pub tokio::sync::oneshot::Sender<Result<(), String>>);
#[derive(Debug)]
pub struct CoordinatedInviteReply(pub tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>);

impl std::fmt::Debug for InstallReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InstallReply")
    }
}

/// Node -> orchestrator.
#[derive(Debug)]
pub enum ReachabilityCommand {
    /// Boot or epoch cutover: (re)build the mesh for this member set. A
    /// retarget SUPERSEDES any epoch still assembling — tear down in-flight
    /// state and start over.
    Retarget(MeshEpochEvent),
    /// Install a JOIN-WINDOW tunnel peer, live and epoch-independent: the
    /// invite layer. The node has already authenticated the request (the
    /// invite blob's envelope on the joiner side; the token-verified intro
    /// datagram on the inviter side) — the orchestrator only merges the peer
    /// onto the interface. Invite peers are the WEAKEST layer: an epoch's
    /// validated plan or a standby's signed record for the same identity
    /// supersedes them, and the entry dissolves once one exists.
    InstallInvitePeer {
        /// The counterparty's ed25519 identity (its overlay ULA derives from
        /// this).
        peer: ed25519::PublicKey,
        /// The counterparty's X25519 WireGuard key.
        wireguard_public_key: wireguard::X25519PublicKey,
        /// Where to dial it: the blob's advertised endpoint on the joiner
        /// side; the intro datagram's observed source on the inviter side
        /// (WireGuard roams to the authenticated initiation either way).
        endpoint: SocketAddr,
        /// Resolved with the apply outcome (the inviter acks the intro only
        /// after the peer is really on the interface).
        reply: InstallReply,
    },
    /// Resolve a coordinated invite's inviter through the rendezvous plane,
    /// install the inviter as a join-window tunnel peer, then send the
    /// authenticated intro datagram over the same punched underlay socket.
    BootstrapCoordinatedInvitePeer {
        peer: ed25519::PublicKey,
        wireguard_public_key: wireguard::X25519PublicKey,
        intro: Vec<u8>,
        reply: CoordinatedInviteReply,
    },
    /// Send one datagram over the resolver socket. Used for invite intro ACKs
    /// after the receiving side has installed the join-window peer.
    SendResolverDatagram {
        endpoint: SocketAddr,
        bytes: Vec<u8>,
    },
    /// A reachability-channel message arrived from a mesh peer.
    Deliver {
        from: ed25519::PublicKey,
        bytes: Vec<u8>,
    },
    /// The consensus view advanced (drives expiry checks between cutovers).
    ViewTick(u64),
    /// Periodic controller kick: re-offer whatever this node is still
    /// waiting on — its un-acknowledged gossip (record, then advert) while
    /// the epoch is assembling, then each stalled peer's pending handshake
    /// message (request or response) after the mesh verifies. Mesh sends are
    /// best-effort datagrams — a message fired before the transport has a
    /// live connection to the peer (every boot `Retarget` fires before the
    /// p2p actors even start) is silently dropped, and when BOTH sides lose
    /// their initial record the first-contact heal in `on_record` never
    /// triggers on either. Safe at any cadence: every re-offer re-sends the
    /// STORED message verbatim, never re-signs — receivers dedup gossip by
    /// nonce (the mesh version is unchanged) and recognize handshake
    /// duplicates by hash (each side validates the triple exactly once, so
    /// the shared per-epoch `ReplayCache` never sees a nonce twice).
    Nudge,
    /// Drain and exit; the interface is torn down on the way out.
    Shutdown,
}

/// Orchestrator -> node.
#[derive(Debug)]
pub enum ReachabilityEvent {
    /// Send `bytes` to `to` on the reachability channel.
    Send {
        to: ed25519::PublicKey,
        bytes: Vec<u8>,
    },
    /// Every member's signed advertisement verified into one `MeshView`.
    MeshReady { epoch: u64, version: MeshVersion },
    /// The epoch's tunnel config went through the effect: one interface,
    /// `peers` peer relationships.
    TunnelsApplied {
        epoch: u64,
        interface: String,
        peers: usize,
    },
    /// A peer could not be brought up (handshake refused, resolution
    /// failed) or sent traffic it had no business sending. The mesh keeps
    /// going without it; the node surfaces the warning.
    PeerFailed {
        peer: ed25519::PublicKey,
        reason: String,
    },
    /// The epoch as a whole failed (mesh verification, effect rejection).
    /// Previous tunnels stay as they were; the next cutover retries.
    EpochFailed { epoch: u64, reason: String },
    /// The persisted mesh re-applied at boot: `peers` tunnels from epoch
    /// `epoch`'s remembered records, endpoints freshly coordinator-resolved.
    /// Purely a gossip carrier — the boot epoch's own assembly replaces it.
    MeshRestored {
        epoch: u64,
        interface: String,
        peers: usize,
    },
    /// The boot re-apply could not run (unreadable state file, effect
    /// rejection). The plane continues on live assembly alone — exactly the
    /// pre-persistence behavior.
    RestoreFailed { reason: String },
    /// The epoch applied but its mesh could not be persisted; a cold restart
    /// would restore the PREVIOUS persisted epoch (or nothing).
    PersistFailed { reason: String },
    /// Record-derived pre-warm tunnels merged onto the interface: `peers`
    /// counts the standby<->member relationships now installed alongside
    /// (on a member) or ahead of (on a standby) the phase-A mesh.
    StandbyTunnelsApplied {
        epoch: u64,
        interface: String,
        peers: usize,
    },
    /// A join-window invite peer merged onto the interface (see
    /// [`ReachabilityCommand::InstallInvitePeer`]).
    InvitePeerInstalled {
        peer: ed25519::PublicKey,
        interface: String,
    },
}

/// How a peer's WireGuard endpoint was resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// Dial the advertised endpoint as-is (public or already-reachable
    /// address; also the no-coordinator dev path).
    Advertised,
    /// Hole-punch succeeded: dial the peer's punched reflexive.
    Punched(SocketAddr),
}

/// Per-peer endpoint resolution, pluggable so the orchestrator's protocol
/// logic tests deterministically without UDP. The real implementation is
/// [`NatResolver`]; tests use [`StaticResolver`].
#[allow(
    async_fn_in_trait,
    reason = "the resolver is consumed on a single-thread block_on root; no Send bound is wanted"
)]
pub trait EndpointResolver {
    /// Resolve `peer`'s dialable UDP address given its advertised WireGuard
    /// endpoint. Errors mean the peer stays on its advertised endpoint and a
    /// `PeerFailed` is emitted for observability.
    async fn resolve(
        &mut self,
        peer: NodeKey,
        advertised: SocketAddr,
    ) -> Result<Resolution, String>;

    /// Resolve a peer strictly through rendezvous. Callers use this when
    /// there is no trusted advertised endpoint in the protocol payload.
    async fn resolve_rendezvous_endpoint(&mut self, peer: NodeKey) -> Result<SocketAddr, String> {
        let placeholder = SocketAddr::from(([0, 0, 0, 0], 0));
        match self.resolve(peer, placeholder).await? {
            Resolution::Punched(endpoint) => Ok(endpoint),
            Resolution::Advertised => {
                Err("coordinated invite requires a coordinator-resolved endpoint".into())
            }
        }
    }

    /// Send one datagram from the same socket the resolver uses. Only the
    /// production rendezvous resolver supports this; tests may no-op.
    async fn send_datagram(&mut self, _peer: SocketAddr, _bytes: Vec<u8>) -> Result<(), String> {
        Err("resolver datagram sending unavailable".into())
    }

    /// Send one datagram and wait for the first non-rendezvous datagram from
    /// that same endpoint. Used by invite bootstrap so "sent" does not get
    /// mistaken for "the inviter installed us".
    async fn send_datagram_and_recv(
        &mut self,
        _peer: SocketAddr,
        _bytes: Vec<u8>,
        _timeout: Duration,
    ) -> Result<Vec<u8>, String> {
        Err("resolver datagram responses unavailable".into())
    }
}

/// Test resolver: a fixed map, `Advertised` for anything unlisted.
#[derive(Default)]
pub struct StaticResolver(pub HashMap<NodeKey, Resolution>);

impl EndpointResolver for StaticResolver {
    async fn resolve(
        &mut self,
        peer: NodeKey,
        _advertised: SocketAddr,
    ) -> Result<Resolution, String> {
        Ok(self.0.get(&peer).copied().unwrap_or(Resolution::Advertised))
    }

    async fn send_datagram(&mut self, _peer: SocketAddr, _bytes: Vec<u8>) -> Result<(), String> {
        Ok(())
    }
}

/// How long each coordinator interaction (reflexive discovery, lookup) may
/// take before the resolver moves on.
const COORD_STEP_TIMEOUT: Duration = Duration::from_secs(3);
/// One punch exchange attempt; retried [`PUNCH_TRIES`] times before the
/// resolution fails (the peer then rides its advertised endpoint — the
/// coordinator is rendezvous-only, there is no relay to fall back to).
const PUNCH_STEP_TIMEOUT: Duration = Duration::from_secs(1);
const PUNCH_TRIES: usize = 3;

/// How often the rendezvous pump re-advertises this node to its coordinator.
/// Must sit well under common NAT UDP mapping timeouts (~30 s): the keepalive
/// holds the pinhole open AND refreshes the coordinator's registration TTL
/// (`nat_traversal::REGISTRATION_TTL_SECS`). Distinct from the WireGuard
/// `KEEPALIVE_SECONDS` — different plane, different socket.
pub const RENDEZVOUS_KEEPALIVE: Duration = Duration::from_secs(25);

/// Minimum spacing between by-identity rendezvous-fallback attempts for the
/// same endpoint-less peer (change 2 / issue #331): `Nudge` fires every 2s
/// and would otherwise re-attempt a stalled resolve before the resolver's
/// own worst-case attempt (`COORD_STEP_TIMEOUT` + `PUNCH_TRIES` punch
/// windows) could even finish, storming the coordinator. Matches the
/// resolver's own timeout envelope — the same discipline the invite-
/// bootstrap path gets for free by only ever trying once.
const RENDEZVOUS_FALLBACK_BACKOFF: Duration = Duration::from_secs(
    COORD_STEP_TIMEOUT.as_secs() + PUNCH_STEP_TIMEOUT.as_secs() * PUNCH_TRIES as u64,
);

/// Cap on rendezvous-fallback attempts per peer PER EPOCH. Each attempt
/// blocks the single-threaded driver loop for up to the resolver's full
/// timeout envelope, so an unbounded sweep against a peer that stays
/// unpunchable — never registered, coordinator down, or already healed by
/// WireGuard roaming (invisible to this layer) — would starve
/// `Deliver`/gossip for healthy peers forever. After the cap the peer stops
/// being swept for the epoch; the next `Retarget`'s fresh `EpochState`
/// resets `rendezvous_attempted` and grants a new budget.
const RENDEZVOUS_FALLBACK_MAX_ATTEMPTS: u32 = 3;

/// Pure backoff/budget decision: attempt iff never attempted this epoch, or
/// the backoff window has elapsed AND the per-epoch attempt budget remains.
/// `previous` = `(elapsed since the last attempt, attempts made so far)`;
/// `None` = never attempted — also the shape a fresh epoch's reset map
/// produces, which is how "a new epoch resets the budget" happens. Split out
/// from `resolve_peer` so the decision is testable without standing up a
/// `Driver`/real clock races.
fn should_attempt_rendezvous_fallback(previous: Option<(Duration, u32)>) -> bool {
    match previous {
        None => true,
        Some((elapsed, attempts)) => {
            attempts < RENDEZVOUS_FALLBACK_MAX_ATTEMPTS && elapsed >= RENDEZVOUS_FALLBACK_BACKOFF
        }
    }
}

/// How every coordinator request is presented: `Some((signer, cap))`
/// authenticates each request with a proof-of-possession over `signer`
/// (whose public key MUST match the resolver's node key), carrying `cap`
/// for a private (genesis-gated) coordinator or `None` for a public
/// PoP-only one; `None` sends bare requests — the legacy unauthenticated
/// dev path for fully-open coordinators.
pub type CoordinatorAuth = Option<(
    commonware_cryptography::ed25519::PrivateKey,
    Option<nat_traversal::CoordCap>,
)>;

/// The production resolver: a handle to the rendezvous PUMP task that owns
/// the `NatClient`'s receive side. The pump answers unsolicited `PunchSync`
/// fan-outs while this node is otherwise idle (the passive half of somebody
/// else's punch — previously those datagrams were eaten by whichever
/// blocking recv happened to poll, so a punch only completed when both sides
/// resolved simultaneously) and serves `resolve()` commands; a separate
/// SEND-ONLY task keepalive-readvertises on the same socket, so a long run
/// of busy resolves can never starve the keepalive past the coordinator's
/// registration TTL. With NO coordinators configured every resolution is
/// `Advertised` and no task is spawned.
///
/// Establishment (reflexive discovery + registration) happens IN the task,
/// not at construction: a coordinator that is unreachable at boot — the
/// machine woke before its network, the coordinator restarted — must not
/// cost the process its rendezvous for life. Until establishment lands,
/// `resolve()` answers with a prompt, honest error and the task retries
/// with backoff; [`Self::status`] observes the transitions.
pub struct NatResolver {
    commands: Option<tokio::sync::mpsc::Sender<ResolveCmd>>,
    status: Option<tokio::sync::watch::Receiver<RendezvousStatus>>,
}

/// Where rendezvous establishment currently stands, observable via
/// [`NatResolver::status`]. Terminal state is `Ready`; `Unavailable` means
/// the establish task is between backoff retries, still self-healing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RendezvousStatus {
    /// The first discovery attempt has not concluded yet.
    Establishing,
    /// `attempts` establishment rounds have failed; the next retry is
    /// scheduled (backoff doubles up to [`ESTABLISH_RETRY_MAX`]).
    Unavailable { attempts: u32 },
    /// Registered — the coordinator observed this node at `reflexive`.
    Ready { reflexive: SocketAddr },
}

/// Backoff bounds for rendezvous establishment retries. The first attempt
/// fires immediately at spawn (a healthy boot is Ready within milliseconds);
/// failures then retry at 3 s doubling to 30 s — fast enough that "the
/// laptop's Wi-Fi came up ten seconds after the node" heals promptly, slow
/// enough that a long outage never floods a dead route.
const ESTABLISH_RETRY_MIN: Duration = Duration::from_secs(3);
const ESTABLISH_RETRY_MAX: Duration = Duration::from_secs(30);

enum ResolveCmd {
    Resolve {
        peer: NodeKey,
        reply: tokio::sync::oneshot::Sender<Result<Resolution, String>>,
    },
    SendDatagram {
        peer: SocketAddr,
        bytes: Vec<u8>,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    SendDatagramAndRecv {
        peer: SocketAddr,
        bytes: Vec<u8>,
        timeout: Duration,
        reply: tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>,
    },
}

impl NatResolver {
    /// Bind the nat client's UDP socket, discover this node's reflexive
    /// (failing over across the coordinator hints), register, and spawn the
    /// pump. `key` is this node's identity bytes (`binding::node_key`). An
    /// empty coordinator set yields the pass-through resolver.
    pub async fn bind(
        key: NodeKey,
        coordinators: Vec<SocketAddr>,
        auth: CoordinatorAuth,
    ) -> std::io::Result<Self> {
        Self::bind_with_keepalive(key, coordinators, auth, RENDEZVOUS_KEEPALIVE).await
    }

    /// [`Self::bind`] with an explicit keepalive interval (tests shrink it).
    pub async fn bind_with_keepalive(
        key: NodeKey,
        coordinators: Vec<SocketAddr>,
        auth: CoordinatorAuth,
        keepalive: Duration,
    ) -> std::io::Result<Self> {
        if coordinators.is_empty() {
            return Ok(Self {
                commands: None,
                status: None,
            });
        }
        let client = match auth {
            Some((signer, cap)) => {
                NatClient::bind_multi_auth(key, coordinators, signer, cap).await?
            }
            None => NatClient::bind_multi(key, coordinators).await?,
        };
        Ok(Self::from_client(client, keepalive))
    }

    /// Stand the resolver up over an ALREADY-CONSTRUCTED client — socket
    /// mode's path, where the client rides the WireGuard underlay socket
    /// (`nat_traversal::NatSocket::Shared`) so the punch originates from the
    /// tunnel's own 5-tuple. Establishment happens in the spawned task,
    /// exactly like [`Self::bind`].
    pub fn from_client(client: NatClient, keepalive: Duration) -> Self {
        Self::from_client_with_datagram_sink(client, keepalive, None)
    }

    /// [`Self::from_client`] plus an explicit datagram sink. Non-rendezvous
    /// datagrams received on the socket are forwarded to `datagrams`, which
    /// lets invite-intro bootstrap share the WireGuard underlay socket without
    /// changing the default rendezvous-only event stream.
    ///
    /// Infallible: reflexive discovery and registration are the spawned
    /// task's job, retried with backoff until a coordinator answers — a
    /// coordinator that is dark AT BOOT must not disable rendezvous for the
    /// life of the process (it used to: the one-shot construction failure
    /// degraded the caller to a permanent pass-through resolver).
    pub fn from_client_with_datagram_sink(
        client: NatClient,
        keepalive: Duration,
        datagrams: Option<tokio::sync::mpsc::Sender<(SocketAddr, Vec<u8>)>>,
    ) -> Self {
        let (status_tx, status_rx) = tokio::sync::watch::channel(RendezvousStatus::Establishing);
        let (commands, rx) = tokio::sync::mpsc::channel(8);
        tokio::spawn(establish_then_pump(
            client, rx, datagrams, status_tx, keepalive,
        ));
        Self {
            commands: Some(commands),
            status: Some(status_rx),
        }
    }

    /// The coordinator-observed reflexive address, once establishment landed —
    /// what a NATed node should advertise as its WireGuard endpoint. `None`
    /// while establishment is still retrying (and always, for the
    /// pass-through resolver).
    pub fn reflexive(&self) -> Option<SocketAddr> {
        self.status.as_ref().and_then(|s| match *s.borrow() {
            RendezvousStatus::Ready { reflexive } => Some(reflexive),
            _ => None,
        })
    }

    /// Watch rendezvous establishment transitions (`Establishing` →
    /// `Unavailable{attempts}`* → `Ready`). `None` for the pass-through
    /// resolver (no coordinators configured).
    pub fn status(&self) -> Option<tokio::sync::watch::Receiver<RendezvousStatus>> {
        self.status.clone()
    }
}

/// Reply to a command that arrived before rendezvous establishment landed:
/// a prompt, honest error. A caller parked forever on one of these replies
/// is exactly the silent stall the establish task exists to prevent.
fn reply_not_established(cmd: ResolveCmd, attempts: u32) {
    let err = format!(
        "rendezvous not established yet (no coordinator answered, {attempts} attempt(s)) — \
         retrying in the background"
    );
    match cmd {
        ResolveCmd::Resolve { reply, .. } => {
            let _ = reply.send(Err(err));
        }
        ResolveCmd::SendDatagram { reply, .. } => {
            let _ = reply.send(Err(err));
        }
        ResolveCmd::SendDatagramAndRecv { reply, .. } => {
            let _ = reply.send(Err(err));
        }
    }
}

/// Establish rendezvous (reflexive discovery + registration, retried with
/// backoff), then run the pump. Commands arriving during establishment are
/// answered with an honest not-ready error instead of queueing unanswered;
/// between attempts the socket stays served (punch-backs, datagram
/// forwarding) so the shared-underlay paths that need no coordinator keep
/// working while the coordinator is dark.
async fn establish_then_pump(
    mut client: NatClient,
    mut commands: tokio::sync::mpsc::Receiver<ResolveCmd>,
    datagrams: Option<tokio::sync::mpsc::Sender<(SocketAddr, Vec<u8>)>>,
    status: tokio::sync::watch::Sender<RendezvousStatus>,
    keepalive: Duration,
) {
    let mut attempts = 0u32;
    let mut backoff = ESTABLISH_RETRY_MIN;
    let reflexive = loop {
        // Scoped so the attempt future (and its &mut borrow of the client)
        // is dropped before the backoff arm below serves the socket.
        let outcome = {
            let attempt = async {
                let (_idx, reflexive) =
                    client.discover_reflexive_failover(COORD_STEP_TIMEOUT).await?;
                client.register().await?;
                Ok::<SocketAddr, std::io::Error>(reflexive)
            };
            tokio::pin!(attempt);
            loop {
                tokio::select! {
                    res = &mut attempt => break res,
                    cmd = commands.recv() => match cmd {
                        Some(cmd) => reply_not_established(cmd, attempts),
                        // Resolver dropped — nothing left to establish for.
                        None => return,
                    },
                }
            }
        };
        match outcome {
            Ok(reflexive) => break reflexive,
            Err(_unreachable) => {
                attempts += 1;
                let _ = status.send(RendezvousStatus::Unavailable { attempts });
                let wait = tokio::time::sleep(backoff);
                tokio::pin!(wait);
                loop {
                    tokio::select! {
                        _ = &mut wait => break,
                        cmd = commands.recv() => match cmd {
                            Some(cmd) => reply_not_established(cmd, attempts),
                            None => return,
                        },
                        ev = client.recv_socket_event() => {
                            handle_idle_socket_event(&client, ev, datagrams.as_ref()).await;
                        }
                    }
                }
                backoff = (backoff * 2).min(ESTABLISH_RETRY_MAX);
            }
        }
    };
    let _ = status.send(RendezvousStatus::Ready { reflexive });
    let client = std::sync::Arc::new(client);
    // The keepalive is SEND-ONLY (readvertise never touches the recv
    // side), so it runs as its own task on the shared socket: the same
    // socket keeps the same NAT pinhole and coordinator mapping, while a
    // resolve() that runs for its full budget can no longer delay the
    // keepalive past the registration TTL. It holds a Weak handle and
    // exits within one interval of the pump dropping the client. Spawned
    // only now — readvertising before the first registration would be
    // datagrams at a coordinator that never observed us.
    tokio::spawn(rendezvous_keepalive(
        std::sync::Arc::downgrade(&client),
        keepalive,
    ));
    rendezvous_pump(client, commands, datagrams).await
}

/// The pump's idle-arm socket handling, shared with the establishment
/// backoff wait: answer punch-backs, forward non-rendezvous datagrams, and
/// pace transient recv errors so a broken socket cannot spin the loop hot.
async fn handle_idle_socket_event(
    client: &NatClient,
    ev: std::io::Result<SocketEvent>,
    datagrams: Option<&tokio::sync::mpsc::Sender<(SocketAddr, Vec<u8>)>>,
) {
    match ev {
        Ok(SocketEvent::Rendezvous(ClientEvent::PunchSync { peer_reflexive, .. })) => {
            // The passive half of a peer's rendezvous: open our pinhole
            // toward the address the coordinator vouched for. Bounded — one
            // punch per coordinator-sourced PunchSync (the active side's
            // per-try re-Lookup drives repeats).
            let _ = client.send_punch_to(peer_reflexive).await;
        }
        Ok(SocketEvent::Datagram { src, bytes }) => {
            if let Some(datagrams) = datagrams {
                let _ = datagrams.try_send((src, bytes));
            }
        }
        Ok(_) => {}
        Err(_) => {
            // A transient recv error (interface flap, ENOBUFS) must not kill
            // rendezvous for the rest of the process — the old per-call
            // clients isolated failures to one resolve, and this loop must
            // not be weaker. Back off briefly so a persistently-broken
            // socket cannot spin hot; if it IS permanently dead, every
            // resolve() surfaces its own error exactly like the pre-pump
            // code did.
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

impl EndpointResolver for NatResolver {
    async fn resolve(
        &mut self,
        peer: NodeKey,
        _advertised: SocketAddr,
    ) -> Result<Resolution, String> {
        let Some(commands) = &self.commands else {
            return Ok(Resolution::Advertised);
        };
        let (reply, rx) = tokio::sync::oneshot::channel();
        commands
            .send(ResolveCmd::Resolve { peer, reply })
            .await
            .map_err(|_| "rendezvous pump terminated".to_string())?;
        rx.await
            .map_err(|_| "rendezvous pump terminated".to_string())?
    }

    async fn send_datagram(&mut self, peer: SocketAddr, bytes: Vec<u8>) -> Result<(), String> {
        let Some(commands) = &self.commands else {
            return Err("no coordinator socket available for resolver datagram".into());
        };
        let (reply, rx) = tokio::sync::oneshot::channel();
        commands
            .send(ResolveCmd::SendDatagram { peer, bytes, reply })
            .await
            .map_err(|_| "rendezvous pump terminated".to_string())?;
        rx.await
            .map_err(|_| "rendezvous pump terminated".to_string())?
    }

    async fn send_datagram_and_recv(
        &mut self,
        peer: SocketAddr,
        bytes: Vec<u8>,
        timeout: Duration,
    ) -> Result<Vec<u8>, String> {
        let Some(commands) = &self.commands else {
            return Err("no coordinator socket available for resolver datagram response".into());
        };
        let (reply, rx) = tokio::sync::oneshot::channel();
        commands
            .send(ResolveCmd::SendDatagramAndRecv {
                peer,
                bytes,
                timeout,
                reply,
            })
            .await
            .map_err(|_| "rendezvous pump terminated".to_string())?;
        rx.await
            .map_err(|_| "rendezvous pump terminated".to_string())?
    }
}

/// The keepalive body: a SEND-ONLY loop on the shared rendezvous socket.
/// Readvertise nonces are wall-clock-seeded so a REBOOTED node's first
/// keepalive strictly supersedes every nonce its previous life published —
/// otherwise the coordinator would keep answering lookups with the dead
/// pre-reboot mapping (for up to the TTL) while rejecting the fresh adverts
/// as stale replays. Exits within one interval of the pump releasing the
/// client (the `Weak` stops upgrading).
async fn rendezvous_keepalive(client: std::sync::Weak<NatClient>, keepalive: Duration) {
    let mut tick = tokio::time::interval(keepalive);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tick.tick().await; // an interval's first tick fires immediately — consume it.
    let mut nonce = nat_traversal::now_secs();
    loop {
        tick.tick().await;
        let Some(client) = client.upgrade() else {
            return;
        };
        nonce = nonce.max(nat_traversal::now_secs()) + 1;
        let _ = client.readvertise(nonce).await;
    }
}

/// The pump body: single owner of the rendezvous socket's RECEIVE side, so
/// every datagram reaches ONE dispatch point instead of whichever blocking
/// recv was polling. Two duties — serve `resolve()` commands and answer
/// unsolicited `PunchSync` while idle. (The keepalive is deliberately NOT a
/// third select arm: a resolve() runs its full budget inside one arm, and a
/// sequential burst of dead-peer resolves — an epoch cutover with a dozen
/// unreachable peers — would starve an in-loop tick past the registration
/// TTL. It lives in [`rendezvous_keepalive`] on the shared socket instead.)
async fn rendezvous_pump(
    client: std::sync::Arc<NatClient>,
    mut commands: tokio::sync::mpsc::Receiver<ResolveCmd>,
    datagrams: Option<tokio::sync::mpsc::Sender<(SocketAddr, Vec<u8>)>>,
) {
    loop {
        tokio::select! {
            cmd = commands.recv() => {
                let Some(cmd) = cmd else { return };
                match cmd {
                    ResolveCmd::Resolve { peer, reply } => {
                        let _ = reply.send(do_resolve(&client, peer, datagrams.as_ref()).await);
                    }
                    ResolveCmd::SendDatagram { peer, bytes, reply } => {
                        let _ = reply.send(
                            client
                                .send_datagram_to(&bytes, peer)
                                .await
                                .map_err(|e| e.to_string()),
                        );
                    }
                    ResolveCmd::SendDatagramAndRecv {
                        peer,
                        bytes,
                        timeout,
                        reply,
                    } => {
                        let _ = reply.send(
                            send_datagram_and_recv(
                                &client,
                                peer,
                                bytes,
                                timeout,
                                datagrams.as_ref(),
                            )
                            .await,
                        );
                    }
                }
            }
            ev = client.recv_socket_event() => {
                handle_idle_socket_event(&client, ev, datagrams.as_ref()).await;
            }
        }
    }
}

/// One resolve: per TRY, a fresh `Lookup` (each one re-fans `PunchSync` to
/// BOTH sides — the retry is what absorbs a lost fan-out datagram or a
/// momentarily busy peer pump), then a punch exchange bounded by
/// [`PUNCH_STEP_TIMEOUT`]. PunchSyncs arriving mid-resolve are answered
/// inline: this node can simultaneously be the passive side of a DIFFERENT
/// pair's rendezvous. No relay fallback exists — a failed punch is surfaced
/// as an error so the peer rides its advertised endpoint and a `PeerFailed`
/// is emitted for observability.
async fn do_resolve(
    client: &NatClient,
    peer: NodeKey,
    datagrams: Option<&tokio::sync::mpsc::Sender<(SocketAddr, Vec<u8>)>>,
) -> Result<Resolution, String> {
    let mut lookup_timeouts = 0usize;
    for _ in 0..PUNCH_TRIES {
        client
            .send_lookup(peer)
            .await
            .map_err(|e| format!("coordinator lookup: {e}"))?;
        let looked_up = tokio::time::timeout(COORD_STEP_TIMEOUT, async {
            loop {
                match client.recv_socket_event().await {
                    Ok(SocketEvent::Rendezvous(ClientEvent::LookupResponse { key, reflexive }))
                        if key == peer =>
                    {
                        return Ok(reflexive);
                    }
                    Ok(SocketEvent::Rendezvous(ClientEvent::PunchSync {
                        peer_reflexive, ..
                    })) => {
                        let _ = client.send_punch_to(peer_reflexive).await;
                    }
                    Ok(SocketEvent::Datagram { src, bytes }) => {
                        if let Some(datagrams) = datagrams {
                            let _ = datagrams.try_send((src, bytes));
                        }
                    }
                    Ok(_) => {}
                    Err(e) => return Err(format!("coordinator lookup: {e}")),
                }
            }
        })
        .await;
        let peer_reflexive = match looked_up {
            Err(_elapsed) => {
                lookup_timeouts += 1;
                continue;
            }
            Ok(Err(e)) => return Err(e),
            Ok(Ok(None)) => return Err("peer not registered with coordinator".into()),
            Ok(Ok(Some(addr))) => addr,
        };
        if let Err(e) = client.send_punch_to(peer_reflexive).await {
            return Err(format!("punch send: {e}"));
        }
        let punched = tokio::time::timeout(PUNCH_STEP_TIMEOUT, async {
            loop {
                match client.recv_socket_event().await {
                    Ok(SocketEvent::Rendezvous(ClientEvent::Punch { src, .. }))
                        if src == peer_reflexive =>
                    {
                        return Ok(());
                    }
                    Ok(SocketEvent::Rendezvous(ClientEvent::PunchSync {
                        peer_reflexive: sync_to,
                        ..
                    })) => {
                        let _ = client.send_punch_to(sync_to).await;
                    }
                    Ok(SocketEvent::Datagram { src, bytes }) => {
                        if let Some(datagrams) = datagrams {
                            let _ = datagrams.try_send((src, bytes));
                        }
                    }
                    Ok(_) => {}
                    Err(e) => return Err(format!("punch recv: {e}")),
                }
            }
        })
        .await;
        match punched {
            Ok(Ok(())) => return Ok(Resolution::Punched(peer_reflexive)),
            Ok(Err(e)) => return Err(e),
            Err(_elapsed) => continue, // this try's punch window closed — re-Lookup and retry.
        }
    }
    if lookup_timeouts == PUNCH_TRIES {
        return Err("coordinator lookup timed out".to_string());
    }
    Err(format!("hole-punch failed after {PUNCH_TRIES} tries"))
}

async fn send_datagram_and_recv(
    client: &NatClient,
    peer: SocketAddr,
    bytes: Vec<u8>,
    timeout: Duration,
    datagrams: Option<&tokio::sync::mpsc::Sender<(SocketAddr, Vec<u8>)>>,
) -> Result<Vec<u8>, String> {
    client
        .send_datagram_to(&bytes, peer)
        .await
        .map_err(|e| format!("resolver datagram send: {e}"))?;
    tokio::time::timeout(timeout, async {
        loop {
            match client.recv_socket_event().await {
                Ok(SocketEvent::Datagram { src, bytes }) if src == peer => return Ok(bytes),
                Ok(SocketEvent::Datagram { src, bytes }) => {
                    if let Some(datagrams) = datagrams {
                        let _ = datagrams.try_send((src, bytes));
                    }
                }
                Ok(SocketEvent::Rendezvous(ClientEvent::PunchSync { peer_reflexive, .. })) => {
                    let _ = client.send_punch_to(peer_reflexive).await;
                }
                Ok(_) => {}
                Err(e) => return Err(format!("resolver datagram recv: {e}")),
            }
        }
    })
    .await
    .map_err(|_| "resolver datagram response timed out".to_string())?
}

#[derive(Debug, thiserror::Error)]
pub enum ReachabilityError {
    #[error("wireguard keystore: {0}")]
    Key(#[from] KeyError),
    #[error("protocol: {0:?}")]
    Upgrade(UpgradeError),
    #[error("message codec: {0}")]
    Msg(#[from] MsgError),
    #[error("the node dropped a reachability channel")]
    ChannelClosed,
    #[error("wireguard effect: {0}")]
    Effect(String),
}

impl From<UpgradeError> for ReachabilityError {
    fn from(err: UpgradeError) -> Self {
        Self::Upgrade(err)
    }
}

/// Which half of a pending handshake this node is waiting on. Every stored
/// message is re-sent VERBATIM on retry — re-signing would mint a fresh
/// nonce, and a triple whose parts disagree can never validate on both
/// sides (the ack pins request and response by hash).
// a handful of entries per epoch — variant size imbalance is irrelevant.
#[allow(
    clippy::large_enum_variant,
    reason = "each epoch holds only a handful of handshakes; boxing would complicate the retry state"
)]
enum PeerHandshake {
    /// We initiated and sent the request; the peer's response is due.
    AwaitingResponse { request: TunnelUpgradeRequest },
    /// We responded to the peer's request; its ack is due.
    AwaitingAck {
        request: TunnelUpgradeRequest,
        response: TunnelUpgradeResponse,
    },
    /// The triple validated and the plan is recorded. Kept (not removed) so
    /// re-delivered peer messages are recognized as duplicates of THIS
    /// handshake instead of violations — and never re-validated, which is
    /// what keeps retries out of the shared per-epoch `ReplayCache`.
    Done {
        request_hash: [u8; 32],
        response_hash: [u8; 32],
        /// `Some` on the initiator: a re-delivered response means the peer
        /// never got our single-shot ack — re-send this stored one.
        /// `None` on the responder (its ack receipt ended the exchange).
        ack: Option<TunnelUpgradeAck>,
    },
}

/// A foreign handshake message this node carries between two OTHER members
/// that share no direct link: the latest-STAGE signed message per ordered
/// `(initiator, responder)` pair, re-offered on nudge until superseded or
/// expired. Signature-verified before acceptance, so a malicious member
/// cannot evict a real in-flight message by poisoning the slot.
struct RelaySlot {
    /// Request=0 < Response=1 < Ack=2 — a later stage proves the earlier
    /// one arrived, so it supersedes the slot.
    stage: u8,
    /// The member whose signature the slot's message carries — the one peer
    /// a re-offer never needs to reach (it already has its own message).
    signer: ValidatorIdentity,
    msg: ReachabilityMsg,
    expires_at_view: u64,
}

/// A verified candidate to store and fan out through the relay mesh.
/// Grouping the signed-message metadata keeps the relay boundary explicit.
struct RelayedHandshake {
    pair: (ValidatorIdentity, ValidatorIdentity),
    stage: u8,
    signer: ValidatorIdentity,
    expires_at_view: u64,
    verified: bool,
    msg: ReachabilityMsg,
}

/// Which side of the plane this node runs for the epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    /// In the epoch's `ActiveValidatorSet`: full phase-A assembly, plus the
    /// pre-warm layer toward the epoch's standbys.
    Member,
    /// In the standby set only: record exchange and pre-warm tunnels toward
    /// the members — no advert, no handshakes, no relay duty.
    Standby,
}

/// Everything one epoch accumulates on the way to its `apply` call.
struct EpochState {
    epoch: u64,
    role: Role,
    set: ActiveValidatorSet,
    peers: Vec<ValidatorIdentity>,
    /// The epoch's standby identities (never in `set`).
    standbys: Vec<ValidatorIdentity>,
    pk_of: HashMap<ValidatorIdentity, ed25519::PublicKey>,
    /// The accepted nonce per pre-warm counterparty (standby records on a
    /// member, member records on a standby). Higher nonce wins — the live
    /// re-advertisement rule the phase-A member set deliberately does not
    /// have; anything at or below drops silently (re-offers are routine).
    prewarm_nonces: BTreeMap<ValidatorIdentity, u64>,
    /// The standbys' owner-signed records as accepted (member role only) —
    /// the form the nudge re-offers and the accept-gated flood relays.
    standby_records: BTreeMap<ValidatorIdentity, SignedEndpointRecord>,
    /// The tunnel parts derived from accepted pre-warm records (endpoint
    /// resolved, overlay route derived) — merged over the interface's base
    /// peers on every change.
    prewarm_peers: BTreeMap<ValidatorIdentity, PeerTunnelConfig>,
    /// Standby-directed sends route to the transport identity that DELIVERED
    /// the standby's record when that identity is not a member (the lobby
    /// ingress a parked joiner connects under, or the standby's own key) —
    /// a standby is not necessarily dialable under its record identity.
    routes: HashMap<ValidatorIdentity, ed25519::PublicKey>,
    /// One strictly-monotonic counter for EVERYTHING this identity signs in
    /// the epoch — replay keys are `(identity, epoch, nonce)`, and the
    /// advert duplicate rule wants strictly-increasing nonces too.
    nonce: u64,
    /// This node's own signed record for the epoch — what the nudge
    /// re-offers. In the member role it also lives in `records`; in the
    /// standby role this field is its only home (a standby is never part of
    /// the member record set).
    own_record: SignedEndpointRecord,
    /// Owner-signed records as they arrived (our own included) — the form
    /// that can be re-gossiped to peers the owner has no link to.
    records: BTreeMap<ValidatorIdentity, SignedEndpointRecord>,
    adverts: BTreeMap<ValidatorIdentity, EndpointAdvertisement>,
    own_advert_sent: bool,
    view_state: Option<MeshView>,
    replay: ReplayCache,
    /// Requests that arrived before our own `MeshView` completed (the peer
    /// verified faster); drained the moment it does. Keyed by initiator so
    /// nudged re-offers of the same request collapse to one entry.
    pending_requests: BTreeMap<ValidatorIdentity, TunnelUpgradeRequest>,
    handshakes: HashMap<ValidatorIdentity, PeerHandshake>,
    /// Relay slots keyed by `(initiator, responder)` for handshakes between
    /// two OTHER members.
    relayed: BTreeMap<(ValidatorIdentity, ValidatorIdentity), RelaySlot>,
    plans: BTreeMap<ValidatorIdentity, TunnelInstallPlan>,
    overrides: BTreeMap<ValidatorIdentity, SocketAddr>,
    /// By-identity rendezvous-fallback bookkeeping per endpoint-less peer
    /// (change 2 / issue #331): `(last attempt instant, attempts so far)` —
    /// backs `should_attempt_rendezvous_fallback`'s backoff + per-epoch
    /// budget (`RENDEZVOUS_FALLBACK_MAX_ATTEMPTS`) so `Nudge`'s 2s cadence
    /// neither storms the coordinator nor sweeps an unpunchable peer
    /// forever. Fresh per epoch — a `Retarget` resets the budget.
    rendezvous_attempted: BTreeMap<ValidatorIdentity, (Instant, u32)>,
    failed: HashSet<ValidatorIdentity>,
    applied: bool,
}

impl EpochState {
    fn next_nonce(&mut self) -> u64 {
        self.nonce += 1;
        self.nonce
    }

    /// Every record this epoch holds, whether it arrived as signed record
    /// gossip or embedded in a member's (signed) advertisement — per member
    /// the higher nonce wins. The advance gate and the mesh version compute
    /// over THIS merged set, so a member whose record only ever reached us
    /// inside its advertisement still counts.
    fn known_records(&self) -> BTreeMap<ValidatorIdentity, EndpointRecord> {
        let mut out: BTreeMap<ValidatorIdentity, EndpointRecord> = self
            .records
            .iter()
            .map(|(id, signed)| (*id, signed.record.clone()))
            .collect();
        for (id, advert) in &self.adverts {
            match out.get(id) {
                Some(prev) if advert.record.nonce <= prev.nonce => {}
                _ => {
                    out.insert(*id, advert.record.clone());
                }
            }
        }
        out
    }
}

/// The orchestrator's cross-epoch context.
struct Driver<'a, E, R> {
    config: &'a ReachabilityConfig,
    keypair: WireGuardKeypair,
    me: ValidatorIdentity,
    overlay: OverlayPolicy,
    interface: String,
    effect: E,
    resolver: R,
    events: mpsc::Sender<ReachabilityEvent>,
    view: u64,
    state: Option<EpochState>,
    /// A previous epoch's interface is live and must be removed before (or
    /// instead of) the next apply.
    interface_live: bool,
    /// The interface's BASE peers, keyed by identity: the validated plans of
    /// the last epoch apply, or the restored mesh at boot. `Some` iff the
    /// live interface carries a base the pre-warm layer may merge over;
    /// pre-warm peers layer on top (same identity: the fresher pre-warm
    /// entry wins). Survives retargets — the physical interface does too —
    /// until the next apply replaces it.
    base_peers: Option<BTreeMap<ValidatorIdentity, PeerTunnelConfig>>,
    /// The cold-restart re-apply runs at most once per process life, on the
    /// first `Retarget` (boot). Later retargets are live cutovers with a
    /// working transport — restoring over them would tear down good tunnels.
    restore_tried: bool,
    /// JOIN-WINDOW peers (see `ReachabilityCommand::InstallInvitePeer`):
    /// epoch-independent, merged into every apply as the weakest layer (an
    /// entry never overrides a validated plan or a pre-warm record for the
    /// same identity, and dissolves once one exists).
    invite_peers: BTreeMap<ValidatorIdentity, PeerTunnelConfig>,
}

/// Drive the reachability plane until `Shutdown` (clean exit) or a channel
/// closes (error). One call outlives every epoch; `Retarget` events move it
/// between epochs. The per-epoch state machine:
///
/// 1. **Bind.** Derive the epoch's `ActiveValidatorSet` via
///    [`binding::active_set`]; fresh replay cache and nonce counter.
/// 2. **Record gossip.** Send our `EndpointRecord` (WG public key, control +
///    wireguard endpoints, `+ ADVERT_TTL_VIEWS` expiry) to every other
///    member; collect theirs, re-sending ours on first contact so joining
///    order can't strand anyone.
/// 3. **Advertise.** With ALL records held: `compute_mesh_version`, sign the
///    `EndpointAdvertisement`, fan out; collect everyone's.
/// 4. **Verify.** With all advertisements held: `MeshView::verify`; emit
///    `MeshReady` or `EpochFailed`.
/// 5. **Handshakes.** Initiator per [`initiates`]: resolve the peer's
///    endpoint, then request -> response -> ack; both sides validate the
///    triple via `validate_upgrade_as` from their own perspective under the
///    `OverlayPolicy::ula_v6` overlay and one shared per-epoch replay cache.
///    Single-shot loss at any stage heals by `Nudge` re-offers: stored
///    messages re-sent verbatim (never re-signed) into duplicate-tolerant
///    receivers, so each side still validates the triple exactly once.
/// 6. **Apply.** When every peer has a validated plan or a `PeerFailed`:
///    tear down any previous interface and make the epoch's ONE
///    `apply_tunnel_plans` call; emit `TunnelsApplied`.
pub async fn run<E, R>(
    config: ReachabilityConfig,
    effect: E,
    resolver: R,
    mut commands: mpsc::Receiver<ReachabilityCommand>,
    events: mpsc::Sender<ReachabilityEvent>,
) -> Result<(), ReachabilityError>
where
    E: WireGuardEffect,
    R: EndpointResolver,
{
    let (keypair, _generated) = WireGuardKeypair::load_or_generate(&config.wireguard_key_file)?;
    let mut driver = Driver {
        me: binding::identity_of(&config.signer.public_key()),
        overlay: OverlayPolicy::ula_v6(config.chain_id.clone()),
        interface: binding::interface_name(&config.chain_id),
        keypair,
        config: &config,
        effect,
        resolver,
        events,
        view: 0,
        state: None,
        interface_live: false,
        base_peers: None,
        restore_tried: false,
        invite_peers: BTreeMap::new(),
    };
    while let Some(command) = commands.recv().await {
        match command {
            ReachabilityCommand::Retarget(event) => driver.retarget(event).await?,
            ReachabilityCommand::InstallInvitePeer {
                peer,
                wireguard_public_key,
                endpoint,
                reply,
            } => {
                driver
                    .install_invite_peer(peer, wireguard_public_key, endpoint, reply)
                    .await?
            }
            ReachabilityCommand::BootstrapCoordinatedInvitePeer {
                peer,
                wireguard_public_key,
                intro,
                reply,
            } => {
                driver
                    .bootstrap_coordinated_invite_peer(peer, wireguard_public_key, intro, reply)
                    .await?
            }
            ReachabilityCommand::SendResolverDatagram { endpoint, bytes } => {
                let _ = driver.resolver.send_datagram(endpoint, bytes).await;
            }
            ReachabilityCommand::Deliver { from, bytes } => driver.deliver(from, bytes).await?,
            ReachabilityCommand::ViewTick(view) => driver.view = driver.view.max(view),
            ReachabilityCommand::Nudge => driver.nudge().await?,
            ReachabilityCommand::Shutdown => {
                if driver.interface_live {
                    // best-effort: exiting matters more than the teardown's
                    // error detail.
                    let _ = driver.effect.remove_interface();
                }
                return Ok(());
            }
        }
    }
    Err(ReachabilityError::ChannelClosed)
}

impl<E, R> Driver<'_, E, R>
where
    E: WireGuardEffect,
    R: EndpointResolver,
{
    async fn emit(&self, event: ReachabilityEvent) -> Result<(), ReachabilityError> {
        self.events
            .send(event)
            .await
            .map_err(|_| ReachabilityError::ChannelClosed)
    }

    async fn send_msg(
        &self,
        to: ValidatorIdentity,
        msg: &ReachabilityMsg,
    ) -> Result<(), ReachabilityError> {
        let Some(state) = &self.state else {
            return Ok(());
        };
        // a learned transport route wins over the identity itself: a parked
        // standby may only be dialable under the ingress identity that
        // delivered its record.
        let Some(pk) = state
            .routes
            .get(&to)
            .or_else(|| state.pk_of.get(&to))
            .cloned()
        else {
            return Ok(());
        };
        self.emit(ReachabilityEvent::Send {
            to: pk,
            bytes: msg.encode(),
        })
        .await
    }

    /// Fan one of OUR handshake messages to every peer: the addressee
    /// processes it, everyone else is a candidate relay toward an addressee
    /// we may share no direct link with. Mesh sends are best-effort, so the
    /// sender cannot know which links exist — all paths carry the message
    /// and receivers dedup.
    async fn fan_msg(&self, msg: &ReachabilityMsg) -> Result<(), ReachabilityError> {
        let Some(state) = &self.state else {
            return Ok(());
        };
        let peers = state.peers.clone();
        for peer in peers {
            self.send_msg(peer, msg).await?;
        }
        Ok(())
    }

    async fn retarget(&mut self, event: MeshEpochEvent) -> Result<(), ReachabilityError> {
        self.view = self.view.max(event.current_view);
        let identities: Vec<ValidatorIdentity> =
            event.members.iter().map(binding::identity_of).collect();
        // members win over a stale standby listing; anything in neither set
        // is inert (demotion normally exits the node before this).
        let standby_ids: Vec<ValidatorIdentity> = event
            .standbys
            .iter()
            .map(binding::identity_of)
            .filter(|id| !identities.contains(id))
            .collect();
        let role = if identities.contains(&self.me) {
            Role::Member
        } else if standby_ids.contains(&self.me) {
            Role::Standby
        } else {
            // be inert, not wrong: drop epoch state and any live tunnel.
            if self.interface_live {
                let _ = self.effect.remove_interface();
                self.interface_live = false;
            }
            self.base_peers = None;
            self.state = None;
            return self
                .emit(ReachabilityEvent::EpochFailed {
                    epoch: event.epoch,
                    reason: "this node is neither a member nor a standby of the epoch".into(),
                })
                .await;
        };
        if !self.restore_tried {
            self.restore_tried = true;
            self.restore(&event).await?;
        }
        let set = binding::active_set(&self.config.chain_id, event.epoch, identities.clone())?;
        let pk_of: HashMap<ValidatorIdentity, ed25519::PublicKey> = event
            .members
            .iter()
            .chain(event.standbys.iter())
            .map(|pk| (binding::identity_of(pk), pk.clone()))
            .collect();
        // a member's gossip/handshake counterparties exclude itself; a
        // standby's are exactly the members.
        let peers: Vec<ValidatorIdentity> = identities
            .iter()
            .copied()
            .filter(|id| *id != self.me)
            .collect();
        let own = SignedEndpointRecord::sign(
            EndpointRecord {
                namespace: self.config.chain_id.clone(),
                epoch: event.epoch,
                valset_root: set.valset_root,
                admission_root: set.admission_root,
                validator_identity: self.me,
                wireguard_public_key: self.keypair.public_key(),
                control_endpoint: self.config.control_endpoint,
                wireguard_endpoint: self.config.wireguard_advertised,
                expires_at_view: self.view + ADVERT_TTL_VIEWS,
                // the epoch's first signed nonce; the counter below starts
                // past it.
                nonce: 1,
            },
            &self.config.signer,
        );
        let mut state = EpochState {
            epoch: event.epoch,
            role,
            set,
            peers,
            standbys: standby_ids,
            pk_of,
            prewarm_nonces: BTreeMap::new(),
            standby_records: BTreeMap::new(),
            prewarm_peers: BTreeMap::new(),
            routes: HashMap::new(),
            nonce: 1,
            own_record: own.clone(),
            records: BTreeMap::new(),
            adverts: BTreeMap::new(),
            own_advert_sent: false,
            view_state: None,
            replay: ReplayCache::default(),
            pending_requests: BTreeMap::new(),
            handshakes: HashMap::new(),
            relayed: BTreeMap::new(),
            plans: BTreeMap::new(),
            overrides: BTreeMap::new(),
            rendezvous_attempted: BTreeMap::new(),
            failed: HashSet::new(),
            applied: false,
        };
        if role == Role::Member {
            state.records.insert(self.me, own.clone());
        }
        let peers = state.peers.clone();
        let standbys = state.standbys.clone();
        self.state = Some(state);
        for peer in peers {
            self.send_msg(peer, &ReachabilityMsg::Record(own.clone()))
                .await?;
        }
        if role == Role::Member {
            // seed the pre-warm layer's counterparties too (a lost send
            // heals by nudge; a standby with no route yet just misses this
            // round).
            for standby in standbys {
                self.send_msg(standby, &ReachabilityMsg::Record(own.clone()))
                    .await?;
            }
            // a single-member network is a complete mesh already.
            return self.advance().await;
        }
        Ok(())
    }

    /// The cold-restart re-apply: bring the LAST applied epoch's tunnels
    /// back from the persisted mesh so plane gossip has a path on a node
    /// that restarted with zero TCP links (NATed member whose join ingress
    /// is gone; whole-network cold start). Everything re-derives from the
    /// persisted records — peer WireGuard keys and advertised endpoints from
    /// the records themselves, overlay addresses from `(chain_id, identity)`
    /// — except endpoints behind NAT, which are re-resolved FRESH through
    /// the coordinator (a persisted punch observation died with the
    /// downtime's NAT mappings; re-resolution needs no gossip). One-sided
    /// resolution suffices: WireGuard roams a peer's endpoint on any
    /// authenticated inbound packet, so whichever side resolves a working
    /// path first heals the pair.
    ///
    /// Strictly best-effort and strictly a bootstrap: failures degrade to
    /// the pre-persistence behavior (live assembly only), and the boot
    /// epoch's own assembly replaces the restored interface at its apply.
    async fn restore(&mut self, event: &MeshEpochEvent) -> Result<(), ReachabilityError> {
        let Some(path) = &self.config.persist_file else {
            return Ok(());
        };
        let mesh = match store::load(path, &self.config.chain_id) {
            Ok(Some(mesh)) => mesh,
            Ok(None) => return Ok(()),
            Err(err) => {
                return self
                    .emit(ReachabilityEvent::RestoreFailed {
                        reason: err.to_string(),
                    })
                    .await;
            }
        };
        // the BOOT epoch's members gate the restore: a departed member's
        // tunnel is dead weight, an arrival has no persisted record (its
        // tunnel assembles live). Signatures were verified by `load`.
        let member_pk_of: HashMap<ValidatorIdentity, ed25519::PublicKey> = event
            .members
            .iter()
            .map(|pk| (binding::identity_of(pk), pk.clone()))
            .collect();
        let records: Vec<EndpointRecord> = mesh
            .adverts
            .iter()
            .map(|advert| advert.record.clone())
            .filter(|record| {
                record.validator_identity != self.me
                    && member_pk_of.contains_key(&record.validator_identity)
            })
            .collect();
        if records.is_empty() {
            return Ok(());
        }
        let mut peers: BTreeMap<ValidatorIdentity, PeerTunnelConfig> = BTreeMap::new();
        for record in &records {
            // an endpoint-less record installs without an endpoint (nothing
            // to resolve): that peer initiates and WireGuard roams to it.
            let endpoint = match record.wireguard_endpoint.map(|e| e.socket_addr()) {
                None => None,
                Some(advertised) => Some(
                    match self
                        .resolver
                        .resolve(binding::node_key(record.validator_identity), advertised)
                        .await
                    {
                        Ok(Resolution::Advertised) => advertised,
                        Ok(Resolution::Punched(addr)) => addr,
                        Err(reason) => {
                            // same contract as live assembly: the peer rides its
                            // advertised endpoint and the failure is surfaced.
                            self.emit(ReachabilityEvent::PeerFailed {
                                peer: member_pk_of[&record.validator_identity].clone(),
                                reason: format!("restore endpoint resolution: {reason}"),
                            })
                            .await?;
                            advertised
                        }
                    },
                ),
            };
            let allowed_ips = self
                .overlay
                .identity_allowed_ips(record.validator_identity);
            peers.insert(
                record.validator_identity,
                PeerTunnelConfig {
                    wireguard_public_key: record.wireguard_public_key,
                    endpoint,
                    allowed_ips,
                    keepalive_seconds: Some(KEEPALIVE_SECONDS),
                },
            );
        }
        let local_interface_ips = self
            .overlay
            .identity_allowed_ips(self.me);
        let peer_count = peers.len();
        // the join-window invite layer rides the restore apply too (a node
        // rebooting mid-window keeps its invite tunnel), but never enters
        // the restored BASE below — the base is the persisted mesh only.
        let mut applied = peers.clone();
        self.merge_invite_layer(&mut applied);
        let parts: Vec<PeerTunnelConfig> = applied.values().cloned().collect();
        // the invite bootstrap may have brought the interface up before the
        // first epoch event (a NATed member re-running first contact at
        // boot) — reconfigure it rather than re-create it, so the restore
        // neither dies on `AlreadyCreated` nor drops the live join tunnel.
        let outcome = if self.interface_live {
            update_peer_tunnels(
                &mut self.effect,
                self.interface.clone(),
                self.keypair.private_key_base64(),
                self.config.wireguard_port,
                &local_interface_ips,
                &parts,
            )
        } else {
            apply_peer_tunnels(
                &mut self.effect,
                self.interface.clone(),
                self.keypair.private_key_base64(),
                self.config.wireguard_port,
                &local_interface_ips,
                &parts,
            )
        };
        match outcome {
            Ok(()) => {
                self.interface_live = true;
                // the restored mesh is the interface's base — the pre-warm
                // layer merges its record-derived peers over it.
                self.base_peers = Some(peers);
                self.emit(ReachabilityEvent::MeshRestored {
                    epoch: mesh.epoch,
                    interface: self.interface.clone(),
                    peers: peer_count,
                })
                .await
            }
            Err(err) => {
                self.emit(ReachabilityEvent::RestoreFailed {
                    reason: format!("wireguard effect: {err:?}"),
                })
                .await
            }
        }
    }

    /// Re-offer whatever the current stage is still waiting on, always the
    /// STORED message, never re-signed: pre-version, a fresh record nonce
    /// would change the mesh version peers already computed; post-verify, a
    /// re-signed handshake message would desynchronize the hash-pinned
    /// triple and mint nonces the peer's replay validation has not burnt.
    ///
    /// Gossip stages re-offer EVERY record and advert this node holds — not
    /// only its own — to every peer (receivers dedup by nonce): a peer with
    /// no link to some member receives that member's gossip from us, which
    /// is what assembles a star topology. The handshake stage re-offers per
    /// stalled peer (the pending request while we await its response, our
    /// response while we await its ack) plus every live relay slot this node
    /// carries for pairs that share no direct link. The completed side never
    /// re-offers — a `Done` initiator re-sends its stored ack only when the
    /// peer's re-delivered response proves the ack was lost (see
    /// `on_response`), so retries terminate once both sides are done and the
    /// relay slots expire by view.
    ///
    /// The pre-warm layer re-offers in every member stage (it has no version
    /// lock): member gossip to the standbys, known standby records to
    /// everyone else. A standby's own nudge re-offers exactly its record.
    async fn nudge(&mut self) -> Result<(), ReachabilityError> {
        let view = self.view;
        let sends: Vec<(ValidatorIdentity, ReachabilityMsg)> = {
            let Some(state) = &mut self.state else {
                return Ok(());
            };
            if state.role == Role::Standby {
                // a standby re-offers exactly its own record, to every
                // member: its single job is being installable, and member
                // gossip flows back through the members' own re-offers.
                let own = ReachabilityMsg::Record(state.own_record.clone());
                state
                    .peers
                    .iter()
                    .map(|peer| (*peer, own.clone()))
                    .collect()
            } else if state.view_state.is_none() {
                // gossip stages: everything known to everyone (an advert
                // doubles as a record carrier for members whose record
                // gossip never reached a peer directly).
                let msgs: Vec<ReachabilityMsg> = state
                    .records
                    .values()
                    .map(|record| ReachabilityMsg::Record(record.clone()))
                    .chain(
                        state
                            .adverts
                            .values()
                            .map(|advert| ReachabilityMsg::Advert(advert.clone())),
                    )
                    .collect();
                state
                    .peers
                    .iter()
                    .flat_map(|peer| msgs.iter().map(|msg| (*peer, msg.clone())))
                    .collect()
            } else {
                state.relayed.retain(|_, slot| slot.expires_at_view >= view);
                // our own stalled halves fan to EVERY peer, not only the
                // counterparty: the direct link may be the one that does not
                // exist, and any other peer can relay.
                let own: Vec<(ValidatorIdentity, ReachabilityMsg)> = state
                    .handshakes
                    .values()
                    .filter_map(|handshake| match handshake {
                        PeerHandshake::AwaitingResponse { request } => {
                            Some(ReachabilityMsg::Request(request.clone()))
                        }
                        PeerHandshake::AwaitingAck { response, .. } => {
                            Some(ReachabilityMsg::Response(response.clone()))
                        }
                        PeerHandshake::Done { .. } => None,
                    })
                    .flat_map(|msg| state.peers.iter().map(move |peer| (*peer, msg.clone())))
                    .collect();
                // relay slots fan to every peer except the message's own
                // signer: this node cannot know which peer has the working
                // link to the addressee, so all candidate paths carry it.
                let relayed = state.relayed.values().flat_map(|slot| {
                    state
                        .peers
                        .iter()
                        .filter(|peer| **peer != slot.signer)
                        .map(|peer| (*peer, slot.msg.clone()))
                });
                own.into_iter().chain(relayed).collect()
            }
        };
        // the pre-warm layer's re-offers, in BOTH member stages: member
        // gossip (records + adverts) to every standby — a standby with one
        // ingress link learns every member through it — and known standby
        // records to member peers and the other standbys, so a lost relay
        // heals. Receivers dedup by nonce; standbys are few.
        let prewarm_sends: Vec<(ValidatorIdentity, ReachabilityMsg)> = {
            let state = self.state.as_ref().expect("nudge checked state");
            if state.role == Role::Member && !state.standbys.is_empty() {
                let member_gossip: Vec<ReachabilityMsg> = state
                    .records
                    .values()
                    .map(|record| ReachabilityMsg::Record(record.clone()))
                    .chain(
                        state
                            .adverts
                            .values()
                            .map(|advert| ReachabilityMsg::Advert(advert.clone())),
                    )
                    .collect();
                let to_standbys = state
                    .standbys
                    .iter()
                    .flat_map(|standby| member_gossip.iter().map(|msg| (*standby, msg.clone())));
                let standby_records = state.standby_records.values().flat_map(|record| {
                    let owner = record.record.validator_identity;
                    let msg = ReachabilityMsg::Record(record.clone());
                    state
                        .peers
                        .iter()
                        .chain(state.standbys.iter())
                        .filter(move |target| **target != owner)
                        .map(move |target| (*target, msg.clone()))
                        .collect::<Vec<_>>()
                });
                to_standbys.chain(standby_records).collect()
            } else {
                Vec::new()
            }
        };
        for (peer, msg) in sends.into_iter().chain(prewarm_sends) {
            self.send_msg(peer, &msg).await?;
        }
        // change 2 / issue #331: retry the by-identity rendezvous fallback
        // for any MEMBER peer that is still endpoint-less and still missing
        // a punched override. `resolve_peer`'s single attempt at handshake
        // time can lose the race against the peer's own coordinator
        // registration (both sides often boot together); `resolve_peer`'s
        // own backoff + bounded per-epoch budget
        // (`should_attempt_rendezvous_fallback` /
        // `RENDEZVOUS_FALLBACK_MAX_ATTEMPTS`) keeps this from hammering the
        // coordinator on every 2s `Nudge` and from sweeping an unpunchable
        // peer forever — the sweep goes quiet once the budget is spent and
        // re-arms only at the next epoch's `Retarget`.
        let retry_targets: Vec<ValidatorIdentity> = match &self.state {
            Some(state) if state.role == Role::Member => state
                .peers
                .iter()
                .copied()
                .filter(|peer| {
                    !state.overrides.contains_key(peer)
                        && state
                            .view_state
                            .as_ref()
                            .and_then(|view| view.record(*peer))
                            .is_some_and(|record| record.wireguard_endpoint.is_none())
                })
                .collect(),
            _ => Vec::new(),
        };
        for peer in retry_targets {
            self.resolve_peer(peer).await?;
        }
        Ok(())
    }

    /// Route one delivered message. `via` is the transport-authenticated
    /// DELIVERING member — with relaying it need not be the message's owner,
    /// so every handler authenticates the content signature and binds
    /// protocol state to the identity INSIDE the message; `via` only gates
    /// membership and takes the blame for undecodable/unverifiable junk.
    async fn deliver(
        &mut self,
        from: ed25519::PublicKey,
        bytes: Vec<u8>,
    ) -> Result<(), ReachabilityError> {
        let via = binding::identity_of(&from);
        let Some(state) = &mut self.state else {
            // no active epoch (pre-boot traffic) — nothing to bind it to.
            return Ok(());
        };
        // membership gate on the DELIVERING identity: plane participants
        // (members + standbys), plus the configured gossip ingress — the
        // lobby key a parked standby connects under. Content signatures do
        // the real authentication either way.
        let ingress = self.config.gossip_ingress.as_ref() == Some(&from);
        if !state.pk_of.contains_key(&via) && !ingress {
            return self
                .emit(ReachabilityEvent::PeerFailed {
                    peer: from,
                    reason: "reachability traffic from a non-participant".into(),
                })
                .await;
        }
        let msg = match ReachabilityMsg::decode(&bytes) {
            Ok(msg) => msg,
            Err(err) => {
                return self
                    .emit(ReachabilityEvent::PeerFailed {
                        peer: from,
                        reason: format!("undecodable reachability message: {err}"),
                    })
                    .await;
            }
        };
        if state.role == Role::Standby {
            // a standby consumes gossip only: member records and adverts
            // feed its pre-warm tunnels; handshake traffic (fanned blindly
            // by members — senders cannot know which links exist) is not
            // for it and carries no relay duty.
            return match msg {
                ReachabilityMsg::Record(record) => self.on_member_record(via, record).await,
                ReachabilityMsg::Advert(advert) => self.on_member_advert(via, advert).await,
                ReachabilityMsg::Request(_)
                | ReachabilityMsg::Response(_)
                | ReachabilityMsg::Ack(_) => Ok(()),
            };
        }
        match msg {
            ReachabilityMsg::Record(record) => self.on_record(via, from, record).await,
            ReachabilityMsg::Advert(advert) => self.on_advert(via, advert).await,
            ReachabilityMsg::Request(request) => {
                let initiator = request.fields.initiator_identity;
                let responder = request.fields.responder_identity;
                if responder == self.me {
                    return self.on_request(via, request).await;
                }
                if initiator == self.me {
                    // our own message relayed back around — nothing to do.
                    return Ok(());
                }
                let expires = request.fields.expires_at_view;
                let verified = request.verify_signature().is_ok();
                self.relay(
                    via,
                    RelayedHandshake {
                        pair: (initiator, responder),
                        stage: 0,
                        signer: initiator,
                        expires_at_view: expires,
                        verified,
                        msg: ReachabilityMsg::Request(request),
                    },
                )
                .await
            }
            ReachabilityMsg::Response(response) => {
                let responder = response.fields.responder_identity;
                let initiator = response.fields.initiator_identity;
                if initiator == self.me {
                    return self.on_response(via, response).await;
                }
                if responder == self.me {
                    return Ok(());
                }
                let expires = response.fields.expires_at_view;
                let verified = response.verify_signature().is_ok();
                self.relay(
                    via,
                    RelayedHandshake {
                        pair: (initiator, responder),
                        stage: 1,
                        signer: responder,
                        expires_at_view: expires,
                        verified,
                        msg: ReachabilityMsg::Response(response),
                    },
                )
                .await
            }
            ReachabilityMsg::Ack(ack) => {
                let initiator = ack.fields.initiator_identity;
                let responder = ack.fields.responder_identity;
                if responder == self.me {
                    return self.on_ack(via, ack).await;
                }
                if initiator == self.me {
                    return Ok(());
                }
                let expires = ack.fields.expires_at_view;
                let verified = ack.verify_signature().is_ok();
                self.relay(
                    via,
                    RelayedHandshake {
                        pair: (initiator, responder),
                        stage: 2,
                        signer: initiator,
                        expires_at_view: expires,
                        verified,
                        msg: ReachabilityMsg::Ack(ack),
                    },
                )
                .await
            }
        }
    }

    /// Carry a handshake message between two OTHER members: verify, slot by
    /// `(initiator, responder)` with stage supersession, and fan out to every
    /// peer except the delivering one and the message's signer — this node
    /// cannot know which peer holds the working link to the addressee.
    async fn relay(
        &mut self,
        via: ValidatorIdentity,
        relayed: RelayedHandshake,
    ) -> Result<(), ReachabilityError> {
        let RelayedHandshake {
            pair,
            stage,
            signer,
            expires_at_view,
            verified,
            msg,
        } = relayed;
        if !verified {
            return self
                .fail_peer(via, "relayed an unverifiable handshake message")
                .await;
        }
        let state = self.state.as_mut().expect("deliver checked state");
        if !state.set.contains(pair.0) || !state.set.contains(pair.1) {
            return self
                .fail_peer(via, "relayed a handshake for a non-member pair")
                .await;
        }
        if expires_at_view < self.view {
            return Ok(());
        }
        match state.relayed.get(&pair) {
            // same or later stage already carried — this sighting adds
            // nothing, and dropping it is what terminates the flood.
            Some(slot) if stage <= slot.stage => return Ok(()),
            _ => {}
        }
        state.relayed.insert(
            pair,
            RelaySlot {
                stage,
                signer,
                msg: msg.clone(),
                expires_at_view,
            },
        );
        let targets: Vec<ValidatorIdentity> = state
            .peers
            .iter()
            .copied()
            .filter(|peer| *peer != via && *peer != signer)
            .collect();
        for peer in targets {
            self.send_msg(peer, &msg).await?;
        }
        Ok(())
    }

    async fn on_record(
        &mut self,
        via: ValidatorIdentity,
        from: ed25519::PublicKey,
        signed: SignedEndpointRecord,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        let owner = signed.record.validator_identity;
        // content authentication: the record may have been relayed, so the
        // delivering link proves nothing about the record's owner.
        if signed.verify().is_err() {
            return self.fail_peer(via, "record signature invalid").await;
        }
        if state.standbys.contains(&owner) {
            return self.on_standby_record(via, from, signed).await;
        }
        if !state.set.contains(owner) {
            return self.fail_peer(via, "record from an unknown identity").await;
        }
        if signed.record.epoch != state.epoch {
            // cutover skew, not a violation: nodes cross epoch boundaries at
            // slightly different times (a just-activated standby above all),
            // so gossip signed against the neighboring epoch is routine —
            // its owner re-signs once it observes the boundary.
            return Ok(());
        }
        if owner == self.me {
            // our own record echoed back around the relay ring.
            return Ok(());
        }
        // phase A: the set locks at version time — later (higher-nonce)
        // re-advertisements retunnel at the next cutover.
        if state.own_advert_sent {
            return Ok(());
        }
        let first_contact = !state.records.contains_key(&owner);
        let accepted = match state.records.get(&owner) {
            Some(prev) if signed.record.nonce <= prev.record.nonce => false,
            _ => {
                state.records.insert(owner, signed.clone());
                true
            }
        };
        if first_contact {
            // heal join-order: the member that just appeared may have missed
            // our initial fan-out.
            let own = state.records.get(&self.me).cloned().expect("own record");
            self.send_msg(owner, &ReachabilityMsg::Record(own)).await?;
        }
        if accepted {
            // relay the news: peers with no link to the owner only ever see
            // its record through us — the standbys included, whose pre-warm
            // tunnels want every member's record. Accept-gated, so the
            // flood terminates.
            let targets: Vec<ValidatorIdentity> = {
                let state = self.state.as_ref().expect("still in epoch");
                state
                    .peers
                    .iter()
                    .chain(state.standbys.iter())
                    .copied()
                    .filter(|peer| *peer != owner && *peer != via)
                    .collect()
            };
            for peer in targets {
                self.send_msg(peer, &ReachabilityMsg::Record(signed.clone()))
                    .await?;
            }
        }
        self.advance().await
    }

    /// A standby's owner-signed record (member role): validate, resolve its
    /// endpoint, and merge the tunnel onto the live interface — the pre-warm
    /// layer's whole trick. A higher nonce supersedes in place (the live
    /// re-advertisement rule); duplicates drop silently because every member
    /// re-offers standby records on nudge.
    async fn on_standby_record(
        &mut self,
        via: ValidatorIdentity,
        from: ed25519::PublicKey,
        signed: SignedEndpointRecord,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        let owner = signed.record.validator_identity;
        // the record must bind to THIS epoch's member set — the same tuple
        // its owner derives from the boundary it synced. Another chain's
        // record is a violation; a neighboring epoch's is cutover skew (the
        // standby re-signs once its manifest poll crosses the boundary).
        if signed.record.namespace != state.set.namespace {
            return self
                .fail_peer(via, "standby record from another chain")
                .await;
        }
        if signed.record.epoch != state.set.epoch
            || signed.record.valset_root != state.set.valset_root
            || signed.record.admission_root != state.set.admission_root
        {
            return Ok(());
        }
        if let Err(err) = signed.record.check(&self.config.port_policy, self.view) {
            return self
                .fail_peer(via, &format!("standby record refused: {err:?}"))
                .await;
        }
        match state.prewarm_nonces.get(&owner) {
            Some(prev) if signed.record.nonce <= *prev => return Ok(()),
            _ => {}
        }
        let first_contact = !state.prewarm_nonces.contains_key(&owner);
        state.prewarm_nonces.insert(owner, signed.record.nonce);
        state.standby_records.insert(owner, signed.clone());
        // learn the transport route: a delivery straight off the standby's
        // own link (via is no member) tells us which identity reaches it —
        // its own key, or the shared lobby ingress it parks under.
        if via != owner && !state.set.contains(via) {
            state.routes.insert(owner, from);
        } else if via == owner {
            state.routes.remove(&owner);
        }
        // endpoint-less standby: install without an endpoint — it initiates.
        let endpoint = match signed.record.wireguard_endpoint.map(|e| e.socket_addr()) {
            None => None,
            Some(advertised) => Some(self.resolve_prewarm_endpoint(owner, advertised).await?),
        };
        let allowed_ips = self
            .overlay
            .identity_allowed_ips(owner);
        let state = self.state.as_mut().expect("still in epoch");
        state.prewarm_peers.insert(
            owner,
            PeerTunnelConfig {
                wireguard_public_key: signed.record.wireguard_public_key,
                endpoint,
                allowed_ips,
                keepalive_seconds: Some(KEEPALIVE_SECONDS),
            },
        );
        self.sync_prewarm().await?;
        if first_contact {
            // the standby just appeared: hand it our own gossip directly —
            // the nudge re-offers cover everything else it is missing.
            let (own_record, own_advert) = {
                let state = self.state.as_ref().expect("still in epoch");
                (
                    state.own_record.clone(),
                    state.adverts.get(&self.me).cloned(),
                )
            };
            self.send_msg(owner, &ReachabilityMsg::Record(own_record))
                .await?;
            if let Some(advert) = own_advert {
                self.send_msg(owner, &ReachabilityMsg::Advert(advert))
                    .await?;
            }
        }
        // relay the accepted record onward — members with no link to the
        // standby, and the other standbys, see it through us. Accept-gated,
        // so the flood terminates.
        let targets: Vec<ValidatorIdentity> = {
            let state = self.state.as_ref().expect("still in epoch");
            state
                .peers
                .iter()
                .chain(state.standbys.iter())
                .copied()
                .filter(|peer| *peer != owner && *peer != via)
                .collect()
        };
        for peer in targets {
            self.send_msg(peer, &ReachabilityMsg::Record(signed.clone()))
                .await?;
        }
        Ok(())
    }

    async fn on_advert(
        &mut self,
        via: ValidatorIdentity,
        advert: EndpointAdvertisement,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        let owner = advert.record.validator_identity;
        if advert.verify_signature().is_err() {
            return self.fail_peer(via, "advert signature invalid").await;
        }
        if !state.set.contains(owner) {
            return self.fail_peer(via, "advert from an unknown identity").await;
        }
        if advert.record.epoch != state.epoch {
            // cutover skew — same tolerance as records.
            return Ok(());
        }
        if owner == self.me {
            return Ok(());
        }
        if state.view_state.is_some() {
            return Ok(());
        }
        let first_contact = !state.adverts.contains_key(&owner);
        let accepted = match state.adverts.get(&owner) {
            Some(prev) if advert.record.nonce <= prev.record.nonce => false,
            _ => {
                state.adverts.insert(owner, advert.clone());
                true
            }
        };
        if first_contact && state.own_advert_sent {
            let own = state.adverts.get(&self.me).cloned().expect("own advert");
            self.send_msg(owner, &ReachabilityMsg::Advert(own)).await?;
        }
        if accepted {
            // standbys ride the advert flood too: the signed advert set is
            // what they persist for their promotion reboot's restore.
            let targets: Vec<ValidatorIdentity> = {
                let state = self.state.as_ref().expect("still in epoch");
                state
                    .peers
                    .iter()
                    .chain(state.standbys.iter())
                    .copied()
                    .filter(|peer| *peer != owner && *peer != via)
                    .collect()
            };
            for peer in targets {
                self.send_msg(peer, &ReachabilityMsg::Advert(advert.clone()))
                    .await?;
            }
        }
        self.advance().await
    }

    /// A member's record, received in the STANDBY role: validate and merge
    /// the member's tunnel — the standby side of the pre-warm layer. Another
    /// standby's record (members re-fan those to everyone) is silently not
    /// for us; standby<->standby tunnels assemble after activation like any
    /// member pair.
    async fn on_member_record(
        &mut self,
        via: ValidatorIdentity,
        signed: SignedEndpointRecord,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        let owner = signed.record.validator_identity;
        if signed.verify().is_err() {
            return self.fail_peer(via, "record signature invalid").await;
        }
        if owner == self.me || state.standbys.contains(&owner) {
            return Ok(());
        }
        if !state.set.contains(owner) {
            return self.fail_peer(via, "record identity/epoch mismatch").await;
        }
        self.merge_member_prewarm(&signed.record).await
    }

    /// A member's advertisement, received in the STANDBY role: the richer
    /// form of its record — accepted for the SAME pre-warm merge, and
    /// persisted, because the signed advert set is what the promotion
    /// reboot's cold-restart restore reads back from disk.
    async fn on_member_advert(
        &mut self,
        via: ValidatorIdentity,
        advert: EndpointAdvertisement,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        let owner = advert.record.validator_identity;
        if advert.verify_signature().is_err() {
            return self.fail_peer(via, "advert signature invalid").await;
        }
        if owner == self.me || state.standbys.contains(&owner) {
            return Ok(());
        }
        if !state.set.contains(owner) {
            return self.fail_peer(via, "advert identity/epoch mismatch").await;
        }
        let accepted = match state.adverts.get(&owner) {
            Some(prev) if advert.record.nonce <= prev.record.nonce => false,
            _ => {
                state.adverts.insert(owner, advert.clone());
                true
            }
        };
        if accepted && let Some(path) = &self.config.persist_file {
            let state = self.state.as_ref().expect("still in epoch");
            let mesh = PersistedMesh::new(
                self.config.chain_id.clone(),
                state.epoch,
                state.adverts.values().cloned().collect(),
            );
            if let Err(err) = store::save(path, &mesh) {
                self.emit(ReachabilityEvent::PersistFailed {
                    reason: err.to_string(),
                })
                .await?;
            }
        }
        let record = advert.record.clone();
        self.merge_member_prewarm(&record).await
    }

    /// The standby side's shared merge: bind a member record to the epoch
    /// tuple, dedup by nonce, resolve, and re-apply the interface.
    async fn merge_member_prewarm(
        &mut self,
        record: &EndpointRecord,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("pre-warm inside an epoch");
        let owner = record.validator_identity;
        if record.namespace != state.set.namespace {
            return self
                .fail_peer(owner, "member record from another chain")
                .await;
        }
        if record.epoch != state.set.epoch
            || record.valset_root != state.set.valset_root
            || record.admission_root != state.set.admission_root
        {
            // cutover skew: this standby's manifest poll and the members'
            // boundary crossing are not synchronized.
            return Ok(());
        }
        if let Err(err) = record.check(&self.config.port_policy, self.view) {
            return self
                .fail_peer(owner, &format!("member record refused: {err:?}"))
                .await;
        }
        match state.prewarm_nonces.get(&owner) {
            Some(prev) if record.nonce <= *prev => return Ok(()),
            _ => {}
        }
        state.prewarm_nonces.insert(owner, record.nonce);
        // endpoint-less member record: install without an endpoint — it initiates.
        let endpoint = match record.wireguard_endpoint.map(|e| e.socket_addr()) {
            None => None,
            Some(advertised) => Some(self.resolve_prewarm_endpoint(owner, advertised).await?),
        };
        let allowed_ips = self
            .overlay
            .identity_allowed_ips(owner);
        let state = self.state.as_mut().expect("still in epoch");
        state.prewarm_peers.insert(
            owner,
            PeerTunnelConfig {
                wireguard_public_key: record.wireguard_public_key,
                endpoint,
                allowed_ips,
                keepalive_seconds: Some(KEEPALIVE_SECONDS),
            },
        );
        self.sync_prewarm().await
    }

    /// Move the epoch forward through every stage the accumulated state now
    /// satisfies: records complete -> sign + fan out our advert; adverts
    /// complete -> verify the mesh + start handshakes; plans complete ->
    /// apply. Idempotent; called after every state change.
    async fn advance(&mut self) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("advance without epoch");

        // records -> our signed advert. The gate counts records however
        // they arrived — direct gossip, relayed gossip, or embedded in a
        // faster member's advertisement.
        let known = state.known_records();
        if !state.own_advert_sent && known.len() == state.set.validators().len() {
            let records: Vec<EndpointRecord> = known.into_values().collect();
            let version = compute_mesh_version(&records)?;
            let own_record = state
                .records
                .get(&self.me)
                .cloned()
                .expect("own record")
                .record;
            let advert = EndpointAdvertisement::sign(own_record, version, &self.config.signer);
            state.adverts.insert(self.me, advert.clone());
            state.own_advert_sent = true;
            let peers = state.peers.clone();
            for peer in peers {
                self.send_msg(peer, &ReachabilityMsg::Advert(advert.clone()))
                    .await?;
            }
            return Box::pin(self.advance()).await;
        }

        // adverts -> the verified mesh view + handshake kick-off
        if state.view_state.is_none()
            && state.own_advert_sent
            && state.adverts.len() == state.set.validators().len()
        {
            let ads: Vec<EndpointAdvertisement> = state.adverts.values().cloned().collect();
            let epoch = state.epoch;
            match MeshView::verify(state.set.clone(), ads, &self.config.port_policy, self.view) {
                Ok(view) => {
                    let version = view.mesh_version;
                    state.view_state = Some(view);
                    self.emit(ReachabilityEvent::MeshReady { epoch, version })
                        .await?;
                    self.start_handshakes().await?;
                    let state = self.state.as_mut().expect("still in epoch");
                    let pending = std::mem::take(&mut state.pending_requests);
                    for (sender, request) in pending {
                        self.on_request(sender, request).await?;
                    }
                    return Box::pin(self.advance()).await;
                }
                Err(err) => {
                    return self
                        .emit(ReachabilityEvent::EpochFailed {
                            epoch,
                            reason: format!("mesh verification: {err:?}"),
                        })
                        .await;
                }
            }
        }

        // plans (or failures) complete -> the epoch's one apply
        if !state.applied
            && state.view_state.is_some()
            && state.plans.len() + state.failed.len() == state.peers.len()
        {
            state.applied = true;
            let epoch = state.epoch;
            let plans: Vec<TunnelInstallPlan> = state.plans.values().cloned().collect();
            let overrides = state.overrides.clone();
            let adverts: Vec<EndpointAdvertisement> = state.adverts.values().cloned().collect();
            // the epoch's validated plans become the interface's new BASE;
            // the pre-warm peers merge over it (same identity: the fresher
            // pre-warm entry wins), so a standby tunnel that assembled
            // during the epoch's bring-up survives the replace.
            let base: BTreeMap<ValidatorIdentity, PeerTunnelConfig> = plans
                .iter()
                .map(TunnelInstallPlan::peer_identity)
                .zip(plan_peer_configs(&plans, &overrides))
                .collect();
            let mut merged = base.clone();
            merged.extend(state.prewarm_peers.clone());
            merged.remove(&self.me);
            let prewarm_count = state.prewarm_peers.len();
            self.merge_invite_layer(&mut merged);
            if self.interface_live {
                let _ = self.effect.remove_interface();
                self.interface_live = false;
            }
            self.base_peers = None;
            {
                // Apply even when `merged` is empty: a single-member network
                // (every fresh desktop workspace) and an all-peers-failed
                // epoch still need the interface up — the node's own /128 is
                // what the per-use media planes (voice/video hub) bind, so a
                // peer-less interface is the difference between a working
                // solo huddle and a join that hangs in "connecting" forever.
                let peers: Vec<PeerTunnelConfig> = merged.values().cloned().collect();
                // the plane's overlay is ula_v6: the local side is the same
                // identity-derived /128 every validated plan carries.
                let local_interface_ips = self
                    .overlay
                    .identity_allowed_ips(self.me);
                if let Err(err) = apply_peer_tunnels(
                    &mut self.effect,
                    self.interface.clone(),
                    self.keypair.private_key_base64(),
                    self.config.wireguard_port,
                    &local_interface_ips,
                    &peers,
                ) {
                    return self
                        .emit(ReachabilityEvent::EpochFailed {
                            epoch,
                            reason: format!("wireguard effect: {err:?}"),
                        })
                        .await;
                }
                self.interface_live = true;
                self.base_peers = Some(base);
                // the epoch's mesh is now REAL — remember it for the
                // cold-restart re-apply. Only with member plans: an
                // all-peers-failed epoch must not clobber the last mesh that
                // actually carried member tunnels (pre-warm peers alone are
                // not it — their owners persist their own side).
                if !plans.is_empty()
                    && let Some(path) = &self.config.persist_file
                {
                    let mesh = PersistedMesh::new(self.config.chain_id.clone(), epoch, adverts);
                    if let Err(err) = store::save(path, &mesh) {
                        self.emit(ReachabilityEvent::PersistFailed {
                            reason: err.to_string(),
                        })
                        .await?;
                    }
                }
            }
            self.emit(ReachabilityEvent::TunnelsApplied {
                epoch,
                interface: self.interface.clone(),
                peers: plans.len(),
            })
            .await?;
            if prewarm_count > 0 {
                self.emit(ReachabilityEvent::StandbyTunnelsApplied {
                    epoch,
                    interface: self.interface.clone(),
                    peers: prewarm_count,
                })
                .await?;
            }
            return Ok(());
        }
        Ok(())
    }

    /// Initiator side: resolve each lower-identity-initiates peer and send
    /// the signed request.
    async fn start_handshakes(&mut self) -> Result<(), ReachabilityError> {
        let state = self.state.as_ref().expect("mesh just verified");
        let targets: Vec<ValidatorIdentity> = state
            .peers
            .iter()
            .copied()
            .filter(|peer| initiates(self.me, *peer))
            .collect();
        for peer in targets {
            self.resolve_peer(peer).await?;
            let state = self.state.as_mut().expect("still in epoch");
            let nonce = state.next_nonce();
            let view = state.view_state.as_ref().expect("mesh verified");
            let fields = TunnelUpgradeRequestFields {
                namespace: state.set.namespace.clone(),
                epoch: state.set.epoch,
                valset_root: state.set.valset_root,
                admission_root: state.set.admission_root,
                mesh_version: view.mesh_version,
                initiator_identity: self.me,
                responder_identity: peer,
                initiator_wireguard_public_key: self.keypair.public_key(),
                initiator_wireguard_endpoint: self.config.wireguard_advertised,
                requested_allowed_ips: self.overlay.allowed_ips_for(view, peer)?,
                port_policy_hash: self.config.port_policy.hash(),
                expires_at_view: self.view + HANDSHAKE_TTL_VIEWS,
                nonce,
            };
            let request = TunnelUpgradeRequest::sign(fields, &self.config.signer);
            state.handshakes.insert(
                peer,
                PeerHandshake::AwaitingResponse {
                    request: request.clone(),
                },
            );
            self.fan_msg(&ReachabilityMsg::Request(request)).await?;
        }
        Ok(())
    }

    /// Run the endpoint resolver for `peer`, recording a punched override
    /// or a `PeerFailed` observability event (the peer then rides its
    /// advertised endpoint).
    async fn resolve_peer(&mut self, peer: ValidatorIdentity) -> Result<(), ReachabilityError> {
        let state = self.state.as_ref().expect("resolving inside an epoch");
        let advertised = state
            .view_state
            .as_ref()
            .and_then(|view| view.record(peer))
            .and_then(|record| record.wireguard_endpoint)
            .map(|endpoint| endpoint.socket_addr());
        // no record, or an endpoint-less peer: nothing to resolve AGAINST —
        // but a configured coordinator can still rendezvous by identity
        // (change 2 / issue #331); the base "the peer initiates and
        // WireGuard roams to it" contract stands when there is no
        // coordinator to ask.
        let Some(advertised) = advertised else {
            return self.resolve_peer_via_rendezvous_fallback(peer).await;
        };
        match self
            .resolver
            .resolve(binding::node_key(peer), advertised)
            .await
        {
            Ok(Resolution::Advertised) => Ok(()),
            Ok(Resolution::Punched(addr)) => {
                let state = self.state.as_mut().expect("still in epoch");
                state.overrides.insert(peer, addr);
                Ok(())
            }
            Err(reason) => {
                let pk = state.pk_of.get(&peer).cloned();
                if let Some(pk) = pk {
                    self.emit(ReachabilityEvent::PeerFailed {
                        peer: pk,
                        reason: format!("endpoint resolution: {reason}"),
                    })
                    .await?;
                }
                Ok(())
            }
        }
    }

    /// The endpoint-less fallback (change 2 / issue #331): a member↔member
    /// pair that both advertise no endpoint (the default for every
    /// invite-joined node) can never initiate a WireGuard handshake — WITH a
    /// coordinator configured, rendezvous the peer by identity instead
    /// (`resolve_rendezvous_endpoint`, the same by-identity resolution the
    /// invite bootstrap already uses). No coordinator configured ⇒ today's
    /// behavior exactly: install endpoint-less and wait for the peer's own
    /// initiation. A failed resolve stays terminal for THIS attempt — no
    /// relay (locked design) — but a per-peer backoff lets a later `Nudge`
    /// retry once the peer has had time to register, up to a bounded
    /// per-epoch budget (`RENDEZVOUS_FALLBACK_MAX_ATTEMPTS`).
    async fn resolve_peer_via_rendezvous_fallback(
        &mut self,
        peer: ValidatorIdentity,
    ) -> Result<(), ReachabilityError> {
        if self.config.coordinators.is_empty() {
            return Ok(());
        }
        let state = self.state.as_ref().expect("resolving inside an epoch");
        if state.overrides.contains_key(&peer) {
            return Ok(()); // already resolved this epoch.
        }
        let now = Instant::now();
        let previous = state
            .rendezvous_attempted
            .get(&peer)
            .map(|(last, attempts)| (now.saturating_duration_since(*last), *attempts));
        if !should_attempt_rendezvous_fallback(previous) {
            return Ok(());
        }
        let pk = state.pk_of.get(&peer).cloned();
        let state = self.state.as_mut().expect("still in epoch");
        let entry = state.rendezvous_attempted.entry(peer).or_insert((now, 0));
        *entry = (now, entry.1 + 1);
        match self
            .resolver
            .resolve_rendezvous_endpoint(binding::node_key(peer))
            .await
        {
            Ok(addr) => {
                let state = self.state.as_mut().expect("still in epoch");
                state.overrides.insert(peer, addr);
                Ok(())
            }
            Err(reason) => {
                if let Some(pk) = pk {
                    self.emit(ReachabilityEvent::PeerFailed {
                        peer: pk,
                        reason: format!("rendezvous fallback: {reason}"),
                    })
                    .await?;
                }
                Ok(())
            }
        }
    }

    /// Resolve a pre-warm counterparty's dialable endpoint, with the same
    /// contract as the restore path: a resolver failure surfaces as
    /// `PeerFailed` and the peer rides its advertised endpoint.
    async fn resolve_prewarm_endpoint(
        &mut self,
        peer: ValidatorIdentity,
        advertised: SocketAddr,
    ) -> Result<SocketAddr, ReachabilityError> {
        match self
            .resolver
            .resolve(binding::node_key(peer), advertised)
            .await
        {
            Ok(Resolution::Advertised) => Ok(advertised),
            Ok(Resolution::Punched(addr)) => Ok(addr),
            Err(reason) => {
                let pk = self
                    .state
                    .as_ref()
                    .and_then(|state| state.pk_of.get(&peer))
                    .cloned();
                if let Some(pk) = pk {
                    self.emit(ReachabilityEvent::PeerFailed {
                        peer: pk,
                        reason: format!("pre-warm endpoint resolution: {reason}"),
                    })
                    .await?;
                }
                Ok(advertised)
            }
        }
    }

    /// Push the interface's full desired configuration — the phase-A base
    /// (validated plans or the restored mesh) with the pre-warm peers merged
    /// over it — onto the effect. Live interfaces reconfigure in place; a
    /// standby with nothing applied yet brings its interface up here (its
    /// interface exists purely for pre-warm). A member whose epoch is still
    /// assembling holds off — its one epoch apply merges the pre-warm set.
    /// Merge the join-window invite layer into an assembled peer map: an
    /// invite peer never overrides an entry the stronger layers (validated
    /// plans, restored mesh, pre-warm records) already carry — and once one
    /// exists for the same identity, the invite entry has served its purpose
    /// and dissolves.
    fn merge_invite_layer(&mut self, merged: &mut BTreeMap<ValidatorIdentity, PeerTunnelConfig>) {
        self.invite_peers.retain(|id, _| !merged.contains_key(id));
        for (id, cfg) in &self.invite_peers {
            merged.entry(*id).or_insert_with(|| cfg.clone());
        }
    }

    /// Install a join-window tunnel peer (node-authenticated; see the
    /// command doc) and re-apply the interface — the invite layer's own
    /// `sync_prewarm` analogue, usable BEFORE any epoch exists.
    async fn install_invite_peer(
        &mut self,
        peer: ed25519::PublicKey,
        wireguard_public_key: wireguard::X25519PublicKey,
        endpoint: SocketAddr,
        reply: InstallReply,
    ) -> Result<(), ReachabilityError> {
        let identity = binding::identity_of(&peer);
        if identity == self.me {
            let _ = reply
                .0
                .send(Err("refusing an invite tunnel to self".into()));
            return Ok(());
        }
        let allowed_ips = self
            .overlay
            .identity_allowed_ips(identity);
        self.invite_peers.insert(
            identity,
            PeerTunnelConfig {
                wireguard_public_key,
                // the intro datagram's observed source — always concrete.
                endpoint: Some(endpoint),
                allowed_ips,
                keepalive_seconds: Some(KEEPALIVE_SECONDS),
            },
        );

        let mut merged = self.base_peers.clone().unwrap_or_default();
        if let Some(state) = &self.state {
            merged.extend(state.prewarm_peers.clone());
        }
        self.merge_invite_layer(&mut merged);
        merged.remove(&self.me);
        let peers: Vec<PeerTunnelConfig> = merged.values().cloned().collect();
        let local_interface_ips = self
            .overlay
            .identity_allowed_ips(self.me);
        let outcome = if self.interface_live {
            update_peer_tunnels(
                &mut self.effect,
                self.interface.clone(),
                self.keypair.private_key_base64(),
                self.config.wireguard_port,
                &local_interface_ips,
                &peers,
            )
        } else {
            apply_peer_tunnels(
                &mut self.effect,
                self.interface.clone(),
                self.keypair.private_key_base64(),
                self.config.wireguard_port,
                &local_interface_ips,
                &peers,
            )
            .inspect(|()| {
                self.interface_live = true;
                // the interface is now live over an empty base — later
                // merges reconfigure instead of re-creating.
                if self.base_peers.is_none() {
                    self.base_peers = Some(BTreeMap::new());
                }
            })
        };
        match outcome {
            Ok(()) => {
                let _ = reply.0.send(Ok(()));
                self.emit(ReachabilityEvent::InvitePeerInstalled {
                    peer,
                    interface: self.interface.clone(),
                })
                .await
            }
            Err(err) => {
                // the interface keeps whatever configuration it had; the
                // caller decides whether to retry.
                let _ = reply.0.send(Err(format!("{err:?}")));
                Ok(())
            }
        }
    }

    /// Coordinated invite bootstrap: rendezvous the inviter's WireGuard
    /// underlay endpoint, install it as the local join-window peer, and send
    /// the authenticated intro over that same punched socket so the inviter
    /// can install this node in return.
    async fn bootstrap_coordinated_invite_peer(
        &mut self,
        peer: ed25519::PublicKey,
        wireguard_public_key: wireguard::X25519PublicKey,
        intro: Vec<u8>,
        reply: CoordinatedInviteReply,
    ) -> Result<(), ReachabilityError> {
        let identity = binding::identity_of(&peer);
        let endpoint = match self
            .resolver
            .resolve_rendezvous_endpoint(binding::node_key(identity))
            .await
        {
            Ok(endpoint) => endpoint,
            Err(reason) => {
                let _ = reply.0.send(Err(format!(
                    "coordinated invite endpoint resolution: {reason}"
                )));
                return Ok(());
            }
        };

        let (install_tx, install_rx) = tokio::sync::oneshot::channel();
        self.install_invite_peer(
            peer,
            wireguard_public_key,
            endpoint,
            InstallReply(install_tx),
        )
        .await?;
        match install_rx.await {
            Ok(Ok(())) => {
                match self
                    .resolver
                    .send_datagram_and_recv(endpoint, intro, Duration::from_secs(2))
                    .await
                {
                    Ok(bytes) => {
                        let _ = reply.0.send(Ok(bytes));
                    }
                    Err(reason) => {
                        let _ = reply
                            .0
                            .send(Err(format!("coordinated invite intro ack: {reason}")));
                    }
                }
            }
            Ok(Err(reason)) => {
                let _ = reply.0.send(Err(reason));
            }
            Err(_) => {
                let _ = reply.0.send(Err("invite peer installer exited".into()));
            }
        }
        Ok(())
    }

    async fn sync_prewarm(&mut self) -> Result<(), ReachabilityError> {
        let state = self.state.as_ref().expect("pre-warm inside an epoch");
        if state.prewarm_peers.is_empty() {
            return Ok(());
        }
        let epoch = state.epoch;
        let prewarm_count = state.prewarm_peers.len();
        let mut merged = match (&self.base_peers, state.role) {
            (Some(base), _) => base.clone(),
            (None, Role::Standby) => BTreeMap::new(),
            (None, Role::Member) => return Ok(()),
        };
        merged.extend(state.prewarm_peers.clone());
        self.merge_invite_layer(&mut merged);
        // the pre-warm layer never carries a tunnel to this node itself
        // (records filter by owner class), but a restored base may still
        // hold an entry for an identity that since became us — impossible
        // today, cheap to keep impossible.
        merged.remove(&self.me);
        let peers: Vec<PeerTunnelConfig> = merged.values().cloned().collect();
        let local_interface_ips = self
            .overlay
            .identity_allowed_ips(self.me);
        let outcome = if self.interface_live {
            update_peer_tunnels(
                &mut self.effect,
                self.interface.clone(),
                self.keypair.private_key_base64(),
                self.config.wireguard_port,
                &local_interface_ips,
                &peers,
            )
        } else {
            apply_peer_tunnels(
                &mut self.effect,
                self.interface.clone(),
                self.keypair.private_key_base64(),
                self.config.wireguard_port,
                &local_interface_ips,
                &peers,
            )
            .inspect(|()| {
                self.interface_live = true;
                // the standby's interface is now live over an empty base —
                // later merges reconfigure instead of re-creating.
                if self.base_peers.is_none() {
                    self.base_peers = Some(BTreeMap::new());
                }
            })
        };
        match outcome {
            Ok(()) => {
                self.emit(ReachabilityEvent::StandbyTunnelsApplied {
                    epoch,
                    interface: self.interface.clone(),
                    peers: prewarm_count,
                })
                .await
            }
            Err(err) => {
                // the interface keeps whatever configuration it had; the
                // next accepted record (or nudge-driven re-offer) retries.
                self.emit(ReachabilityEvent::EpochFailed {
                    epoch,
                    reason: format!("pre-warm tunnel apply: {err:?}"),
                })
                .await
            }
        }
    }

    /// Responder side: answer a request with our signed response. A
    /// duplicate of the request we already answered (the initiator nudging —
    /// our single-shot response may be lost) re-sends the STORED response:
    /// re-signing would orphan the initiator's eventual ack, which pins ONE
    /// response by hash. `via` is the delivering member (possibly a relay);
    /// the counterparty is the request's SIGNED initiator.
    async fn on_request(
        &mut self,
        via: ValidatorIdentity,
        request: TunnelUpgradeRequest,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        let sender = request.fields.initiator_identity;
        if request.verify_signature().is_err() {
            return self.fail_peer(via, "request signature invalid").await;
        }
        if !state.set.contains(sender) {
            return self
                .fail_peer(via, "request from a non-member initiator")
                .await;
        }
        if state.failed.contains(&sender) {
            // the pair already failed this epoch — its nonces are burnt in
            // the replay cache, so no retry can revive it; stay quiet.
            return Ok(());
        }
        if request.fields.epoch != state.epoch || !initiates(sender, self.me) {
            return self.fail_peer(sender, "request from the wrong side").await;
        }
        match state.handshakes.get(&sender) {
            Some(PeerHandshake::AwaitingAck {
                request: stored,
                response,
            }) if stored.hash() == request.hash() => {
                let response = response.clone();
                return self.fan_msg(&ReachabilityMsg::Response(response)).await;
            }
            // stale in-flight duplicate: our ack receipt proves the
            // initiator completed long ago — nothing left to answer.
            Some(PeerHandshake::Done { request_hash, .. }) if *request_hash == request.hash() => {
                return Ok(());
            }
            // a DIFFERENT request over an in-flight/completed handshake is a
            // re-sign the protocol never does — loud, like every mismatch.
            Some(_) => {
                return self
                    .fail_peer(sender, "conflicting handshake request")
                    .await;
            }
            None => {}
        }
        if state.view_state.is_none() {
            // the peer's mesh completed before ours; answer once ours does.
            state.pending_requests.insert(sender, request);
            return Ok(());
        }
        self.resolve_peer(sender).await?;
        let state = self.state.as_mut().expect("still in epoch");
        let nonce = state.next_nonce();
        let view = state.view_state.as_ref().expect("mesh verified");
        let fields = TunnelUpgradeResponseFields {
            request_hash: request.hash(),
            namespace: state.set.namespace.clone(),
            epoch: state.set.epoch,
            valset_root: state.set.valset_root,
            admission_root: state.set.admission_root,
            mesh_version: view.mesh_version,
            responder_identity: self.me,
            initiator_identity: sender,
            responder_wireguard_public_key: self.keypair.public_key(),
            responder_wireguard_endpoint: self.config.wireguard_advertised,
            accepted_allowed_ips: self.overlay.allowed_ips_for(view, sender)?,
            keepalive_seconds: Some(KEEPALIVE_SECONDS),
            expires_at_view: self.view + HANDSHAKE_TTL_VIEWS,
            nonce,
        };
        let response = TunnelUpgradeResponse::sign(fields, &self.config.signer);
        state.handshakes.insert(
            sender,
            PeerHandshake::AwaitingAck {
                request,
                response: response.clone(),
            },
        );
        self.fan_msg(&ReachabilityMsg::Response(response)).await
    }

    /// Initiator side: the peer responded — ack, then validate our plan.
    /// A duplicate of the response we already validated means the responder
    /// never received our single-shot ack: re-send the stored ack VERBATIM,
    /// and never re-validate — each side runs `validate_upgrade_as` exactly
    /// once per peer, so the shared replay cache never sees a nonce twice.
    /// `via` is the delivering member (possibly a relay); the counterparty
    /// is the response's SIGNED responder.
    async fn on_response(
        &mut self,
        via: ValidatorIdentity,
        response: TunnelUpgradeResponse,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        let sender = response.fields.responder_identity;
        if response.verify_signature().is_err() {
            return self.fail_peer(via, "response signature invalid").await;
        }
        if !state.set.contains(sender) {
            return self
                .fail_peer(via, "response from a non-member responder")
                .await;
        }
        if state.failed.contains(&sender) {
            // failed pairs stay failed for the epoch — see `on_request`.
            return Ok(());
        }
        let request = match state.handshakes.get(&sender) {
            Some(PeerHandshake::AwaitingResponse { request }) => request.clone(),
            Some(PeerHandshake::Done {
                response_hash,
                ack: Some(ack),
                ..
            }) if *response_hash == response.hash() => {
                let ack = ack.clone();
                return self.fan_msg(&ReachabilityMsg::Ack(ack)).await;
            }
            _ => {
                return self
                    .fail_peer(sender, "unsolicited handshake response")
                    .await;
            }
        };
        if response.fields.request_hash != request.hash() {
            return self
                .fail_peer(sender, "response does not match our request")
                .await;
        }
        let view = state.view_state.as_ref().expect("mesh verified").clone();
        let fields = TunnelUpgradeAckFields {
            request_hash: request.hash(),
            response_hash: response.hash(),
            namespace: state.set.namespace.clone(),
            epoch: state.set.epoch,
            valset_root: state.set.valset_root,
            admission_root: state.set.admission_root,
            mesh_version: view.mesh_version,
            initiator_identity: self.me,
            responder_identity: sender,
            installed_at_view: self.view,
            expires_at_view: self.view + HANDSHAKE_TTL_VIEWS,
            nonce: state.next_nonce(),
        };
        let ack = TunnelUpgradeAck::sign(fields, &self.config.signer);
        let plan = wireguard::validate_upgrade_as(
            Perspective::Initiator,
            &view,
            &self.config.port_policy,
            &self.overlay,
            self.view,
            &request,
            &response,
            &ack,
            &mut state.replay,
        );
        // send the ack regardless of our local verdict? No: an invalid
        // triple must not be acked into the peer's replay state — fail loud
        // and let the peer's own validation refuse it too.
        match plan {
            Ok(plan) => {
                state.handshakes.insert(
                    sender,
                    PeerHandshake::Done {
                        request_hash: request.hash(),
                        response_hash: response.hash(),
                        ack: Some(ack.clone()),
                    },
                );
                state.plans.insert(sender, plan);
                self.fan_msg(&ReachabilityMsg::Ack(ack)).await?;
                self.advance().await
            }
            Err(err) => {
                state.handshakes.remove(&sender);
                state.failed.insert(sender);
                let reason = format!("handshake validation: {err:?}");
                self.fail_peer(sender, &reason).await?;
                self.advance().await
            }
        }
    }

    /// Responder side: the initiator acked — validate our plan. A duplicate
    /// of the ack that already completed this handshake is dropped without
    /// re-validation (see `on_response` for the replay argument). `via` is
    /// the delivering member (possibly a relay); the counterparty is the
    /// ack's SIGNED initiator.
    async fn on_ack(
        &mut self,
        via: ValidatorIdentity,
        ack: TunnelUpgradeAck,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        let sender = ack.fields.initiator_identity;
        if ack.verify_signature().is_err() {
            return self.fail_peer(via, "ack signature invalid").await;
        }
        if !state.set.contains(sender) {
            return self.fail_peer(via, "ack from a non-member initiator").await;
        }
        if state.failed.contains(&sender) {
            // failed pairs stay failed for the epoch — see `on_request`.
            return Ok(());
        }
        let (request, response) = match state.handshakes.get(&sender) {
            Some(PeerHandshake::AwaitingAck { request, response }) => {
                (request.clone(), response.clone())
            }
            Some(PeerHandshake::Done {
                request_hash,
                response_hash,
                ..
            }) if *request_hash == ack.fields.request_hash
                && *response_hash == ack.fields.response_hash =>
            {
                return Ok(());
            }
            _ => {
                return self.fail_peer(sender, "unsolicited handshake ack").await;
            }
        };
        if ack.fields.request_hash != request.hash() || ack.fields.response_hash != response.hash()
        {
            return self
                .fail_peer(sender, "ack does not match the handshake")
                .await;
        }
        let view = state.view_state.as_ref().expect("mesh verified").clone();
        let plan = wireguard::validate_upgrade_as(
            Perspective::Responder,
            &view,
            &self.config.port_policy,
            &self.overlay,
            self.view,
            &request,
            &response,
            &ack,
            &mut state.replay,
        );
        match plan {
            Ok(plan) => {
                state.handshakes.insert(
                    sender,
                    PeerHandshake::Done {
                        request_hash: request.hash(),
                        response_hash: response.hash(),
                        ack: None,
                    },
                );
                state.plans.insert(sender, plan);
                self.advance().await
            }
            Err(err) => {
                state.handshakes.remove(&sender);
                state.failed.insert(sender);
                let reason = format!("handshake validation: {err:?}");
                self.fail_peer(sender, &reason).await?;
                self.advance().await
            }
        }
    }

    async fn fail_peer(
        &mut self,
        peer: ValidatorIdentity,
        reason: &str,
    ) -> Result<(), ReachabilityError> {
        let pk = self
            .state
            .as_ref()
            .and_then(|state| state.pk_of.get(&peer))
            .cloned();
        let Some(pk) = pk else {
            return Ok(());
        };
        self.emit(ReachabilityEvent::PeerFailed {
            peer: pk,
            reason: reason.to_string(),
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_one_side_of_every_pair_initiates() {
        let low = ValidatorIdentity([1; 32]);
        let high = ValidatorIdentity([2; 32]);
        assert!(initiates(low, high));
        assert!(!initiates(high, low));
        // self-pairs never occur (a node has no tunnel to itself), and the
        // rule is strict either way.
        assert!(!initiates(low, low));
    }

    /// The endpoint-less rendezvous-fallback backoff + budget decision
    /// (change 2 / issue #331): never-attempted always fires; immediately
    /// after an attempt is suppressed; once the resolver's own worst-case
    /// attempt window has elapsed, retrying is allowed again — until the
    /// per-epoch attempt budget is spent, after which no amount of elapsed
    /// time re-arms it. A new epoch resets the budget (a `Retarget` builds a
    /// fresh `EpochState` whose empty `rendezvous_attempted` map yields the
    /// `None` shape again).
    #[test]
    fn rendezvous_fallback_backoff_suppresses_immediate_retry_and_caps_attempts() {
        assert!(
            should_attempt_rendezvous_fallback(None),
            "never attempted before (or a fresh epoch reset the map) — must fire"
        );
        assert!(
            !should_attempt_rendezvous_fallback(Some((Duration::from_millis(1), 1))),
            "1ms after the last attempt — must NOT storm the coordinator"
        );
        assert!(
            !should_attempt_rendezvous_fallback(Some((
                RENDEZVOUS_FALLBACK_BACKOFF - Duration::from_millis(1),
                1
            ))),
            "just under the backoff window — still suppressed"
        );
        assert!(
            should_attempt_rendezvous_fallback(Some((RENDEZVOUS_FALLBACK_BACKOFF, 1))),
            "exactly the backoff window, budget remaining — allowed"
        );
        assert!(
            should_attempt_rendezvous_fallback(Some((
                RENDEZVOUS_FALLBACK_BACKOFF + Duration::from_secs(60),
                RENDEZVOUS_FALLBACK_MAX_ATTEMPTS - 1
            ))),
            "well past the backoff window on the last budgeted attempt — allowed"
        );
        assert!(
            !should_attempt_rendezvous_fallback(Some((
                RENDEZVOUS_FALLBACK_BACKOFF + Duration::from_secs(3600),
                RENDEZVOUS_FALLBACK_MAX_ATTEMPTS
            ))),
            "budget spent — suppressed no matter how much time has passed; only the next \
             epoch's fresh EpochState (the None shape above) re-arms the sweep"
        );
        assert!(
            !should_attempt_rendezvous_fallback(Some((
                Duration::from_secs(3600),
                RENDEZVOUS_FALLBACK_MAX_ATTEMPTS + 1
            ))),
            "past the cap stays suppressed (defensive: the counter never exceeds the cap in \
             practice, but the decision must not wrap back to allowed)"
        );
    }

    mod nat_pump {
        use super::super::*;
        use tokio::net::UdpSocket;

        /// Wait (bounded) until a resolver's rendezvous establishment lands —
        /// construction returns before discovery now, so tests that need a
        /// live registration wait here first.
        async fn ready(resolver: &NatResolver) {
            let mut status = resolver.status().expect("resolver has coordinators");
            tokio::time::timeout(Duration::from_secs(5), async {
                while !matches!(*status.borrow_and_update(), RendezvousStatus::Ready { .. }) {
                    status.changed().await.expect("establish task alive");
                }
            })
            .await
            .expect("rendezvous must establish against a live coordinator");
        }

        #[tokio::test]
        async fn passive_resolver_punches_back_while_idle() {
            // A real coordinator, open policy.
            let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let coord_addr = coord_sock.local_addr().unwrap();
            tokio::spawn(nat_traversal::run_coordinator(
                coord_sock,
                nat_traversal::AuthPolicy::Open { require_pop: false },
            ));

            let a_key = binding::node_key(ValidatorIdentity([0xaa; 32]));
            let b_key = binding::node_key(ValidatorIdentity([0xbb; 32]));
            let mut a = NatResolver::bind(a_key, vec![coord_addr], None)
                .await
                .unwrap();
            let b = NatResolver::bind(b_key, vec![coord_addr], None)
                .await
                .unwrap();
            ready(&a).await;
            ready(&b).await;

            // B NEVER calls resolve. Under the pre-pump code its socket sat
            // deaf outside resolve() windows: the coordinator's PunchSync
            // fan-out was eaten unanswered, B never punched, and A's resolve
            // failed with "hole-punch failed after 3 tries". The pump answers
            // from B's side while B is idle.
            let advertised: SocketAddr = "203.0.113.9:1".parse().unwrap();
            let resolution = a
                .resolve(b_key, advertised)
                .await
                .expect("punch completes against an idle peer");
            match resolution {
                Resolution::Punched(_) => {}
                other => panic!("expected a punched path, got {other:?}"),
            }
        }

        #[tokio::test]
        async fn keepalive_readvertises_hold_the_registration_past_the_ttl() {
            // A coordinator whose registrations expire after 1 second.
            let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let coord_addr = coord_sock.local_addr().unwrap();
            let coordinator = nat_traversal::Coordinator::with_policy_and_ttl(
                nat_traversal::AuthPolicy::Open { require_pop: false },
                1,
            );
            tokio::spawn(nat_traversal::run_coordinator_with(coord_sock, coordinator));

            // A keeps itself alive on a 300ms keepalive; X registers once and
            // goes silent.
            let a_key = binding::node_key(ValidatorIdentity([0x0a; 32]));
            let x_key = binding::node_key(ValidatorIdentity([0x0f; 32]));
            let a = NatResolver::bind_with_keepalive(
                a_key,
                vec![coord_addr],
                None,
                Duration::from_millis(300),
            )
            .await
            .unwrap();
            ready(&a).await;
            let x = nat_traversal::NatClient::bind(x_key, coord_addr)
                .await
                .unwrap();
            x.register().await.unwrap();

            // Whole seconds: `now_secs()` truncates, so a 1.x s sleep can look
            // like Δ=1 ≤ ttl. 2.5 s guarantees an integer-second delta ≥ 2.
            tokio::time::sleep(Duration::from_millis(2_500)).await;

            // A probe client resolves A (kept alive) but not X (expired).
            let probe = nat_traversal::NatClient::bind(
                binding::node_key(ValidatorIdentity([0x01; 32])),
                coord_addr,
            )
            .await
            .unwrap();
            tokio::time::timeout(Duration::from_secs(2), probe.lookup(a_key))
                .await
                .expect("bounded")
                .expect("keepalives held A's registration past the TTL");
            let miss = tokio::time::timeout(Duration::from_secs(1), probe.lookup(x_key)).await;
            assert!(
                miss.is_err() || miss.unwrap().is_err(),
                "X registered once, sent no keepalives, and must have expired"
            );
        }

        #[tokio::test]
        async fn keepalives_survive_a_busy_resolve() {
            // The keepalive lives on its own send-only task, so a resolve()
            // that runs its full budget cannot starve it past the TTL. Rig: a
            // 1s-TTL coordinator; X is registered (its test task readvertises
            // every 300ms) but SILENT — it never punches — so A's resolve
            // grinds through all its tries (~4s of continuous pump busyness,
            // several times the TTL). Under an in-pump keepalive tick, A's
            // registration would expire mid-resolve; with the split task it
            // must survive.
            let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let coord_addr = coord_sock.local_addr().unwrap();
            let coordinator = nat_traversal::Coordinator::with_policy_and_ttl(
                nat_traversal::AuthPolicy::Open { require_pop: false },
                1,
            );
            tokio::spawn(nat_traversal::run_coordinator_with(coord_sock, coordinator));

            let a_key = binding::node_key(ValidatorIdentity([0x1a; 32]));
            let x_key = binding::node_key(ValidatorIdentity([0x1f; 32]));
            let mut a = NatResolver::bind_with_keepalive(
                a_key,
                vec![coord_addr],
                None,
                Duration::from_millis(300),
            )
            .await
            .unwrap();
            ready(&a).await;
            // X: a raw client (answers nothing) kept registered by a test task.
            let x = std::sync::Arc::new(
                nat_traversal::NatClient::bind(x_key, coord_addr)
                    .await
                    .unwrap(),
            );
            x.register().await.unwrap();
            let x_keepalive = x.clone();
            tokio::spawn(async move {
                let mut nonce = 0u64;
                loop {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    nonce += 1;
                    let _ = x_keepalive.readvertise(nonce).await;
                }
            });

            // The busy resolve: X resolves but never punches back, so this
            // fails only after every try's punch window closes.
            let advertised: SocketAddr = "203.0.113.9:1".parse().unwrap();
            let err = a
                .resolve(x_key, advertised)
                .await
                .expect_err("a silent peer cannot be punched");
            assert!(err.contains("hole-punch failed"), "unexpected error: {err}");

            // A's own registration survived the busy window.
            let probe = nat_traversal::NatClient::bind(
                binding::node_key(ValidatorIdentity([0x11; 32])),
                coord_addr,
            )
            .await
            .unwrap();
            tokio::time::timeout(Duration::from_secs(2), probe.lookup(a_key))
                .await
                .expect("bounded")
                .expect("keepalives must survive a busy resolve");
        }

        #[tokio::test]
        async fn rendezvous_establishes_in_background_when_the_coordinator_comes_up_late() {
            // The boot-4 shape from the field: a node boots while its
            // coordinator is unreachable (machine woke before Wi-Fi, or the
            // coordinator restarted). Reserve an address, then leave it DARK.
            let placeholder = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let coord_addr = placeholder.local_addr().unwrap();
            drop(placeholder);

            let a_key = binding::node_key(ValidatorIdentity([0x2a; 32]));
            let b_key = binding::node_key(ValidatorIdentity([0x2b; 32]));
            let mut a = NatResolver::bind_with_keepalive(
                a_key,
                vec![coord_addr],
                None,
                Duration::from_millis(300),
            )
            .await
            .expect("a dark coordinator must not fail resolver construction");

            // While establishment retries, a resolve is an HONEST, PROMPT
            // error — never a hang (a caller parked on this reply is exactly
            // the silent forever-stall this path used to produce).
            let advertised: SocketAddr = "203.0.113.9:1".parse().unwrap();
            let early = tokio::time::timeout(
                Duration::from_secs(1),
                a.resolve(b_key, advertised),
            )
            .await
            .expect("resolve during establishment must answer promptly, not hang");
            assert!(
                early.is_err(),
                "rendezvous cannot resolve before the coordinator ever answered"
            );

            // The coordinator comes up LATE, on the same address...
            let coord_sock = UdpSocket::bind(coord_addr).await.unwrap();
            tokio::spawn(nat_traversal::run_coordinator(
                coord_sock,
                nat_traversal::AuthPolicy::Open { require_pop: false },
            ));

            // ...and the resolver heals on its own: reflexive discovery and
            // registration land without any caller re-driving construction.
            let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
            while a.reflexive().is_none() {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "the resolver must establish rendezvous once the coordinator answers"
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            // Registration is live at the coordinator: a probe can look A up.
            let probe = nat_traversal::NatClient::bind(
                binding::node_key(ValidatorIdentity([0x2c; 32])),
                coord_addr,
            )
            .await
            .unwrap();
            tokio::time::timeout(Duration::from_secs(2), probe.lookup(a_key))
                .await
                .expect("bounded")
                .expect("the late-established registration must be resolvable");
        }

        #[tokio::test]
        async fn no_coordinators_still_passes_through_to_advertised() {
            let key = binding::node_key(ValidatorIdentity([0x33; 32]));
            let mut r = NatResolver::bind(key, Vec::new(), None).await.unwrap();
            assert_eq!(r.reflexive(), None);
            let advertised: SocketAddr = "203.0.113.7:51820".parse().unwrap();
            assert!(matches!(
                r.resolve(key, advertised).await,
                Ok(Resolution::Advertised)
            ));
        }
    }
}
