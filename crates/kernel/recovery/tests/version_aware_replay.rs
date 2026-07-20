//! FORK-CRITICAL: recovery replay must reconstruct the byte-identical app-hash
//! across an activation boundary H, not just below it. a dual-path module's
//! `root()` branches on a non-hashed `active_version`, and the effective version
//! for a block is `effective_version(height)` over the committed upgrade state.
//! the live node stamps that per block (and the driver flips `active_version` at
//! H); replay has no driver, so `recover()` must re-derive both from the replayed
//! upgrade state per height — otherwise a node recovering across H recomputes a
//! stale-version root and mass-halts on the app-hash check once forge v2 diverges.
//!
//! this simulates the boundary with two mock modules: `Dual` (a stand-in for
//! forge — its `root()` folds the branch selector) and an `upgrade` module that
//! reports a STATIC armed status so `effective_version(height) = V` at/after H and
//! baseline below it. the assertion is that a checkpoint below H + a journal
//! suffix crossing H recovers to the identical tip app-hash — which only holds
//! because `apply_block` stamps the per-height version and drives `active_version`.

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::{BlockContext, Host};
use node::{OrderedNode, RoundOrderer};
use recovery::{Manifest, Recovery};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle, UpgradeCoords};
use sha2::{Digest, Sha256};

fn sk(seed: u64) -> commonware_cryptography::ed25519::PrivateKey {
    use commonware_cryptography::Signer as _;
    commonware_cryptography::ed25519::PrivateKey::from_seed(seed)
}

// ---- Dual: a dual-path module whose root() branches on active_version --------
// committed state is a single counter; `active_version` is a NON-hashed branch
// selector (never persisted in the snapshot), exactly like forge's. `root()`
// folds BOTH, so replaying at the wrong version yields a different root.
struct Dual {
    id: ModuleId,
    counter: u64,
    pending: Option<u64>,
    active_version: u32,
}

impl Dual {
    fn new() -> Self {
        Self {
            id: "dual".into(),
            counter: 0,
            pending: None,
            active_version: 0,
        }
    }
    fn root_of(counter: u64, active_version: u32) -> StateRoot {
        let mut h = Sha256::new();
        h.update(b"dual:");
        h.update(counter.to_le_bytes());
        // the version-branched preimage: v0 and v1 hash differently for the SAME
        // committed counter — the whole point of a root()-changing upgrade.
        h.update(active_version.to_le_bytes());
        StateRoot(h.finalize().into())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Dual {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }
    fn root(&self) -> StateRoot {
        Dual::root_of(self.counter, self.active_version)
    }
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        // the snapshot is committed-state ONLY (the counter) — active_version is
        // never serialized, mirroring forge.
        Ok(StateSyncHandle::SnapshotBytes(self.counter.to_le_bytes().to_vec()))
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        // any op bumps the counter (staged; published at commit_block).
        self.pending = Some(self.counter + 1);
        Ok(())
    }
    async fn commit_block(&mut self) -> Result<(), Error> {
        if let Some(v) = self.pending.take() {
            self.counter = v;
        }
        Ok(())
    }
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending = None;
        Ok(())
    }
    fn set_active_version(&mut self, version: u32) {
        self.active_version = version;
    }
}

/// A stateless stand-in for a module that joins the registry at a protocol
/// boundary. Its zero state is still app-hash-visible once the id is active.
struct DormantMarker;

#[async_trait::async_trait(?Send)]
impl Module for DormantMarker {
    fn id(&self) -> ModuleId {
        "clients".into()
    }

    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::Stateless)
    }

    async fn execute(&mut self, _ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        Ok(())
    }
}

fn install_dual(bytes: &[u8], expected: StateRoot, active_version: u32) -> Dual {
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&bytes[..8]);
    let counter = u64::from_le_bytes(arr);
    assert_eq!(
        Dual::root_of(counter, active_version),
        expected,
        "installed dual root must match the checkpoint at the boundary version"
    );
    Dual {
        id: "dual".into(),
        counter,
        pending: None,
        active_version,
    }
}

// ---- a static armed upgrade module ------------------------------------------
// reports a fixed pending upgrade at H with the sole member already ready, so
// `lifecycle::effective_version(height, ..)` returns V at/after H and 0
// below it. `root()` is constant (the config never mutates), so it contributes an
// identical, stable root on both the live and the recovered side. the injected
// boundary `Advance` is accepted as a no-op (this mock does not model the flip —
// it keeps the armed predicate true by height alone, which is all replay reads).
struct StaticUpgrade {
    name: String,
    activation_height: u64,
    to_version: u32,
    member: Vec<u8>,
}

#[async_trait::async_trait(?Send)]
impl Module for StaticUpgrade {
    fn id(&self) -> ModuleId {
        "lifecycle".into()
    }
    fn root(&self) -> StateRoot {
        let mut h = Sha256::new();
        h.update(b"static-upgrade:");
        h.update(self.name.as_bytes());
        h.update(self.activation_height.to_le_bytes());
        h.update(self.to_version.to_le_bytes());
        StateRoot(h.finalize().into())
    }
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        // recreated identically on restore; no bytes to transfer.
        Ok(StateSyncHandle::Stateless)
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // the host injects exactly one System-origin `Advance` at each block >= H;
        // accept it as a no-op (this mock arms purely by height).
        match lifecycle::decode_msg(&msg.payload).map_err(Error::Module)? {
            lifecycle::LifecycleMsg::Advance => Ok(()),
            other => Err(Error::Module(format!("static lifecycle got {other:?}"))),
        }
    }
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let lifecycle::LifecycleQuery::UpgradeStatus =
            lifecycle::decode_query(req).map_err(Error::Module)?
        else {
            return Err(Error::QueryUnsupported);
        };
        let status = lifecycle::UpgradeStatus {
            current_version: 0,
            pending: Some(lifecycle::ScheduledUpgrade {
                name: self.name.clone(),
                activation_height: self.activation_height,
                to_version: self.to_version,
            }),
            members: vec![self.member.clone()],
            ready: vec![self.member.clone()],
            member_count: 1,
            ready_count: 1,
            armed: true,
        };
        Ok(lifecycle::encode_reply(
            &lifecycle::LifecycleReply::UpgradeStatus(status),
        ))
    }
}

fn upgrade_mock(member: &[u8], h: u64, v: u32) -> StaticUpgrade {
    StaticUpgrade {
        name: "forge-v2".into(),
        activation_height: h,
        to_version: v,
        member: member.to_vec(),
    }
}

fn op() -> Msg {
    Msg {
        target: "dual".into(),
        payload: vec![],
    }
}

#[test]
fn replay_recomputes_the_identical_app_hash_across_an_armed_boundary() {
    // the deterministic harness seals the first batch of 2 ops at heights 0,1
    // (checkpoint at 1) and the second at heights 2,3 (tip at 3). H = 2 puts the
    // activation boundary exactly at the first post-checkpoint block, so the live
    // flip (below) and `effective_version(height)` agree: blocks 0,1 run v0, blocks
    // 2,3 run v1 — the boundary the replay must reconstruct.
    const H: u64 = 2; // activation height
    const V: u32 = 1; // to_version

    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        let signer = sk(1);
        let me = {
            use commonware_cryptography::Signer as _;
            signer.public_key().as_ref().to_vec()
        };

        // ---- live run through a real recovery store -----------------------
        let recovery = Recovery::open(context.child("r1")).await.expect("open");
        let host = Host::genesis(vec![
            Box::new(Dual::new()),
            Box::new(upgrade_mock(&me, H, V)),
        ])
        .expect("genesis");
        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);

        // two ops BELOW H: baseline version, dual root uses v0.
        node.submit(&signer, 0, op()).await.expect("submit");
        node.flush_batch().await.expect("flush");
        node.submit(&signer, 1, op()).await.expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 2);
        let checkpoint_height = node.finalized().expect("boundary").height;
        assert!(checkpoint_height < H, "checkpoint must sit below H");
        let hash_below = node.app_hash();

        // checkpoint below H: current_version 0, a pending upgrade at H (its
        // required_min_version stays baseline below H).
        let pos = node.sink_mut().oplog_pos().await;
        let manifest = Manifest::capture(
            node.host(),
            Some(checkpoint_height),
            0,
            0,
            vec![],
            vec![],
            None,
            0,
            Some(UpgradeCoords {
                name: "forge-v2".into(),
                activation_height: H,
                to_version: V,
            }),
            pos,
            2,
        )
        .expect("capture");
        assert_eq!(
            manifest.required_min_version,
            0,
            "below H the boundary still runs baseline"
        );
        node.sink_mut()
            .write_manifest(&manifest)
            .await
            .expect("write manifest");

        // ACTIVATION (what the driver does at H): flip the dual-path branch
        // selector to the boundary version so blocks at/after H seal a v1 root.
        node.host_mut().set_active_version(V);
        let hash_after_flip = node.app_hash();
        assert_ne!(
            hash_below, hash_after_flip,
            "the dual module's root MUST be version-sensitive or the test proves nothing"
        );

        // two ops AT/ABOVE H: version-1, dual root uses v1. the host injects a
        // boundary `Advance` at each (>= H) — the mock accepts it.
        node.submit(&signer, 2, op()).await.expect("submit");
        node.flush_batch().await.expect("flush");
        node.submit(&signer, 3, op()).await.expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 2);
        let tip = node.finalized().expect("boundary");
        assert!(tip.height >= H, "tip must sit at/above H");
        let tip_hash = node.app_hash();
        assert_ne!(tip_hash, hash_after_flip, "the ops moved the counter");

        node.sink_mut().sync().await.expect("shutdown sync");
        drop(node);

        // ---- crash + boot: restore the checkpoint, replay across H ---------
        let mut recovery = Recovery::open(context.child("r2")).await.expect("reopen");
        let manifest = recovery
            .manifest()
            .expect("decodes")
            .expect("present");

        // restore the dual module AT THE CHECKPOINT version (0, below H) — its
        // committed counter with the baseline branch selector.
        let dual = install_dual(
            manifest.snapshot("dual").expect("dual snapshot"),
            manifest.root("dual").expect("dual root"),
            0,
        );
        // the upgrade module is recreated identically (stateless config).
        let mut host = Host::genesis(vec![
            Box::new(dual),
            Box::new(upgrade_mock(&me, H, V)),
        ])
        .expect("genesis");

        // pre-replay the restored host is at the checkpoint (below H) — a NAIVE
        // replay that kept this baseline selector across H would fork here.
        assert_eq!(host.app_hash(), hash_below);

        let recovered = recovery
            .recover(&mut host, &manifest)
            .await
            .expect("recover across the armed boundary");

        // THE property: version-aware replay reconstructs the byte-identical tip.
        assert_eq!(recovered.height, Some(tip.height));
        assert_eq!(
            recovered.app_hash, tip_hash,
            "recovery must recompute the v1 root across H — a stale-version replay would mismatch"
        );
        assert_eq!(host.app_hash(), tip_hash);
    });
}

#[test]
fn replay_adds_a_dormant_module_at_the_exact_activation_height() {
    const H: u64 = 2;
    const V: u32 = 1;

    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        let signer = sk(7);
        let me = {
            use commonware_cryptography::Signer as _;
            signer.public_key().as_ref().to_vec()
        };

        let recovery = Recovery::open(context.child("registry_live"))
            .await
            .expect("open");
        let mut host = Host::genesis(vec![
            Box::new(Dual::new()),
            Box::new(upgrade_mock(&me, H, V)),
            Box::new(DormantMarker),
        ])
        .expect("genesis");
        host.defer_module_until("clients", V)
            .expect("defer clients");
        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);

        for seq in 0..2 {
            node.submit(&signer, seq, op()).await.expect("submit pre-H");
            node.flush_batch().await.expect("flush pre-H");
        }
        assert_eq!(node.drain_delivered().await.expect("drain pre-H"), 2);
        let checkpoint_height = node.finalized().expect("checkpoint boundary").height;
        assert!(checkpoint_height < H);
        assert!(node.host().module_root("clients").is_none());
        assert!(
            node.host()
                .state_schema()
                .iter()
                .all(|(id, _)| id != "clients")
        );
        let checkpoint_hash = node.app_hash();
        let pos = node.sink_mut().oplog_pos().await;
        let manifest = Manifest::capture(
            node.host(),
            Some(checkpoint_height),
            0,
            0,
            vec![],
            vec![],
            None,
            0,
            Some(UpgradeCoords {
                name: "forge-v2".into(),
                activation_height: H,
                to_version: V,
            }),
            pos,
            2,
        )
        .expect("capture pre-H registry");
        node.sink_mut()
            .write_manifest(&manifest)
            .await
            .expect("write manifest");

        node.submit(&signer, 2, op()).await.expect("submit at H");
        node.flush_batch().await.expect("flush at H");
        assert_eq!(node.drain_delivered().await.expect("drain at H"), 1);
        assert_eq!(node.finalized().expect("tip").height, H);
        assert_eq!(node.host().module_root("clients"), Some(StateRoot::ZERO));
        assert!(
            node.host()
                .state_schema()
                .iter()
                .any(|(id, _)| id == "clients")
        );
        let tip_hash = node.app_hash();
        assert_ne!(tip_hash, checkpoint_hash);
        node.sink_mut().sync().await.expect("sync live state");
        drop(node);

        let mut recovery = Recovery::open(context.child("registry_replay"))
            .await
            .expect("reopen");
        let manifest = recovery.manifest().expect("decode").expect("manifest");
        let dual = install_dual(
            manifest.snapshot("dual").expect("dual snapshot"),
            manifest.root("dual").expect("dual root"),
            0,
        );
        let mut restored = Host::genesis(vec![
            Box::new(dual),
            Box::new(upgrade_mock(&me, H, V)),
            Box::new(DormantMarker),
        ])
        .expect("restore genesis");
        restored
            .defer_module_until("clients", V)
            .expect("restore dormant clients");
        assert_eq!(restored.app_hash(), checkpoint_hash);
        assert!(restored.module_root("clients").is_none());

        let recovered = recovery
            .recover(&mut restored, &manifest)
            .await
            .expect("replay registry activation");
        assert_eq!(recovered.height, Some(H));
        assert_eq!(recovered.app_hash, tip_hash);
        assert_eq!(restored.app_hash(), tip_hash);
        assert_eq!(restored.module_root("clients"), Some(StateRoot::ZERO));
    });
}

/// FORK-CRITICAL, roll-forward variant: a crash landing between the activation
/// block's WAL record and its SEAL. recovery has two version-aware branches — the
/// sealed-block loop (covered above) AND the trailing-unsealed roll-forward block.
/// this exercises the second: the H-crossing block is a torn WAL record, and the
/// roll-forward MUST apply it under the boundary version `effective_version(H)=V`
/// (not the stale checkpoint version), or a node that crashed exactly at H forks.
#[test]
fn roll_forward_of_an_unsealed_block_across_h_is_version_aware() {
    const H: u64 = 2;
    const V: u32 = 1;

    let executor = deterministic::Runner::default();
    executor.start(|context| async move {
        let signer = sk(1);
        let me = {
            use commonware_cryptography::Signer as _;
            signer.public_key().as_ref().to_vec()
        };

        // ---- reference (no recovery store, so it cannot pollute the roll store):
        // apply 0,1 below H (v0), flip the selector at H, apply the H block (v1).
        // this is the tip the roll-forward must reconstruct. ----
        let tip_hash = {
            let mut host = Host::genesis(vec![
                Box::new(Dual::new()),
                Box::new(upgrade_mock(&me, H, V)),
            ])
            .expect("genesis");
            for h in 0..H {
                host.submit_at(
                    BlockContext { height: h, consensus_time: h, origin: Origin::System, protocol_version: 0 },
                    op(),
                )
                .await
                .expect("apply below H");
            }
            host.set_active_version(V);
            host.submit_at(
                BlockContext { height: H, consensus_time: H, origin: Origin::System, protocol_version: V },
                op(),
            )
            .await
            .expect("apply the H block at v1");
            host.app_hash()
        };

        // ---- roll-forward run: seal 0,1 below H + checkpoint, then a TORN write
        // of the H block (pre_apply without seal), then crash. ----
        let recovery = Recovery::open(context.child("roll")).await.expect("open");
        let host = Host::genesis(vec![
            Box::new(Dual::new()),
            Box::new(upgrade_mock(&me, H, V)),
        ])
        .expect("genesis");
        let mut node = OrderedNode::with_sink(host, RoundOrderer::new(), recovery);
        node.submit(&signer, 0, op()).await.expect("submit");
        node.flush_batch().await.expect("flush");
        node.submit(&signer, 1, op()).await.expect("submit");
        node.flush_batch().await.expect("flush");
        assert_eq!(node.drain_delivered().await.expect("drain"), 2);
        let checkpoint_height = node.finalized().expect("boundary").height;
        assert!(checkpoint_height < H, "checkpoint below H");
        let hash_below = node.app_hash();

        let pos = node.sink_mut().oplog_pos().await;
        let manifest = Manifest::capture(
            node.host(),
            Some(checkpoint_height),
            0,
            0,
            vec![],
            vec![],
            None,
            0,
            Some(UpgradeCoords { name: "forge-v2".into(), activation_height: H, to_version: V }),
            pos,
            2,
        )
        .expect("capture");
        node.sink_mut().write_manifest(&manifest).await.expect("write manifest");

        // the torn write: the H block's WAL record lands, its apply/seal does not.
        // the record is a BATCH super-frame (single member), as the live drain pins.
        let frame = node::encode_batch(&[node::encode_frame(&signer, 2, &op())]);
        {
            use node::BlockSink as _;
            node.sink_mut().pre_apply(H, &frame).await.expect("wal record");
        }
        drop(node);

        // ---- boot: restore the dual at the CHECKPOINT version (0, below H), then
        // roll the unsealed H block forward — it MUST land under v1. ----
        let mut recovery = Recovery::open(context.child("roll")).await.expect("reopen");
        let manifest = recovery.manifest().expect("decodes").expect("present");
        let dual = install_dual(
            manifest.snapshot("dual").expect("dual snapshot"),
            manifest.root("dual").expect("dual root"),
            0,
        );
        let mut host = Host::genesis(vec![
            Box::new(dual),
            Box::new(upgrade_mock(&me, H, V)),
        ])
        .expect("genesis");
        assert_eq!(host.app_hash(), hash_below, "restored below H");

        let recovered = recovery
            .recover(&mut host, &manifest)
            .await
            .expect("recover roll-forward across the armed boundary");
        assert!(recovered.rolled_forward, "the unsealed H block rolled forward");
        assert_eq!(recovered.height, Some(H));
        assert_eq!(
            recovered.app_hash, tip_hash,
            "the rolled-forward H block MUST apply under the boundary version — a stale-version roll-forward would fork"
        );
        assert_eq!(host.app_hash(), tip_hash);
    });
}
