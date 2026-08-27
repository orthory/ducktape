//! The signed handshake triple (request -> response -> ack) and its relay
//! routing: for each unordered member pair exactly one side initiates
//! ([`super::initiates`]), both sides validate the triple exactly once via
//! `validate_upgrade_as`, and members carry each other's halves when two
//! members share no direct link.

use std::net::SocketAddr;

use wireguard::{
    Perspective, TunnelUpgradeAck, TunnelUpgradeAckFields, TunnelUpgradeRequest,
    TunnelUpgradeRequestFields, TunnelUpgradeResponse, TunnelUpgradeResponseFields, UpgradeError,
    ValidatorIdentity,
};

use crate::binding;
use crate::contract::{Effect, Resolution};
use crate::epoch::{EpochState, PeerHandshake, RelayVerdict, RelayedHandshake};
use crate::msg::ReachabilityMsg;

use super::pending::PendingOp;
use super::{Driver, HANDSHAKE_TTL_VIEWS, KEEPALIVE_SECONDS, initiates, short};

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

impl Driver {
    pub(crate) fn route_request(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        request: TunnelUpgradeRequest,
    ) -> Result<(), UpgradeError> {
        let bearing = bearing(
            self.me,
            request.fields.initiator_identity,
            request.fields.responder_identity,
        );
        match bearing {
            Bearing::Addressee => self.on_request(state, via, request),
            // our own message relayed back around — nothing to do.
            Bearing::Author => Ok(()),
            Bearing::Bystander => {
                self.relay(state, via, RelayedHandshake::request(request));
                Ok(())
            }
        }
    }

    pub(crate) fn route_response(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        response: TunnelUpgradeResponse,
    ) -> Result<(), UpgradeError> {
        let bearing = bearing(
            self.me,
            response.fields.responder_identity,
            response.fields.initiator_identity,
        );
        match bearing {
            Bearing::Addressee => self.on_response(state, via, response),
            Bearing::Author => Ok(()),
            Bearing::Bystander => {
                self.relay(state, via, RelayedHandshake::response(response));
                Ok(())
            }
        }
    }

    pub(crate) fn route_ack(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        ack: TunnelUpgradeAck,
    ) -> Result<(), UpgradeError> {
        let bearing = bearing(
            self.me,
            ack.fields.initiator_identity,
            ack.fields.responder_identity,
        );
        match bearing {
            Bearing::Addressee => self.on_ack(state, via, ack),
            Bearing::Author => Ok(()),
            Bearing::Bystander => {
                self.relay(state, via, RelayedHandshake::ack(ack));
                Ok(())
            }
        }
    }

    /// Carry a handshake message between two OTHER members: verify, slot by
    /// `(initiator, responder)` with stage supersession, and fan out to every
    /// peer except the delivering one and the message's signer — this node
    /// cannot know which peer holds the working link to the addressee.
    fn relay(&mut self, state: &mut EpochState, via: ValidatorIdentity, relayed: RelayedHandshake) {
        if !relayed.verified() {
            self.fail_peer(state, via, "relayed an unverifiable handshake message");
            return;
        }
        match state.slot_relay(&relayed, self.view) {
            RelayVerdict::NonMemberPair => {
                self.fail_peer(state, via, "relayed a handshake for a non-member pair");
            }
            RelayVerdict::Drop => {}
            RelayVerdict::Carry => {
                let targets: Vec<ValidatorIdentity> = state
                    .peers
                    .iter()
                    .copied()
                    .filter(|peer| *peer != via && *peer != relayed.signer)
                    .collect();
                for peer in targets {
                    self.send_msg(state, peer, &relayed.msg);
                }
            }
        }
    }

    /// Initiator side: start the endpoint resolve for each
    /// lower-identity-initiates peer and fan its signed request. The
    /// request does not depend on the resolve's outcome (it carries this
    /// node's CONFIGURED endpoint), so the two proceed concurrently; a
    /// punched resolution landing after the epoch applied writes itself
    /// through to the live interface.
    pub(crate) fn start_handshakes(&mut self, state: &mut EpochState) -> Result<(), UpgradeError> {
        let targets: Vec<ValidatorIdentity> = state
            .peers
            .iter()
            .copied()
            .filter(|peer| initiates(self.me, *peer))
            .collect();
        for peer in targets {
            self.request_resolve_peer(state, peer);
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
                initiator_wireguard_public_key: self.config.wireguard_public,
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
            self.fan_msg(state, &ReachabilityMsg::Request(request));
        }
        Ok(())
    }

    /// Start the endpoint resolve for `peer`: a punched result records an
    /// override (written through to the live interface when it lands
    /// post-apply), a failure surfaces as `PeerFailed` and the peer rides
    /// its advertised endpoint.
    pub(crate) fn request_resolve_peer(&mut self, state: &mut EpochState, peer: ValidatorIdentity) {
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
            self.request_peer_rendezvous_fallback(state, peer);
            return;
        };
        let req = self.mint_req();
        self.effects.push(Effect::ResolveStart {
            req,
            peer: binding::node_key(peer),
            advertised,
        });
        self.pending.insert(req, PendingOp::PeerEndpoint { peer });
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
    fn request_peer_rendezvous_fallback(&mut self, state: &mut EpochState, peer: ValidatorIdentity) {
        if self.config.coordinators.is_empty() {
            return;
        }
        let already_resolved = state.overrides.contains_key(&peer);
        if already_resolved {
            return;
        }
        self.request_rendezvous_by_identity(state, peer, PendingOp::PeerRendezvous { peer });
    }

    /// A peer's advertised-endpoint resolve came back.
    pub(crate) fn peer_endpoint_resolved(
        &mut self,
        state: &mut EpochState,
        peer: ValidatorIdentity,
        outcome: Result<Resolution, String>,
    ) {
        match outcome {
            Ok(Resolution::Advertised) => {}
            Ok(Resolution::Punched(addr)) => {
                state.overrides.insert(peer, addr);
                self.write_through_if_applied(state, peer, addr);
            }
            Err(reason) => {
                self.fail_peer(state, peer, &format!("endpoint resolution: {reason}"));
            }
        }
    }

    /// A peer's by-identity rendezvous came back (handshake fallback and
    /// the member sweep share this settlement).
    pub(crate) fn peer_rendezvous_resolved(
        &mut self,
        state: &mut EpochState,
        peer: ValidatorIdentity,
        outcome: Result<SocketAddr, String>,
    ) {
        match outcome {
            Ok(addr) => {
                state.overrides.insert(peer, addr);
                self.write_through_if_applied(state, peer, addr);
            }
            Err(reason) => {
                self.fail_peer(state, peer, &format!("rendezvous fallback: {reason}"));
            }
        }
    }

    /// Responder side: answer a request with our signed response. A
    /// duplicate of the request we already answered (the initiator nudging —
    /// our single-shot response may be lost) re-sends the STORED response:
    /// re-signing would orphan the initiator's eventual ack, which pins ONE
    /// response by hash. `via` is the delivering member (possibly a relay);
    /// the counterparty is the request's SIGNED initiator.
    pub(crate) fn on_request(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        request: TunnelUpgradeRequest,
    ) -> Result<(), UpgradeError> {
        let sender = request.fields.initiator_identity;
        if request.verify_signature().is_err() {
            self.fail_peer(state, via, "request signature invalid");
            return Ok(());
        }
        if !state.set.contains(sender) {
            self.fail_peer(state, via, "request from a non-member initiator");
            return Ok(());
        }
        // the pair already failed this epoch — its nonces are burnt in the
        // replay cache, so no retry can revive it; stay quiet.
        if state.failed.contains(&sender) {
            return Ok(());
        }
        let wrong_side = request.fields.epoch != state.epoch || !initiates(sender, self.me);
        if wrong_side {
            self.fail_peer(state, sender, "request from the wrong side");
            return Ok(());
        }
        match state.handshakes.get(&sender) {
            Some(PeerHandshake::AwaitingAck {
                request: stored,
                response,
            }) if stored.hash() == request.hash() => {
                let response = response.clone();
                self.fan_msg(state, &ReachabilityMsg::Response(response));
                return Ok(());
            }
            // stale in-flight duplicate: our ack receipt proves the
            // initiator completed long ago — nothing left to answer.
            Some(PeerHandshake::Done { request_hash, .. }) if *request_hash == request.hash() => {
                return Ok(());
            }
            // a DIFFERENT request over an in-flight/completed handshake is a
            // re-sign the protocol never does — loud, like every mismatch.
            Some(_) => {
                self.fail_peer(state, sender, "conflicting handshake request");
                return Ok(());
            }
            None => {}
        }
        // the peer's mesh completed before ours; answer once ours does.
        let mesh_verified = state.view().is_some();
        if !mesh_verified {
            state.pending_requests.insert(sender, request);
            return Ok(());
        }
        self.request_resolve_peer(state, sender);
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
            responder_wireguard_public_key: self.config.wireguard_public,
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
        self.fan_msg(state, &ReachabilityMsg::Response(response));
        Ok(())
    }

    /// Initiator side: the peer responded — ack, then validate our plan.
    /// A duplicate of the response we already validated means the responder
    /// never received our single-shot ack: re-send the stored ack VERBATIM,
    /// and never re-validate — each side runs `validate_upgrade_as` exactly
    /// once per peer, so the shared replay cache never sees a nonce twice.
    /// `via` is the delivering member (possibly a relay); the counterparty
    /// is the response's SIGNED responder.
    pub(crate) fn on_response(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        response: TunnelUpgradeResponse,
    ) -> Result<(), UpgradeError> {
        let sender = response.fields.responder_identity;
        if response.verify_signature().is_err() {
            self.fail_peer(state, via, "response signature invalid");
            return Ok(());
        }
        if !state.set.contains(sender) {
            self.fail_peer(state, via, "response from a non-member responder");
            return Ok(());
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
                self.fan_msg(state, &ReachabilityMsg::Ack(ack));
                return Ok(());
            }
            // no handshake with this member AT ALL: an authentically-signed
            // response can still reach a node that never sent its request —
            // the relay ring re-offers a completed exchange's latest half at
            // whoever might be missing it, and a node that re-assembled
            // mid-epoch (a restart) holds nothing from its previous life.
            // Stale, not hostile: drop quietly — the record paths already
            // carry the re-join, and failing the signer here would blame a
            // healthy peer once per re-offer for the rest of the epoch.
            None => {
                tracing::debug!(
                    target: "ducktape::reachability",
                    peer = %short(sender), epoch = state.epoch,
                    "response dropped: no handshake with this member in this life"
                );
                return Ok(());
            }
            // a mismatching message over LIVE handshake state is a re-sign
            // the protocol never does — loud, like every mismatch.
            Some(_) => {
                self.fail_peer(state, sender, "unsolicited handshake response");
                return Ok(());
            }
        };
        if response.fields.request_hash != request.hash() {
            self.fail_peer(state, sender, "response does not match our request");
            return Ok(());
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
                self.fan_msg(state, &ReachabilityMsg::Ack(ack));
                self.advance(state)
            }
            Err(err) => {
                self.settle_failed_handshake(state, sender, err);
                self.advance(state)
            }
        }
    }

    /// Responder side: the initiator acked — validate our plan. A duplicate
    /// of the ack that already completed this handshake is dropped without
    /// re-validation (see `on_response` for the replay argument). `via` is
    /// the delivering member (possibly a relay); the counterparty is the
    /// ack's SIGNED initiator.
    pub(crate) fn on_ack(
        &mut self,
        state: &mut EpochState,
        via: ValidatorIdentity,
        ack: TunnelUpgradeAck,
    ) -> Result<(), UpgradeError> {
        let sender = ack.fields.initiator_identity;
        if ack.verify_signature().is_err() {
            self.fail_peer(state, via, "ack signature invalid");
            return Ok(());
        }
        if !state.set.contains(sender) {
            self.fail_peer(state, via, "ack from a non-member initiator");
            return Ok(());
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
            // no handshake with this member AT ALL — the stale-relay shape
            // `on_response` documents: a re-assembled node receiving the
            // relay ring's re-offer of its previous life's completed
            // exchange. Drop quietly.
            None => {
                tracing::debug!(
                    target: "ducktape::reachability",
                    peer = %short(sender), epoch = state.epoch,
                    "ack dropped: no handshake with this member in this life"
                );
                return Ok(());
            }
            // a mismatching message over LIVE handshake state is a re-sign
            // the protocol never does — loud, like every mismatch.
            Some(_) => {
                self.fail_peer(state, sender, "unsolicited handshake ack");
                return Ok(());
            }
        };
        let pinned_triple = ack.fields.request_hash == request.hash()
            && ack.fields.response_hash == response.hash();
        if !pinned_triple {
            self.fail_peer(state, sender, "ack does not match the handshake");
            return Ok(());
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
                self.advance(state)
            }
            Err(err) => {
                self.settle_failed_handshake(state, sender, err);
                self.advance(state)
            }
        }
    }

    /// A triple that failed validation settles the peer as failed for the
    /// epoch: its nonces are burnt in the replay cache, so no retry can
    /// revive the pair, and the apply gate counts it as done.
    fn settle_failed_handshake(
        &mut self,
        state: &mut EpochState,
        peer: ValidatorIdentity,
        err: UpgradeError,
    ) {
        state.handshakes.remove(&peer);
        state.failed.insert(peer);
        self.fail_peer(state, peer, &format!("handshake validation: {err:?}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
