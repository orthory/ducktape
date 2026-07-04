//! qmdb-backed key-value module.
//!
//! wraps a commonware qmdb `any/unordered/variable` database (32-byte hashed keys,
//! variable-length byte values, sha256-merkleized) and exposes it as an
//! [`sdk::Module`]. the module's authenticated [`StateRoot`] IS the qmdb merkle
//! root — a real cryptographic commitment to the whole store, refreshed on every
//! write — so it flows directly into the global app-hash via `state::global_root`.
//!
//! ## keys are hashed to a fixed width
//!
//! the logical key is a `Vec<u8>` at the [`Module`]/interface seam, but the qmdb
//! key is `sha256(logical_key)` — a fixed 32-byte [`commonware_utils`] `Array`.
//! this is deliberate and load-bearing: commonware's state-sync resolvers for the
//! overwriteable variable db are bounded on `K: Array`, and its own variable-db
//! usage keys on a `Digest`. hashing is the canonical authenticated-KV pattern
//! (cf. `keccak(address)` in an eth state trie). the cost is that the store commits
//! to `hash(key) -> value` and cannot enumerate original keys — a get/set KV never
//! needs to.
//!
//! ## state-sync
//!
//! a joiner (dynamic-valset catch-up, a fresh full node, crash recovery) rebuilds
//! this store from a peer via [`Kv::sync_target`] / [`Kv::sync_from`], delegating
//! to commonware's qmdb `sync` engine. this is the qmdb backend of layer-3 in
//! `handoff/data-plane-and-state-sync.md`: the reconstructed store's root equals
//! the source root, and every fetched batch is merkle-verified against that root,
//! so the source is untrusted — the root is the trust anchor.

use std::collections::BTreeMap;
use std::num::{NonZeroU16, NonZeroU64, NonZeroUsize};
use std::sync::Arc;

use commonware_codec::RangeCfg;
use commonware_cryptography::{Hasher, Sha256};
use commonware_parallel::Sequential;
use commonware_runtime::{BufferPooler, buffer::paged::CacheRef};
use commonware_storage::{
    Context, journal, mmr,
    qmdb::{
        any::{VariableConfig, unordered::variable::Db},
        sync::{self, DbResolver, Target, engine::Config as SyncConfig},
    },
    translator::TwoCap,
};
use commonware_utils::range::NonEmptyRange;

use sdk::{Ctx, Error, Module, ModuleId, Msg, ResolverSyncTarget, StateRoot, StateSyncHandle};

/// write-time cap on a LOGICAL key. the qmdb key is the 32-byte hash, so this is
/// a hygiene bound at the interface seam (an unbounded key would still bloat the
/// pending overlay and every message that carries it), not a storage-layout limit.
pub const MAX_KEY_LEN: usize = 4 * 1024;

/// write-time cap on a value. [`kv_config`]'s codec [`RangeCfg`] bounds a stored
/// value at 1 MiB AT DECODE TIME — an oversized value would COMMIT fine and then
/// panic every later read (and any log replay / sync batch decode) on every
/// validator: a poison pill. rejecting here keeps it out of the log entirely.
/// the 4 KiB margin below the 1 MiB codec bound covers the serialized operation's
/// framing (32-byte hashed key, varint length prefix, operation tag), so the
/// WHOLE stored form — not just the raw value — stays under the codec bound and
/// under 1 MiB-scale wire-message caps when ops ship in sync batches.
pub const MAX_VALUE_LEN: usize = (1 << 20) - 4 * 1024;

/// the qmdb key: a fixed 32-byte sha256 digest of the logical key. fixed width is
/// what lets a store be state-synced (commonware's resolvers require `K: Array`).
type KvKey = <Sha256 as Hasher>::Digest;

/// the concrete qmdb store: 32-byte hashed keys, variable byte values, sha256
/// hasher, two-byte translator, sequential (deterministic) merkle strategy.
type KvDb<E> = Db<mmr::Family, E, KvKey, Vec<u8>, Sha256, TwoCap, Sequential>;

/// the qmdb configuration for a kv store — shared by [`Kv::init`] (fresh open)
/// and [`Kv::sync_from`] (state-sync target) so a synced store's storage layout
/// is byte-identical to a freshly-opened one. the key codec cfg is `()` (fixed
/// width); only the variable value carries a [`RangeCfg`].
type KvConfig = VariableConfig<TwoCap, ((), (RangeCfg<usize>, ())), Sequential>;

/// a state-sync target: a qmdb merkle root plus the operation range a joiner must
/// pull to reconstruct a store with an identical root. produced by
/// [`Kv::sync_target`], consumed by [`Kv::sync_from`].
pub type KvTarget = Target<mmr::Family, KvKey>;

/// hash a logical key to its fixed-width qmdb key. deterministic, so every
/// validator maps a given logical key to the same store slot.
fn hash_key(key: &[u8]) -> KvKey {
    let mut h = Sha256::new();
    h.update(key);
    h.finalize()
}

/// build the qmdb [`VariableConfig`] for module `id` on `context`. partitions are
/// namespaced by `id` so several qmdb-backed modules can share one runtime context
/// without colliding on storage. the single source of truth for a kv store's
/// storage layout, so [`Kv::init`] and [`Kv::sync_from`] can never drift apart.
fn kv_config<E>(context: &E, id: &str) -> KvConfig
where
    E: Context + BufferPooler,
{
    // a single page-cache handle shared by both sub-configs (cheap to clone).
    let page_cache = CacheRef::from_pooler(
        context,
        NonZeroU16::new(128).unwrap(),
        NonZeroUsize::new(64).unwrap(),
    );

    // codec config for Operation<.., KvKey, Vec<u8>>: (key_cfg, value_cfg). the
    // key is a fixed-width digest so its cfg is `()`; the value is a Vec<u8> whose
    // <Vec<u8> as Read>::Cfg == (RangeCfg<usize>, ()). bound generously; values
    // are tiny.
    let codec_config = ((), (RangeCfg::from(0..=1 << 20), ()));

    VariableConfig {
        merkle_config: mmr::full::Config {
            journal_partition: format!("{id}-merkle-journal"),
            metadata_partition: format!("{id}-merkle-meta"),
            items_per_blob: NonZeroU64::new(64).unwrap(),
            write_buffer: NonZeroUsize::new(1024).unwrap(),
            strategy: Sequential,
            page_cache: page_cache.clone(),
        },
        journal_config: journal::contiguous::variable::Config {
            partition: format!("{id}-log"),
            items_per_section: NonZeroU64::new(64).unwrap(),
            write_buffer: NonZeroUsize::new(1024).unwrap(),
            compression: None,
            codec_config,
            page_cache,
        },
        translator: TwoCap,
    }
}

/// a qmdb-backed key-value module.
pub struct Kv<E>
where
    E: Context + BufferPooler,
{
    id: ModuleId,
    db: KvDb<E>,
    /// writes staged during the current block, keyed by LOGICAL key. read ahead of
    /// committed state by `get` (read-your-writes) and flushed to qmdb (under the
    /// hashed key) in one batch by `commit_block`; NOT reflected in `root()` until
    /// then.
    pending: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl<E> Kv<E>
where
    E: Context + BufferPooler,
{
    /// open (or recover) the store on `context` under module identity `id`.
    /// async because qmdb opens its log and writes an initial commit floor.
    pub async fn init(context: E, id: impl Into<ModuleId>) -> Self {
        let id = id.into();
        let cfg = kv_config(&context, &id);
        let db = KvDb::<E>::init(context, cfg)
            .await
            .expect("qmdb init failed");
        Self {
            id,
            db,
            pending: BTreeMap::new(),
        }
    }

    /// upsert `key -> value`, re-merkleize, apply, and flush. after this returns
    /// `root()` reflects the new committed merkle root. the qmdb key is
    /// `sha256(key)`. a direct test/dev convenience — the consensus write path is
    /// `execute` -> `stage`, which enforces the write-time size caps.
    pub async fn set(&mut self, key: Vec<u8>, value: Vec<u8>) {
        let batch = self
            .db
            .new_batch()
            .write(hash_key(&key), Some(value))
            .merkleize(&self.db, None::<Vec<u8>>)
            .await
            .expect("merkleize failed");
        self.db
            .apply_batch(batch)
            .await
            .expect("apply_batch failed");
        self.db.commit().await.expect("commit failed");
    }

    /// reject a write that would poison the store: [`kv_config`]'s codec bound is
    /// enforced only at DECODE time, so an oversized value commits fine and then
    /// panics every later read of that key on EVERY validator. checked at write
    /// time (see [`MAX_KEY_LEN`] / [`MAX_VALUE_LEN`] for the cap rationale).
    fn check_write_caps(key: &[u8], value: &[u8]) -> Result<(), Error> {
        if key.len() > MAX_KEY_LEN {
            return Err(Error::Module(format!(
                "key too large: {} bytes exceeds the {MAX_KEY_LEN}-byte cap",
                key.len()
            )));
        }
        if value.len() > MAX_VALUE_LEN {
            return Err(Error::Module(format!(
                "value too large: {} bytes exceeds the {MAX_VALUE_LEN}-byte cap",
                value.len()
            )));
        }
        Ok(())
    }

    /// stage `key -> value` for this block WITHOUT committing. visible to `get`
    /// at once (read-your-writes) but folded into the qmdb store — and `root()` —
    /// only when the host calls `commit_block` at the block boundary. rejects an
    /// over-cap key/value BEFORE staging, so a failed op leaves no overlay entry.
    pub fn stage(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), Error> {
        Self::check_write_caps(&key, &value)?;
        self.pending.insert(key, value);
        Ok(())
    }

    /// read `key`: a STAGED (this-block) write shadows committed qmdb state, so a
    /// later op in the same block sees an earlier staged write. committed reads go
    /// through the hashed key.
    pub async fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(v) = self.pending.get(key) {
            return Some(v.clone());
        }
        self.db.get(&hash_key(key)).await.expect("get failed")
    }

    // ---- state-sync ---------------------------------------------------------
    // reconstruct a byte-identical-rooted store from a peer WITHOUT replaying the
    // op history in application order — commonware's qmdb sync ships the live op
    // range and merkle-verifies every batch against the target root.

    /// the sync [`KvTarget`] for this store: its qmdb merkle root plus the LIVE
    /// operation range `[sync_boundary, end)`. hand it to [`Kv::sync_from`] to
    /// rebuild a store with an identical root. async only because `bounds()`
    /// reads the committed log tail.
    ///
    /// the range starts at `sync_boundary()`, not `0`: qmdb compacts overwritten
    /// history below its inactivity floor, so only the active tail ships (pinned
    /// merkle nodes cover the pruned prefix). that IS checkpoint semantics — the
    /// snapshot half of snapshot-plus-replay-tail.
    pub async fn sync_target(&self) -> KvTarget {
        let end = self.db.bounds().await.end;
        let start = self.db.sync_boundary();
        Target {
            root: self.db.root(),
            range: NonEmptyRange::new(start..end)
                .expect("a committed store has a non-empty op range"),
        }
    }

    /// consume this store into an `Arc`-wrapped raw qmdb that serves as a sync
    /// resolver: it answers a joiner's op-range requests with proof-carrying
    /// batches. a LIVE source still taking writes would instead wrap
    /// `Arc<AsyncRwLock<..>>`; this consuming form is the handoff / test source.
    pub fn into_resolver(self) -> Arc<KvDb<E>> {
        Arc::new(self.db)
    }

    /// reconstruct a `Kv` at `id` on `context` whose qmdb root EQUALS
    /// `target.root`, by pulling `target`'s op range from `resolver`. commonware's
    /// sync engine merkle-verifies every fetched batch against `target.root`, so a
    /// byzantine source cannot produce a store with a matching root but forged
    /// contents — the root is the trust anchor. reuses [`kv_config`] so the synced
    /// store's storage layout matches a freshly-opened one.
    pub async fn sync_from<R>(
        context: E,
        id: impl Into<ModuleId>,
        target: KvTarget,
        resolver: R,
    ) -> Self
    where
        R: DbResolver<KvDb<E>>,
    {
        let id = id.into();
        let db_config = kv_config(&context, &id);
        let config = SyncConfig {
            context,
            resolver,
            target,
            max_outstanding_requests: 1,
            fetch_batch_size: NonZeroU64::new(64).unwrap(),
            apply_batch_size: 1024,
            db_config,
            update_rx: None,
            finish_rx: None,
            reached_target_tx: None,
            max_retained_roots: 8,
        };
        let db = sync::sync(config).await.expect("qmdb sync failed");
        Self {
            id,
            db,
            pending: BTreeMap::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl<E> Module for Kv<E>
where
    E: Context + BufferPooler,
{
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the REAL qmdb merkle root over all current keys, as a 32-byte state root.
    /// sync, as the trait requires: qmdb caches its root and `db.root()` returns
    /// it by value (sha256 digest == 32 bytes == ROOT_LEN). never a placeholder.
    fn root(&self) -> StateRoot {
        StateRoot(self.db.root().0)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::ResolverBacked {
            backend: "qmdb".into(),
            detail: "serve_sync answers qmdb op-range requests (statesync wire)".into(),
        })
    }

    /// the network state-sync serve lane: answers the shared qmdb wire requests
    /// (historical proof-carrying op ranges) from committed state. read-only;
    /// the joiner's sync engine merkle-verifies every batch.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        statesync::qmdb::serve_bytes(&self.db, req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        statesync::qmdb::resolver_sync_target(&self.db).await
    }

    /// interpret the payload as a json-encoded `(key, value)` write and apply it
    /// to own state. the only `.await` is on own qmdb state — deterministic, so
    /// this is replay-safe across validators. an over-cap key/value is rejected
    /// here (write time), never staged, never committed — see [`MAX_VALUE_LEN`].
    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match kv_interface::decode(&msg.payload).map_err(Error::Module)? {
            kv_interface::KvMsg::Set { key, value } => self.stage(key, value),
        }
    }

    /// real async read of own qmdb state — the async-query seam in action.
    /// serves STAGED-over-committed via `get`, so cross-module reads within a
    /// block observe this block's staged writes.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match kv_interface::decode_query(req).map_err(Error::Module)? {
            kv_interface::KvQuery::Get { key } => Ok(kv_interface::encode_reply(
                &kv_interface::KvReply::Value(self.get(&key).await),
            )),
        }
    }

    /// publish the block's staged writes in ONE qmdb batch: write every pending
    /// key (hashed), merkleize, apply, commit. no-op (and no root movement) if
    /// nothing was staged. a single-write block issues the exact same sequence
    /// `set` did, so its committed root is byte-identical to the per-op path.
    async fn commit_block(&mut self) -> Result<(), Error> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let mut batch = self.db.new_batch();
        for (key, value) in &self.pending {
            batch = batch.write(hash_key(key), Some(value.clone()));
        }
        let batch = batch
            .merkleize(&self.db, None::<Vec<u8>>)
            .await
            .expect("merkleize failed");
        self.db
            .apply_batch(batch)
            .await
            .expect("apply_batch failed");
        self.db.commit().await.expect("commit failed");
        self.pending.clear();
        Ok(())
    }

    /// discard the block's staged writes — nothing reached qmdb, so `root()` is
    /// unchanged.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
    use state::global_root;

    // a fixed-root stand-in module, so we can prove the kv root composes into the
    // global app-hash alongside another module.
    struct StubModule;
    #[async_trait::async_trait(?Send)]
    impl Module for StubModule {
        fn id(&self) -> ModuleId {
            "stub".to_string()
        }
        fn root(&self) -> StateRoot {
            StateRoot([7u8; sdk::ROOT_LEN])
        }
        async fn execute(&mut self, _ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
            Ok(())
        }
    }

    #[test]
    fn real_qmdb_root_flows_into_app_hash() {
        deterministic::Runner::default().start(|context| async move {
            let mut kv = Kv::init(context, "kv").await;
            let stub = StubModule;

            let r0 = kv.root();

            kv.set(b"k1".to_vec(), b"v1".to_vec()).await;
            let r1 = kv.root();
            let app1 = {
                let mods: [&dyn Module; 2] = [&kv, &stub];
                global_root(&mods)
            };

            kv.set(b"k2".to_vec(), b"v2".to_vec()).await;
            let r2 = kv.root();
            let app2 = {
                let mods: [&dyn Module; 2] = [&kv, &stub];
                global_root(&mods)
            };

            // every write moves the real merkle root, and the post-write roots are
            // genuine (never the zero placeholder).
            assert_ne!(r0, r1, "first write must move the root");
            assert_ne!(r1, r2, "second write must move the root");
            assert_ne!(r1, StateRoot::ZERO, "root after write must be non-zero");
            assert_ne!(r2, StateRoot::ZERO, "root after write must be non-zero");

            // values round-trip through the store.
            assert_eq!(kv.get(b"k1").await.as_deref(), Some(b"v1".as_ref()));
            assert_eq!(kv.get(b"k2").await.as_deref(), Some(b"v2".as_ref()));

            // the kv merkle root genuinely flows into the composed app-hash: only
            // kv changed between r1 and r2, yet the global root differs.
            assert_ne!(
                app1, app2,
                "mutating only the kv module must change the global app-hash"
            );
        });
    }

    // robustness guard: the qmdb read/write/merkle path must survive EVERY task
    // schedule the deterministic runtime can produce — that is exactly the
    // property a consensus state machine needs (each validator schedules
    // differently). a lost write under any seed would be a real ordering bug.
    #[test]
    fn no_lost_writes_across_schedules() {
        let mut fails: Vec<u64> = Vec::new();
        for seed in 0..64u64 {
            let ok = deterministic::Runner::seeded(seed).start(|context| async move {
                let mut kv = Kv::init(context, "kv").await;
                kv.set(b"k1".to_vec(), b"v1".to_vec()).await;
                let g1 = kv.get(b"k1").await;
                kv.set(b"k2".to_vec(), b"v2".to_vec()).await;
                let g2 = kv.get(b"k2").await;
                g1.as_deref() == Some(b"v1".as_ref())
                    && g2.as_deref() == Some(b"v2".as_ref())
                    && kv.root() != StateRoot::ZERO
            });
            if !ok {
                fails.push(seed);
            }
        }
        assert!(fails.is_empty(), "lost write / None on seeds: {:?}", fails);
    }

    // a minimal Ctx so execute can be driven without a full host.
    struct TestCtx {
        env: sdk::Env,
    }
    impl TestCtx {
        fn new() -> Self {
            Self {
                env: sdk::Env { protocol_version: 0,
                    height: 0,
                    consensus_time: 0,
                    origin: sdk::Origin::System,
                    me: "kv".into(),
                },
            }
        }
    }
    #[async_trait::async_trait(?Send)]
    impl Ctx for TestCtx {
        fn env(&self) -> &sdk::Env {
            &self.env
        }
        fn module_root(&self, _t: &str) -> Option<StateRoot> {
            None
        }
        async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> {
            Err(Error::QueryUnsupported)
        }
        fn emit_msg(&mut self, _m: Msg) {}
        fn emit_event(&mut self, _e: sdk::Event) {}
        fn request_effect(&mut self, _e: sdk::Effect) {}
    }

    // the poison-pill guard: an over-cap set is rejected at WRITE time — never
    // staged, never committed, root unchanged — instead of committing fine and
    // panicking every later decode of that key on every validator.
    #[test]
    fn oversized_writes_are_rejected_before_staging() {
        deterministic::Runner::default().start(|context| async move {
            let mut kv = Kv::init(context, "kv").await;
            let r0 = kv.root();

            // value one byte over the cap -> rejected, nothing staged.
            let huge_value = kv_interface::encode(&kv_interface::KvMsg::Set {
                key: b"k".to_vec(),
                value: vec![0u8; MAX_VALUE_LEN + 1],
            });
            let err = kv
                .execute(
                    &mut TestCtx::new(),
                    &Msg {
                        target: "kv".into(),
                        payload: huge_value,
                    },
                )
                .await
                .expect_err("over-cap value must be rejected");
            assert!(
                matches!(err, Error::Module(ref m) if m.contains("value too large")),
                "unexpected error: {err:?}"
            );

            // key one byte over the cap -> rejected, nothing staged.
            let huge_key = kv_interface::encode(&kv_interface::KvMsg::Set {
                key: vec![b'k'; MAX_KEY_LEN + 1],
                value: b"v".to_vec(),
            });
            let err = kv
                .execute(
                    &mut TestCtx::new(),
                    &Msg {
                        target: "kv".into(),
                        payload: huge_key,
                    },
                )
                .await
                .expect_err("over-cap key must be rejected");
            assert!(
                matches!(err, Error::Module(ref m) if m.contains("key too large")),
                "unexpected error: {err:?}"
            );

            // the rejects happened BEFORE staging: no overlay entry, and a commit
            // is a no-op that leaves the root byte-identical.
            assert!(kv.pending.is_empty(), "a rejected write must not be staged");
            kv.commit_block().await.expect("commit");
            assert_eq!(kv.root(), r0, "a rejected write must not move the root");

            // boundary: exactly-at-cap writes are accepted and commit fine.
            kv.stage(vec![b'k'; MAX_KEY_LEN], vec![0u8; MAX_VALUE_LEN])
                .expect("at-cap write");
            kv.commit_block().await.expect("commit at-cap write");
            assert_eq!(
                kv.get(&vec![b'k'; MAX_KEY_LEN]).await.map(|v| v.len()),
                Some(MAX_VALUE_LEN)
            );
        });
    }

    // isolation: two qmdb modules on ONE runtime context must not share storage.
    // same key written to each stays independent, and the roots diverge.
    #[test]
    fn two_modules_on_one_context_dont_collide() {
        deterministic::Runner::default().start(|context| async move {
            let mut a = Kv::init(context.child("alpha"), "alpha").await;
            let mut b = Kv::init(context.child("beta"), "beta").await;
            a.set(b"x".to_vec(), b"1".to_vec()).await;
            b.set(b"x".to_vec(), b"2".to_vec()).await;
            assert_eq!(a.get(b"x").await.as_deref(), Some(b"1".as_ref()));
            assert_eq!(b.get(b"x").await.as_deref(), Some(b"2".as_ref()));
            assert_ne!(
                a.root(),
                b.root(),
                "isolated modules must have distinct roots"
            );
        });
    }
}
