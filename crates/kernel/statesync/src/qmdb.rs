//! the qmdb half of the state-sync wire: serve a live store's sync surface
//! (target + proof-carrying op ranges) as opaque bytes, and resolve those bytes
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

use std::num::NonZeroU64;

use commonware_codec::{Decode, DecodeExt as _, Encode, RangeCfg};
use commonware_cryptography::{Hasher, Sha256};
use commonware_parallel::Sequential;
use commonware_runtime::BufferPooler;
use commonware_storage::{
    merkle::{self, Location, Proof},
    qmdb::{
        any::unordered::variable::{Db, Operation},
        sync::{self, Target},
    },
    translator::TwoCap,
    Context,
};
use commonware_utils::channel::oneshot;

use crate::wire::{self, WireError};
use crate::{SyncClient, SyncError, SyncRequest, SyncResponse};

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
/// at 1 MiB). a mismatch would reject ops the source's own journal accepted.
fn op_read_cfg() -> ((), (RangeCfg<usize>, ())) {
    ((), (RangeCfg::from(0..=1 << 20), ()))
}

/// generous ceiling on proof digests per response (proofs are O(log n) in
/// store size; 4096 covers stores far beyond any plausible module).
const MAX_PROOF_DIGESTS: usize = 4096;

/// ceiling on ops per fetched batch accepted from a peer — the engine asks for
/// small batches (kv syncs at 64); this only bounds a malicious oversized reply.
const MAX_OPS_PER_BATCH: u64 = 4096;

// ============================================================================
// the request body a module's `serve_sync` understands.
// ============================================================================

/// a byte-level request against one qmdb-backed module's sync surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QmdbSyncReq {
    /// the store's CURRENT sync target (root + live op range).
    Target,
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
        QmdbSyncReq::Target => out.push(0u8),
        QmdbSyncReq::Ops {
            op_count,
            start_loc,
            max_ops,
            include_pinned,
        } => {
            out.push(1u8);
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
        0 => QmdbSyncReq::Target,
        1 => QmdbSyncReq::Ops {
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
/// keep older targets servable while the store keeps advancing. this is what a
/// qmdb-backed module's [`sdk::Module::serve_sync`] delegates to.
pub async fn serve<E>(db: &SyncDb<E>, req: &QmdbSyncReq) -> Result<Vec<u8>, sdk::Error>
where
    E: Context + BufferPooler,
{
    match req {
        QmdbSyncReq::Target => {
            let end = db.bounds().await.end;
            let start = db.sync_boundary();
            let range = commonware_utils::range::NonEmptyRange::new(start..end).map_err(|_| {
                sdk::Error::Module("store has no committed operations to sync".into())
            })?;
            let target = SyncTarget {
                root: db.root(),
                range,
            };
            Ok(target.encode().to_vec())
        }
        QmdbSyncReq::Ops {
            op_count,
            start_loc,
            max_ops,
            include_pinned,
        } => {
            let max_ops = NonZeroU64::new((*max_ops).min(MAX_OPS_PER_BATCH))
                .ok_or_else(|| sdk::Error::Module("max_ops must be non-zero".into()))?;
            let (proof, operations) = db
                .historical_proof(Location::new(*op_count), Location::new(*start_loc), max_ops)
                .await
                .map_err(|e| sdk::Error::Module(format!("historical proof failed: {e}")))?;
            let pinned_nodes = if *include_pinned {
                Some(
                    db.pinned_nodes_at(Location::new(*start_loc))
                        .await
                        .map_err(|e| sdk::Error::Module(format!("pinned nodes failed: {e}")))?,
                )
            } else {
                None
            };
            Ok(encode_ops_envelope(&OpsEnvelope {
                proof,
                operations,
                pinned_nodes,
            }))
        }
    }
}

/// convenience for module `serve_sync` impls: decode + serve in one call.
pub async fn serve_bytes<E>(db: &SyncDb<E>, req: &[u8]) -> Result<Vec<u8>, sdk::Error>
where
    E: Context + BufferPooler,
{
    let req = decode_qmdb_req(req).map_err(|e| sdk::Error::Module(e.to_string()))?;
    serve(db, &req).await
}

/// decode a served target payload back into a typed [`SyncTarget`].
pub fn decode_target(bytes: &[u8]) -> Result<SyncTarget, WireError> {
    SyncTarget::decode_cfg(bytes, &()).map_err(|e| WireError::Codec(format!("target: {e}")))
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
    module_id: String,
}

impl<C> RemoteQmdbResolver<C> {
    pub fn new(client: C, module_id: impl Into<String>) -> Self {
        Self {
            client,
            module_id: module_id.into(),
        }
    }
}

impl<C> RemoteQmdbResolver<C>
where
    C: SyncClient,
{
    /// fetch the module's CURRENT sync target from the serving peer.
    pub async fn fetch_target(&self) -> Result<SyncTarget, SyncError> {
        let body = encode_qmdb_req(&QmdbSyncReq::Target);
        let resp = self
            .client
            .request(SyncRequest::Module {
                module_id: self.module_id.clone(),
                body,
            })
            .await?;
        match resp {
            SyncResponse::Module(bytes) => Ok(decode_target(&bytes)?),
            SyncResponse::Error(e) => Err(SyncError::Server(e)),
            other => Err(SyncError::UnexpectedResponse(other.kind_name())),
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
                module_id: self.module_id.clone(),
                body,
            })
            .await?;
        let bytes = match resp {
            SyncResponse::Module(bytes) => bytes,
            SyncResponse::Error(e) => return Err(SyncError::Server(e)),
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
