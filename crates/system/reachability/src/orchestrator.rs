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
//! Phase-A scope, deliberately: the record/advert set LOCKS once the epoch's
//! mesh version is computed — a mid-epoch re-advertisement (NAT rebind, key
//! rotation) retunnels at the NEXT cutover, not live. And a member that
//! never shows up stalls its epoch's bring-up exactly like
//! `MeshView::verify`'s all-members rule says it must; the previous epoch's
//! tunnels stay up meanwhile.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use commonware_cryptography::{Signer as _, ed25519};
use nat_traversal::{NatClient, NodeKey};
use tokio::sync::mpsc;
use wireguard_effect::{WireGuardEffect, apply_tunnel_plans};
use wireguard_upgrade::{
    ActiveValidatorSet, EndpointAdvertisement, EndpointRecord, Endpoint, MeshVersion, MeshView,
    OverlayPolicy, Perspective, PortPolicy, ReplayCache, TunnelInstallPlan, TunnelUpgradeAck,
    TunnelUpgradeAckFields, TunnelUpgradeRequest, TunnelUpgradeRequestFields,
    TunnelUpgradeResponse, TunnelUpgradeResponseFields, UpgradeError, ValidatorIdentity,
    compute_mesh_version,
};

use crate::binding;
use crate::keys::{KeyError, WireGuardKeypair};
use crate::msg::{MsgError, ReachabilityMsg};

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
    /// The node's own advertised WireGuard UDP endpoint.
    pub wireguard_listen: Endpoint,
    /// The node's own advertised control-mesh endpoint.
    pub control_endpoint: Endpoint,
    /// Rendezvous coordinators (from `Resolved.coordinated`), possibly none:
    /// with an empty list every peer resolves to its advertised endpoint.
    pub coordinators: Vec<SocketAddr>,
    /// The endpoint policy advertisements and handshakes validate against.
    pub port_policy: PortPolicy,
}

/// A valset cutover (or boot) the orchestrator must retarget to.
#[derive(Clone, Debug)]
pub struct MeshEpochEvent {
    pub epoch: u64,
    /// The epoch's consensus members' ed25519 public keys, this node
    /// included. Order is irrelevant — every derived commitment sorts.
    pub members: Vec<ed25519::PublicKey>,
    /// The consensus view at the cutover; the freshness clock for expiries.
    pub current_view: u64,
}

/// Node -> orchestrator.
#[derive(Debug)]
pub enum ReachabilityCommand {
    /// Boot or epoch cutover: (re)build the mesh for this member set. A
    /// retarget SUPERSEDES any epoch still assembling — tear down in-flight
    /// state and start over.
    Retarget(MeshEpochEvent),
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
}

/// How a peer's WireGuard endpoint was resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// Dial the advertised endpoint as-is (public or already-reachable
    /// address; also the no-coordinator dev path).
    Advertised,
    /// Hole-punch succeeded: dial the peer's punched reflexive.
    Punched(SocketAddr),
    /// Punch failed; a coordinator granted a relay session at this address.
    Relayed(SocketAddr),
}

/// Per-peer endpoint resolution, pluggable so the orchestrator's protocol
/// logic tests deterministically without UDP. The real implementation is
/// [`NatResolver`]; tests use [`StaticResolver`].
#[allow(async_fn_in_trait)] // consumed on a single-thread block_on root; no Send bound wanted
pub trait EndpointResolver {
    /// Resolve `peer`'s dialable UDP address given its advertised WireGuard
    /// endpoint. Errors mean the peer stays on its advertised endpoint and a
    /// `PeerFailed` is emitted for observability.
    async fn resolve(
        &mut self,
        peer: NodeKey,
        advertised: SocketAddr,
    ) -> Result<Resolution, String>;
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
}

/// How long each coordinator interaction (reflexive discovery, lookup,
/// relay grant) may take before the resolver moves on.
const COORD_STEP_TIMEOUT: Duration = Duration::from_secs(3);
/// One punch exchange attempt; retried [`PUNCH_TRIES`] times before the
/// relay fallback.
const PUNCH_STEP_TIMEOUT: Duration = Duration::from_secs(1);
const PUNCH_TRIES: usize = 3;

/// The production resolver: drives `nat_traversal::NatClient` against the
/// configured coordinators — reflexive discovery + `register` at bind, then
/// per peer `lookup` -> simultaneous-open punch -> `request_relay` fallback.
/// With NO coordinators configured every resolution is `Advertised`.
pub struct NatResolver {
    client: Option<NatClient>,
    reflexive: Option<SocketAddr>,
}

impl NatResolver {
    /// Bind the nat client's UDP socket, discover this node's reflexive
    /// (failing over across the coordinator hints), and register. `key` is
    /// this node's identity bytes (`binding::node_key`). An empty
    /// coordinator set yields the pass-through resolver.
    pub async fn bind(key: NodeKey, coordinators: Vec<SocketAddr>) -> std::io::Result<Self> {
        if coordinators.is_empty() {
            return Ok(Self {
                client: None,
                reflexive: None,
            });
        }
        let mut client = NatClient::bind_multi(key, coordinators).await?;
        let (_idx, reflexive) = client.discover_reflexive_failover(COORD_STEP_TIMEOUT).await?;
        client.register().await?;
        Ok(Self {
            client: Some(client),
            reflexive: Some(reflexive),
        })
    }

    /// The coordinator-observed reflexive address, when one was discovered —
    /// what a NATed node should advertise as its WireGuard endpoint.
    pub fn reflexive(&self) -> Option<SocketAddr> {
        self.reflexive
    }
}

impl EndpointResolver for NatResolver {
    async fn resolve(
        &mut self,
        peer: NodeKey,
        _advertised: SocketAddr,
    ) -> Result<Resolution, String> {
        let Some(client) = &self.client else {
            return Ok(Resolution::Advertised);
        };
        let peer_reflexive = tokio::time::timeout(COORD_STEP_TIMEOUT, client.lookup(peer))
            .await
            .map_err(|_| "coordinator lookup timed out".to_string())?
            .map_err(|e| format!("coordinator lookup: {e}"))?;
        // simultaneous open: both sides resolve around the same time (the
        // initiator's lookup fans a PunchSync to the passive side, and the
        // passive side runs its own lookup anyway), so a few send/recv
        // rounds absorb the timing skew.
        for _ in 0..PUNCH_TRIES {
            if let Err(e) = client.send_punch_to(peer_reflexive).await {
                return Err(format!("punch send: {e}"));
            }
            match tokio::time::timeout(PUNCH_STEP_TIMEOUT, client.recv_punch_from(peer_reflexive))
                .await
            {
                Ok(Ok(_)) => return Ok(Resolution::Punched(peer_reflexive)),
                Ok(Err(e)) => return Err(format!("punch recv: {e}")),
                Err(_) => continue,
            }
        }
        let (_session, relay) = tokio::time::timeout(COORD_STEP_TIMEOUT, client.request_relay(peer))
            .await
            .map_err(|_| "relay request timed out".to_string())?
            .map_err(|e| format!("relay request: {e}"))?;
        Ok(Resolution::Relayed(relay))
    }
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
#[allow(clippy::large_enum_variant)]
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

/// Everything one epoch accumulates on the way to its `apply` call.
struct EpochState {
    epoch: u64,
    set: ActiveValidatorSet,
    peers: Vec<ValidatorIdentity>,
    pk_of: HashMap<ValidatorIdentity, ed25519::PublicKey>,
    /// One strictly-monotonic counter for EVERYTHING this identity signs in
    /// the epoch — replay keys are `(identity, epoch, nonce)`, and the
    /// advert duplicate rule wants strictly-increasing nonces too.
    nonce: u64,
    records: BTreeMap<ValidatorIdentity, EndpointRecord>,
    adverts: BTreeMap<ValidatorIdentity, EndpointAdvertisement>,
    own_advert_sent: bool,
    view_state: Option<MeshView>,
    replay: ReplayCache,
    /// Requests that arrived before our own `MeshView` completed (the peer
    /// verified faster); drained the moment it does. Keyed by initiator so
    /// nudged re-offers of the same request collapse to one entry.
    pending_requests: BTreeMap<ValidatorIdentity, TunnelUpgradeRequest>,
    handshakes: HashMap<ValidatorIdentity, PeerHandshake>,
    plans: BTreeMap<ValidatorIdentity, TunnelInstallPlan>,
    overrides: BTreeMap<ValidatorIdentity, SocketAddr>,
    failed: HashSet<ValidatorIdentity>,
    applied: bool,
}

impl EpochState {
    fn next_nonce(&mut self) -> u64 {
        self.nonce += 1;
        self.nonce
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
    };
    while let Some(command) = commands.recv().await {
        match command {
            ReachabilityCommand::Retarget(event) => driver.retarget(event).await?,
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
        let Some(pk) = state.pk_of.get(&to) else {
            return Ok(());
        };
        self.emit(ReachabilityEvent::Send {
            to: pk.clone(),
            bytes: msg.encode(),
        })
        .await
    }

    async fn retarget(&mut self, event: MeshEpochEvent) -> Result<(), ReachabilityError> {
        self.view = self.view.max(event.current_view);
        let identities: Vec<ValidatorIdentity> =
            event.members.iter().map(binding::identity_of).collect();
        if !identities.contains(&self.me) {
            // demotion normally exits the node before this; be inert, not
            // wrong: drop epoch state and any live tunnel.
            if self.interface_live {
                let _ = self.effect.remove_interface();
                self.interface_live = false;
            }
            self.state = None;
            return self
                .emit(ReachabilityEvent::EpochFailed {
                    epoch: event.epoch,
                    reason: "this node is not a member of the epoch".into(),
                })
                .await;
        }
        let set = binding::active_set(&self.config.chain_id, event.epoch, identities.clone())?;
        let pk_of: HashMap<ValidatorIdentity, ed25519::PublicKey> = event
            .members
            .iter()
            .map(|pk| (binding::identity_of(pk), pk.clone()))
            .collect();
        let peers: Vec<ValidatorIdentity> = identities
            .iter()
            .copied()
            .filter(|id| *id != self.me)
            .collect();
        let mut state = EpochState {
            epoch: event.epoch,
            set,
            peers,
            pk_of,
            nonce: 0,
            records: BTreeMap::new(),
            adverts: BTreeMap::new(),
            own_advert_sent: false,
            view_state: None,
            replay: ReplayCache::default(),
            pending_requests: BTreeMap::new(),
            handshakes: HashMap::new(),
            plans: BTreeMap::new(),
            overrides: BTreeMap::new(),
            failed: HashSet::new(),
            applied: false,
        };
        let own = EndpointRecord {
            namespace: self.config.chain_id.clone(),
            epoch: state.epoch,
            valset_root: state.set.valset_root,
            admission_root: state.set.admission_root,
            validator_identity: self.me,
            wireguard_public_key: self.keypair.public_key(),
            control_endpoint: self.config.control_endpoint,
            wireguard_endpoint: self.config.wireguard_listen,
            capabilities: vec![],
            expires_at_view: self.view + ADVERT_TTL_VIEWS,
            nonce: state.next_nonce(),
        };
        state.records.insert(self.me, own.clone());
        let peers = state.peers.clone();
        self.state = Some(state);
        for peer in peers {
            self.send_msg(peer, &ReachabilityMsg::Record(own.clone()))
                .await?;
        }
        // a single-member network is a complete mesh already.
        self.advance().await
    }

    /// Re-offer whatever the current stage is still waiting on, always the
    /// STORED message, never re-signed: pre-version, a fresh record nonce
    /// would change the mesh version peers already computed; post-verify, a
    /// re-signed handshake message would desynchronize the hash-pinned
    /// triple and mint nonces the peer's replay validation has not burnt.
    ///
    /// Gossip stages re-offer our record/advert to EVERY peer (receivers
    /// dedup by nonce). The handshake stage re-offers per stalled peer: the
    /// pending request while we await its response, our response while we
    /// await its ack. The completed side never re-offers — a `Done`
    /// initiator re-sends its stored ack only when the peer's re-delivered
    /// response proves the ack was lost (see `on_response`), so retries
    /// terminate the moment both sides are done.
    async fn nudge(&mut self) -> Result<(), ReachabilityError> {
        let sends: Vec<(ValidatorIdentity, ReachabilityMsg)> = {
            let Some(state) = &self.state else {
                return Ok(());
            };
            if !state.own_advert_sent {
                let own = state.records.get(&self.me).cloned().expect("own record");
                state
                    .peers
                    .iter()
                    .map(|peer| (*peer, ReachabilityMsg::Record(own.clone())))
                    .collect()
            } else if state.view_state.is_none() {
                let own = state.adverts.get(&self.me).cloned().expect("own advert");
                state
                    .peers
                    .iter()
                    .map(|peer| (*peer, ReachabilityMsg::Advert(own.clone())))
                    .collect()
            } else {
                state
                    .handshakes
                    .iter()
                    .filter_map(|(peer, handshake)| match handshake {
                        PeerHandshake::AwaitingResponse { request } => {
                            Some((*peer, ReachabilityMsg::Request(request.clone())))
                        }
                        PeerHandshake::AwaitingAck { response, .. } => {
                            Some((*peer, ReachabilityMsg::Response(response.clone())))
                        }
                        PeerHandshake::Done { .. } => None,
                    })
                    .collect()
            }
        };
        for (peer, msg) in sends {
            self.send_msg(peer, &msg).await?;
        }
        Ok(())
    }

    async fn deliver(
        &mut self,
        from: ed25519::PublicKey,
        bytes: Vec<u8>,
    ) -> Result<(), ReachabilityError> {
        let sender = binding::identity_of(&from);
        let Some(state) = &mut self.state else {
            // no active epoch (pre-boot traffic) — nothing to bind it to.
            return Ok(());
        };
        if !state.pk_of.contains_key(&sender) {
            return self
                .emit(ReachabilityEvent::PeerFailed {
                    peer: from,
                    reason: "reachability traffic from a non-member".into(),
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
        match msg {
            ReachabilityMsg::Record(record) => self.on_record(sender, record).await,
            ReachabilityMsg::Advert(advert) => self.on_advert(sender, advert).await,
            ReachabilityMsg::Request(request) => self.on_request(sender, request).await,
            ReachabilityMsg::Response(response) => self.on_response(sender, response).await,
            ReachabilityMsg::Ack(ack) => self.on_ack(sender, ack).await,
        }
    }

    async fn on_record(
        &mut self,
        sender: ValidatorIdentity,
        record: EndpointRecord,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        if record.validator_identity != sender || record.epoch != state.epoch {
            return self.fail_peer(sender, "record identity/epoch mismatch").await;
        }
        // phase A: the set locks at version time — later (higher-nonce)
        // re-advertisements retunnel at the next cutover.
        if state.own_advert_sent {
            return Ok(());
        }
        let first_contact = !state.records.contains_key(&sender);
        match state.records.get(&sender) {
            Some(prev) if record.nonce <= prev.nonce => {}
            _ => {
                state.records.insert(sender, record);
            }
        }
        if first_contact {
            // heal join-order: the peer that just appeared may have missed
            // our initial fan-out.
            let own = state.records.get(&self.me).cloned().expect("own record");
            self.send_msg(sender, &ReachabilityMsg::Record(own)).await?;
        }
        self.advance().await
    }

    async fn on_advert(
        &mut self,
        sender: ValidatorIdentity,
        advert: EndpointAdvertisement,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        if advert.record.validator_identity != sender || advert.record.epoch != state.epoch {
            return self.fail_peer(sender, "advert identity/epoch mismatch").await;
        }
        if state.view_state.is_some() {
            return Ok(());
        }
        let first_contact = !state.adverts.contains_key(&sender);
        match state.adverts.get(&sender) {
            Some(prev) if advert.record.nonce <= prev.record.nonce => {}
            _ => {
                state.adverts.insert(sender, advert);
            }
        }
        if first_contact && state.own_advert_sent {
            let own = state.adverts.get(&self.me).cloned().expect("own advert");
            self.send_msg(sender, &ReachabilityMsg::Advert(own)).await?;
        }
        self.advance().await
    }

    /// Move the epoch forward through every stage the accumulated state now
    /// satisfies: records complete -> sign + fan out our advert; adverts
    /// complete -> verify the mesh + start handshakes; plans complete ->
    /// apply. Idempotent; called after every state change.
    async fn advance(&mut self) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("advance without epoch");

        // records -> our signed advert
        if !state.own_advert_sent && state.records.len() == state.set.validators().len() {
            let records: Vec<EndpointRecord> = state.records.values().cloned().collect();
            let version = compute_mesh_version(&records)?;
            let own_record = state.records.get(&self.me).cloned().expect("own record");
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
            if self.interface_live {
                let _ = self.effect.remove_interface();
                self.interface_live = false;
            }
            if !plans.is_empty() {
                if let Err(err) = apply_tunnel_plans(
                    &mut self.effect,
                    self.interface.clone(),
                    self.keypair.private_key_base64(),
                    self.config.wireguard_listen,
                    &plans,
                    &overrides,
                ) {
                    return self
                        .emit(ReachabilityEvent::EpochFailed {
                            epoch,
                            reason: format!("wireguard effect: {err:?}"),
                        })
                        .await;
                }
                self.interface_live = true;
            }
            return self
                .emit(ReachabilityEvent::TunnelsApplied {
                    epoch,
                    interface: self.interface.clone(),
                    peers: plans.len(),
                })
                .await;
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
                initiator_wireguard_endpoint: self.config.wireguard_listen,
                requested_allowed_ips: self.overlay.allowed_ips_for(view, peer)?,
                port_policy_hash: self.config.port_policy.hash(),
                expires_at_view: self.view + HANDSHAKE_TTL_VIEWS,
                nonce,
            };
            let request = TunnelUpgradeRequest::sign(fields, &self.config.signer);
            state
                .handshakes
                .insert(peer, PeerHandshake::AwaitingResponse {
                    request: request.clone(),
                });
            self.send_msg(peer, &ReachabilityMsg::Request(request))
                .await?;
        }
        Ok(())
    }

    /// Run the endpoint resolver for `peer`, recording a punched/relayed
    /// override or a `PeerFailed` observability event (the peer then rides
    /// its advertised endpoint).
    async fn resolve_peer(&mut self, peer: ValidatorIdentity) -> Result<(), ReachabilityError> {
        let state = self.state.as_ref().expect("resolving inside an epoch");
        let advertised = state
            .view_state
            .as_ref()
            .and_then(|view| view.record(peer))
            .map(|record| record.wireguard_endpoint.socket_addr());
        let Some(advertised) = advertised else {
            return Ok(());
        };
        match self
            .resolver
            .resolve(binding::node_key(peer), advertised)
            .await
        {
            Ok(Resolution::Advertised) => Ok(()),
            Ok(Resolution::Punched(addr)) | Ok(Resolution::Relayed(addr)) => {
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

    /// Responder side: answer a request with our signed response. A
    /// duplicate of the request we already answered (the initiator nudging —
    /// our single-shot response may be lost) re-sends the STORED response:
    /// re-signing would orphan the initiator's eventual ack, which pins ONE
    /// response by hash.
    async fn on_request(
        &mut self,
        sender: ValidatorIdentity,
        request: TunnelUpgradeRequest,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        if state.failed.contains(&sender) {
            // the pair already failed this epoch — its nonces are burnt in
            // the replay cache, so no retry can revive it; stay quiet.
            return Ok(());
        }
        if request.fields.initiator_identity != sender
            || request.fields.epoch != state.epoch
            || !initiates(sender, self.me)
        {
            return self.fail_peer(sender, "request from the wrong side").await;
        }
        match state.handshakes.get(&sender) {
            Some(PeerHandshake::AwaitingAck { request: stored, response })
                if stored.hash() == request.hash() =>
            {
                let response = response.clone();
                return self
                    .send_msg(sender, &ReachabilityMsg::Response(response))
                    .await;
            }
            // stale in-flight duplicate: our ack receipt proves the
            // initiator completed long ago — nothing left to answer.
            Some(PeerHandshake::Done { request_hash, .. })
                if *request_hash == request.hash() =>
            {
                return Ok(());
            }
            // a DIFFERENT request over an in-flight/completed handshake is a
            // re-sign the protocol never does — loud, like every mismatch.
            Some(_) => {
                return self.fail_peer(sender, "conflicting handshake request").await;
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
            responder_wireguard_endpoint: self.config.wireguard_listen,
            accepted_allowed_ips: self.overlay.allowed_ips_for(view, sender)?,
            relay_candidates: vec![],
            direct_dial_failure: None,
            keepalive_seconds: Some(KEEPALIVE_SECONDS),
            expires_at_view: self.view + HANDSHAKE_TTL_VIEWS,
            nonce,
        };
        let response = TunnelUpgradeResponse::sign(fields, &self.config.signer);
        state
            .handshakes
            .insert(sender, PeerHandshake::AwaitingAck {
                request,
                response: response.clone(),
            });
        self.send_msg(sender, &ReachabilityMsg::Response(response))
            .await
    }

    /// Initiator side: the peer responded — ack, then validate our plan.
    /// A duplicate of the response we already validated means the responder
    /// never received our single-shot ack: re-send the stored ack VERBATIM,
    /// and never re-validate — each side runs `validate_upgrade_as` exactly
    /// once per peer, so the shared replay cache never sees a nonce twice.
    async fn on_response(
        &mut self,
        sender: ValidatorIdentity,
        response: TunnelUpgradeResponse,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        if state.failed.contains(&sender) {
            // failed pairs stay failed for the epoch — see `on_request`.
            return Ok(());
        }
        let request = match state.handshakes.get(&sender) {
            Some(PeerHandshake::AwaitingResponse { request }) => request.clone(),
            Some(PeerHandshake::Done { response_hash, ack: Some(ack), .. })
                if *response_hash == response.hash() =>
            {
                let ack = ack.clone();
                return self.send_msg(sender, &ReachabilityMsg::Ack(ack)).await;
            }
            _ => {
                return self.fail_peer(sender, "unsolicited handshake response").await;
            }
        };
        if response.fields.responder_identity != sender
            || response.fields.request_hash != request.hash()
        {
            return self.fail_peer(sender, "response does not match our request").await;
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
        let plan = wireguard_upgrade::validate_upgrade_as(
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
                state.handshakes.insert(sender, PeerHandshake::Done {
                    request_hash: request.hash(),
                    response_hash: response.hash(),
                    ack: Some(ack.clone()),
                });
                state.plans.insert(sender, plan);
                self.send_msg(sender, &ReachabilityMsg::Ack(ack)).await?;
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
    /// re-validation (see `on_response` for the replay argument).
    async fn on_ack(
        &mut self,
        sender: ValidatorIdentity,
        ack: TunnelUpgradeAck,
    ) -> Result<(), ReachabilityError> {
        let state = self.state.as_mut().expect("deliver checked state");
        if state.failed.contains(&sender) {
            // failed pairs stay failed for the epoch — see `on_request`.
            return Ok(());
        }
        let (request, response) = match state.handshakes.get(&sender) {
            Some(PeerHandshake::AwaitingAck { request, response }) => {
                (request.clone(), response.clone())
            }
            Some(PeerHandshake::Done { request_hash, response_hash, .. })
                if *request_hash == ack.fields.request_hash
                    && *response_hash == ack.fields.response_hash =>
            {
                return Ok(());
            }
            _ => {
                return self.fail_peer(sender, "unsolicited handshake ack").await;
            }
        };
        if ack.fields.initiator_identity != sender
            || ack.fields.request_hash != request.hash()
            || ack.fields.response_hash != response.hash()
        {
            return self.fail_peer(sender, "ack does not match the handshake").await;
        }
        let view = state.view_state.as_ref().expect("mesh verified").clone();
        let plan = wireguard_upgrade::validate_upgrade_as(
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
                state.handshakes.insert(sender, PeerHandshake::Done {
                    request_hash: request.hash(),
                    response_hash: response.hash(),
                    ack: None,
                });
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
}
