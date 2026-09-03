//! The envelope continuation lane stays DELETED. It was a consensus takeover.
//!
//! The lane let one signed frame carry a second op (`continue`). The host
//! released that op under `Origin::Module(parent_op_target)` — a string the
//! frame's own author chose — so any key that could submit a signed frame
//! (`verify_relay_submit` adds no policy beyond the signature) reached
//! every `Origin::Module(_)`-gated arm in the tree.
//!
//! Reproduced against the pre-deletion tree on the real ordered lane
//! (`OrderedNode::submit_frame` → `decode_member` → `Host::submit_block_ops`)
//! over the real `valset` module. One frame, signed by a key that was not a
//! validator, targeting the module `not-a-real-module` — which does not exist,
//! so the parent op was REJECTED `UnknownModule(not-a-real-module)` — carried a
//! continuation `valset::Join{self}`. The continuation dispatched under
//! `Origin::Module("not-a-real-module")` and applied: the validator set went
//! 2 → 3. Two more frames carrying `Leave` evicted both founders, leaving the
//! attacker the sole validator. The identical `Join` sent as a plain frame is
//! refused `valset membership changes only via governance`.
//!
//! Two halves, because the lane needs two things to come back and either one
//! alone is enough to stop it:
//!
//! 1. THE WIRE cannot express a second op. A frame is exactly
//!    `preimage || signature`; anything appended is a decode rejection, so a
//!    continuation section cannot be grafted onto a valid frame.
//! 2. THE HOST synthesizes a module origin in exactly ONE place — the emitted
//!    follow-up push, where the id is the module that just ran. A second
//!    construction site is a second lane, and rustc cannot see the difference.
//!
//! Half 2 is a source-parsing lint because the shape is load-bearing and
//! invisible to the compiler: `Origin::Module(anything)` type-checks.

use std::path::Path;

use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519::PrivateKey;
use sdk::{Msg, Origin};

fn sk(seed: u64) -> PrivateKey {
    PrivateKey::from_seed(seed)
}

fn msg() -> Msg {
    Msg {
        target: "chat".into(),
        payload: b"add-reaction".to_vec(),
    }
}

fn kernel(crate_name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(crate_name)
        .join("src/lib.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// the length-prefixed section a `continue` used to occupy, as an attacker
/// would append it to a validly-signed frame: `[len target][target]
/// [len payload][payload]`, optionally behind the old `cont_flag` byte.
fn continuation_section(target: &str, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(target.len() as u64).to_le_bytes());
    out.extend_from_slice(target.as_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

// ---- half 1: the wire ------------------------------------------------------

/// premise: an honest frame round-trips. without this the rejection tests
/// below could pass vacuously (everything rejects).
#[test]
fn honest_frame_round_trips() {
    let signer = sk(1);
    let frame = node::encode_frame(&signer, 3, &msg());
    let (origin, m) = node::decode_frame(&frame).expect("an honest frame decodes");
    assert_eq!(m, msg(), "the op survives the round-trip");
    assert_eq!(
        origin,
        Origin::External(signer.public_key().as_ref().to_vec()),
        "authorship is the verified signer",
    );
}

/// THE REINTRODUCTION ATTEMPT. Splice a continuation section onto a valid
/// frame, exactly as the deleted codec laid it out, and every variant must
/// fail to decode — the signature covers only the one op, and trailing bytes
/// are not a frame.
#[test]
fn a_frame_cannot_carry_a_second_op() {
    let signer = sk(2);
    let honest = node::encode_frame(&signer, 0, &msg());
    let section = continuation_section("valset", b"join-me");

    // (a) appended raw, (b) behind the old `cont_flag = 1`, (c) spliced
    // BEFORE the signature, which is where the deleted codec put it.
    let (sig_at, _) = honest.split_at(honest.len() - 64);
    let mut behind_flag = vec![1u8];
    behind_flag.extend_from_slice(&section);
    let mut spliced = sig_at.to_vec();
    spliced.extend_from_slice(&behind_flag);
    spliced.extend_from_slice(&honest[honest.len() - 64..]);

    let attempts: [(&str, Vec<u8>); 3] = [
        ("appended raw", [honest.clone(), section.clone()].concat()),
        ("behind a cont_flag", [honest.clone(), behind_flag].concat()),
        ("spliced before the signature", spliced),
    ];
    for (how, forged) in attempts {
        assert!(
            node::decode_frame(&forged).is_err(),
            "a continuation section ({how}) must not decode — the lane is deleted",
        );
    }
}

/// the signature binds the whole preimage: flipping any op byte fails
/// verification, so a continuation cannot ride in by mutating an honest frame.
#[test]
fn signature_binds_the_whole_op() {
    let honest = node::encode_frame(&sk(3), 0, &msg());
    for i in [0, honest.len() / 3, honest.len() - 65] {
        let mut forged = honest.clone();
        forged[i] ^= 0x01;
        assert!(
            node::decode_frame(&forged).is_err(),
            "a flipped preimage byte at {i} must fail verification",
        );
    }
}

/// one frame decodes to one op — the `BlockOp` has no slot for a second.
#[test]
fn decode_member_yields_exactly_one_op() {
    let signer = sk(4);
    let frame = node::encode_frame(&signer, 0, &msg());
    let op = node::decode_member(&frame).expect("frame decodes");
    assert_eq!(op.msg, msg());
    assert_eq!(op.frame, node::frame_id(&frame), "member frame id stamped");
}

// ---- half 2: the host's one module-origin construction site ----------------

/// A dispatch runs under `Origin::Module(id)` for exactly one reason: module
/// `id` emitted it as a follow-up while it was running. The host must
/// therefore build an `Origin::Module` in exactly ONE place, from the module
/// that just executed.
///
/// The deleted lane was a SECOND construction site — `Origin::Module(msg.target)`
/// where `msg.target` came off an attacker-signed frame. A new one would
/// compile, review as plumbing, and reopen a network takeover.
#[test]
fn the_host_synthesizes_a_module_origin_in_exactly_one_place() {
    let src = kernel("host");
    let sites: Vec<(usize, &str)> = src
        .lines()
        .enumerate()
        .map(|(n, line)| (n + 1, line.trim()))
        .filter(|(_, line)| line.contains("Origin::Module(") && !line.starts_with("//"))
        .collect();

    assert_eq!(
        sites.len(),
        1,
        "the host must construct Origin::Module in exactly one place (the \
         emitted-follow-up push). every extra site is a lane that can dispatch \
         under a module identity the module did not earn — that is what the \
         deleted continuation lane was. found:\n{}",
        sites
            .iter()
            .map(|(n, l)| format!("  host/src/lib.rs:{n}  {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let (line_no, site) = sites[0];
    assert!(
        site.contains("queue.push_back((Origin::Module(msg.target.clone()), m))"),
        "the one site must be the emitted-follow-up push, whose id is the \
         module that just ran — not an id read off a submitted op. \
         host/src/lib.rs:{line_no}  {site}",
    );
}

/// `encode_frame` must not regrow a continuation parameter: the composer is
/// the other half of the lane, and a frame carries one op.
#[test]
fn encode_frame_takes_no_continuation() {
    let src = kernel("node");
    let sig = src
        .lines()
        .find(|line| line.contains("pub fn encode_frame("))
        .expect("encode_frame is declared in node/src/lib.rs");
    assert_eq!(
        sig.trim(),
        "pub fn encode_frame(signer: &PrivateKey, seq: u64, msg: &Msg) -> Vec<u8> {",
        "encode_frame signs exactly one op — a fourth parameter is the \
         continuation lane returning",
    );
}
