//! op-frame v3 — the continuation-capable envelope codec and its
//! protocol-version gate at the ordered lane. properties:
//!
//! 1. round-trip: with and without a continuation, decode returns exactly
//!    what encode framed, under the v3 signing domain;
//! 2. the signature covers the CONTINUATION: mutating either continuation
//!    field (or the flag) after signing fails verification — no grafting a
//!    continuation onto (or stripping one off) someone else's op;
//! 3. `cont_flag` outside {0,1} is not a canonical frame;
//! 4. cross-codec: the v2 decoder deterministically rejects a v3 frame and
//!    the v3 decoder rejects a v2 frame — the pre-activation fence is
//!    structural, not a config check;
//! 5. the continuation payload cap rejects at decode;
//! 6. `decode_member` accepts v2 at any version and v3 only at/after
//!    [`node::CONTINUATION_ACTIVATION_VERSION`], with a versioned reason
//!    below it; `decode_frame_any` reads both codecs ungated;
//! 7. the flag day end-to-end: below the boundary a v3 frame is refused at
//!    admission AND deterministically rejected by the drain (a byzantine
//!    proposal); at/after it the same frame admits, applies its parent, and
//!    releases the continuation in the same block — surfaced on
//!    [`node::DrainedOp::continuation`];
//! 8. determinism: two nodes fed the identical v3-carrying batch drain
//!    identical app-hashes, dispositions, and continuation traces.

use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519::PrivateKey;
use directory::{
    DirMsg, DirQuery, DirReply, Directory, decode_reply as dir_decode_reply,
    encode_msg as dir_encode, encode_query as dir_encode_query,
};
use futures::executor::block_on;
use host::Host;
use node::{
    CONTINUATION_ACTIVATION_VERSION, Disposition, OrderedNode, Orderer as _, RoundOrderer,
};
use sdk::{Continuation, Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};

fn sk(seed: u64) -> PrivateKey {
    PrivateKey::from_seed(seed)
}

fn msg() -> Msg {
    Msg {
        target: "chat".into(),
        payload: b"add-reaction".to_vec(),
    }
}

fn cont() -> Continuation {
    Continuation {
        target: "runs".into(),
        payload: b"resume-run-7".to_vec(),
    }
}

// (1) round-trip, both arms.
#[test]
fn v3_roundtrips_with_and_without_continuation() {
    let signer = sk(1);
    for c in [None, Some(cont())] {
        let frame = node::encode_frame_v3(&signer, 3, &msg(), c.as_ref());
        let (origin, m, dc) = node::decode_frame_v3(&frame).expect("v3 frame decodes");
        assert_eq!(m, msg(), "root msg survives the round-trip");
        assert_eq!(dc, c, "continuation survives the round-trip");
        let Origin::External(key) = origin else {
            panic!("a wire frame only ever yields External authorship");
        };
        assert_eq!(key, signer.public_key().as_ref().to_vec());
    }
}

// (2) the signature binds the continuation: flip one byte anywhere in the
// continuation section and verification must fail.
#[test]
fn v3_signature_covers_continuation() {
    let frame = node::encode_frame_v3(&sk(2), 0, &msg(), Some(&cont()));
    // the continuation section sits between the root payload and the 64-byte
    // signature; flip a byte there (the payload tail is well inside it).
    let idx = frame.len() - 64 - 2;
    for tamper in [idx, frame.len() - 64 - cont().payload.len() - 9] {
        let mut forged = frame.clone();
        forged[tamper] ^= 0x01;
        assert!(
            node::decode_frame_v3(&forged).is_err(),
            "a tampered continuation byte (index {tamper}) must fail verification"
        );
    }
}

// (3) a flag outside {0,1} is rejected — exactly one valid encoding.
#[test]
fn v3_rejects_bad_cont_flag() {
    let signer = sk(3);
    let frame = node::encode_frame_v3(&signer, 0, &msg(), None);
    // the flag is the byte right before the 64-byte signature on the None arm.
    let mut forged = frame.clone();
    let flag_idx = forged.len() - 64 - 1;
    assert_eq!(forged[flag_idx], 0, "premise: locating the flag byte");
    forged[flag_idx] = 2;
    assert!(
        node::decode_frame_v3(&forged).is_err(),
        "cont_flag 2 is not canonical (and breaks the signature anyway)"
    );
}

// (4) cross-codec rejection, both directions — the structural fence.
#[test]
fn v2_and_v3_decoders_reject_each_other() {
    let signer = sk(4);
    let v3 = node::encode_frame_v3(&signer, 0, &msg(), Some(&cont()));
    assert!(
        node::decode_frame(&v3).is_err(),
        "the live v2 decoder deterministically rejects a v3 frame"
    );
    let v2 = node::encode_frame(&signer, 0, &msg());
    assert!(
        node::decode_frame_v3(&v2).is_err(),
        "the v3 decoder rejects a v2 frame (no flag byte, wrong domain)"
    );
}

// (5) an over-cap continuation payload rejects at decode even when honestly
// signed — the cap is consensus admission, not composer politeness.
#[test]
fn v3_rejects_over_cap_continuation_payload() {
    let big = Continuation {
        target: "runs".into(),
        payload: vec![0u8; sdk::MAX_CONTINUATION_BYTES + 1],
    };
    let frame = node::encode_frame_v3(&sk(5), 0, &msg(), Some(&big));
    let err = node::decode_frame_v3(&frame).expect_err("over-cap continuation must reject");
    assert!(
        err.to_string().contains("exceeds cap"),
        "rejection names the cap: {err}"
    );
}

// (6) the member decode gate: v2 at ANY version, v3 only at/after the flag day.
#[test]
fn decode_member_gates_v3_on_the_protocol_version() {
    let signer = sk(6);
    let v2 = node::encode_frame(&signer, 0, &msg());
    let v3 = node::encode_frame_v3(&signer, 0, &msg(), Some(&cont()));

    for version in [0, CONTINUATION_ACTIVATION_VERSION] {
        let op = node::decode_member(&v2, version).expect("v2 decodes at any version");
        assert!(op.continuation.is_none());
        assert_eq!(op.frame, node::frame_id(&v2), "member frame id stamped");
    }

    let err = node::decode_member(&v3, CONTINUATION_ACTIVATION_VERSION - 1)
        .expect_err("v3 below the gate must reject");
    assert!(
        err.to_string()
            .contains(&format!("activates at protocol v{CONTINUATION_ACTIVATION_VERSION}")),
        "the reason names the flag day: {err}"
    );

    let op = node::decode_member(&v3, CONTINUATION_ACTIVATION_VERSION)
        .expect("v3 decodes at the gate");
    assert_eq!(op.continuation, Some(cont()), "the envelope continuation rides the BlockOp");
    assert_eq!(op.frame, node::frame_id(&v3), "member frame id stamped");
    assert_eq!(op.msg, msg());

    assert!(
        node::decode_member(b"junk", CONTINUATION_ACTIVATION_VERSION).is_err(),
        "junk is junk at any version"
    );
}

// (6b) the policy-door decode: both codecs, no gate.
#[test]
fn decode_frame_any_reads_both_codecs() {
    let signer = sk(7);
    let v2 = node::encode_frame(&signer, 0, &msg());
    let (_, m, c) = node::decode_frame_any(&v2).expect("v2 arm");
    assert_eq!((m, c), (msg(), None));

    let v3 = node::encode_frame_v3(&signer, 0, &msg(), Some(&cont()));
    let (_, m, c) = node::decode_frame_any(&v3).expect("v3 arm");
    assert_eq!((m, c), (msg(), Some(cont())));

    assert!(node::decode_frame_any(b"junk").is_err());
}

// ---- the ordered-lane gate, end to end --------------------------------------

const DIR: &str = "directory";

/// a mock `upgrade` module reporting a STATIC armed schedule to
/// `CONTINUATION_ACTIVATION_VERSION` at `activation_height` — so
/// `Host::effective_version(h)` crosses the flag day at exactly that height
/// with none of the valset/readiness choreography (the gate under test is the
/// DRAIN's, not the upgrade module's). the host's boundary `Advance` injection
/// is accepted as a no-op: the mock arms purely by height, which is all the
/// version derivation reads.
struct GateAt {
    activation_height: u64,
}

#[async_trait::async_trait(?Send)]
impl Module for GateAt {
    fn id(&self) -> ModuleId {
        "upgrade".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot([0x47; 32])
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match upgrade::decode_msg(&msg.payload).map_err(Error::Module)? {
            upgrade::UpgradeMsg::Advance => Ok(()),
            other => Err(Error::Module(format!("gate mock got {other:?}"))),
        }
    }
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let upgrade::UpgradeQuery::Status = upgrade::decode_query(req).map_err(Error::Module)?;
        let member = vec![0xEE; 32];
        Ok(upgrade::encode_reply(&upgrade::UpgradeReply::Status(
            upgrade::UpgradeStatus {
                current_version: 0,
                pending: Some(upgrade::ScheduledUpgrade {
                    name: "continuation-tx".into(),
                    activation_height: self.activation_height,
                    to_version: CONTINUATION_ACTIVATION_VERSION,
                }),
                members: vec![member.clone()],
                ready: vec![member],
                member_count: 1,
                ready_count: 1,
                armed: true,
            },
        )))
    }
}

fn gated_host(activation_height: u64) -> Host {
    Host::genesis(vec![
        Box::new(Directory::new(DIR)),
        Box::new(GateAt { activation_height }),
    ])
    .expect("genesis")
}

fn dir_set(key: &str, value: &str) -> Msg {
    Msg {
        target: DIR.into(),
        payload: dir_encode(&DirMsg::Set {
            key: key.into(),
            value: value.into(),
        }),
    }
}

async fn dir_get(host: &Host, key: &str) -> Option<String> {
    let reply = host
        .query(DIR, &dir_encode_query(&DirQuery::Get { key: key.into() }))
        .await
        .expect("dir query");
    match dir_decode_reply(&reply).expect("dir reply") {
        DirReply::Value(v) => v,
    }
}

/// a v3 envelope: parent sets `a=1`, continuation sets `b=2`.
fn v3_envelope(signer: &PrivateKey, seq: u64) -> Vec<u8> {
    node::encode_frame_v3(
        signer,
        seq,
        &dir_set("a", "1"),
        Some(&Continuation {
            target: DIR.into(),
            payload: dir_encode(&DirMsg::Set {
                key: "b".into(),
                value: "2".into(),
            }),
        }),
    )
}

// (7) the flag day: refused at admission and rejected by the drain below the
// boundary; admitted, applied, and released at/after it.
#[test]
fn drain_gates_v3_on_the_upgrade_flag_day() {
    block_on(async {
        // activation at height 2: heights 0,1 run v0; heights >= 2 run v4.
        let mut node = OrderedNode::new(gated_host(2), RoundOrderer::new());
        let signer = sk(10);
        let v3 = v3_envelope(&signer, 0);

        // ADMISSION, pre-activation: refused with the versioned reason.
        let err = node
            .submit_frame(v3.clone())
            .await
            .expect_err("v3 must not admit below the flag day");
        assert!(
            err.to_string().contains(&format!(
                "activates at protocol v{CONTINUATION_ACTIVATION_VERSION}"
            )),
            "admission names the flag day: {err}"
        );

        // the DRAIN, pre-activation: a byzantine proposer forces the identical
        // bytes into the order anyway — every honest node rejects them
        // deterministically, with the versioned reason, moving no state.
        let before = node.app_hash();
        node.orderer_mut()
            .submit(node::encode_batch(std::slice::from_ref(&v3)))
            .await
            .expect("byzantine propose");
        while node.drain_delivered().await.expect("drain") != 0 {}
        let drained = node.take_drained();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].disposition, Disposition::Rejected);
        assert!(drained[0].op.is_none(), "nothing decodes below the gate");
        assert!(
            drained[0].reason.as_deref().unwrap_or_default().contains(&format!(
                "activates at protocol v{CONTINUATION_ACTIVATION_VERSION}"
            )),
            "the drained reason names the flag day: {:?}",
            drained[0].reason
        );
        assert_eq!(node.app_hash(), before, "a rejected v3 frame moves nothing");
        assert_eq!(dir_get(node.host(), "a").await, None);

        // a filler block below the boundary (height 1) advances the chain.
        node.submit(&signer, 1, dir_set("x", "9")).await.expect("filler");
        node.flush_batch().await.expect("flush");
        while node.drain_delivered().await.expect("drain") != 0 {}
        node.take_drained();

        // ADMISSION at the boundary (next height = 2): the SAME frame admits,
        // applies its parent, and releases the continuation in the same block.
        node.submit_frame(v3.clone()).await.expect("v3 admits at the flag day");
        node.flush_batch().await.expect("flush");
        while node.drain_delivered().await.expect("drain") != 0 {}
        let drained = node.take_drained();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].height, 2, "the flag-day block");
        assert_eq!(drained[0].disposition, Disposition::Applied);
        let op = drained[0].op.as_ref().expect("decoded op");
        assert_eq!(op.target, DIR);
        let released = op.continuation.as_ref().expect("continuation surfaced");
        assert_eq!(released.target, DIR);
        assert_eq!(released.disposition, Disposition::Applied);
        assert_eq!(released.reason, None);
        assert_eq!(released.dispatches.len(), 1, "the continuation's own trace");
        assert_eq!(
            released.dispatches[0].origin,
            Origin::Module(DIR.into()),
            "released on the parent target's module lane"
        );

        // both writes committed: parent then continuation, one block.
        assert_eq!(dir_get(node.host(), "a").await.as_deref(), Some("1"));
        assert_eq!(dir_get(node.host(), "b").await.as_deref(), Some("2"));
        // the boundary block also carried the System Advance injection.
        let system = node.take_system_dispatches();
        assert!(
            system.iter().any(|(h, d)| *h == 2 && !d.is_empty()),
            "the flag-day block injects the boundary Advance: {system:?}"
        );
    });
}

// (8) determinism: two nodes fed the IDENTICAL v3-carrying batch drain the
// identical app-hash, dispositions, and continuation traces.
#[test]
fn identical_v3_batches_drain_identically_on_two_nodes() {
    block_on(async {
        // active from genesis: every height runs v4.
        let mut n1 = OrderedNode::new(gated_host(0), RoundOrderer::new());
        let mut n2 = OrderedNode::new(gated_host(0), RoundOrderer::new());

        let batch = node::encode_batch(&[
            node::encode_frame(&sk(11), 0, &dir_set("x", "9")),
            v3_envelope(&sk(12), 0),
        ]);
        for n in [&mut n1, &mut n2] {
            n.orderer_mut().submit(batch.clone()).await.expect("propose");
            while n.drain_delivered().await.expect("drain") != 0 {}
        }

        assert_eq!(n1.app_hash(), n2.app_hash(), "identical app-hashes");
        let (d1, d2) = (n1.take_drained(), n2.take_drained());
        assert_eq!(d1.len(), d2.len());
        for (a, b) in d1.iter().zip(&d2) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.disposition, b.disposition);
            assert_eq!(a.app_hash, b.app_hash);
            let (oa, ob) = (a.op.as_ref().expect("op"), b.op.as_ref().expect("op"));
            assert_eq!(oa.dispatches, ob.dispatches, "identical member traces");
            match (&oa.continuation, &ob.continuation) {
                (None, None) => {}
                (Some(ca), Some(cb)) => {
                    assert_eq!(ca.target, cb.target);
                    assert_eq!(ca.disposition, cb.disposition);
                    assert_eq!(ca.dispatches, cb.dispatches, "identical continuation traces");
                }
                other => panic!("continuation presence diverged: {other:?}"),
            }
        }
        assert!(
            d2.iter().any(|d| d
                .op
                .as_ref()
                .is_some_and(|op| op.continuation.is_some())),
            "the v3 member's continuation surfaced on both nodes"
        );
    });
}
