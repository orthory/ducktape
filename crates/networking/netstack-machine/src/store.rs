//! The persisted-mesh codec: the pure half of cold-restart persistence. The
//! machine ENCODES the mesh worth remembering (its [`crate::Effect::Persist`]
//! carries these bytes) and DECODES-and-verifies the bytes the host hands
//! back at boot; where those bytes live — the file, the atomic write — is
//! the host's domain (`reachability::store`).
//!
//! Adverts rather than plans deliberately: `TunnelInstallPlan` is mintable
//! only by `validate_upgrade_as` (an invariant this codec must not weaken),
//! and in ULA-overlay mode everything a tunnel needs re-derives from the
//! records — peer WireGuard keys and endpoints live inside them, overlay
//! addresses are pure functions of `(chain_id, identity)`. The signed advert
//! set is the mesh's own canonical, tamper-evident form: [`decode_verified`]
//! re-verifies every owner signature, so a corrupted or hand-edited file is
//! refused rather than applied.

use serde::{Deserialize, Serialize};
use wireguard::{EndpointAdvertisement, SignedEndpointRecord};

/// The persisted mesh: every member's signed advertisement from the last
/// epoch this node applied tunnels for (this node's own included), plus the
/// post-lock member re-advertisements and the standby records accepted by
/// then. no format version (flag-day rule):
/// `deny_unknown_fields` plus the required-field set IS the schema guard —
/// the restore is best-effort, and a file this build cannot parse just means
/// one boot without it, then the next apply rewrites the current form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedMesh {
    pub chain_id: String,
    pub epoch: u64,
    pub adverts: Vec<EndpointAdvertisement>,
    /// Post-lock member re-advertisements accepted by the persisting epoch:
    /// a member's fresh life, signed after the epoch locked its mesh. On
    /// restore, the higher record nonce per member wins between these and
    /// the adverts — the fresh life's tunnel parts are the ones worth
    /// re-applying.
    pub member_records: Vec<SignedEndpointRecord>,
    /// The pre-warm layer's accepted standby records (member side). These
    /// persist because a parked resident cannot re-introduce itself to a
    /// member that forgot its WireGuard key: its invite token was consumed
    /// at admission and every transport it has left rides the overlay this
    /// very snapshot re-establishes.
    pub standby_records: Vec<SignedEndpointRecord>,
}

impl PersistedMesh {
    pub fn new(
        chain_id: String,
        epoch: u64,
        adverts: Vec<EndpointAdvertisement>,
        member_records: Vec<SignedEndpointRecord>,
        standby_records: Vec<SignedEndpointRecord>,
    ) -> Self {
        Self {
            chain_id,
            epoch,
            adverts,
            member_records,
            standby_records,
        }
    }
}

/// Why persisted-mesh BYTES were refused, independent of where they came
/// from. The path-based host store wraps these with the file's path; the
/// machine's restore reports them as-is.
#[derive(Debug, thiserror::Error)]
pub enum MeshDecodeError {
    #[error("{0}")]
    Codec(#[from] serde_json::Error),
    #[error("chain id {found:?} does not match {expected:?}")]
    ChainMismatch { found: String, expected: String },
    #[error("persisted signature invalid")]
    BadSignature,
}

/// Encode `mesh` to the exact bytes the host store writes.
pub fn encode(mesh: &PersistedMesh) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec_pretty(mesh)
}

/// Decode and verify persisted-mesh bytes. Refuses — rather than degrades
/// on — an unparseable schema, a chain mismatch, and any advert or record
/// whose owner signature fails to verify: the caller treats every refusal
/// as "no restore" and says why.
pub fn decode_verified(bytes: &[u8], chain_id: &str) -> Result<PersistedMesh, MeshDecodeError> {
    let mesh: PersistedMesh = serde_json::from_slice(bytes)?;
    if mesh.chain_id != chain_id {
        return Err(MeshDecodeError::ChainMismatch {
            found: mesh.chain_id,
            expected: chain_id.to_string(),
        });
    }
    for advert in &mesh.adverts {
        if advert.verify_signature().is_err() {
            return Err(MeshDecodeError::BadSignature);
        }
    }
    for record in mesh.member_records.iter().chain(&mesh.standby_records) {
        if record.verify().is_err() {
            return Err(MeshDecodeError::BadSignature);
        }
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
    use std::net::{IpAddr, Ipv4Addr};
    use wireguard::{
        AdmissionRoot, Endpoint, EndpointRecord, MeshVersion, PortPolicy, Root, Transport,
        ValidatorIdentity, X25519PublicKey,
    };

    fn record_of(seed: u64, octet: u8) -> (EndpointRecord, PrivateKey) {
        let policy = PortPolicy::production();
        let signer = PrivateKey::from_seed(seed);
        let endpoint = |port: u16, transport| {
            Endpoint::new(
                IpAddr::V4(Ipv4Addr::new(8, 8, 8, octet)),
                port,
                transport,
                &policy,
            )
            .unwrap()
        };
        let record = EndpointRecord {
            namespace: "net#machine-store".into(),
            epoch: 3,
            valset_root: Root([1; 32]),
            admission_root: AdmissionRoot([2; 32]),
            validator_identity: ValidatorIdentity::try_from(signer.public_key().as_ref()).unwrap(),
            wireguard_public_key: X25519PublicKey([octet; 32]),
            control_endpoint: endpoint(443, Transport::Tcp),
            wireguard_endpoint: Some(endpoint(51820, Transport::Udp)),
            nonce: 1,
        };
        (record, signer)
    }

    fn sample() -> PersistedMesh {
        let advert = |seed, octet| {
            let (record, signer) = record_of(seed, octet);
            EndpointAdvertisement::sign(record, MeshVersion([7; 32]), &signer)
        };
        let signed = |seed, octet| {
            let (record, signer) = record_of(seed, octet);
            SignedEndpointRecord::sign(record, &signer)
        };
        PersistedMesh::new(
            "net#machine-store".into(),
            3,
            vec![advert(1, 10), advert(2, 20)],
            vec![signed(4, 40)],
            vec![signed(3, 30)],
        )
    }

    #[test]
    fn bytes_round_trip_through_the_codec() {
        let mesh = sample();
        let bytes = encode(&mesh).unwrap();
        assert_eq!(decode_verified(&bytes, "net#machine-store").unwrap(), mesh);
    }

    #[test]
    fn a_tampered_record_and_a_wrong_chain_are_refused() {
        let text = String::from_utf8(encode(&sample()).unwrap()).unwrap();

        // flipping one record's endpoint breaks its owner signature.
        let tampered = text.replace("8.8.8.40", "8.8.8.41");
        assert!(matches!(
            decode_verified(tampered.as_bytes(), "net#machine-store"),
            Err(MeshDecodeError::BadSignature)
        ));

        assert!(matches!(
            decode_verified(text.as_bytes(), "net#other"),
            Err(MeshDecodeError::ChainMismatch { .. })
        ));

        // a key this build does not know is refused outright, never
        // partially adopted (the flag-day schema guard).
        let unknown = text.replacen('{', "{\n  \"format\": 2,", 1);
        assert!(matches!(
            decode_verified(unknown.as_bytes(), "net#machine-store"),
            Err(MeshDecodeError::Codec(_))
        ));
    }
}
