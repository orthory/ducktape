//! the qmdb half of the state-sync wire: serve a live store's proof-carrying op
//! ranges as opaque bytes, and resolve those bytes
//! back on the joiner — including a [`RemoteQmdbResolver`] that plugs straight
//! into commonware's qmdb sync engine as its network
//! [`Resolver`](commonware_storage::qmdb::sync::Resolver).
//!
//! every qmdb-backed module in this workspace shares ONE store shape —
//! 32-byte sha256-hashed keys, variable byte values, sha256 merkleization,
//! two-byte translator, sequential strategy — so one wire format serves kv,
//! document, messaging, and the wrappers over them. the serve side answers with
//! HISTORICAL proofs: a source that keeps committing new blocks still serves a
//! joiner's in-flight target consistently (the proofs are anchored at the
//! target's op count, not the live head), until compaction prunes below the
//! target range — at which point the joiner refetches a fresh manifest and
//! retries. trust never comes from the source: the sync engine merkle-verifies
//! every batch against the consensus-committed target root.

use std::num::{NonZeroU16, NonZeroU64, NonZeroUsize};

use commonware_codec::{Decode, DecodeExt as _, Encode, EncodeSize as _, RangeCfg};
use commonware_cryptography::{Hasher, Sha256};
use commonware_parallel::Sequential;
use commonware_runtime::{BufferPooler, buffer::paged::CacheRef};
use commonware_storage::{
    Context, journal,
    merkle::{self, Location, Proof},
    qmdb::{
        any::{
            VariableConfig,
            unordered::variable::{Db, Operation},
        },
        sync::{self, DbResolver, Target, engine::Config as SyncConfig},
    },
    translator::TwoCap,
};
use commonware_utils::channel::oneshot;

use crate::wire::{self, WireError};
use crate::{BoundaryId, SyncClient, SyncError, SyncRequest, SyncResponse};

/// the shared digest type: sha256, used as both the hashed key and the proof
/// digest by every qmdb module in the workspace.
pub type SyncDigest = <Sha256 as Hasher>::Digest;

/// the ONE qmdb store shape every module in this workspace uses.
pub type SyncDb<E> = Db<merkle::mmr::Family, E, SyncDigest, Vec<u8>, Sha256, TwoCap, Sequential>;

/// the op type that store's journal carries (what sync batches ship).
pub type SyncOp = Operation<merkle::mmr::Family, SyncDigest, Vec<u8>>;

/// a sync target for that store shape: root + live op range.
pub type SyncTarget = Target<merkle::mmr::Family, SyncDigest>;

/// the proof type op-range responses carry.
pub type SyncProof = Proof<merkle::mmr::Family, SyncDigest>;

/// the codec read-config for [`SyncOp`] — MUST mirror the journal codec config
/// every module's store is built with (fixed-width key => `()`, value bounded
/// at [`sdk::MAX_STORE_VALUE_BYTES`]). a mismatch would reject ops the
/// source's own journal accepted.
fn op_read_cfg() -> ((), (RangeCfg<usize>, ())) {
    ((), (RangeCfg::from(0..=sdk::MAX_STORE_VALUE_BYTES), ()))
}

/// generous ceiling on proof digests per response (proofs are O(log n) in
/// store size; 4096 covers stores far beyond any plausible module).
const MAX_PROOF_DIGESTS: usize = 4096;

/// ceiling on ops per fetched batch accepted from a peer — the engine asks for
/// small batches (kv syncs at 64); this only bounds a malicious oversized reply.
const MAX_OPS_PER_BATCH: u64 = 4096;

/// ceiling on ONE encoded op-batch reply (proof + ops + pinned nodes) a
/// module's `serve_sync` hands back. the reply rides the mesh as a
/// `SyncResponse::Module` body, and the p2p sender ASSERTS on its 2 MiB
/// message cap — an op COUNT cap alone let a batch of adjacent ~1 MiB records
/// (tasks, pages) walk straight into that assert. [`serve`] trims a batch to
/// the largest op prefix that fits; the sync engine resumes by location, so a
/// shorter batch is progress, never an error. sized so one op carrying the
/// largest value the journal codec admits ([`sdk::MAX_STORE_VALUE_BYTES`])
/// plus a proof and pinned nodes at their decode ceilings always fits.
/// bin/node compile-asserts this stays under its `MAX_MESSAGE_SIZE` with
/// rpc-envelope headroom.
pub const MAX_MODULE_REPLY_BYTES: usize = (1 << 21) - (64 << 10);
const _: () = assert!(
    MAX_MODULE_REPLY_BYTES >= sdk::MAX_STORE_VALUE_BYTES + 2 * MAX_PROOF_DIGESTS * 32 + 4096
);

// ============================================================================
// the request body a module's `serve_sync` understands.
// ============================================================================

/// a byte-level request against one qmdb-backed module's sync surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QmdbSyncReq {
    /// a proof-carrying op range, anchored at `op_count` (historical — valid
    /// for an older target even after the store advanced).
    Ops {
        op_count: u64,
        start_loc: u64,
        max_ops: u64,
        include_pinned: bool,
    },
}

pub fn encode_qmdb_req(req: &QmdbSyncReq) -> Vec<u8> {
    let mut out = Vec::new();
    match req {
        QmdbSyncReq::Ops {
            op_count,
            start_loc,
            max_ops,
            include_pinned,
        } => {
            out.push(0u8);
            out.extend_from_slice(&op_count.to_le_bytes());
            out.extend_from_slice(&start_loc.to_le_bytes());
            out.extend_from_slice(&max_ops.to_le_bytes());
            out.push(u8::from(*include_pinned));
        }
    }
    out
}

pub fn decode_qmdb_req(bytes: &[u8]) -> Result<QmdbSyncReq, WireError> {
    let mut buf = bytes;
    let tag = wire::take_u8(&mut buf)?;
    let req = match tag {
        0 => QmdbSyncReq::Ops {
            op_count: wire::take_u64(&mut buf)?,
            start_loc: wire::take_u64(&mut buf)?,
            max_ops: wire::take_u64(&mut buf)?,
            include_pinned: wire::take_u8(&mut buf)? != 0,
        },
        other => return Err(WireError::BadTag("QmdbSyncReq", other)),
    };
    wire::expect_empty(buf)?;
    Ok(req)
}

// ============================================================================
// the op-range envelope: proof + ops + pinned nodes as one byte payload.
// ============================================================================

/// a fetched op range as it crosses the wire.
pub struct OpsEnvelope {
    pub proof: SyncProof,
    pub operations: Vec<SyncOp>,
    pub pinned_nodes: Option<Vec<SyncDigest>>,
}

pub fn encode_ops_envelope(env: &OpsEnvelope) -> Vec<u8> {
    let mut out = Vec::new();
    wire::put_bytes(&mut out, &env.proof.encode());
    out.extend_from_slice(&(env.operations.len() as u64).to_le_bytes());
    for op in &env.operations {
        wire::put_bytes(&mut out, &op.encode());
    }
    match &env.pinned_nodes {
        None => out.push(0u8),
        Some(nodes) => {
            out.push(1u8);
            out.extend_from_slice(&(nodes.len() as u64).to_le_bytes());
            for d in nodes {
                out.extend_from_slice(d.as_ref());
            }
        }
    }
    out
}

pub fn decode_ops_envelope(bytes: &[u8]) -> Result<OpsEnvelope, WireError> {
    let mut buf = bytes;
    let proof_bytes = wire::take_bytes(&mut buf)?;
    let proof = SyncProof::decode_cfg(proof_bytes, &MAX_PROOF_DIGESTS)
        .map_err(|e| WireError::Codec(format!("proof: {e}")))?;

    let count = wire::take_u64(&mut buf)?;
    if count > MAX_OPS_PER_BATCH {
        return Err(WireError::Codec(format!(
            "op batch of {count} exceeds the {MAX_OPS_PER_BATCH} ceiling"
        )));
    }
    let mut operations = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let op_bytes = wire::take_bytes(&mut buf)?;
        let op = SyncOp::decode_cfg(op_bytes, &op_read_cfg())
            .map_err(|e| WireError::Codec(format!("operation: {e}")))?;
        operations.push(op);
    }

    let pinned_nodes = match wire::take_u8(&mut buf)? {
        0 => None,
        1 => {
            let n = wire::take_u64(&mut buf)?;
            if n > MAX_PROOF_DIGESTS as u64 {
                return Err(WireError::Codec(format!(
                    "{n} pinned nodes exceeds the {MAX_PROOF_DIGESTS} ceiling"
                )));
            }
            let mut nodes = Vec::with_capacity(n as usize);
            for _ in 0..n {
                let raw = wire::take_array::<32>(&mut buf)?;
                let digest = SyncDigest::decode(raw.as_ref())
                    .map_err(|e| WireError::Codec(format!("pinned digest: {e}")))?;
                nodes.push(digest);
            }
            Some(nodes)
        }
        other => return Err(WireError::BadTag("pinned_nodes flag", other)),
    };
    wire::expect_empty(buf)?;
    Ok(OpsEnvelope {
        proof,
        operations,
        pinned_nodes,
    })
}

// ============================================================================
// SERVE — answer a decoded request against a live store (module side).
// ============================================================================

/// serve one [`QmdbSyncReq`] from a live store. read-only; historical proofs
/// keep older manifest-pinned targets servable while the store keeps advancing.
/// this is what a qmdb-backed module's [`sdk::Module::serve_sync`] delegates to.
pub async fn serve<E>(db: &SyncDb<E>, req: &QmdbSyncReq) -> Result<Vec<u8>, sdk::Error>
where
    E: Context + BufferPooler,
{
    match req {
        QmdbSyncReq::Ops {
            op_count,
            start_loc,
            max_ops,
            include_pinned,
        } => {
            let mut max_ops = NonZeroU64::new((*max_ops).min(MAX_OPS_PER_BATCH))
                .ok_or_else(|| sdk::Error::Module("max_ops must be non-zero".into()))?;
            let pinned_nodes = if *include_pinned {
                Some(
                    db.pinned_nodes_at(Location::new(*start_loc))
                        .await
                        .map_err(|e| sdk::Error::Module(format!("pinned nodes failed: {e}")))?,
                )
            } else {
                None
            };
            // the BYTE budget ([`MAX_MODULE_REPLY_BYTES`]): a batch that encodes
            // over it is re-proved over the largest op prefix that fits. the
            // proof is a range proof over exactly the ops served, so a shorter
            // batch is a fresh `historical_proof`, never a sliced envelope. the
            // prefix is strictly shorter each round, so this settles in a
            // couple of rounds at most.
            loop {
                let (proof, operations) = db
                    .historical_proof(Location::new(*op_count), Location::new(*start_loc), max_ops)
                    .await
                    .map_err(|e| sdk::Error::Module(format!("historical proof failed: {e}")))?;
                let envelope = OpsEnvelope {
                    proof,
                    operations,
                    pinned_nodes: pinned_nodes.clone(),
                };
                let encoded = encode_ops_envelope(&envelope);
                let fits_budget = encoded.len() <= MAX_MODULE_REPLY_BYTES;
                if fits_budget {
                    return Ok(encoded);
                }
                let Some(shorter) = NonZeroU64::new(fitting_op_prefix(&envelope, encoded.len()))
                else {
                    return Err(sdk::Error::Module(format!(
                        "one op exceeds the {MAX_MODULE_REPLY_BYTES}-byte module reply budget"
                    )));
                };
                debug_assert!(
                    shorter < max_ops,
                    "an over-budget batch shrinks every round"
                );
                max_ops = shorter;
            }
        }
    }
}

/// how many leading ops of `envelope` fit [`MAX_MODULE_REPLY_BYTES`] once the
/// envelope's non-op bytes (proof, pinned nodes, counts) are taken out.
/// `encoded_len` is the whole envelope's encoded length. zero means the FIRST
/// op alone does not fit (a codec-ceiling value under a ceiling-sized proof —
/// precluded by the const assert on the budget, so an error, never a spin).
fn fitting_op_prefix(envelope: &OpsEnvelope, encoded_len: usize) -> u64 {
    // `put_bytes` = u64 length prefix + the op's own encoding.
    let op_lens: Vec<usize> = envelope
        .operations
        .iter()
        .map(|op| 8 + op.encode_size())
        .collect();
    let overhead = encoded_len - op_lens.iter().sum::<usize>();
    let mut budget = MAX_MODULE_REPLY_BYTES.saturating_sub(overhead);
    let mut fitting = 0u64;
    for len in op_lens {
        if len > budget {
            break;
        }
        budget -= len;
        fitting += 1;
    }
    fitting
}

/// describe the store's current sync target for manifest capture. this is not a
/// wire request; callers pin the returned target into the manifest before a
/// joiner starts fetching operation ranges.
pub async fn resolver_sync_target<E>(db: &SyncDb<E>) -> Result<sdk::ResolverSyncTarget, sdk::Error>
where
    E: Context + BufferPooler,
{
    let end = db.bounds().await.end;
    let start = db.sync_boundary();
    let range = commonware_utils::range::NonEmptyRange::new(start..end)
        .map_err(|_| sdk::Error::Module("store has no committed operations to sync".into()))?;
    Ok(sdk::ResolverSyncTarget {
        root: sdk::StateRoot(db.root().0),
        start: range.start().as_u64(),
        op_count: range.end().as_u64(),
    })
}

/// convenience for module `serve_sync` impls: decode + serve in one call.
pub async fn serve_bytes<E>(db: &SyncDb<E>, req: &[u8]) -> Result<Vec<u8>, sdk::Error>
where
    E: Context + BufferPooler,
{
    let req = decode_qmdb_req(req).map_err(|e| sdk::Error::Module(e.to_string()))?;
    serve(db, &req).await
}

pub fn module_lane_error(module_id: &str, error: String) -> SyncError {
    if is_pruned_range_error(&error) {
        SyncError::Pruned {
            module: module_id.to_string(),
            reason: error,
        }
    } else {
        SyncError::Server(error)
    }
}

fn is_pruned_range_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("operation pruned")
        || lower.contains("itempruned")
        || lower.contains("item pruned")
        || lower.contains("historical range pruned")
}

// ============================================================================
// STORE — the host-constructed concrete store, injected into modules as
// `Box<dyn sdk::MerkleStore>`.
// ============================================================================

/// the qmdb configuration for the shared store shape. the key codec cfg is `()`
/// (fixed-width digest); only the variable value carries a [`RangeCfg`].
pub type SyncDbConfig = VariableConfig<TwoCap, ((), (RangeCfg<usize>, ())), Sequential>;

/// build the qmdb [`VariableConfig`] for module `id` on `context`. partitions
/// are namespaced by `id` so several qmdb-backed modules can share one runtime
/// context without colliding on storage. the single source of truth for a
/// module store's on-disk layout — [`QmdbStore::init`] (fresh open) and
/// [`QmdbStore::sync_from`] (state-sync target) both build from it, so a synced
/// store's storage layout is byte-identical to a freshly-opened one. the
/// partition-name format and every constant are load-bearing: this function
/// replaced per-module copies (kv, pages, chat) and an existing store on disk
/// only reopens if none of them drift.
pub fn store_config<E>(context: &E, id: &str) -> SyncDbConfig
where
    E: Context + BufferPooler,
{
    // a single page-cache handle shared by both sub-configs (cheap to clone).
    let page_cache = CacheRef::from_pooler(
        context,
        NonZeroU16::new(128).unwrap(),
        NonZeroUsize::new(64).unwrap(),
    );

    VariableConfig {
        merkle_config: merkle::mmr::full::Config {
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
            // the journal codec config IS the wire read-config (fixed-width key
            // => `()`, value bounded at 1 MiB); reusing [`op_read_cfg`] keeps
            // the two mirrored by construction instead of by comment.
            codec_config: op_read_cfg(),
            page_cache,
        },
        translator: TwoCap,
    }
}

/// the concrete qmdb-backed [`sdk::MerkleStore`]. the HOST constructs one per
/// module (it owns the runtime context and storage) and injects it as
/// `Box<dyn MerkleStore>`, so module crates stay pure logic over the trait and
/// never depend on commonware-storage/-runtime themselves.
pub struct QmdbStore<E>
where
    E: Context + BufferPooler,
{
    db: SyncDb<E>,
}

impl<E> QmdbStore<E>
where
    E: Context + BufferPooler,
{
    /// open (or recover) the store on `context` under module identity `id`.
    /// async because qmdb opens its log and writes an initial commit floor.
    pub async fn init(context: E, id: &str) -> Self {
        let cfg = store_config(&context, id);
        let db = SyncDb::<E>::init(context, cfg)
            .await
            .expect("qmdb init failed");
        Self { db }
    }

    /// reconstruct a store at `id` on `context` whose qmdb root EQUALS
    /// `target.root`, by pulling `target`'s op range from `resolver`. the sync
    /// engine merkle-verifies every fetched batch against `target.root`, so a
    /// byzantine source cannot produce a store with a matching root but forged
    /// contents — the root is the trust anchor. reuses [`store_config`] so the
    /// synced store's storage layout matches a freshly-opened one.
    pub async fn sync_from<R>(
        context: E,
        id: &str,
        target: SyncTarget,
        resolver: R,
    ) -> Result<Self, String>
    where
        R: DbResolver<SyncDb<E>>,
    {
        let db_config = store_config(&context, id);
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
        // a sync failure (transport blip, dropped source) is the caller's
        // retry loop to own — never a process kill.
        let db = sync::sync(config)
            .await
            .map_err(|e| format!("qmdb sync: {e:?}"))?;
        Ok(Self { db })
    }

    /// the engine-native [`SyncTarget`] for this store: its qmdb merkle root
    /// plus the LIVE operation range `[sync_boundary, end)`. hand it to
    /// [`QmdbStore::sync_from`] to rebuild a store with an identical root.
    /// async only because `bounds()` reads the committed log tail.
    ///
    /// the range starts at `sync_boundary()`, not `0`: qmdb compacts
    /// overwritten history below its inactivity floor, so only the active tail
    /// ships (pinned merkle nodes cover the pruned prefix). that IS checkpoint
    /// semantics — the snapshot half of snapshot-plus-replay-tail.
    pub async fn sync_boundary_target(&self) -> SyncTarget {
        let end = self.db.bounds().await.end;
        let start = self.db.sync_boundary();
        Target {
            root: self.db.root(),
            range: commonware_utils::range::NonEmptyRange::new(start..end)
                .expect("a committed store has a non-empty op range"),
        }
    }

    /// consume this store into an `Arc`-wrapped raw qmdb that serves as a sync
    /// resolver: it answers a joiner's op-range requests with proof-carrying
    /// batches. a LIVE source still taking writes would instead wrap
    /// `Arc<AsyncRwLock<..>>`; this consuming form is the handoff / test source.
    pub fn into_resolver(self) -> std::sync::Arc<SyncDb<E>> {
        std::sync::Arc::new(self.db)
    }
}

#[async_trait::async_trait(?Send)]
impl<E> sdk::MerkleStore for QmdbStore<E>
where
    E: Context + BufferPooler,
{
    async fn get(&self, key: &[u8; sdk::ROOT_LEN]) -> Result<Option<Vec<u8>>, sdk::Error> {
        self.db
            .get(&SyncDigest::from(*key))
            .await
            .map_err(|e| sdk::Error::Module(format!("qmdb get failed: {e}")))
    }

    /// apply ONE ordered batch: write every hashed key, merkleize, apply,
    /// commit — the exact call sequence the modules issued inline before the
    /// store was injected, and in the caller's given order, so committed roots
    /// stay byte-identical across the cutover.
    async fn commit_batch(
        &mut self,
        writes: Vec<([u8; sdk::ROOT_LEN], Option<Vec<u8>>)>,
    ) -> Result<(), sdk::Error> {
        let mut batch = self.db.new_batch();
        for (key, value) in writes {
            batch = batch.write(SyncDigest::from(key), value);
        }
        let batch = batch
            .merkleize(&self.db, None::<Vec<u8>>)
            .await
            .map_err(|e| sdk::Error::Module(format!("merkleize failed: {e}")))?;
        self.db
            .apply_batch(batch)
            .await
            .map_err(|e| sdk::Error::Module(format!("apply_batch failed: {e}")))?;
        self.db
            .commit()
            .await
            .map_err(|e| sdk::Error::Module(format!("commit failed: {e}")))?;
        Ok(())
    }

    /// the REAL qmdb merkle root over all committed keys — qmdb caches it, so
    /// this is sync and by-value (sha256 digest == 32 bytes == ROOT_LEN).
    fn root(&self) -> sdk::StateRoot {
        sdk::StateRoot(self.db.root().0)
    }

    async fn sync_target(&self) -> Result<sdk::ResolverSyncTarget, sdk::Error> {
        resolver_sync_target(&self.db).await
    }

    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, sdk::Error> {
        serve_bytes(&self.db, req).await
    }
}

// ============================================================================
// RESOLVE — the joiner-side network resolver for the qmdb sync engine.
// ============================================================================

/// a network-backed [`Resolver`](sync::Resolver) for the shared store shape:
/// `get_operations` becomes a [`SyncRequest::Module`] round-trip through a
/// [`SyncClient`], and the response envelope is decoded back into the proof +
/// ops + pinned nodes the sync engine verifies against its target root. the
/// engine's merkle verification is the trust boundary — a lying server fails
/// verification, never installs.
#[derive(Clone)]
pub struct RemoteQmdbResolver<C> {
    client: C,
    boundary: BoundaryId,
    module_id: String,
}

impl<C> RemoteQmdbResolver<C> {
    pub fn new(client: C, boundary: BoundaryId, module_id: impl Into<String>) -> Self {
        Self {
            client,
            boundary,
            module_id: module_id.into(),
        }
    }
}

impl<C> sync::resolver::Resolver for RemoteQmdbResolver<C>
where
    C: SyncClient,
{
    type Family = merkle::mmr::Family;
    type Digest = SyncDigest;
    type Op = SyncOp;
    type Error = SyncError;

    async fn get_operations(
        &self,
        op_count: Location<Self::Family>,
        start_loc: Location<Self::Family>,
        max_ops: NonZeroU64,
        include_pinned_nodes: bool,
        _cancel_rx: oneshot::Receiver<()>,
    ) -> Result<sync::resolver::FetchResult<Self::Family, Self::Op, Self::Digest>, Self::Error>
    {
        let body = encode_qmdb_req(&QmdbSyncReq::Ops {
            op_count: op_count.as_u64(),
            start_loc: start_loc.as_u64(),
            max_ops: max_ops.get(),
            include_pinned: include_pinned_nodes,
        });
        let resp = self
            .client
            .request(SyncRequest::Module {
                boundary: self.boundary,
                module_id: self.module_id.clone(),
                body,
            })
            .await?;
        let bytes = match resp {
            SyncResponse::Module(bytes) => bytes,
            SyncResponse::Error(e) => return Err(module_lane_error(&self.module_id, e)),
            other => return Err(SyncError::UnexpectedResponse(other.kind_name())),
        };
        let env = decode_ops_envelope(&bytes)?;
        Ok(sync::resolver::FetchResult::new(
            env.proof,
            env.operations,
            env.pinned_nodes,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
    use sdk::MerkleStore as _;

    /// the byte budget: three records at the tasks module's record ceiling
    /// (~1 MiB each — two already exceed the mesh cap) do not ride one reply.
    /// the server trims the batch to the prefix that fits, and the cursor
    /// walk the sync engine performs still reaches every op.
    #[test]
    fn an_op_batch_is_trimmed_to_the_module_reply_budget_and_resumes_by_cursor() {
        deterministic::Runner::default().start(|context| async move {
            let mut store = QmdbStore::init(context.child("tasks"), "tasks").await;
            let value = vec![0x5A; (1 << 20) - 4096];
            for i in 0u8..3 {
                store
                    .commit_batch(vec![([i; sdk::ROOT_LEN], Some(value.clone()))])
                    .await
                    .expect("commit");
            }
            let target = resolver_sync_target(&store.db).await.expect("target");

            let mut cursor = target.start;
            let mut served = 0u64;
            let mut pages = 0u32;
            while cursor < target.op_count {
                let req = QmdbSyncReq::Ops {
                    op_count: target.op_count,
                    start_loc: cursor,
                    max_ops: 64,
                    include_pinned: cursor == target.start,
                };
                let bytes = serve(&store.db, &req).await.expect("serve");
                assert!(
                    bytes.len() <= MAX_MODULE_REPLY_BYTES,
                    "page {pages} encodes to {} bytes, over the {MAX_MODULE_REPLY_BYTES} budget",
                    bytes.len()
                );
                let env = decode_ops_envelope(&bytes).expect("envelope decodes");
                assert!(!env.operations.is_empty(), "a page always makes progress");
                cursor += env.operations.len() as u64;
                served += env.operations.len() as u64;
                pages += 1;
            }
            assert_eq!(
                served,
                target.op_count - target.start,
                "every op is served once"
            );
            assert!(
                pages > 1,
                "three ~1 MiB records cannot ride one 2 MiB reply"
            );
        });
    }
}
