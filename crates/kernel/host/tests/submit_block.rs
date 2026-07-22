//! the batch-apply API: [`Host::submit_block`] applies a batch of ops as ONE
//! block with per-op isolation, a SINGLE commit boundary, and ONE post-batch
//! root-hash shared by every applied member. four properties:
//!
//! 1. per-op isolation: a batch `[A, <reject>, C]` applies A and C and rejects
//!    the middle op, landing committed state byte-identical to applying A then C
//!    in isolation, under ONE shared post-batch root-hash;
//! 2. an all-reject batch commits nothing — root-hash unchanged from pre-batch;
//! 3. a batch-of-one equals `submit_at` of that op (same root-hash + roots);
//! 4. an empty batch is an empty block — root-hash unchanged when nothing pending.
//!
//! the `directory` module is an in-memory, content-addressed, staging module
//! (stage on `execute`, merge on `commit_block`, discard on `abort_block`), so a
//! `DirMsg::Set` is a deterministically-APPLYING op. an op whose target is not
//! registered fails the drain on the remove with `Error::UnknownModule` — a
//! deterministically-REJECTING op (the same shape the node's heartbeat nop uses).

use directory::{DirMsg, Directory, encode_msg as dir_encode};
use futures::executor::block_on;
use host::{BlockContext, Host, MemberOutcome};
use sdk::{Msg, Origin};

const DIR: &str = "directory";

fn set(key: &str, value: &str) -> Msg {
    Msg {
        target: DIR.into(),
        payload: dir_encode(&DirMsg::Set {
            key: key.into(),
            value: value.into(),
        }),
    }
}

// an op targeting a module that isn't registered: the drain fails on the remove
// with `Error::UnknownModule` — a DETERMINISTIC rejection.
fn reject() -> Msg {
    Msg {
        target: "no-such-module".into(),
        payload: Vec::new(),
    }
}

fn ext() -> Origin {
    Origin::External(Vec::new())
}

fn host() -> Host {
    Host::genesis(vec![Box::new(Directory::new(DIR))]).expect("genesis")
}

// (1) a middle op that rejects deterministically leaves the committed state
// identical to applying only the surviving ops, in order, in isolation — under
// exactly one shared post-batch root-hash.
#[test]
fn submit_block_per_op_isolation() {
    block_on(async {
        let mut batch = host();
        let out = batch
            .submit_block(
                BlockContext::default(),
                vec![
                    (ext(), set("a", "1")),
                    (ext(), reject()),
                    (ext(), set("b", "2")),
                ],
            )
            .await
            .expect("the batch itself applies; per-op verdicts live in members");

        // members mirror input order: applied, rejected, applied.
        assert_eq!(out.members.len(), 3);
        assert!(
            matches!(out.members[0], MemberOutcome::Applied { .. }),
            "op0 applies"
        );
        assert!(
            matches!(out.members[1], MemberOutcome::Rejected { .. }),
            "op1 rejects deterministically"
        );
        assert!(
            matches!(out.members[2], MemberOutcome::Applied { .. }),
            "op2 applies"
        );

        // reference: submit_at(Set a=1) then submit_at(Set b=2), each in isolation.
        let mut reference = host();
        reference
            .submit_at(BlockContext::default(), set("a", "1"))
            .await
            .expect("apply a");
        let ref_out = reference
            .submit_at(BlockContext::default(), set("b", "2"))
            .await
            .expect("apply b");

        // every module root is byte-identical to the isolated-application reference.
        assert_eq!(
            batch.module_root(DIR).unwrap(),
            reference.module_root(DIR).unwrap(),
            "batch directory root must equal applying a then b in isolation"
        );
        // exactly one post-batch root-hash, equal to the reference's, recompute-stable.
        assert_eq!(
            out.root_hash, ref_out.root_hash,
            "the one batch root-hash equals the reference root-hash"
        );
        assert_eq!(batch.root_hash(), reference.root_hash());
        assert_eq!(
            out.root_hash,
            batch.root_hash(),
            "the returned root-hash is recompute-stable"
        );
    });
}

// (2) an all-reject batch commits nothing — every member rejects and the
// root-hash is byte-identical to pre-batch.
#[test]
fn submit_block_all_reject() {
    block_on(async {
        let mut host = host();
        let app0 = host.root_hash();

        let out = host
            .submit_block(
                BlockContext::default(),
                vec![(ext(), reject()), (ext(), reject()), (ext(), reject())],
            )
            .await
            .expect("an all-reject batch still commits an (empty) block");

        assert_eq!(out.members.len(), 3);
        assert!(
            out.members
                .iter()
                .all(|m| matches!(m, MemberOutcome::Rejected { .. })),
            "every member rejected"
        );
        assert_eq!(
            out.root_hash, app0,
            "no member applied — root-hash unchanged from pre-batch"
        );
        assert_eq!(host.root_hash(), app0);
    });
}

// (3) the batch-of-one path equals the single-op path: same root-hash and same
// committed roots as `submit_at` of that op.
#[test]
fn submit_block_single_matches_submit_at() {
    block_on(async {
        let mut batch = host();
        let out = batch
            .submit_block(BlockContext::default(), vec![(ext(), set("k", "v"))])
            .await
            .expect("batch-of-one applies");
        assert_eq!(out.members.len(), 1);
        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));

        let mut single = host();
        let single_out = single
            .submit_at(BlockContext::default(), set("k", "v"))
            .await
            .expect("single op applies");

        assert_eq!(
            out.root_hash, single_out.root_hash,
            "batch-of-one root-hash == submit_at root-hash"
        );
        assert_eq!(
            batch.module_root(DIR).unwrap(),
            single.module_root(DIR).unwrap(),
            "batch-of-one committed root == submit_at committed root"
        );
        assert_eq!(batch.root_hash(), single.root_hash());
    });
}

// (4) an empty batch is an empty block: no members, and with nothing pending to
// inject the root-hash is unchanged.
#[test]
fn submit_block_empty_is_empty_block() {
    block_on(async {
        let mut host = host();
        let app0 = host.root_hash();

        let out = host
            .submit_block(BlockContext::default(), vec![])
            .await
            .expect("an empty batch commits an empty block");

        assert!(out.members.is_empty(), "no ops -> no members");
        assert!(out.events.is_empty());
        assert!(
            out.system_dispatches.is_empty(),
            "no upgrade/dispatch modules -> nothing injected"
        );
        assert_eq!(
            out.root_hash, app0,
            "no ops, no injections — root-hash unchanged (an empty block)"
        );
        assert_eq!(host.root_hash(), app0);
    });
}
