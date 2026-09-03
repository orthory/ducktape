//! Member-role phase-A gossip: records while the set assembles, adverts
//! until the mesh verifies — and the post-lock paths a locked epoch still
//! serves: healing behind peers, and re-tunneling a member's fresh life in
//! place (the live re-advertisement).

use std::net::SocketAddr;

use commonware_cryptography::ed25519;
use wireguard::effect::PeerTunnelConfig;
use wireguard::{EndpointAdvertisement, SignedEndpointRecord, UpgradeError, ValidatorIdentity};

use crate::binding;
use crate::contract::{Effect, Resolution};
use crate::epoch::{Admission, EpochState, MemberRecordVerdict, Phase};
use crate::msg::ReachabilityMsg;

use super::pending::{LayersFollowUp, PendingOp};
use super::{Driver, KEEPALIVE_SECONDS, short};

impl Driver {
    pub(crate) fn on_record(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        from: ed25519::PublicKey,
        signed: SignedEndpointRecord,
    ) -> Result<(), UpgradeError> {
        let owner = signed.record.validator_identity;
        // content authentication: the record may have been relayed, so the
        // delivering link proves nothing about the record's owner.
        if signed.verify().is_err() {
            self.fail_peer(state, via, "record signature invalid");
            return Ok(());
        }
        if state.standbys.contains(&owner) {
            self.on_standby_record(state, via, from, signed);
            return Ok(());
        }
        if !state.set.contains(owner) {
            self.fail_peer(state, via, "record from an unknown identity");
            return Ok(());
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
        // once the set locks, the nonce decides what the record IS: a
        // behind peer's stale life, or a fresh one signed within the epoch.
        match state.judge_member_record(owner, signed.record.nonce) {
            MemberRecordVerdict::Assembling => self.on_assembling_record(state, via, signed),
            MemberRecordVerdict::Behind => {
                self.on_behind_record(state, owner);
                Ok(())
            }
            MemberRecordVerdict::Readvertised => {
                self.on_member_readvertisement(state, via, signed);
                Ok(())
            }
        }
    }

    /// Phase A with the record set still open: admit by nonce, heal
    /// join-order, flood an acceptance onward, and take any step the state
    /// now satisfies.
    fn on_assembling_record(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        signed: SignedEndpointRecord,
    ) -> Result<(), UpgradeError> {
        let owner = signed.record.validator_identity;
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
            self.send_msg(state, owner, &own);
        }
        if admission.accepted() {
            self.observe_control_endpoint(owner, signed.record.control_endpoint);
            // relay the news: peers with no link to the owner only ever see
            // its record through us — the standbys included, whose pre-warm
            // tunnels want every member's record. Accept-gated, so the
            // flood terminates.
            let record = ReachabilityMsg::Record(signed);
            for peer in state.flood_targets(owner, via) {
                self.send_msg(state, peer, &record);
            }
        }
        self.advance(state)
    }

    /// A record at or below the nonce this epoch locked for its owner: the
    /// owner is behind in phase A, which means it never got our half —
    /// answer it on the next nudge rather than going deaf.
    fn on_behind_record(&mut self, state: &mut EpochState, owner: ValidatorIdentity) {
        state.request_heal(owner, self.nudges);
        tracing::debug!(
            target: "ducktape::reachability",
            peer = %short(owner), epoch = state.epoch,
            "record dropped: at or below the locked set — healing this peer"
        );
    }

    /// A member signed a NEW record after this epoch locked its set — a
    /// restart or an address rebind within the epoch. The locked mesh
    /// version cannot change mid-epoch, so the fresh record re-tunnels the
    /// member IN PLACE, as a layer over the applied base; the next cutover
    /// folds everything back into one verified mesh. The owner is also
    /// (re)assembling its own phase A, so it is healed like any behind
    /// peer, and an accepted re-advertisement floods onward so members with
    /// no link to the owner re-tunnel its new life too.
    fn on_member_readvertisement(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        signed: SignedEndpointRecord,
    ) {
        let owner = signed.record.validator_identity;
        // the owner is missing our half either way — cooldown-gated, so its
        // steady re-offers cannot heal-bomb us.
        state.request_heal(owner, self.nudges);
        if let Err(err) = signed.record.check(&self.config.port_policy) {
            self.fail_peer(
                state,
                via,
                &format!("re-advertised record refused: {err:?}"),
            );
            return;
        }
        let admission = state.admit_readvertisement(owner, signed.clone());
        tracing::debug!(
            target: "ducktape::reachability",
            peer = %short(owner), epoch = state.epoch,
            accepted = admission.accepted(),
            "member re-advertisement in"
        );
        if !admission.accepted() {
            return;
        }
        // a new life's reachability is decided fresh: neither the previous
        // life's punched endpoint nor its spent rendezvous budget binds it.
        state.overrides.remove(&owner);
        state.reset_rendezvous_budget(owner);
        self.observe_control_endpoint(owner, signed.record.control_endpoint);
        self.begin_readvertised_endpoint(state, owner, signed, via);
    }

    /// A re-advertised record's dialable endpoint. An advertised endpoint
    /// resolves like every live acceptance (a resolver failure surfaces as
    /// `PeerFailed` and the advertised address stands). An endpoint-less
    /// record tries the by-identity rendezvous fallback once when a
    /// coordinator is configured — a produced address is recorded as an
    /// override so the sweep sees the peer as resolved; `None` installs
    /// endpoint-less, and the nudge sweep keeps retrying the fallback under
    /// its budget while the owner initiates with the endpoints its heal
    /// carries.
    fn begin_readvertised_endpoint(
        &mut self,
        state: &mut EpochState,
        owner: ValidatorIdentity,
        signed: SignedEndpointRecord,
        via: ValidatorIdentity,
    ) {
        match signed.record.wireguard_endpoint.map(|e| e.socket_addr()) {
            Some(advertised) => {
                let req = self.mint_req();
                self.effects.push(Effect::ResolveStart {
                    req,
                    peer: binding::node_key(owner),
                    advertised,
                });
                self.pending.insert(
                    req,
                    PendingOp::ReadvertisedEndpoint {
                        owner,
                        signed,
                        via,
                        advertised,
                    },
                );
            }
            None if self.config.coordinators.is_empty() => {
                self.finish_readvertisement(state, owner, signed, via, None);
            }
            None => {
                if !state.claim_rendezvous_attempt(owner, self.now_ms) {
                    self.finish_readvertisement(state, owner, signed, via, None);
                    return;
                }
                let req = self.mint_req();
                self.effects.push(Effect::RendezvousStart {
                    req,
                    peer: binding::node_key(owner),
                });
                self.pending.insert(
                    req,
                    PendingOp::ReadvertisedRendezvous { owner, signed, via },
                );
            }
        }
    }

    /// The re-advertisement's advertised-endpoint resolve came back.
    pub(crate) fn readvertised_endpoint_resolved(
        &mut self,
        state: &mut EpochState,
        owner: ValidatorIdentity,
        signed: SignedEndpointRecord,
        via: ValidatorIdentity,
        advertised: SocketAddr,
        outcome: Result<Resolution, String>,
    ) {
        if !self.readvertisement_current(state, owner, &signed) {
            return;
        }
        let endpoint = self.live_endpoint(state, owner, advertised, outcome);
        self.finish_readvertisement(state, owner, signed, via, Some(endpoint));
    }

    /// The endpoint-less re-advertisement's rendezvous fallback came back.
    pub(crate) fn readvertised_rendezvous_resolved(
        &mut self,
        state: &mut EpochState,
        owner: ValidatorIdentity,
        signed: SignedEndpointRecord,
        via: ValidatorIdentity,
        outcome: Result<SocketAddr, String>,
    ) {
        if !self.readvertisement_current(state, owner, &signed) {
            return;
        }
        match outcome {
            Ok(addr) => {
                state.overrides.insert(owner, addr);
                self.finish_readvertisement(state, owner, signed, via, Some(addr));
            }
            Err(reason) => {
                self.fail_peer(state, owner, &format!("rendezvous fallback: {reason}"));
                self.finish_readvertisement(state, owner, signed, via, None);
            }
        }
    }

    /// A parked re-advertisement resumption is valid only while its record
    /// is still the owner's freshest accepted life — an even fresher one
    /// superseding mid-resolve runs its own full path.
    fn readvertisement_current(
        &self,
        state: &EpochState,
        owner: ValidatorIdentity,
        signed: &SignedEndpointRecord,
    ) -> bool {
        let current = state.readvertised_nonce(owner) == Some(signed.record.nonce);
        if !current {
            tracing::debug!(
                target: "ducktape::reachability",
                peer = %short(owner), epoch = state.epoch,
                "re-advertisement resolution dropped: a fresher life superseded it"
            );
        }
        current
    }

    /// Re-tunnel the fresh life in place, remember it, and flood it onward
    /// (members and standbys with no link to the owner see its new life
    /// only through us — accept-gated, so the flood terminates).
    fn finish_readvertisement(
        &mut self,
        state: &mut EpochState,
        owner: ValidatorIdentity,
        signed: SignedEndpointRecord,
        via: ValidatorIdentity,
        endpoint: Option<SocketAddr>,
    ) {
        state.readvertised_peers.insert(
            owner,
            PeerTunnelConfig {
                wireguard_public_key: signed.record.wireguard_public_key,
                endpoint,
                allowed_ips: self.overlay.identity_allowed_ips(owner),
                keepalive_seconds: Some(KEEPALIVE_SECONDS),
            },
        );
        self.persist_mesh(state);
        self.request_epoch_layers_push(state, LayersFollowUp::Readvertised { owner });
        let record = ReachabilityMsg::Record(signed);
        for peer in state.flood_targets(owner, via) {
            self.send_msg(state, peer, &record);
        }
    }

    /// THIS node's own reflexive mapping moved (a mid-epoch NAT rebind).
    /// Every peer holds a tunnel aimed at the dead mapping and nothing in
    /// the protocol makes them look again — an endpoint-less node is
    /// resolved by rendezvous exactly once, when the record it locked was
    /// admitted. So the machine gives them a NEW LIFE to admit: the same
    /// record under a fresh nonce, which lands on each peer as the post-lock
    /// re-advertisement ([`Self::on_member_readvertisement`]) and re-runs
    /// the by-identity rendezvous against the coordinator the host has
    /// already re-registered with. The standing goes `Live` so every nudge
    /// keeps re-offering the fresh life until each peer has re-tunneled it.
    ///
    /// Only an APPLIED mesh needs this. An epoch still assembling resolves
    /// every peer AFTER the rebind — the coordinator the host just
    /// re-registered with is what it asks — so the fresh mapping is what
    /// that assembly would have picked up anyway; and re-signing under an
    /// advert already signed over the old record would strand this node on
    /// a mesh version no peer can reach.
    pub(crate) fn on_reflexive_changed(
        &mut self,
        epoch: Option<&mut EpochState>,
        endpoint: SocketAddr,
    ) {
        let Some(state) = epoch else {
            tracing::debug!(
                target: "ducktape::reachability",
                %endpoint,
                "reflexive move ignored: this plane has no epoch"
            );
            return;
        };
        let tunnels_are_live = matches!(state.phase, Phase::Applied { .. });
        if !tunnels_are_live {
            tracing::debug!(
                target: "ducktape::reachability",
                %endpoint, epoch = state.epoch,
                "reflexive move ignored: this epoch has applied no tunnels yet"
            );
            return;
        }
        let mut record = state.own_record.record.clone();
        record.nonce = state.next_nonce();
        let signed = wireguard::SignedEndpointRecord::sign(record, &*self.signer);
        state.readvertise_own(signed.clone());
        tracing::info!(
            target: "ducktape::reachability",
            %endpoint, epoch = state.epoch, nonce = signed.record.nonce,
            "own reflexive moved — re-advertising this node's record to the mesh"
        );
        let own = ReachabilityMsg::Record(signed);
        for peer in state.peers.clone() {
            self.send_msg(state, peer, &own);
        }
        // the pre-warm layer holds tunnels toward this node too: a standby
        // left on the dead mapping is a promotion that starts dark.
        for standby in state.standbys.clone() {
            self.send_msg(state, standby, &own);
        }
    }

    pub(crate) fn on_advert(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        advert: EndpointAdvertisement,
    ) -> Result<(), UpgradeError> {
        let owner = advert.record.validator_identity;
        if advert.verify_signature().is_err() {
            self.fail_peer(state, via, "advert signature invalid");
            return Ok(());
        }
        if !state.set.contains(owner) {
            self.fail_peer(state, via, "advert from an unknown identity");
            return Ok(());
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
            self.send_msg(state, owner, &ReachabilityMsg::Advert(own));
        }
        if admission.accepted() {
            self.observe_control_endpoint(owner, advert.record.control_endpoint);
            // standbys ride the advert flood too: the signed advert set is
            // what they persist for their promotion reboot's restore.
            let advert = ReachabilityMsg::Advert(advert);
            for peer in state.flood_targets(owner, via) {
                self.send_msg(state, peer, &advert);
            }
        }
        self.advance(state)
    }
}
