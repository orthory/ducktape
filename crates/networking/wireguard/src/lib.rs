//! Validator-set backed WireGuard tunnel-upgrade protocol, plus the effect
//! layer that applies it.
//!
//! The crate root owns the security boundary before any node installs a
//! WireGuard peer. It verifies the active validator set, endpoint
//! advertisements, port policy, overlay routes, replay nonces, and the
//! request/response/ack handshake. A successful validation yields a
//! [`TunnelInstallPlan`]; the [`effect`] module converts plans into
//! peer/interface configuration (`defguard_wireguard_rs` types) and pushes
//! them through a `WireGuardEffect` (fake in tests, the in-process
//! userspace backend in production).

pub mod effect;

use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryFrom;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer, Verifier, ed25519};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// the signed record layout carries `wireguard_public_key`; a layout
// change under a signature domain always bumps the domain, so old and new
// blobs can never cross-verify.
const ENDPOINT_NS: &[u8] = b"ducktape:wireguard-endpoint:v1";
const ENDPOINT_RECORD_NS: &[u8] = b"ducktape:wireguard-endpoint-record:v1";
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
    #[error("missing admission root")]
    MissingAdmissionRoot,
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
    #[error("invalid WireGuard key")]
    InvalidWireGuardKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Root(pub [u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AdmissionRoot(pub [u8; 32]);

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
#[serde(rename_all = "snake_case")]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ActiveValidatorSet {
    pub namespace: String,
    pub epoch: u64,
    pub valset_root: Root,
    pub admission_root: AdmissionRoot,
    validators: Vec<ValidatorIdentity>,
}

impl ActiveValidatorSet {
    pub fn new(
        namespace: impl Into<String>,
        epoch: u64,
        valset_root: Root,
        admission_root: AdmissionRoot,
        validators: Vec<ValidatorIdentity>,
    ) -> Result<Self, UpgradeError> {
        if validators.is_empty() {
            return Err(UpgradeError::EmptyValidatorSet);
        }
        validate_admission_root(admission_root)?;
        let mut sorted = validators;
        sorted.sort();
        if sorted.windows(2).any(|w| w[0] == w[1]) {
            return Err(UpgradeError::DuplicateValidator);
        }
        Ok(Self {
            namespace: namespace.into(),
            epoch,
            valset_root,
            admission_root,
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
    pub admission_root: AdmissionRoot,
    pub validator_identity: ValidatorIdentity,
    /// The validator's WireGuard X25519 public key. Lives HERE — in the
    /// ed25519-signed, mesh-versioned advertisement — rather than in consensus
    /// state: rotation is a re-advertisement (a new mesh version), never a
    /// state schema change. `validate_upgrade_as` pins the tunnel handshake's
    /// keys to these records, so a coordinator or relay on the path cannot
    /// substitute its own key without breaking the record signature.
    pub wireguard_public_key: X25519PublicKey,
    pub control_endpoint: Endpoint,
    /// The member's dialable WireGuard UDP endpoint — `None` for a NAT'd
    /// member with no dialable underlay address (a joiner behind a home
    /// router). Peers install its tunnel WITHOUT an endpoint and wait: the
    /// endpoint-less side holds every peer's endpoint from these records, so
    /// it initiates, and WireGuard's roaming pins the observed source. On the
    /// wire `None` omits the field, keeping endpoint-ful records bit-identical
    /// to the pre-Option encoding (and old records decodable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wireguard_endpoint: Option<Endpoint>,
    pub expires_at_view: u64,
    pub nonce: u64,
}

impl EndpointRecord {
    /// The record-level validity checks [`MeshView::verify`] runs per
    /// member advertisement — endpoint policy on both endpoints, a real
    /// X25519 key, freshness — available standalone for records consumed
    /// OUTSIDE a verified view (a standby's pre-warm record, whose owner is
    /// not in the epoch's `ActiveValidatorSet` and therefore can never be
    /// part of a `MeshView`). The caller still owns signature verification
    /// and the epoch-tuple/membership binding.
    pub fn check(&self, policy: &PortPolicy, current_view: u64) -> Result<(), UpgradeError> {
        if current_view > self.expires_at_view {
            return Err(UpgradeError::Expired);
        }
        policy.check_endpoint(&self.control_endpoint)?;
        if let Some(wireguard_endpoint) = &self.wireguard_endpoint {
            policy.check_endpoint(wireguard_endpoint)?;
        }
        ensure_x25519(self.wireguard_public_key)
    }
}

/// An [`EndpointRecord`] signed by its own validator, for gossip paths where
/// the transport does not authenticate the record's OWNER — a record relayed
/// through a third member arrives on a link authenticated to the forwarder,
/// so the record itself must carry the owner's signature. Distinct signing
/// domain from [`EndpointAdvertisement`] (which additionally commits to a
/// mesh version): the two blobs can never cross-verify.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedEndpointRecord {
    pub record: EndpointRecord,
    pub signature: SignatureBytes,
}

impl SignedEndpointRecord {
    pub fn sign(record: EndpointRecord, signer: &ed25519::PrivateKey) -> Self {
        let mut msg = Vec::new();
        put_endpoint_record(&mut msg, &record);
        let signature = signer.sign(ENDPOINT_RECORD_NS, &msg);
        Self {
            record,
            signature: signature_bytes(&signature),
        }
    }

    /// Verify the owner signature: the signer is always
    /// `record.validator_identity` — a record is only ever self-signed.
    pub fn verify(&self) -> Result<(), UpgradeError> {
        let mut msg = Vec::new();
        put_endpoint_record(&mut msg, &self.record);
        verify_ed25519(
            self.record.validator_identity,
            ENDPOINT_RECORD_NS,
            &msg,
            &self.signature,
        )
    }
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

    pub fn verify_signature(&self) -> Result<(), UpgradeError> {
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
        validate_admission_root(active_set.admission_root)?;
        let mut selected: BTreeMap<ValidatorIdentity, EndpointAdvertisement> = BTreeMap::new();
        for ad in advertisements {
            let record = &ad.record;
            if record.namespace != active_set.namespace
                || record.epoch != active_set.epoch
                || record.valset_root != active_set.valset_root
                || record.admission_root != active_set.admission_root
                || !active_set.contains(record.validator_identity)
            {
                return Err(UpgradeError::UnknownValidator);
            }
            if current_view > record.expires_at_view {
                return Err(UpgradeError::Expired);
            }
            policy.check_endpoint(&record.control_endpoint)?;
            if let Some(wireguard_endpoint) = &record.wireguard_endpoint {
                policy.check_endpoint(wireguard_endpoint)?;
            }
            ensure_x25519(record.wireguard_public_key)?;
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
}

/// the documented preimage (docs/records/protocols/wireguard-tunnel-upgrade.md "Mesh
/// Version"): HASH(domain || namespace || epoch || valset_root ||
/// admission_root || SORT_ASC(endpoint_record_hashes)). this layout differs from the retired one
/// only in each record hash now covering `wireguard_public_key` (and the
/// bumped domain string). the epoch tuple is hashed at the TOP LEVEL — not
/// only inside each record hash — exactly as the doc specifies, so an
/// independent implementation working from the doc produces the same
/// version. records carrying mismatched tuples cannot version at all (a
/// mixed set is a protocol violation, never hashable).
pub fn compute_mesh_version(records: &[EndpointRecord]) -> Result<MeshVersion, UpgradeError> {
    let Some(first) = records.first() else {
        return Err(UpgradeError::MissingAdvertisement);
    };
    let tuple = (
        first.namespace.as_str(),
        first.epoch,
        first.valset_root,
        first.admission_root,
    );
    if records.iter().any(|r| {
        (
            r.namespace.as_str(),
            r.epoch,
            r.valset_root,
            r.admission_root,
        ) != tuple
    }) {
        return Err(UpgradeError::MeshVersionMismatch);
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
    put_str(&mut out, &first.namespace);
    put_u64(&mut out, first.epoch);
    put_root(&mut out, first.valset_root);
    put_admission_root(&mut out, first.admission_root);
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

/// The overlay addressing scheme: identity-hash /128s inside a chain-scoped
/// IPv6 ULA /48. an address is a function of (chain_id, identity) ONLY —
/// every node derives every peer's address without an allocator or index,
/// and it never moves when the valset reorders. fd00::/8 cannot collide with
/// RFC1918 v4 or the 100.64.0.0/10 CGNAT block a resident Tailscale
/// occupies, which is what lets a `dt-*` interface coexist with a personal
/// tailnet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayPolicy {
    chain_id: String,
}

/// The chain-scoped ULA /48 prefix: `fd` followed by the first 40 bits of
/// HASH("ducktape:overlay-ula:v1" || chain_id).
pub fn ula_v6_prefix(chain_id: &str) -> Ipv6Addr {
    let mut pre = Vec::new();
    put_str(&mut pre, "ducktape:overlay-ula:v1");
    put_str(&mut pre, chain_id);
    let h = hash32(&pre);
    let mut b = [0u8; 16];
    b[0] = 0xfd;
    b[1..6].copy_from_slice(&h[..5]);
    Ipv6Addr::from(b)
}

/// A member's overlay /128 inside the chain's ULA /48: the low 80 bits are
/// the first 80 bits of HASH("ducktape:overlay-addr:v1" || chain_id ||
/// identity). Deterministic from public inputs, so the whole mesh agrees on
/// every member's address with no coordination.
pub fn ula_v6_member_addr(chain_id: &str, identity: ValidatorIdentity) -> Ipv6Addr {
    let prefix = ula_v6_prefix(chain_id).octets();
    let mut pre = Vec::new();
    put_str(&mut pre, "ducktape:overlay-addr:v1");
    put_str(&mut pre, chain_id);
    put_identity(&mut pre, identity);
    let h = hash32(&pre);
    let mut b = [0u8; 16];
    b[..6].copy_from_slice(&prefix[..6]);
    b[6..16].copy_from_slice(&h[..10]);
    Ipv6Addr::from(b)
}

impl OverlayPolicy {
    /// The node-driven WireGuard overlay: identity-hash /128s in the chain's
    /// ULA /48 (see [`ula_v6_prefix`] / [`ula_v6_member_addr`]). `chain_id`
    /// must be the same string the mesh uses as its advertisement namespace.
    pub fn ula_v6(chain_id: impl Into<String>) -> Self {
        Self {
            chain_id: chain_id.into(),
        }
    }

    pub fn allowed_ips_for(
        &self,
        view: &MeshView,
        identity: ValidatorIdentity,
    ) -> Result<Vec<AllowedIp>, UpgradeError> {
        // membership gate: an overlay address exists only for a validator of
        // this view, even though the ULA derivation would happily hash any
        // identity.
        view.stable_index(identity)
            .ok_or(UpgradeError::UnknownValidator)?;
        Ok(self.identity_allowed_ips(identity))
    }

    /// View-free [`OverlayPolicy::allowed_ips_for`] — the address is a pure
    /// function of identity. Exists for re-deriving a PREVIOUSLY validated
    /// mesh from persisted records at boot, where no live `MeshView` can
    /// exist yet — the caller owns the membership gate the view would
    /// otherwise supply.
    pub fn identity_allowed_ips(&self, identity: ValidatorIdentity) -> Vec<AllowedIp> {
        vec![AllowedIp {
            addr: IpAddr::V6(ula_v6_member_addr(&self.chain_id, identity)),
            cidr: 128,
        }]
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
        for (i, route) in routes.iter().enumerate() {
            reject_stealing_route(*route)?;
            if !canonical.contains(route) {
                return Err(UpgradeError::InvalidAllowedIp);
            }
            // Reject a duplicate-bearing vector. The signed preimage sorts+dedups
            // (`put_allowed_ips`), so a signature still verifies over a vector that
            // repeats a canonical route many times — but the effect layer
            // materializes EVERY entry into the WireGuard peer config. Without this
            // an admitted validator could sign ONE request that inflates a peer's
            // allowed-ips into a memory/CPU DoS. A legitimate sender advertises a
            // set (a subset of the canonical routes), never a repeat, so bounding
            // `routes` to distinct canonical entries caps it at `canonical.len()`.
            if routes[..i].contains(route) {
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
    pub admission_root: AdmissionRoot,
    pub mesh_version: MeshVersion,
    pub initiator_identity: ValidatorIdentity,
    pub responder_identity: ValidatorIdentity,
    pub initiator_wireguard_public_key: X25519PublicKey,
    /// `None` when the initiator advertises no WireGuard endpoint (NAT'd,
    /// endpoint-less); must equal the initiator's record either way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initiator_wireguard_endpoint: Option<Endpoint>,
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

    pub fn verify_signature(&self) -> Result<(), UpgradeError> {
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
    pub admission_root: AdmissionRoot,
    pub mesh_version: MeshVersion,
    pub responder_identity: ValidatorIdentity,
    pub initiator_identity: ValidatorIdentity,
    pub responder_wireguard_public_key: X25519PublicKey,
    /// `None` when the responder advertises no WireGuard endpoint (NAT'd,
    /// endpoint-less); must equal the responder's record either way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responder_wireguard_endpoint: Option<Endpoint>,
    pub accepted_allowed_ips: Vec<AllowedIp>,
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

    pub fn verify_signature(&self) -> Result<(), UpgradeError> {
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
    pub admission_root: AdmissionRoot,
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

    pub fn verify_signature(&self) -> Result<(), UpgradeError> {
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

    fn check_batch(
        &self,
        keys: &[(ValidatorIdentity, u64, u64)],
    ) -> Result<BTreeSet<(ValidatorIdentity, u64, u64)>, UpgradeError> {
        let mut pending = BTreeSet::new();
        for (identity, epoch, nonce) in keys {
            if !pending.insert((*identity, *epoch, *nonce)) {
                return Err(UpgradeError::Replay);
            }
            self.check(*identity, *epoch, *nonce)?;
        }
        Ok(pending)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TunnelInstallContext {
    pub namespace: String,
    pub epoch: u64,
    pub valset_root: Root,
    pub admission_root: AdmissionRoot,
    pub mesh_version: MeshVersion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TunnelInstallPlan {
    context: TunnelInstallContext,
    local_identity: ValidatorIdentity,
    peer_identity: ValidatorIdentity,
    local_wireguard_public_key: X25519PublicKey,
    peer_wireguard_public_key: X25519PublicKey,
    peer_endpoint: Option<Endpoint>,
    local_interface_ips: Vec<AllowedIp>,
    allowed_ips: Vec<AllowedIp>,
    keepalive_seconds: Option<u16>,
}

impl TunnelInstallPlan {
    pub fn context(&self) -> &TunnelInstallContext {
        &self.context
    }

    pub fn local_identity(&self) -> ValidatorIdentity {
        self.local_identity
    }

    pub fn peer_identity(&self) -> ValidatorIdentity {
        self.peer_identity
    }

    pub fn local_wireguard_public_key(&self) -> X25519PublicKey {
        self.local_wireguard_public_key
    }

    pub fn peer_wireguard_public_key(&self) -> X25519PublicKey {
        self.peer_wireguard_public_key
    }

    pub fn peer_endpoint(&self) -> Option<Endpoint> {
        self.peer_endpoint
    }

    pub fn local_interface_ips(&self) -> &[AllowedIp] {
        &self.local_interface_ips
    }

    pub fn allowed_ips(&self) -> &[AllowedIp] {
        &self.allowed_ips
    }

    pub fn keepalive_seconds(&self) -> Option<u16> {
        self.keepalive_seconds
    }
}

/// Which side of a validated handshake a [`TunnelInstallPlan`] is built for.
/// [`validate_upgrade_as`] lets either party derive its OWN install plan
/// from the identical signed triple.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perspective {
    Initiator,
    Responder,
}

/// Validate the signed request/response/ack triple and return the install
/// plan for the requested `perspective`. The initiator and responder each
/// hold a full copy of the same triple; each calls this ONCE, from its own
/// `MeshView` and its own `ReplayCache`, with its own perspective, to derive
/// its own `local_*`/`peer_*` install config.
#[allow(
    clippy::too_many_arguments,
    reason = "the public validation boundary keeps every signed transcript and context input explicit"
)]
pub fn validate_upgrade_as(
    perspective: Perspective,
    view: &MeshView,
    policy: &PortPolicy,
    overlay: &OverlayPolicy,
    current_view: u64,
    request: &TunnelUpgradeRequest,
    response: &TunnelUpgradeResponse,
    ack: &TunnelUpgradeAck,
    replay: &mut ReplayCache,
) -> Result<TunnelInstallPlan, UpgradeError> {
    validate_admission_root(view.active_set.admission_root)?;
    request.verify_signature()?;
    response.verify_signature()?;
    ack.verify_signature()?;

    let rq = &request.fields;
    let rs = &response.fields;
    let ak = &ack.fields;
    let root = view.active_set.valset_root;
    let admission_root = view.active_set.admission_root;
    if rq.request_tuple()
        != (
            view.active_set.namespace.as_str(),
            view.active_set.epoch,
            root,
            admission_root,
            view.mesh_version,
        )
        || rs.request_hash != request.hash()
        || rs.request_tuple()
            != (
                view.active_set.namespace.as_str(),
                view.active_set.epoch,
                root,
                admission_root,
                view.mesh_version,
            )
        || ak.request_hash != request.hash()
        || ak.response_hash != response.hash()
        || ak.request_tuple()
            != (
                view.active_set.namespace.as_str(),
                view.active_set.epoch,
                root,
                admission_root,
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
    // the handshake's X25519 keys must be the ones the mesh-versioned,
    // ed25519-signed records advertise — same rule as the endpoint pin above.
    // without this, a party could complete a handshake under a fresh WG key
    // the rest of the mesh never versioned, and the tunnel would silently
    // diverge from the advertised mesh.
    if rq.initiator_wireguard_public_key != initiator_record.wireguard_public_key
        || rs.responder_wireguard_public_key != responder_record.wireguard_public_key
    {
        return Err(UpgradeError::HandshakeMismatch);
    }
    ensure_x25519(rq.initiator_wireguard_public_key)?;
    ensure_x25519(rs.responder_wireguard_public_key)?;
    if let Some(endpoint) = &rq.initiator_wireguard_endpoint {
        policy.check_endpoint(endpoint)?;
    }
    if let Some(endpoint) = &rs.responder_wireguard_endpoint {
        policy.check_endpoint(endpoint)?;
    }
    if current_view > rq.expires_at_view
        || current_view > rs.expires_at_view
        || current_view > ak.expires_at_view
    {
        return Err(UpgradeError::Expired);
    }
    // freshness is a SYMMETRIC window: the two ends of a handshake run
    // independent view clocks (each node's plane learns views from its own
    // finalization drain), so a genuine ack routinely arrives with
    // `installed_at_view` a tick or two ahead of the validator's clock. a
    // zero-tolerance future check permanently failed real cross-node pairs
    // (initiator applied, responder refused the same triple); bounding both
    // directions by the same lag keeps the staleness envelope without
    // punishing ordinary skew.
    if ak.installed_at_view.saturating_sub(current_view) > MAX_ACK_INSTALL_LAG
        || current_view.saturating_sub(ak.installed_at_view) > MAX_ACK_INSTALL_LAG
    {
        return Err(UpgradeError::BadAckView);
    }
    overlay.validate_for(view, rq.responder_identity, &rq.requested_allowed_ips)?;
    overlay.validate_for(view, rq.initiator_identity, &rs.accepted_allowed_ips)?;
    // the two parties' canonical overlay routes must differ — equality means
    // the derivation collided (identity-hash ULA) or mis-indexed (v4), and a
    // tunnel whose local and peer routes coincide cannot route.
    if rq.requested_allowed_ips == rs.accepted_allowed_ips {
        return Err(UpgradeError::InvalidAllowedIp);
    }
    let replay_keys = replay.check_batch(&[
        (rq.initiator_identity, rq.epoch, rq.nonce),
        (rs.responder_identity, rs.epoch, rs.nonce),
        (ak.initiator_identity, ak.epoch, ak.nonce),
    ])?;

    for (identity, epoch, nonce) in replay_keys {
        replay.insert(identity, epoch, nonce);
    }

    let context = TunnelInstallContext {
        namespace: view.active_set.namespace.clone(),
        epoch: view.active_set.epoch,
        valset_root: root,
        admission_root,
        mesh_version: view.mesh_version,
    };
    Ok(match perspective {
        Perspective::Initiator => TunnelInstallPlan {
            context,
            local_identity: rq.initiator_identity,
            peer_identity: rq.responder_identity,
            local_wireguard_public_key: rq.initiator_wireguard_public_key,
            peer_wireguard_public_key: rs.responder_wireguard_public_key,
            peer_endpoint: rs.responder_wireguard_endpoint,
            local_interface_ips: rs.accepted_allowed_ips.clone(),
            allowed_ips: rq.requested_allowed_ips.clone(),
            keepalive_seconds: rs.keepalive_seconds,
        },
        Perspective::Responder => TunnelInstallPlan {
            context,
            local_identity: rq.responder_identity,
            peer_identity: rq.initiator_identity,
            local_wireguard_public_key: rs.responder_wireguard_public_key,
            peer_wireguard_public_key: rq.initiator_wireguard_public_key,
            peer_endpoint: rq.initiator_wireguard_endpoint,
            local_interface_ips: rq.requested_allowed_ips.clone(),
            allowed_ips: rs.accepted_allowed_ips.clone(),
            keepalive_seconds: rs.keepalive_seconds,
        },
    })
}

trait CommonRequestFields {
    fn request_tuple(&self) -> (&str, u64, Root, AdmissionRoot, MeshVersion);
}

impl CommonRequestFields for TunnelUpgradeRequestFields {
    fn request_tuple(&self) -> (&str, u64, Root, AdmissionRoot, MeshVersion) {
        (
            &self.namespace,
            self.epoch,
            self.valset_root,
            self.admission_root,
            self.mesh_version,
        )
    }
}

impl CommonRequestFields for TunnelUpgradeResponseFields {
    fn request_tuple(&self) -> (&str, u64, Root, AdmissionRoot, MeshVersion) {
        (
            &self.namespace,
            self.epoch,
            self.valset_root,
            self.admission_root,
            self.mesh_version,
        )
    }
}

impl CommonRequestFields for TunnelUpgradeAckFields {
    fn request_tuple(&self) -> (&str, u64, Root, AdmissionRoot, MeshVersion) {
        (
            &self.namespace,
            self.epoch,
            self.valset_root,
            self.admission_root,
            self.mesh_version,
        )
    }
}

fn ensure_x25519(key: X25519PublicKey) -> Result<(), UpgradeError> {
    if key.0 == [0u8; 32] {
        return Err(UpgradeError::InvalidWireGuardKey);
    }
    Ok(())
}

fn validate_admission_root(admission_root: AdmissionRoot) -> Result<(), UpgradeError> {
    if admission_root.0 == [0u8; 32] {
        return Err(UpgradeError::MissingAdmissionRoot);
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
                || is_v4_this_network(ip)
                || is_v4_protocol_assignment(ip)
                || is_v4_benchmarking(ip)
                || is_v4_reserved(ip)
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
            if (ip.is_private() || ip.is_link_local() || is_v4_shared_address_space(ip))
                && !policy.allow_private_ip
            {
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

fn is_v4_this_network(ip: Ipv4Addr) -> bool {
    ip.octets()[0] == 0
}

fn is_v4_shared_address_space(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (o[1] & 0b1100_0000) == 64
}

fn is_v4_protocol_assignment(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 192 && o[1] == 0 && o[2] == 0
}

fn is_v4_benchmarking(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 198 && (o[1] == 18 || o[1] == 19)
}

fn is_v4_reserved(ip: Ipv4Addr) -> bool {
    ip.octets()[0] >= 240
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
    put_admission_root(out, record.admission_root);
    put_identity(out, record.validator_identity);
    put_x25519(out, record.wireguard_public_key);
    put_endpoint(out, record.control_endpoint);
    put_opt_endpoint(out, record.wireguard_endpoint);
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
    put_admission_root(out, fields.admission_root);
    put_mesh_version(out, fields.mesh_version);
    put_identity(out, fields.initiator_identity);
    put_identity(out, fields.responder_identity);
    put_x25519(out, fields.initiator_wireguard_public_key);
    put_opt_endpoint(out, fields.initiator_wireguard_endpoint);
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
    put_admission_root(out, fields.admission_root);
    put_mesh_version(out, fields.mesh_version);
    put_identity(out, fields.responder_identity);
    put_identity(out, fields.initiator_identity);
    put_x25519(out, fields.responder_wireguard_public_key);
    put_opt_endpoint(out, fields.responder_wireguard_endpoint);
    put_allowed_ips(out, &fields.accepted_allowed_ips);
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
    put_admission_root(out, fields.admission_root);
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

fn put_admission_root(out: &mut Vec<u8>, value: AdmissionRoot) {
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

/// `None` is a single `0` where a present endpoint's transport byte (1|2)
/// would sit — unambiguous, and `Some` bytes stay identical to the
/// pre-Option encoding, so existing signatures keep verifying.
fn put_opt_endpoint(out: &mut Vec<u8>, endpoint: Option<Endpoint>) {
    match endpoint {
        Some(endpoint) => put_endpoint(out, endpoint),
        None => out.push(0),
    }
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
