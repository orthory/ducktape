use super::*;
use crate::sync::catchup::write_post_reboot_catchup_checkpoint;
use commonware_cryptography::ed25519;
use directory::{DirMsg, Directory, encode_msg};
use host::Host;
use recovery::Manifest;
use sdk::{Ctx, Error, Module, StateSyncHandle};
use std::sync::{Arc, Mutex};

#[test]
fn gateway_requires_a_loopback_node_api_and_real_overlay() {
    let wireguard = Some("127.0.0.1:51820".parse().unwrap());
    assert!(gateway_can_start(
        false,
        Some("127.0.0.1:0"),
        Some("127.0.0.1:8844"),
        wireguard,
    ));
    for allowed in [
        gateway_can_start(false, Some("127.0.0.1:0"), Some("0.0.0.0:8844"), wireguard),
        gateway_can_start(true, Some("127.0.0.1:0"), Some("127.0.0.1:8844"), wireguard),
        gateway_can_start(false, Some("127.0.0.1:0"), Some("127.0.0.1:8844"), None),
    ] {
        assert!(!allowed);
    }
}

fn test_root(byte: u8) -> StateRoot {
    StateRoot([byte; sdk::ROOT_LEN])
}

fn test_me() -> Vec<u8> {
    vec![1u8; 32]
}

fn test_manifest(
    height: u64,
    root_hash: StateRoot,
    floor_cert: Option<Vec<u8>>,
) -> statesync::Manifest {
    statesync::Manifest {
        height,
        root_hash,
        epoch: 0,
        view_base: 0,
        participants: vec![test_me()],
        residents: vec![],
        floor_cert,
        entries: vec![],
    }
}

fn test_manifest_with_participants(
    height: u64,
    root_hash: StateRoot,
    floor_cert: Option<Vec<u8>>,
    participants: Vec<Vec<u8>>,
) -> statesync::Manifest {
    statesync::Manifest {
        participants,
        ..test_manifest(height, root_hash, floor_cert)
    }
}

fn test_manifest_with_base(
    height: u64,
    view_base: u64,
    root_hash: StateRoot,
    floor_cert: Option<Vec<u8>>,
) -> statesync::Manifest {
    statesync::Manifest {
        view_base,
        ..test_manifest(height, root_hash, floor_cert)
    }
}

fn fresh_directory_host() -> Host {
    Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis")
}

#[derive(Clone, Default)]
struct TestDiskStore(Arc<Mutex<u8>>);

impl TestDiskStore {
    fn get(&self) -> u8 {
        *self.0.lock().expect("test disk store lock")
    }

    fn set(&self, value: u8) {
        *self.0.lock().expect("test disk store lock") = value;
    }
}

struct TestDiskModule {
    store: TestDiskStore,
    staged: Option<u8>,
}

impl TestDiskModule {
    fn new(store: TestDiskStore) -> Self {
        Self {
            store,
            staged: None,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for TestDiskModule {
    fn id(&self) -> String {
        "disk".into()
    }

    fn root(&self) -> StateRoot {
        test_root(self.store.get())
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::ResolverBacked {
            backend: "test-disk".into(),
            detail: "test disk module reopens from shared durable state".into(),
        })
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let value = *msg
            .payload
            .first()
            .ok_or_else(|| Error::Module("missing test value".into()))?;
        self.staged = Some(value);
        ctx.emit_msg(Msg {
            target: "mem".into(),
            payload: vec![value],
        });
        Ok(())
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        if let Some(value) = self.staged.take() {
            self.store.set(value);
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged = None;
        Ok(())
    }
}

struct TestMemoryModule {
    value: u8,
    staged: Option<u8>,
}

impl TestMemoryModule {
    fn new(value: u8) -> Self {
        Self {
            value,
            staged: None,
        }
    }

    fn install(&mut self, bytes: &[u8], root: StateRoot) -> Result<(), Error> {
        let [value] = bytes else {
            return Err(Error::Module("bad test memory snapshot".into()));
        };
        if test_root(*value) != root {
            return Err(Error::Module("test memory root mismatch".into()));
        }
        self.value = *value;
        self.staged = None;
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for TestMemoryModule {
    fn id(&self) -> String {
        "mem".into()
    }

    fn root(&self) -> StateRoot {
        test_root(self.value)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(vec![self.value]))
    }

    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let value = *msg
            .payload
            .first()
            .ok_or_else(|| Error::Module("missing test value".into()))?;
        self.staged = Some(value);
        Ok(())
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        if let Some(value) = self.staged.take() {
            self.value = value;
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged = None;
        Ok(())
    }
}

fn mixed_durability_host(store: TestDiskStore, memory_value: u8) -> Host {
    Host::genesis(vec![
        Box::new(TestDiskModule::new(store)),
        Box::new(TestMemoryModule::new(memory_value)),
    ])
    .expect("mixed host")
}

fn restore_mixed_durability_host(store: TestDiskStore, manifest: &Manifest) -> Host {
    let mut memory = TestMemoryModule::new(0);
    memory
        .install(
            manifest.snapshot("mem").expect("mem snapshot"),
            manifest.root("mem").expect("mem root"),
        )
        .expect("mem install");
    Host::genesis(vec![Box::new(TestDiskModule::new(store)), Box::new(memory)])
        .expect("restored mixed host")
}

fn dir_set(key: &str, value: &str) -> Msg {
    Msg {
        target: "directory".into(),
        payload: encode_msg(&DirMsg::Set {
            key: key.into(),
            value: value.into(),
        }),
    }
}

async fn dir_value(host: &Host, key: &str) -> Option<String> {
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
fn resident_manifest_fetch_retry_stays_resident_and_does_not_reannounce() {
    let retry = joiner_manifest_fetch_retry(
        "9f7bae44",
        true,
        "server error: no finalized boundary to serve yet",
    );

    assert!(
        !retry.announce,
        "a resident should not re-announce the invite after standing is known"
    );
    assert!(
        retry.log_line.contains("[node 9f7bae44] resident:"),
        "post-standing retry should be logged as resident follow noise: {}",
        retry.log_line
    );
    assert!(
        retry
            .log_line
            .contains("no finalized boundary to serve yet"),
        "the source fetch detail should remain visible: {}",
        retry.log_line
    );
    assert!(
        !retry.log_line.contains("redemption not landed") && !retry.log_line.contains("joining:"),
        "post-standing retry must not look like a pending invite: {}",
        retry.log_line
    );
}

#[test]
fn parked_manifest_fetch_retry_keeps_join_announce() {
    let retry = joiner_manifest_fetch_retry("9f7bae44", false, "server error: bouncer rejected");

    assert!(retry.announce, "a parked joiner must keep re-announcing");
    assert!(
        retry.log_line.contains("[node 9f7bae44] joining:")
            && retry.log_line.contains("redemption not landed")
            && retry.log_line.contains("bouncer rejected"),
        "parked retry should keep the invite wording and source detail: {}",
        retry.log_line
    );
}

async fn served_directory_frame(
    expected: &mut Host,
    signer: &ed25519::PrivateKey,
    height: u64,
    seq: u64,
    msg: Msg,
) -> statesync::FinalizedFrame {
    let frame = node::encode_frame(signer, seq, &msg, None);
    let (origin, msg, _cont) = node::decode_frame(&frame).expect("decode frame");
    // a block is a BATCH super-frame: apply the single member via the batch
    // API and serve the batch bytes, so the catch-up replay reproduces this.
    expected
        .submit_block(
            host::BlockContext {
                height,
                consensus_time: height,
                origin: origin.clone(),
            },
            vec![(origin, msg)],
        )
        .await
        .expect("apply");
    statesync::FinalizedFrame {
        height,
        frame: node::encode_batch(&[frame]),
        disposition: statesync::FrameDisposition::Applied,
        roots: expected.module_roots(),
        root_hash: expected.root_hash(),
    }
}

async fn served_mixed_frame(
    expected: &mut Host,
    signer: &ed25519::PrivateKey,
    height: u64,
    seq: u64,
    value: u8,
) -> statesync::FinalizedFrame {
    let frame = node::encode_frame(
        signer,
        seq,
        &Msg {
            target: "disk".into(),
            payload: vec![value],
        },
        None,
    );
    let (origin, msg, _cont) = node::decode_frame(&frame).expect("decode frame");
    // a block is a BATCH super-frame: apply the single member through the
    // batch API so the served root-hash matches what recovery reproduces on
    // replay (which decodes the frame as a batch), and serve the batch bytes.
    expected
        .submit_block(
            host::BlockContext {
                height,
                consensus_time: height,
                origin: origin.clone(),
            },
            vec![(origin, msg)],
        )
        .await
        .expect("apply mixed frame");
    statesync::FinalizedFrame {
        height,
        frame: node::encode_batch(&[frame]),
        disposition: statesync::FrameDisposition::Applied,
        roots: expected.module_roots(),
        root_hash: expected.root_hash(),
    }
}

#[test]
fn floor_cert_view_must_map_to_boundary_height() {
    assert!(assert_floor_binds_view(30, 36, 6).is_ok());
    assert!(assert_floor_binds_view(30, 36, 4).is_err());
}

#[test]
fn promotion_boundary_prefers_latest_same_state_height() {
    let host_hash = test_root(7);
    let latest = test_manifest(12, host_hash, Some(vec![2]));

    match choose_promotion_boundary(host_hash, &latest, &test_me()) {
        PromotionBoundary::Promote { boundary, source } => {
            assert_eq!(boundary.height, 12);
            assert_eq!(source, PromotionBoundarySource::Latest);
        }
        PromotionBoundary::Retry => panic!("same-state latest boundary should promote"),
    }
}

#[test]
fn promotion_boundary_retries_when_latest_excludes_self() {
    let host_hash = test_root(7);
    let me = vec![1u8; 32];
    let latest = test_manifest_with_participants(12, host_hash, Some(vec![2]), vec![vec![9u8; 32]]);

    match choose_promotion_boundary(host_hash, &latest, &me) {
        PromotionBoundary::Retry => {}
        PromotionBoundary::Promote { .. } => {
            panic!("latest boundary excluding this node must not promote")
        }
    }
}

#[test]
fn promotion_boundary_accepts_latest_at_view_base_without_floor() {
    let host_hash = test_root(7);
    let latest = test_manifest_with_base(12, 12, host_hash, None);

    match choose_promotion_boundary(host_hash, &latest, &test_me()) {
        PromotionBoundary::Promote { boundary, source } => {
            assert_eq!(boundary.height, 12);
            assert_eq!(source, PromotionBoundarySource::Latest);
        }
        PromotionBoundary::Retry => panic!("view-base latest boundary should promote"),
    }
}

#[test]
fn promotion_boundary_requires_latest_floor_past_view_base() {
    let host_hash = test_root(7);
    let latest = test_manifest_with_base(12, 10, host_hash, None);

    match choose_promotion_boundary(host_hash, &latest, &test_me()) {
        PromotionBoundary::Retry => {}
        PromotionBoundary::Promote { .. } => {
            panic!("past-base latest boundary without a floor should retry")
        }
    }
}

#[test]
fn promotion_boundary_retries_when_latest_changed() {
    let host_hash = test_root(7);
    let latest = test_manifest(12, test_root(9), Some(vec![2]));

    match choose_promotion_boundary(host_hash, &latest, &test_me()) {
        PromotionBoundary::Retry => {}
        PromotionBoundary::Promote { .. } => {
            panic!("changed latest boundary should retry")
        }
    }
}

#[test]
fn promotion_boundary_retries_when_no_manifest_matches_host() {
    let host_hash = test_root(7);
    let latest = test_manifest(12, test_root(9), Some(vec![2]));

    match choose_promotion_boundary(host_hash, &latest, &test_me()) {
        PromotionBoundary::Retry => {}
        PromotionBoundary::Promote { .. } => panic!("changed roots should retry"),
    }
}

#[test]
fn suffix_installer_rejects_mismatched_served_seal() {
    let executor = commonware_runtime::deterministic::Runner::default();
    executor.start(|_| async move {
        let signer = commonware_cryptography::ed25519::PrivateKey::from_seed(77);
        let msg = Msg {
            target: "directory".into(),
            payload: encode_msg(&DirMsg::Set {
                key: "k".into(),
                value: "v".into(),
            }),
        };
        let frame = node::encode_frame(&signer, 0, &msg, None);

        let mut expected_host =
            Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
        let (origin, msg, _cont) = node::decode_frame(&frame).expect("decode frame");
        expected_host
            .submit_at(
                host::BlockContext {
                    height: 1,
                    consensus_time: 1,
                    origin,
                },
                msg,
            )
            .await
            .expect("apply");

        let served = statesync::FinalizedFrame {
            height: 1,
            frame,
            disposition: statesync::FrameDisposition::Applied,
            roots: expected_host.module_roots(),
            root_hash: StateRoot([0xA5; sdk::ROOT_LEN]),
        };
        let mut host = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
        let err = apply_verified_suffix_frame(&mut host, &served, &host::NoCodeSource)
            .await
            .expect_err("served seal mismatch must abort");
        assert!(
            err.contains("served seal"),
            "unexpected mismatch error: {err}"
        );
    });
}

#[test]
fn post_reboot_catchup_applies_verifies_and_journals_served_suffix() {
    let executor = commonware_runtime::deterministic::Runner::default();
    executor.start(|context| async move {
        let signer = ed25519::PrivateKey::from_seed(78);
        let mut expected = fresh_directory_host();
        let frames = vec![
            served_directory_frame(&mut expected, &signer, 1, 0, dir_set("a", "1")).await,
            served_directory_frame(&mut expected, &signer, 2, 1, dir_set("b", "2")).await,
        ];

        let mut host = fresh_directory_host();
        let mut recovery = Recovery::open(context.child("post_catchup_ok"))
            .await
            .expect("open recovery");
        let applied =
            apply_post_reboot_catchup_frames(&mut recovery, &mut host, 0, 2, frames.clone(), None)
                .await
                .expect("catch up");

        assert_eq!(applied.applied, 2);
        assert_eq!(host.root_hash(), expected.root_hash());
        assert_eq!(dir_value(&host, "a").await.as_deref(), Some("1"));
        assert_eq!(dir_value(&host, "b").await.as_deref(), Some("2"));
        let journaled = recovery
            .read_finalized_frames(0, 2)
            .await
            .expect("read frames");
        assert_eq!(journaled.len(), 2);
        assert_eq!(journaled[0].height, 1);
        assert_eq!(journaled[1].height, 2);
    });
}

#[test]
fn post_reboot_catchup_checkpoint_makes_mixed_durability_suffix_recoverable() {
    let executor = commonware_runtime::deterministic::Runner::default();
    executor.start(|context| async move {
        let signer = ed25519::PrivateKey::from_seed(81);
        let durable_store = TestDiskStore::default();
        let base_host = mixed_durability_host(durable_store.clone(), 0);
        let base_manifest =
            Manifest::capture(&base_host, None, 0, 0, vec![test_me()], vec![], None, 0, 1)
        .expect("base manifest");

        let mut expected = mixed_durability_host(TestDiskStore::default(), 0);
        let served = served_mixed_frame(&mut expected, &signer, 1, 0, 7).await;
        let target = statesync::Manifest {
            height: 1,
            root_hash: served.root_hash,
            epoch: 0,
            view_base: 0,
            participants: vec![test_me()],
            residents: vec![],
            floor_cert: Some(vec![1, 2, 3]),
            entries: vec![],
        };

        let mut host = mixed_durability_host(durable_store.clone(), 0);
        let mut recovery = Recovery::open(context.child("post_catchup_mixed"))
            .await
            .expect("open recovery");
        recovery
            .write_manifest(&base_manifest)
            .await
            .expect("write base manifest");
        let applied =
            apply_post_reboot_catchup_frames(&mut recovery, &mut host, 0, 1, vec![served], None)
                .await
                .expect("catch up");

        assert_eq!(applied.applied, 1);
        assert_eq!(
            durable_store.get(),
            7,
            "disk cohort committed the catch-up block durably"
        );

        // an old-base replay reconciles the torn sealed block via selective
        // replay (the still-at-pre memory cohort recommits, the already-
        // durable disk cohort aborts) rather than fail-stopping; the
        // checkpoint's value below is recovering WITHOUT that replay.
        let mut torn_host = restore_mixed_durability_host(durable_store.clone(), &base_manifest);
        let healed = recovery
            .recover(&mut torn_host, &base_manifest)
            .await
            .expect("old base replay heals the torn sealed block selectively");
        assert_eq!(healed.height, Some(1));
        assert_eq!(healed.root_hash, target.root_hash);
        assert_eq!(healed.applied, 1, "the torn suffix frame was replayed");
        assert_eq!(
            durable_store.get(),
            7,
            "disk cohort stays at its durable post-state"
        );

        let ckpt = write_post_reboot_catchup_checkpoint(
            &mut recovery,
            &host,
            Some(&base_manifest),
            &target,
            &applied.blocks,
            1,
        )
        .await
        .expect("write catch-up checkpoint");
        assert_eq!(ckpt.height, Some(1));
        assert_eq!(ckpt.root_hash, target.root_hash);
        assert_eq!(ckpt.snapshot("mem"), Some([7u8].as_slice()));

        let mut restored = restore_mixed_durability_host(durable_store, &ckpt);
        let recovered = recovery
            .recover(&mut restored, &ckpt)
            .await
            .expect("T checkpoint must recover without replaying the torn suffix");
        assert_eq!(recovered.height, Some(1));
        assert_eq!(recovered.root_hash, target.root_hash);
        assert_eq!(recovered.applied, 0);
    });
}

#[test]
fn post_reboot_catchup_aborts_on_mismatched_served_seal() {
    let executor = commonware_runtime::deterministic::Runner::default();
    executor.start(|context| async move {
        let signer = ed25519::PrivateKey::from_seed(79);
        let mut expected = fresh_directory_host();
        let mut served =
            served_directory_frame(&mut expected, &signer, 1, 0, dir_set("a", "1")).await;
        served.root_hash = test_root(0xA5);

        let mut host = fresh_directory_host();
        let mut recovery = Recovery::open(context.child("post_catchup_mismatch"))
            .await
            .expect("open recovery");
        let err =
            apply_post_reboot_catchup_frames(&mut recovery, &mut host, 0, 1, vec![served], None)
                .await
                .expect_err("seal mismatch must abort");

        assert!(
            err.contains("served seal"),
            "unexpected mismatch error: {err}"
        );
    });
}

#[test]
fn post_reboot_catchup_is_noop_when_there_is_no_gap() {
    let executor = commonware_runtime::deterministic::Runner::default();
    executor.start(|context| async move {
        let mut host = fresh_directory_host();
        let before = host.root_hash();
        let mut recovery = Recovery::open(context.child("post_catchup_noop"))
            .await
            .expect("open recovery");
        let applied =
            apply_post_reboot_catchup_frames(&mut recovery, &mut host, 5, 5, Vec::new(), None)
                .await
                .expect("noop catch up");

        assert_eq!(applied.applied, 0);
        assert_eq!(host.root_hash(), before);
        assert!(
            recovery
                .read_finalized_frames(5, 5)
                .await
                .expect("empty range")
                .is_empty()
        );
    });
}

// ---- explorer-row rebuild (boot fold == live drain) ---------------------

fn row_dispatches(payload: &[u8], origin: &sdk::Origin) -> Vec<host::DispatchRecord> {
    vec![host::DispatchRecord {
        module: "directory".into(),
        origin: origin.clone(),
        payload: payload.to_vec(),
        emitted_msgs: 0,
        emitted_events: 0,
    }]
}

/// the boot fold rebuilds a block's per-op rows from its sealed BATCH frame,
/// re-staging each op's payload so `GET /v1/files/blob/{op_hash}` answers
/// again after a restart. the block coordinates and every op's identity
/// (proposer/target/payload/op_hash) match the drain's live row; the only
/// difference is the per-op dispatch TRACE — recovery folds the block-level
/// aggregate, not per-member, so a replayed op carries an empty trace (a
/// documented degradation visible only when the index is rebuilt).
#[test]
fn boot_fold_rebuilds_a_batch_block_ops() {
    let signer = ed25519::PrivateKey::from_seed(42);
    let payload = br#"{"set":{"key":"who","value":"ducktape"}}"#.to_vec();
    let msg = Msg {
        target: "directory".into(),
        payload: payload.clone(),
    };
    let frame = node::encode_frame(&signer, 1, &msg, None);
    let (origin, decoded, _cont) = node::decode_frame(&frame).expect("frame decodes");
    let dispatches = row_dispatches(&payload, &origin);
    let root_hash = test_root(9);

    // the drain's construction: one member op with its full dispatch trace.
    let drain_blobs = blobstore::BlobHandle::default();
    let drain_row = noded::block_row(&noded::BlockRecord {
        height: 7,
        hash: noded::hex_bytes(&node::frame_id(&frame)),
        commit_hash: hex(&root_hash),
        ops: vec![project_root_op(
            &drain_blobs,
            &origin,
            &decoded.target,
            &decoded.payload,
            &dispatches,
            noded::BlockDisposition::Applied,
        )],
    });
    let drain: serde_json::Value = serde_json::from_slice(&drain_row).unwrap();

    // the boot fold's construction: the sealed frame is a BATCH.
    let batch = node::encode_batch(std::slice::from_ref(&frame));
    let fold_blobs = blobstore::BlobHandle::default();
    let fold_row = sealed_frame_block_row(
        &fold_blobs,
        &recovery::FoldedBlock {
            height: 7,
            frame: &batch,
            disposition: node::Disposition::Applied,
            root_hash,
            dispatches: &dispatches,
        },
    )
    .expect("an applied non-nop batch rebuilds its row");
    let row: serde_json::Value = serde_json::from_slice(&fold_row).expect("row json");

    // block coordinates match the drain.
    assert_eq!(row["height"], 7);
    assert_eq!(row["hash"], noded::hex_bytes(&node::frame_id(&batch)));
    assert_eq!(row["commit_hash"], hex(&root_hash));
    assert_eq!(row["ops"].as_array().unwrap().len(), 1);
    // the op's identity matches the drain byte-for-byte.
    assert_eq!(row["ops"][0]["proposer"], drain["ops"][0]["proposer"]);
    assert_eq!(row["ops"][0]["target"], "directory");
    assert_eq!(row["ops"][0]["payload"], drain["ops"][0]["payload"]);
    assert_eq!(row["ops"][0]["op_hash"], drain["ops"][0]["op_hash"]);
    // the fold carries an empty per-op trace (recovery folds the aggregate).
    assert_eq!(row["ops"][0]["operations"].as_array().unwrap().len(), 0);

    // the rebuild re-staged the payload: op_hash is dereferencable again
    // from the FOLD's (fresh, post-restart) blob store.
    let op_digest = drain_blobs.put_chunk(payload.clone());
    assert_eq!(row["ops"][0]["op_hash"], noded::hex_bytes(&op_digest));
    assert!(fold_blobs.has_chunk(&op_digest));
}

/// the fold's `None` gates mirror the drain's: a heartbeat nop and an
/// undecodable frame produce no explorer row (the drain's `op` is `None` /
/// nop-filtered for exactly these).
#[test]
fn boot_fold_skips_nop_and_undecodable_frames() {
    let blobs = blobstore::BlobHandle::default();
    let signer = ed25519::PrivateKey::from_seed(43);
    let nop = node::encode_frame(
        &signer,
        1,
        &Msg {
            target: crate::constants::NOP_TARGET.into(),
            payload: Vec::new(),
        },
        None,
    );
    for frame in [nop.as_slice(), b"not a frame".as_slice()] {
        assert!(
            sealed_frame_block_row(
                &blobs,
                &recovery::FoldedBlock {
                    height: 3,
                    frame,
                    disposition: node::Disposition::Applied,
                    root_hash: test_root(1),
                    dispatches: &[],
                },
            )
            .is_none()
        );
    }
}

/// a REJECTED sealed frame still gets its row (the drain writes one for a
/// decoded non-nop reject), with an empty dispatch trace.
#[test]
fn boot_fold_rebuilds_rejected_rows_with_empty_trace() {
    let blobs = blobstore::BlobHandle::default();
    let signer = ed25519::PrivateKey::from_seed(44);
    let frame = node::encode_frame(
        &signer,
        2,
        &Msg {
            target: "directory".into(),
            payload: b"garbage-the-module-rejects".to_vec(),
        },
        None,
    );
    let batch = node::encode_batch(&[frame]);
    let row = sealed_frame_block_row(
        &blobs,
        &recovery::FoldedBlock {
            height: 5,
            frame: &batch,
            disposition: node::Disposition::Rejected,
            root_hash: test_root(2),
            dispatches: &[],
        },
    )
    .expect("a decoded non-nop reject still shows in the explorer");
    let row: serde_json::Value = serde_json::from_slice(&row).expect("row json");
    assert_eq!(row["ops"][0]["disposition"], "rejected");
    assert_eq!(
        row["ops"][0]["operations"].as_array().map(Vec::len),
        Some(0)
    );
}
