//! the submit-relay channel wire format — how a relaying node delivers a
//! signed frame, and how a validator answers with the frame's consensus fate.
//!
//! transport: ordinary submits ship the frame bytes on `CHANNEL_SUBMIT_RELAY`
//! to one current validator, exactly as `node::encode_frame` produced them.
//! a frame that references a node-local forge pack first fans that pack out to
//! EVERY current validator in bounded, content-addressed chunks; only after all
//! validators acknowledge the bytes does one validator take consensus custody.
//! the SENDING peer's transport identity is deliberately NOT consulted:
//! mesh admission and submit authorization are separate facts, and a
//! peer-vs-origin gate would fork this lane apart from the validator's local
//! HTTP submit lane. the frame's OWN signature is the authorization
//! AND the whole door: it binds (origin, seq, target, payload) to the origin
//! key, so forgery is impossible, and a byte-identical replay collapses in the
//! consensus lane's exactly-once digest gate. the door adds NO standing
//! policy — any key's validly signed frame enters consensus, the same
//! contract as a validator's local HTTP submit lane. who may do WHAT is
//! per-module policy, decided deterministically inside the state machine (the
//! acl module's dispatch gate plus each module's own origin checks), never at
//! the transport door. the validator takes consensus custody via
//! `submit_frame` and replies when the frame drains — Applied with the sealed
//! block's coordinates, Rejected for a deterministic no-op, Refused for door
//! failures and expired holds.
//!
//! json on the wire: matches the module-interface idiom. blob chunks use hex rather than a
//! JSON byte array so the encoded message stays below commonware's 2 MiB cap.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// Forge packs relayed by a resident or fanned out by a validator are bounded
/// below the smart-HTTP lane's 512 MiB ceiling. The relay channel has a bounded
/// message queue and is shared with ordinary submits, so bulkier repositories
/// must be pushed directly after an operator arranges a dedicated blob plane.
pub const MAX_RELAY_BLOB_BYTES: usize = 64 * 1024 * 1024;

/// 768 KiB raw -> 1.5 MiB hex plus a small JSON envelope, safely below the
/// process-wide 2 MiB commonware message cap while keeping a 64 MiB transfer to
/// 86 chunk messages (under the channel's 128-message inbound backlog).
pub const RELAY_BLOB_CHUNK_BYTES: usize = 768 * 1024;

/// The extra hold a forge pack transfer earns on top of `SUBMIT_HOLD`,
/// budgeted at a 1 MiB/s payload floor. The base hold alone assumed the pack
/// lands within an app-submit budget — structurally impossible for a multi-MB
/// pack crossing a WAN validator link (chunks ride hex-encoded, doubling the
/// wire bytes), so every such push died as "timed out receiving the forge
/// pack" at 10s. `MAX_RELAY_BLOB_BYTES` bounds the whole allowance at 64s.
pub fn blob_transfer_allowance(total: u64) -> std::time::Duration {
    const FLOOR_BYTES_PER_SEC: u64 = 1024 * 1024;
    std::time::Duration::from_secs(total.div_ceil(FLOOR_BYTES_PER_SEC))
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RelayOutcome {
    /// drained Applied at `height`; `root_hash` is the PER-BLOCK boundary
    /// hash the frame settled at (what a local app-surface hold reports).
    Applied { height: u64, root_hash: String },
    /// finalized but deterministically rejected by its module.
    Rejected { detail: String },
    /// refused at the door (bad frame / non-external origin) or the
    /// validator's hold expired before finalization — the op may still
    /// land later; clients re-query on block events.
    Refused { detail: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RelayMsg {
    /// a resident-signed frame, bytes exactly as `encode_frame` produced.
    Submit { frame: Vec<u8> },
    /// Authorize and begin a pre-consensus pack transfer. `frame` is signed and
    /// must reference `digest`; receivers allocate nothing until it verifies.
    BlobOffer {
        frame: Vec<u8>,
        digest: [u8; 32],
        total: u64,
    },
    /// One ordered chunk for an accepted offer. Hex avoids JSON's ~3.5x byte
    /// array expansion; content addressing verifies the completed transfer.
    BlobChunk {
        frame_id: [u8; 32],
        digest: [u8; 32],
        offset: u64,
        chunk_hex: String,
    },
    /// A validator's acknowledgement (or clean refusal) of one blob offer.
    BlobResult {
        frame_id: [u8; 32],
        digest: [u8; 32],
        error: Option<String>,
    },
    /// a validator-to-validator LEADER NUDGE: the sender holds real parked
    /// proposals and (by its local estimate) the receiver leads the CURRENT
    /// view — close it now by beating the idle nop early, so leadership
    /// rotation runs at network speed instead of the 1s idle beat. carries
    /// nothing and grants nothing: the receiver acts only when quiet, only
    /// ever by beating one deterministic nop, and only for a sender that is a
    /// current validator — a stray, stale, or mis-aimed nudge is harmless.
    Nudge,
    /// the validator's answer, keyed by the frame's content address.
    Reply {
        frame_id: [u8; 32],
        outcome: RelayOutcome,
    },
}

pub fn encode_msg(m: &RelayMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<RelayMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

/// The one node-local blob a signed frame needs before entering consensus.
/// Non-forge and malformed forge payloads return `None`: malformed module ops
/// still reach the deterministic module rejection path instead of becoming a
/// relay-specific policy decision.
pub fn required_blob_digest(frame: &[u8]) -> Option<[u8; 32]> {
    // the door reads policy fields only.
    let (_, msg) = node::decode_frame(frame).ok()?;
    if msg.target != "forge" {
        return None;
    }
    match forge::decode_msg(&msg.payload).ok()? {
        forge::ForgeMsg::PushRefs { pack_digest, .. } => {
            pack_digest.as_deref().and_then(digest_bytes)
        }
        forge::ForgeMsg::MergePr { pack_digest, .. } => digest_hex(&pack_digest),
        _ => None,
    }
}

fn digest_bytes(bytes: &[u8]) -> Option<[u8; 32]> {
    bytes.try_into().ok()
}

fn digest_hex(hex: &str) -> Option<[u8; 32]> {
    let bytes = decode_hex(hex).ok()?;
    digest_bytes(&bytes)
}

pub use duckfs_core::{to_hex as encode_hex, unhex as decode_hex};

/// Ordered, bounded assembly for one accepted blob offer. The digest check is
/// completed before bytes enter the shared blob store.
pub struct BlobAssembly {
    digest: [u8; 32],
    total: usize,
    bytes: Vec<u8>,
}

impl BlobAssembly {
    pub fn new(digest: [u8; 32], total: u64) -> Result<Self, String> {
        let total = usize::try_from(total).map_err(|_| "relay blob length does not fit usize")?;
        if total == 0 || total > MAX_RELAY_BLOB_BYTES {
            return Err(format!(
                "relay blob must be 1..={MAX_RELAY_BLOB_BYTES} bytes, got {total}"
            ));
        }
        Ok(Self {
            digest,
            total,
            bytes: Vec::new(),
        })
    }

    /// Append one exact-next chunk. `Ok(None)` needs more data;
    /// `Ok(Some(bytes))` is a complete, digest-verified pack.
    pub fn push(&mut self, offset: u64, chunk_hex: &str) -> Result<Option<Vec<u8>>, String> {
        let offset = usize::try_from(offset).map_err(|_| "blob chunk offset does not fit usize")?;
        if offset != self.bytes.len() {
            return Err(format!(
                "blob chunk offset {offset} does not match next offset {}",
                self.bytes.len()
            ));
        }
        if chunk_hex.len() > RELAY_BLOB_CHUNK_BYTES * 2 {
            return Err("blob chunk exceeds the relay chunk ceiling".into());
        }
        let chunk = decode_hex(chunk_hex)?;
        if chunk.is_empty() {
            return Err("blob chunk must not be empty".into());
        }
        if self.bytes.len().saturating_add(chunk.len()) > self.total {
            return Err("blob chunks exceed the offered total".into());
        }
        self.bytes.extend_from_slice(&chunk);
        if self.bytes.len() != self.total {
            return Ok(None);
        }
        let actual: [u8; 32] = Sha256::digest(&self.bytes).into();
        if actual != self.digest {
            return Err("completed relay blob does not match its digest".into());
        }
        Ok(Some(std::mem::take(&mut self.bytes)))
    }
}

/// the validator's door check, pure so it is testable without a mesh: the
/// frame must decode AND verify (the kernel checks the signature binds
/// origin/seq/target/payload), and its origin must be `Origin::External`.
/// that is the WHOLE door — no standing set is consulted. any key's validly
/// signed frame enters consensus here, exactly as it would on a validator's
/// local HTTP submit lane; the two lanes deliberately carry one contract.
/// the sending peer is DELIBERATELY not an argument: mesh admission and
/// submit authorization are separate facts, and a peer-vs-origin check would
/// fork the two submit lanes apart. authorization is per-module policy resolved
/// deterministically at dispatch (the acl module's gate plus each module's
/// own origin checks), never a transport-door decision — a door-side policy
/// would only fork the two submit lanes apart again.
pub fn verify_relay_submit(frame: &[u8]) -> Result<node::FrameId, String> {
    let (origin, _msg) = node::decode_frame(frame).map_err(|e| format!("bad frame: {e}"))?;
    let sdk::Origin::External(_) = origin else {
        return Err("relayed frames carry an external origin".into());
    };
    Ok(node::frame_id(frame))
}

/// Blob offers may originate from a standing resident or a current validator
/// (the latter is the direct-to-validator HTTP push path fanning out to its
/// peers). The signed frame authorizes the offered digest before allocation.
pub fn verify_blob_offer(
    frame: &[u8],
    digest: &[u8; 32],
    members: &[Vec<u8>],
    residents: &[Vec<u8>],
) -> Result<node::FrameId, String> {
    let (origin, _msg) = node::decode_frame(frame).map_err(|e| format!("bad frame: {e}"))?;
    let sdk::Origin::External(origin_bytes) = origin else {
        return Err("blob offers carry an external origin".into());
    };
    if !members
        .iter()
        .chain(residents)
        .any(|key| key.as_slice() == origin_bytes.as_slice())
    {
        return Err("blob offer origin holds no committed node standing".into());
    }
    if required_blob_digest(frame).as_ref() != Some(digest) {
        return Err("blob offer digest is not referenced by its signed frame".into());
    }
    Ok(node::frame_id(frame))
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::Signer as _;

    fn sk(seed: u64) -> commonware_cryptography::ed25519::PrivateKey {
        commonware_cryptography::ed25519::PrivateKey::from_seed(seed)
    }

    fn msg() -> sdk::Msg {
        sdk::Msg {
            target: "kv".into(),
            payload: b"{}".to_vec(),
        }
    }

    #[test]
    fn a_pack_transfer_earns_hold_proportional_to_its_size() {
        use std::time::Duration;
        assert_eq!(blob_transfer_allowance(1), Duration::from_secs(1));
        assert_eq!(
            blob_transfer_allowance(4 * 1024 * 1024),
            Duration::from_secs(4),
            "a 4 MiB pack earns 4s of transfer on top of the base hold"
        );
        assert_eq!(
            blob_transfer_allowance(MAX_RELAY_BLOB_BYTES as u64),
            Duration::from_secs(64),
            "the relay cap bounds the allowance"
        );
    }

    #[test]
    fn wire_round_trips() {
        for m in [
            RelayMsg::Submit {
                frame: vec![1, 2, 3],
            },
            RelayMsg::BlobOffer {
                frame: vec![4, 5],
                digest: [6; 32],
                total: 7,
            },
            RelayMsg::BlobChunk {
                frame_id: [8; 32],
                digest: [9; 32],
                offset: 10,
                chunk_hex: "abcd".into(),
            },
            RelayMsg::BlobResult {
                frame_id: [11; 32],
                digest: [12; 32],
                error: None,
            },
            RelayMsg::Reply {
                frame_id: [7; 32],
                outcome: RelayOutcome::Applied {
                    height: 42,
                    root_hash: "aa".into(),
                },
            },
            RelayMsg::Reply {
                frame_id: [0; 32],
                outcome: RelayOutcome::Refused { detail: "x".into() },
            },
        ] {
            assert_eq!(decode_msg(&encode_msg(&m)).expect("round trip"), m);
        }
    }

    #[test]
    fn door_accepts_any_validly_signed_external_frame() {
        // NO standing set exists at this door: a fresh key nobody has ever
        // granted anything submits on the same contract as a validator's local
        // HTTP lane. the signature is the whole gate; policy is per-module,
        // resolved at dispatch.
        let author = sk(7);
        let frame = node::encode_frame(&author, 3, &msg());
        let id = verify_relay_submit(&frame).expect("accepted");
        assert_eq!(id, node::frame_id(&frame));
    }

    #[test]
    fn door_refuses_a_signature_tampered_frame_that_still_parses() {
        let author = sk(7);
        let mut tampered = node::encode_frame(&author, 0, &msg());

        // flip a bit INSIDE the trailing 64-byte ed25519 signature: the binary
        // envelope (the length-prefixed origin/seq/target/payload preimage) is
        // untouched, so the frame still PARSES — only the signature binding
        // breaks. this exercises the signature gate, not the envelope parser.
        let sig_start = tampered.len() - 64;
        tampered[sig_start] ^= 0x01;

        // it fails at proof verification, NOT as a parse error: a genuine
        // junk envelope errors with different wording.
        let junk = verify_relay_submit(b"not a frame").unwrap_err();
        let err = verify_relay_submit(&tampered).unwrap_err();
        assert_ne!(err, junk, "tamper must fail at the proof, not the parser");
        assert!(err.contains("frame proof does not bind"), "{err}");
    }

    #[test]
    fn blob_offer_requires_standing_and_a_matching_signed_digest() {
        let author = sk(8);
        let me = author.public_key().as_ref().to_vec();
        let digest = [0xCD; 32];
        let msg = sdk::Msg {
            target: "forge".into(),
            payload: forge::encode_msg(&forge::ForgeMsg::PushRefs {
                repo: "ducktape".into(),
                updates: Vec::new(),
                pack_digest: Some(digest.to_vec()),
                cert: None,
            }),
        };
        let frame = node::encode_frame(&author, 1, &msg);
        assert!(verify_blob_offer(&frame, &digest, std::slice::from_ref(&me), &[]).is_ok());
        assert!(verify_blob_offer(&frame, &digest, &[], std::slice::from_ref(&me)).is_ok());
        assert!(verify_blob_offer(&frame, &digest, &[], &[]).is_err());
        assert!(verify_blob_offer(&frame, &[0; 32], std::slice::from_ref(&me), &[]).is_err());
    }

    #[test]
    fn forge_pack_digest_is_discovered_from_signed_pushes_and_merges() {
        let author = sk(9);
        let digest = [0xAB; 32];
        let push = sdk::Msg {
            target: "forge".into(),
            payload: forge::encode_msg(&forge::ForgeMsg::PushRefs {
                repo: "ducktape".into(),
                updates: Vec::new(),
                pack_digest: Some(digest.to_vec()),
                cert: None,
            }),
        };
        assert_eq!(
            required_blob_digest(&node::encode_frame(&author, 1, &push)),
            Some(digest)
        );

        let merge = sdk::Msg {
            target: "forge".into(),
            payload: forge::encode_msg(&forge::ForgeMsg::MergePr {
                repo: "ducktape".into(),
                number: 1,
                prev_target_oid: "1".repeat(40),
                expected_source_oid: "2".repeat(40),
                merge_oid: "3".repeat(40),
                pack_digest: encode_hex(&digest),
            }),
        };
        assert_eq!(
            required_blob_digest(&node::encode_frame(&author, 2, &merge)),
            Some(digest)
        );
        assert_eq!(
            required_blob_digest(&node::encode_frame(&author, 3, &msg())),
            None
        );
    }

    #[test]
    fn blob_assembly_is_ordered_bounded_and_digest_checked() {
        let bytes = b"the complete git pack";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let mut assembly = BlobAssembly::new(digest, bytes.len() as u64).unwrap();
        assert!(
            assembly
                .push(0, &encode_hex(&bytes[..7]))
                .unwrap()
                .is_none()
        );
        let complete = assembly
            .push(7, &encode_hex(&bytes[7..]))
            .unwrap()
            .expect("complete");
        assert_eq!(complete, bytes);

        let mut wrong_offset = BlobAssembly::new(digest, bytes.len() as u64).unwrap();
        assert!(wrong_offset.push(1, "00").unwrap_err().contains("offset"));
        let mut wrong_digest = BlobAssembly::new([0; 32], bytes.len() as u64).unwrap();
        assert!(
            wrong_digest
                .push(0, &encode_hex(bytes))
                .unwrap_err()
                .contains("digest")
        );
        assert!(BlobAssembly::new(digest, (MAX_RELAY_BLOB_BYTES + 1) as u64).is_err());
    }

    #[test]
    fn largest_blob_chunk_stays_below_the_mesh_message_cap() {
        let msg = RelayMsg::BlobChunk {
            frame_id: [0xFF; 32],
            digest: [0xFF; 32],
            offset: MAX_RELAY_BLOB_BYTES as u64,
            chunk_hex: encode_hex(&vec![0xFF; RELAY_BLOB_CHUNK_BYTES]),
        };
        assert!(
            encode_msg(&msg).len() < 1 << 21,
            "encoded relay chunk must fit the process-wide 2 MiB p2p cap"
        );
    }
}
