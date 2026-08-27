//! Cold-restart persistence for the verified mesh: the last epoch whose
//! tunnels APPLIED, remembered as its full advertisement set. A NATed member
//! restarting with zero TCP links (ingress gone, tunnels torn down at exit)
//! has no path for plane gossip — and a whole-network cold start is the same
//! brick — so the node re-applies THIS remembered mesh at boot, with fresh
//! coordinator-resolved endpoints, purely to carry the next epoch's gossip.
//!
//! Adverts rather than plans deliberately: `TunnelInstallPlan` is mintable
//! only by `validate_upgrade_as` (an invariant this file must not weaken),
//! and in ULA-overlay mode everything a tunnel needs re-derives from the
//! records — peer WireGuard keys and endpoints live inside them, overlay
//! addresses are pure functions of `(chain_id, identity)`. The signed advert
//! set is the mesh's own canonical, tamper-evident form: `load` re-verifies
//! every owner signature, so a corrupted or hand-edited file is refused
//! rather than applied.

use std::path::Path;

use serde::{Deserialize, Serialize};
use wireguard::{EndpointAdvertisement, SignedEndpointRecord};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("mesh state file {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("mesh state file {path}: {source}")]
    Codec {
        path: String,
        source: serde_json::Error,
    },
    #[error("mesh state file {path}: chain id {found:?} does not match {expected:?}")]
    ChainMismatch {
        path: String,
        found: String,
        expected: String,
    },
    #[error("mesh state file {path}: persisted signature invalid")]
    BadSignature { path: String },
}

/// The persisted mesh: every member's signed advertisement from the last
/// epoch this node applied tunnels for (this node's own included), plus the
/// post-lock member re-advertisements and the standby records accepted by
/// then. no format version (flag-day rule):
/// `deny_unknown_fields` plus the required-field set IS the schema guard —
/// the restore is best-effort, and a file this build cannot parse just means
/// one boot without it, then `save` rewrites the current form.
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
    /// very file re-establishes.
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

/// Write `mesh` to `path` atomically (temp file + rename in the same
/// directory), so a crash mid-write can never leave a half-written file
/// where the next boot's restore would find it.
pub fn save(path: &Path, mesh: &PersistedMesh) -> Result<(), StoreError> {
    let io = |source| StoreError::Io {
        path: path.display().to_string(),
        source,
    };
    let bytes = serde_json::to_vec_pretty(mesh).map_err(|source| StoreError::Codec {
        path: path.display().to_string(),
        source,
    })?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &bytes).map_err(io)?;
    std::fs::rename(&tmp, path).map_err(io)
}

/// Read the mesh persisted at `path`; `Ok(None)` when no file exists (a
/// first boot). Refuses — rather than degrades on — an unparseable schema, a
/// chain mismatch, and any advert whose owner signature fails to verify: the
/// caller treats every refusal as "no restore" and says why.
pub fn load(path: &Path, chain_id: &str) -> Result<Option<PersistedMesh>, StoreError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(StoreError::Io {
                path: path.display().to_string(),
                source,
            });
        }
    };
    let mesh: PersistedMesh =
        serde_json::from_slice(&bytes).map_err(|source| StoreError::Codec {
            path: path.display().to_string(),
            source,
        })?;
    if mesh.chain_id != chain_id {
        return Err(StoreError::ChainMismatch {
            path: path.display().to_string(),
            found: mesh.chain_id,
            expected: chain_id.to_string(),
        });
    }
    for advert in &mesh.adverts {
        if advert.verify_signature().is_err() {
            return Err(StoreError::BadSignature {
                path: path.display().to_string(),
            });
        }
    }
    for record in mesh.member_records.iter().chain(&mesh.standby_records) {
        if record.verify().is_err() {
            return Err(StoreError::BadSignature {
                path: path.display().to_string(),
            });
        }
    }
    Ok(Some(mesh))
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
            namespace: "net#store".into(),
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

    fn advert(seed: u64, octet: u8) -> EndpointAdvertisement {
        let (record, signer) = record_of(seed, octet);
        EndpointAdvertisement::sign(record, MeshVersion([7; 32]), &signer)
    }

    fn signed_record(seed: u64, octet: u8) -> SignedEndpointRecord {
        let (record, signer) = record_of(seed, octet);
        SignedEndpointRecord::sign(record, &signer)
    }

    fn sample() -> PersistedMesh {
        PersistedMesh::new(
            "net#store".into(),
            3,
            vec![advert(1, 10), advert(2, 20)],
            vec![signed_record(4, 40)],
            vec![signed_record(3, 30)],
        )
    }

    #[test]
    fn round_trips_and_absence_is_a_clean_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mesh-state.json");

        assert!(load(&path, "net#store").unwrap().is_none());

        let mesh = sample();
        save(&path, &mesh).unwrap();
        assert_eq!(load(&path, "net#store").unwrap(), Some(mesh));
    }

    #[test]
    fn refuses_a_tampered_advert() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mesh-state.json");
        save(&path, &sample()).unwrap();

        // flip the persisted epoch inside one record: the owner signature no
        // longer covers what the file claims.
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replace("\"epoch\": 3", "\"epoch\": 4")).unwrap();

        assert!(matches!(
            load(&path, "net#store"),
            Err(StoreError::BadSignature { .. })
        ));
    }

    #[test]
    fn refuses_a_tampered_member_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mesh-state.json");
        save(&path, &sample()).unwrap();

        // redirect the re-advertised member record's endpoint (its address
        // is unique to it in the file): the owner signature no longer
        // covers the claim.
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replace("8.8.8.40", "8.8.8.41")).unwrap();

        assert!(matches!(
            load(&path, "net#store"),
            Err(StoreError::BadSignature { .. })
        ));
    }

    #[test]
    fn refuses_a_tampered_standby_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mesh-state.json");
        save(&path, &sample()).unwrap();

        // redirect the standby record's endpoint (its address is unique to
        // it in the file): the owner signature no longer covers the claim.
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replace("8.8.8.30", "8.8.8.31")).unwrap();

        assert!(matches!(
            load(&path, "net#store"),
            Err(StoreError::BadSignature { .. })
        ));
    }

    #[test]
    fn refuses_the_wrong_chain_and_an_unknown_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mesh-state.json");
        save(&path, &sample()).unwrap();
        assert!(matches!(
            load(&path, "net#other"),
            Err(StoreError::ChainMismatch { .. })
        ));

        // a file carrying a key this build does not know is refused (one boot
        // without a restore), never partially adopted.
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replacen('{', "{\n  \"format\": 2,", 1)).unwrap();
        assert!(matches!(
            load(&path, "net#store"),
            Err(StoreError::Codec { .. })
        ));
    }

    #[test]
    fn refuses_garbage_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mesh-state.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(matches!(
            load(&path, "net#store"),
            Err(StoreError::Codec { .. })
        ));
    }
}
