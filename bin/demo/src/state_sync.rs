//! transport-neutral state-sync request/response frames for the demo joiner path.
//!
//! The peer identity is the ed25519 public key carried by validator-set state.
//! A Tailscale-like WireGuard mesh can implement the same frames with endpoints
//! resolved from consensus-known validators that also act as bootnodes,
//! relayers, and control participants; this layer never assumes a static
//! external relay.

use std::collections::{BTreeMap, BTreeSet};

use commonware_codec::{Decode, Encode};
use sdk::{ModuleId, ROOT_LEN, StateRoot};
use serde::{Deserialize, Serialize};

const ED25519_PEER_ID_LEN: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateSyncError {
    InvalidPeerIdLen {
        got: usize,
    },
    UnknownParticipant(StateSyncPeerId),
    ParticipantCannotServe(StateSyncPeerId),
    UnknownModule {
        source: StateSyncPeerId,
        module_id: ModuleId,
    },
    RootMismatch {
        module_id: ModuleId,
        expected: StateRoot,
        actual: StateRoot,
    },
    KindMismatch {
        module_id: ModuleId,
        expected: StateSyncKind,
        actual: StateSyncKind,
    },
    Decode(String),
}

impl std::fmt::Display for StateSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPeerIdLen { got } => {
                write!(
                    f,
                    "invalid peer id length: expected {ED25519_PEER_ID_LEN}, got {got}"
                )
            }
            Self::UnknownParticipant(peer) => write!(f, "unknown mesh participant: {peer:?}"),
            Self::ParticipantCannotServe(peer) => {
                write!(f, "participant cannot serve state-sync: {peer:?}")
            }
            Self::UnknownModule { source, module_id } => {
                write!(f, "{source:?} does not serve module {module_id}")
            }
            Self::RootMismatch {
                module_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "{module_id} state-sync root mismatch: expected {expected:?}, actual {actual:?}"
                )
            }
            Self::KindMismatch {
                module_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "{module_id} state-sync kind mismatch: expected {expected:?}, actual {actual:?}"
                )
            }
            Self::Decode(err) => write!(f, "state-sync frame decode failed: {err}"),
        }
    }
}

impl std::error::Error for StateSyncError {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StateSyncPeerId(Vec<u8>);

impl StateSyncPeerId {
    pub fn ed25519_public_key(bytes: Vec<u8>) -> Result<Self, StateSyncError> {
        if bytes.len() != ED25519_PEER_ID_LEN {
            return Err(StateSyncError::InvalidPeerIdLen { got: bytes.len() });
        }
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshRole {
    Validator,
    Bootnode,
    Relayer,
    Control,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshParticipant {
    peer_id: StateSyncPeerId,
    roles: BTreeSet<MeshRole>,
}

impl MeshParticipant {
    pub fn with_roles(peer_id: StateSyncPeerId, roles: impl IntoIterator<Item = MeshRole>) -> Self {
        Self {
            peer_id,
            roles: roles.into_iter().collect(),
        }
    }

    pub fn validator_set_participant(peer_id: StateSyncPeerId) -> Self {
        Self::with_roles(
            peer_id,
            [
                MeshRole::Validator,
                MeshRole::Bootnode,
                MeshRole::Relayer,
                MeshRole::Control,
            ],
        )
    }

    pub fn peer_id(&self) -> &StateSyncPeerId {
        &self.peer_id
    }

    pub fn has_role(&self, role: MeshRole) -> bool {
        self.roles.contains(&role)
    }

    pub fn can_serve_state_sync(&self) -> bool {
        self.has_role(MeshRole::Validator)
            || self.has_role(MeshRole::Bootnode)
            || self.has_role(MeshRole::Relayer)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSyncRoot([u8; ROOT_LEN]);

impl From<StateRoot> for StateSyncRoot {
    fn from(root: StateRoot) -> Self {
        Self(root.0)
    }
}

impl From<StateSyncRoot> for StateRoot {
    fn from(root: StateSyncRoot) -> Self {
        Self(root.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateSyncKind {
    Snapshot,
    QmdbTarget,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateSyncPayload {
    Snapshot(Vec<u8>),
    QmdbTarget(Vec<u8>),
}

impl StateSyncPayload {
    pub fn kind(&self) -> StateSyncKind {
        match self {
            Self::Snapshot(_) => StateSyncKind::Snapshot,
            Self::QmdbTarget(_) => StateSyncKind::QmdbTarget,
        }
    }

    pub fn into_snapshot_bytes(self) -> Result<Vec<u8>, StateSyncError> {
        match self {
            Self::Snapshot(bytes) => Ok(bytes),
            other => Err(StateSyncError::KindMismatch {
                module_id: "<payload>".into(),
                expected: StateSyncKind::Snapshot,
                actual: other.kind(),
            }),
        }
    }

    pub fn into_qmdb_target_bytes(self) -> Result<Vec<u8>, StateSyncError> {
        match self {
            Self::QmdbTarget(bytes) => Ok(bytes),
            other => Err(StateSyncError::KindMismatch {
                module_id: "<payload>".into(),
                expected: StateSyncKind::QmdbTarget,
                actual: other.kind(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSyncRequest {
    pub requester: StateSyncPeerId,
    pub source: StateSyncPeerId,
    pub module_id: ModuleId,
    pub expected_root: StateSyncRoot,
    pub kind: StateSyncKind,
}

impl StateSyncRequest {
    pub fn new(
        requester: StateSyncPeerId,
        source: StateSyncPeerId,
        module_id: impl Into<ModuleId>,
        expected_root: StateRoot,
        kind: StateSyncKind,
    ) -> Self {
        Self {
            requester,
            source,
            module_id: module_id.into(),
            expected_root: expected_root.into(),
            kind,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSyncResponse {
    pub source: MeshParticipant,
    pub request: StateSyncRequest,
    pub payload: StateSyncPayload,
}

pub fn encode_request(request: &StateSyncRequest) -> Vec<u8> {
    serde_json::to_vec(request).expect("state-sync request is serializable")
}

pub fn decode_request(bytes: &[u8]) -> Result<StateSyncRequest, StateSyncError> {
    serde_json::from_slice(bytes).map_err(|e| StateSyncError::Decode(e.to_string()))
}

pub fn encode_response(response: &StateSyncResponse) -> Vec<u8> {
    serde_json::to_vec(response).expect("state-sync response is serializable")
}

pub fn decode_response(bytes: &[u8]) -> Result<StateSyncResponse, StateSyncError> {
    serde_json::from_slice(bytes).map_err(|e| StateSyncError::Decode(e.to_string()))
}

pub fn encode_qmdb_target<T>(target: &T) -> Vec<u8>
where
    T: Encode,
{
    target.encode().to_vec()
}

pub fn decode_qmdb_target<T>(bytes: &[u8]) -> Result<T, StateSyncError>
where
    T: Decode<Cfg = ()>,
{
    T::decode_cfg(bytes, &()).map_err(|e| StateSyncError::Decode(e.to_string()))
}

#[derive(Clone, Debug)]
struct ServedModule {
    root: StateRoot,
    payload: StateSyncPayload,
}

#[derive(Default)]
pub struct LoopbackStateSyncResolver {
    participants: BTreeMap<StateSyncPeerId, MeshParticipant>,
    modules: BTreeMap<(StateSyncPeerId, ModuleId), ServedModule>,
}

impl LoopbackStateSyncResolver {
    pub fn insert_participant(&mut self, participant: MeshParticipant) {
        self.participants
            .insert(participant.peer_id.clone(), participant);
    }

    pub fn serve_module(
        &mut self,
        peer_id: &StateSyncPeerId,
        module_id: impl Into<ModuleId>,
        root: StateRoot,
        payload: StateSyncPayload,
    ) -> Result<(), StateSyncError> {
        let participant = self
            .participants
            .get(peer_id)
            .ok_or_else(|| StateSyncError::UnknownParticipant(peer_id.clone()))?;
        if !participant.can_serve_state_sync() {
            return Err(StateSyncError::ParticipantCannotServe(peer_id.clone()));
        }
        self.modules.insert(
            (peer_id.clone(), module_id.into()),
            ServedModule { root, payload },
        );
        Ok(())
    }

    pub fn resolve(&self, request: StateSyncRequest) -> Result<StateSyncResponse, StateSyncError> {
        let participant = self
            .participants
            .get(&request.source)
            .ok_or_else(|| StateSyncError::UnknownParticipant(request.source.clone()))?;
        if !participant.can_serve_state_sync() {
            return Err(StateSyncError::ParticipantCannotServe(
                request.source.clone(),
            ));
        }

        let module = self
            .modules
            .get(&(request.source.clone(), request.module_id.clone()))
            .ok_or_else(|| StateSyncError::UnknownModule {
                source: request.source.clone(),
                module_id: request.module_id.clone(),
            })?;

        let expected = StateRoot::from(request.expected_root);
        if module.root != expected {
            return Err(StateSyncError::RootMismatch {
                module_id: request.module_id.clone(),
                expected,
                actual: module.root,
            });
        }
        if module.payload.kind() != request.kind {
            return Err(StateSyncError::KindMismatch {
                module_id: request.module_id.clone(),
                expected: request.kind,
                actual: module.payload.kind(),
            });
        }

        Ok(StateSyncResponse {
            source: participant.clone(),
            request,
            payload: module.payload.clone(),
        })
    }

    pub fn resolve_bytes(&self, request: &[u8]) -> Result<Vec<u8>, StateSyncError> {
        let request = decode_request(request)?;
        self.resolve(request)
            .map(|response| encode_response(&response))
    }
}
