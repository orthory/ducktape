//! the submit-relay channel wire format — how an observer-standing node
//! delivers a frame it signed, and how a validator answers with the frame's
//! consensus fate.
//!
//! transport: the observer ships the frame bytes on `CHANNEL_SUBMIT_RELAY`
//! to ONE current validator, exactly as `node::encode_frame` produced them.
//! the SENDING peer's transport identity is deliberately NOT consulted:
//! observers speak from the network's DERIVED LOBBY identity (the lobby key
//! folded into every mesh), which ANY invite holder can derive — so origin
//! could never equal a real observer's transport peer, and a peer-vs-origin
//! gate adds nothing anyway. the frame's OWN signature is the authorization:
//! it binds (origin, seq, target, payload) to the origin key, so forgery is
//! impossible; committed observer standing on the ORIGIN is the policy gate;
//! and a byte-identical replay collapses in the consensus lane's exactly-once
//! digest gate. the validator takes consensus custody via `submit_frame` and
//! replies when the frame drains — Applied with the sealed block's
//! coordinates, Rejected for a deterministic no-op, Refused for door failures
//! and expired holds.
//!
//! json on the wire: matches the lobby idiom — this lane is low-volume (a
//! human posting messages), and the frame bytes inside are already signed.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RelayOutcome {
    /// drained Applied at `height`; `app_hash` is the PER-BLOCK boundary
    /// hash the frame settled at (what a local app-surface hold reports).
    Applied { height: u64, app_hash: String },
    /// finalized but deterministically rejected by its module.
    Rejected { detail: String },
    /// refused at the door (bad frame / origin lacks observer standing) or
    /// the validator's hold expired before finalization — the op may still
    /// land later; clients re-query on block events.
    Refused { detail: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RelayMsg {
    /// an observer-signed frame, bytes exactly as `encode_frame` produced.
    Submit { frame: Vec<u8> },
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

/// the validator's door check, pure so it is testable without a mesh: the
/// frame must decode AND verify (the kernel checks the signature binds
/// origin/seq/target/payload), its origin must be `Origin::External`, and
/// that ORIGIN must hold committed observer standing (validators submit
/// locally; parked joiners have no standing). the sending peer is
/// DELIBERATELY not an argument: observers ride the network's derived lobby
/// transport identity — derivable by any invite holder — so a peer-vs-origin
/// check could never pass for a real observer and would gate nothing. the
/// frame's signature is the authorization (forgery impossible) and the
/// exactly-once digest gate collapses byte-identical replays; committed
/// standing is the only policy the door adds. membership-current state is the
/// CALLER's to fetch — this needs only bytes.
pub fn verify_relay_submit(
    frame: &[u8],
    observers: &[Vec<u8>],
) -> Result<node::FrameId, String> {
    let (origin, _msg) = node::decode_frame(frame).map_err(|e| format!("bad frame: {e}"))?;
    let sdk::Origin::External(origin_bytes) = origin else {
        return Err("relayed frames carry an external origin".into());
    };
    if !observers.iter().any(|o| o.as_slice() == origin_bytes.as_slice()) {
        return Err(
            "origin holds no committed observer standing — submit ops via a validator".into(),
        );
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
    fn wire_round_trips() {
        for m in [
            RelayMsg::Submit { frame: vec![1, 2, 3] },
            RelayMsg::Reply {
                frame_id: [7; 32],
                outcome: RelayOutcome::Applied {
                    height: 42,
                    app_hash: "aa".into(),
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
    fn door_accepts_a_frame_from_a_standing_origin() {
        let author = sk(7);
        let me = author.public_key().as_ref().to_vec();
        let frame = node::encode_frame(&author, 3, &msg());
        // the sending peer is never consulted — standing rides on the ORIGIN.
        let id = verify_relay_submit(&frame, &[me.clone()]).expect("accepted");
        assert_eq!(id, node::frame_id(&frame));
    }

    #[test]
    fn door_refuses_without_observer_standing() {
        let author = sk(7);
        let frame = node::encode_frame(&author, 0, &msg());
        let err = verify_relay_submit(&frame, &[]).unwrap_err();
        assert!(err.contains("standing"), "{err}");
    }

    #[test]
    fn door_refuses_a_signature_tampered_frame_that_still_parses() {
        let author = sk(7);
        let me = author.public_key().as_ref().to_vec();
        let frame = node::encode_frame(&author, 0, &msg());

        // flip a byte INSIDE the signature: the frame still PARSES as json (the
        // envelope is intact) but the ed25519 verification no longer binds, so
        // this exercises the signature gate — not the json parser. (flipping the
        // last raw byte, as a naive tamper would, only breaks the envelope.)
        let mut v: serde_json::Value = serde_json::from_slice(&frame).expect("frame is json");
        let sig = v["sig"].as_array_mut().expect("sig is a byte array");
        let b = sig[0].as_u64().expect("sig byte");
        sig[0] = serde_json::Value::from(b ^ 0x01);
        let tampered = serde_json::to_vec(&v).expect("re-serialize");

        // it fails at signature verification, NOT as a parse error: a genuine
        // junk-json envelope errors with different wording.
        let junk = verify_relay_submit(b"not a frame", &[me.clone()]).unwrap_err();
        let err = verify_relay_submit(&tampered, &[me.clone()]).unwrap_err();
        assert_ne!(err, junk, "tamper must fail at the signature, not the parser");
        assert!(err.contains("signature"), "{err}");
    }
}
