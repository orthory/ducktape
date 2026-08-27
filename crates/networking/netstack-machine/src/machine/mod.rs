//! The machine itself: one visible dispatch over every [`Event`], a
//! cross-epoch [`Driver`] holding the few writers every effect goes
//! through, and the per-epoch state ([`crate::epoch::EpochState`]) the
//! handlers decide over.
//!
//! Layout: `mod.rs` is the dispatch and the writers; the protocol areas
//! live beside it — `restore` (retarget + the cold-restart re-apply),
//! `gossip` (member phase-A records/adverts + live re-advertisement),
//! `prewarm` (the standby layer, both roles), `handshake` (the signed
//! triple + relay routing), `steps` (the epoch step executors: advert
//! signing, mesh verification, adoption, the epoch apply), `invite` (the
//! join-window layer), `nudge` (re-offers, heals, the rendezvous sweeps),
//! and `pending` (the parked-work vocabulary).
//!
//! The per-epoch data and every pure decision over it (phase, next step,
//! nudge re-offers, nonce admission, relay slotting, the rendezvous budget)
//! live in [`crate::epoch`]; this module EXECUTES what those decisions say
//! by buffering effects.

mod gossip;
mod handshake;
mod invite;
mod nudge;
mod pending;
mod prewarm;
mod restore;
mod steps;

use std::collections::BTreeMap;
use std::net::SocketAddr;

use commonware_cryptography::{Signer as _, ed25519};
use wireguard::effect::PeerTunnelConfig;
use wireguard::{Endpoint, OverlayPolicy, UpgradeError, ValidatorIdentity};

use crate::binding;
use crate::contract::{
    Effect, Event, MachineConfig, MeshEpochEvent, ReachabilityEvent, ReqId, Resolution,
};
use crate::epoch::{EpochState, Phase, Role};
use crate::msg::ReachabilityMsg;
use crate::store::{self, PersistedMesh};
use pending::{LayersFollowUp, PendingAdopt, PendingOp, PendingRestore, WgCont};

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

/// How long the coordinated-invite bootstrap waits for the inviter's intro
/// ack over the punched underlay socket before the bootstrap reply fails.
pub(crate) const INTRO_ACK_TIMEOUT_MS: u64 = 2_000;

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
pub(crate) fn short(id: ValidatorIdentity) -> String {
    id.0[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// The reachability protocol as a stepped state machine: feed it every
/// [`Event`] with the caller's unix-millisecond stamp, perform the returned
/// [`Effect`]s in order. The one obligation the caller carries beyond
/// performing effects: an [`Effect::WgApply`] must be answered with its
/// [`Event::WgApplied`] BEFORE any other event is stepped — the push
/// round-trips inside the cascade that requested it.
pub struct Machine {
    driver: Driver,
    /// The live epoch, if any. `None` before the first retarget, while a
    /// boot restore is still resolving, and after a stand-down.
    epoch: Option<EpochState>,
}

/// The machine's cross-epoch context: everything that outlives an epoch,
/// the parked work, and the writers every effect goes through. The
/// per-epoch state is NOT here — [`Machine`] owns it and hands it to each
/// handler, so no handler has to re-borrow an optional epoch mid-flight.
pub(crate) struct Driver {
    pub(crate) config: MachineConfig,
    pub(crate) me: ValidatorIdentity,
    pub(crate) overlay: OverlayPolicy,
    pub(crate) interface: String,
    /// The step clock: unix milliseconds, stamped by the caller on every
    /// step. Record-nonce seeds and the rendezvous budget count in it.
    pub(crate) now_ms: u64,
    pub(crate) view: u64,
    /// How many nudge ticks have found the plane without an epoch.
    ///
    /// A plane that was wired but never `Retarget`ed is a black hole in both
    /// directions — it drops every inbound record and advert and sends none of
    /// its own — and silence here costs a live session to diagnose from p2p
    /// byte counters, with the symptom misread as a NAT problem.
    pub(crate) untargeted_nudges: u64,
    /// Nudge ticks since this machine started — the clock the per-peer heal
    /// cooldown counts in (see `epoch::EpochState::request_heal`).
    pub(crate) nudges: u64,
    /// A previous epoch's interface is live and must be removed before (or
    /// instead of) the next apply.
    pub(crate) interface_live: bool,
    /// The interface's BASE peers, keyed by identity: the validated plans of
    /// the last epoch apply, or the restored mesh at boot. `Some` iff the
    /// live interface carries a base the pre-warm layer may merge over;
    /// pre-warm peers layer on top (same identity: the fresher pre-warm
    /// entry wins). Survives retargets — the physical interface does too —
    /// until the next apply replaces it.
    pub(crate) base_peers: Option<BTreeMap<ValidatorIdentity, PeerTunnelConfig>>,
    /// JOIN-WINDOW peers (see [`Event::InstallInvitePeer`]):
    /// epoch-independent, merged into every apply as the weakest layer (an
    /// entry never overrides a validated plan or a pre-warm record for the
    /// same identity, and dissolves once one exists).
    pub(crate) invite_peers: BTreeMap<ValidatorIdentity, PeerTunnelConfig>,
    /// the last CONTROL endpoint observed per identity — the only-on-change
    /// ledger behind [`ReachabilityEvent::ControlEndpointObserved`].
    /// deliberately epoch-independent: a cutover must not re-announce
    /// unchanged addresses.
    pub(crate) control_endpoints: BTreeMap<ValidatorIdentity, SocketAddr>,
    /// The step's effect buffer, drained by [`Machine::step`]'s return.
    pub(crate) effects: Vec<Effect>,
    /// The correlation-id mint for the machine's own operations.
    pub(crate) next_req: u64,
    /// Host-runtime operations in flight, keyed by their [`ReqId`].
    pub(crate) pending: BTreeMap<ReqId, PendingOp>,
    /// The single in-flight interface push (see [`WgCont`]).
    pub(crate) wg: Option<(ReqId, WgCont)>,
    /// The boot restore mid-resolution (see [`PendingRestore`]).
    pub(crate) pending_restore: Option<PendingRestore>,
    /// The peers-locked-mesh adoption mid-resolution (see [`PendingAdopt`]).
    pub(crate) pending_adopt: Option<PendingAdopt>,
}

impl Machine {
    pub fn new(config: MachineConfig) -> Self {
        let me = binding::identity_of(&config.signer.public_key());
        let overlay = OverlayPolicy::ula_v6(config.chain_id.clone());
        let interface = binding::interface_name(&config.chain_id);
        Self {
            driver: Driver {
                config,
                me,
                overlay,
                interface,
                now_ms: 0,
                view: 0,
                untargeted_nudges: 0,
                nudges: 0,
                interface_live: false,
                base_peers: None,
                invite_peers: BTreeMap::new(),
                control_endpoints: BTreeMap::new(),
                effects: Vec::new(),
                next_req: 0,
                pending: BTreeMap::new(),
                wg: None,
                pending_restore: None,
                pending_adopt: None,
            },
            epoch: None,
        }
    }

    /// Step one event, stamped with the caller's unix-millisecond clock,
    /// and return the effects to perform IN ORDER. An error is a protocol
    /// invariant breach (a commitment that failed to compute) — the plane
    /// cannot continue past it.
    pub fn step(&mut self, event: Event, now_ms: u64) -> Result<Vec<Effect>, UpgradeError> {
        self.driver.now_ms = now_ms;
        debug_assert!(
            self.driver.wg.is_none() || matches!(event, Event::WgApplied { .. }),
            "an interface push must round-trip before any other event is stepped"
        );
        match event {
            Event::Retarget { event, persisted } => self.on_retarget(event, persisted)?,
            Event::Deliver { from, bytes } => {
                self.driver.deliver(self.epoch.as_mut(), from, bytes)?
            }
            Event::ViewTick(view) => self.driver.observe_view(view),
            Event::Nudge => self.driver.nudge(self.epoch.as_mut()),
            Event::InstallInvitePeer {
                token,
                peer,
                wireguard_public_key,
                endpoint,
            } => self.driver.install_invite_peer(
                self.epoch.as_ref(),
                token,
                peer,
                wireguard_public_key,
                endpoint,
            ),
            Event::BootstrapCoordinatedInvitePeer {
                token,
                peer,
                wireguard_public_key,
                intro,
            } => self
                .driver
                .bootstrap_coordinated_invite_peer(token, peer, wireguard_public_key, intro),
            Event::SendResolverDatagram { endpoint, bytes } => {
                self.driver.send_resolver_datagram(endpoint, bytes)
            }
            Event::Resolved { req, outcome } => self.on_resolved(req, outcome),
            Event::RendezvousResolved { req, outcome } => self.on_rendezvous_resolved(req, outcome),
            Event::DatagramReplied { req, outcome } => self.driver.on_datagram_replied(req, outcome),
            Event::WgApplied { req, outcome } => self.on_wg_applied(req, outcome)?,
            Event::Shutdown => self.driver.shutdown(),
        }
        Ok(std::mem::take(&mut self.driver.effects))
    }

    /// Boot or epoch cutover. Everything parked belongs to the superseded
    /// epoch (or the superseded boot restore) and is dropped; the retarget
    /// either lands an epoch now, suspends behind the boot restore, or
    /// stands the node down.
    fn on_retarget(
        &mut self,
        event: MeshEpochEvent,
        persisted: Option<Vec<u8>>,
    ) -> Result<(), UpgradeError> {
        self.driver.abandon_pending();
        self.epoch = self.driver.retarget(event, persisted)?;
        Ok(())
    }

    /// An advertised-endpoint resolution came back: resume the parked
    /// operation it belongs to, if the state it checkpointed still stands.
    fn on_resolved(&mut self, req: ReqId, outcome: Result<Resolution, String>) {
        let Some(op) = self.driver.pending.remove(&req) else {
            tracing::debug!(
                target: "ducktape::reachability",
                req = req.0,
                "resolution dropped: its operation was superseded"
            );
            return;
        };
        match op {
            PendingOp::RestoreEndpoint { owner, advertised } => self
                .driver
                .restore_endpoint_resolved(req, owner, advertised, outcome),
            PendingOp::AdoptEndpoint {
                peer,
                advertised,
                wireguard_public_key,
            } => {
                let Some(state) = self.epoch.as_mut() else {
                    return;
                };
                self.driver.adopt_endpoint_resolved(
                    state,
                    req,
                    peer,
                    advertised,
                    wireguard_public_key,
                    outcome,
                )
            }
            PendingOp::ReadvertisedEndpoint {
                owner,
                signed,
                via,
                advertised,
            } => {
                let Some(state) = self.epoch.as_mut() else {
                    return;
                };
                self.driver
                    .readvertised_endpoint_resolved(state, owner, signed, via, advertised, outcome)
            }
            PendingOp::StandbyPrewarmEndpoint(op) => {
                let Some(state) = self.epoch.as_mut() else {
                    return;
                };
                self.driver.standby_prewarm_endpoint_resolved(state, op, outcome)
            }
            PendingOp::MemberPrewarmEndpoint { record, advertised } => {
                let Some(state) = self.epoch.as_mut() else {
                    return;
                };
                self.driver
                    .member_prewarm_endpoint_resolved(state, record, advertised, outcome)
            }
            PendingOp::PeerEndpoint { peer } => {
                let Some(state) = self.epoch.as_mut() else {
                    return;
                };
                self.driver.peer_endpoint_resolved(state, peer, outcome)
            }
            PendingOp::ReadvertisedRendezvous { .. }
            | PendingOp::StandbyPrewarmRendezvous { .. }
            | PendingOp::PeerRendezvous { .. }
            | PendingOp::InviteRendezvous { .. }
            | PendingOp::IntroAck { .. } => {
                debug_assert!(false, "an endpoint resolution answered a non-resolve operation");
            }
        }
    }

    /// A by-identity rendezvous lookup came back.
    fn on_rendezvous_resolved(&mut self, req: ReqId, outcome: Result<SocketAddr, String>) {
        let Some(op) = self.driver.pending.remove(&req) else {
            tracing::debug!(
                target: "ducktape::reachability",
                req = req.0,
                "rendezvous resolution dropped: its operation was superseded"
            );
            return;
        };
        match op {
            PendingOp::ReadvertisedRendezvous { owner, signed, via } => {
                let Some(state) = self.epoch.as_mut() else {
                    return;
                };
                self.driver
                    .readvertised_rendezvous_resolved(state, owner, signed, via, outcome)
            }
            PendingOp::StandbyPrewarmRendezvous { peer } => {
                let Some(state) = self.epoch.as_mut() else {
                    return;
                };
                self.driver
                    .standby_prewarm_rendezvous_resolved(state, peer, outcome)
            }
            PendingOp::PeerRendezvous { peer } => {
                let Some(state) = self.epoch.as_mut() else {
                    return;
                };
                self.driver.peer_rendezvous_resolved(state, peer, outcome)
            }
            PendingOp::InviteRendezvous {
                token,
                peer,
                wireguard_public_key,
                intro,
            } => self.driver.invite_rendezvous_resolved(
                self.epoch.as_ref(),
                token,
                peer,
                wireguard_public_key,
                intro,
                outcome,
            ),
            PendingOp::RestoreEndpoint { .. }
            | PendingOp::AdoptEndpoint { .. }
            | PendingOp::ReadvertisedEndpoint { .. }
            | PendingOp::StandbyPrewarmEndpoint(..)
            | PendingOp::MemberPrewarmEndpoint { .. }
            | PendingOp::PeerEndpoint { .. }
            | PendingOp::IntroAck { .. } => {
                debug_assert!(false, "a rendezvous resolution answered a non-rendezvous operation");
            }
        }
    }

    /// The in-flight interface push settled: mirror what a landed push
    /// means for the interface (live, with at least an empty base), then
    /// settle whatever the push was FOR.
    fn on_wg_applied(&mut self, req: ReqId, outcome: Result<(), String>) -> Result<(), UpgradeError> {
        let Some((expected, cont)) = self.driver.wg.take() else {
            tracing::debug!(
                target: "ducktape::reachability",
                req = req.0,
                "interface push outcome dropped: no push in flight"
            );
            return Ok(());
        };
        debug_assert!(expected == req, "interface push outcomes must not reorder");
        if outcome.is_ok() {
            self.driver.interface_live = true;
            if self.driver.base_peers.is_none() {
                self.driver.base_peers = Some(BTreeMap::new());
            }
        }
        match cont {
            WgCont::Restore(apply) => {
                let (event, role, restored) = self.driver.finish_restore_apply(apply, outcome);
                self.epoch = Some(self.driver.retarget_tail(event, role, restored)?);
                Ok(())
            }
            WgCont::EpochApply {
                view,
                base,
                plans_len,
            } => {
                let Some(state) = self.epoch.as_mut() else {
                    return Ok(());
                };
                self.driver
                    .finish_epoch_apply(state, view, base, plans_len, outcome)
            }
            WgCont::Adopt {
                version,
                records,
                base,
                peer_count,
            } => {
                let Some(state) = self.epoch.as_mut() else {
                    return Ok(());
                };
                self.driver
                    .finish_adopt_apply(state, version, records, base, peer_count, outcome)
            }
            WgCont::Layers(follow) => {
                let Some(state) = self.epoch.as_mut() else {
                    return Ok(());
                };
                self.driver.finish_layers(state, follow, outcome);
                Ok(())
            }
            WgCont::InviteInstall { token, peer } => {
                self.driver.finish_invite_install(token, peer, outcome);
                Ok(())
            }
            WgCont::InviteBootstrap {
                token,
                peer,
                endpoint,
                intro,
            } => {
                self.driver
                    .finish_invite_bootstrap(token, peer, endpoint, intro, outcome);
                Ok(())
            }
        }
    }
}

impl Driver {
    // ----- writers: the few places an effect is buffered --------------------

    pub(crate) fn emit(&mut self, event: ReachabilityEvent) {
        self.effects.push(Effect::Observe(event));
    }

    pub(crate) fn send_msg(&mut self, state: &EpochState, to: ValidatorIdentity, msg: &ReachabilityMsg) {
        let Some(pk) = state.route_to(to) else {
            return;
        };
        self.effects.push(Effect::MeshSend {
            to: pk,
            bytes: msg.encode(),
        });
    }

    /// Fan one of OUR handshake messages to every peer: the addressee
    /// processes it, everyone else is a candidate relay toward an addressee
    /// we may share no direct link with. Mesh sends are best-effort, so the
    /// sender cannot know which links exist — all paths carry the message
    /// and receivers dedup.
    pub(crate) fn fan_msg(&mut self, state: &EpochState, msg: &ReachabilityMsg) {
        for peer in state.peers.clone() {
            self.send_msg(state, peer, msg);
        }
    }

    pub(crate) fn fail_peer(&mut self, state: &EpochState, peer: ValidatorIdentity, reason: &str) {
        let Some(pk) = state.pk_of.get(&peer).cloned() else {
            return;
        };
        self.emit(ReachabilityEvent::PeerFailed {
            peer: pk,
            reason: reason.to_string(),
        });
    }

    /// announce an accepted CONTROL endpoint to the node, only when it
    /// differs from the last one observed for that identity. one ledger for
    /// every acceptance path (member/standby role, records, adverts, the
    /// boot restore), so the mesh address book upstream is never churned by
    /// re-gossip of an unchanged address. own endpoint is skipped — the node
    /// does not dial itself.
    pub(crate) fn observe_control_endpoint(&mut self, owner: ValidatorIdentity, endpoint: Endpoint) {
        if owner == self.me {
            return;
        }
        let socket = endpoint.socket_addr();
        if !control_endpoint_changed(&mut self.control_endpoints, owner, socket) {
            return;
        }
        self.emit(ReachabilityEvent::ControlEndpointObserved {
            peer: owner,
            control_endpoint: socket,
        });
    }

    /// Mint the correlation id for one started operation.
    pub(crate) fn mint_req(&mut self) -> ReqId {
        self.next_req += 1;
        ReqId(self.next_req)
    }

    /// The one writer for the interface's desired peer set: request the
    /// host push (bring-up or in-place reconfiguration — the machine tracks
    /// which) and park what its outcome settles. At most one push is ever
    /// in flight: the host answers it within this very step cascade.
    pub(crate) fn start_wg_push(&mut self, peers: Vec<PeerTunnelConfig>, cont: WgCont) {
        debug_assert!(self.wg.is_none(), "a second interface push cannot start mid-flight");
        let req = self.mint_req();
        let bring_up = !self.interface_live;
        self.wg = Some((req, cont));
        self.effects.push(Effect::WgApply {
            req,
            bring_up,
            peers,
        });
    }

    /// Tear the live interface down, best-effort: every caller is leaving
    /// the mesh (stand-down) or the process (shutdown), where the
    /// teardown's error detail does not change what happens next.
    pub(crate) fn teardown_interface(&mut self) {
        if !self.interface_live {
            return;
        }
        self.effects.push(Effect::WgRemove);
        self.interface_live = false;
    }

    /// The interface's full desired peer list from the stronger layers
    /// already merged in `merged`: the join-window invite layer goes on
    /// last, and a tunnel to this node itself never exists (a restored base
    /// could in principle carry an identity that since became us).
    pub(crate) fn assemble_peers(
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
    /// is what carries the cutover: every apply reconfigures in place, so
    /// an unchanged entry (same key + same endpoint) keeps the tunnel's
    /// live sessions outright, and a changed one can re-initiate
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

    /// One merge for every interface push while an epoch is live: the
    /// stronger base (validated plans, an adopted mesh, or the restored
    /// one) under the epoch's live layers — post-lock member
    /// re-advertisements, then the pre-warm entries — with every resolved
    /// endpoint override written through (matched by identity and winning
    /// over the layered endpoint, exactly as inside `plan_peer_configs`).
    /// The join-window invite layer and the self-exclusion ride
    /// `assemble_peers`, as everywhere.
    pub(crate) fn epoch_layered_peers(
        state: &EpochState,
        mut merged: BTreeMap<ValidatorIdentity, PeerTunnelConfig>,
    ) -> BTreeMap<ValidatorIdentity, PeerTunnelConfig> {
        merged.extend(state.readvertised_peers.clone());
        merged.extend(state.prewarm_peers.clone());
        for (peer, endpoint) in &state.overrides {
            if let Some(entry) = merged.get_mut(peer) {
                entry.endpoint = Some(*endpoint);
            }
        }
        merged
    }

    /// Push the interface's full desired configuration — the phase-A base
    /// (validated plans, an adopted mesh, or the restored one) with the
    /// epoch's live layers merged over it. A member whose epoch is still
    /// assembling holds off (the follow-up is dropped) — its one epoch
    /// apply merges the layers. A push refusal keeps whatever configuration
    /// the interface had, surfaces as `EpochFailed`, and drops the
    /// follow-up too (the next accepted record or nudge retries); `follow`
    /// lands only with a push that landed.
    pub(crate) fn request_epoch_layers_push(&mut self, state: &EpochState, follow: LayersFollowUp) {
        let base = match (&self.base_peers, state.role) {
            (Some(base), _) => base.clone(),
            (None, Role::Standby) => BTreeMap::new(),
            (None, Role::Member) => return,
        };
        let merged = Self::epoch_layered_peers(state, base);
        let peers = self.assemble_peers(merged);
        self.start_wg_push(peers, WgCont::Layers(follow));
    }

    /// Settle a layered push: refusal surfaces as `EpochFailed` and drops
    /// the follow-up; a landed push emits the observation it was for.
    pub(crate) fn finish_layers(
        &mut self,
        state: &EpochState,
        follow: LayersFollowUp,
        outcome: Result<(), String>,
    ) {
        if let Err(err) = outcome {
            self.emit(ReachabilityEvent::EpochFailed {
                epoch: state.epoch,
                reason: format!("layered tunnel apply: {err}"),
            });
            return;
        }
        match follow {
            LayersFollowUp::Readvertised { owner } => {
                let Some(peer) = state.pk_of.get(&owner).cloned() else {
                    return;
                };
                self.emit(ReachabilityEvent::PeerReadvertised {
                    peer,
                    interface: self.interface.clone(),
                });
            }
            LayersFollowUp::Prewarm => {
                self.emit(ReachabilityEvent::StandbyTunnelsApplied {
                    epoch: state.epoch,
                    interface: self.interface.clone(),
                    peers: state.prewarm_peers.len(),
                });
            }
            LayersFollowUp::EndpointWriteThrough { peer, endpoint } => {
                let Some(pk) = state.pk_of.get(&peer).cloned() else {
                    return;
                };
                self.emit(ReachabilityEvent::PeerEndpointResolved { peer: pk, endpoint });
            }
        }
    }

    /// A late endpoint resolution: pre-apply, the epoch's one apply consumes
    /// the override; post-apply nothing else would, so it writes through to
    /// the live interface itself.
    pub(crate) fn write_through_if_applied(
        &mut self,
        state: &EpochState,
        peer: ValidatorIdentity,
        endpoint: SocketAddr,
    ) {
        let applied = matches!(state.phase, Phase::Applied { .. });
        if !applied {
            return;
        }
        self.request_epoch_layers_push(state, LayersFollowUp::EndpointWriteThrough { peer, endpoint });
    }

    /// The shared settlement for a live-accepted record's endpoint resolve
    /// — the pre-warm, re-advertisement, adoption, and restore paths all
    /// ride it: a resolver failure surfaces as `PeerFailed` (via the given
    /// reporter) and the peer rides its advertised endpoint.
    pub(crate) fn live_endpoint(
        &mut self,
        state: &EpochState,
        peer: ValidatorIdentity,
        advertised: SocketAddr,
        outcome: Result<Resolution, String>,
    ) -> SocketAddr {
        match outcome {
            Ok(Resolution::Advertised) => advertised,
            Ok(Resolution::Punched(addr)) => addr,
            Err(reason) => {
                self.fail_peer(state, peer, &format!("live endpoint resolution: {reason}"));
                advertised
            }
        }
    }

    /// Persist the mesh snapshot the cold-restart restore reads back: the
    /// member adverts, the post-lock member re-advertisements (a member's
    /// current life supersedes the one the epoch locked), AND the accepted
    /// standby records. The standby records ride
    /// along because a parked resident cannot re-introduce itself to a
    /// member that forgot its WireGuard key — its invite token was consumed
    /// at admission and its every remaining transport rides this overlay —
    /// so this snapshot is its only way back onto a rebooted member's
    /// interface.
    pub(crate) fn persist_mesh(&mut self, state: &EpochState) {
        if !self.config.persist {
            return;
        }
        let mesh = PersistedMesh::new(
            self.config.chain_id.clone(),
            state.epoch,
            state.adverts.values().cloned().collect(),
            state.readvertised.values().cloned().collect(),
            state.standby_records.values().cloned().collect(),
        );
        match store::encode(&mesh) {
            Ok(bytes) => self.effects.push(Effect::Persist { bytes }),
            Err(err) => self.emit(ReachabilityEvent::PersistFailed {
                reason: err.to_string(),
            }),
        }
    }

    /// A retarget supersedes everything parked: host-runtime operations,
    /// the boot restore, an adoption mid-resolution. Outcomes for dropped
    /// requests arrive later and are ignored by their absent [`ReqId`].
    pub(crate) fn abandon_pending(&mut self) {
        self.pending.clear();
        self.pending_restore = None;
        self.pending_adopt = None;
    }

    // ----- commands ---------------------------------------------------------

    pub(crate) fn observe_view(&mut self, view: u64) {
        self.view = self.view.max(view);
    }

    pub(crate) fn send_resolver_datagram(&mut self, endpoint: SocketAddr, bytes: Vec<u8>) {
        self.effects.push(Effect::UdpSend { endpoint, bytes });
    }

    /// Drain and exit; the interface is torn down on the way out.
    pub(crate) fn shutdown(&mut self) {
        self.teardown_interface();
    }

    /// Be inert, not wrong: this node is in neither set of the epoch, so it
    /// drops any live tunnel and runs no epoch until the next cutover.
    pub(crate) fn stand_down(&mut self, epoch: u64) {
        self.teardown_interface();
        self.base_peers = None;
        self.emit(ReachabilityEvent::EpochFailed {
            epoch,
            reason: "this node is neither a member nor a standby of the epoch".into(),
        });
    }

    /// Route one delivered message. `via` is the transport-authenticated
    /// DELIVERING member — with relaying it need not be the message's owner,
    /// so every handler authenticates the content signature and binds
    /// protocol state to the identity INSIDE the message; `via` only gates
    /// membership and takes the blame for undecodable/unverifiable junk.
    pub(crate) fn deliver(
        &mut self,
        epoch: Option<&mut EpochState>,
        from: ed25519::PublicKey,
        bytes: Vec<u8>,
    ) -> Result<(), UpgradeError> {
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
            self.emit(ReachabilityEvent::PeerFailed {
                peer: from,
                reason: "reachability traffic from a non-participant".into(),
            });
            return Ok(());
        }
        let msg = match ReachabilityMsg::decode(&bytes) {
            Ok(msg) => msg,
            Err(err) => {
                self.emit(ReachabilityEvent::PeerFailed {
                    peer: from,
                    reason: format!("undecodable reachability message: {err}"),
                });
                return Ok(());
            }
        };
        // a standby consumes gossip only: member records and adverts feed
        // its pre-warm tunnels; handshake traffic (fanned blindly by members
        // — senders cannot know which links exist) is not for it and carries
        // no relay duty.
        match (state.role, msg) {
            (Role::Standby, ReachabilityMsg::Record(record)) => {
                self.on_member_record(state, via, record);
                Ok(())
            }
            (Role::Standby, ReachabilityMsg::Advert(advert)) => {
                self.on_member_advert(state, via, advert);
                Ok(())
            }
            (
                Role::Standby,
                ReachabilityMsg::Request(_)
                | ReachabilityMsg::Response(_)
                | ReachabilityMsg::Ack(_),
            ) => Ok(()),
            (Role::Member, ReachabilityMsg::Record(record)) => {
                self.on_record(state, via, from, record)
            }
            (Role::Member, ReachabilityMsg::Advert(advert)) => self.on_advert(state, via, advert),
            (Role::Member, ReachabilityMsg::Request(request)) => {
                self.route_request(state, via, request)
            }
            (Role::Member, ReachabilityMsg::Response(response)) => {
                self.route_response(state, via, response)
            }
            (Role::Member, ReachabilityMsg::Ack(ack)) => self.route_ack(state, via, ack),
        }
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
}
