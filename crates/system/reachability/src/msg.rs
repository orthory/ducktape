//! The reachability plane's control-mesh messages: what members exchange
//! over the node's dedicated reachability channel to assemble a `MeshView`
//! and run tunnel handshakes. Every payload is already ed25519-signed by
//! `wireguard-upgrade` (advertisements and the handshake triple) or feeds a
//! signed artifact (the pre-version `Record` gossip), and the channel itself
//! rides the key-authenticated commonware mesh — the codec adds transport
//! framing, not trust.
//!
//! serde_json deliberately: control-plane rate (a handful of messages per
//! epoch), human-debuggable on the wire, and the same codec the node's other
//! operator surfaces speak.

use serde::{Deserialize, Serialize};
use wireguard_upgrade::{
    EndpointAdvertisement, EndpointRecord, TunnelUpgradeAck, TunnelUpgradeRequest,
    TunnelUpgradeResponse,
};

#[derive(Debug, thiserror::Error)]
#[error("reachability message: {0}")]
pub struct MsgError(#[from] serde_json::Error);

/// One reachability-channel message.
///
/// `Record` exists because `mesh_version` is derived from EVERY member's
/// record: a member must see all records before it can compute the version
/// its signed advertisement commits to. So each epoch runs record gossip
/// first, then signed advertisements over the agreed set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReachabilityMsg {
    /// Pre-version gossip: this member's record for the current epoch.
    Record(EndpointRecord),
    /// The signed, mesh-versioned advertisement (`MeshView::verify` input).
    Advert(EndpointAdvertisement),
    /// Tunnel handshake, initiator -> responder.
    Request(TunnelUpgradeRequest),
    /// Tunnel handshake, responder -> initiator.
    Response(TunnelUpgradeResponse),
    /// Tunnel handshake close, initiator -> responder.
    Ack(TunnelUpgradeAck),
}

impl ReachabilityMsg {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("reachability messages always serialize")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, MsgError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use wireguard_upgrade::{
        AdmissionRoot, Endpoint, MeshCapability, PortPolicy, Root, Transport, ValidatorIdentity,
        X25519PublicKey,
    };

    #[test]
    fn round_trips_every_variant_shape() {
        let policy = PortPolicy::production();
        let endpoint = |octet: u8, port: u16, transport| {
            Endpoint::new(
                IpAddr::V4(Ipv4Addr::new(8, 8, 8, octet)),
                port,
                transport,
                &policy,
            )
            .unwrap()
        };
        let record = EndpointRecord {
            namespace: "net#1".into(),
            epoch: 7,
            valset_root: Root([1; 32]),
            admission_root: AdmissionRoot([2; 32]),
            validator_identity: ValidatorIdentity([3; 32]),
            wireguard_public_key: X25519PublicKey([4; 32]),
            control_endpoint: endpoint(10, 443, Transport::Tcp),
            wireguard_endpoint: endpoint(10, 51820, Transport::Udp),
            capabilities: vec![MeshCapability::Relay],
            expires_at_view: 50,
            nonce: 1,
        };
        let msg = ReachabilityMsg::Record(record);
        assert_eq!(ReachabilityMsg::decode(&msg.encode()).unwrap(), msg);

        assert!(ReachabilityMsg::decode(b"not json").is_err());
    }
}
