//! the recovery loop end-to-end, minus networking: an ordered node journals
//! through a real [`recovery::Recovery`] store (deterministic runtime), the
//! process "crashes" (everything in memory is dropped), and boot rebuilds the
//! host from the checkpoint + journal suffix to the byte-identical app-hash.
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
    Msg {
        target: "directory".into(),
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
    let reply = host
        .query(
            "directory",
            &encode_query(&DirQuery::Get { key: key.into() }),
        )
        .await
        .expect("query");
    match decode_reply(&reply).expect("decode") {
        DirReply::Value(v) => v,
    }
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
        let genesis_hash = host.app_hash();
        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);

        // genesis checkpoint: height 0 = nothing applied.
        let pos = node.sink_mut().oplog_pos().await;
        let manifest =
            Manifest::capture(node.host(), None, 0, 0, vec![], vec![], None, pos, 1).expect("capture");
        assert_eq!(manifest.app_hash, genesis_hash);
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
        let tip_hash = node.app_hash();

        // ---- a graceful shutdown: the journal tail is made durable ---------
        node.sink_mut().sync().await.expect("shutdown sync");
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
            recovered.app_hash, tip_hash,
            "recomposed app-hash is byte-identical"
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
                app_hash: recovered.app_hash,
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
        let frame = node::encode_batch(&[node::encode_frame(&signer, 1, &set("b", "2"), None)]);
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
        assert_eq!(again.app_hash, recovered.app_hash);
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
        let frame2 = node::encode_batch(&[node::encode_frame(&signer, 2, &set("k2", "v2"), None)]);
        let frame3 = node::encode_batch(&[node::encode_frame(
            &signer,
            3,
            &Msg {
                target: "no-such-module".into(),
                payload: vec![1],
            },
        None,
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
        let frame = node::encode_batch(&[node::encode_frame(&signer, 9, &set("k9", "v9"), None)]);
        pruned.pre_apply(2, &frame).await.expect("wal record");
        pruned
            .seal(&node::BlockSeal {
                height: 2,
                disposition: Disposition::Applied,
                roots: vec![],
                app_hash: sdk::StateRoot([0u8; 32]),
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
