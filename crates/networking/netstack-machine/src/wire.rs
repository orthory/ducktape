//! The contract's WIRE form: borsh — name-free, little-endian, fixed-width
//! tags — the bytes the frozen scenario traces record, the schema hash
//! pins, and the `ducktape:netstack` guest boundary carries. Everything
//! here derives from the contract types themselves; nothing is
//! hand-mirrored, so a contract change is a wire change by construction,
//! and [`schema_text`] is what says so.
//!
//! Foreign types the contract carries and borsh does not describe get a
//! helper module each (`key`, `keys`, `result_socket_addr`; the socket
//! address itself lives in `wireguard::wire_schema`). A helper's serialized
//! form is the plain borsh encoding of its underlying bytes, and its schema
//! declares exactly that, so the description never drifts from the bytes.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use borsh::schema::{Declaration, Definition, Fields};
use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;
use sha2::{Digest, Sha256};
use wireguard::wire_schema::socket_addr;

use crate::contract::{Effect, Event, ReachabilityEvent};

/// The pinned hash of [`schema_text`]: the contract's version. A change
/// here is a wire change — the schema test refuses drift until the fixture
/// (`tests/fixtures/contract.schema`) and this constant are regenerated in
/// the same PR (`UPDATE_TRACES=1 cargo test -p netstack-machine`).
pub const CONTRACT_SCHEMA_HASH: &str =
    "e7c399754832777546900cefbee47ce07dd0157ff0bd503b5ae518167bfa3512";

#[derive(Debug, thiserror::Error)]
#[error("contract wire: {0}")]
pub struct WireError(#[from] borsh::io::Error);

pub fn encode_event(event: &Event) -> Vec<u8> {
    borsh::to_vec(event).expect("contract types always serialize")
}

pub fn decode_event(bytes: &[u8]) -> Result<Event, WireError> {
    Ok(borsh::from_slice(bytes)?)
}

pub fn encode_effects(effects: &[Effect]) -> Vec<u8> {
    borsh::to_vec(effects).expect("contract types always serialize")
}

pub fn decode_effects(bytes: &[u8]) -> Result<Vec<Effect>, WireError> {
    Ok(borsh::from_slice(bytes)?)
}

/// The wire's three roots — the event in, the effect list out, the
/// observation the host surfaces — with every definition they reach, as
/// one canonical text. Definitions are a sorted map, so the text is stable
/// across runs and its hash identifies the contract.
pub fn schema_text() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "root event = {}", Event::declaration());
    let _ = writeln!(out, "root effects = {}", <Vec<Effect>>::declaration());
    let _ = writeln!(
        out,
        "root observation = {}",
        ReachabilityEvent::declaration()
    );
    let mut definitions = BTreeMap::new();
    Event::add_definitions_recursively(&mut definitions);
    <Vec<Effect>>::add_definitions_recursively(&mut definitions);
    ReachabilityEvent::add_definitions_recursively(&mut definitions);
    for (declaration, definition) in &definitions {
        let _ = writeln!(out, "{declaration} = {definition:?}");
    }
    out
}

pub fn schema_hash() -> String {
    Sha256::digest(schema_text().as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// An ed25519 public key: its 32 raw bytes, validated as a point on the
/// way back in.
pub(crate) mod key {
    use super::*;

    pub fn serialize<W: borsh::io::Write>(
        key: &ed25519::PublicKey,
        writer: &mut W,
    ) -> Result<(), borsh::io::Error> {
        writer.write_all(key.as_ref())
    }

    pub fn deserialize<R: borsh::io::Read>(
        reader: &mut R,
    ) -> Result<ed25519::PublicKey, borsh::io::Error> {
        let bytes = <[u8; 32]>::deserialize_reader(reader)?;
        ed25519::PublicKey::decode(&bytes[..]).map_err(|_| {
            borsh::io::Error::new(
                borsh::io::ErrorKind::InvalidData,
                "not an ed25519 public key",
            )
        })
    }

    pub fn declaration() -> Declaration {
        "Ed25519PublicKey".into()
    }

    pub fn definitions(definitions: &mut BTreeMap<Declaration, Definition>) {
        borsh::schema::add_definition(
            declaration(),
            Definition::Struct {
                fields: Fields::UnnamedFields(vec![<[u8; 32]>::declaration()]),
            },
            definitions,
        );
        <[u8; 32]>::add_definitions_recursively(definitions);
    }
}

/// A list of ed25519 public keys, laid out as borsh lays out every
/// sequence: a `u32` count, then the elements.
pub(crate) mod keys {
    use super::*;

    pub fn serialize<W: borsh::io::Write>(
        keys: &[ed25519::PublicKey],
        writer: &mut W,
    ) -> Result<(), borsh::io::Error> {
        let count = u32::try_from(keys.len()).map_err(|_| {
            borsh::io::Error::new(borsh::io::ErrorKind::InvalidData, "too many keys")
        })?;
        count.serialize(writer)?;
        keys.iter().try_for_each(|key| key::serialize(key, writer))
    }

    pub fn deserialize<R: borsh::io::Read>(
        reader: &mut R,
    ) -> Result<Vec<ed25519::PublicKey>, borsh::io::Error> {
        let count = u32::deserialize_reader(reader)?;
        (0..count).map(|_| key::deserialize(reader)).collect()
    }

    pub fn declaration() -> Declaration {
        format!("Vec<{}>", key::declaration())
    }

    pub fn definitions(definitions: &mut BTreeMap<Declaration, Definition>) {
        borsh::schema::add_definition(
            declaration(),
            Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: key::declaration(),
            },
            definitions,
        );
        key::definitions(definitions);
    }
}

/// `Result<SocketAddr, String>` — schema only (borsh serializes it natively),
/// laid out as borsh lays out every `Result`: tag 1 = `Ok`, tag 0 = `Err`.
pub(crate) mod result_socket_addr {
    use super::*;

    pub fn declaration() -> Declaration {
        format!(
            "Result<{}, {}>",
            socket_addr::declaration(),
            String::declaration()
        )
    }

    pub fn definitions(definitions: &mut BTreeMap<Declaration, Definition>) {
        borsh::schema::add_definition(
            declaration(),
            Definition::Enum {
                tag_width: 1,
                variants: vec![
                    (1, "Ok".into(), socket_addr::declaration()),
                    (0, "Err".into(), String::declaration()),
                ],
            },
            definitions,
        );
        socket_addr::definitions(definitions);
        String::add_definitions_recursively(definitions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{CmdToken, MeshEpochEvent, ReqId, Resolution};
    use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
    use std::net::SocketAddr;
    use std::path::Path;
    use wireguard::effect::PeerTunnelConfig;
    use wireguard::{AllowedIp, MeshVersion, ValidatorIdentity, X25519PublicKey};

    fn key(seed: u64) -> ed25519::PublicKey {
        PrivateKey::from_seed(seed).public_key()
    }

    fn addr(octet: u8, port: u16) -> SocketAddr {
        SocketAddr::from(([8, 8, 8, octet], port))
    }

    fn peer(octet: u8) -> PeerTunnelConfig {
        PeerTunnelConfig {
            wireguard_public_key: X25519PublicKey([octet; 32]),
            endpoint: Some(addr(octet, 51820)),
            allowed_ips: vec![AllowedIp::new("fd00::1".parse().unwrap(), 128).unwrap()],
            keepalive_seconds: Some(25),
        }
    }

    /// One value of every observation variant; the schema fixture is what
    /// guards a variant this list does not know.
    fn observations() -> Vec<ReachabilityEvent> {
        vec![
            ReachabilityEvent::Send {
                to: key(1),
                bytes: vec![1, 2, 3],
            },
            ReachabilityEvent::MeshReady {
                epoch: 3,
                version: MeshVersion([7; 32]),
            },
            ReachabilityEvent::TunnelsApplied {
                epoch: 3,
                interface: "dt-x".into(),
                peers: 2,
            },
            ReachabilityEvent::PeerFailed {
                peer: key(2),
                reason: "refused".into(),
            },
            ReachabilityEvent::EpochFailed {
                epoch: 3,
                reason: "verify".into(),
            },
            ReachabilityEvent::MeshRestored {
                epoch: 2,
                interface: "dt-x".into(),
                peers: 1,
            },
            ReachabilityEvent::RestoreFailed {
                reason: "bad file".into(),
            },
            ReachabilityEvent::PersistFailed {
                reason: "disk".into(),
            },
            ReachabilityEvent::StandbyTunnelsApplied {
                epoch: 3,
                interface: "dt-x".into(),
                peers: 1,
            },
            ReachabilityEvent::MeshAdopted {
                epoch: 3,
                version: MeshVersion([8; 32]),
                peers: 2,
            },
            ReachabilityEvent::PeerReadvertised {
                peer: key(3),
                interface: "dt-x".into(),
            },
            ReachabilityEvent::PeerEndpointResolved {
                peer: key(3),
                endpoint: addr(3, 4000),
            },
            ReachabilityEvent::InvitePeerInstalled {
                peer: key(4),
                interface: "dt-x".into(),
            },
            ReachabilityEvent::ControlEndpointObserved {
                peer: ValidatorIdentity([5; 32]),
                control_endpoint: addr(5, 8846),
            },
        ]
    }

    fn events() -> Vec<Event> {
        vec![
            Event::Retarget {
                event: MeshEpochEvent {
                    epoch: 3,
                    members: vec![key(1), key(2)],
                    standbys: vec![key(9)],
                    current_view: 10,
                },
                persisted: Some(vec![9, 9]),
            },
            Event::Deliver {
                from: key(2),
                bytes: vec![4, 5],
            },
            Event::ViewTick(11),
            Event::Nudge,
            Event::InstallInvitePeer {
                token: CmdToken(1),
                peer: key(4),
                wireguard_public_key: X25519PublicKey([4; 32]),
                endpoint: addr(4, 51820),
            },
            Event::BootstrapCoordinatedInvitePeer {
                token: CmdToken(2),
                peer: key(6),
                wireguard_public_key: X25519PublicKey([6; 32]),
                intro: vec![6],
            },
            Event::SendResolverDatagram {
                endpoint: addr(6, 7),
                bytes: vec![7],
            },
            Event::Resolved {
                req: ReqId(1),
                outcome: Ok(Resolution::Punched(addr(2, 40000))),
            },
            Event::Resolved {
                req: ReqId(2),
                outcome: Err("dead".into()),
            },
            Event::RendezvousResolved {
                req: ReqId(3),
                outcome: Ok(addr(3, 40001)),
            },
            Event::DatagramReplied {
                req: ReqId(4),
                outcome: Ok(vec![1]),
            },
            Event::WgApplied {
                req: ReqId(5),
                outcome: Err("backend".into()),
            },
            Event::Shutdown,
        ]
    }

    fn effects() -> Vec<Effect> {
        vec![
            Effect::MeshSend {
                to: key(2),
                bytes: vec![1],
            },
            Effect::Observe(ReachabilityEvent::MeshReady {
                epoch: 1,
                version: MeshVersion([1; 32]),
            }),
            Effect::WgApply {
                req: ReqId(1),
                bring_up: true,
                peers: vec![peer(2), peer(3)],
            },
            Effect::WgRemove,
            Effect::ResolveStart {
                req: ReqId(2),
                peer: nat_traversal::NodeKey([2; 32]),
                advertised: addr(2, 51820),
            },
            Effect::RendezvousStart {
                req: ReqId(3),
                peer: nat_traversal::NodeKey([3; 32]),
            },
            Effect::UdpSend {
                endpoint: addr(4, 1),
                bytes: vec![4],
            },
            Effect::UdpSendAwait {
                req: ReqId(4),
                endpoint: addr(5, 1),
                bytes: vec![5],
                timeout_ms: 2000,
            },
            Effect::ReplyInstall {
                token: CmdToken(1),
                outcome: Ok(()),
            },
            Effect::ReplyIntro {
                token: CmdToken(2),
                outcome: Err("no ack".into()),
            },
            Effect::Persist { bytes: vec![8] },
        ]
    }

    #[test]
    fn every_variant_round_trips() {
        for event in events() {
            assert_eq!(decode_event(&encode_event(&event)).unwrap(), event);
        }
        let effects = effects();
        assert_eq!(decode_effects(&encode_effects(&effects)).unwrap(), effects);
        for observation in observations() {
            let bytes = borsh::to_vec(&observation).unwrap();
            assert_eq!(
                borsh::from_slice::<ReachabilityEvent>(&bytes).unwrap(),
                observation
            );
        }
    }

    #[test]
    fn a_key_that_is_not_a_point_is_refused() {
        let event = Event::Deliver {
            from: key(1),
            bytes: vec![],
        };
        let mut bytes = encode_event(&event);
        // a byte pattern the curve itself refuses (no square root for its
        // x coordinate), spliced in after the variant tag.
        let bad = (0u8..=255)
            .map(|fill| [fill; 32])
            .find(|candidate| ed25519::PublicKey::decode(&candidate[..]).is_err())
            .expect("some constant 32-byte pattern is not an ed25519 point");
        bytes[1..33].copy_from_slice(&bad);
        assert!(decode_event(&bytes).is_err());
    }

    /// The schema fixture is the reviewable contract; the hash constant is
    /// the runtime pin. Both must move together, in the PR that changes the
    /// types: `UPDATE_TRACES=1` rewrites the fixture and prints the hash.
    #[test]
    fn the_contract_schema_is_frozen() {
        let text = schema_text();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/contract.schema");
        if std::env::var_os("UPDATE_TRACES").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &text).unwrap();
        }
        let pinned = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            text == pinned,
            "the contract schema drifted from {} — a wire change; regenerate with \
             UPDATE_TRACES=1 in the same PR and review the fixture diff",
            path.display()
        );
        let hash = schema_hash();
        assert!(
            hash == CONTRACT_SCHEMA_HASH,
            "CONTRACT_SCHEMA_HASH must be {hash} for the current contract"
        );
    }
}
