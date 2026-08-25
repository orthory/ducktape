//! The reachability orchestrator: the driver that takes epoch cutover events
//! from the node and turns them into a live WireGuard mesh — record gossip ->
//! signed advertisements -> `MeshView::verify` -> pairwise tunnel handshakes
//! -> ONE `apply_peer_tunnels` call per epoch through a `WireGuardEffect`.
//!
//! Layout: this module is the EXECUTOR — the command loop, the effect
//! writers, and the message handlers. The per-epoch data and every pure
//! decision over it (phase, next step, nudge re-offers, nonce admission,
//! relay slotting, the rendezvous budget) live in `epoch`; the coordinator
//! rendezvous runtime lives in `rendezvous`.
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
use tokio::sync::mpsc;
use wireguard::effect::{
    PeerTunnelConfig, WireGuardEffect, apply_peer_tunnels, plan_peer_configs, update_peer_tunnels,
};
use wireguard::{
    Endpoint, EndpointAdvertisement, EndpointRecord, MeshVersion, MeshView, OverlayPolicy,
    Perspective, PortPolicy, SignedEndpointRecord, TunnelInstallPlan, TunnelUpgradeAck,
    TunnelUpgradeAckFields, TunnelUpgradeRequest, TunnelUpgradeRequestFields,
    TunnelUpgradeResponse, TunnelUpgradeResponseFields, UpgradeError, ValidatorIdentity,
    compute_mesh_version,
};

use crate::binding;
use crate::epoch::{
    Admission, EpochState, PeerHandshake, Phase, RelayVerdict, RelayedHandshake, Role, Step,
    epoch_nonce_seed,
};
use crate::keys::{KeyError, WireGuardKeypair};
use crate::msg::{MsgError, ReachabilityMsg};
use crate::rendezvous::{EndpointResolver, Resolution};
use crate::store::{self, PersistedMesh};

/// Views a handshake message stays valid for. Tight on purpose: a handshake
/// is a live conversation, not a standing record. (Endpoint RECORDS carry no
/// TTL at all — signed once per epoch and re-offered verbatim, their lifetime
/// IS the epoch tuple; a record TTL would expire every record on any epoch
/// that outlives it.)
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

/// Nudge ticks an untargeted plane is given before it is called a defect.
///
/// Small, because the only legitimate window is the boot race between wiring
/// the plane and the boot `Retarget` that follows it a few statements later —
/// anything past that never resolves on its own.
const UNTARGETED_NUDGE_GRACE: u64 = 3;

/// an identity's first four bytes, hex — the form every other plane's logs
/// use for a peer, and short enough to read a gossip trace in a terminal.
fn short(id: ValidatorIdentity) -> String {
    id.0[..4].iter().map(|b| format!("{b:02x}")).collect()
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
    /// A peer's signed CONTROL endpoint (its mesh address) was accepted with
    /// a value that differs from the last one observed for that identity —
    /// the node feeds it to the mesh transport's address book. Emitted
    /// only-on-change from every record/advert acceptance path and the boot
    /// restore; `peer` is the owner's raw ed25519 identity bytes.
    ControlEndpointObserved {
        peer: ValidatorIdentity,
        control_endpoint: SocketAddr,
    },
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

/// Where a handshake message stands relative to this node: the party it is
/// for, the party that signed it, or neither — a message between two OTHER
/// members that this node carries for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bearing {
    Addressee,
    Author,
    Bystander,
}

fn bearing(
    me: ValidatorIdentity,
    author: ValidatorIdentity,
    addressee: ValidatorIdentity,
) -> Bearing {
    match (addressee == me, author == me) {
        (true, _) => Bearing::Addressee,
        (false, true) => Bearing::Author,
        (false, false) => Bearing::Bystander,
    }
}

/// The orchestrator's cross-epoch context: everything that outlives an
/// epoch, and the writers every effect goes through. The per-epoch state is
/// NOT here — [`run`] owns it and hands it to each handler, so no handler
/// has to re-borrow an optional epoch mid-flight.
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
    /// How many nudge ticks have found the plane without an epoch.
    ///
    /// A plane that was wired but never `Retarget`ed is a black hole in both
    /// directions — it drops every inbound record and advert and sends none of
    /// its own — and silence here costs a live session to diagnose from p2p
    /// byte counters, with the symptom misread as a NAT problem.
    untargeted_nudges: u64,
    /// Nudge ticks since this plane started — the clock the per-peer heal
    /// cooldown counts in (see `epoch::EpochState::request_heal`).
    nudges: u64,
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
    /// the last CONTROL endpoint observed per identity — the only-on-change
    /// ledger behind [`ReachabilityEvent::ControlEndpointObserved`].
    /// deliberately epoch-independent: a cutover must not re-announce
    /// unchanged addresses.
    control_endpoints: BTreeMap<ValidatorIdentity, SocketAddr>,
}

/// Drive the reachability plane until `Shutdown` (clean exit) or a channel
/// closes (error). One call outlives every epoch; `Retarget` events move it
/// between epochs. The per-epoch state machine ([`epoch::Phase`]):
///
/// 1. **Bind.** Derive the epoch's `ActiveValidatorSet` via
///    [`binding::active_set`]; fresh replay cache and nonce counter.
/// 2. **Records.** Send our `EndpointRecord` (WG public key, control +
///    wireguard endpoints) to every other member; collect theirs, re-sending
///    ours on first contact so joining order can't strand anyone.
/// 3. **Adverts.** With ALL records held: `compute_mesh_version`, sign the
///    `EndpointAdvertisement`, fan out; collect everyone's.
/// 4. **Handshakes.** With all advertisements held: `MeshView::verify`; emit
///    `MeshReady` or `EpochFailed`. Initiator per [`initiates`]: resolve
///    the peer's endpoint, then request -> response -> ack; both sides
///    validate the triple via `validate_upgrade_as` from their own
///    perspective under the `OverlayPolicy::ula_v6` overlay and one shared
///    per-epoch replay cache. Single-shot loss at any stage heals by `Nudge`
///    re-offers: stored messages re-sent verbatim (never re-signed) into
///    duplicate-tolerant receivers, so each side still validates the triple
///    exactly once.
/// 5. **Applied.** When every peer has a validated plan or a `PeerFailed`:
///    tear down any previous interface and make the epoch's ONE
///    `apply_peer_tunnels` call; emit `TunnelsApplied`.
///
/// Every step is DECIDED by [`EpochState::next_step`] over the accumulated
/// state and EXECUTED here; every message handler mutates the state and
/// then re-decides.
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
    let mut driver = Driver::new(&config, effect, resolver, events)?;
    let mut epoch: Option<EpochState> = None;
    while let Some(command) = commands.recv().await {
        match command {
            ReachabilityCommand::Retarget(event) => epoch = driver.retarget(event).await?,
            ReachabilityCommand::InstallInvitePeer {
                peer,
                wireguard_public_key,
                endpoint,
                reply,
            } => {
                driver
                    .install_invite_peer(
                        epoch.as_ref(),
                        peer,
                        wireguard_public_key,
                        endpoint,
                        reply,
                    )
                    .await?
            }
            ReachabilityCommand::BootstrapCoordinatedInvitePeer {
                peer,
                wireguard_public_key,
                intro,
                reply,
            } => {
                driver
                    .bootstrap_coordinated_invite_peer(
                        epoch.as_ref(),
                        peer,
                        wireguard_public_key,
                        intro,
                        reply,
                    )
                    .await?
            }
            ReachabilityCommand::SendResolverDatagram { endpoint, bytes } => {
                driver.send_resolver_datagram(endpoint, bytes).await
            }
            ReachabilityCommand::Deliver { from, bytes } => {
                driver.deliver(epoch.as_mut(), from, bytes).await?
            }
            ReachabilityCommand::ViewTick(view) => driver.observe_view(view),
            ReachabilityCommand::Nudge => driver.nudge(epoch.as_mut()).await?,
            ReachabilityCommand::Shutdown => return driver.shutdown(),
        }
    }
    Err(ReachabilityError::ChannelClosed)
}

impl<'a, E, R> Driver<'a, E, R>
where
    E: WireGuardEffect,
    R: EndpointResolver,
{
    fn new(
        config: &'a ReachabilityConfig,
        effect: E,
        resolver: R,
        events: mpsc::Sender<ReachabilityEvent>,
    ) -> Result<Self, ReachabilityError> {
        let (keypair, _generated) = WireGuardKeypair::load_or_generate(&config.wireguard_key_file)?;
        Ok(Self {
            me: binding::identity_of(&config.signer.public_key()),
            overlay: OverlayPolicy::ula_v6(config.chain_id.clone()),
            interface: binding::interface_name(&config.chain_id),
            keypair,
            config,
            effect,
            resolver,
            events,
            view: 0,
            untargeted_nudges: 0,
            nudges: 0,
            interface_live: false,
            base_peers: None,
            restore_tried: false,
            invite_peers: BTreeMap::new(),
            control_endpoints: BTreeMap::new(),
        })
    }

    // ----- writers: the few places an effect happens -----------------------

    async fn emit(&self, event: ReachabilityEvent) -> Result<(), ReachabilityError> {
        self.events
            .send(event)
            .await
            .map_err(|_| ReachabilityError::ChannelClosed)
    }

    async fn send_msg(
        &self,
        state: &EpochState,
        to: ValidatorIdentity,
        msg: &ReachabilityMsg,
    ) -> Result<(), ReachabilityError> {
        let Some(pk) = state.route_to(to) else {
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
    async fn fan_msg(
        &self,
        state: &EpochState,
        msg: &ReachabilityMsg,
    ) -> Result<(), ReachabilityError> {
        for peer in &state.peers {
            self.send_msg(state, *peer, msg).await?;
        }
        Ok(())
    }

    async fn fail_peer(
        &self,
        state: &EpochState,
        peer: ValidatorIdentity,
        reason: &str,
    ) -> Result<(), ReachabilityError> {
        let Some(pk) = state.pk_of.get(&peer).cloned() else {
            return Ok(());
        };
        self.emit(ReachabilityEvent::PeerFailed {
            peer: pk,
            reason: reason.to_string(),
        })
        .await
    }

    /// announce an accepted CONTROL endpoint to the node, only when it
    /// differs from the last one observed for that identity. one ledger for
    /// every acceptance path (member/standby role, records, adverts, the
    /// boot restore), so the mesh address book upstream is never churned by
    /// re-gossip of an unchanged address. own endpoint is skipped — the node
    /// does not dial itself.
    async fn observe_control_endpoint(
        &mut self,
        owner: ValidatorIdentity,
        endpoint: Endpoint,
    ) -> Result<(), ReachabilityError> {
        if owner == self.me {
            return Ok(());
        }
        let socket = endpoint.socket_addr();
        if !control_endpoint_changed(&mut self.control_endpoints, owner, socket) {
            return Ok(());
        }
        self.emit(ReachabilityEvent::ControlEndpointObserved {
            peer: owner,
            control_endpoint: socket,
        })
        .await
    }

    /// The one writer for the interface's desired peer set: reconfigure a
    /// live interface in place, or bring it up. On success the interface is
    /// live and carries a base the pre-warm layer may merge over — an empty
    /// one when nothing stronger was applied yet (a standby's or a
    /// join-window interface exists purely for its merged layers).
    fn push_interface(&mut self, peers: &[PeerTunnelConfig]) -> Result<(), E::Error> {
        // the plane's overlay is ula_v6: the local side is the same
        // identity-derived /128 every validated plan carries.
        let local_interface_ips = self.overlay.identity_allowed_ips(self.me);
        if self.interface_live {
            update_peer_tunnels(
                &mut self.effect,
                self.interface.clone(),
                self.keypair.private_key_bytes(),
                self.config.wireguard_port,
                &local_interface_ips,
                peers,
            )?;
        } else {
            apply_peer_tunnels(
                &mut self.effect,
                self.interface.clone(),
                self.keypair.private_key_bytes(),
                self.config.wireguard_port,
                &local_interface_ips,
                peers,
            )?;
        }
        self.interface_live = true;
        if self.base_peers.is_none() {
            self.base_peers = Some(BTreeMap::new());
        }
        Ok(())
    }

    /// Tear the live interface down, best-effort: every caller is on its
    /// way to a rebuild or an exit, where the teardown's error detail does
    /// not change what happens next.
    fn teardown_interface(&mut self) {
        if !self.interface_live {
            return;
        }
        let _ = self.effect.remove_interface();
        self.interface_live = false;
    }

    /// The interface's full desired peer list from the stronger layers
    /// already merged in `merged`: the join-window invite layer goes on
    /// last, and a tunnel to this node itself never exists (a restored base
    /// could in principle carry an identity that since became us).
    fn assemble_peers(
        &mut self,
        mut merged: BTreeMap<ValidatorIdentity, PeerTunnelConfig>,
    ) -> Vec<PeerTunnelConfig> {
        self.merge_invite_layer(&mut merged);
        merged.remove(&self.me);
        merged.into_values().collect()
    }

    /// Merge the join-window invite layer into an assembled peer map: an
    /// invite peer never overrides an entry the stronger layers (validated
    /// plans, restored mesh, pre-warm records) already carry — and once one
    /// with a CONCRETE endpoint exists for the same identity, the invite
    /// entry has served its purpose and dissolves.
    ///
    /// An endpoint-less stronger entry (a NATed peer's record advertises
    /// nothing) instead has the invite entry's endpoint grafted in: the
    /// invite endpoint is OBSERVED — the intro datagram's source, or the
    /// rendezvous-resolved path — and dropping it for `None` on an
    /// endpoint-less pair leaves BOTH sides unable to initiate, killing the
    /// live tunnel the join rode (and with it a fresh resident's only
    /// statesync source, right as its standing lands). The retained endpoint
    /// is what carries the cutover: a reconfigure-in-place apply keeps the
    /// tunnel's live sessions outright (same key + same endpoint = unchanged
    /// config), and the epoch apply's full interface rebuild can re-initiate
    /// immediately instead of deadlocking endpoint-less.
    fn merge_invite_layer(&mut self, merged: &mut BTreeMap<ValidatorIdentity, PeerTunnelConfig>) {
        self.invite_peers
            .retain(|id, invite| match merged.get_mut(id) {
                Some(entry) => {
                    let graft = entry.endpoint.is_none()
                        && entry.wireguard_public_key == invite.wireguard_public_key;
                    if graft {
                        entry.endpoint = invite.endpoint;
                    }
                    // grafting keeps the invite entry (later re-merges rebuild
                    // `merged` from the still-endpoint-less records); a concrete
                    // or re-keyed stronger entry retires it.
                    graft
                }
                None => true,
            });
        for (id, cfg) in &self.invite_peers {
            merged.entry(*id).or_insert_with(|| cfg.clone());
        }
    }

    /// Persist the mesh snapshot the cold-restart restore reads back: the
    /// member adverts AND the accepted standby records. The records ride
    /// along because a parked resident cannot re-introduce itself to a
    /// member that forgot its WireGuard key — its invite token was consumed
    /// at admission and its every remaining transport rides this overlay —
    /// so this file is its only way back onto a rebooted member's interface.
    async fn persist_mesh(&self, state: &EpochState) -> Result<(), ReachabilityError> {
        let Some(path) = self.config.persist_file.as_deref() else {
            return Ok(());
        };
        let mesh = PersistedMesh::new(
            self.config.chain_id.clone(),
            state.epoch,
            state.adverts.values().cloned().collect(),
            state.standby_records.values().cloned().collect(),
        );
        let Err(err) = store::save(path, &mesh) else {
            return Ok(());
        };
        self.emit(ReachabilityEvent::PersistFailed {
            reason: err.to_string(),
        })
        .await
    }

    // ----- commands ---------------------------------------------------------

    fn observe_view(&mut self, view: u64) {
        self.view = self.view.max(view);
    }

    async fn send_resolver_datagram(&mut self, endpoint: SocketAddr, bytes: Vec<u8>) {
        let _ = self.resolver.send_datagram(endpoint, bytes).await;
    }

    /// Drain and exit; the interface is torn down on the way out.
    fn shutdown(&mut self) -> Result<(), ReachabilityError> {
        self.teardown_interface();
        Ok(())
    }

    /// Boot or epoch cutover: bind the epoch, run the one-time cold-restart
    /// restore, fan out our record, and take every step the fresh state
    /// already satisfies (a single-member network is a complete mesh).
    /// Returns the epoch to drive from here on — `None` when this node is
    /// neither a member nor a standby of it.
    async fn retarget(
        &mut self,
        event: MeshEpochEvent,
    ) -> Result<Option<EpochState>, ReachabilityError> {
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
        let is_member = identities.contains(&self.me);
        let is_standby = standby_ids.contains(&self.me);
        let role = match (is_member, is_standby) {
            (true, _) => Role::Member,
            (false, true) => Role::Standby,
            (false, false) => {
                self.stand_down(event.epoch).await?;
                return Ok(None);
            }
        };
        // the plane's one lifecycle fact per epoch, and the positive half of
        // the `no_epoch_target` warn in `nudge`: an operator reading a node
        // that gossips nothing needs to know whether this line ever printed.
        match role {
            Role::Member => tracing::info!(
                target: "ducktape::reachability",
                epoch = event.epoch,
                members = identities.len(),
                standbys = standby_ids.len(),
                view = event.current_view,
                "plane targeted: member at epoch"
            ),
            Role::Standby => tracing::info!(
                target: "ducktape::reachability",
                epoch = event.epoch,
                members = identities.len(),
                standbys = standby_ids.len(),
                view = event.current_view,
                "plane targeted: standby at epoch"
            ),
        }
        let restored_standbys = if self.restore_tried {
            Vec::new()
        } else {
            self.restore_tried = true;
            self.restore(&event).await?
        };
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
        // the epoch's first signed nonce — wall-clock-seeded so a reboot's
        // re-signed record strictly supersedes its previous life's (see
        // `epoch_nonce_seed`); the epoch's counter starts past it.
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
                nonce: epoch_nonce_seed(),
            },
            &self.config.signer,
        );
        let mut state = EpochState::new(
            event.epoch,
            role,
            set,
            peers,
            standby_ids,
            pk_of,
            own.clone(),
        );
        // the restored standby records seed the boot epoch's pre-warm layer
        // as if just delivered — the epoch's own apply REPLACES the restored
        // interface, and a parked standby cannot re-deliver its record over
        // the dead overlay it is parked behind. Nonces stay unseeded so the
        // owner's live re-offer re-runs the full accept path: an idempotent
        // reinstall plus the first-contact gossip-back it heals by.
        for signed in restored_standbys {
            let identity = signed.record.validator_identity;
            state
                .prewarm_peers
                .insert(identity, self.standby_peer_config(&signed.record));
            state.standby_records.insert(identity, signed);
        }
        let own_record = ReachabilityMsg::Record(own);
        for peer in &state.peers {
            self.send_msg(&state, *peer, &own_record).await?;
        }
        match role {
            Role::Standby => Ok(Some(state)),
            Role::Member => {
                // seed the pre-warm layer's counterparties too (a lost send
                // heals by nudge; a standby with no route yet just misses
                // this round).
                for standby in &state.standbys {
                    self.send_msg(&state, *standby, &own_record).await?;
                }
                self.advance(&mut state).await?;
                Ok(Some(state))
            }
        }
    }

    /// Be inert, not wrong: this node is in neither set of the epoch, so it
    /// drops any live tunnel and runs no epoch until the next cutover.
    async fn stand_down(&mut self, epoch: u64) -> Result<(), ReachabilityError> {
        self.teardown_interface();
        self.base_peers = None;
        self.emit(ReachabilityEvent::EpochFailed {
            epoch,
            reason: "this node is neither a member nor a standby of the epoch".into(),
        })
        .await
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
    ///
    /// Returns the resident-gated standby records it reinstalled, for the
    /// caller to seed into the boot epoch's pre-warm layer — on the
    /// restored interface alone they would die at that very replace.
    async fn restore(
        &mut self,
        event: &MeshEpochEvent,
    ) -> Result<Vec<SignedEndpointRecord>, ReachabilityError> {
        let Some(path) = &self.config.persist_file else {
            return Ok(Vec::new());
        };
        let mesh = match store::load(path, &self.config.chain_id) {
            Ok(Some(mesh)) => mesh,
            Ok(None) => return Ok(Vec::new()),
            Err(err) => {
                self.emit(ReachabilityEvent::RestoreFailed {
                    reason: err.to_string(),
                })
                .await?;
                return Ok(Vec::new());
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
        // the boot epoch's RESIDENT set gates the persisted standby records
        // exactly as its member set gates the adverts: a departed standby's
        // tunnel is dead weight. One still parked is why these persist at
        // all — it cannot re-introduce itself to a member that forgot its
        // WireGuard key (invite token consumed at admission, every remaining
        // transport rides this overlay), so only this reinstall lets its
        // ongoing handshake retries land again after a reboot.
        let standby_ids: HashSet<ValidatorIdentity> = event
            .standbys
            .iter()
            .map(binding::identity_of)
            .filter(|id| *id != self.me && !member_pk_of.contains_key(id))
            .collect();
        let standby_records: Vec<SignedEndpointRecord> = mesh
            .standby_records
            .iter()
            .filter(|signed| standby_ids.contains(&signed.record.validator_identity))
            .cloned()
            .collect();
        // the restored records' control endpoints feed the mesh address
        // book exactly like live acceptances — a cold restart's book starts
        // from the same signed evidence the tunnels do.
        for record in &records {
            self.observe_control_endpoint(record.validator_identity, record.control_endpoint)
                .await?;
        }
        for signed in &standby_records {
            self.observe_control_endpoint(
                signed.record.validator_identity,
                signed.record.control_endpoint,
            )
            .await?;
        }
        if records.is_empty() && standby_records.is_empty() {
            return Ok(Vec::new());
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
            let allowed_ips = self.overlay.identity_allowed_ips(record.validator_identity);
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
        for signed in &standby_records {
            peers.insert(
                signed.record.validator_identity,
                self.standby_peer_config(&signed.record),
            );
        }
        let peer_count = peers.len();
        // the join-window invite layer rides the restore apply too (a node
        // rebooting mid-window keeps its invite tunnel), but never enters
        // the restored BASE below — the base is the persisted mesh only.
        // And the invite bootstrap may have brought the interface up before
        // the first epoch event (a NATed member re-running first contact at
        // boot) — the writer reconfigures it rather than re-creating it, so
        // the restore neither dies on `AlreadyCreated` nor drops the live
        // join tunnel.
        let parts = self.assemble_peers(peers.clone());
        match self.push_interface(&parts) {
            Ok(()) => {
                // the restored mesh — standby entries included — is the
                // interface's base; the pre-warm layer merges its live
                // record-derived peers over it (same identity: fresher wins).
                self.base_peers = Some(peers);
                self.emit(ReachabilityEvent::MeshRestored {
                    epoch: mesh.epoch,
                    interface: self.interface.clone(),
                    peers: peer_count,
                })
                .await?;
                Ok(standby_records)
            }
            Err(err) => {
                self.emit(ReachabilityEvent::RestoreFailed {
                    reason: format!("wireguard effect: {err:?}"),
                })
                .await?;
                Ok(Vec::new())
            }
        }
    }

    /// A standby record's peer tunnel config, endpoint taken VERBATIM (no
    /// rendezvous resolution): the parked side initiates (and roams), so
    /// its recorded endpoint is a first target, not a requirement — the
    /// install's real cargo is the WireGuard key.
    fn standby_peer_config(&self, record: &EndpointRecord) -> PeerTunnelConfig {
        PeerTunnelConfig {
            wireguard_public_key: record.wireguard_public_key,
            endpoint: record.wireguard_endpoint.map(|e| e.socket_addr()),
            allowed_ips: self.overlay.identity_allowed_ips(record.validator_identity),
            keepalive_seconds: Some(KEEPALIVE_SECONDS),
        }
    }

    /// Re-offer whatever the epoch is still waiting on (decided by
    /// [`EpochState::reoffers`]), then run the role's endpoint-less
    /// rendezvous sweep. Both sweeps ride the same backoff + bounded
    /// per-epoch budget, which keeps the nudge cadence from hammering the
    /// coordinator and from sweeping an unpunchable peer forever — a sweep
    /// goes quiet once the budget is spent and re-arms only at the next
    /// epoch's `Retarget`.
    async fn nudge(&mut self, epoch: Option<&mut EpochState>) -> Result<(), ReachabilityError> {
        self.nudges += 1;
        let Some(state) = epoch else {
            // A few ticks of this are the boot race — the fresh-boot path
            // sends its Retarget right after wiring, and a nudge can beat
            // it. Past the grace it is a wiring defect: this plane will
            // never gossip and never accept, for the life of the process.
            self.untargeted_nudges += 1;
            let past_grace = self.untargeted_nudges >= UNTARGETED_NUDGE_GRACE;
            let periodic = self.untargeted_nudges.is_multiple_of(64);
            if past_grace && (self.untargeted_nudges == UNTARGETED_NUDGE_GRACE || periodic) {
                tracing::warn!(
                    target: "ducktape::reachability",
                    reason = "no_epoch_target",
                    attempts = self.untargeted_nudges,
                    "this reachability plane was never told its epoch — it is dropping \
                     every record and advert it receives and sending none of its own"
                );
            }
            return Ok(());
        };
        self.untargeted_nudges = 0;
        state.expire_relays(self.view);
        for (peer, msg) in state.reoffers() {
            self.send_msg(state, peer, &msg).await?;
        }
        match state.role {
            Role::Member => {
                self.heal_behind_peers(state).await?;
                self.sweep_member_rendezvous_fallback(state).await
            }
            Role::Standby => self.sweep_standby_rendezvous_fallback(state).await,
        }
    }

    /// THE HEAL: a peer still gossiping phase-A at a node whose sets have
    /// locked is a peer that never got our half. Its record and advert are
    /// dropped by the phase gates, but the drop RECORDS the ask
    /// (`request_heal`), and one nudge later this sends our record and
    /// advert back.
    ///
    /// Without this, missing one fan-out is permanent: the exchange is
    /// one-shot and the sender moves on. That loss is routine — a member
    /// learns how to DIAL a promoted joiner from the very record that
    /// completes its own assembly, so its reply goes out microseconds before
    /// the link exists and the lane drops it; the joiner then retries
    /// forever into a node that will not answer until the next cutover.
    /// Rate: at most one record+advert pair per asking peer per tick, and
    /// only to a peer that asked by gossiping at us.
    async fn heal_behind_peers(&mut self, state: &mut EpochState) -> Result<(), ReachabilityError> {
        for (peer, msg) in state.heal_sends(self.nudges) {
            self.send_msg(state, peer, &msg).await?;
        }
        Ok(())
    }

    /// Retry the by-identity rendezvous fallback for any MEMBER peer that
    /// is still endpoint-less and still missing a punched override —
    /// `resolve_peer`'s single attempt at handshake time can lose the race
    /// against the peer's own coordinator registration (both sides often
    /// boot together).
    async fn sweep_member_rendezvous_fallback(
        &mut self,
        state: &mut EpochState,
    ) -> Result<(), ReachabilityError> {
        let retry_targets: Vec<ValidatorIdentity> = state
            .peers
            .iter()
            .copied()
            .filter(|peer| {
                let unresolved = !state.overrides.contains_key(peer);
                let endpoint_less = state
                    .view()
                    .and_then(|view| view.record(*peer))
                    .is_some_and(|record| record.wireguard_endpoint.is_none());
                unresolved && endpoint_less
            })
            .collect();
        for peer in retry_targets {
            self.resolve_peer(state, peer).await?;
        }
        Ok(())
    }

    /// The standby's half of the sweep: rendezvous any member whose
    /// EFFECTIVE pre-warm entry (the pre-warm layer merged over the restored
    /// base, `sync_prewarm`'s own layering) is endpoint-less. After a reboot
    /// that is every fully-NATed member: `restore()` reinstalls them
    /// endpoint-less from the persisted mesh, and their live records cannot
    /// arrive to replace them — plane gossip rides the very tunnels the
    /// missing endpoints keep down. A member with no entry in either layer
    /// is NOT swept: with no record there is no WireGuard key to install,
    /// and live assembly still owes us the record itself.
    async fn sweep_standby_rendezvous_fallback(
        &mut self,
        state: &mut EpochState,
    ) -> Result<(), ReachabilityError> {
        let targets: Vec<ValidatorIdentity> = state
            .peers
            .iter()
            .copied()
            .filter(|peer| {
                let effective = state
                    .prewarm_peers
                    .get(peer)
                    .or_else(|| self.base_peers.as_ref().and_then(|base| base.get(peer)));
                effective.is_some_and(|config| config.endpoint.is_none())
            })
            .collect();
        let mut healed = 0usize;
        for peer in targets {
            if self
                .resolve_standby_prewarm_via_rendezvous(state, peer)
                .await?
            {
                healed += 1;
            }
        }
        if healed == 0 {
            return Ok(());
        }
        self.sync_prewarm(state).await
    }

    /// Route one delivered message. `via` is the transport-authenticated
    /// DELIVERING member — with relaying it need not be the message's owner,
    /// so every handler authenticates the content signature and binds
    /// protocol state to the identity INSIDE the message; `via` only gates
    /// membership and takes the blame for undecodable/unverifiable junk.
    async fn deliver(
        &mut self,
        epoch: Option<&mut EpochState>,
        from: ed25519::PublicKey,
        bytes: Vec<u8>,
    ) -> Result<(), ReachabilityError> {
        let via = binding::identity_of(&from);
        // no active epoch (pre-boot traffic) — nothing to bind it to. This
        // is the INBOUND half of the black hole `no_epoch_target` warns
        // about: past the boot race, every one of these is a message a
        // targeted plane would have used.
        let Some(state) = epoch else {
            tracing::debug!(
                target: "ducktape::reachability",
                peer = %short(via), bytes = bytes.len(),
                "inbound dropped: this plane has no epoch"
            );
            return Ok(());
        };
        // membership gate on the DELIVERING identity: plane participants
        // (members + standbys), plus the configured gossip ingress — the
        // lobby key a parked standby connects under. Content signatures do
        // the real authentication either way.
        let participant = state.pk_of.contains_key(&via);
        let ingress = self.config.gossip_ingress.as_ref() == Some(&from);
        if !participant && !ingress {
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
        // a standby consumes gossip only: member records and adverts feed
        // its pre-warm tunnels; handshake traffic (fanned blindly by members
        // — senders cannot know which links exist) is not for it and carries
        // no relay duty.
        match (state.role, msg) {
            (Role::Standby, ReachabilityMsg::Record(record)) => {
                self.on_member_record(state, via, record).await
            }
            (Role::Standby, ReachabilityMsg::Advert(advert)) => {
                self.on_member_advert(state, via, advert).await
            }
            (
                Role::Standby,
                ReachabilityMsg::Request(_)
                | ReachabilityMsg::Response(_)
                | ReachabilityMsg::Ack(_),
            ) => Ok(()),
            (Role::Member, ReachabilityMsg::Record(record)) => {
                self.on_record(state, via, from, record).await
            }
            (Role::Member, ReachabilityMsg::Advert(advert)) => {
                self.on_advert(state, via, advert).await
            }
            (Role::Member, ReachabilityMsg::Request(request)) => {
                self.route_request(state, via, request).await
            }
            (Role::Member, ReachabilityMsg::Response(response)) => {
                self.route_response(state, via, response).await
            }
            (Role::Member, ReachabilityMsg::Ack(ack)) => self.route_ack(state, via, ack).await,
        }
    }

    async fn route_request(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        request: TunnelUpgradeRequest,
    ) -> Result<(), ReachabilityError> {
        let bearing = bearing(
            self.me,
            request.fields.initiator_identity,
            request.fields.responder_identity,
        );
        match bearing {
            Bearing::Addressee => self.on_request(state, via, request).await,
            // our own message relayed back around — nothing to do.
            Bearing::Author => Ok(()),
            Bearing::Bystander => {
                self.relay(state, via, RelayedHandshake::request(request))
                    .await
            }
        }
    }

    async fn route_response(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        response: TunnelUpgradeResponse,
    ) -> Result<(), ReachabilityError> {
        let bearing = bearing(
            self.me,
            response.fields.responder_identity,
            response.fields.initiator_identity,
        );
        match bearing {
            Bearing::Addressee => self.on_response(state, via, response).await,
            Bearing::Author => Ok(()),
            Bearing::Bystander => {
                self.relay(state, via, RelayedHandshake::response(response))
                    .await
            }
        }
    }

    async fn route_ack(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        ack: TunnelUpgradeAck,
    ) -> Result<(), ReachabilityError> {
        let bearing = bearing(
            self.me,
            ack.fields.initiator_identity,
            ack.fields.responder_identity,
        );
        match bearing {
            Bearing::Addressee => self.on_ack(state, via, ack).await,
            Bearing::Author => Ok(()),
            Bearing::Bystander => self.relay(state, via, RelayedHandshake::ack(ack)).await,
        }
    }

    /// Carry a handshake message between two OTHER members: verify, slot by
    /// `(initiator, responder)` with stage supersession, and fan out to every
    /// peer except the delivering one and the message's signer — this node
    /// cannot know which peer holds the working link to the addressee.
    async fn relay(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        relayed: RelayedHandshake,
    ) -> Result<(), ReachabilityError> {
        if !relayed.verified() {
            return self
                .fail_peer(state, via, "relayed an unverifiable handshake message")
                .await;
        }
        match state.slot_relay(&relayed, self.view) {
            RelayVerdict::NonMemberPair => {
                self.fail_peer(state, via, "relayed a handshake for a non-member pair")
                    .await
            }
            RelayVerdict::Drop => Ok(()),
            RelayVerdict::Carry => {
                let targets: Vec<ValidatorIdentity> = state
                    .peers
                    .iter()
                    .copied()
                    .filter(|peer| *peer != via && *peer != relayed.signer)
                    .collect();
                for peer in targets {
                    self.send_msg(state, peer, &relayed.msg).await?;
                }
                Ok(())
            }
        }
    }

    // ----- member role: phase-A gossip -------------------------------------

    async fn on_record(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        from: ed25519::PublicKey,
        signed: SignedEndpointRecord,
    ) -> Result<(), ReachabilityError> {
        let owner = signed.record.validator_identity;
        // content authentication: the record may have been relayed, so the
        // delivering link proves nothing about the record's owner.
        if signed.verify().is_err() {
            return self.fail_peer(state, via, "record signature invalid").await;
        }
        if state.standbys.contains(&owner) {
            return self.on_standby_record(state, via, from, signed).await;
        }
        if !state.set.contains(owner) {
            return self
                .fail_peer(state, via, "record from an unknown identity")
                .await;
        }
        // cutover skew, not a violation: nodes cross epoch boundaries at
        // slightly different times (a just-activated standby above all), so
        // gossip signed against the neighboring epoch is routine — its owner
        // re-signs once it observes the boundary.
        let cutover_skew = signed.record.epoch != state.epoch;
        if cutover_skew {
            tracing::debug!(
                target: "ducktape::reachability",
                peer = %short(owner), record_epoch = signed.record.epoch, epoch = state.epoch,
                "record dropped: cutover skew"
            );
            return Ok(());
        }
        // our own record echoed back around the relay ring.
        if owner == self.me {
            return Ok(());
        }
        // phase A: the set locks at version time — later (higher-nonce)
        // re-advertisements retunnel at the next cutover.
        let record_set_open = matches!(state.phase, Phase::Records);
        if !record_set_open {
            // this peer is behind us in phase A, which means it never got
            // our record: answer it on the next nudge rather than going deaf.
            state.request_heal(owner, self.nudges);
            tracing::debug!(
                target: "ducktape::reachability",
                peer = %short(owner), epoch = state.epoch,
                "record dropped: phase A already closed — healing this peer"
            );
            return Ok(());
        }
        let admission = state.admit_record(owner, signed.clone());
        tracing::debug!(
            target: "ducktape::reachability",
            peer = %short(owner), epoch = state.epoch,
            accepted = admission.accepted(),
            first_contact = admission == Admission::FirstContact,
            have = state.records.len(), want = state.set.validators().len(),
            "record in"
        );
        if admission == Admission::FirstContact {
            // heal join-order: the member that just appeared may have missed
            // our initial fan-out.
            let own = ReachabilityMsg::Record(state.own_record.clone());
            self.send_msg(state, owner, &own).await?;
        }
        if admission.accepted() {
            self.observe_control_endpoint(owner, signed.record.control_endpoint)
                .await?;
            // relay the news: peers with no link to the owner only ever see
            // its record through us — the standbys included, whose pre-warm
            // tunnels want every member's record. Accept-gated, so the
            // flood terminates.
            let record = ReachabilityMsg::Record(signed);
            for peer in state.flood_targets(owner, via) {
                self.send_msg(state, peer, &record).await?;
            }
        }
        self.advance(state).await
    }

    async fn on_advert(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        advert: EndpointAdvertisement,
    ) -> Result<(), ReachabilityError> {
        let owner = advert.record.validator_identity;
        if advert.verify_signature().is_err() {
            return self.fail_peer(state, via, "advert signature invalid").await;
        }
        if !state.set.contains(owner) {
            return self
                .fail_peer(state, via, "advert from an unknown identity")
                .await;
        }
        // cutover skew — same tolerance as records.
        let cutover_skew = advert.record.epoch != state.epoch;
        if cutover_skew {
            tracing::debug!(
                target: "ducktape::reachability",
                peer = %short(owner), advert_epoch = advert.record.epoch, epoch = state.epoch,
                "advert dropped: cutover skew"
            );
            return Ok(());
        }
        if owner == self.me {
            return Ok(());
        }
        // the advert set locks at verification; a failed epoch takes no
        // more (its retry is the next cutover).
        let advert_set_open = matches!(state.phase, Phase::Records | Phase::Adverts);
        if !advert_set_open {
            // decided views do not change, but a peer still advertising has
            // not assembled one — it is missing our advert. Send it back.
            let view_decided = state.view().is_some();
            if view_decided {
                state.request_heal(owner, self.nudges);
            }
            tracing::debug!(
                target: "ducktape::reachability",
                peer = %short(owner), epoch = state.epoch,
                "advert dropped: this mesh view is already verified — healing this peer"
            );
            return Ok(());
        }
        let admission = state.admit_advert(owner, advert.clone());
        tracing::debug!(
            target: "ducktape::reachability",
            peer = %short(owner), epoch = state.epoch,
            accepted = admission.accepted(),
            first_contact = admission == Admission::FirstContact,
            have = state.adverts.len(), want = state.set.validators().len(),
            "advert in"
        );
        // heal join-order for an advert we signed before this member
        // appeared (own advert exists from the advert phase on).
        if admission == Admission::FirstContact
            && let Some(own) = state.own_advert().cloned()
        {
            self.send_msg(state, owner, &ReachabilityMsg::Advert(own))
                .await?;
        }
        if admission.accepted() {
            self.observe_control_endpoint(owner, advert.record.control_endpoint)
                .await?;
            // standbys ride the advert flood too: the signed advert set is
            // what they persist for their promotion reboot's restore.
            let advert = ReachabilityMsg::Advert(advert);
            for peer in state.flood_targets(owner, via) {
                self.send_msg(state, peer, &advert).await?;
            }
        }
        self.advance(state).await
    }

    // ----- the pre-warm layer ------------------------------------------------

    /// A standby's owner-signed record (member role): validate, resolve its
    /// endpoint, and merge the tunnel onto the live interface — the pre-warm
    /// layer's whole trick. A higher nonce supersedes in place (the live
    /// re-advertisement rule); duplicates drop silently because every member
    /// re-offers standby records on nudge.
    async fn on_standby_record(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        from: ed25519::PublicKey,
        signed: SignedEndpointRecord,
    ) -> Result<(), ReachabilityError> {
        let owner = signed.record.validator_identity;
        // the record must bind to THIS epoch's member set — the same tuple
        // its owner derives from the boundary it synced. Another chain's
        // record is a violation; a neighboring epoch's is cutover skew (the
        // standby re-signs once its manifest poll crosses the boundary).
        if signed.record.namespace != state.set.namespace {
            return self
                .fail_peer(state, via, "standby record from another chain")
                .await;
        }
        let cutover_skew = signed.record.epoch != state.set.epoch
            || signed.record.valset_root != state.set.valset_root
            || signed.record.admission_root != state.set.admission_root;
        if cutover_skew {
            return Ok(());
        }
        if let Err(err) = signed.record.check(&self.config.port_policy) {
            return self
                .fail_peer(state, via, &format!("standby record refused: {err:?}"))
                .await;
        }
        let admission = state.admit_prewarm_nonce(owner, signed.record.nonce);
        if !admission.accepted() {
            return Ok(());
        }
        state.standby_records.insert(owner, signed.clone());
        state.learn_route(owner, via, from);
        self.observe_control_endpoint(owner, signed.record.control_endpoint)
            .await?;
        // the accepted record reaches disk NOW, not at the epoch apply: a
        // solo member never mints plans, and a reboot between accept and the
        // next apply would otherwise strand this standby for good (it cannot
        // re-introduce itself — see the restore).
        self.persist_mesh(state).await?;
        // endpoint-less standby: install without an endpoint — it initiates.
        let endpoint = match signed.record.wireguard_endpoint.map(|e| e.socket_addr()) {
            None => None,
            Some(advertised) => Some(
                self.resolve_prewarm_endpoint(state, owner, advertised)
                    .await?,
            ),
        };
        let allowed_ips = self.overlay.identity_allowed_ips(owner);
        state.prewarm_peers.insert(
            owner,
            PeerTunnelConfig {
                wireguard_public_key: signed.record.wireguard_public_key,
                endpoint,
                allowed_ips,
                keepalive_seconds: Some(KEEPALIVE_SECONDS),
            },
        );
        self.sync_prewarm(state).await?;
        if admission == Admission::FirstContact {
            // the standby just appeared: hand it our own gossip directly —
            // the nudge re-offers cover everything else it is missing.
            let own_record = ReachabilityMsg::Record(state.own_record.clone());
            self.send_msg(state, owner, &own_record).await?;
            if let Some(advert) = state.own_advert().cloned() {
                self.send_msg(state, owner, &ReachabilityMsg::Advert(advert))
                    .await?;
            }
        }
        // relay the accepted record onward — members with no link to the
        // standby, and the other standbys, see it through us. Accept-gated,
        // so the flood terminates.
        let record = ReachabilityMsg::Record(signed);
        for peer in state.flood_targets(owner, via) {
            self.send_msg(state, peer, &record).await?;
        }
        Ok(())
    }

    /// A member's record, received in the STANDBY role: validate and merge
    /// the member's tunnel — the standby side of the pre-warm layer. Another
    /// standby's record (members re-fan those to everyone) is silently not
    /// for us; standby<->standby tunnels assemble after activation like any
    /// member pair.
    async fn on_member_record(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        signed: SignedEndpointRecord,
    ) -> Result<(), ReachabilityError> {
        let owner = signed.record.validator_identity;
        if signed.verify().is_err() {
            return self.fail_peer(state, via, "record signature invalid").await;
        }
        let not_for_us = owner == self.me || state.standbys.contains(&owner);
        if not_for_us {
            return Ok(());
        }
        if !state.set.contains(owner) {
            return self
                .fail_peer(state, via, "record identity/epoch mismatch")
                .await;
        }
        self.merge_member_prewarm(state, &signed.record).await
    }

    /// A member's advertisement, received in the STANDBY role: the richer
    /// form of its record — accepted for the SAME pre-warm merge, and
    /// persisted, because the signed advert set is what the promotion
    /// reboot's cold-restart restore reads back from disk.
    async fn on_member_advert(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        advert: EndpointAdvertisement,
    ) -> Result<(), ReachabilityError> {
        let owner = advert.record.validator_identity;
        if advert.verify_signature().is_err() {
            return self.fail_peer(state, via, "advert signature invalid").await;
        }
        let not_for_us = owner == self.me || state.standbys.contains(&owner);
        if not_for_us {
            return Ok(());
        }
        if !state.set.contains(owner) {
            return self
                .fail_peer(state, via, "advert identity/epoch mismatch")
                .await;
        }
        let record = advert.record.clone();
        let admission = state.admit_advert(owner, advert);
        if admission.accepted() {
            self.persist_mesh(state).await?;
        }
        self.merge_member_prewarm(state, &record).await
    }

    /// The standby side's shared merge: bind a member record to the epoch
    /// tuple, dedup by nonce, resolve, and re-apply the interface.
    async fn merge_member_prewarm(
        &mut self,
        state: &mut EpochState,
        record: &EndpointRecord,
    ) -> Result<(), ReachabilityError> {
        let owner = record.validator_identity;
        if record.namespace != state.set.namespace {
            return self
                .fail_peer(state, owner, "member record from another chain")
                .await;
        }
        // cutover skew: this standby's manifest poll and the members'
        // boundary crossing are not synchronized.
        let cutover_skew = record.epoch != state.set.epoch
            || record.valset_root != state.set.valset_root
            || record.admission_root != state.set.admission_root;
        if cutover_skew {
            return Ok(());
        }
        if let Err(err) = record.check(&self.config.port_policy) {
            return self
                .fail_peer(state, owner, &format!("member record refused: {err:?}"))
                .await;
        }
        let admission = state.admit_prewarm_nonce(owner, record.nonce);
        if !admission.accepted() {
            return Ok(());
        }
        self.observe_control_endpoint(owner, record.control_endpoint)
            .await?;
        // endpoint-less member record: install without an endpoint — it initiates.
        let endpoint = match record.wireguard_endpoint.map(|e| e.socket_addr()) {
            None => None,
            Some(advertised) => Some(
                self.resolve_prewarm_endpoint(state, owner, advertised)
                    .await?,
            ),
        };
        let allowed_ips = self.overlay.identity_allowed_ips(owner);
        state.prewarm_peers.insert(
            owner,
            PeerTunnelConfig {
                wireguard_public_key: record.wireguard_public_key,
                endpoint,
                allowed_ips,
                keepalive_seconds: Some(KEEPALIVE_SECONDS),
            },
        );
        self.sync_prewarm(state).await
    }

    /// Push the interface's full desired configuration — the phase-A base
    /// (validated plans or the restored mesh) with the pre-warm peers merged
    /// over it — onto the effect. Live interfaces reconfigure in place; a
    /// standby with nothing applied yet brings its interface up here (its
    /// interface exists purely for pre-warm). A member whose epoch is still
    /// assembling holds off — its one epoch apply merges the pre-warm set.
    async fn sync_prewarm(&mut self, state: &EpochState) -> Result<(), ReachabilityError> {
        if state.prewarm_peers.is_empty() {
            return Ok(());
        }
        let mut merged = match (&self.base_peers, state.role) {
            (Some(base), _) => base.clone(),
            (None, Role::Standby) => BTreeMap::new(),
            (None, Role::Member) => return Ok(()),
        };
        merged.extend(state.prewarm_peers.clone());
        let peers = self.assemble_peers(merged);
        match self.push_interface(&peers) {
            Ok(()) => {
                self.emit(ReachabilityEvent::StandbyTunnelsApplied {
                    epoch: state.epoch,
                    interface: self.interface.clone(),
                    peers: state.prewarm_peers.len(),
                })
                .await
            }
            Err(err) => {
                // the interface keeps whatever configuration it had; the
                // next accepted record (or nudge-driven re-offer) retries.
                self.emit(ReachabilityEvent::EpochFailed {
                    epoch: state.epoch,
                    reason: format!("pre-warm tunnel apply: {err:?}"),
                })
                .await
            }
        }
    }

    /// Resolve a pre-warm counterparty's dialable endpoint, with the same
    /// contract as the restore path: a resolver failure surfaces as
    /// `PeerFailed` and the peer rides its advertised endpoint.
    async fn resolve_prewarm_endpoint(
        &mut self,
        state: &EpochState,
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
                self.fail_peer(
                    state,
                    peer,
                    &format!("pre-warm endpoint resolution: {reason}"),
                )
                .await?;
                Ok(advertised)
            }
        }
    }

    /// The standby twin of `resolve_peer_via_rendezvous_fallback`: same
    /// coordinator gate, same shared per-epoch budget — but the source of
    /// truth and the write target are the pre-warm layer, not the phase-A
    /// view/overrides a standby never assembles. The resolved address lands
    /// as a pre-warm entry cloned from the effective config (the WireGuard
    /// key and allowed-ips carry over); the sweep batches one `sync_prewarm`
    /// for all of them. Returns whether an endpoint was written.
    async fn resolve_standby_prewarm_via_rendezvous(
        &mut self,
        state: &mut EpochState,
        peer: ValidatorIdentity,
    ) -> Result<bool, ReachabilityError> {
        if self.config.coordinators.is_empty() {
            return Ok(false);
        }
        let effective = state
            .prewarm_peers
            .get(&peer)
            .or_else(|| self.base_peers.as_ref().and_then(|base| base.get(&peer)))
            .cloned();
        let Some(mut config) = effective else {
            return Ok(false);
        };
        let already_dialable = config.endpoint.is_some();
        if already_dialable {
            return Ok(false);
        }
        let Some(addr) = self.attempt_rendezvous_by_identity(state, peer).await? else {
            return Ok(false);
        };
        config.endpoint = Some(addr);
        state.prewarm_peers.insert(peer, config);
        Ok(true)
    }

    /// The by-identity rendezvous attempt both role sweeps share: burn one
    /// unit of the per-epoch budget, resolve through the coordinator,
    /// surface a failed resolve as `PeerFailed`. `None` means no address
    /// this round — the budget refused the attempt, or the resolve failed
    /// (already reported); both non-fatal, a later `Nudge` retries.
    async fn attempt_rendezvous_by_identity(
        &mut self,
        state: &mut EpochState,
        peer: ValidatorIdentity,
    ) -> Result<Option<SocketAddr>, ReachabilityError> {
        if !state.claim_rendezvous_attempt(peer, Instant::now()) {
            return Ok(None);
        }
        match self
            .resolver
            .resolve_rendezvous_endpoint(binding::node_key(peer))
            .await
        {
            Ok(addr) => Ok(Some(addr)),
            Err(reason) => {
                self.fail_peer(state, peer, &format!("rendezvous fallback: {reason}"))
                    .await?;
                Ok(None)
            }
        }
    }

    // ----- the join-window invite layer -------------------------------------

    /// Install a join-window tunnel peer (node-authenticated; see the
    /// command doc) and re-apply the interface — the invite layer's own
    /// `sync_prewarm` analogue, usable BEFORE any epoch exists.
    async fn install_invite_peer(
        &mut self,
        epoch: Option<&EpochState>,
        peer: ed25519::PublicKey,
        wireguard_public_key: wireguard::X25519PublicKey,
        endpoint: SocketAddr,
        reply: InstallReply,
    ) -> Result<(), ReachabilityError> {
        let outcome = self.install_invite_tunnel(epoch, &peer, wireguard_public_key, endpoint);
        let _ = reply.0.send(outcome.clone());
        // on failure the interface keeps whatever configuration it had; the
        // caller decides whether to retry.
        if outcome.is_err() {
            return Ok(());
        }
        self.emit(ReachabilityEvent::InvitePeerInstalled {
            peer,
            interface: self.interface.clone(),
        })
        .await
    }

    /// Merge one invite peer onto the interface. The error is the caller's
    /// reply text: an apply refusal, or a tunnel to this node itself.
    fn install_invite_tunnel(
        &mut self,
        epoch: Option<&EpochState>,
        peer: &ed25519::PublicKey,
        wireguard_public_key: wireguard::X25519PublicKey,
        endpoint: SocketAddr,
    ) -> Result<(), String> {
        let identity = binding::identity_of(peer);
        if identity == self.me {
            return Err("refusing an invite tunnel to self".into());
        }
        let allowed_ips = self.overlay.identity_allowed_ips(identity);
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
        if let Some(state) = epoch {
            merged.extend(state.prewarm_peers.clone());
        }
        let peers = self.assemble_peers(merged);
        self.push_interface(&peers)
            .map_err(|err| format!("{err:?}"))
    }

    /// Coordinated invite bootstrap: rendezvous the inviter's WireGuard
    /// underlay endpoint, install it as the local join-window peer, and send
    /// the authenticated intro over that same punched socket so the inviter
    /// can install this node in return.
    async fn bootstrap_coordinated_invite_peer(
        &mut self,
        epoch: Option<&EpochState>,
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
        if let Err(reason) =
            self.install_invite_tunnel(epoch, &peer, wireguard_public_key, endpoint)
        {
            let _ = reply.0.send(Err(reason));
            return Ok(());
        }
        self.emit(ReachabilityEvent::InvitePeerInstalled {
            peer,
            interface: self.interface.clone(),
        })
        .await?;
        let intro_ack = self
            .resolver
            .send_datagram_and_recv(endpoint, intro, Duration::from_secs(2))
            .await
            .map_err(|reason| format!("coordinated invite intro ack: {reason}"));
        let _ = reply.0.send(intro_ack);
        Ok(())
    }

    // ----- the epoch machine: decide, then execute -------------------------

    /// Take every step the accumulated state now satisfies: the decision is
    /// [`EpochState::next_step`]'s, re-taken after each executed step until
    /// the phase is gathering again (or terminal). Idempotent; called after
    /// every state change.
    async fn advance(&mut self, state: &mut EpochState) -> Result<(), ReachabilityError> {
        while let Some(step) = state.next_step() {
            match step {
                Step::SignAdvert => self.sign_advert(state).await?,
                Step::VerifyMesh => self.verify_mesh(state).await?,
                Step::Apply => self.apply_epoch(state).await?,
            }
        }
        Ok(())
    }

    /// Records -> Adverts: compute the mesh version over the locked record
    /// set, sign our advertisement over it, and fan it out.
    async fn sign_advert(&mut self, state: &mut EpochState) -> Result<(), ReachabilityError> {
        let records: Vec<EndpointRecord> = state.known_records().into_values().collect();
        let version = compute_mesh_version(&records)?;
        let advert = EndpointAdvertisement::sign(
            state.own_record.record.clone(),
            version,
            &self.config.signer,
        );
        state.adverts.insert(self.me, advert.clone());
        state.phase = Phase::Adverts;
        tracing::debug!(
            target: "ducktape::reachability",
            epoch = state.epoch, peers = state.peers.len(),
            "phase A complete: fanning out our advert"
        );
        self.fan_msg(state, &ReachabilityMsg::Advert(advert)).await
    }

    /// Adverts -> Handshakes: verify every advert into one mesh view, then
    /// start the handshakes this node initiates and answer the requests
    /// that arrived from peers whose mesh completed before ours.
    async fn verify_mesh(&mut self, state: &mut EpochState) -> Result<(), ReachabilityError> {
        let epoch = state.epoch;
        let ads: Vec<EndpointAdvertisement> = state.adverts.values().cloned().collect();
        let view = match MeshView::verify(state.set.clone(), ads, &self.config.port_policy) {
            Ok(view) => view,
            Err(err) => {
                state.phase = Phase::Failed;
                return self
                    .emit(ReachabilityEvent::EpochFailed {
                        epoch,
                        reason: format!("mesh verification: {err:?}"),
                    })
                    .await;
            }
        };
        let version = view.mesh_version;
        state.phase = Phase::Handshakes { view };
        self.emit(ReachabilityEvent::MeshReady { epoch, version })
            .await?;
        self.start_handshakes(state).await?;
        let pending = std::mem::take(&mut state.pending_requests);
        for (sender, request) in pending {
            self.on_request(state, sender, request).await?;
        }
        Ok(())
    }

    /// Handshakes -> Applied: the epoch's ONE interface apply. The validated
    /// plans become the interface's new BASE; the pre-warm peers merge over
    /// it (same identity: the fresher pre-warm entry wins), so a standby
    /// tunnel that assembled during the epoch's bring-up survives the
    /// replace. The apply runs even with no peers at all: a single-member
    /// network (every fresh desktop workspace) and an all-peers-failed
    /// epoch still need the interface up — the node's own /128 is what the
    /// per-use media planes bind, so a peer-less interface is the
    /// difference between a working solo huddle and a join that hangs in
    /// "connecting" forever.
    async fn apply_epoch(&mut self, state: &mut EpochState) -> Result<(), ReachabilityError> {
        let view = state
            .view()
            .cloned()
            .expect("the apply step is decided only over a verified view");
        let epoch = state.epoch;
        let plans: Vec<TunnelInstallPlan> = state.plans.values().cloned().collect();
        let base: BTreeMap<ValidatorIdentity, PeerTunnelConfig> = plans
            .iter()
            .map(TunnelInstallPlan::peer_identity)
            .zip(plan_peer_configs(&plans, &state.overrides))
            .collect();
        let mut merged = base.clone();
        merged.extend(state.prewarm_peers.clone());
        let peers = self.assemble_peers(merged);
        // the epoch's mesh is a full interface REBUILD over the new base,
        // never a reconfigure of the previous epoch's.
        self.teardown_interface();
        self.base_peers = None;
        if let Err(err) = self.push_interface(&peers) {
            state.phase = Phase::Failed;
            return self
                .emit(ReachabilityEvent::EpochFailed {
                    epoch,
                    reason: format!("wireguard effect: {err:?}"),
                })
                .await;
        }
        self.base_peers = Some(base);
        state.phase = Phase::Applied { view };
        // the epoch's mesh is now REAL — remember it for the cold-restart
        // re-apply. Only with member plans: an all-peers-failed epoch must
        // not clobber the last mesh that actually carried member tunnels.
        // (The accepted standby records ride every snapshot regardless —
        // their own persist trigger is the accept itself.)
        if !plans.is_empty() {
            self.persist_mesh(state).await?;
        }
        self.emit(ReachabilityEvent::TunnelsApplied {
            epoch,
            interface: self.interface.clone(),
            peers: plans.len(),
        })
        .await?;
        let prewarm_count = state.prewarm_peers.len();
        if prewarm_count > 0 {
            self.emit(ReachabilityEvent::StandbyTunnelsApplied {
                epoch,
                interface: self.interface.clone(),
                peers: prewarm_count,
            })
            .await?;
        }
        Ok(())
    }

    // ----- the handshake triple ----------------------------------------------

    /// Initiator side: resolve each lower-identity-initiates peer and send
    /// the signed request.
    async fn start_handshakes(&mut self, state: &mut EpochState) -> Result<(), ReachabilityError> {
        let targets: Vec<ValidatorIdentity> = state
            .peers
            .iter()
            .copied()
            .filter(|peer| initiates(self.me, *peer))
            .collect();
        for peer in targets {
            self.resolve_peer(state, peer).await?;
            let nonce = state.next_nonce();
            let view = state.handshake_view();
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
            self.fan_msg(state, &ReachabilityMsg::Request(request))
                .await?;
        }
        Ok(())
    }

    /// Run the endpoint resolver for `peer`, recording a punched override
    /// or a `PeerFailed` observability event (the peer then rides its
    /// advertised endpoint).
    async fn resolve_peer(
        &mut self,
        state: &mut EpochState,
        peer: ValidatorIdentity,
    ) -> Result<(), ReachabilityError> {
        let advertised = state
            .view()
            .and_then(|view| view.record(peer))
            .and_then(|record| record.wireguard_endpoint)
            .map(|endpoint| endpoint.socket_addr());
        // no record, or an endpoint-less peer: nothing to resolve AGAINST —
        // but a configured coordinator can still rendezvous by identity; the
        // base "the peer initiates and WireGuard roams to it" contract
        // stands when there is no coordinator to ask.
        let Some(advertised) = advertised else {
            return self.resolve_peer_via_rendezvous_fallback(state, peer).await;
        };
        match self
            .resolver
            .resolve(binding::node_key(peer), advertised)
            .await
        {
            Ok(Resolution::Advertised) => Ok(()),
            Ok(Resolution::Punched(addr)) => {
                state.overrides.insert(peer, addr);
                Ok(())
            }
            Err(reason) => {
                self.fail_peer(state, peer, &format!("endpoint resolution: {reason}"))
                    .await
            }
        }
    }

    /// The endpoint-less fallback: a member↔member pair that both advertise
    /// no endpoint (the default for every invite-joined node) can never
    /// initiate a WireGuard handshake — WITH a coordinator configured,
    /// rendezvous the peer by identity instead (the same by-identity
    /// resolution the invite bootstrap uses). No coordinator configured ⇒
    /// install endpoint-less and wait for the peer's own initiation. A
    /// failed resolve stays terminal for THIS attempt — no relay — but a
    /// per-peer backoff lets a later `Nudge` retry once the peer has had
    /// time to register, up to a bounded per-epoch budget.
    async fn resolve_peer_via_rendezvous_fallback(
        &mut self,
        state: &mut EpochState,
        peer: ValidatorIdentity,
    ) -> Result<(), ReachabilityError> {
        if self.config.coordinators.is_empty() {
            return Ok(());
        }
        let already_resolved = state.overrides.contains_key(&peer);
        if already_resolved {
            return Ok(());
        }
        let Some(addr) = self.attempt_rendezvous_by_identity(state, peer).await? else {
            return Ok(());
        };
        state.overrides.insert(peer, addr);
        Ok(())
    }

    /// Responder side: answer a request with our signed response. A
    /// duplicate of the request we already answered (the initiator nudging —
    /// our single-shot response may be lost) re-sends the STORED response:
    /// re-signing would orphan the initiator's eventual ack, which pins ONE
    /// response by hash. `via` is the delivering member (possibly a relay);
    /// the counterparty is the request's SIGNED initiator.
    async fn on_request(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        request: TunnelUpgradeRequest,
    ) -> Result<(), ReachabilityError> {
        let sender = request.fields.initiator_identity;
        if request.verify_signature().is_err() {
            return self
                .fail_peer(state, via, "request signature invalid")
                .await;
        }
        if !state.set.contains(sender) {
            return self
                .fail_peer(state, via, "request from a non-member initiator")
                .await;
        }
        // the pair already failed this epoch — its nonces are burnt in the
        // replay cache, so no retry can revive it; stay quiet.
        if state.failed.contains(&sender) {
            return Ok(());
        }
        let wrong_side = request.fields.epoch != state.epoch || !initiates(sender, self.me);
        if wrong_side {
            return self
                .fail_peer(state, sender, "request from the wrong side")
                .await;
        }
        match state.handshakes.get(&sender) {
            Some(PeerHandshake::AwaitingAck {
                request: stored,
                response,
            }) if stored.hash() == request.hash() => {
                let response = response.clone();
                return self
                    .fan_msg(state, &ReachabilityMsg::Response(response))
                    .await;
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
                    .fail_peer(state, sender, "conflicting handshake request")
                    .await;
            }
            None => {}
        }
        // the peer's mesh completed before ours; answer once ours does.
        let mesh_verified = state.view().is_some();
        if !mesh_verified {
            state.pending_requests.insert(sender, request);
            return Ok(());
        }
        self.resolve_peer(state, sender).await?;
        let nonce = state.next_nonce();
        let view = state.handshake_view();
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
        self.fan_msg(state, &ReachabilityMsg::Response(response))
            .await
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
        state: &mut EpochState,
        via: ValidatorIdentity,
        response: TunnelUpgradeResponse,
    ) -> Result<(), ReachabilityError> {
        let sender = response.fields.responder_identity;
        if response.verify_signature().is_err() {
            return self
                .fail_peer(state, via, "response signature invalid")
                .await;
        }
        if !state.set.contains(sender) {
            return self
                .fail_peer(state, via, "response from a non-member responder")
                .await;
        }
        // failed pairs stay failed for the epoch — see `on_request`.
        if state.failed.contains(&sender) {
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
                return self.fan_msg(state, &ReachabilityMsg::Ack(ack)).await;
            }
            _ => {
                return self
                    .fail_peer(state, sender, "unsolicited handshake response")
                    .await;
            }
        };
        if response.fields.request_hash != request.hash() {
            return self
                .fail_peer(state, sender, "response does not match our request")
                .await;
        }
        let view = state.handshake_view().clone();
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
        // an invalid triple must not be acked into the peer's replay state —
        // fail loud and let the peer's own validation refuse it too.
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
                self.fan_msg(state, &ReachabilityMsg::Ack(ack)).await?;
                self.advance(state).await
            }
            Err(err) => {
                self.settle_failed_handshake(state, sender, err).await?;
                self.advance(state).await
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
        state: &mut EpochState,
        via: ValidatorIdentity,
        ack: TunnelUpgradeAck,
    ) -> Result<(), ReachabilityError> {
        let sender = ack.fields.initiator_identity;
        if ack.verify_signature().is_err() {
            return self.fail_peer(state, via, "ack signature invalid").await;
        }
        if !state.set.contains(sender) {
            return self
                .fail_peer(state, via, "ack from a non-member initiator")
                .await;
        }
        // failed pairs stay failed for the epoch — see `on_request`.
        if state.failed.contains(&sender) {
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
                return self
                    .fail_peer(state, sender, "unsolicited handshake ack")
                    .await;
            }
        };
        let pinned_triple = ack.fields.request_hash == request.hash()
            && ack.fields.response_hash == response.hash();
        if !pinned_triple {
            return self
                .fail_peer(state, sender, "ack does not match the handshake")
                .await;
        }
        let view = state.handshake_view().clone();
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
                self.advance(state).await
            }
            Err(err) => {
                self.settle_failed_handshake(state, sender, err).await?;
                self.advance(state).await
            }
        }
    }

    /// A triple that failed validation settles the peer as failed for the
    /// epoch: its nonces are burnt in the replay cache, so no retry can
    /// revive the pair, and the apply gate counts it as done.
    async fn settle_failed_handshake(
        &mut self,
        state: &mut EpochState,
        peer: ValidatorIdentity,
        err: UpgradeError,
    ) -> Result<(), ReachabilityError> {
        state.handshakes.remove(&peer);
        state.failed.insert(peer);
        self.fail_peer(state, peer, &format!("handshake validation: {err:?}"))
            .await
    }
}

/// record `socket` as `owner`'s control endpoint in the ledger, answering
/// whether it CHANGED — the pure decision behind
/// [`ReachabilityEvent::ControlEndpointObserved`]'s only-on-change contract.
fn control_endpoint_changed(
    ledger: &mut BTreeMap<ValidatorIdentity, SocketAddr>,
    owner: ValidatorIdentity,
    socket: SocketAddr,
) -> bool {
    ledger.insert(owner, socket) != Some(socket)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_endpoint_ledger_fires_only_on_change() {
        let mut ledger = BTreeMap::new();
        let owner = ValidatorIdentity([7; 32]);
        let first: SocketAddr = "192.0.2.1:8846".parse().unwrap();
        let moved: SocketAddr = "192.0.2.2:8846".parse().unwrap();

        assert!(
            control_endpoint_changed(&mut ledger, owner, first),
            "first observation is a change"
        );
        assert!(
            !control_endpoint_changed(&mut ledger, owner, first),
            "re-gossip of the same endpoint is silent"
        );
        assert!(
            control_endpoint_changed(&mut ledger, owner, moved),
            "a moved endpoint fires"
        );
        assert!(
            control_endpoint_changed(&mut ledger, ValidatorIdentity([8; 32]), first),
            "identities are independent"
        );
    }

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

    #[test]
    fn a_handshake_message_bears_on_its_addressee_author_or_a_bystander() {
        let me = ValidatorIdentity([1; 32]);
        let other = ValidatorIdentity([2; 32]);
        let third = ValidatorIdentity([3; 32]);
        assert_eq!(bearing(me, other, me), Bearing::Addressee);
        assert_eq!(bearing(me, me, other), Bearing::Author);
        assert_eq!(bearing(me, other, third), Bearing::Bystander);
    }
}
