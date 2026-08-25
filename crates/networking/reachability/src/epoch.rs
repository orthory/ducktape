//! One epoch of the reachability plane: the state a member or standby
//! accumulates between two valset cutovers, and every PURE decision over it
//! — which phase the assembly is in, which step it can take next, what the
//! periodic nudge re-offers, how a nonce-versioned gossip item fares against
//! what is already held. Nothing here performs an effect: the orchestrator
//! executes the steps and sends the messages this module decides, which is
//! what keeps the transitions testable without a transport or a clock.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use commonware_cryptography::ed25519;
use wireguard::effect::PeerTunnelConfig;
use wireguard::{
    ActiveValidatorSet, EndpointAdvertisement, EndpointRecord, MeshView, ReplayCache,
    SignedEndpointRecord, TunnelInstallPlan, TunnelUpgradeAck, TunnelUpgradeRequest,
    TunnelUpgradeResponse, ValidatorIdentity,
};

use crate::msg::ReachabilityMsg;
use crate::rendezvous::{COORD_STEP_TIMEOUT, PUNCH_STEP_TIMEOUT, PUNCH_TRIES};

/// Nudge ticks between two heals of the SAME peer.
///
/// The heal answers a peer that is behind in phase A, and the answer itself
/// lands at a peer that may also be past its own gate — which asks it to heal
/// us back, forever. The cooldown makes that exchange cost two messages per
/// pair per cooldown instead of two per tick, while a genuinely-stuck peer
/// still gets our record and advert within a few seconds (`NUDGE_INTERVAL` is
/// 2 s in the node).
const HEAL_COOLDOWN_NUDGES: u64 = 4;

/// Minimum spacing between by-identity rendezvous-fallback attempts for the
/// same endpoint-less peer. The nudge fires every couple of seconds and
/// would otherwise re-attempt a stalled resolve before the resolver's own
/// worst-case attempt (`COORD_STEP_TIMEOUT` + `PUNCH_TRIES` punch windows)
/// could even finish, storming the coordinator — so the spacing IS the
/// resolver's timeout envelope.
pub(crate) const RENDEZVOUS_FALLBACK_BACKOFF: Duration = Duration::from_secs(
    COORD_STEP_TIMEOUT.as_secs() + PUNCH_STEP_TIMEOUT.as_secs() * PUNCH_TRIES as u64,
);

/// Cap on rendezvous-fallback attempts per peer PER EPOCH. Each attempt
/// blocks the single-threaded driver loop for up to the resolver's full
/// timeout envelope, so an unbounded sweep against a peer that stays
/// unpunchable — never registered, coordinator down, or already healed by
/// WireGuard roaming (invisible to this layer) — would starve
/// `Deliver`/gossip for healthy peers forever. After the cap the peer stops
/// being swept for the epoch; the next `Retarget`'s fresh [`EpochState`]
/// grants a new budget.
pub(crate) const RENDEZVOUS_FALLBACK_MAX_ATTEMPTS: u32 = 3;

/// The epoch's record-nonce seed: unix time in MILLISECONDS. Wall-clock for
/// the same reason the rendezvous readvertise nonce is: a REBOOTED node
/// re-signs the SAME epoch tuple, and its previous life's nonces are already
/// burnt into every peer's dedup gates (`prewarm_nonces`, the phase-A record
/// map) — a fixed seed would replay-drop the reboot's re-introduction for
/// the rest of the epoch. Milliseconds so even a sub-second orchestrator
/// relaunch still climbs. A broken clock degrades to 0 exactly like
/// `nat_traversal::now_secs`: the node then re-advertises as a stale life
/// and heals at the next cutover.
pub(crate) fn epoch_nonce_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Pure backoff/budget decision: attempt iff never attempted this epoch, or
/// the backoff window has elapsed AND the per-epoch attempt budget remains.
/// `previous` = `(elapsed since the last attempt, attempts made so far)`;
/// `None` = never attempted — also the shape a fresh epoch's reset map
/// produces, which is how "a new epoch resets the budget" happens.
pub(crate) fn should_attempt_rendezvous_fallback(previous: Option<(Duration, u32)>) -> bool {
    match previous {
        None => true,
        Some((elapsed, attempts)) => {
            attempts < RENDEZVOUS_FALLBACK_MAX_ATTEMPTS && elapsed >= RENDEZVOUS_FALLBACK_BACKOFF
        }
    }
}

/// Which side of the plane this node runs for the epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    /// In the epoch's `ActiveValidatorSet`: full phase-A assembly, plus the
    /// pre-warm layer toward the epoch's standbys.
    Member,
    /// In the standby set only: record exchange and pre-warm tunnels toward
    /// the members — no advert, no handshakes, no relay duty.
    Standby,
}

/// Where a member's phase-A assembly stands. Each phase LOCKS what the
/// previous one gathered: the record set once the advert is signed over it,
/// the advert set once the view verified. A standby never leaves
/// [`Phase::Records`] — its pre-warm layer has no version lock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    /// Collecting every member's endpoint record; this node's advert is not
    /// signed yet, so the record set is still open to higher nonces.
    Records,
    /// This node's advert is signed over the locked record set and fanned
    /// out; collecting every member's advert.
    Adverts,
    /// Every advert verified into one mesh view; the pairwise handshakes are
    /// assembling the tunnel plans.
    Handshakes { view: MeshView },
    /// The epoch's one interface apply ran over the verified view.
    Applied { view: MeshView },
    /// Mesh verification or the interface apply refused the epoch: the
    /// previous tunnels stay as they were and only the next cutover retries.
    Failed,
}

impl Phase {
    /// The verified mesh view, from verification on.
    pub(crate) fn view(&self) -> Option<&MeshView> {
        match self {
            Phase::Handshakes { view } | Phase::Applied { view } => Some(view),
            Phase::Records | Phase::Adverts | Phase::Failed => None,
        }
    }
}

/// The next effectful step an epoch's accumulated state satisfies, decided
/// by [`EpochState::next_step`] and executed by the orchestrator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Every member's record is known: compute the mesh version and sign
    /// this node's advertisement over the locked set.
    SignAdvert,
    /// Every member's advert is known: verify the mesh view and start the
    /// handshakes.
    VerifyMesh,
    /// Every peer holds a validated plan or has failed: the epoch's one
    /// interface apply.
    Apply,
}

/// How a nonce-versioned gossip item fared against what the epoch already
/// holds for its owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Admission {
    /// Nothing was held for the owner: stored, and the owner just appeared.
    FirstContact,
    /// A lower nonce was held: replaced.
    Superseded,
    /// This nonce or a higher one was held: dropped (re-offers are routine).
    Stale,
}

impl Admission {
    pub(crate) fn accepted(self) -> bool {
        match self {
            Admission::FirstContact | Admission::Superseded => true,
            Admission::Stale => false,
        }
    }
}

/// Store `value` under `owner` iff its `nonce` beats the held item's.
fn admit<V>(
    held: &mut BTreeMap<ValidatorIdentity, V>,
    owner: ValidatorIdentity,
    nonce: u64,
    held_nonce: impl Fn(&V) -> u64,
    value: V,
) -> Admission {
    let admission = match held.get(&owner) {
        None => Admission::FirstContact,
        Some(prev) if nonce > held_nonce(prev) => Admission::Superseded,
        Some(_) => Admission::Stale,
    };
    if admission.accepted() {
        held.insert(owner, value);
    }
    admission
}

/// A handshake message's stage in the request -> response -> ack triple.
/// Ordered: a later stage proves the earlier one arrived, so it supersedes
/// a relay slot holding the earlier one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HandshakeStage {
    Request,
    Response,
    Ack,
}

/// Which half of a pending handshake this node is waiting on. Every stored
/// message is re-sent VERBATIM on retry — re-signing would mint a fresh
/// nonce, and a triple whose parts disagree can never validate on both
/// sides (the ack pins request and response by hash).
#[allow(
    clippy::large_enum_variant,
    reason = "each epoch holds only a handful of handshakes; boxing would complicate the retry state"
)]
pub(crate) enum PeerHandshake {
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

impl PeerHandshake {
    /// The half this node still owes the wire: re-offered on nudge until
    /// the counterparty's next message proves it arrived. A completed
    /// handshake re-offers nothing.
    fn pending_msg(&self) -> Option<ReachabilityMsg> {
        match self {
            PeerHandshake::AwaitingResponse { request } => {
                Some(ReachabilityMsg::Request(request.clone()))
            }
            PeerHandshake::AwaitingAck { response, .. } => {
                Some(ReachabilityMsg::Response(response.clone()))
            }
            PeerHandshake::Done { .. } => None,
        }
    }
}

/// A foreign handshake message this node carries between two OTHER members
/// that share no direct link: the latest-STAGE signed message per ordered
/// `(initiator, responder)` pair, re-offered on nudge until superseded or
/// expired. Signature-verified before acceptance, so a malicious member
/// cannot evict a real in-flight message by poisoning the slot.
pub(crate) struct RelaySlot {
    pub(crate) stage: HandshakeStage,
    /// The member whose signature the slot's message carries — the one peer
    /// a re-offer never needs to reach (it already has its own message).
    pub(crate) signer: ValidatorIdentity,
    pub(crate) msg: ReachabilityMsg,
    pub(crate) expires_at_view: u64,
}

/// A handshake message between two OTHER members, as the relay path sees
/// it: the ordered pair it belongs to, its stage, its signer, its expiry,
/// and the signed message itself.
pub(crate) struct RelayedHandshake {
    pub(crate) pair: (ValidatorIdentity, ValidatorIdentity),
    pub(crate) stage: HandshakeStage,
    pub(crate) signer: ValidatorIdentity,
    pub(crate) expires_at_view: u64,
    pub(crate) msg: ReachabilityMsg,
}

impl RelayedHandshake {
    pub(crate) fn request(request: TunnelUpgradeRequest) -> Self {
        let fields = &request.fields;
        Self {
            pair: (fields.initiator_identity, fields.responder_identity),
            stage: HandshakeStage::Request,
            signer: fields.initiator_identity,
            expires_at_view: fields.expires_at_view,
            msg: ReachabilityMsg::Request(request),
        }
    }

    pub(crate) fn response(response: TunnelUpgradeResponse) -> Self {
        let fields = &response.fields;
        Self {
            pair: (fields.initiator_identity, fields.responder_identity),
            stage: HandshakeStage::Response,
            signer: fields.responder_identity,
            expires_at_view: fields.expires_at_view,
            msg: ReachabilityMsg::Response(response),
        }
    }

    pub(crate) fn ack(ack: TunnelUpgradeAck) -> Self {
        let fields = &ack.fields;
        Self {
            pair: (fields.initiator_identity, fields.responder_identity),
            stage: HandshakeStage::Ack,
            signer: fields.initiator_identity,
            expires_at_view: fields.expires_at_view,
            msg: ReachabilityMsg::Ack(ack),
        }
    }

    /// The signer's content signature checks out — the only trust a relay
    /// extends, since the delivering link proves nothing about the author.
    pub(crate) fn verified(&self) -> bool {
        match &self.msg {
            ReachabilityMsg::Request(request) => request.verify_signature().is_ok(),
            ReachabilityMsg::Response(response) => response.verify_signature().is_ok(),
            ReachabilityMsg::Ack(ack) => ack.verify_signature().is_ok(),
            // gossip is flooded by its own accept-gated path, never slotted.
            ReachabilityMsg::Record(_) | ReachabilityMsg::Advert(_) => false,
        }
    }
}

/// What a relay slot did with a sighting of a foreign handshake message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RelayVerdict {
    /// Stored (fresh pair or a later stage): fan it onward.
    Carry,
    /// The pair straddles a non-member: the deliverer misbehaved.
    NonMemberPair,
    /// Expired, or a same-or-later stage is already carried: dropping it is
    /// what terminates the flood.
    Drop,
}

/// Everything one epoch accumulates on the way to its apply.
pub(crate) struct EpochState {
    pub(crate) epoch: u64,
    pub(crate) role: Role,
    pub(crate) set: ActiveValidatorSet,
    /// A member's gossip/handshake counterparties (itself excluded); a
    /// standby's are exactly the members.
    pub(crate) peers: Vec<ValidatorIdentity>,
    /// The epoch's standby identities (never in `set`).
    pub(crate) standbys: Vec<ValidatorIdentity>,
    pub(crate) pk_of: HashMap<ValidatorIdentity, ed25519::PublicKey>,
    /// The accepted nonce per pre-warm counterparty (standby records on a
    /// member, member records on a standby). Higher nonce wins — the live
    /// re-advertisement rule the phase-A member set deliberately does not
    /// have; anything at or below drops silently (re-offers are routine).
    prewarm_nonces: BTreeMap<ValidatorIdentity, u64>,
    /// The standbys' owner-signed records as accepted (member role only) —
    /// the form the nudge re-offers and the accept-gated flood relays.
    pub(crate) standby_records: BTreeMap<ValidatorIdentity, SignedEndpointRecord>,
    /// The tunnel parts derived from accepted pre-warm records (endpoint
    /// resolved, overlay route derived) — merged over the interface's base
    /// peers on every change.
    pub(crate) prewarm_peers: BTreeMap<ValidatorIdentity, PeerTunnelConfig>,
    /// Standby-directed sends route to the transport identity that DELIVERED
    /// the standby's record when that identity is not a member (the lobby
    /// ingress a parked joiner connects under, or the standby's own key) —
    /// a standby is not necessarily dialable under its record identity.
    routes: HashMap<ValidatorIdentity, ed25519::PublicKey>,
    /// One strictly-monotonic counter for EVERYTHING this identity signs in
    /// the epoch — replay keys are `(identity, epoch, nonce)`, and the
    /// advert duplicate rule wants strictly-increasing nonces too. Seeded
    /// from wall clock (`epoch_nonce_seed`), never a constant, so a reboot's
    /// fresh counter still supersedes everything the previous life signed
    /// for this same epoch tuple.
    nonce: u64,
    /// This node's own signed record for the epoch — what the nudge
    /// re-offers. In the member role it also lives in `records`; in the
    /// standby role this field is its only home (a standby is never part of
    /// the member record set).
    pub(crate) own_record: SignedEndpointRecord,
    /// Owner-signed records as they arrived (our own included) — the form
    /// that can be re-gossiped to peers the owner has no link to.
    pub(crate) records: BTreeMap<ValidatorIdentity, SignedEndpointRecord>,
    pub(crate) adverts: BTreeMap<ValidatorIdentity, EndpointAdvertisement>,
    pub(crate) phase: Phase,
    /// Peers that gossiped phase-A state at us AFTER the relevant set locked
    /// (a record once our advert signed, an advert once the view verified) —
    /// i.e. peers still missing our half of the exchange. The next nudge
    /// answers them ([`Self::heal_sends`]) and clears this.
    heal_requests: HashSet<ValidatorIdentity>,
    /// The nudge tick each peer was last healed at — the cooldown clock that
    /// keeps two settled nodes from healing each other every tick forever.
    heal_backoff: HashMap<ValidatorIdentity, u64>,
    pub(crate) replay: ReplayCache,
    /// Requests that arrived before our own `MeshView` completed (the peer
    /// verified faster); drained the moment it does. Keyed by initiator so
    /// nudged re-offers of the same request collapse to one entry.
    pub(crate) pending_requests: BTreeMap<ValidatorIdentity, TunnelUpgradeRequest>,
    pub(crate) handshakes: HashMap<ValidatorIdentity, PeerHandshake>,
    /// Relay slots keyed by `(initiator, responder)` for handshakes between
    /// two OTHER members.
    relayed: BTreeMap<(ValidatorIdentity, ValidatorIdentity), RelaySlot>,
    pub(crate) plans: BTreeMap<ValidatorIdentity, TunnelInstallPlan>,
    pub(crate) overrides: BTreeMap<ValidatorIdentity, SocketAddr>,
    /// By-identity rendezvous-fallback bookkeeping per endpoint-less peer:
    /// `(last attempt instant, attempts so far)` — the backoff + per-epoch
    /// budget behind [`Self::claim_rendezvous_attempt`]. Fresh per epoch: a
    /// `Retarget` resets the budget.
    rendezvous_attempted: BTreeMap<ValidatorIdentity, (Instant, u32)>,
    pub(crate) failed: HashSet<ValidatorIdentity>,
}

impl EpochState {
    /// Bind a fresh epoch. `own_record` is this node's signed record for
    /// the epoch tuple; its nonce seeds the epoch's signing counter, and in
    /// the member role it is the record set's first entry.
    pub(crate) fn new(
        epoch: u64,
        role: Role,
        set: ActiveValidatorSet,
        peers: Vec<ValidatorIdentity>,
        standbys: Vec<ValidatorIdentity>,
        pk_of: HashMap<ValidatorIdentity, ed25519::PublicKey>,
        own_record: SignedEndpointRecord,
    ) -> Self {
        let mut records = BTreeMap::new();
        if role == Role::Member {
            records.insert(own_record.record.validator_identity, own_record.clone());
        }
        Self {
            epoch,
            role,
            set,
            peers,
            standbys,
            pk_of,
            prewarm_nonces: BTreeMap::new(),
            standby_records: BTreeMap::new(),
            prewarm_peers: BTreeMap::new(),
            routes: HashMap::new(),
            nonce: own_record.record.nonce,
            own_record,
            records,
            adverts: BTreeMap::new(),
            phase: Phase::Records,
            heal_requests: HashSet::new(),
            heal_backoff: HashMap::new(),
            replay: ReplayCache::default(),
            pending_requests: BTreeMap::new(),
            handshakes: HashMap::new(),
            relayed: BTreeMap::new(),
            plans: BTreeMap::new(),
            overrides: BTreeMap::new(),
            rendezvous_attempted: BTreeMap::new(),
            failed: HashSet::new(),
        }
    }

    pub(crate) fn me(&self) -> ValidatorIdentity {
        self.own_record.record.validator_identity
    }

    pub(crate) fn next_nonce(&mut self) -> u64 {
        self.nonce += 1;
        self.nonce
    }

    /// The verified mesh view, from verification on.
    pub(crate) fn view(&self) -> Option<&MeshView> {
        self.phase.view()
    }

    /// The verified view a live handshake runs over. A handshake entry, and
    /// a request answered rather than parked, exist only from the handshake
    /// phase on — so the view is present by construction.
    pub(crate) fn handshake_view(&self) -> &MeshView {
        self.phase
            .view()
            .expect("a live handshake exists only over a verified mesh view")
    }

    /// This node's own signed advertisement, from the advert phase on.
    pub(crate) fn own_advert(&self) -> Option<&EndpointAdvertisement> {
        self.adverts.get(&self.me())
    }

    /// Note a peer that showed itself behind in phase A (it gossiped at a
    /// set this node already locked), so the next nudge can answer it with
    /// our record and advert. Cooldown-gated per peer: the first ask always
    /// lands; after that, once per [`HEAL_COOLDOWN_NUDGES`] ticks — so two
    /// settled nodes exchanging stale gossip fall silent instead of healing
    /// each other every tick forever.
    pub(crate) fn request_heal(&mut self, peer: ValidatorIdentity, nudges: u64) {
        let due = match self.heal_backoff.get(&peer) {
            None => true,
            Some(last) => nudges.saturating_sub(*last) >= HEAL_COOLDOWN_NUDGES,
        };
        if due {
            self.heal_requests.insert(peer);
        }
    }

    /// The heal sends for nudge tick `nudges`: this node's record and advert
    /// to every peer that asked (at most one pair per peer per tick — the
    /// same rate as the phase-A gossip it stands in for), stamping each
    /// peer's cooldown and clearing the ask set.
    pub(crate) fn heal_sends(&mut self, nudges: u64) -> Vec<(ValidatorIdentity, ReachabilityMsg)> {
        let mine: Vec<ReachabilityMsg> =
            std::iter::once(ReachabilityMsg::Record(self.own_record.clone()))
                .chain(self.own_advert().cloned().map(ReachabilityMsg::Advert))
                .collect();
        let asking = std::mem::take(&mut self.heal_requests);
        for peer in &asking {
            self.heal_backoff.insert(*peer, nudges);
        }
        asking
            .iter()
            .flat_map(|peer| mine.iter().map(|msg| (*peer, msg.clone())))
            .collect()
    }

    /// The transport identity a message for `to` is sent under: a learned
    /// route wins over the identity itself — a parked standby may only be
    /// dialable under the ingress identity that delivered its record.
    pub(crate) fn route_to(&self, to: ValidatorIdentity) -> Option<ed25519::PublicKey> {
        self.routes
            .get(&to)
            .or_else(|| self.pk_of.get(&to))
            .cloned()
    }

    /// Learn how a standby is reached from who delivered its record: a
    /// delivery straight off a non-member link (`via` is no member) names
    /// the identity that reaches it — its own key, or the shared lobby
    /// ingress it parks under; the owner delivering under its own identity
    /// retires any learned route.
    pub(crate) fn learn_route(
        &mut self,
        owner: ValidatorIdentity,
        via: ValidatorIdentity,
        from: ed25519::PublicKey,
    ) {
        let delivered_by_owner = via == owner;
        let delivered_off_mesh = !self.set.contains(via);
        match (delivered_by_owner, delivered_off_mesh) {
            (true, _) => {
                self.routes.remove(&owner);
            }
            (false, true) => {
                self.routes.insert(owner, from);
            }
            (false, false) => {}
        }
    }

    /// Every record this epoch holds, whether it arrived as signed record
    /// gossip or embedded in a member's (signed) advertisement — per member
    /// the higher nonce wins. The advance gate and the mesh version compute
    /// over THIS merged set, so a member whose record only ever reached us
    /// inside its advertisement still counts.
    pub(crate) fn known_records(&self) -> BTreeMap<ValidatorIdentity, EndpointRecord> {
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

    /// The next step the accumulated state satisfies, or `None` while the
    /// current phase is still gathering. Idempotent: decided again after
    /// every state change and after every executed step.
    pub(crate) fn next_step(&self) -> Option<Step> {
        let member_count = self.set.validators().len();
        match &self.phase {
            Phase::Records => {
                let every_record_known = self.known_records().len() == member_count;
                every_record_known.then_some(Step::SignAdvert)
            }
            Phase::Adverts => {
                let every_advert_known = self.adverts.len() == member_count;
                every_advert_known.then_some(Step::VerifyMesh)
            }
            Phase::Handshakes { .. } => {
                let every_peer_settled = self.plans.len() + self.failed.len() == self.peers.len();
                every_peer_settled.then_some(Step::Apply)
            }
            Phase::Applied { .. } | Phase::Failed => None,
        }
    }

    /// A member's phase-A record: stored iff the set is still open (no
    /// advert signed over it yet) and the nonce beats what is held.
    pub(crate) fn admit_record(
        &mut self,
        owner: ValidatorIdentity,
        signed: SignedEndpointRecord,
    ) -> Admission {
        let nonce = signed.record.nonce;
        admit(
            &mut self.records,
            owner,
            nonce,
            |held| held.record.nonce,
            signed,
        )
    }

    /// A member's advertisement, stored iff its nonce beats what is held.
    pub(crate) fn admit_advert(
        &mut self,
        owner: ValidatorIdentity,
        advert: EndpointAdvertisement,
    ) -> Admission {
        let nonce = advert.record.nonce;
        admit(
            &mut self.adverts,
            owner,
            nonce,
            |held| held.record.nonce,
            advert,
        )
    }

    /// A pre-warm counterparty's record nonce: the live re-advertisement
    /// rule — a higher nonce supersedes in place, anything else drops.
    pub(crate) fn admit_prewarm_nonce(
        &mut self,
        owner: ValidatorIdentity,
        nonce: u64,
    ) -> Admission {
        admit(&mut self.prewarm_nonces, owner, nonce, |held| *held, nonce)
    }

    /// The identities an accepted gossip item floods onward to: every peer
    /// and standby except the item's owner and the link that delivered it.
    pub(crate) fn flood_targets(
        &self,
        owner: ValidatorIdentity,
        via: ValidatorIdentity,
    ) -> Vec<ValidatorIdentity> {
        self.peers
            .iter()
            .chain(self.standbys.iter())
            .copied()
            .filter(|target| *target != owner && *target != via)
            .collect()
    }

    /// Slot a foreign handshake message by its ordered pair with stage
    /// supersession, answering whether it is worth carrying onward.
    pub(crate) fn slot_relay(&mut self, relayed: &RelayedHandshake, view: u64) -> RelayVerdict {
        let pair_in_set = self.set.contains(relayed.pair.0) && self.set.contains(relayed.pair.1);
        if !pair_in_set {
            return RelayVerdict::NonMemberPair;
        }
        let expired = relayed.expires_at_view < view;
        if expired {
            return RelayVerdict::Drop;
        }
        let already_carried = self
            .relayed
            .get(&relayed.pair)
            .is_some_and(|slot| relayed.stage <= slot.stage);
        if already_carried {
            return RelayVerdict::Drop;
        }
        self.relayed.insert(
            relayed.pair,
            RelaySlot {
                stage: relayed.stage,
                signer: relayed.signer,
                msg: relayed.msg.clone(),
                expires_at_view: relayed.expires_at_view,
            },
        );
        RelayVerdict::Carry
    }

    /// Forget every relay slot whose message expired before `view`.
    pub(crate) fn expire_relays(&mut self, view: u64) {
        self.relayed.retain(|_, slot| slot.expires_at_view >= view);
    }

    /// Burn one unit of the per-epoch rendezvous-fallback budget for `peer`
    /// if the backoff and budget allow an attempt now.
    pub(crate) fn claim_rendezvous_attempt(
        &mut self,
        peer: ValidatorIdentity,
        now: Instant,
    ) -> bool {
        let previous = self
            .rendezvous_attempted
            .get(&peer)
            .map(|(last, attempts)| (now.saturating_duration_since(*last), *attempts));
        if !should_attempt_rendezvous_fallback(previous) {
            return false;
        }
        let entry = self.rendezvous_attempted.entry(peer).or_insert((now, 0));
        *entry = (now, entry.1 + 1);
        true
    }

    /// Everything the nudge re-offers, always the STORED message, never
    /// re-signed: pre-version, a fresh record nonce would change the mesh
    /// version peers already computed; post-verify, a re-signed handshake
    /// message would desynchronize the hash-pinned triple and mint nonces
    /// the peer's replay validation has not burnt.
    ///
    /// A standby re-offers exactly its own record to every member: its
    /// single job is being installable, and member gossip flows back through
    /// the members' own re-offers. A member's gossip phases re-offer EVERY
    /// record and advert it holds — not only its own — to every peer
    /// (receivers dedup by nonce): a peer with no link to some member
    /// receives that member's gossip from us, which is what assembles a star
    /// topology. The handshake phases re-offer per stalled peer (the pending
    /// request while we await its response, our response while we await its
    /// ack) plus every live relay slot this node carries for pairs that
    /// share no direct link. A failed epoch re-offers nothing — the next
    /// cutover is its retry. The pre-warm layer re-offers in every member
    /// phase (it has no version lock).
    pub(crate) fn reoffers(&self) -> Vec<(ValidatorIdentity, ReachabilityMsg)> {
        let phase_sends = match (self.role, &self.phase) {
            (Role::Standby, _) => self.standby_reoffers(),
            (Role::Member, Phase::Records | Phase::Adverts) => self.gossip_reoffers(),
            (Role::Member, Phase::Handshakes { .. } | Phase::Applied { .. }) => {
                self.handshake_reoffers()
            }
            (Role::Member, Phase::Failed) => Vec::new(),
        };
        phase_sends
            .into_iter()
            .chain(self.prewarm_reoffers())
            .collect()
    }

    fn standby_reoffers(&self) -> Vec<(ValidatorIdentity, ReachabilityMsg)> {
        let own = ReachabilityMsg::Record(self.own_record.clone());
        self.peers.iter().map(|peer| (*peer, own.clone())).collect()
    }

    /// Every record and advert held, as messages (an advert doubles as a
    /// record carrier for members whose record gossip never reached a peer
    /// directly).
    fn member_gossip(&self) -> Vec<ReachabilityMsg> {
        self.records
            .values()
            .map(|record| ReachabilityMsg::Record(record.clone()))
            .chain(
                self.adverts
                    .values()
                    .map(|advert| ReachabilityMsg::Advert(advert.clone())),
            )
            .collect()
    }

    fn gossip_reoffers(&self) -> Vec<(ValidatorIdentity, ReachabilityMsg)> {
        let gossip = self.member_gossip();
        self.peers
            .iter()
            .flat_map(|peer| gossip.iter().map(|msg| (*peer, msg.clone())))
            .collect()
    }

    /// Our own stalled halves fan to EVERY peer, not only the counterparty:
    /// the direct link may be the one that does not exist, and any other
    /// peer can relay. Relay slots fan to every peer except the message's
    /// own signer: this node cannot know which peer has the working link to
    /// the addressee, so all candidate paths carry it.
    fn handshake_reoffers(&self) -> Vec<(ValidatorIdentity, ReachabilityMsg)> {
        let own = self
            .handshakes
            .values()
            .filter_map(PeerHandshake::pending_msg)
            .flat_map(|msg| self.peers.iter().map(move |peer| (*peer, msg.clone())));
        let relayed = self.relayed.values().flat_map(|slot| {
            self.peers
                .iter()
                .filter(|peer| **peer != slot.signer)
                .map(|peer| (*peer, slot.msg.clone()))
        });
        own.chain(relayed).collect()
    }

    /// The pre-warm layer's re-offers (member role with standbys only):
    /// member gossip to every standby — a standby with one ingress link
    /// learns every member through it — and known standby records to the
    /// member peers and the other standbys, so a lost relay heals.
    fn prewarm_reoffers(&self) -> Vec<(ValidatorIdentity, ReachabilityMsg)> {
        let has_prewarm_layer = self.role == Role::Member && !self.standbys.is_empty();
        if !has_prewarm_layer {
            return Vec::new();
        }
        let gossip = self.member_gossip();
        let to_standbys = self
            .standbys
            .iter()
            .flat_map(|standby| gossip.iter().map(|msg| (*standby, msg.clone())));
        let standby_records = self.standby_records.values().flat_map(|record| {
            let owner = record.record.validator_identity;
            let msg = ReachabilityMsg::Record(record.clone());
            self.peers
                .iter()
                .chain(self.standbys.iter())
                .filter(move |target| **target != owner)
                .map(move |target| (*target, msg.clone()))
                .collect::<Vec<_>>()
        });
        to_standbys.chain(standby_records).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The endpoint-less rendezvous-fallback backoff + budget decision:
    /// never-attempted always fires; immediately after an attempt is
    /// suppressed; once the resolver's own worst-case attempt window has
    /// elapsed, retrying is allowed again — until the per-epoch attempt
    /// budget is spent, after which no amount of elapsed time re-arms it. A
    /// new epoch resets the budget (a `Retarget` builds a fresh `EpochState`
    /// whose empty attempt map yields the `None` shape again).
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

    #[test]
    fn nonce_admission_is_first_contact_then_strictly_increasing() {
        let mut held: BTreeMap<ValidatorIdentity, u64> = BTreeMap::new();
        let owner = ValidatorIdentity([1; 32]);
        assert_eq!(
            admit(&mut held, owner, 5, |n| *n, 5),
            Admission::FirstContact
        );
        assert_eq!(admit(&mut held, owner, 5, |n| *n, 5), Admission::Stale);
        assert_eq!(admit(&mut held, owner, 4, |n| *n, 4), Admission::Stale);
        assert_eq!(admit(&mut held, owner, 6, |n| *n, 6), Admission::Superseded);
        assert_eq!(
            held[&owner], 6,
            "a stale item never overwrites the held one"
        );
    }

    #[test]
    fn handshake_stages_supersede_in_protocol_order() {
        assert!(HandshakeStage::Request < HandshakeStage::Response);
        assert!(HandshakeStage::Response < HandshakeStage::Ack);
    }

    mod fixtures {
        use std::net::{IpAddr, Ipv4Addr};

        use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
        use wireguard::{
            ActiveValidatorSet, Endpoint, EndpointRecord, MeshVersion, PortPolicy,
            SignedEndpointRecord, Transport, TunnelUpgradeAck, TunnelUpgradeAckFields,
            TunnelUpgradeRequest, TunnelUpgradeRequestFields, TunnelUpgradeResponse,
            TunnelUpgradeResponseFields, ValidatorIdentity, X25519PublicKey,
        };

        use super::super::{EpochState, Role};
        use crate::binding;

        pub(super) const VERSION: MeshVersion = MeshVersion([9; 32]);

        pub(super) fn signer(seed: u64) -> PrivateKey {
            PrivateKey::from_seed(seed)
        }

        pub(super) fn identity(signer: &PrivateKey) -> ValidatorIdentity {
            binding::identity_of(&signer.public_key())
        }

        pub(super) fn set(members: &[&PrivateKey]) -> ActiveValidatorSet {
            binding::active_set(
                "net#epoch",
                7,
                members.iter().map(|s| identity(s)).collect(),
            )
            .unwrap()
        }

        pub(super) fn record(
            signer: &PrivateKey,
            set: &ActiveValidatorSet,
            octet: u8,
            nonce: u64,
        ) -> SignedEndpointRecord {
            let policy = PortPolicy::production();
            let endpoint = |port: u16, transport| {
                Endpoint::new(
                    IpAddr::V4(Ipv4Addr::new(8, 8, 8, octet)),
                    port,
                    transport,
                    &policy,
                )
                .unwrap()
            };
            SignedEndpointRecord::sign(
                EndpointRecord {
                    namespace: set.namespace.clone(),
                    epoch: set.epoch,
                    valset_root: set.valset_root,
                    admission_root: set.admission_root,
                    validator_identity: identity(signer),
                    wireguard_public_key: X25519PublicKey([octet; 32]),
                    control_endpoint: endpoint(443, Transport::Tcp),
                    wireguard_endpoint: Some(endpoint(51820, Transport::Udp)),
                    nonce,
                },
                signer,
            )
        }

        /// An epoch from `me`'s side: `members` are the phase-A set (`me`
        /// included in the member role), `standbys` the resident tier.
        pub(super) fn epoch(
            me: &PrivateKey,
            role: Role,
            members: &[&PrivateKey],
            standbys: &[&PrivateKey],
        ) -> EpochState {
            let set = set(members);
            let pk_of = members
                .iter()
                .chain(standbys.iter())
                .map(|s| (identity(s), s.public_key()))
                .collect();
            let peers = members
                .iter()
                .map(|s| identity(s))
                .filter(|id| *id != identity(me))
                .collect();
            let standby_ids = standbys.iter().map(|s| identity(s)).collect();
            let own = record(me, &set, 1, 100);
            EpochState::new(set.epoch, role, set, peers, standby_ids, pk_of, own)
        }

        pub(super) fn request(
            set: &ActiveValidatorSet,
            initiator: &PrivateKey,
            responder: ValidatorIdentity,
            expires_at_view: u64,
        ) -> TunnelUpgradeRequest {
            TunnelUpgradeRequest::sign(
                TunnelUpgradeRequestFields {
                    namespace: set.namespace.clone(),
                    epoch: set.epoch,
                    valset_root: set.valset_root,
                    admission_root: set.admission_root,
                    mesh_version: VERSION,
                    initiator_identity: identity(initiator),
                    responder_identity: responder,
                    initiator_wireguard_public_key: X25519PublicKey([1; 32]),
                    initiator_wireguard_endpoint: None,
                    requested_allowed_ips: Vec::new(),
                    port_policy_hash: PortPolicy::production().hash(),
                    expires_at_view,
                    nonce: 1,
                },
                initiator,
            )
        }

        pub(super) fn response(
            set: &ActiveValidatorSet,
            request: &TunnelUpgradeRequest,
            responder: &PrivateKey,
            expires_at_view: u64,
        ) -> TunnelUpgradeResponse {
            TunnelUpgradeResponse::sign(
                TunnelUpgradeResponseFields {
                    request_hash: request.hash(),
                    namespace: set.namespace.clone(),
                    epoch: set.epoch,
                    valset_root: set.valset_root,
                    admission_root: set.admission_root,
                    mesh_version: VERSION,
                    responder_identity: identity(responder),
                    initiator_identity: request.fields.initiator_identity,
                    responder_wireguard_public_key: X25519PublicKey([2; 32]),
                    responder_wireguard_endpoint: None,
                    accepted_allowed_ips: Vec::new(),
                    keepalive_seconds: None,
                    expires_at_view,
                    nonce: 2,
                },
                responder,
            )
        }

        pub(super) fn ack(
            set: &ActiveValidatorSet,
            request: &TunnelUpgradeRequest,
            response: &TunnelUpgradeResponse,
            initiator: &PrivateKey,
            expires_at_view: u64,
        ) -> TunnelUpgradeAck {
            TunnelUpgradeAck::sign(
                TunnelUpgradeAckFields {
                    request_hash: request.hash(),
                    response_hash: response.hash(),
                    namespace: set.namespace.clone(),
                    epoch: set.epoch,
                    valset_root: set.valset_root,
                    admission_root: set.admission_root,
                    mesh_version: VERSION,
                    initiator_identity: identity(initiator),
                    responder_identity: response.fields.responder_identity,
                    installed_at_view: 1,
                    expires_at_view,
                    nonce: 3,
                },
                initiator,
            )
        }
    }

    use commonware_cryptography::Signer as _;
    use fixtures::{VERSION, ack, epoch, identity, record, request, response, set, signer};
    use wireguard::EndpointAdvertisement;

    #[test]
    fn next_step_walks_the_phases_as_the_state_fills() {
        let (me, peer) = (signer(1), signer(2));
        let set = set(&[&me, &peer]);
        let mut state = epoch(&me, Role::Member, &[&me, &peer], &[]);
        assert_eq!(state.phase, Phase::Records);
        assert_eq!(
            state.next_step(),
            None,
            "own record alone: the peer's is owed"
        );

        let peer_record = record(&peer, &set, 2, 200);
        assert_eq!(
            state.admit_record(identity(&peer), peer_record.clone()),
            Admission::FirstContact
        );
        assert_eq!(state.next_step(), Some(Step::SignAdvert));

        // the executor signs the advert over the locked set and moves on
        state.adverts.insert(
            state.me(),
            EndpointAdvertisement::sign(state.own_record.record.clone(), VERSION, &me),
        );
        state.phase = Phase::Adverts;
        assert_eq!(state.next_step(), None, "the peer's advert is owed");
        let peer_advert = EndpointAdvertisement::sign(peer_record.record, VERSION, &peer);
        assert_eq!(
            state.admit_advert(identity(&peer), peer_advert),
            Admission::FirstContact
        );
        assert_eq!(state.next_step(), Some(Step::VerifyMesh));

        let view = MeshView {
            active_set: set,
            mesh_version: VERSION,
            records: state.known_records().into_values().collect(),
        };
        state.phase = Phase::Handshakes { view: view.clone() };
        assert_eq!(state.next_step(), None, "the peer's handshake is unsettled");
        state.failed.insert(identity(&peer));
        assert_eq!(
            state.next_step(),
            Some(Step::Apply),
            "a failed peer settles the apply gate exactly like a plan"
        );

        state.phase = Phase::Applied { view };
        assert_eq!(state.next_step(), None, "applied is terminal");
        state.phase = Phase::Failed;
        assert_eq!(
            state.next_step(),
            None,
            "failed is terminal: the next cutover retries"
        );
    }

    #[test]
    fn a_record_embedded_in_an_advert_counts_toward_the_record_gate() {
        let (me, peer) = (signer(1), signer(2));
        let set = set(&[&me, &peer]);
        let mut state = epoch(&me, Role::Member, &[&me, &peer], &[]);
        let advert =
            EndpointAdvertisement::sign(record(&peer, &set, 2, 200).record, VERSION, &peer);
        state.admit_advert(identity(&peer), advert);
        assert_eq!(
            state.next_step(),
            Some(Step::SignAdvert),
            "the peer's record reached us only inside its advert"
        );
    }

    #[test]
    fn a_standby_reoffers_exactly_its_own_record_to_every_member() {
        let (a, b, me) = (signer(1), signer(2), signer(3));
        let state = epoch(&me, Role::Standby, &[&a, &b], &[&me]);
        let sends = state.reoffers();
        assert_eq!(sends.len(), 2);
        for (to, msg) in sends {
            assert!(to == identity(&a) || to == identity(&b));
            assert_eq!(msg, ReachabilityMsg::Record(state.own_record.clone()));
        }
    }

    #[test]
    fn member_gossip_phases_reoffer_every_held_record_and_advert() {
        let (me, peer, standby) = (signer(1), signer(2), signer(3));
        let set = set(&[&me, &peer]);
        let mut state = epoch(&me, Role::Member, &[&me, &peer], &[&standby]);
        let advert =
            EndpointAdvertisement::sign(record(&peer, &set, 2, 200).record, VERSION, &peer);
        state.admit_advert(identity(&peer), advert.clone());

        let sends = state.reoffers();
        let to_peer: Vec<_> = sends
            .iter()
            .filter(|(to, _)| *to == identity(&peer))
            .map(|(_, msg)| msg.clone())
            .collect();
        assert_eq!(
            to_peer,
            vec![
                ReachabilityMsg::Record(state.own_record.clone()),
                ReachabilityMsg::Advert(advert.clone()),
            ],
            "own record + the peer's advert (its record only arrived embedded)"
        );
        let to_standby: Vec<_> = sends
            .iter()
            .filter(|(to, _)| *to == identity(&standby))
            .map(|(_, msg)| msg.clone())
            .collect();
        assert_eq!(
            to_standby,
            vec![
                ReachabilityMsg::Record(state.own_record.clone()),
                ReachabilityMsg::Advert(advert),
            ],
            "the pre-warm layer hands every standby the member gossip"
        );
    }

    #[test]
    fn handshake_phases_reoffer_pending_halves_and_relay_slots_but_never_done() {
        let (me, a, b) = (signer(1), signer(2), signer(3));
        let set = set(&[&me, &a, &b]);
        let mut state = epoch(&me, Role::Member, &[&me, &a, &b], &[]);
        state.phase = Phase::Handshakes {
            view: MeshView {
                active_set: set.clone(),
                mesh_version: VERSION,
                records: Vec::new(),
            },
        };
        // our pending request to a
        let own_request = request(&set, &me, identity(&a), 50);
        state.handshakes.insert(
            identity(&a),
            PeerHandshake::AwaitingResponse {
                request: own_request.clone(),
            },
        );
        // a completed handshake with b
        state.handshakes.insert(
            identity(&b),
            PeerHandshake::Done {
                request_hash: [0; 32],
                response_hash: [0; 32],
                ack: None,
            },
        );
        // a foreign a->b request we carry
        let foreign = request(&set, &a, identity(&b), 50);
        assert_eq!(
            state.slot_relay(&RelayedHandshake::request(foreign.clone()), 10),
            RelayVerdict::Carry
        );

        let sends = state.reoffers();
        let own_msg = ReachabilityMsg::Request(own_request);
        let own_targets: Vec<_> = sends
            .iter()
            .filter(|(_, msg)| *msg == own_msg)
            .map(|(to, _)| *to)
            .collect();
        assert_eq!(
            own_targets,
            vec![identity(&a), identity(&b)],
            "our stalled half fans to EVERY peer — any of them can relay"
        );
        let relay_msg = ReachabilityMsg::Request(foreign);
        let relay_targets: Vec<_> = sends
            .iter()
            .filter(|(_, msg)| *msg == relay_msg)
            .map(|(to, _)| *to)
            .collect();
        assert_eq!(
            relay_targets,
            vec![identity(&b)],
            "a relay slot fans to every peer except its signer"
        );
        assert_eq!(sends.len(), 3, "the completed handshake re-offers nothing");

        state.phase = Phase::Failed;
        assert!(
            state.reoffers().is_empty(),
            "a failed epoch re-offers nothing"
        );
    }

    #[test]
    fn relay_slots_supersede_by_stage_and_drop_expired_or_foreign_pairs() {
        let (me, a, b, outsider) = (signer(1), signer(2), signer(3), signer(4));
        let set = set(&[&me, &a, &b]);
        let mut state = epoch(&me, Role::Member, &[&me, &a, &b], &[]);
        state.phase = Phase::Handshakes {
            view: MeshView {
                active_set: set.clone(),
                mesh_version: VERSION,
                records: Vec::new(),
            },
        };
        let view = 10;

        let req = request(&set, &a, identity(&b), 50);
        assert_eq!(
            state.slot_relay(&RelayedHandshake::request(req.clone()), view),
            RelayVerdict::Carry
        );
        assert_eq!(
            state.slot_relay(&RelayedHandshake::request(req.clone()), view),
            RelayVerdict::Drop,
            "a repeat sighting adds nothing"
        );
        let resp = response(&set, &req, &b, 50);
        assert_eq!(
            state.slot_relay(&RelayedHandshake::response(resp.clone()), view),
            RelayVerdict::Carry,
            "a later stage supersedes the slot"
        );
        assert_eq!(
            state.slot_relay(&RelayedHandshake::request(req.clone()), view),
            RelayVerdict::Drop,
            "an earlier stage never re-takes the slot"
        );
        let stale_ack = ack(&set, &req, &resp, &a, view - 1);
        assert_eq!(
            state.slot_relay(&RelayedHandshake::ack(stale_ack), view),
            RelayVerdict::Drop,
            "expired before it arrived"
        );
        let foreign = request(&set, &a, identity(&outsider), 50);
        assert_eq!(
            state.slot_relay(&RelayedHandshake::request(foreign), view),
            RelayVerdict::NonMemberPair
        );

        state.expire_relays(51);
        assert!(
            state.reoffers().is_empty(),
            "an expired slot is forgotten, not re-offered"
        );
    }

    #[test]
    fn a_standby_route_is_learned_from_an_off_mesh_delivery_only() {
        let (me, peer, standby, ingress) = (signer(1), signer(2), signer(3), signer(4));
        let mut state = epoch(&me, Role::Member, &[&me, &peer], &[&standby]);
        let s = identity(&standby);
        assert_eq!(
            state.route_to(s),
            Some(standby.public_key()),
            "no route learned: the identity itself"
        );
        state.learn_route(s, identity(&ingress), ingress.public_key());
        assert_eq!(
            state.route_to(s),
            Some(ingress.public_key()),
            "delivered off a non-member link: that link reaches the standby"
        );
        state.learn_route(s, identity(&peer), peer.public_key());
        assert_eq!(
            state.route_to(s),
            Some(ingress.public_key()),
            "a member relaying the record says nothing about the standby's own link"
        );
        state.learn_route(s, s, standby.public_key());
        assert_eq!(
            state.route_to(s),
            Some(standby.public_key()),
            "the owner delivering under its own identity retires the learned route"
        );
    }

    #[test]
    fn rendezvous_attempts_are_claimed_against_the_backoff_and_budget() {
        let (me, peer) = (signer(1), signer(2));
        let mut state = epoch(&me, Role::Member, &[&me, &peer], &[]);
        let p = identity(&peer);
        let t0 = Instant::now();
        assert!(
            state.claim_rendezvous_attempt(p, t0),
            "first attempt is free"
        );
        assert!(
            !state.claim_rendezvous_attempt(p, t0),
            "inside the backoff window"
        );
        let t1 = t0 + RENDEZVOUS_FALLBACK_BACKOFF;
        assert!(state.claim_rendezvous_attempt(p, t1));
        let t2 = t1 + RENDEZVOUS_FALLBACK_BACKOFF;
        assert!(
            state.claim_rendezvous_attempt(p, t2),
            "the last budgeted attempt"
        );
        let t3 = t2 + RENDEZVOUS_FALLBACK_BACKOFF * 10;
        assert!(
            !state.claim_rendezvous_attempt(p, t3),
            "budget spent for the epoch"
        );
        assert!(
            state.claim_rendezvous_attempt(identity(&me), t3),
            "budgets are per peer"
        );
    }
}
