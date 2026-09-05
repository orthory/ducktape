//! the recovery loop end-to-end, minus networking: an ordered node journals
//! through a real [`recovery::Recovery`] store (deterministic runtime), the
//! process "crashes" (everything in memory is dropped), and boot rebuilds the
//! host from the checkpoint + journal suffix to the byte-identical root-hash.
//!
//! directory is the module under test on purpose: it is one of the in-memory
//! canonical-bytes modules that today vanish on restart — exactly the state
//! this machinery exists to bring back.

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use directory::Directory;
use directory::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};
use host::Host;
use node::{Disposition, OrderedNode, RoundOrderer};
use recovery::{Manifest, Recovery};
use sdk::Msg;

fn sk(seed: u64) -> commonware_cryptography::ed25519::PrivateKey {
    use commonware_cryptography::Signer as _;
    commonware_cryptography::ed25519::PrivateKey::from_seed(seed)
}

fn set(key: &str, value: &str) -> Msg {
    set_on("directory", key, value)
}

/// a directory `Set` addressed to `target` — a second directory instance
/// stands in for a post-genesis admission below.
fn set_on(target: &str, key: &str, value: &str) -> Msg {
    Msg {
        target: target.into(),
        payload: encode_msg(&DirMsg::Set {
            key: key.into(),
            value: value.into(),
        }),
    }
}

fn fresh_host() -> Host {
    Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis")
}

async fn get(host: &Host, key: &str) -> Option<String> {
    get_on(host, "directory", key).await
}

async fn get_on(host: &Host, target: &str, key: &str) -> Option<String> {
    let reply = host
        .query(target, &encode_query(&DirQuery::Get { key: key.into() }))
        .await
        .expect("query");
    match decode_reply(&reply).expect("decode") {
        DirReply::Value(v) => v,
    }
}

/// a module admitted AFTER the checkpoint: the manifest never captured it, and
/// the composer starts it FRESH (its first activation is past the checkpoint
/// height). the first block that touches it must replay from that empty
/// pre-root — not be classed torn because the manifest holds no root for it.
///
/// the module's op must be the FIRST thing sealed after the checkpoint: every
/// seal records every host root, so an earlier replayed block would carry
/// `later: empty` in its seal, be classed `at_post` for it, and recovery's
/// own post-block bookkeeping would seed the very root under test before the
/// op block arrived — green with or without the seed. with the op in the first
/// block, `changed == {later}` and only the seed makes it `at_pre`.
#[test]
fn an_adopted_empty_module_replays_its_first_op_from_the_empty_pre_root() {
    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        // ---- the first run: the checkpoint predates `later` -----------------
        let recovery = Recovery::open(context.child("r1"))
            .await
            .expect("open recovery");
        let live = Host::genesis(vec![
            Box::new(Directory::new("directory")),
            Box::new(Directory::new("later")),
        ])
        .expect("genesis");
        let mut node = OrderedNode::with_sink(live, RoundOrderer::new(), recovery);
        // the genesis checkpoint, captured from a host WITHOUT `later` —
        // exactly what a checkpoint taken before an admission holds.
        let pos = node.sink_mut().oplog_pos().await;
        let manifest = Manifest::capture(&fresh_host(), None, 0, 0, vec![], vec![], None, pos, 1)
            .expect("capture");
        node.sink_mut()
            .write_manifest(&manifest)
            .await
            .expect("write manifest");

        let signer = sk(1);
        // `later`'s first op, in the first block after the checkpoint: the
        // block a live admission would have activated it in.
        node.submit(&signer, 0, set_on("later", "k1", "v1"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        node.submit(&signer, 1, set("k0", "v0"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 2);
        let tip = node.finalized().expect("boundary");
        let tip_hash = node.root_hash();
        // shutdown with NO explicit barrier: `seal` fsyncs where it is written,
        // so the tip is durable already. deleting this line is the regression
        // proof — without the seal's own sync the replay below loses the tip.
        drop(node);

        // ---- boot: `later` is adopted empty, absent from the manifest -------
        let mut recovery = Recovery::open(context.child("r2"))
            .await
            .expect("reopen recovery");
        let manifest = recovery
            .manifest()
            .expect("manifest decodes")
            .expect("manifest present");
        assert!(
            manifest.root("later").is_none(),
            "the checkpoint predates the admission"
        );
        let mut host = fresh_host();
        host.register(Box::new(Directory::new("later")));

        let recovered = recovery
            .recover(&mut host, &manifest)
            .await
            .expect("the adopting block replays from the empty pre-root");
        assert_eq!(recovered.height, Some(tip.height));
        assert_eq!(
            recovered.root_hash, tip_hash,
            "recomposed root-hash is byte-identical"
        );
        assert_eq!(recovered.applied, 2, "both blocks re-applied");
        assert_eq!(get_on(&host, "later", "k1").await.as_deref(), Some("v1"));
    });
}

#[test]
fn state_survives_a_crash_and_replays_to_the_sealed_tip() {
    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        // ---- the "first run": genesis, ops, a checkpoint, more ops --------
        let recovery = Recovery::open(context.child("r1"))
            .await
            .expect("open recovery");
        let host = fresh_host();
        let genesis_hash = host.root_hash();
        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);

        // genesis checkpoint: height 0 = nothing applied.
        let pos = node.sink_mut().oplog_pos().await;
        let manifest =
            Manifest::capture(node.host(), None, 0, 0, vec![], vec![], None, pos, 1).expect("capture");
        assert_eq!(manifest.root_hash, genesis_hash);
        node.sink_mut()
            .write_manifest(&manifest)
            .await
            .expect("write manifest");

        // two ops before the checkpoint...
        let signer = sk(1);
        node.submit(&signer, 0, set("k0", "v0"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        node.submit(&signer, 1, set("k1", "v1"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 2);
        let checkpoint_height = node.finalized().expect("boundary").height;

        // ...checkpoint...
        let pos = node.sink_mut().oplog_pos().await;
        let manifest = Manifest::capture(
            node.host(),
            Some(checkpoint_height),
            0,
            0,
            vec![],
            vec![],
            None,
            pos,
            2,)
        .expect("capture");
        node.sink_mut()
            .write_manifest(&manifest)
            .await
            .expect("write manifest");

        // ...and two more ops the checkpoint does NOT cover (the journal
        // suffix replay has to bring these back), one of them a REJECTED op
        // (unknown module target -> deterministic no-op, sealed as such).
        node.submit(&signer, 2, set("k2", "v2"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        node.submit(
            &signer,
            3,
            Msg {
                target: "no-such-module".into(),
                payload: vec![1],
            },
        )
        .await
        .expect("submit");
        node.flush_batch().await.expect("flush");
        node.submit(&signer, 4, set("k0", "v0-final"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 3);
        let tip = node.finalized().expect("boundary");
        let tip_hash = node.root_hash();

        // ---- shutdown, no explicit barrier: the seal fsync'd itself --------
        drop(node);

        // ---- boot: reopen the store, restore the checkpoint, replay -------
        let mut recovery = Recovery::open(context.child("r2"))
            .await
            .expect("reopen recovery");
        let manifest = recovery
            .manifest()
            .expect("manifest decodes")
            .expect("manifest present");
        assert_eq!(manifest.height, Some(checkpoint_height));

        // the in-memory module restores from the checkpoint snapshot — the
        // disk substrates would reopen themselves here.
        let mut directory = Directory::new("directory");
        directory
            .install(
                manifest.snapshot("directory").expect("directory snapshot"),
                manifest.root("directory").expect("directory root"),
            )
            .expect("install");
        let mut host = Host::genesis(vec![Box::new(directory)]).expect("genesis");

        // pre-replay: the restored host is AT the checkpoint, not the tip.
        assert_eq!(get(&host, "k0").await.as_deref(), Some("v0"));
        assert_eq!(get(&host, "k2").await, None);

        let recovered = recovery
            .recover(&mut host, &manifest)
            .await
            .expect("recover");
        assert_eq!(recovered.height, Some(tip.height));
        assert_eq!(
            recovered.root_hash, tip_hash,
            "recomposed root-hash is byte-identical"
        );
        assert_eq!(
            recovered.applied, 2,
            "the two post-checkpoint applied ops replayed"
        );
        assert_eq!(
            recovered.skipped, 1,
            "the rejected op is skipped (nothing to redo)"
        );
        assert!(
            !recovered.rolled_forward,
            "clean shutdown leaves no unsealed block"
        );
        assert!(
            !recovered.frames.is_empty(),
            "journaled frames surface for content-store seeding"
        );

        // the recovered state answers queries at the tip.
        assert_eq!(get(&host, "k0").await.as_deref(), Some("v0-final"));
        assert_eq!(get(&host, "k1").await.as_deref(), Some("v1"));
        assert_eq!(get(&host, "k2").await.as_deref(), Some("v2"));

        // and the ordered lane resumes from the recovered boundary: history
        // re-reported at or below the tip is skipped, new ops apply above it.
        // the orderer resumes its view counter past the tip, as a real engine
        // does when it reopens its journal.
        let tip_height = recovered.height.expect("blocks were applied");
        let mut node = OrderedNode::resume(
            host,
            RoundOrderer::resume_at(tip_height - recovered.view_base + 1),
            recovery,
            Some(host::FinalizedBlock {
                height: tip_height,
                root_hash: recovered.root_hash,
            }),
            recovered.view_base,
        );
        node.submit(&signer, 5, set("k3", "v3"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        node.drain_delivered().await.expect("drain");
        assert_eq!(get(node.host(), "k3").await.as_deref(), Some("v3"));
    });
}

#[test]
fn a_crash_mid_apply_rolls_the_unsealed_block_forward() {
    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        // first run: one applied+sealed op, then a WAL record whose apply
        // "never happened" (simulated by journaling the block record directly
        // — the crash point is after pre_apply, before the host mutated).
        let mut recovery = Recovery::open(context.child("r3"))
            .await
            .expect("open recovery");
        let host = fresh_host();
        let manifest = Manifest::capture(&host, None, 0, 0, vec![], vec![], None, 0, 1).expect("capture");
        recovery
            .write_manifest(&manifest)
            .await
            .expect("write manifest");

        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);
        let signer = sk(1);
        node.submit(&signer, 0, set("a", "1"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        let sealed_height = node.finalized().expect("boundary").height;

        // the torn write: pre_apply lands, the apply does not. the journaled
        // block record is a BATCH super-frame (single member here), exactly what
        // the live drain pins before it mutates state.
        let frame = node::encode_batch(&[node::encode_frame(&signer, 1, &set("b", "2"))]);
        {
            use node::BlockSink as _;
            node.sink_mut()
                .pre_apply(sealed_height + 1, &frame)
                .await
                .expect("wal record");
        }
        drop(node);

        // boot: the trailing unsealed block rolls forward and gets sealed.
        let mut recovery = Recovery::open(context.child("r4")).await.expect("reopen");
        let manifest = recovery.manifest().expect("decodes").expect("present");
        let mut host = fresh_host();
        let recovered = recovery
            .recover(&mut host, &manifest)
            .await
            .expect("recover");
        assert!(
            recovered.rolled_forward,
            "the unsealed tip block was rolled forward"
        );
        assert_eq!(recovered.height, Some(sealed_height + 1));
        assert_eq!(get(&host, "a").await.as_deref(), Some("1"));
        assert_eq!(get(&host, "b").await.as_deref(), Some("2"));

        // a SECOND boot over the same journal is idempotent: the roll-forward
        // sealed the block, so it now replays as a normal applied block.
        drop(recovery);
        let mut recovery = Recovery::open(context.child("r5"))
            .await
            .expect("reopen again");
        let manifest = recovery.manifest().expect("decodes").expect("present");
        let mut host = fresh_host();
        let again = recovery
            .recover(&mut host, &manifest)
            .await
            .expect("recover again");
        assert!(!again.rolled_forward);
        assert_eq!(again.height, recovered.height);
        assert_eq!(again.root_hash, recovered.root_hash);
    });
}

#[test]
fn a_journal_without_a_manifest_is_damage_not_a_fresh_dir() {
    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        let mut recovery = Recovery::open(context.child("r6")).await.expect("open");
        assert!(recovery.journal_is_empty().await, "fresh dir");
        assert!(
            recovery.manifest().expect("decodes").is_none(),
            "no manifest yet"
        );

        // journal something without ever writing a manifest.
        {
            use node::BlockSink as _;
            recovery.pin(b"orphan frame").await.expect("pin");
        }
        drop(recovery);

        let recovery = Recovery::open(context.child("r7")).await.expect("reopen");
        assert!(!recovery.journal_is_empty().await);
        assert!(
            recovery.manifest().expect("decodes").is_none(),
            "boot must treat journal-without-manifest as damaged state"
        );
    });
}

#[test]
fn recovery_range_read_returns_sealed_suffix_and_reports_pruned_boundary() {
    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        let recovery = Recovery::open(context.child("range1"))
            .await
            .expect("open recovery");
        let host = fresh_host();
        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);
        let signer = sk(9);

        node.submit(&signer, 0, set("k0", "v0"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        node.submit(&signer, 1, set("k1", "v1"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 2);
        let checkpoint_height = node.finalized().expect("boundary").height;
        assert_eq!(checkpoint_height, 1);

        let pos = node.sink_mut().oplog_pos().await;
        let manifest = Manifest::capture(
            node.host(),
            Some(checkpoint_height),
            0,
            0,
            vec![],
            vec![],
            None,
            pos,
            2,)
        .expect("capture");
        node.sink_mut()
            .write_manifest(&manifest)
            .await
            .expect("write manifest");

        // each op is flushed as its OWN single-member batch, so the finalized
        // block bytes read back are the BATCH super-frame wrapping that member.
        let frame2 = node::encode_batch(&[node::encode_frame(&signer, 2, &set("k2", "v2"))]);
        let frame3 = node::encode_batch(&[node::encode_frame(
            &signer,
            3,
            &Msg {
                target: "no-such-module".into(),
                payload: vec![1],
            },
        )]);
        node.submit(&signer, 2, set("k2", "v2"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        node.submit(
            &signer,
            3,
            Msg {
                target: "no-such-module".into(),
                payload: vec![1],
            },
        )
        .await
        .expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 2);

        let frames = node
            .sink_mut()
            .read_finalized_frames(checkpoint_height, 3)
            .await
            .expect("range read");
        assert_eq!(frames.len(), 2);
        // the round's two single-member batches deliver in the orderer's
        // deterministic BYTE sort, which now ranks BATCH super-frames (not the
        // raw member frames): batch(no-such-module) sorts before batch(k2) at the
        // member-length varint (143 < 170), so the rejected op lands at height 2
        // and the applied op at height 3 — the reverse of the raw-frame order.
        assert_eq!(frames[0].height, 2);
        assert_eq!(frames[0].frame, frame3);
        assert_eq!(frames[0].disposition, Disposition::Rejected);
        assert_eq!(frames[1].height, 3);
        assert_eq!(frames[1].frame, frame2);
        assert_eq!(frames[1].disposition, Disposition::Applied);

        // a checkpoint alone does NOT refuse: the journal still physically
        // holds block 1, and the retention floor is the journal's own — not
        // the manifest height, which advances even when pruning is deferred
        // (the sync retention lease would otherwise be useless).
        let still_served = node
            .sink_mut()
            .read_finalized_frames(0, 3)
            .await
            .expect("frames below the checkpoint are served while retained");
        assert_eq!(still_served.len(), 3);
        assert_eq!(still_served[0].height, 1);

        let _ = pos;
    });
}

#[test]
fn range_read_refuses_below_the_retained_floor() {
    // a journal whose first retained block is height 2 — a pruned prefix's
    // exact shape (journal pruning is section-granular, so a small journal
    // cannot be pruned in place; this constructs the post-prune shape
    // directly). below the floor is a genuine gap, and the reported floor is
    // the lowest anchorable height (first retained - 1). its own runner:
    // deterministic storage partitions are global per runner, so a second
    // Recovery in the same test would share the first one's journal.
    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        use node::BlockSink as _;
        let mut pruned = Recovery::open(context.child("floor"))
            .await
            .expect("open pruned-shape recovery");
        let signer = sk(9);
        let frame = node::encode_batch(&[node::encode_frame(&signer, 9, &set("k9", "v9"))]);
        pruned.pre_apply(2, &frame).await.expect("wal record");
        pruned
            .seal(&node::BlockSeal {
                height: 2,
                disposition: Disposition::Applied,
                roots: vec![],
                root_hash: sdk::StateRoot([0u8; 32]),
            })
            .await
            .expect("seal");
        let err = pruned
            .read_finalized_frames(0, 2)
            .await
            .expect_err("range below the retained floor is refused");
        assert!(matches!(
            err,
            recovery::Error::RangePruned {
                after_height: 0,
                retained_start: 1
            }
        ));
        // at the floor itself the journal serves: the gap is real, not a
        // manifest-proxy refusal.
        let frames = pruned
            .read_finalized_frames(1, 2)
            .await
            .expect("the retained frame serves");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].height, 2);
    });
}

/// THE REPLAY WINDOW IS PERSISTED STATE, NOT UPTIME. the journal suffix a
/// checkpoint leaves behind is shallower than the protocol window on purpose
/// — that is what checkpointing is for — so a restart that rebuilt the guard
/// from the suffix alone would refuse fewer replayed batches than a peer that
/// never restarted, and the two fork on the first batch in the difference.
/// the checkpoint carries the window; the suffix extends it, one entry per
/// height.
#[test]
fn recovery_restores_the_checkpoint_window_and_extends_it_with_the_suffix() {
    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        let recovery = Recovery::open(context.child("w1"))
            .await
            .expect("open recovery");
        let mut node = OrderedNode::with_sink(fresh_host(), RoundOrderer::new(), recovery);
        let signer = sk(3);

        // one block, then a checkpoint carrying a window whose ids are
        // SENTINELS: nothing in this journal can produce them, so finding one
        // in the restored window proves it came off the checkpoint.
        node.submit(&signer, 0, set("k0", "v0"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        let checkpoint_height = node.finalized().expect("boundary").height;
        let inherited = vec![(checkpoint_height, [0xC1; 32])];

        let pos = node.sink_mut().oplog_pos().await;
        let manifest = Manifest::capture(
            node.host(),
            Some(checkpoint_height),
            0,
            0,
            vec![],
            vec![],
            None,
            pos,
            1,
        )
        .expect("capture")
        .with_replay_window(inherited.clone());
        node.sink_mut()
            .write_manifest(&manifest)
            .await
            .expect("write manifest");

        // two more blocks above the checkpoint: the suffix the replay walks.
        node.submit(&signer, 1, set("k1", "v1"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        node.submit(&signer, 2, set("k2", "v2"))
            .await
            .expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 2);
        let tip_height = node.finalized().expect("boundary").height;
        let live_window = node.replay_window();
        drop(node);

        let mut recovery = Recovery::open(context.child("w2"))
            .await
            .expect("reopen recovery");
        let restored = recovery
            .manifest()
            .expect("manifest decodes")
            .expect("manifest present");
        assert_eq!(
            restored.applied_frames, inherited,
            "the window survives the checkpoint codec"
        );

        let mut directory = Directory::new("directory");
        directory
            .install(
                restored.snapshot("directory").expect("directory snapshot"),
                restored.root("directory").expect("directory root"),
            )
            .expect("install");
        let mut host = Host::genesis(vec![Box::new(directory)]).expect("genesis");
        let recovered = recovery
            .recover(&mut host, &restored)
            .await
            .expect("replay the suffix");

        // the checkpoint's entry is IN the restored window, sentinel id and
        // all — and it did not double up with the same height the retained
        // journal still holds.
        assert!(
            recovered
                .applied_frames
                .contains(&(checkpoint_height, [0xC1; 32])),
            "the checkpoint window seeds the restored one"
        );
        let heights: Vec<u64> = recovered.applied_frames.iter().map(|(h, _)| *h).collect();
        let mut one_per_height = heights.clone();
        one_per_height.sort_unstable();
        one_per_height.dedup();
        assert_eq!(
            heights, one_per_height,
            "ascending, one entry per height — a duplicate costs a window slot"
        );
        // and every block the suffix walked extends it, up to the tip.
        for (height, id) in live_window {
            if height > checkpoint_height {
                assert!(
                    recovered.applied_frames.contains(&(height, id)),
                    "the suffix block at {height} extends the restored window"
                );
            }
        }
        assert_eq!(heights.last().copied(), Some(tip_height));
    });
}
