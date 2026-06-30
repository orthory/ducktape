//! qmdb-backed key-value module.
//!
//! wraps a commonware qmdb `any/unordered/variable` database (byte keys, byte
//! values, sha256-merkleized) and exposes it as an [`sdk::Module`]. the module's
//! authenticated [`StateRoot`] IS the qmdb merkle root — a real cryptographic
//! commitment to every key currently in the store, refreshed on every write —
//! so it flows directly into the global app-hash via `state::global_root`.
//!
//! the sdk-facing surface is [`Module`]: `id`/`root` for app-hash composition,
//! plus `execute` for host dispatch. an inbound [`Msg`] payload is the json
//! encoding of `(key, value)` byte vectors — a trivial deterministic wire so the
//! host can drive a write without any commonware type leaking through the seam.
//! `query` keeps the default (Unsupported): qmdb reads are async, the sync query
//! projection lands in a later slice.

use std::num::{NonZeroU16, NonZeroU64, NonZeroUsize};

use commonware_codec::RangeCfg;
use commonware_cryptography::Sha256;
use commonware_parallel::Sequential;
use commonware_runtime::{buffer::paged::CacheRef, BufferPooler};
use commonware_storage::{
    journal, mmr,
    qmdb::any::{unordered::variable::Db, VariableConfig},
    translator::TwoCap,
    Context,
};

use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};

/// the concrete qmdb store: arbitrary byte keys and values, sha256 hasher,
/// two-byte translator, sequential (deterministic) merkle strategy.
type KvDb<E> = Db<mmr::Family, E, Vec<u8>, Vec<u8>, Sha256, TwoCap, Sequential>;

/// a qmdb-backed key-value module.
pub struct Kv<E>
where
    E: Context + BufferPooler,
{
    id: ModuleId,
    db: KvDb<E>,
}

impl<E> Kv<E>
where
    E: Context + BufferPooler,
{
    /// open (or recover) the store on `context` under module identity `id`.
    /// async because qmdb opens its log and writes an initial commit floor.
    pub async fn init(context: E, id: impl Into<ModuleId>) -> Self {
        // namespace every qmdb partition by module id so multiple qmdb-backed
        // modules can share one runtime context without colliding on storage.
        let id = id.into();
        // a single page-cache handle shared by both sub-configs (cheap to clone).
        let page_cache = CacheRef::from_pooler(
            &context,
            NonZeroU16::new(128).unwrap(),
            NonZeroUsize::new(64).unwrap(),
        );

        // codec config for Operation<.., Vec<u8>, Vec<u8>>: (key_cfg, value_cfg),
        // and <Vec<u8> as Read>::Cfg == (RangeCfg<usize>, ()). bound generously;
        // our values are tiny.
        let codec_config = (
            (RangeCfg::from(0..=1 << 20), ()),
            (RangeCfg::from(0..=1 << 20), ()),
        );

        let cfg: VariableConfig<TwoCap, ((RangeCfg<usize>, ()), (RangeCfg<usize>, ())), Sequential> =
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
            };

        let db = KvDb::<E>::init(context, cfg)
            .await
            .expect("qmdb init failed");

        Self { id, db }
    }

    /// upsert `key -> value`, re-merkleize, apply, and flush. after this returns
    /// `root()` reflects the new committed merkle root.
    pub async fn set(&mut self, key: Vec<u8>, value: Vec<u8>) {
        let batch = self
            .db
            .new_batch()
            .write(key, Some(value))
            .merkleize(&self.db, None::<Vec<u8>>)
            .await
            .expect("merkleize failed");
        self.db.apply_batch(batch).await.expect("apply_batch failed");
        self.db.commit().await.expect("commit failed");
    }

    /// read the value currently associated with `key`, if any.
    pub async fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.db.get(&key.to_vec()).await.expect("get failed")
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

    /// interpret the payload as a json-encoded `(key, value)` write and apply it
    /// to own state. the only `.await` is on own qmdb state — deterministic, so
    /// this is replay-safe across validators.
    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let (key, value): (Vec<u8>, Vec<u8>) =
            serde_json::from_slice(&msg.payload).map_err(|e| Error::Module(e.to_string()))?;
        self.set(key, value).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_runtime::{deterministic, Runner as _, Supervisor as _};
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
            if !ok { fails.push(seed); }
        }
        assert!(fails.is_empty(), "lost write / None on seeds: {:?}", fails);
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
            assert_ne!(a.root(), b.root(), "isolated modules must have distinct roots");
        });
    }
}
