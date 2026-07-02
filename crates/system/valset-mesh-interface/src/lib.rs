//! deterministic mesh control-plane types derived from valset membership.
//!
//! The mesh model is validator-owned: every control participant, bootnode, and
//! relay candidate is a validator in the current valset projection. This crate
//! does not configure WireGuard devices or perform key exchange; it only defines
//! the stable contract future adapters can consume.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const VALIDATOR_IDENTITY_LEN: usize = 32;

/// The valset epoch or cutover version this mesh projection belongs to.
pub type MeshEpoch = u64;

/// A deterministic content version for a [`MeshView`].
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshVersion(pub [u8; 32]);

/// A validator identity as stored by the valset module.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValidatorIdentity(pub Vec<u8>);

impl ValidatorIdentity {
    pub fn new(bytes: Vec<u8>) -> Result<Self, MeshDeriveError> {
        validate_identity(&bytes)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Endpoints a validator advertises for mesh control and data traffic.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MeshEndpoints {
    pub control: String,
    pub data: String,
}

impl MeshEndpoints {
    pub fn new(control: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            control: control.into(),
            data: data.into(),
        }
    }
}

/// One valset member plus its mesh endpoint advertisement.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ValidatorMeshAdvertisement {
    pub identity: Vec<u8>,
    pub endpoints: MeshEndpoints,
}

impl ValidatorMeshAdvertisement {
    pub fn new(identity: Vec<u8>, endpoints: MeshEndpoints) -> Self {
        Self {
            identity,
            endpoints,
        }
    }
}

/// Capabilities derived for a validator in a mesh view.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshCapabilities {
    pub control_participant: bool,
    pub data_participant: bool,
    pub bootnode: bool,
    pub relay: bool,
}

impl MeshCapabilities {
    fn validator_owned() -> Self {
        Self {
            control_participant: true,
            data_participant: true,
            bootnode: true,
            relay: true,
        }
    }
}

/// A validator's deterministic role assignment in one mesh view.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MeshValidator {
    pub stable_index: u64,
    pub identity: ValidatorIdentity,
    pub endpoints: MeshEndpoints,
    pub capabilities: MeshCapabilities,
}

/// The complete mesh projection for one valset epoch.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MeshView {
    pub epoch: MeshEpoch,
    pub version: MeshVersion,
    pub validators: Vec<MeshValidator>,
}

impl MeshView {
    pub fn bootnodes(&self) -> impl Iterator<Item = &MeshValidator> {
        self.validators.iter().filter(|v| v.capabilities.bootnode)
    }

    pub fn relay_candidates(&self) -> impl Iterator<Item = &MeshValidator> {
        self.validators.iter().filter(|v| v.capabilities.relay)
    }

    pub fn requires_external_relay(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshDeriveError {
    EmptyValidatorSet,
    InvalidIdentityLength {
        len: usize,
    },
    DuplicateValidator {
        identity: Vec<u8>,
    },
    MissingEndpoint {
        identity: Vec<u8>,
    },
    EmptyEndpoint {
        identity: Vec<u8>,
        field: &'static str,
    },
}

impl fmt::Display for MeshDeriveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyValidatorSet => write!(f, "validator mesh requires at least one validator"),
            Self::InvalidIdentityLength { len } => write!(
                f,
                "validator identity must be {VALIDATOR_IDENTITY_LEN} bytes, got {len}"
            ),
            Self::DuplicateValidator { identity } => {
                write!(f, "duplicate validator identity: {identity:?}")
            }
            Self::MissingEndpoint { identity } => {
                write!(
                    f,
                    "missing mesh endpoint advertisement for validator {identity:?}"
                )
            }
            Self::EmptyEndpoint { identity, field } => {
                write!(f, "empty {field} endpoint for validator {identity:?}")
            }
        }
    }
}

impl std::error::Error for MeshDeriveError {}

/// Derive a deterministic mesh view from valset membership and endpoint
/// advertisements. Input order is deliberately ignored; validator identity
/// byte-order defines the stable order.
pub fn derive_mesh(
    epoch: MeshEpoch,
    advertisements: impl IntoIterator<Item = ValidatorMeshAdvertisement>,
) -> Result<MeshView, MeshDeriveError> {
    let mut by_identity: BTreeMap<ValidatorIdentity, MeshEndpoints> = BTreeMap::new();
    for advertisement in advertisements {
        validate_identity(&advertisement.identity)?;
        validate_endpoints(&advertisement.identity, &advertisement.endpoints)?;
        let identity = ValidatorIdentity(advertisement.identity.clone());
        if by_identity
            .insert(identity, advertisement.endpoints)
            .is_some()
        {
            return Err(MeshDeriveError::DuplicateValidator {
                identity: advertisement.identity,
            });
        }
    }

    if by_identity.is_empty() {
        return Err(MeshDeriveError::EmptyValidatorSet);
    }

    let validators: Vec<MeshValidator> = by_identity
        .into_iter()
        .enumerate()
        .map(|(idx, (identity, endpoints))| MeshValidator {
            stable_index: idx as u64,
            identity,
            endpoints,
            capabilities: MeshCapabilities::validator_owned(),
        })
        .collect();
    let version = version_for(epoch, &validators);
    Ok(MeshView {
        epoch,
        version,
        validators,
    })
}

/// Derive a mesh view from the valset module's sorted membership reply plus a
/// deterministic endpoint lookup owned by the caller.
pub fn derive_mesh_from_valset<F>(
    epoch: MeshEpoch,
    validators: impl IntoIterator<Item = Vec<u8>>,
    mut endpoint_for: F,
) -> Result<MeshView, MeshDeriveError>
where
    F: FnMut(&[u8]) -> Option<MeshEndpoints>,
{
    let mut advertisements = Vec::new();
    for identity in validators {
        validate_identity(&identity)?;
        let endpoints =
            endpoint_for(&identity).ok_or_else(|| MeshDeriveError::MissingEndpoint {
                identity: identity.clone(),
            })?;
        advertisements.push(ValidatorMeshAdvertisement::new(identity, endpoints));
    }
    derive_mesh(epoch, advertisements)
}

pub fn encode_view(view: &MeshView) -> Vec<u8> {
    serde_json::to_vec(view).expect("serializable")
}

pub fn decode_view(bytes: &[u8]) -> Result<MeshView, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

fn validate_identity(identity: &[u8]) -> Result<(), MeshDeriveError> {
    if identity.len() != VALIDATOR_IDENTITY_LEN {
        return Err(MeshDeriveError::InvalidIdentityLength {
            len: identity.len(),
        });
    }
    Ok(())
}

fn validate_endpoints(identity: &[u8], endpoints: &MeshEndpoints) -> Result<(), MeshDeriveError> {
    if endpoints.control.trim().is_empty() {
        return Err(MeshDeriveError::EmptyEndpoint {
            identity: identity.to_vec(),
            field: "control",
        });
    }
    if endpoints.data.trim().is_empty() {
        return Err(MeshDeriveError::EmptyEndpoint {
            identity: identity.to_vec(),
            field: "data",
        });
    }
    Ok(())
}

fn version_for(epoch: MeshEpoch, validators: &[MeshValidator]) -> MeshVersion {
    let mut h = Sha256::new();
    h.update(b"ducktape:valset-mesh-interface:v1");
    h.update(epoch.to_le_bytes());
    h.update((validators.len() as u64).to_le_bytes());
    for validator in validators {
        h.update(validator.stable_index.to_le_bytes());
        update_bytes(&mut h, validator.identity.as_bytes());
        update_str(&mut h, &validator.endpoints.control);
        update_str(&mut h, &validator.endpoints.data);
        h.update([
            validator.capabilities.control_participant as u8,
            validator.capabilities.data_participant as u8,
            validator.capabilities.bootnode as u8,
            validator.capabilities.relay as u8,
        ]);
    }
    MeshVersion(h.finalize().into())
}

fn update_str(h: &mut Sha256, value: &str) {
    update_bytes(h, value.as_bytes());
}

fn update_bytes(h: &mut Sha256, value: &[u8]) {
    h.update((value.len() as u64).to_le_bytes());
    h.update(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> Vec<u8> {
        vec![byte; VALIDATOR_IDENTITY_LEN]
    }

    fn endpoints(byte: u8) -> MeshEndpoints {
        MeshEndpoints::new(
            format!("tcp://10.0.0.{byte}:7000"),
            format!("mesh://100.64.0.{byte}:51820"),
        )
    }

    fn endpoint_map(keys: &[Vec<u8>]) -> BTreeMap<Vec<u8>, MeshEndpoints> {
        keys.iter()
            .map(|key| (key.clone(), endpoints(key[0])))
            .collect()
    }

    fn identities(view: &MeshView) -> Vec<Vec<u8>> {
        view.validators
            .iter()
            .map(|v| v.identity.0.clone())
            .collect()
    }

    #[test]
    fn validators_become_control_bootnode_and_relay_participants() {
        let keys = vec![key(3), key(1), key(2)];
        let endpoints = endpoint_map(&keys);
        let view = derive_mesh_from_valset(7, keys.clone(), |id| endpoints.get(id).cloned())
            .expect("mesh view");

        assert_eq!(view.epoch, 7);
        assert_eq!(view.validators.len(), 3);
        for validator in &view.validators {
            assert!(keys.contains(&validator.identity.0));
            assert!(validator.capabilities.control_participant);
            assert!(validator.capabilities.data_participant);
            assert!(validator.capabilities.bootnode);
            assert!(validator.capabilities.relay);
            assert_eq!(
                validator.endpoints,
                endpoints
                    .get(validator.identity.as_bytes())
                    .unwrap()
                    .clone()
            );
        }
    }

    #[test]
    fn stable_ordering_does_not_depend_on_insertion_order() {
        let ascending = vec![key(1), key(2), key(3)];
        let descending = vec![key(3), key(2), key(1)];
        let endpoints = endpoint_map(&ascending);

        let a = derive_mesh_from_valset(11, ascending.clone(), |id| endpoints.get(id).cloned())
            .expect("ascending mesh");
        let b = derive_mesh_from_valset(11, descending, |id| endpoints.get(id).cloned())
            .expect("descending mesh");

        assert_eq!(identities(&a), ascending);
        assert_eq!(a, b);
        assert_eq!(
            a.validators
                .iter()
                .map(|v| v.stable_index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn membership_changes_move_the_mesh_version() {
        let base = vec![key(1), key(2)];
        let changed = vec![key(1), key(2), key(3)];
        let endpoints = endpoint_map(&changed);

        let before = derive_mesh_from_valset(22, base, |id| endpoints.get(id).cloned())
            .expect("before mesh");
        let after = derive_mesh_from_valset(22, changed, |id| endpoints.get(id).cloned())
            .expect("after mesh");

        assert_ne!(before.version, after.version);
    }

    #[test]
    fn relay_candidates_are_members_not_external_relays() {
        let keys = vec![key(4), key(5), key(6)];
        let endpoints = endpoint_map(&keys);
        let view = derive_mesh_from_valset(30, keys.clone(), |id| endpoints.get(id).cloned())
            .expect("mesh view");

        let relays: Vec<Vec<u8>> = view
            .relay_candidates()
            .map(|v| v.identity.0.clone())
            .collect();
        let bootnodes: Vec<Vec<u8>> = view.bootnodes().map(|v| v.identity.0.clone()).collect();

        assert!(!view.requires_external_relay());
        assert_eq!(relays, identities(&view));
        assert_eq!(bootnodes, identities(&view));
        assert!(relays.iter().all(|identity| keys.contains(identity)));
    }
}
