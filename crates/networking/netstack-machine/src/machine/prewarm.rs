//! The pre-warm layer, both roles: a member merging a STANDBY's owner-signed
//! record onto its live interface, and a standby merging the MEMBERS'
//! records onto its own — so the tunnels to a joining node exist before its
//! activation cutover.

use std::net::SocketAddr;

use commonware_cryptography::ed25519;
use wireguard::effect::PeerTunnelConfig;
use wireguard::{EndpointAdvertisement, EndpointRecord, SignedEndpointRecord, ValidatorIdentity};

use crate::binding;
use crate::contract::{Effect, Resolution};
use crate::epoch::{Admission, EpochState};
use crate::msg::ReachabilityMsg;

use super::pending::{LayersFollowUp, PendingOp, StandbyPrewarm};
use super::{Driver, KEEPALIVE_SECONDS, short};

impl Driver {
    /// A standby's owner-signed record (member role): validate, resolve its
    /// endpoint, and merge the tunnel onto the live interface — the pre-warm
    /// layer's whole trick. A higher nonce supersedes in place (the live
    /// re-advertisement rule); duplicates drop silently because every member
    /// re-offers standby records on nudge.
    pub(crate) fn on_standby_record(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        from: ed25519::PublicKey,
        signed: SignedEndpointRecord,
    ) {
        let owner = signed.record.validator_identity;
        // the record must bind to THIS epoch's member set — the same tuple
        // its owner derives from the boundary it synced. Another chain's
        // record is a violation; a neighboring epoch's is cutover skew (the
        // standby re-signs once its manifest poll crosses the boundary).
        if signed.record.namespace != state.set.namespace {
            self.fail_peer(state, via, "standby record from another chain");
            return;
        }
        let cutover_skew = signed.record.epoch != state.set.epoch
            || signed.record.valset_root != state.set.valset_root
            || signed.record.admission_root != state.set.admission_root;
        if cutover_skew {
            return;
        }
        if let Err(err) = signed.record.check(&self.config.port_policy) {
            self.fail_peer(state, via, &format!("standby record refused: {err:?}"));
            return;
        }
        let admission = state.admit_prewarm_nonce(owner, signed.record.nonce);
        if !admission.accepted() {
            return;
        }
        state.standby_records.insert(owner, signed.clone());
        state.learn_route(owner, via, from);
        self.observe_control_endpoint(owner, signed.record.control_endpoint);
        // the accepted record reaches disk NOW, not at the epoch apply: a
        // solo member never mints plans, and a reboot between accept and the
        // next apply would otherwise strand this standby for good (it cannot
        // re-introduce itself — see the restore).
        self.persist_mesh(state);
        let first_contact = admission == Admission::FirstContact;
        // endpoint-less standby: install without an endpoint — it initiates.
        match signed.record.wireguard_endpoint.map(|e| e.socket_addr()) {
            None => self.finish_standby_record(state, owner, signed, via, first_contact, None),
            Some(advertised) => {
                let req = self.mint_req();
                self.effects.push(Effect::ResolveStart {
                    req,
                    peer: binding::node_key(owner),
                    advertised,
                });
                self.pending.insert(
                    req,
                    PendingOp::StandbyPrewarmEndpoint(StandbyPrewarm {
                        owner,
                        signed,
                        via,
                        first_contact,
                        advertised,
                    }),
                );
            }
        }
    }

    /// The standby record's endpoint resolve came back.
    pub(crate) fn standby_prewarm_endpoint_resolved(
        &mut self,
        state: &mut EpochState,
        op: StandbyPrewarm,
        outcome: Result<Resolution, String>,
    ) {
        let StandbyPrewarm {
            owner,
            signed,
            via,
            first_contact,
            advertised,
        } = op;
        if !self.prewarm_current(state, owner, signed.record.nonce) {
            return;
        }
        let endpoint = self.live_endpoint(state, owner, advertised, outcome);
        self.finish_standby_record(state, owner, signed, via, first_contact, Some(endpoint));
    }

    /// Merge the accepted standby record's tunnel, push, greet a
    /// first-contact standby with our own gossip, and flood the record
    /// onward — members with no link to the standby, and the other
    /// standbys, see it through us. Accept-gated, so the flood terminates.
    fn finish_standby_record(
        &mut self,
        state: &mut EpochState,
        owner: ValidatorIdentity,
        signed: SignedEndpointRecord,
        via: ValidatorIdentity,
        first_contact: bool,
        endpoint: Option<SocketAddr>,
    ) {
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
        self.sync_prewarm(state);
        if first_contact {
            // the standby just appeared: hand it our own gossip directly —
            // the nudge re-offers cover everything else it is missing.
            let own_record = ReachabilityMsg::Record(state.own_record.clone());
            self.send_msg(state, owner, &own_record);
            if let Some(advert) = state.own_advert().cloned() {
                self.send_msg(state, owner, &ReachabilityMsg::Advert(advert));
            }
        }
        let record = ReachabilityMsg::Record(signed);
        for peer in state.flood_targets(owner, via) {
            self.send_msg(state, peer, &record);
        }
    }

    /// A member's record, received in the STANDBY role: validate and merge
    /// the member's tunnel — the standby side of the pre-warm layer. Another
    /// standby's record (members re-fan those to everyone) is silently not
    /// for us; standby<->standby tunnels assemble after activation like any
    /// member pair.
    pub(crate) fn on_member_record(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        signed: SignedEndpointRecord,
    ) {
        let owner = signed.record.validator_identity;
        if signed.verify().is_err() {
            self.fail_peer(state, via, "record signature invalid");
            return;
        }
        let not_for_us = owner == self.me || state.standbys.contains(&owner);
        if not_for_us {
            return;
        }
        if !state.set.contains(owner) {
            self.fail_peer(state, via, "record identity/epoch mismatch");
            return;
        }
        self.merge_member_prewarm(state, &signed.record);
    }

    /// A member's advertisement, received in the STANDBY role: the richer
    /// form of its record — accepted for the SAME pre-warm merge, and
    /// persisted, because the signed advert set is what the promotion
    /// reboot's cold-restart restore reads back from disk.
    pub(crate) fn on_member_advert(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        advert: EndpointAdvertisement,
    ) {
        let owner = advert.record.validator_identity;
        if advert.verify_signature().is_err() {
            self.fail_peer(state, via, "advert signature invalid");
            return;
        }
        let not_for_us = owner == self.me || state.standbys.contains(&owner);
        if not_for_us {
            return;
        }
        if !state.set.contains(owner) {
            self.fail_peer(state, via, "advert identity/epoch mismatch");
            return;
        }
        let record = advert.record.clone();
        let admission = state.admit_advert(owner, advert);
        if admission.accepted() {
            self.persist_mesh(state);
        }
        self.merge_member_prewarm(state, &record);
    }

    /// The standby side's shared merge: bind a member record to the epoch
    /// tuple, dedup by nonce, resolve, and re-apply the interface.
    fn merge_member_prewarm(&mut self, state: &mut EpochState, record: &EndpointRecord) {
        let owner = record.validator_identity;
        if record.namespace != state.set.namespace {
            self.fail_peer(state, owner, "member record from another chain");
            return;
        }
        // cutover skew: this standby's manifest poll and the members'
        // boundary crossing are not synchronized.
        let cutover_skew = record.epoch != state.set.epoch
            || record.valset_root != state.set.valset_root
            || record.admission_root != state.set.admission_root;
        if cutover_skew {
            return;
        }
        if let Err(err) = record.check(&self.config.port_policy) {
            self.fail_peer(state, owner, &format!("member record refused: {err:?}"));
            return;
        }
        let admission = state.admit_prewarm_nonce(owner, record.nonce);
        if !admission.accepted() {
            return;
        }
        self.observe_control_endpoint(owner, record.control_endpoint);
        // endpoint-less member record: install without an endpoint — it initiates.
        match record.wireguard_endpoint.map(|e| e.socket_addr()) {
            None => self.finish_member_prewarm(state, record.clone(), None),
            Some(advertised) => {
                let req = self.mint_req();
                self.effects.push(Effect::ResolveStart {
                    req,
                    peer: binding::node_key(owner),
                    advertised,
                });
                self.pending.insert(
                    req,
                    PendingOp::MemberPrewarmEndpoint {
                        record: record.clone(),
                        advertised,
                    },
                );
            }
        }
    }

    /// The member record's endpoint resolve came back (standby side).
    pub(crate) fn member_prewarm_endpoint_resolved(
        &mut self,
        state: &mut EpochState,
        record: EndpointRecord,
        advertised: SocketAddr,
        outcome: Result<Resolution, String>,
    ) {
        let owner = record.validator_identity;
        if !self.prewarm_current(state, owner, record.nonce) {
            return;
        }
        let endpoint = self.live_endpoint(state, owner, advertised, outcome);
        self.finish_member_prewarm(state, record, Some(endpoint));
    }

    fn finish_member_prewarm(
        &mut self,
        state: &mut EpochState,
        record: EndpointRecord,
        endpoint: Option<SocketAddr>,
    ) {
        let owner = record.validator_identity;
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
        self.sync_prewarm(state);
    }

    /// A parked pre-warm resumption is valid only while its record is still
    /// the owner's freshest accepted nonce — a fresher record arriving
    /// mid-resolve runs its own full accept path.
    fn prewarm_current(&self, state: &EpochState, owner: ValidatorIdentity, nonce: u64) -> bool {
        let current = state.prewarm_nonce(owner) == Some(nonce);
        if !current {
            tracing::debug!(
                target: "ducktape::reachability",
                peer = %short(owner), epoch = state.epoch,
                "pre-warm resolution dropped: a fresher record superseded it"
            );
        }
        current
    }

    /// A pre-warm change's push: the shared layered apply
    /// ([`Driver::request_epoch_layers_push`], which owns the hold-off and
    /// refusal contract), surfaced as `StandbyTunnelsApplied` when it lands.
    pub(crate) fn sync_prewarm(&mut self, state: &EpochState) {
        if state.prewarm_peers.is_empty() {
            return;
        }
        self.request_epoch_layers_push(state, LayersFollowUp::Prewarm);
    }

    /// The standby sweep's rendezvous for one endpoint-less pre-warm member
    /// came back: the resolved address lands as a pre-warm entry cloned
    /// from the effective config (the WireGuard key and allowed-ips carry
    /// over), unless a live record made the member dialable meanwhile.
    pub(crate) fn standby_prewarm_rendezvous_resolved(
        &mut self,
        state: &mut EpochState,
        peer: ValidatorIdentity,
        outcome: Result<SocketAddr, String>,
    ) {
        let addr = match outcome {
            Ok(addr) => addr,
            Err(reason) => {
                self.fail_rendezvous_fallback(state, peer, &reason);
                return;
            }
        };
        let effective = state
            .prewarm_peers
            .get(&peer)
            .or_else(|| self.base_peers.as_ref().and_then(|base| base.get(&peer)))
            .cloned();
        let Some(mut config) = effective else {
            return;
        };
        let already_dialable = config.endpoint.is_some();
        if already_dialable {
            return;
        }
        config.endpoint = Some(addr);
        state.prewarm_peers.insert(peer, config);
        self.sync_prewarm(state);
    }
}
