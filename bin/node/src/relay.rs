//! the submit-relay channel wire format — how an observer-standing node
//! delivers a frame it signed, and how a validator answers with the frame's
//! consensus fate.
//!
//! transport: the observer is already an authenticated mesh peer; it speaks
//! on `CHANNEL_SUBMIT_RELAY` to ONE current validator. the message carries
//! the frame bytes exactly as `node::encode_frame` produced them — the
//! frame's own signature (origin, seq, target, payload) is the authorship;
//! the channel peer identity only GATES (origin must equal the sender, so a
//! node relays nothing but its own ops, and the origin must hold committed
//! observer standing). the validator takes consensus custody via
//! `submit_frame` and replies when the frame drains — Applied with the
//! sealed block's coordinates, Rejected for a deterministic no-op, Refused
//! for door failures and expired holds.
//!
//! json on the wire: matches the lobby idiom — this lane is low-volume (a
//! human posting messages), and the frame bytes inside are already signed.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RelayOutcome {
    /// drained Applied at `height`; `app_hash` is the PER-BLOCK boundary
    /// hash the frame settled at (what a local app-surface hold reports).
    Applied { height: u64, app_hash: String },
    /// finalized but deterministically rejected by its module.
    Rejected { detail: String },
    /// refused at the door (bad frame / origin mismatch / no standing) or
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

/// the validator's door check, pure so it is testable without a mesh:
/// the frame must decode AND verify (the kernel checks the signature binds
/// origin/seq/target/payload), its origin must BE the sending peer (a node
/// relays only its own ops — no laundering), and that origin must hold
/// committed observer standing (validators submit locally; parked joiners
/// have no standing). membership-current state is the CALLER's to fetch —
/// this needs only bytes.
pub fn verify_relay_submit(
    frame: &[u8],
    sender: &[u8],
    observers: &[Vec<u8>],
) -> Result<node::FrameId, String> {
    let (origin, _msg) = node::decode_frame(frame).map_err(|e| format!("bad frame: {e}"))?;
    let sdk::Origin::External(origin_bytes) = origin else {
        return Err("relayed frames carry an external origin".into());
    };
    if origin_bytes.as_slice() != sender {
        return Err("frame origin is not the relaying peer — a node relays only its own ops".into());
    }
    if !observers.iter().any(|o| o.as_slice() == sender) {
        return Err("origin holds no committed observer standing — submit via a validator".into());
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
    fn door_accepts_a_standing_observers_own_frame() {
        let author = sk(7);
        let me = author.public_key().as_ref().to_vec();
        let frame = node::encode_frame(&author, 3, &msg());
        let id = verify_relay_submit(&frame, &me, &[me.clone()]).expect("accepted");
        assert_eq!(id, node::frame_id(&frame));
    }

    #[test]
    fn door_refuses_origin_that_is_not_the_sender() {
        let author = sk(7);
        let other = sk(8).public_key().as_ref().to_vec();
        let frame = node::encode_frame(&author, 0, &msg());
        let err = verify_relay_submit(&frame, &other, &[other.clone()]).unwrap_err();
        assert!(err.contains("only its own ops"), "{err}");
    }

    #[test]
    fn door_refuses_without_observer_standing() {
        let author = sk(7);
        let me = author.public_key().as_ref().to_vec();
        let frame = node::encode_frame(&author, 0, &msg());
        let err = verify_relay_submit(&frame, &me, &[]).unwrap_err();
        assert!(err.contains("standing"), "{err}");
    }

    #[test]
    fn door_refuses_tampered_bytes() {
        let author = sk(7);
        let me = author.public_key().as_ref().to_vec();
        let mut frame = node::encode_frame(&author, 0, &msg());
        let last = frame.len() - 1;
        frame[last] ^= 0x01;
        assert!(verify_relay_submit(&frame, &me, &[me.clone()]).is_err());
    }
}
