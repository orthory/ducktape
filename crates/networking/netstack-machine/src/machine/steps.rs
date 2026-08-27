//! The epoch step executors: every step [`EpochState::next_step`] decides —
//! signing the advert over the locked record set, verifying the mesh view
//! (or adopting the peers' lock after a mid-epoch restart), and the epoch's
//! ONE interface apply.

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;

use wireguard::effect::{PeerTunnelConfig, plan_peer_configs};
use wireguard::{
    EndpointAdvertisement, EndpointRecord, MeshVersion, MeshView, TunnelInstallPlan, UpgradeError,
    ValidatorIdentity, X25519PublicKey, compute_mesh_version,
};

use crate::binding;
use crate::contract::{Effect, ReachabilityEvent, ReqId, Resolution};
use crate::epoch::{EpochState, OwnRecordStanding, Phase, Step};
use crate::msg::ReachabilityMsg;

use super::pending::{PendingAdopt, PendingOp, WgCont};
use super::{Driver, KEEPALIVE_SECONDS};

impl Driver {
    /// Take every step the accumulated state now satisfies: the decision is
    /// [`EpochState::next_step`]'s, re-taken after each executed step until
    /// the phase is gathering again (or terminal), or until a step
    /// suspended behind the host — an interface push in flight, or an
    /// adoption resolving endpoints — whose own settlement re-advances.
    /// Idempotent; called after every state change.
    pub(crate) fn advance(&mut self, state: &mut EpochState) -> Result<(), UpgradeError> {
        let adopting = self.pending_adopt.is_some();
        if adopting {
            return Ok(());
        }
        while let Some(step) = state.next_step() {
            match step {
                Step::SignAdvert => self.sign_advert(state)?,
                Step::VerifyMesh => self.verify_mesh(state)?,
                Step::Apply => self.apply_epoch(state),
            }
            let suspended = self.wg.is_some() || self.pending_adopt.is_some();
            if suspended {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Records -> Adverts: compute the mesh version over the locked record
    /// set, sign our advertisement over it, and fan it out.
    fn sign_advert(&mut self, state: &mut EpochState) -> Result<(), UpgradeError> {
        let records: Vec<EndpointRecord> = state.known_records().into_values().collect();
        let version = compute_mesh_version(&records)?;
        let advert =
            EndpointAdvertisement::sign(state.own_record.record.clone(), version, &*self.signer);
        state.adverts.insert(self.me, advert.clone());
        state.phase = Phase::Adverts;
        tracing::debug!(
            target: "ducktape::reachability",
            epoch = state.epoch, peers = state.peers.len(),
            "phase A complete: fanning out our advert"
        );
        self.fan_msg(state, &ReachabilityMsg::Advert(advert));
        Ok(())
    }

    /// Adverts -> Handshakes: verify every advert into one mesh view, then
    /// start the handshakes this node initiates and answer the requests
    /// that arrived from peers whose mesh completed before ours.
    fn verify_mesh(&mut self, state: &mut EpochState) -> Result<(), UpgradeError> {
        let epoch = state.epoch;
        let ads: Vec<EndpointAdvertisement> = state.adverts.values().cloned().collect();
        let view = match MeshView::verify(state.set.clone(), ads, &self.config.port_policy) {
            Ok(view) => view,
            // every peer on one version that is not ours: the peers locked
            // this mesh while this node re-assembled (a mid-epoch restart)
            // — adopt their lock instead of failing the epoch.
            Err(UpgradeError::MeshVersionMismatch) => {
                self.adopt_peers_locked_view(state);
                return Ok(());
            }
            Err(err) => {
                state.phase = Phase::Failed;
                self.emit(ReachabilityEvent::EpochFailed {
                    epoch,
                    reason: format!("mesh verification: {err:?}"),
                });
                return Ok(());
            }
        };
        let version = view.mesh_version;
        state.phase = Phase::Handshakes { view };
        self.emit(ReachabilityEvent::MeshReady { epoch, version });
        self.start_handshakes(state)?;
        let pending = std::mem::take(&mut state.pending_requests);
        for (sender, request) in pending {
            self.on_request(state, sender, request)?;
        }
        Ok(())
    }

    /// The peers all locked one mesh version this node's own advert is not
    /// part of: this node re-assembled mid-epoch (a restart, typically),
    /// and the set it can lock now contains its OWN fresh record, so no
    /// re-verification can ever reach the peers' version. Failing the epoch
    /// here would strand the node until the next cutover — instead it
    /// ADOPTS the peers' lock: every peer advert is owner-signed and bound
    /// to this epoch tuple, so their records install as the applied base
    /// exactly like the cold-restart restore (endpoints freshly resolved),
    /// the adopted view carries the peers' version verbatim, and this
    /// node's fresh record keeps re-offering ([`OwnRecordStanding::Live`])
    /// until every peer re-tunnels it through the re-advertisement path.
    /// No unanimous peer version — several nodes re-assembling at once —
    /// stays a failed epoch: the next cutover reassembles from scratch.
    fn adopt_peers_locked_view(&mut self, state: &mut EpochState) {
        let epoch = state.epoch;
        let Some(version) = state.peers_locked_version() else {
            state.phase = Phase::Failed;
            self.emit(ReachabilityEvent::EpochFailed {
                epoch,
                reason: "mesh verification: version mismatch with no unanimous peer version".into(),
            });
            return;
        };
        // every peer's advert is present (`peers_locked_version` requires
        // it). The adopted view holds the peers' locked records with this
        // node's own fresh record in its slot — the version stays the
        // peers' lock, never a recomputation, because the fresh record is
        // exactly what that lock predates.
        let peer_records: Vec<(ValidatorIdentity, EndpointRecord)> = state
            .peers
            .iter()
            .map(|peer| (*peer, state.adverts[peer].record.clone()))
            .collect();
        let records: Vec<EndpointRecord> = state
            .set
            .validators()
            .iter()
            .map(|id| match *id == self.me {
                true => state.own_record.record.clone(),
                false => state.adverts[id].record.clone(),
            })
            .collect();
        let mut base: BTreeMap<ValidatorIdentity, PeerTunnelConfig> = BTreeMap::new();
        let mut outstanding: BTreeSet<ReqId> = BTreeSet::new();
        for (peer, record) in peer_records {
            // same contract as the restore: an endpoint-less record installs
            // without an endpoint (the peer initiates and WireGuard roams);
            // the nudge sweep owns the rendezvous fallback afterwards.
            match record.wireguard_endpoint.map(|e| e.socket_addr()) {
                None => {
                    base.insert(
                        peer,
                        PeerTunnelConfig {
                            wireguard_public_key: record.wireguard_public_key,
                            endpoint: None,
                            allowed_ips: self.overlay.identity_allowed_ips(peer),
                            keepalive_seconds: Some(KEEPALIVE_SECONDS),
                        },
                    );
                }
                Some(advertised) => {
                    let req = self.mint_req();
                    self.effects.push(Effect::ResolveStart {
                        req,
                        peer: binding::node_key(peer),
                        advertised,
                    });
                    self.pending.insert(
                        req,
                        PendingOp::AdoptEndpoint {
                            peer,
                            advertised,
                            wireguard_public_key: record.wireguard_public_key,
                        },
                    );
                    outstanding.insert(req);
                }
            }
        }
        let ready = outstanding.is_empty();
        self.pending_adopt = Some(PendingAdopt {
            version,
            records,
            base,
            outstanding,
        });
        if ready {
            self.finish_adopt_resolves(state);
        }
    }

    /// One adopted peer record's endpoint resolved; the last one in joins
    /// the adoption into its apply.
    pub(crate) fn adopt_endpoint_resolved(
        &mut self,
        state: &mut EpochState,
        req: ReqId,
        peer: ValidatorIdentity,
        advertised: SocketAddr,
        wireguard_public_key: X25519PublicKey,
        outcome: Result<Resolution, String>,
    ) {
        let endpoint = self.live_endpoint(state, peer, advertised, outcome);
        let allowed_ips = self.overlay.identity_allowed_ips(peer);
        let Some(adopt) = self.pending_adopt.as_mut() else {
            return;
        };
        adopt.base.insert(
            peer,
            PeerTunnelConfig {
                wireguard_public_key,
                endpoint: Some(endpoint),
                allowed_ips,
                keepalive_seconds: Some(KEEPALIVE_SECONDS),
            },
        );
        adopt.outstanding.remove(&req);
        if adopt.outstanding.is_empty() {
            self.finish_adopt_resolves(state);
        }
    }

    /// Every adopted endpoint settled: push the adopted base under the
    /// epoch's live layers.
    fn finish_adopt_resolves(&mut self, state: &mut EpochState) {
        let Some(adopt) = self.pending_adopt.take() else {
            return;
        };
        let peer_count = adopt.base.len();
        let merged = Self::epoch_layered_peers(state, adopt.base.clone());
        let peers = self.assemble_peers(merged);
        self.start_wg_push(
            peers,
            WgCont::Adopt {
                version: adopt.version,
                records: adopt.records,
                base: adopt.base,
                peer_count,
            },
        );
    }

    /// The adoption's push settled.
    pub(crate) fn finish_adopt_apply(
        &mut self,
        state: &mut EpochState,
        version: MeshVersion,
        records: Vec<EndpointRecord>,
        base: BTreeMap<ValidatorIdentity, PeerTunnelConfig>,
        peer_count: usize,
        outcome: Result<(), String>,
    ) -> Result<(), UpgradeError> {
        let epoch = state.epoch;
        if let Err(err) = outcome {
            state.phase = Phase::Failed;
            self.emit(ReachabilityEvent::EpochFailed {
                epoch,
                reason: format!("wireguard effect: {err}"),
            });
            return Ok(());
        }
        self.base_peers = Some(base);
        let view = MeshView {
            active_set: state.set.clone(),
            mesh_version: version,
            records,
        };
        state.phase = Phase::Applied { view };
        state.own_standing = OwnRecordStanding::Live;
        self.persist_mesh(state);
        self.emit(ReachabilityEvent::MeshAdopted {
            epoch,
            version,
            peers: peer_count,
        });
        self.emit(ReachabilityEvent::TunnelsApplied {
            epoch,
            interface: self.interface.clone(),
            peers: peer_count,
        });
        let prewarm_count = state.prewarm_peers.len();
        if prewarm_count > 0 {
            self.emit(ReachabilityEvent::StandbyTunnelsApplied {
                epoch,
                interface: self.interface.clone(),
                peers: prewarm_count,
            });
        }
        // requests parked while this node re-assembled validate against the
        // adopted view now, exactly as the verified path drains them.
        let pending = std::mem::take(&mut state.pending_requests);
        for (sender, request) in pending {
            self.on_request(state, sender, request)?;
        }
        self.advance(state)
    }

    /// Handshakes -> Applied: the epoch's ONE interface apply. The validated
    /// plans become the interface's new BASE; the epoch's live layers merge
    /// over it (post-lock re-advertisements, then the pre-warm peers — same
    /// identity: the fresher layered entry wins), so a standby tunnel or a
    /// re-tunneled member that assembled during the epoch's bring-up
    /// survives the cutover. The apply runs even with no peers at all: a
    /// single-member
    /// network (every fresh desktop workspace) and an all-peers-failed
    /// epoch still need the interface up — the node's own /128 is what the
    /// per-use media planes bind, so a peer-less interface is the
    /// difference between a working solo huddle and a join that hangs in
    /// "connecting" forever.
    fn apply_epoch(&mut self, state: &mut EpochState) {
        let view = state
            .view()
            .cloned()
            .expect("the apply step is decided only over a verified view");
        let plans: Vec<TunnelInstallPlan> = state.plans.values().cloned().collect();
        let base: BTreeMap<ValidatorIdentity, PeerTunnelConfig> = plans
            .iter()
            .map(TunnelInstallPlan::peer_identity)
            .zip(plan_peer_configs(&plans, &state.overrides))
            .collect();
        let merged = Self::epoch_layered_peers(state, base.clone());
        let peers = self.assemble_peers(merged);
        // the epoch's apply RECONFIGURES the live interface onto its new
        // base (or brings it up on the first apply): the interface, its
        // local address, and every tunnel whose config is unchanged carry
        // straight across the cutover — an established WireGuard session
        // never drops for a membership change elsewhere in the set. On an
        // effect refusal the interface keeps its previous configuration and
        // the epoch fails; the next cutover retries.
        self.start_wg_push(
            peers,
            WgCont::EpochApply {
                view,
                base,
                plans_len: plans.len(),
            },
        );
    }

    /// The epoch apply's push settled.
    pub(crate) fn finish_epoch_apply(
        &mut self,
        state: &mut EpochState,
        view: MeshView,
        base: BTreeMap<ValidatorIdentity, PeerTunnelConfig>,
        plans_len: usize,
        outcome: Result<(), String>,
    ) -> Result<(), UpgradeError> {
        let epoch = state.epoch;
        if let Err(err) = outcome {
            state.phase = Phase::Failed;
            self.emit(ReachabilityEvent::EpochFailed {
                epoch,
                reason: format!("wireguard effect: {err}"),
            });
            return Ok(());
        }
        self.base_peers = Some(base);
        state.phase = Phase::Applied { view };
        // the epoch's mesh is now REAL — remember it for the cold-restart
        // re-apply. Only with member plans: an all-peers-failed epoch must
        // not clobber the last mesh that actually carried member tunnels.
        // (The accepted standby records ride every snapshot regardless —
        // their own persist trigger is the accept itself.)
        if plans_len > 0 {
            self.persist_mesh(state);
        }
        self.emit(ReachabilityEvent::TunnelsApplied {
            epoch,
            interface: self.interface.clone(),
            peers: plans_len,
        });
        let prewarm_count = state.prewarm_peers.len();
        if prewarm_count > 0 {
            self.emit(ReachabilityEvent::StandbyTunnelsApplied {
                epoch,
                interface: self.interface.clone(),
                peers: prewarm_count,
            });
        }
        self.advance(state)
    }
}
