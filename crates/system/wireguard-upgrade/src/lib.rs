//! Validator-set backed WireGuard tunnel-upgrade protocol.
//!
//! This crate owns the security boundary before any node installs a WireGuard
//! peer. It verifies the active validator set, endpoint advertisements, port
//! policy, overlay routes, replay nonces, and the request/response/ack handshake.
//! A successful validation yields a [`TunnelInstallPlan`] that can be converted
//! into defguard `wireguard-rs` peer/interface configuration and handed to
//! `WGApi::<Userspace|Kernel>::configure_interface` by the effectful node layer.

use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryFrom;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer, Verifier, ed25519};
use defguard_wireguard_rs::{
    InterfaceConfiguration, key::Key as DefguardKey, net::IpAddrMask, peer::Peer as DefguardPeer,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const ENDPOINT_NS: &[u8] = b"ducktape:wireguard-endpoint:v1";
const UPGRADE_REQUEST_NS: &[u8] = b"ducktape:wireguard-upgrade-request:v1";
const UPGRADE_RESPONSE_NS: &[u8] = b"ducktape:wireguard-upgrade-response:v1";
const UPGRADE_ACK_NS: &[u8] = b"ducktape:wireguard-upgrade-ack:v1";
const MAX_ACK_INSTALL_LAG: u64 = 8;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UpgradeError {
    #[error("invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },
    #[error("empty active validator set")]
    EmptyValidatorSet,
    #[error("duplicate active validator")]
    DuplicateValidator,
    #[error("unknown validator")]
    UnknownValidator,
    #[error("missing validator advertisement")]
    MissingAdvertisement,
    #[error("stale duplicate advertisement")]
    StaleDuplicateAdvertisement,
    #[error("expired message or advertisement")]
    Expired,
    #[error("invalid endpoint: {0}")]
    InvalidEndpoint(String),
    #[error("port policy mismatch")]
    PortPolicyMismatch,
    #[error("mesh version mismatch")]
    MeshVersionMismatch,
    #[error("signature verification failed")]
    BadSignature,
    #[error("hash mismatch")]
    HashMismatch,
    #[error("handshake field mismatch")]
    HandshakeMismatch,
    #[error("ack view is stale or from the future")]
    BadAckView,
    #[error("replayed signed message")]
    Replay,
    #[error("invalid allowed IP route")]
    InvalidAllowedIp,
    #[error("invalid relay candidate")]
    InvalidRelay,
    #[error("invalid WireGuard key")]
    InvalidWireGuardKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Root(pub [u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MeshVersion(pub [u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PolicyHash(pub [u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ValidatorIdentity(pub [u8; 32]);

impl TryFrom<&[u8]> for ValidatorIdentity {
    type Error = UpgradeError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != 32 {
            return Err(UpgradeError::InvalidKeyLength {
                expected: 32,
                actual: value.len(),
            });
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(value);
        Ok(Self(out))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct X25519PublicKey(pub [u8; 32]);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SignatureBytes(pub Vec<u8>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Transport {
    Tcp,
    Udp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Endpoint {
    pub addr: IpAddr,
    pub port: u16,
    pub transport: Transport,
}

impl Endpoint {
    pub fn parse(
        input: &str,
        transport: Transport,
        policy: &PortPolicy,
    ) -> Result<Self, UpgradeError> {
        let socket = SocketAddr::from_str(input).map_err(|_| {
            UpgradeError::InvalidEndpoint(
                "endpoint must be a canonical IP literal with port".into(),
            )
        })?;
        Self::new(socket.ip(), socket.port(), transport, policy)
    }

    pub fn new(
        addr: IpAddr,
        port: u16,
        transport: Transport,
        policy: &PortPolicy,
    ) -> Result<Self, UpgradeError> {
        let endpoint = Self {
            addr: normalize_ip(addr),
            port,
            transport,
        };
        policy.check_endpoint(&endpoint)?;
        Ok(endpoint)
    }

    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.addr, self.port)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortPolicy {
    pub name: String,
    pub allowed_control_tcp_ports: Vec<u16>,
    pub allowed_wireguard_udp_ports: Vec<u16>,
    pub allow_loopback: bool,
    pub allow_private_ip: bool,
}

impl PortPolicy {
    pub fn production() -> Self {
        Self {
            name: "production".into(),
            allowed_control_tcp_ports: vec![443],
            allowed_wireguard_udp_ports: vec![51820],
            allow_loopback: false,
            allow_private_ip: false,
        }
    }

    pub fn hash(&self) -> PolicyHash {
        let mut out = Vec::new();
        put_str(&mut out, &self.name);
        put_u16_vec(&mut out, &self.allowed_control_tcp_ports);
        put_u16_vec(&mut out, &self.allowed_wireguard_udp_ports);
        out.push(self.allow_loopback as u8);
        out.push(self.allow_private_ip as u8);
        PolicyHash(hash32(&out))
    }

    fn check_endpoint(&self, endpoint: &Endpoint) -> Result<(), UpgradeError> {
        if endpoint.port == 0 {
            return Err(UpgradeError::InvalidEndpoint("port 0 is forbidden".into()));
        }
        match endpoint.transport {
            Transport::Tcp if !self.allowed_control_tcp_ports.contains(&endpoint.port) => {
                return Err(UpgradeError::InvalidEndpoint(
                    "control TCP port is not allowlisted".into(),
                ));
            }
            Transport::Udp if !self.allowed_wireguard_udp_ports.contains(&endpoint.port) => {
                return Err(UpgradeError::InvalidEndpoint(
                    "WireGuard UDP port is not allowlisted".into(),
                ));
            }
            _ => {}
        }
        check_ip_policy(endpoint.addr, self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MeshCapability {
    Bootnode,
    Relay,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveValidatorSet {
    pub namespace: String,
    pub epoch: u64,
    pub valset_root: Root,
    validators: Vec<ValidatorIdentity>,
}

impl ActiveValidatorSet {
    pub fn new(
        namespace: impl Into<String>,
        epoch: u64,
        valset_root: Root,
        validators: Vec<ValidatorIdentity>,
    ) -> Result<Self, UpgradeError> {
        if validators.is_empty() {
            return Err(UpgradeError::EmptyValidatorSet);
        }
        let mut sorted = validators;
        sorted.sort();
        if sorted.windows(2).any(|w| w[0] == w[1]) {
            return Err(UpgradeError::DuplicateValidator);
        }
        Ok(Self {
            namespace: namespace.into(),
            epoch,
            valset_root,
            validators: sorted,
        })
    }

    pub fn contains(&self, identity: ValidatorIdentity) -> bool {
        self.validators.binary_search(&identity).is_ok()
    }

    pub fn stable_index(&self, identity: ValidatorIdentity) -> Option<usize> {
        self.validators.binary_search(&identity).ok()
    }

    pub fn validators(&self) -> &[ValidatorIdentity] {
        &self.validators
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointRecord {
    pub namespace: String,
    pub epoch: u64,
    pub valset_root: Root,
    pub validator_identity: ValidatorIdentity,
    pub control_endpoint: Endpoint,
    pub wireguard_endpoint: Endpoint,
    pub capabilities: Vec<MeshCapability>,
    pub expires_at_view: u64,
    pub nonce: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointAdvertisement {
    pub record: EndpointRecord,
    pub mesh_version: MeshVersion,
    pub signature: SignatureBytes,
}

impl EndpointAdvertisement {
    pub fn sign(
        record: EndpointRecord,
        mesh_version: MeshVersion,
        signer: &ed25519::PrivateKey,
    ) -> Self {
        let mut msg = Vec::new();
        put_endpoint_ad_without_signature(&mut msg, &record, mesh_version);
        let signature = signer.sign(ENDPOINT_NS, &msg);
        Self {
            record,
            mesh_version,
            signature: signature_bytes(&signature),
        }
    }

    fn verify_signature(&self) -> Result<(), UpgradeError> {
        let mut msg = Vec::new();
        put_endpoint_ad_without_signature(&mut msg, &self.record, self.mesh_version);
        verify_ed25519(
            self.record.validator_identity,
            ENDPOINT_NS,
            &msg,
            &self.signature,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshView {
    pub active_set: ActiveValidatorSet,
    pub mesh_version: MeshVersion,
    pub records: Vec<EndpointRecord>,
}

impl MeshView {
    pub fn verify(
        active_set: ActiveValidatorSet,
        advertisements: Vec<EndpointAdvertisement>,
        policy: &PortPolicy,
        current_view: u64,
    ) -> Result<Self, UpgradeError> {
        let mut selected: BTreeMap<ValidatorIdentity, EndpointAdvertisement> = BTreeMap::new();
        for ad in advertisements {
            let record = &ad.record;
            if record.namespace != active_set.namespace
                || record.epoch != active_set.epoch
                || record.valset_root != active_set.valset_root
                || !active_set.contains(record.validator_identity)
            {
                return Err(UpgradeError::UnknownValidator);
            }
            if current_view > record.expires_at_view {
                return Err(UpgradeError::Expired);
            }
            policy.check_endpoint(&record.control_endpoint)?;
            policy.check_endpoint(&record.wireguard_endpoint)?;
            match selected.get(&record.validator_identity) {
                Some(prev) if record.nonce <= prev.record.nonce => {
                    return Err(UpgradeError::StaleDuplicateAdvertisement);
                }
                _ => {
                    selected.insert(record.validator_identity, ad);
                }
            }
        }
        if selected.len() != active_set.validators.len() {
            return Err(UpgradeError::MissingAdvertisement);
        }
        let records: Vec<EndpointRecord> = active_set
            .validators
            .iter()
            .map(|id| {
                selected
                    .get(id)
                    .expect("len already checked")
                    .record
                    .clone()
            })
            .collect();
        let mesh_version = compute_mesh_version(&records)?;
        for ad in selected.values() {
            if ad.mesh_version != mesh_version {
                return Err(UpgradeError::MeshVersionMismatch);
            }
            ad.verify_signature()?;
        }
        Ok(Self {
            active_set,
            mesh_version,
            records,
        })
    }

    pub fn stable_index(&self, identity: ValidatorIdentity) -> Option<usize> {
        self.active_set.stable_index(identity)
    }

    pub fn record(&self, identity: ValidatorIdentity) -> Option<&EndpointRecord> {
        self.records
            .iter()
            .find(|r| r.validator_identity == identity)
    }

    pub fn relay_candidates(&self) -> Vec<ValidatorIdentity> {
        self.records
            .iter()
            .filter(|r| r.capabilities.contains(&MeshCapability::Relay))
            .map(|r| r.validator_identity)
            .collect()
    }
}

pub fn compute_mesh_version(records: &[EndpointRecord]) -> Result<MeshVersion, UpgradeError> {
    if records.is_empty() {
        return Err(UpgradeError::MissingAdvertisement);
    }
    let mut hashes: Vec<[u8; 32]> = records
        .iter()
        .map(|record| {
            let mut out = Vec::new();
            put_endpoint_record(&mut out, record);
            hash32(&out)
        })
        .collect();
    hashes.sort();
    let mut out = Vec::new();
    put_str(&mut out, "ducktape:validator-mesh-version:v1");
    for hash in hashes {
        put_fixed(&mut out, &hash);
    }
    Ok(MeshVersion(hash32(&out)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AllowedIp {
    pub addr: IpAddr,
    pub cidr: u8,
}

impl AllowedIp {
    pub fn new(addr: IpAddr, cidr: u8) -> Result<Self, UpgradeError> {
        match addr {
            IpAddr::V4(_) if cidr <= 32 => Ok(Self { addr, cidr }),
            IpAddr::V6(_) if cidr <= 128 => Ok(Self { addr, cidr }),
            _ => Err(UpgradeError::InvalidAllowedIp),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayPolicy {
    base_v4: Ipv4Addr,
    prefix_v4: u8,
}

impl OverlayPolicy {
    pub fn default_v4() -> Self {
        Self {
            base_v4: Ipv4Addr::new(100, 64, 0, 0),
            prefix_v4: 16,
        }
    }

    pub fn allowed_ips_for(
        &self,
        view: &MeshView,
        identity: ValidatorIdentity,
    ) -> Result<Vec<AllowedIp>, UpgradeError> {
        let index = view
            .stable_index(identity)
            .ok_or(UpgradeError::UnknownValidator)? as u32;
        let base = u32::from(self.base_v4);
        let host_count = 1u32 << (32 - self.prefix_v4 as u32);
        let offset = index + 1;
        if offset >= host_count {
            return Err(UpgradeError::InvalidAllowedIp);
        }
        Ok(vec![AllowedIp {
            addr: IpAddr::V4(Ipv4Addr::from(base + offset)),
            cidr: 32,
        }])
    }

    fn validate_for(
        &self,
        view: &MeshView,
        identity: ValidatorIdentity,
        routes: &[AllowedIp],
    ) -> Result<(), UpgradeError> {
        let canonical = self.allowed_ips_for(view, identity)?;
        if routes == canonical.as_slice() {
            return Ok(());
        }
        if routes.is_empty() {
            return Err(UpgradeError::InvalidAllowedIp);
        }
        for route in routes {
            reject_stealing_route(*route)?;
            if !canonical.contains(route) {
                return Err(UpgradeError::InvalidAllowedIp);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelUpgradeRequestFields {
    pub namespace: String,
    pub epoch: u64,
    pub valset_root: Root,
    pub mesh_version: MeshVersion,
    pub initiator_identity: ValidatorIdentity,
    pub responder_identity: ValidatorIdentity,
    pub initiator_wireguard_public_key: X25519PublicKey,
    pub initiator_wireguard_endpoint: Endpoint,
    pub requested_allowed_ips: Vec<AllowedIp>,
    pub port_policy_hash: PolicyHash,
    pub expires_at_view: u64,
    pub nonce: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelUpgradeRequest {
    pub fields: TunnelUpgradeRequestFields,
    pub signature: SignatureBytes,
}

impl TunnelUpgradeRequest {
    pub fn sign(fields: TunnelUpgradeRequestFields, signer: &ed25519::PrivateKey) -> Self {
        let mut msg = Vec::new();
        put_request_fields(&mut msg, &fields);
        let signature = signer.sign(UPGRADE_REQUEST_NS, &msg);
        Self {
            fields,
            signature: signature_bytes(&signature),
        }
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut out = Vec::new();
        put_request_fields(&mut out, &self.fields);
        put_signature(&mut out, &self.signature);
        hash32(&out)
    }

    fn verify_signature(&self) -> Result<(), UpgradeError> {
        let mut msg = Vec::new();
        put_request_fields(&mut msg, &self.fields);
        verify_ed25519(
            self.fields.initiator_identity,
            UPGRADE_REQUEST_NS,
            &msg,
            &self.signature,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelUpgradeResponseFields {
    pub request_hash: [u8; 32],
    pub namespace: String,
    pub epoch: u64,
    pub valset_root: Root,
    pub mesh_version: MeshVersion,
    pub responder_identity: ValidatorIdentity,
    pub initiator_identity: ValidatorIdentity,
    pub responder_wireguard_public_key: X25519PublicKey,
    pub responder_wireguard_endpoint: Endpoint,
    pub accepted_allowed_ips: Vec<AllowedIp>,
    pub relay_candidates: Vec<ValidatorIdentity>,
    pub keepalive_seconds: Option<u16>,
    pub expires_at_view: u64,
    pub nonce: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelUpgradeResponse {
    pub fields: TunnelUpgradeResponseFields,
    pub signature: SignatureBytes,
}

impl TunnelUpgradeResponse {
    pub fn sign(fields: TunnelUpgradeResponseFields, signer: &ed25519::PrivateKey) -> Self {
        let mut msg = Vec::new();
        put_response_fields(&mut msg, &fields);
        let signature = signer.sign(UPGRADE_RESPONSE_NS, &msg);
        Self {
            fields,
            signature: signature_bytes(&signature),
        }
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut out = Vec::new();
        put_response_fields(&mut out, &self.fields);
        put_signature(&mut out, &self.signature);
        hash32(&out)
    }

    fn verify_signature(&self) -> Result<(), UpgradeError> {
        let mut msg = Vec::new();
        put_response_fields(&mut msg, &self.fields);
        verify_ed25519(
            self.fields.responder_identity,
            UPGRADE_RESPONSE_NS,
            &msg,
            &self.signature,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelUpgradeAckFields {
    pub request_hash: [u8; 32],
    pub response_hash: [u8; 32],
    pub namespace: String,
    pub epoch: u64,
    pub valset_root: Root,
    pub mesh_version: MeshVersion,
    pub initiator_identity: ValidatorIdentity,
    pub responder_identity: ValidatorIdentity,
    pub installed_at_view: u64,
    pub expires_at_view: u64,
    pub nonce: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelUpgradeAck {
    pub fields: TunnelUpgradeAckFields,
    pub signature: SignatureBytes,
}

impl TunnelUpgradeAck {
    pub fn sign(fields: TunnelUpgradeAckFields, signer: &ed25519::PrivateKey) -> Self {
        let mut msg = Vec::new();
        put_ack_fields(&mut msg, &fields);
        let signature = signer.sign(UPGRADE_ACK_NS, &msg);
        Self {
            fields,
            signature: signature_bytes(&signature),
        }
    }

    fn verify_signature(&self) -> Result<(), UpgradeError> {
        let mut msg = Vec::new();
        put_ack_fields(&mut msg, &self.fields);
        verify_ed25519(
            self.fields.initiator_identity,
            UPGRADE_ACK_NS,
            &msg,
            &self.signature,
        )
    }
}

#[derive(Default, Clone, Debug)]
pub struct ReplayCache {
    seen: BTreeSet<(ValidatorIdentity, u64, u64)>,
}

impl ReplayCache {
    fn check(
        &self,
        identity: ValidatorIdentity,
        epoch: u64,
        nonce: u64,
    ) -> Result<(), UpgradeError> {
        if self.seen.contains(&(identity, epoch, nonce)) {
            Err(UpgradeError::Replay)
        } else {
            Ok(())
        }
    }

    fn insert(&mut self, identity: ValidatorIdentity, epoch: u64, nonce: u64) {
        self.seen.insert((identity, epoch, nonce));
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TunnelInstallPlan {
    pub local_identity: ValidatorIdentity,
    pub peer_identity: ValidatorIdentity,
    pub local_wireguard_public_key: X25519PublicKey,
    pub peer_wireguard_public_key: X25519PublicKey,
    pub peer_endpoint: Endpoint,
    pub local_interface_ips: Vec<AllowedIp>,
    pub allowed_ips: Vec<AllowedIp>,
    pub relay_candidates: Vec<ValidatorIdentity>,
    pub keepalive_seconds: Option<u16>,
}

#[allow(clippy::too_many_arguments)]
pub fn validate_upgrade(
    view: &MeshView,
    policy: &PortPolicy,
    overlay: &OverlayPolicy,
    current_view: u64,
    request: &TunnelUpgradeRequest,
    response: &TunnelUpgradeResponse,
    ack: &TunnelUpgradeAck,
    replay: &mut ReplayCache,
) -> Result<TunnelInstallPlan, UpgradeError> {
    request.verify_signature()?;
    response.verify_signature()?;
    ack.verify_signature()?;

    let rq = &request.fields;
    let rs = &response.fields;
    let ak = &ack.fields;
    let root = view.active_set.valset_root;
    if rq.request_tuple()
        != (
            view.active_set.namespace.as_str(),
            view.active_set.epoch,
            root,
            view.mesh_version,
        )
        || rs.request_hash != request.hash()
        || rs.request_tuple()
            != (
                view.active_set.namespace.as_str(),
                view.active_set.epoch,
                root,
                view.mesh_version,
            )
        || ak.request_hash != request.hash()
        || ak.response_hash != response.hash()
        || ak.request_tuple()
            != (
                view.active_set.namespace.as_str(),
                view.active_set.epoch,
                root,
                view.mesh_version,
            )
    {
        return Err(UpgradeError::HandshakeMismatch);
    }
    if rq.port_policy_hash != policy.hash() {
        return Err(UpgradeError::PortPolicyMismatch);
    }
    if rq.initiator_identity != rs.initiator_identity
        || rq.initiator_identity != ak.initiator_identity
        || rq.responder_identity != rs.responder_identity
        || rq.responder_identity != ak.responder_identity
    {
        return Err(UpgradeError::HandshakeMismatch);
    }
    let initiator_record = view
        .record(rq.initiator_identity)
        .ok_or(UpgradeError::UnknownValidator)?;
    let responder_record = view
        .record(rq.responder_identity)
        .ok_or(UpgradeError::UnknownValidator)?;
    if rq.initiator_wireguard_endpoint != initiator_record.wireguard_endpoint
        || rs.responder_wireguard_endpoint != responder_record.wireguard_endpoint
    {
        return Err(UpgradeError::HandshakeMismatch);
    }
    ensure_x25519(rq.initiator_wireguard_public_key)?;
    ensure_x25519(rs.responder_wireguard_public_key)?;
    policy.check_endpoint(&rq.initiator_wireguard_endpoint)?;
    policy.check_endpoint(&rs.responder_wireguard_endpoint)?;
    if current_view > rq.expires_at_view
        || current_view > rs.expires_at_view
        || current_view > ak.expires_at_view
    {
        return Err(UpgradeError::Expired);
    }
    if ak.installed_at_view > current_view
        || current_view.saturating_sub(ak.installed_at_view) > MAX_ACK_INSTALL_LAG
    {
        return Err(UpgradeError::BadAckView);
    }
    overlay.validate_for(view, rq.responder_identity, &rq.requested_allowed_ips)?;
    overlay.validate_for(view, rq.initiator_identity, &rs.accepted_allowed_ips)?;
    for relay in &rs.relay_candidates {
        let record = view.record(*relay).ok_or(UpgradeError::InvalidRelay)?;
        if !record.capabilities.contains(&MeshCapability::Relay) {
            return Err(UpgradeError::InvalidRelay);
        }
    }
    replay.check(rq.initiator_identity, rq.epoch, rq.nonce)?;
    replay.check(rs.responder_identity, rs.epoch, rs.nonce)?;
    replay.check(ak.initiator_identity, ak.epoch, ak.nonce)?;

    replay.insert(rq.initiator_identity, rq.epoch, rq.nonce);
    replay.insert(rs.responder_identity, rs.epoch, rs.nonce);
    replay.insert(ak.initiator_identity, ak.epoch, ak.nonce);

    Ok(TunnelInstallPlan {
        local_identity: rq.initiator_identity,
        peer_identity: rq.responder_identity,
        local_wireguard_public_key: rq.initiator_wireguard_public_key,
        peer_wireguard_public_key: rs.responder_wireguard_public_key,
        peer_endpoint: rs.responder_wireguard_endpoint,
        local_interface_ips: rs.accepted_allowed_ips.clone(),
        allowed_ips: rq.requested_allowed_ips.clone(),
        relay_candidates: rs.relay_candidates.clone(),
        keepalive_seconds: rs.keepalive_seconds,
    })
}

trait CommonRequestFields {
    fn request_tuple(&self) -> (&str, u64, Root, MeshVersion);
}

impl CommonRequestFields for TunnelUpgradeRequestFields {
    fn request_tuple(&self) -> (&str, u64, Root, MeshVersion) {
        (
            &self.namespace,
            self.epoch,
            self.valset_root,
            self.mesh_version,
        )
    }
}

impl CommonRequestFields for TunnelUpgradeResponseFields {
    fn request_tuple(&self) -> (&str, u64, Root, MeshVersion) {
        (
            &self.namespace,
            self.epoch,
            self.valset_root,
            self.mesh_version,
        )
    }
}

impl CommonRequestFields for TunnelUpgradeAckFields {
    fn request_tuple(&self) -> (&str, u64, Root, MeshVersion) {
        (
            &self.namespace,
            self.epoch,
            self.valset_root,
            self.mesh_version,
        )
    }
}

#[derive(Clone, Debug)]
pub struct DefguardPeerConfig {
    pub peer: DefguardPeer,
    pub allowed_ips: Vec<AllowedIp>,
}

impl DefguardPeerConfig {
    pub fn from_plan(plan: &TunnelInstallPlan) -> Self {
        let mut peer = DefguardPeer::new(DefguardKey::new(plan.peer_wireguard_public_key.0));
        peer.endpoint = Some(plan.peer_endpoint.socket_addr());
        peer.persistent_keepalive_interval = plan.keepalive_seconds;
        peer.set_allowed_ips(to_defguard_allowed_ips(&plan.allowed_ips));
        Self {
            peer,
            allowed_ips: plan.allowed_ips.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DefguardInterfaceConfig {
    pub config: InterfaceConfiguration,
}

impl DefguardInterfaceConfig {
    pub fn from_plan(
        name: impl Into<String>,
        private_key_base64: impl Into<String>,
        listen_endpoint: Endpoint,
        plans: Vec<TunnelInstallPlan>,
    ) -> Self {
        let mut addresses = Vec::new();
        let mut seen = BTreeSet::new();
        for plan in &plans {
            for route in &plan.local_interface_ips {
                if seen.insert((route.addr, route.cidr)) {
                    addresses.push(IpAddrMask::new(route.addr, route.cidr));
                }
            }
        }
        let peers = plans
            .iter()
            .map(DefguardPeerConfig::from_plan)
            .map(|cfg| cfg.peer)
            .collect();
        Self {
            config: InterfaceConfiguration {
                name: name.into(),
                prvkey: private_key_base64.into(),
                addresses,
                port: listen_endpoint.port,
                peers,
                mtu: None,
                fwmark: None,
            },
        }
    }
}

fn to_defguard_allowed_ips(routes: &[AllowedIp]) -> Vec<IpAddrMask> {
    routes
        .iter()
        .map(|route| IpAddrMask::new(route.addr, route.cidr))
        .collect()
}

fn ensure_x25519(key: X25519PublicKey) -> Result<(), UpgradeError> {
    if key.0 == [0u8; 32] {
        return Err(UpgradeError::InvalidWireGuardKey);
    }
    Ok(())
}

fn verify_ed25519(
    identity: ValidatorIdentity,
    namespace: &[u8],
    msg: &[u8],
    signature: &SignatureBytes,
) -> Result<(), UpgradeError> {
    let public =
        ed25519::PublicKey::decode(&identity.0[..]).map_err(|_| UpgradeError::BadSignature)?;
    if signature.0.len() != 64 {
        return Err(UpgradeError::BadSignature);
    }
    let sig =
        ed25519::Signature::decode(&signature.0[..]).map_err(|_| UpgradeError::BadSignature)?;
    if public.verify(namespace, msg, &sig) {
        Ok(())
    } else {
        Err(UpgradeError::BadSignature)
    }
}

fn signature_bytes(signature: &ed25519::Signature) -> SignatureBytes {
    SignatureBytes(signature.as_ref().to_vec())
}

fn normalize_ip(addr: IpAddr) -> IpAddr {
    match addr {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(v6),
        },
        other => other,
    }
}

fn check_ip_policy(addr: IpAddr, policy: &PortPolicy) -> Result<(), UpgradeError> {
    match addr {
        IpAddr::V4(ip) => {
            if ip.is_unspecified()
                || ip.is_broadcast()
                || is_v4_documentation(ip)
                || ip.is_multicast()
            {
                return Err(UpgradeError::InvalidEndpoint(
                    "non-global IPv4 address".into(),
                ));
            }
            if ip.is_loopback() && !policy.allow_loopback {
                return Err(UpgradeError::InvalidEndpoint(
                    "loopback endpoint is forbidden".into(),
                ));
            }
            if (ip.is_private() || ip.is_link_local()) && !policy.allow_private_ip {
                return Err(UpgradeError::InvalidEndpoint(
                    "private IPv4 endpoint is forbidden".into(),
                ));
            }
        }
        IpAddr::V6(ip) => {
            if ip.is_unspecified() || ip.is_multicast() || is_v6_documentation(ip) {
                return Err(UpgradeError::InvalidEndpoint(
                    "non-global IPv6 address".into(),
                ));
            }
            if ip.is_loopback() && !policy.allow_loopback {
                return Err(UpgradeError::InvalidEndpoint(
                    "loopback endpoint is forbidden".into(),
                ));
            }
            if (is_v6_unique_local(ip) || is_v6_unicast_link_local(ip)) && !policy.allow_private_ip
            {
                return Err(UpgradeError::InvalidEndpoint(
                    "private IPv6 endpoint is forbidden".into(),
                ));
            }
        }
    }
    Ok(())
}

fn is_v4_documentation(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 192 && o[1] == 0 && o[2] == 2
        || o[0] == 198 && o[1] == 51 && o[2] == 100
        || o[0] == 203 && o[1] == 0 && o[2] == 113
}

fn is_v6_documentation(ip: Ipv6Addr) -> bool {
    ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8
}

fn is_v6_unique_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_v6_unicast_link_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn reject_stealing_route(route: AllowedIp) -> Result<(), UpgradeError> {
    match route.addr {
        IpAddr::V4(_) if route.cidr == 0 => Err(UpgradeError::InvalidAllowedIp),
        IpAddr::V6(_) if route.cidr == 0 => Err(UpgradeError::InvalidAllowedIp),
        _ => Ok(()),
    }
}

fn hash32(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn put_endpoint_record(out: &mut Vec<u8>, record: &EndpointRecord) {
    put_str(out, &record.namespace);
    put_u64(out, record.epoch);
    put_root(out, record.valset_root);
    put_identity(out, record.validator_identity);
    put_endpoint(out, record.control_endpoint);
    put_endpoint(out, record.wireguard_endpoint);
    put_capabilities(out, &record.capabilities);
    put_u64(out, record.expires_at_view);
    put_u64(out, record.nonce);
}

fn put_endpoint_ad_without_signature(
    out: &mut Vec<u8>,
    record: &EndpointRecord,
    mesh_version: MeshVersion,
) {
    put_endpoint_record(out, record);
    put_mesh_version(out, mesh_version);
}

fn put_request_fields(out: &mut Vec<u8>, fields: &TunnelUpgradeRequestFields) {
    put_str(out, &fields.namespace);
    put_u64(out, fields.epoch);
    put_root(out, fields.valset_root);
    put_mesh_version(out, fields.mesh_version);
    put_identity(out, fields.initiator_identity);
    put_identity(out, fields.responder_identity);
    put_x25519(out, fields.initiator_wireguard_public_key);
    put_endpoint(out, fields.initiator_wireguard_endpoint);
    put_allowed_ips(out, &fields.requested_allowed_ips);
    put_policy_hash(out, fields.port_policy_hash);
    put_u64(out, fields.expires_at_view);
    put_u64(out, fields.nonce);
}

fn put_response_fields(out: &mut Vec<u8>, fields: &TunnelUpgradeResponseFields) {
    put_fixed(out, &fields.request_hash);
    put_str(out, &fields.namespace);
    put_u64(out, fields.epoch);
    put_root(out, fields.valset_root);
    put_mesh_version(out, fields.mesh_version);
    put_identity(out, fields.responder_identity);
    put_identity(out, fields.initiator_identity);
    put_x25519(out, fields.responder_wireguard_public_key);
    put_endpoint(out, fields.responder_wireguard_endpoint);
    put_allowed_ips(out, &fields.accepted_allowed_ips);
    put_identities(out, &fields.relay_candidates);
    match fields.keepalive_seconds {
        Some(v) => {
            out.push(1);
            put_u16(out, v);
        }
        None => out.push(0),
    }
    put_u64(out, fields.expires_at_view);
    put_u64(out, fields.nonce);
}

fn put_ack_fields(out: &mut Vec<u8>, fields: &TunnelUpgradeAckFields) {
    put_fixed(out, &fields.request_hash);
    put_fixed(out, &fields.response_hash);
    put_str(out, &fields.namespace);
    put_u64(out, fields.epoch);
    put_root(out, fields.valset_root);
    put_mesh_version(out, fields.mesh_version);
    put_identity(out, fields.initiator_identity);
    put_identity(out, fields.responder_identity);
    put_u64(out, fields.installed_at_view);
    put_u64(out, fields.expires_at_view);
    put_u64(out, fields.nonce);
}

fn put_str(out: &mut Vec<u8>, value: &str) {
    put_bytes(out, value.as_bytes());
}

fn put_bytes(out: &mut Vec<u8>, value: &[u8]) {
    put_u64(out, value.len() as u64);
    out.extend_from_slice(value);
}

fn put_fixed(out: &mut Vec<u8>, value: &[u8; 32]) {
    out.extend_from_slice(value);
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u16_vec(out: &mut Vec<u8>, values: &[u16]) {
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted.dedup();
    put_u64(out, sorted.len() as u64);
    for value in sorted {
        put_u16(out, value);
    }
}

fn put_root(out: &mut Vec<u8>, value: Root) {
    out.extend_from_slice(&value.0);
}

fn put_mesh_version(out: &mut Vec<u8>, value: MeshVersion) {
    out.extend_from_slice(&value.0);
}

fn put_policy_hash(out: &mut Vec<u8>, value: PolicyHash) {
    out.extend_from_slice(&value.0);
}

fn put_identity(out: &mut Vec<u8>, value: ValidatorIdentity) {
    out.extend_from_slice(&value.0);
}

fn put_x25519(out: &mut Vec<u8>, value: X25519PublicKey) {
    out.extend_from_slice(&value.0);
}

fn put_signature(out: &mut Vec<u8>, value: &SignatureBytes) {
    put_bytes(out, &value.0);
}

fn put_endpoint(out: &mut Vec<u8>, endpoint: Endpoint) {
    out.push(match endpoint.transport {
        Transport::Tcp => 1,
        Transport::Udp => 2,
    });
    match endpoint.addr {
        IpAddr::V4(ip) => {
            out.push(4);
            out.extend_from_slice(&ip.octets());
        }
        IpAddr::V6(ip) => {
            out.push(6);
            out.extend_from_slice(&ip.octets());
        }
    }
    put_u16(out, endpoint.port);
}

fn put_capabilities(out: &mut Vec<u8>, values: &[MeshCapability]) {
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted.dedup();
    put_u64(out, sorted.len() as u64);
    for value in sorted {
        out.push(match value {
            MeshCapability::Bootnode => 1,
            MeshCapability::Relay => 2,
        });
    }
}

fn put_allowed_ips(out: &mut Vec<u8>, routes: &[AllowedIp]) {
    let mut sorted = routes.to_vec();
    sorted.sort();
    sorted.dedup();
    put_u64(out, sorted.len() as u64);
    for route in sorted {
        match route.addr {
            IpAddr::V4(ip) => {
                out.push(4);
                out.extend_from_slice(&ip.octets());
            }
            IpAddr::V6(ip) => {
                out.push(6);
                out.extend_from_slice(&ip.octets());
            }
        }
        out.push(route.cidr);
    }
}

fn put_identities(out: &mut Vec<u8>, values: &[ValidatorIdentity]) {
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted.dedup();
    put_u64(out, sorted.len() as u64);
    for value in sorted {
        put_identity(out, value);
    }
}
