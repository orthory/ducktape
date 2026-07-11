//! network state sync: how a joiner rebuilds every module from a RUNNING node.
//!
//! ## role (unified-node design, phases 2-4)
//!
//! state sync is the JOIN-TIME BOOTSTRAP, not the steady state. a standing
//! node follows the head by FOLDING finalized frames (the replica pipeline:
//! verified certificates through `consensus::FollowerOrderer` into the same
//! `OrderedNode` a validator drains); these lanes serve exactly three
//! moments — standing a fresh joiner up at a boundary, the `Frames` suffix
//! that gives a bootstrap (or a fold-driver gap) journal continuity, and a
//! promotion's boundary artifacts. `MAX_CAPTURES` is sized for that
//! join-shaped load: concurrent joiners, not a resident fleet re-polling.
//!
//! ## protocol
//!
//! four request shapes ride one request/response transport (any transport —
//! a p2p channel, a socket, an in-process loopback — via [`SyncClient`]):
//!
//! 1. **Manifest** — the server captures a consistent view of its registry at
//!    its latest finalized boundary (height, app-hash, and per-module root +
//!    sync payload) and caches it; the response lists `(module, root, kind)`.
//!    everything in one capture comes from ONE boundary, so the payloads
//!    compose to exactly the manifest's app-hash.
//! 2. **Chunk** — fetch a captured module's snapshot payload in bounded chunks
//!    (snapshot bytes can exceed a transport's frame cap; chunking is the
//!    protocol's job, not the transport's).
//! 3. **Module** — route module-defined bytes to a live module's
//!    [`serve_sync`](sdk::Module::serve_sync): the qmdb op-range lane. served
//!    with HISTORICAL proofs, so an in-flight joiner target stays servable
//!    while the source keeps finalizing new blocks.
//! 4. **Frames** — fetch a bounded recovery-journal suffix: finalized,
//!    non-discarded frame bytes plus their seal roots/app-hash, so a promoted
//!    joiner can persist the same replay suffix a restart would have.
//! 5. **IndexModules / IndexChunk** — the OPTIONAL shipped-index lane
//!    (indexable spec §7 lane 2): fluent31 checkpoint archives of the
//!    serving node's derived per-module read models, for an instant warm
//!    start. see the trust model — this is the one lane that is not
//!    verifiable, which is why it stays opt-in and why every consumer must
//!    treat a failed or refused fetch as "fall back to the from-state
//!    rebuild", never as an error.
//!
//! ## trust model
//!
//! the server is UNTRUSTED. every installable payload is verified by the
//! joiner against a root it obtained from the manifest — and the manifest's
//! app-hash is what the joiner ultimately recomposes and checks, so a lying
//! manifest fails the final compose. qmdb batches are merkle-verified by the
//! sync engine; snapshot installs re-derive the root before adopting bytes.
//! (the manifest app-hash itself is cross-checked against consensus when the
//! joiner later participates — a fabricated world still cannot vote.)
//!
//! the ONE exception is the shipped-index lane: the derived tier has no root
//! by design (it is never part of the app-hash), so its archives cannot be
//! verified — a joiner that opts in trusts the serving node for VIEW bytes
//! only. consensus state is untouched either way, a lying archive can never
//! fork the node, and the honest remedy for a bad shipment is the same as
//! for any damaged index: rebuild from verified state.
//!
//! ## wire format
//!
//! compact hand-rolled binary (u64-le length prefixes, strict bounds checks,
//! no trailing bytes) — NOT serde_json: snapshot payloads are bulk bytes and
//! json inflates raw bytes ~3.7x, which would silently shrink the usable
//! chunk size under a transport frame cap.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use commonware_codec::DecodeExt as _;
use host::{FinalizedBlock, Host};
use sdk::{ModuleId, ROOT_LEN, StateRoot, StateSyncHandle, UpgradeCoords};

pub mod dataplane;
pub mod p2p;
pub mod qmdb;
pub mod wire;

use wire::WireError;

/// a pinned qmdb sync target for a resolver-backed module at a boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverTarget {
    pub root: qmdb::SyncDigest,
    pub start: u64,
    pub op_count: u64,
}

impl ResolverTarget {
    pub fn to_sync_target(&self) -> Result<qmdb::SyncTarget, String> {
        let range = commonware_utils::range::NonEmptyRange::new(
            commonware_storage::merkle::Location::new(self.start)
                ..commonware_storage::merkle::Location::new(self.op_count),
        )
        .map_err(|_| {
            format!(
                "pinned resolver target has empty range {}..{}",
                self.start, self.op_count
            )
        })?;
        Ok(qmdb::SyncTarget {
            root: self.root,
            range,
        })
    }
}

/// max snapshot bytes per [`SyncResponse::Chunk`]. sized so a chunk plus
/// framing stays far under the mesh's 1 MiB message cap.
pub const CHUNK_LEN: usize = 256 * 1024;
/// max recovery frames per [`SyncResponse::Frames`] batch. suffix install loops
/// over batches; one response stays far below the mesh frame cap unless a
/// single frame itself is already too large for the transport.
pub const FRAME_BATCH_LEN: usize = 64;

/// how many boundary captures a server retains. more than one lets a second
/// joiner start syncing without invalidating the first joiner's in-flight
/// capture when the boundary advances between their manifest fetches.
pub const MAX_CAPTURES: usize = 4;
const MAX_LEASED_BOUNDARIES: usize = MAX_CAPTURES;

/// a deterministic state-sync boundary: both the app height and the module-root
/// composition served at that height.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BoundaryId {
    pub height: u64,
    pub app_hash: StateRoot,
}

impl Ord for BoundaryId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.height
            .cmp(&other.height)
            .then_with(|| self.app_hash.0.cmp(&other.app_hash.0))
    }
}

impl PartialOrd for BoundaryId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ============================================================================
// errors
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("wire error: {0}")]
    Wire(#[from] WireError),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("server error: {0}")]
    Server(String),
    #[error("unexpected response kind: {0}")]
    UnexpectedResponse(&'static str),
    #[error("module {module}: {reason}")]
    Module { module: ModuleId, reason: String },
    #[error("module {module}: pinned qmdb range pruned ({reason}); refetch manifest")]
    Pruned { module: ModuleId, reason: String },
    #[error("recovery frame range pruned after {requested_after}; retained from {retained_from}")]
    RangePruned {
        requested_after: u64,
        retained_from: u64,
    },
}

// ============================================================================
// frames
// ============================================================================

/// how a captured module's state travels to a joiner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadKind {
    /// no durable state; recreate at genesis.
    Stateless,
    /// self-contained snapshot bytes, fetched via [`SyncRequest::Chunk`].
    Snapshot,
    /// module-specific resolver lane via [`SyncRequest::Module`] (qmdb).
    Resolver,
    /// the module declared no sync surface — a joiner cannot rebuild it.
    Unsupported,
}

impl PayloadKind {
    fn to_u8(self) -> u8 {
        match self {
            Self::Stateless => 0,
            Self::Snapshot => 1,
            Self::Resolver => 2,
            Self::Unsupported => 3,
        }
    }

    fn from_u8(v: u8) -> Result<Self, WireError> {
        Ok(match v {
            0 => Self::Stateless,
            1 => Self::Snapshot,
            2 => Self::Resolver,
            3 => Self::Unsupported,
            other => return Err(WireError::BadTag("PayloadKind", other)),
        })
    }
}

/// recovery-equivalent disposition for a finalized, non-discarded frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameDisposition {
    Applied,
    Rejected,
}

impl FrameDisposition {
    fn to_u8(self) -> u8 {
        match self {
            Self::Applied => 0,
            Self::Rejected => 1,
        }
    }

    fn from_u8(v: u8) -> Result<Self, WireError> {
        Ok(match v {
            0 => Self::Applied,
            1 => Self::Rejected,
            other => return Err(WireError::BadTag("FrameDisposition", other)),
        })
    }
}

/// one finalized, non-discarded recovery frame and the seal that consensus
/// served at that height.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedFrame {
    pub height: u64,
    pub frame: Vec<u8>,
    pub disposition: FrameDisposition,
    pub roots: Vec<(ModuleId, StateRoot)>,
    pub app_hash: StateRoot,
}

/// one module's row in a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub module_id: ModuleId,
    pub root: StateRoot,
    pub kind: PayloadKind,
    pub resolver_target: Option<ResolverTarget>,
}

/// the joiner's picture of one captured boundary.
///
/// besides the module payloads, the manifest carries the boundary's CONSENSUS
/// COORDINATES — everything a syncing joiner needs to become a validator at
/// this exact boundary. like the app-hash, these are unauthenticated serving
/// hints under the same trust model: a lying epoch or base makes the joiner's
/// heights (and thus its app-hash) diverge, which fails loudly; a fabricated
/// world still cannot vote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub height: u64,
    pub app_hash: StateRoot,
    /// the consensus epoch whose engine was live at `height`.
    pub epoch: u64,
    /// that epoch's app-height base (`app_height = view_base + engine view`).
    pub view_base: u64,
    /// the epoch's engine participant set (raw public-key bytes) — NOT
    /// necessarily the valset projection at `height`, which may already
    /// stage a change awaiting its cutover.
    pub participants: Vec<Vec<u8>>,
    /// the epoch's RESIDENT set (transport standing, no quorum seat).
    pub residents: Vec<Vec<u8>>,
    /// the scheme-encoded finalization certificate for exactly `height`,
    /// when the serving node holds one (`None` right after a cutover, when
    /// the epoch has not finalized past its base — the joiner then spawns on
    /// the epoch's genesis floor instead).
    pub floor_cert: Option<Vec<u8>>,
    /// the agreed protocol version active at `height`. an UNAUTHENTICATED
    /// serving hint under the untrusted-server model — a lying value can at
    /// worst mis-preflight a joiner (refuse-to-boot, or boot-then-halt at the
    /// app-hash), never fork.
    pub current_version: u32,
    /// the single upgrade armed but not yet activated at `height`, if any.
    /// same trust caveat as `current_version`.
    pub pending_upgrade: Option<UpgradeCoords>,
    /// the highest protocol version any block at or after `height` needs — the
    /// joiner's boot preflight fence (`to_version` once `height >=
    /// pending.activation_height`, else `current_version`).
    pub required_min_version: u32,
    pub entries: Vec<ManifestEntry>,
}

impl Manifest {
    pub fn entry(&self, id: &str) -> Option<&ManifestEntry> {
        self.entries.iter().find(|e| e.module_id == id)
    }

    pub fn boundary_id(&self) -> BoundaryId {
        BoundaryId {
            height: self.height,
            app_hash: self.app_hash,
        }
    }

    /// boot preflight: fail loud when the local build's `max_supported`
    /// protocol version is below this boundary's `required_min_version`. an
    /// early, actionable refusal instead of an opaque post-rebuild app-hash
    /// mismatch. wired into every join lane (park, sync-only boot, validator
    /// recovery).
    pub fn preflight(&self, max_supported: u32) -> Result<(), sdk::UnsupportedVersion> {
        sdk::check_required_version(self.required_min_version, max_supported)
    }
}

/// a state-sync request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncRequest {
    /// capture (or reuse) the latest finalized boundary and describe it.
    Manifest,
    /// fetch a chunk of a captured module's snapshot payload.
    Chunk {
        boundary: BoundaryId,
        module_id: ModuleId,
        offset: u64,
    },
    /// route module-defined bytes to the live module's `serve_sync`.
    Module {
        boundary: BoundaryId,
        module_id: ModuleId,
        body: Vec<u8>,
    },
    /// fetch recovery-equivalent finalized frame records in `(after, up_to]`.
    Frames {
        after_height: u64,
        up_to_height: u64,
    },
    /// list the shipped-index databases attached at a leased boundary — the
    /// UNVERIFIED warm-start lane (indexable spec §7 lane 2). the derived
    /// tier has no root by design, so nothing here composes into the
    /// app-hash check: a joiner that opts in trusts the serving node's
    /// bytes; one that doesn't never sends this request and heals via the
    /// from-state rebuild instead.
    IndexModules { boundary: BoundaryId },
    /// fetch a chunk of one shipped-index database's archive blob. same
    /// trust caveat as [`SyncRequest::IndexModules`].
    IndexChunk {
        boundary: BoundaryId,
        db: String,
        offset: u64,
    },
    /// read the tip's consensus coordinates (membership, epoch, height) —
    /// the DETECTION lane; see [`TipCoords`].
    TipCoords,
    /// fetch one node-local content-addressed blob by its sha256 digest —
    /// the fetch-on-miss lane for host-staged bytes that consensus pins by
    /// hash but never carries (an agent's registered prompt above all). the
    /// HOST layer answers this from its blob store, not [`SyncServer`]:
    /// blobs are node-local staging, no capture or boundary is involved.
    /// content addressing makes the answer self-verifying — the requester
    /// re-hashes the bytes and drops a mismatch, so no trust attaches to
    /// which peer answered.
    Blob { digest: [u8; 32] },
}

/// the tip's consensus coordinates without a captured boundary — the
/// DETECTION lane. a parked or folding resident polls this to track
/// membership, standing, and epoch cutovers; answering costs the server no
/// capture, no lease, and no floor-cert alignment, so a fleet's routine
/// polling never contends with (or churns) the join-shaped capture cache
/// the Manifest lane is sized for. same trust model as the manifest's
/// coordinates: unauthenticated serving hints — action taken on them
/// (ascension, promotion) re-fetches a full [`Manifest`] and verifies its
/// floor certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TipCoords {
    pub height: u64,
    pub app_hash: StateRoot,
    pub epoch: u64,
    pub view_base: u64,
    pub participants: Vec<Vec<u8>>,
    pub residents: Vec<Vec<u8>>,
    /// whether the server holds the finalization certificate for exactly
    /// `height` — a liveness hint, never the certificate itself.
    pub has_floor: bool,
}

/// a state-sync response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncResponse {
    Manifest(Manifest),
    Chunk {
        total: u64,
        bytes: Vec<u8>,
    },
    Module(Vec<u8>),
    Frames {
        frames: Vec<FinalizedFrame>,
    },
    RangePruned {
        requested_after: u64,
        retained_from: u64,
    },
    /// the shipped-index databases attached at a boundary: `(db, blob_len)`
    /// pairs, in db order. empty means the source ships nothing (index off,
    /// poisoned, or nothing attached) — the joiner just falls back to the
    /// from-state rebuild. chunks come back as [`SyncResponse::Chunk`].
    IndexModules { entries: Vec<(String, u64)> },
    /// the tip's consensus coordinates — the [`SyncRequest::TipCoords`] answer.
    TipCoords(TipCoords),
    Error(String),
    /// the [`SyncRequest::Blob`] answer: the digest's bytes when this node
    /// holds them, `None` when it does not — an honest miss, never an error
    /// (the requester's fan-out treats a miss and an old peer's `Error`
    /// identically: try the next peer).
    Blob { bytes: Option<Vec<u8>> },
}

impl SyncResponse {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Manifest(_) => "Manifest",
            Self::Chunk { .. } => "Chunk",
            Self::Module(_) => "Module",
            Self::Frames { .. } => "Frames",
            Self::RangePruned { .. } => "RangePruned",
            Self::IndexModules { .. } => "IndexModules",
            Self::TipCoords(_) => "TipCoords",
            Self::Blob { .. } => "Blob",
            Self::Error(_) => "Error",
        }
    }
}

// ---- frame codec -----------------------------------------------------------

pub fn encode_request(req: &SyncRequest) -> Vec<u8> {
    let mut out = Vec::new();
    match req {
        SyncRequest::Manifest => out.push(0u8),
        SyncRequest::Chunk {
            boundary,
            module_id,
            offset,
        } => {
            out.push(1u8);
            out.extend_from_slice(&boundary.height.to_le_bytes());
            out.extend_from_slice(boundary.app_hash.as_bytes());
            wire::put_str(&mut out, module_id);
            out.extend_from_slice(&offset.to_le_bytes());
        }
        SyncRequest::Module {
            boundary,
            module_id,
            body,
        } => {
            out.push(2u8);
            out.extend_from_slice(&boundary.height.to_le_bytes());
            out.extend_from_slice(boundary.app_hash.as_bytes());
            wire::put_str(&mut out, module_id);
            wire::put_bytes(&mut out, body);
        }
        SyncRequest::Frames {
            after_height,
            up_to_height,
        } => {
            out.push(3u8);
            out.extend_from_slice(&after_height.to_le_bytes());
            out.extend_from_slice(&up_to_height.to_le_bytes());
        }
        SyncRequest::IndexModules { boundary } => {
            out.push(4u8);
            out.extend_from_slice(&boundary.height.to_le_bytes());
            out.extend_from_slice(boundary.app_hash.as_bytes());
        }
        SyncRequest::IndexChunk {
            boundary,
            db,
            offset,
        } => {
            out.push(5u8);
            out.extend_from_slice(&boundary.height.to_le_bytes());
            out.extend_from_slice(boundary.app_hash.as_bytes());
            wire::put_str(&mut out, db);
            out.extend_from_slice(&offset.to_le_bytes());
        }
        SyncRequest::TipCoords => out.push(6u8),
        SyncRequest::Blob { digest } => {
            out.push(7u8);
            out.extend_from_slice(digest);
        }
    }
    out
}

pub fn decode_request(bytes: &[u8]) -> Result<SyncRequest, WireError> {
    let mut buf = bytes;
    let tag = wire::take_u8(&mut buf)?;
    let req = match tag {
        0 => SyncRequest::Manifest,
        1 => SyncRequest::Chunk {
            boundary: BoundaryId {
                height: wire::take_u64(&mut buf)?,
                app_hash: StateRoot(wire::take_array::<ROOT_LEN>(&mut buf)?),
            },
            module_id: wire::take_str(&mut buf)?,
            offset: wire::take_u64(&mut buf)?,
        },
        2 => SyncRequest::Module {
            boundary: BoundaryId {
                height: wire::take_u64(&mut buf)?,
                app_hash: StateRoot(wire::take_array::<ROOT_LEN>(&mut buf)?),
            },
            module_id: wire::take_str(&mut buf)?,
            body: wire::take_bytes(&mut buf)?.to_vec(),
        },
        3 => SyncRequest::Frames {
            after_height: wire::take_u64(&mut buf)?,
            up_to_height: wire::take_u64(&mut buf)?,
        },
        4 => SyncRequest::IndexModules {
            boundary: BoundaryId {
                height: wire::take_u64(&mut buf)?,
                app_hash: StateRoot(wire::take_array::<ROOT_LEN>(&mut buf)?),
            },
        },
        5 => SyncRequest::IndexChunk {
            boundary: BoundaryId {
                height: wire::take_u64(&mut buf)?,
                app_hash: StateRoot(wire::take_array::<ROOT_LEN>(&mut buf)?),
            },
            db: wire::take_str(&mut buf)?,
            offset: wire::take_u64(&mut buf)?,
        },
        6 => SyncRequest::TipCoords,
        7 => SyncRequest::Blob {
            digest: wire::take_array::<32>(&mut buf)?,
        },
        other => return Err(WireError::BadTag("SyncRequest", other)),
    };
    wire::expect_empty(buf)?;
    Ok(req)
}

pub fn encode_response(resp: &SyncResponse) -> Vec<u8> {
    let mut out = Vec::new();
    match resp {
        SyncResponse::Manifest(m) => {
            out.push(0u8);
            out.extend_from_slice(&m.height.to_le_bytes());
            out.extend_from_slice(m.app_hash.as_bytes());
            out.extend_from_slice(&m.epoch.to_le_bytes());
            out.extend_from_slice(&m.view_base.to_le_bytes());
            out.extend_from_slice(&(m.participants.len() as u64).to_le_bytes());
            for p in &m.participants {
                wire::put_bytes(&mut out, p);
            }
            match &m.floor_cert {
                Some(cert) => {
                    out.push(1);
                    wire::put_bytes(&mut out, cert);
                }
                None => out.push(0),
            }
            // version fields (wire-format bump — see decode). placed before the
            // trailing entries so the entries forged-count guard sees an
            // accurate remaining-buffer bound.
            out.extend_from_slice(&m.current_version.to_le_bytes());
            match &m.pending_upgrade {
                Some(u) => {
                    out.push(1);
                    wire::put_str(&mut out, &u.name);
                    out.extend_from_slice(&u.activation_height.to_le_bytes());
                    out.extend_from_slice(&u.to_version.to_le_bytes());
                }
                None => out.push(0),
            }
            out.extend_from_slice(&m.required_min_version.to_le_bytes());
            out.extend_from_slice(&(m.entries.len() as u64).to_le_bytes());
            for e in &m.entries {
                wire::put_str(&mut out, &e.module_id);
                out.extend_from_slice(e.root.as_bytes());
                out.push(e.kind.to_u8());
                match &e.resolver_target {
                    Some(target) => {
                        out.push(1);
                        out.extend_from_slice(target.root.as_ref());
                        out.extend_from_slice(&target.start.to_le_bytes());
                        out.extend_from_slice(&target.op_count.to_le_bytes());
                    }
                    None => out.push(0),
                }
            }
            out.extend_from_slice(&(m.residents.len() as u64).to_le_bytes());
            for o in &m.residents {
                wire::put_bytes(&mut out, o);
            }
        }
        SyncResponse::Chunk { total, bytes } => {
            out.push(1u8);
            out.extend_from_slice(&total.to_le_bytes());
            wire::put_bytes(&mut out, bytes);
        }
        SyncResponse::Module(bytes) => {
            out.push(2u8);
            wire::put_bytes(&mut out, bytes);
        }
        SyncResponse::Frames { frames } => {
            out.push(3u8);
            out.extend_from_slice(&(frames.len() as u64).to_le_bytes());
            for frame in frames {
                out.extend_from_slice(&frame.height.to_le_bytes());
                wire::put_bytes(&mut out, &frame.frame);
                out.push(frame.disposition.to_u8());
                out.extend_from_slice(&(frame.roots.len() as u64).to_le_bytes());
                for (module_id, root) in &frame.roots {
                    wire::put_str(&mut out, module_id);
                    out.extend_from_slice(root.as_bytes());
                }
                out.extend_from_slice(frame.app_hash.as_bytes());
            }
        }
        SyncResponse::RangePruned {
            requested_after,
            retained_from,
        } => {
            out.push(4u8);
            out.extend_from_slice(&requested_after.to_le_bytes());
            out.extend_from_slice(&retained_from.to_le_bytes());
        }
        SyncResponse::Error(msg) => {
            out.push(5u8);
            wire::put_str(&mut out, msg);
        }
        SyncResponse::IndexModules { entries } => {
            out.push(6u8);
            out.extend_from_slice(&(entries.len() as u64).to_le_bytes());
            for (db, len) in entries {
                wire::put_str(&mut out, db);
                out.extend_from_slice(&len.to_le_bytes());
            }
        }
        SyncResponse::Blob { bytes } => {
            out.push(8u8);
            match bytes {
                Some(b) => {
                    out.push(1);
                    wire::put_bytes(&mut out, b);
                }
                None => out.push(0),
            }
        }
        SyncResponse::TipCoords(c) => {
            out.push(7u8);
            out.extend_from_slice(&c.height.to_le_bytes());
            out.extend_from_slice(c.app_hash.as_bytes());
            out.extend_from_slice(&c.epoch.to_le_bytes());
            out.extend_from_slice(&c.view_base.to_le_bytes());
            out.extend_from_slice(&(c.participants.len() as u64).to_le_bytes());
            for p in &c.participants {
                wire::put_bytes(&mut out, p);
            }
            out.extend_from_slice(&(c.residents.len() as u64).to_le_bytes());
            for r in &c.residents {
                wire::put_bytes(&mut out, r);
            }
            out.push(u8::from(c.has_floor));
        }
    }
    out
}

pub fn decode_response(bytes: &[u8]) -> Result<SyncResponse, WireError> {
    let mut buf = bytes;
    let tag = wire::take_u8(&mut buf)?;
    let resp = match tag {
        0 => {
            let height = wire::take_u64(&mut buf)?;
            let app_hash = StateRoot(wire::take_array::<ROOT_LEN>(&mut buf)?);
            let epoch = wire::take_u64(&mut buf)?;
            let view_base = wire::take_u64(&mut buf)?;
            let p = wire::take_u64(&mut buf)?;
            // each participant costs at least its 8-byte length prefix, so a
            // forged count can never drive allocation past the buffer.
            if p > (buf.len() / 8) as u64 {
                return Err(WireError::Codec(format!(
                    "participant count {p} exceeds the {} remaining bytes",
                    buf.len()
                )));
            }
            let mut participants = Vec::with_capacity(p as usize);
            for _ in 0..p {
                participants.push(wire::take_bytes(&mut buf)?.to_vec());
            }
            let floor_cert = match wire::take_u8(&mut buf)? {
                0 => None,
                1 => Some(wire::take_bytes(&mut buf)?.to_vec()),
                t => return Err(WireError::BadTag("floor_cert", t)),
            };
            let current_version = wire::take_u32(&mut buf)?;
            let pending_upgrade = match wire::take_u8(&mut buf)? {
                0 => None,
                1 => Some(UpgradeCoords {
                    name: wire::take_str(&mut buf)?,
                    activation_height: wire::take_u64(&mut buf)?,
                    to_version: wire::take_u32(&mut buf)?,
                }),
                t => return Err(WireError::BadTag("pending_upgrade", t)),
            };
            let required_min_version = wire::take_u32(&mut buf)?;
            let n = wire::take_u64(&mut buf)?;
            // each entry costs at least its id length prefix + root + kind, so
            // a forged count can never drive allocation past the buffer.
            if n > (buf.len() / (8 + ROOT_LEN + 1)) as u64 {
                return Err(WireError::Codec(format!(
                    "manifest count {n} exceeds the {} remaining bytes",
                    buf.len()
                )));
            }
            let mut entries = Vec::with_capacity(n as usize);
            for _ in 0..n {
                let module_id = wire::take_str(&mut buf)?;
                let root = StateRoot(wire::take_array::<ROOT_LEN>(&mut buf)?);
                let kind = PayloadKind::from_u8(wire::take_u8(&mut buf)?)?;
                let resolver_target = match wire::take_u8(&mut buf)? {
                    0 => None,
                    1 => {
                        let raw = wire::take_array::<ROOT_LEN>(&mut buf)?;
                        let digest = qmdb::SyncDigest::decode(raw.as_ref())
                            .map_err(|e| WireError::Codec(format!("resolver target root: {e}")))?;
                        Some(ResolverTarget {
                            root: digest,
                            start: wire::take_u64(&mut buf)?,
                            op_count: wire::take_u64(&mut buf)?,
                        })
                    }
                    t => return Err(WireError::BadTag("resolver_target", t)),
                };
                entries.push(ManifestEntry {
                    module_id,
                    root,
                    kind,
                    resolver_target,
                });
            }
            let o = wire::take_u64(&mut buf)?;
            // each resident costs at least its 8-byte length prefix, so a
            // forged count can never drive allocation past the buffer.
            if o > (buf.len() / 8) as u64 {
                return Err(WireError::Codec(format!(
                    "resident count {o} exceeds the {} remaining bytes",
                    buf.len()
                )));
            }
            let mut residents = Vec::with_capacity(o as usize);
            for _ in 0..o {
                residents.push(wire::take_bytes(&mut buf)?.to_vec());
            }
            SyncResponse::Manifest(Manifest {
                height,
                app_hash,
                epoch,
                view_base,
                participants,
                residents,
                floor_cert,
                current_version,
                pending_upgrade,
                required_min_version,
                entries,
            })
        }
        1 => SyncResponse::Chunk {
            total: wire::take_u64(&mut buf)?,
            bytes: wire::take_bytes(&mut buf)?.to_vec(),
        },
        2 => SyncResponse::Module(wire::take_bytes(&mut buf)?.to_vec()),
        3 => {
            let n = wire::take_u64(&mut buf)?;
            if n > FRAME_BATCH_LEN as u64 {
                return Err(WireError::Codec(format!(
                    "frame batch count {n} exceeds cap {FRAME_BATCH_LEN}"
                )));
            }
            let mut frames = Vec::with_capacity(n as usize);
            for _ in 0..n {
                let height = wire::take_u64(&mut buf)?;
                let frame = wire::take_bytes(&mut buf)?.to_vec();
                let disposition = FrameDisposition::from_u8(wire::take_u8(&mut buf)?)?;
                let roots_len = wire::take_u64(&mut buf)?;
                if roots_len > (buf.len() / (8 + ROOT_LEN)) as u64 {
                    return Err(WireError::Codec(format!(
                        "root count {roots_len} exceeds the {} remaining bytes",
                        buf.len()
                    )));
                }
                let mut roots = Vec::with_capacity(roots_len as usize);
                for _ in 0..roots_len {
                    roots.push((
                        wire::take_str(&mut buf)?,
                        StateRoot(wire::take_array::<ROOT_LEN>(&mut buf)?),
                    ));
                }
                let app_hash = StateRoot(wire::take_array::<ROOT_LEN>(&mut buf)?);
                frames.push(FinalizedFrame {
                    height,
                    frame,
                    disposition,
                    roots,
                    app_hash,
                });
            }
            SyncResponse::Frames { frames }
        }
        4 => SyncResponse::RangePruned {
            requested_after: wire::take_u64(&mut buf)?,
            retained_from: wire::take_u64(&mut buf)?,
        },
        5 => SyncResponse::Error(wire::take_str(&mut buf)?),
        6 => {
            let n = wire::take_u64(&mut buf)?;
            // each entry costs at least its name length prefix + blob length,
            // so a forged count can never drive allocation past the buffer.
            if n > (buf.len() / 16) as u64 {
                return Err(WireError::Codec(format!(
                    "index db count {n} exceeds the {} remaining bytes",
                    buf.len()
                )));
            }
            let mut entries = Vec::with_capacity(n as usize);
            for _ in 0..n {
                entries.push((wire::take_str(&mut buf)?, wire::take_u64(&mut buf)?));
            }
            SyncResponse::IndexModules { entries }
        }
        7 => {
            let height = wire::take_u64(&mut buf)?;
            let app_hash = StateRoot(wire::take_array::<ROOT_LEN>(&mut buf)?);
            let epoch = wire::take_u64(&mut buf)?;
            let view_base = wire::take_u64(&mut buf)?;
            let p = wire::take_u64(&mut buf)?;
            // each key costs at least its 8-byte length prefix, so a forged
            // count can never drive allocation past the buffer.
            if p > (buf.len() / 8) as u64 {
                return Err(WireError::Codec(format!(
                    "participant count {p} exceeds the {} remaining bytes",
                    buf.len()
                )));
            }
            let mut participants = Vec::with_capacity(p as usize);
            for _ in 0..p {
                participants.push(wire::take_bytes(&mut buf)?.to_vec());
            }
            let r = wire::take_u64(&mut buf)?;
            if r > (buf.len() / 8) as u64 {
                return Err(WireError::Codec(format!(
                    "resident count {r} exceeds the {} remaining bytes",
                    buf.len()
                )));
            }
            let mut residents = Vec::with_capacity(r as usize);
            for _ in 0..r {
                residents.push(wire::take_bytes(&mut buf)?.to_vec());
            }
            let has_floor = match wire::take_u8(&mut buf)? {
                0 => false,
                1 => true,
                t => return Err(WireError::BadTag("has_floor", t)),
            };
            SyncResponse::TipCoords(TipCoords {
                height,
                app_hash,
                epoch,
                view_base,
                participants,
                residents,
                has_floor,
            })
        }
        8 => SyncResponse::Blob {
            bytes: match wire::take_u8(&mut buf)? {
                0 => None,
                1 => Some(wire::take_bytes(&mut buf)?.to_vec()),
                t => return Err(WireError::BadTag("blob presence", t)),
            },
        },
        other => return Err(WireError::BadTag("SyncResponse", other)),
    };
    wire::expect_empty(buf)?;
    Ok(resp)
}

/// the rpc envelope pairing responses to in-flight requests over a shared
/// duplex transport (a p2p channel). `id` is requester-local.
pub fn encode_rpc(id: u64, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(body);
    out
}

pub fn decode_rpc(bytes: &[u8]) -> Result<(u64, &[u8]), WireError> {
    if bytes.len() < 8 {
        return Err(WireError::Truncated);
    }
    let (head, rest) = bytes.split_at(8);
    let id = u64::from_le_bytes(head.try_into().expect("split_at(8) yields 8 bytes"));
    Ok((id, rest))
}

// ============================================================================
// SERVER — the capture cache a running node answers from.
// ============================================================================

/// one captured module payload.
#[derive(Debug, Clone)]
enum CapturedPayload {
    Stateless,
    Snapshot(Vec<u8>),
    /// the module serves its own qmdb resolver lane live; the capture records the
    /// pinned op-range target (and the boundary root, in the entry).
    Resolver(ResolverTarget),
    /// an object-store resolver (duckfs-odb): the module serves its refs image
    /// and content-addressed objects live over `serve_sync`, with NO qmdb
    /// op-range target to pin — the boundary root in the entry is all the joiner
    /// needs to root-verify the refs it fetches over the same Module lane.
    ObjectResolver,
    Unsupported,
}

impl CapturedPayload {
    fn kind(&self) -> PayloadKind {
        match self {
            Self::Stateless => PayloadKind::Stateless,
            Self::Snapshot(_) => PayloadKind::Snapshot,
            // both resolver flavors advertise the same wire kind; a joiner
            // distinguishes them by whether the entry pins a qmdb target
            // (`resolver_target: Some` for qmdb, `None` for an object resolver).
            Self::Resolver(_) | Self::ObjectResolver => PayloadKind::Resolver,
            Self::Unsupported => PayloadKind::Unsupported,
        }
    }
}

#[derive(Debug, Clone)]
struct CapturedModule {
    root: StateRoot,
    payload: CapturedPayload,
}

/// consensus coordinates of a served boundary — captured WITH the module
/// payloads so a later manifest request for the same height serves one
/// consistent picture. the caller (the node's pump, which owns both the host
/// and the consensus wiring) supplies them per request; the floor-cert
/// contract is the caller's: pass it only when it certifies exactly the
/// current finalized height.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoundaryCoords {
    pub epoch: u64,
    pub view_base: u64,
    pub participants: Vec<Vec<u8>>,
    pub residents: Vec<Vec<u8>>,
    pub floor_cert: Option<Vec<u8>>,
    /// the agreed protocol version active at the served boundary. the caller
    /// stamps it from live upgrade-module state, like `epoch`/`view_base`.
    pub current_version: u32,
    /// the single upgrade armed but not yet activated at the served boundary.
    pub pending_upgrade: Option<UpgradeCoords>,
}

/// a consistent boundary capture: every payload from ONE finalized boundary.
#[derive(Debug, Clone)]
struct Capture {
    app_hash: StateRoot,
    coords: BoundaryCoords,
    modules: BTreeMap<ModuleId, CapturedModule>,
    /// shipped-index archive blobs, keyed by database name — the unverified
    /// warm-start lane. `None` until the serving node attaches them (cut
    /// lazily on the first index request, so joiners that never opt in cost
    /// nothing); riding the capture ties their lifetime to its lease/evict
    /// lifecycle.
    index_blobs: Option<BTreeMap<String, Vec<u8>>>,
}

/// a capture produced by the state owner, ready to install into a
/// [`SyncServer`] — the request/reply payload that lets serving run on a
/// different task than the host. opaque: only [`capture_boundary`] builds one
/// and only [`SyncServer::install_capture`] consumes it.
#[derive(Debug, Clone)]
pub struct CaptureData {
    app_hash: StateRoot,
    coords: BoundaryCoords,
    modules: BTreeMap<ModuleId, CapturedModule>,
}

/// capture the host's state at `finalized` — the STATE-OWNER half of serving:
/// the one call that must run on the task that owns the [`Host`]. the returned
/// [`CaptureData`] crosses to the serve task and installs via
/// [`SyncServer::install_capture`].
pub async fn capture_boundary(
    host: &Host,
    finalized: FinalizedBlock,
    coords: &BoundaryCoords,
) -> Result<(BoundaryId, CaptureData), String> {
    let snapshot = host
        .capture_finalized_snapshot(finalized)
        .map_err(|e| format!("capture failed: {e}"))?;
    let id = BoundaryId {
        height: finalized.height,
        app_hash: snapshot.app_hash,
    };
    let mut modules = BTreeMap::new();
    for m in snapshot.modules {
        let payload = match m.state_sync {
            StateSyncHandle::Stateless => CapturedPayload::Stateless,
            StateSyncHandle::SnapshotBytes(bytes) => CapturedPayload::Snapshot(bytes),
            // an object-store resolver has no qmdb op-range target to pin: its
            // refs image + content-addressed objects are served live over
            // `serve_sync`, root-verified by the joiner against the boundary
            // root already recorded in the entry. keep the qmdb arm below
            // UNCHANGED.
            StateSyncHandle::ResolverBacked { backend, .. } if backend == "duckfs-odb" => {
                CapturedPayload::ObjectResolver
            }
            StateSyncHandle::ResolverBacked { .. } => {
                let target = host
                    .resolver_sync_target(&m.id)
                    .await
                    .map_err(|e| format!("module {} sync target: {e}", m.id))?;
                if target.root != m.root {
                    return Err(format!(
                        "module {} resolver target root does not match boundary root",
                        m.id
                    ));
                }
                CapturedPayload::Resolver(ResolverTarget {
                    root: commonware_cryptography::sha256::Digest(target.root.0),
                    start: target.start,
                    op_count: target.op_count,
                })
            }
            StateSyncHandle::Unsupported { .. } => CapturedPayload::Unsupported,
        };
        modules.insert(
            m.id,
            CapturedModule {
                root: m.root,
                payload,
            },
        );
    }
    Ok((
        id,
        CaptureData {
            app_hash: snapshot.app_hash,
            coords: coords.clone(),
            modules,
        },
    ))
}

/// what a [`SyncServer::serve`] step needs from its driver: either the answer
/// itself, or one of the STATE TOUCHES only the host-owning task can make —
/// the request/reply seam that keeps serving off the consensus loop.
#[derive(Debug)]
pub enum ServeStep {
    /// the request resolved from served state alone.
    Reply(SyncResponse),
    /// Manifest: obtain the current finalized boundary from the state owner
    /// ([`capture_boundary`] there unless this server already holds the id —
    /// see [`SyncServer::known_boundaries`]), install/refresh it, then finish
    /// with [`SyncServer::manifest_for`].
    NeedBoundary,
    /// Module lane, checks passed: route `body` to the live host's
    /// `serve_sync` and wrap the bytes in [`SyncResponse::Module`].
    NeedModuleServe { module_id: ModuleId, body: Vec<u8> },
    /// Frames lane: read the recovery journal on the state owner.
    NeedFrames {
        after_height: u64,
        up_to_height: u64,
    },
    /// IndexModules on a leased-but-unattached boundary: cut the shipped-index
    /// blobs (or decline with an empty attach), [`SyncServer::attach_index`],
    /// then re-drive the request.
    NeedIndexCut { boundary: BoundaryId },
    /// TipCoords: read the tip's consensus coordinates from the state owner —
    /// no capture, no lease, no floor-cert alignment gate.
    NeedCoords,
}

/// the server side of the protocol: capture consistent boundary views on
/// demand, cache a few, and answer manifest/chunk requests from them; route
/// module-lane requests to the live host. hold one per node; drive it from the
/// same task that owns the host (answers between drains are automatically
/// consistent — no locks, no torn reads).
#[derive(Default)]
pub struct SyncServer {
    captures: BTreeMap<BoundaryId, Capture>,
    leased: BTreeMap<BoundaryId, u64>,
    lease_clock: u64,
}

impl SyncServer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lease(&mut self, id: BoundaryId) {
        self.touch_lease(id);
        self.release_leased_overflow();
        self.evict_unleased_overflow();
    }

    /// every installed capture's id — the `known` list a boundary request
    /// carries (bounded by [`MAX_CAPTURES`]).
    pub fn known_boundaries(&self) -> Vec<BoundaryId> {
        self.captures.keys().copied().collect()
    }

    /// install a state-owner-produced capture (evicting past the cache cap),
    /// or — when `id` is already held — refresh its consensus coordinates:
    /// an epoch cutover at a stalled boundary (a 1->2 admission is the
    /// canonical case) changes the coordinates without changing
    /// (height, app_hash), and a capture taken just before the cutover would
    /// otherwise serve its stale epoch/participants forever.
    pub fn install_capture(&mut self, id: BoundaryId, data: CaptureData) {
        match self.captures.get_mut(&id) {
            Some(capture) => {
                if capture.coords != data.coords {
                    capture.coords = data.coords;
                }
            }
            None => {
                self.captures.insert(
                    id,
                    Capture {
                        app_hash: data.app_hash,
                        coords: data.coords,
                        modules: data.modules,
                        index_blobs: None,
                    },
                );
                // spare the newborn: it is not leased until the manifest_for
                // that every install exists to answer, so when every older
                // capture holds a lease (leases never release, they only age
                // out by overflow) the newborn would be the sole eviction
                // candidate — evicting itself one step before its own
                // manifest lookup ("no capture at boundary N").
                self.evict_unleased_overflow_sparing(Some(id));
            }
        }
    }

    /// refresh a held capture's consensus coordinates without new payload —
    /// the known-boundary half of [`SyncServer::install_capture`].
    pub fn refresh_coords(&mut self, id: BoundaryId, coords: BoundaryCoords) {
        if let Some(capture) = self.captures.get_mut(&id)
            && capture.coords != coords
        {
            capture.coords = coords;
        }
    }

    /// lease `id` and build its manifest — the serve-side finish of a
    /// Manifest request, after the boundary round-trip installed/refreshed
    /// the capture.
    pub fn manifest_for(&mut self, id: BoundaryId) -> Result<SyncResponse, String> {
        self.lease(id);
        let capture = self
            .captures
            .get(&id)
            .ok_or_else(|| format!("no capture at boundary {} (refetch manifest)", id.height))?;
        let required_min_version = sdk::required_min_version(
            capture.coords.current_version,
            capture.coords.pending_upgrade.as_ref(),
            id.height,
        );
        Ok(SyncResponse::Manifest(Manifest {
            height: id.height,
            app_hash: capture.app_hash,
            epoch: capture.coords.epoch,
            view_base: capture.coords.view_base,
            participants: capture.coords.participants.clone(),
            residents: capture.coords.residents.clone(),
            floor_cert: capture.coords.floor_cert.clone(),
            current_version: capture.coords.current_version,
            pending_upgrade: capture.coords.pending_upgrade.clone(),
            required_min_version,
            entries: capture
                .modules
                .iter()
                .map(|(id, m)| ManifestEntry {
                    module_id: id.clone(),
                    root: m.root,
                    kind: m.payload.kind(),
                    resolver_target: match &m.payload {
                        CapturedPayload::Resolver(target) => Some(target.clone()),
                        _ => None,
                    },
                })
                .collect(),
        }))
    }

    /// one PURE serve step: answer what served state can, and name the state
    /// touch the driver must make otherwise (see [`ServeStep`]). this is the
    /// whole protocol minus the host — a serve task drives it off the
    /// consensus loop, round-tripping the named touches to the state owner.
    pub fn serve(&mut self, req: SyncRequest) -> ServeStep {
        let step = self.try_serve(req);
        match step {
            Ok(step) => step,
            Err(msg) => ServeStep::Reply(SyncResponse::Error(msg)),
        }
    }

    fn try_serve(&mut self, req: SyncRequest) -> Result<ServeStep, String> {
        Ok(match req {
            SyncRequest::Manifest => ServeStep::NeedBoundary,
            SyncRequest::TipCoords => ServeStep::NeedCoords,
            // the HOST layer answers blob fetches from its node-local store
            // BEFORE requests reach this server; one arriving here means the
            // host did not intercept — answer honestly instead of wedging.
            SyncRequest::Blob { .. } => {
                return Err("blob requests are answered by the host layer".into());
            }
            SyncRequest::Frames {
                after_height,
                up_to_height,
            } => ServeStep::NeedFrames {
                after_height,
                up_to_height,
            },
            SyncRequest::Chunk {
                boundary,
                module_id,
                offset,
            } => {
                let capture = self.leased_capture(boundary)?;
                let module = capture.modules.get(&module_id).ok_or_else(|| {
                    format!("no module {module_id} in capture {}", boundary.height)
                })?;
                let CapturedPayload::Snapshot(bytes) = &module.payload else {
                    return Err(format!("module {module_id} has no snapshot payload"));
                };
                let total = bytes.len() as u64;
                if offset > total {
                    return Err(format!(
                        "offset {offset} past the {total}-byte snapshot of {module_id}"
                    ));
                }
                let start = offset as usize;
                let end = (start + CHUNK_LEN).min(bytes.len());
                ServeStep::Reply(SyncResponse::Chunk {
                    total,
                    bytes: bytes[start..end].to_vec(),
                })
            }
            SyncRequest::Module {
                boundary,
                module_id,
                body,
            } => {
                let capture = self.leased_capture(boundary)?;
                let module = capture.modules.get(&module_id).ok_or_else(|| {
                    format!("no module {module_id} in capture {}", boundary.height)
                })?;
                // both resolver flavors serve their bytes live through the
                // module's `serve_sync` (qmdb op ranges, or duckfs refs/objects).
                if !matches!(
                    module.payload,
                    CapturedPayload::Resolver(_) | CapturedPayload::ObjectResolver
                ) {
                    return Err(format!("module {module_id} has no resolver payload"));
                }
                ServeStep::NeedModuleServe { module_id, body }
            }
            SyncRequest::IndexModules { boundary } => {
                let capture = self.leased_capture(boundary)?;
                let Some(blobs) = &capture.index_blobs else {
                    // unattached: the driver cuts + attaches (or declines with
                    // an empty attach) and re-drives.
                    return Ok(ServeStep::NeedIndexCut { boundary });
                };
                ServeStep::Reply(SyncResponse::IndexModules {
                    entries: blobs
                        .iter()
                        .map(|(db, blob)| (db.clone(), blob.len() as u64))
                        .collect(),
                })
            }
            SyncRequest::IndexChunk {
                boundary,
                db,
                offset,
            } => {
                let capture = self.leased_capture(boundary)?;
                let blob = capture
                    .index_blobs
                    .as_ref()
                    .and_then(|blobs| blobs.get(&db))
                    .ok_or_else(|| {
                        format!("no shipped index db {db} in capture {}", boundary.height)
                    })?;
                let total = blob.len() as u64;
                if offset > total {
                    return Err(format!(
                        "offset {offset} past the {total}-byte index archive of {db}"
                    ));
                }
                let start = offset as usize;
                let end = (start + CHUNK_LEN).min(blob.len());
                ServeStep::Reply(SyncResponse::Chunk {
                    total,
                    bytes: blob[start..end].to_vec(),
                })
            }
        })
    }

    /// whether shipped-index blobs are already attached at `id` — the caller
    /// (who owns the index store; this crate deliberately does not) checks
    /// this on an [`SyncRequest::IndexModules`] and cuts + attaches first
    /// when they are not.
    pub fn index_attached(&self, id: BoundaryId) -> bool {
        self.captures
            .get(&id)
            .is_some_and(|c| c.index_blobs.is_some())
    }

    /// attach shipped-index archive blobs (database name → encoded archive)
    /// to a leased capture. they are served by [`SyncRequest::IndexModules`]
    /// / [`SyncRequest::IndexChunk`] and live exactly as long as the capture.
    /// an empty map is a valid attachment: "this source ships nothing".
    pub fn attach_index(
        &mut self,
        id: BoundaryId,
        blobs: BTreeMap<String, Vec<u8>>,
    ) -> Result<(), String> {
        if !self.leased.contains_key(&id) {
            return Err(format!(
                "boundary {} {} is not leased (refetch manifest)",
                id.height,
                hex_root(&id.app_hash)
            ));
        }
        let capture = self
            .captures
            .get_mut(&id)
            .ok_or_else(|| format!("no capture at boundary {}", id.height))?;
        capture.index_blobs = Some(blobs);
        Ok(())
    }

    #[doc(hidden)]
    pub fn insert_capture_for_test(&mut self, id: BoundaryId) {
        self.captures.insert(
            id,
            Capture {
                app_hash: id.app_hash,
                coords: BoundaryCoords::default(),
                modules: BTreeMap::new(),
                index_blobs: None,
            },
        );
    }

    /// like [`SyncServer::insert_capture_for_test`] but through the REAL
    /// install path, eviction included — for tests exercising install-time
    /// cache behavior.
    #[doc(hidden)]
    pub fn install_capture_for_test(&mut self, id: BoundaryId) {
        self.install_capture(
            id,
            CaptureData {
                app_hash: id.app_hash,
                coords: BoundaryCoords::default(),
                modules: BTreeMap::new(),
            },
        );
    }

    #[doc(hidden)]
    pub fn has_capture(&self, id: BoundaryId) -> bool {
        self.captures.contains_key(&id)
    }

    #[doc(hidden)]
    pub fn leased_count_for_test(&self) -> usize {
        self.leased.len()
    }

    #[doc(hidden)]
    pub fn is_leased_for_test(&self, id: BoundaryId) -> bool {
        self.leased.contains_key(&id)
    }

    /// handle one decoded request. `finalized` is the node's latest applied
    /// boundary (None before the first block) and `coords` its consensus
    /// coordinates — both required for Manifest.
    pub async fn handle(
        &mut self,
        host: &Host,
        finalized: Option<FinalizedBlock>,
        coords: &BoundaryCoords,
        req: SyncRequest,
    ) -> SyncResponse {
        match self.try_handle(host, finalized, coords, req).await {
            Ok(resp) => resp,
            Err(msg) => SyncResponse::Error(msg),
        }
    }

    /// handle one still-encoded request frame; the transport loop calls this.
    pub async fn handle_frame(
        &mut self,
        host: &Host,
        finalized: Option<FinalizedBlock>,
        coords: &BoundaryCoords,
        frame: &[u8],
    ) -> Vec<u8> {
        let resp = match decode_request(frame) {
            Ok(req) => self.handle(host, finalized, coords, req).await,
            Err(e) => SyncResponse::Error(format!("bad request frame: {e}")),
        };
        encode_response(&resp)
    }

    /// the composed one-owner path: drive [`SyncServer::serve`] and make every
    /// state touch inline against `host`. callers that split ownership (the
    /// node's off-loop serve task) drive `serve()` themselves and round-trip
    /// the touches to the state owner instead.
    async fn try_handle(
        &mut self,
        host: &Host,
        finalized: Option<FinalizedBlock>,
        coords: &BoundaryCoords,
        req: SyncRequest,
    ) -> Result<SyncResponse, String> {
        match self.serve(req) {
            ServeStep::Reply(resp) => Ok(resp),
            ServeStep::NeedBoundary => {
                let finalized = finalized.ok_or("no finalized boundary to serve yet")?;
                let (id, data) = capture_boundary(host, finalized, coords).await?;
                self.install_capture(id, data);
                self.manifest_for(id)
            }
            ServeStep::NeedModuleServe { module_id, body } => host
                .serve_sync(&module_id, &body)
                .await
                .map(SyncResponse::Module)
                .map_err(|e| format!("module {module_id} serve_sync: {e}")),
            ServeStep::NeedFrames { .. } => {
                Err("frame range requests require the recovery journal".into())
            }
            ServeStep::NeedIndexCut { .. } => {
                // this owner attaches nothing (no index store here) — an EMPTY
                // list, not an error, so the joiner cleanly falls back to the
                // from-state rebuild.
                Ok(SyncResponse::IndexModules {
                    entries: Vec::new(),
                })
            }
            ServeStep::NeedCoords => {
                let finalized = finalized.ok_or("no finalized boundary to serve yet")?;
                Ok(SyncResponse::TipCoords(TipCoords {
                    height: finalized.height,
                    app_hash: finalized.app_hash,
                    epoch: coords.epoch,
                    view_base: coords.view_base,
                    participants: coords.participants.clone(),
                    residents: coords.residents.clone(),
                    has_floor: coords.floor_cert.is_some(),
                }))
            }
        }
    }

    fn leased_capture(&mut self, boundary: BoundaryId) -> Result<&Capture, String> {
        if !self.leased.contains_key(&boundary) {
            return Err(format!(
                "boundary {} {} is not leased (refetch manifest)",
                boundary.height,
                hex_root(&boundary.app_hash)
            ));
        }
        self.touch_lease(boundary);
        self.captures.get(&boundary).ok_or_else(|| {
            format!(
                "no capture at boundary {} {} (refetch manifest)",
                boundary.height,
                hex_root(&boundary.app_hash)
            )
        })
    }

    fn touch_lease(&mut self, id: BoundaryId) {
        self.lease_clock = self.lease_clock.wrapping_add(1);
        self.leased.insert(id, self.lease_clock);
    }

    fn release_leased_overflow(&mut self) {
        while self.leased.len() > MAX_LEASED_BOUNDARIES {
            let oldest = self
                .leased
                .iter()
                .min_by_key(|(_, tick)| **tick)
                .map(|(id, _)| *id)
                .expect("leased len above cap implies at least one lease");
            self.leased.remove(&oldest);
        }
    }

    fn evict_unleased_overflow(&mut self) {
        self.evict_unleased_overflow_sparing(None);
    }

    /// evict past the cache cap, never touching a leased capture — nor
    /// `spared`, the id an in-flight install is about to manifest. sparing an
    /// unleased newborn can leave the cache one over cap for the single step
    /// until its `manifest_for` lease lands and the next eviction rebalances.
    fn evict_unleased_overflow_sparing(&mut self, spared: Option<BoundaryId>) {
        while self.captures.len() > MAX_CAPTURES {
            let Some(oldest) = self
                .captures
                .keys()
                .copied()
                .find(|id| !self.leased.contains_key(id) && Some(*id) != spared)
            else {
                break;
            };
            self.captures
                .remove(&oldest)
                .expect("removed capture existed");
        }
    }
}

// ============================================================================
// CLIENT — transport seam + fetch helpers.
// ============================================================================

/// the joiner's transport seam: move one request to the serving peer and bring
/// its response back. `Clone + Send + Sync` because the qmdb sync engine holds
/// the resolver (and therefore the client) across concurrent fetches.
pub trait SyncClient: Clone + Send + Sync + 'static {
    fn request(
        &self,
        req: SyncRequest,
    ) -> impl std::future::Future<Output = Result<SyncResponse, SyncError>> + Send;
}

/// fetch the serving peer's manifest.
pub async fn fetch_manifest<C: SyncClient>(client: &C) -> Result<Manifest, SyncError> {
    match client.request(SyncRequest::Manifest).await? {
        SyncResponse::Manifest(m) => Ok(m),
        SyncResponse::Error(e) => Err(SyncError::Server(e)),
        other => Err(SyncError::UnexpectedResponse(other.kind_name())),
    }
}

/// fetch the serving peer's tip coordinates — the detection lane: membership,
/// epoch, and height without capturing a boundary. action taken on the answer
/// (ascension, promotion) re-fetches a full [`Manifest`] and verifies its
/// floor certificate.
pub async fn fetch_tip_coords<C: SyncClient>(client: &C) -> Result<TipCoords, SyncError> {
    match client.request(SyncRequest::TipCoords).await? {
        SyncResponse::TipCoords(c) => Ok(c),
        SyncResponse::Error(e) => Err(SyncError::Server(e)),
        other => Err(SyncError::UnexpectedResponse(other.kind_name())),
    }
}

/// reassemble one chunked payload: `req(offset)` builds the per-chunk request
/// (Chunk or IndexChunk), `module` and `what` shape the mid-payload error.
async fn fetch_chunked<C: SyncClient>(
    client: &C,
    module: &str,
    what: &str,
    req: impl Fn(u64) -> SyncRequest,
) -> Result<Vec<u8>, SyncError> {
    let mut out: Vec<u8> = Vec::new();
    loop {
        match client.request(req(out.len() as u64)).await? {
            SyncResponse::Chunk { total, bytes } => {
                if bytes.is_empty() && out.len() < total as usize {
                    return Err(SyncError::Module {
                        module: module.to_string(),
                        reason: format!("server returned an empty {what} mid-payload"),
                    });
                }
                out.extend_from_slice(&bytes);
                if out.len() as u64 >= total {
                    // a lying `total` smaller than the stream is impossible:
                    // the server slices from one captured Vec, and we stop at
                    // exactly `total`.
                    out.truncate(total as usize);
                    return Ok(out);
                }
            }
            SyncResponse::Error(e) => return Err(SyncError::Server(e)),
            other => return Err(SyncError::UnexpectedResponse(other.kind_name())),
        }
    }
}

/// fetch a captured module's full snapshot payload, chunk by chunk.
pub async fn fetch_snapshot<C: SyncClient>(
    client: &C,
    boundary: BoundaryId,
    module_id: &str,
) -> Result<Vec<u8>, SyncError> {
    fetch_chunked(client, module_id, "chunk", |offset| SyncRequest::Chunk {
        boundary,
        module_id: module_id.to_string(),
        offset,
    })
    .await
}

// ---- the shipped-index archive framing ------------------------------------
// one shipped database travels as a single blob: `(file name, file bytes)`
// pairs in the store's file order, wire-framed like everything else here.
// the joiner hands the decoded set to the indexer's staging writer, which is
// where hostile names (traversal, hidden files, the engine lock) are refused
// — one enforcement point, at the trust boundary that touches disk.

/// flatten one database's checkpoint file set into an archive blob.
pub fn encode_index_archive(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, bytes) in files {
        wire::put_str(&mut out, name);
        wire::put_bytes(&mut out, bytes);
    }
    out
}

/// decode an archive blob back into its file set. structural only — name
/// policy is the staging writer's.
pub fn decode_index_archive(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, WireError> {
    let mut buf = bytes;
    let mut files = Vec::new();
    while !buf.is_empty() {
        let name = wire::take_str(&mut buf)?;
        let bytes = wire::take_bytes(&mut buf)?.to_vec();
        files.push((name, bytes));
    }
    Ok(files)
}

/// list the shipped-index databases a source attached at a boundary. empty
/// means the source ships nothing — fall back to the from-state rebuild.
pub async fn fetch_index_modules<C: SyncClient>(
    client: &C,
    boundary: BoundaryId,
) -> Result<Vec<(String, u64)>, SyncError> {
    match client.request(SyncRequest::IndexModules { boundary }).await? {
        SyncResponse::IndexModules { entries } => Ok(entries),
        SyncResponse::Error(e) => Err(SyncError::Server(e)),
        other => Err(SyncError::UnexpectedResponse(other.kind_name())),
    }
}

/// fetch one shipped-index database's full archive blob, chunk by chunk —
/// the index twin of [`fetch_snapshot`].
pub async fn fetch_index_db<C: SyncClient>(
    client: &C,
    boundary: BoundaryId,
    db: &str,
) -> Result<Vec<u8>, SyncError> {
    fetch_chunked(client, db, "index chunk", |offset| SyncRequest::IndexChunk {
        boundary,
        db: db.to_string(),
        offset,
    })
    .await
}

/// fetch a finite, ordered recovery-frame suffix in bounded batches.
pub async fn fetch_frames<C: SyncClient>(
    client: &C,
    after_height: u64,
    up_to_height: u64,
) -> Result<Vec<FinalizedFrame>, SyncError> {
    if after_height > up_to_height {
        return Err(SyncError::Server(format!(
            "invalid frame range ({after_height}, {up_to_height}]"
        )));
    }
    let mut out = Vec::new();
    let mut after = after_height;
    while after < up_to_height {
        let resp = client
            .request(SyncRequest::Frames {
                after_height: after,
                up_to_height,
            })
            .await?;
        match resp {
            SyncResponse::Frames { frames } => {
                if frames.is_empty() {
                    return Err(SyncError::Server(format!(
                        "server returned empty frame batch for non-empty range \
                         ({after}, {up_to_height}]"
                    )));
                }
                let mut last = after;
                for frame in frames {
                    if frame.height <= last || frame.height > up_to_height {
                        return Err(SyncError::Server(format!(
                            "server returned out-of-range frame height {} for \
                             ({after}, {up_to_height}]",
                            frame.height
                        )));
                    }
                    last = frame.height;
                    out.push(frame);
                }
                after = last;
            }
            SyncResponse::RangePruned {
                requested_after,
                retained_from,
            } => {
                return Err(SyncError::RangePruned {
                    requested_after,
                    retained_from,
                });
            }
            SyncResponse::Error(e) => return Err(SyncError::Server(e)),
            other => return Err(SyncError::UnexpectedResponse(other.kind_name())),
        }
    }
    Ok(out)
}

// ============================================================================
// OBJECT-RESOLVER DRIVER — the duckfs-odb ("object possession") sync path.
// ============================================================================
//
// unlike the qmdb resolver (a merkle op-range engine), an object-store module
// syncs by CONTENT-ADDRESSED FETCH: install the boundary refs image, then walk
// the reachable-but-absent object set (a BFS whose children are only revealed as
// their parents arrive), fetching each layer over the module's `serve_sync` lane
// until every reachable object is present. this crate is platform surface below
// the module crates, so it cannot name the module's types — the loop is generic
// over two seams the module (or the node glue) supplies:
//
//   * [`ModuleLane`]  — move one `serve_sync` request/response for the module.
//   * [`ObjectFetch`] — the module's install / missing / ingest / possession ops
//                       and its own request/response wire (encode/decode).
//
// the driver owns ONLY the control flow + the full-possession gate, so a joiner
// reports READY only once it holds every object — never on the refs alone.

/// move one `serve_sync` request to the serving peer for `module_id` and bring
/// its response bytes back. deliberately NOT [`SyncClient`]: this lane is driven
/// sequentially and need not be `Send` (an object-store module's `serve_sync`
/// future is `?Send`), so a test can back it with a source module in-process.
pub trait ModuleLane {
    fn fetch(
        &self,
        module_id: &str,
        body: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, SyncError>>;
}

/// a lane backed by a real [`SyncClient`] pinned at a manifest boundary: every
/// fetch is a boundary-scoped [`SyncRequest::Module`] round trip, exactly the
/// lane the server routes to `host.serve_sync`.
#[derive(Clone)]
pub struct ClientModuleLane<C> {
    client: C,
    boundary: BoundaryId,
}

impl<C> ClientModuleLane<C> {
    pub fn new(client: C, boundary: BoundaryId) -> Self {
        Self { client, boundary }
    }
}

impl<C: SyncClient> ModuleLane for ClientModuleLane<C> {
    async fn fetch(&self, module_id: &str, body: Vec<u8>) -> Result<Vec<u8>, SyncError> {
        match self
            .client
            .request(SyncRequest::Module {
                boundary: self.boundary,
                module_id: module_id.to_string(),
                body,
            })
            .await?
        {
            SyncResponse::Module(bytes) => Ok(bytes),
            SyncResponse::Error(e) => Err(qmdb::module_lane_error(module_id, e)),
            other => Err(SyncError::UnexpectedResponse(other.kind_name())),
        }
    }
}

/// the joiner-side seam an object-store ("duckfs-odb") module presents to the
/// generic possession driver: the module owns its `serve_sync` WIRE (encode the
/// request bodies, decode the replies) and its verify-then-store ops; the driver
/// owns the loop. all ops are sync — an object-store module's install / walk /
/// ingest touch only local durable state.
pub trait ObjectFetch {
    /// the `serve_sync` body requesting the boundary refs image.
    fn refs_request(&self) -> Vec<u8>;

    /// verify-then-install a served refs reply against `root`, persisting the
    /// durable envelope at the SYNC-TARGET `height` (a fresh joiner must not
    /// persist height 0 — a restart would replay from a pruned genesis).
    fn install_refs(&mut self, reply: &[u8], root: StateRoot, height: u64) -> Result<(), String>;

    /// the `serve_sync` body requesting up to `limit` reachable-but-absent
    /// objects, or `None` when possession is already complete (nothing missing).
    fn missing_request(&self, limit: usize) -> Result<Option<Vec<u8>>, String>;

    /// verify-then-store a served object reply (each id re-hashed inside);
    /// returns how many objects LANDED. a batch that lands zero while objects are
    /// still missing means the source pruned below the boundary — the driver
    /// stops rather than livelock.
    fn ingest(&mut self, reply: &[u8]) -> Result<usize, String>;

    /// whether every object the committed refs reach is now present.
    fn possession_complete(&self) -> Result<bool, String>;
}

/// drive an object-store module to FULL object possession at a manifest
/// boundary: install the boundary refs (root-verified) at `height`, then loop
/// { missing -> fetch -> ingest } until nothing is missing, and gate READY on
/// possession being genuinely complete. `batch` bounds the objects requested per
/// round (must not exceed the module's `serve_sync` id cap).
pub async fn sync_object_possession<L, M>(
    lane: &L,
    module_id: &str,
    root: StateRoot,
    height: u64,
    module: &mut M,
    batch: usize,
) -> Result<(), SyncError>
where
    L: ModuleLane,
    M: ObjectFetch,
{
    let module_err = |reason: String| SyncError::Module {
        module: module_id.to_string(),
        reason,
    };

    // 1. install the boundary refs image (root-verified) at the sync-target
    //    height. a mismatch (the source advanced past the captured boundary)
    //    fails here — the caller refetches the manifest and retries.
    let refs_reply = lane.fetch(module_id, module.refs_request()).await?;
    module
        .install_refs(&refs_reply, root, height)
        .map_err(module_err)?;

    // 2. the possession walk. each round reveals at least the newly-arrived
    //    layer's children, so the store strictly grows; the loop terminates when
    //    the reachable set is fully present (`missing_request` -> None).
    while let Some(body) = module.missing_request(batch).map_err(module_err)? {
        let reply = lane.fetch(module_id, body).await?;
        let landed = module.ingest(&reply).map_err(module_err)?;
        if landed == 0 {
            return Err(module_err(
                "source served no requested object (pruned below the boundary); \
                 refetch the manifest"
                    .into(),
            ));
        }
    }

    // 3. the full-possession gate — READY only when every reachable object is
    //    held, never on the refs image alone.
    if !module.possession_complete().map_err(module_err)? {
        return Err(module_err(
            "object fetch loop terminated before full possession".into(),
        ));
    }
    Ok(())
}

fn hex_root(root: &StateRoot) -> String {
    root.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_frames_round_trip() {
        for req in [
            SyncRequest::Manifest,
            SyncRequest::Chunk {
                boundary: BoundaryId {
                    height: 42,
                    app_hash: StateRoot([4u8; ROOT_LEN]),
                },
                module_id: "forge".into(),
                offset: 1 << 20,
            },
            SyncRequest::Module {
                boundary: BoundaryId {
                    height: 42,
                    app_hash: StateRoot([4u8; ROOT_LEN]),
                },
                module_id: "kv".into(),
                body: vec![1, 2, 3],
            },
            SyncRequest::Frames {
                after_height: 42,
                up_to_height: 48,
            },
            SyncRequest::IndexModules {
                boundary: BoundaryId {
                    height: 42,
                    app_hash: StateRoot([4u8; ROOT_LEN]),
                },
            },
            SyncRequest::IndexChunk {
                boundary: BoundaryId {
                    height: 42,
                    app_hash: StateRoot([4u8; ROOT_LEN]),
                },
                db: "_blocks".into(),
                offset: 1 << 18,
            },
            SyncRequest::Blob { digest: [7u8; 32] },
        ] {
            let bytes = encode_request(&req);
            assert_eq!(decode_request(&bytes).unwrap(), req);
        }
    }

    #[test]
    fn blob_response_round_trips_hit_and_miss() {
        for resp in [
            SyncResponse::Blob {
                bytes: Some(b"You are quack.".to_vec()),
            },
            SyncResponse::Blob { bytes: None },
        ] {
            let bytes = encode_response(&resp);
            assert_eq!(decode_response(&bytes).unwrap(), resp);
        }
        // a truncated presence flag rejects instead of decoding garbage.
        let mut framed = encode_response(&SyncResponse::Blob { bytes: None });
        framed.truncate(1);
        assert!(decode_response(&framed).is_err());
    }

    #[test]
    fn response_frames_round_trip() {
        for resp in [
            SyncResponse::Manifest(Manifest {
                height: 7,
                app_hash: StateRoot([9u8; ROOT_LEN]),
                epoch: 2,
                view_base: 5,
                participants: vec![vec![3u8; 32], vec![4u8; 32]],
                // non-empty: exercises the additive resident wire tail.
                residents: vec![vec![5u8; 32]],
                floor_cert: Some(vec![0xCC; 96]),
                // a pending upgrade set: exercise the Some arm of the tail.
                current_version: 3,
                pending_upgrade: Some(UpgradeCoords {
                    name: "v4".into(),
                    activation_height: 100,
                    to_version: 4,
                }),
                required_min_version: 3,
                entries: vec![
                    ManifestEntry {
                        module_id: "kv".into(),
                        root: StateRoot([1u8; ROOT_LEN]),
                        kind: PayloadKind::Resolver,
                        resolver_target: Some(ResolverTarget {
                            root: commonware_cryptography::sha256::Digest([1u8; ROOT_LEN]),
                            start: 1,
                            op_count: 2,
                        }),
                    },
                    ManifestEntry {
                        module_id: "valset".into(),
                        root: StateRoot([2u8; ROOT_LEN]),
                        kind: PayloadKind::Snapshot,
                        resolver_target: None,
                    },
                ],
            }),
            // a fresh-epoch boundary: no finalization past the base yet, so
            // no floor certificate — the joiner spawns on the genesis floor.
            // version tail at defaults (no upgrade scheduled).
            SyncResponse::Manifest(Manifest {
                height: 12,
                app_hash: StateRoot([8u8; ROOT_LEN]),
                epoch: 1,
                view_base: 12,
                participants: vec![vec![3u8; 32]],
                residents: vec![],
                floor_cert: None,
                current_version: 0,
                pending_upgrade: None,
                required_min_version: 0,
                entries: vec![],
            }),
            SyncResponse::Chunk {
                total: 10,
                bytes: vec![0xAB; 10],
            },
            SyncResponse::Module(vec![4, 5]),
            SyncResponse::Frames {
                frames: vec![FinalizedFrame {
                    height: 8,
                    frame: vec![0xAB, 0xCD],
                    disposition: FrameDisposition::Applied,
                    roots: vec![("kv".into(), StateRoot([3u8; ROOT_LEN]))],
                    app_hash: StateRoot([4u8; ROOT_LEN]),
                }],
            },
            SyncResponse::RangePruned {
                requested_after: 10,
                retained_from: 12,
            },
            SyncResponse::Error("nope".into()),
            SyncResponse::IndexModules {
                entries: vec![("chat".into(), 4096), ("_blocks".into(), 0)],
            },
            SyncResponse::IndexModules { entries: vec![] },
        ] {
            let bytes = encode_response(&resp);
            assert_eq!(decode_response(&bytes).unwrap(), resp);
        }
    }

    #[test]
    fn index_archive_round_trips_and_rejects_truncation() {
        let files = vec![
            ("manifest-000001".to_string(), vec![1u8, 2, 3]),
            ("sst-000001.tbl".to_string(), vec![0xAB; 300]),
            ("vlog-000001.vlog".to_string(), Vec::new()),
        ];
        let blob = encode_index_archive(&files);
        assert_eq!(decode_index_archive(&blob).unwrap(), files);
        assert_eq!(decode_index_archive(&[]).unwrap(), Vec::new());
        // any cut inside a frame is a loud decode error, not a short file.
        assert!(decode_index_archive(&blob[..blob.len() - 1]).is_err());
        assert!(decode_index_archive(&blob[..9]).is_err());
    }

    #[test]
    fn truncated_and_trailing_frames_reject() {
        let bytes = encode_request(&SyncRequest::Chunk {
            boundary: BoundaryId {
                height: 1,
                app_hash: StateRoot([1u8; ROOT_LEN]),
            },
            module_id: "m".into(),
            offset: 0,
        });
        assert!(
            decode_request(&bytes[..bytes.len() - 1]).is_err(),
            "truncation rejects"
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode_request(&trailing).is_err(), "trailing bytes reject");
    }

    #[test]
    fn rpc_envelope_round_trips() {
        let framed = encode_rpc(99, b"body");
        let (id, body) = decode_rpc(&framed).unwrap();
        assert_eq!(id, 99);
        assert_eq!(body, b"body");
        assert!(decode_rpc(&framed[..7]).is_err(), "short envelope rejects");
    }

    #[test]
    fn forged_manifest_counts_reject_before_allocation() {
        // header: tag 0, height, app_hash, epoch, view_base, then a forged
        // PARTICIPANT count far past the buffer.
        let mut bytes = vec![0u8];
        bytes.extend_from_slice(&1u64.to_le_bytes());
        bytes.extend_from_slice(&[0u8; ROOT_LEN]);
        bytes.extend_from_slice(&2u64.to_le_bytes());
        bytes.extend_from_slice(&3u64.to_le_bytes());
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());
        assert!(decode_response(&bytes).is_err());

        // same header, zero participants + no floor cert + default version
        // tail (current_version, pending tag None, required_min), then a
        // forged ENTRY count.
        let mut bytes = vec![0u8];
        bytes.extend_from_slice(&1u64.to_le_bytes());
        bytes.extend_from_slice(&[0u8; ROOT_LEN]);
        bytes.extend_from_slice(&2u64.to_le_bytes());
        bytes.extend_from_slice(&3u64.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.push(0); // floor_cert: None
        bytes.extend_from_slice(&0u32.to_le_bytes()); // current_version
        bytes.push(0); // pending_upgrade: None
        bytes.extend_from_slice(&0u32.to_le_bytes()); // required_min_version
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());
        assert!(decode_response(&bytes).is_err());
    }

    #[test]
    fn decode_response_rejects_truncated_version_tail() {
        // a manifest frame whose version tail is cut mid-field must fail
        // cleanly (no panic), not silently default.
        let resp = SyncResponse::Manifest(Manifest {
            height: 7,
            app_hash: StateRoot([9u8; ROOT_LEN]),
            epoch: 2,
            view_base: 5,
            participants: vec![],
            residents: vec![],
            floor_cert: None,
            current_version: 3,
            pending_upgrade: None,
            required_min_version: 3,
            entries: vec![],
        });
        let bytes = encode_response(&resp);
        // drop the trailing residents-count and entries-count u64s + the
        // required_min u32 + part of the pending tag, landing inside the
        // version tail.
        for cut in 1..=21 {
            let torn = &bytes[..bytes.len() - cut];
            assert!(
                decode_response(torn).is_err(),
                "truncation at -{cut} must reject"
            );
        }
    }

    #[test]
    fn manifest_preflight_gates_on_required_min() {
        let m = Manifest {
            height: 7,
            app_hash: StateRoot([0u8; ROOT_LEN]),
            epoch: 0,
            view_base: 0,
            participants: vec![],
            residents: vec![],
            floor_cert: None,
            current_version: 3,
            pending_upgrade: None,
            required_min_version: 3,
            entries: vec![],
        };
        assert!(m.preflight(3).is_ok());
        assert!(m.preflight(4).is_ok());
        let err = m.preflight(2).expect_err("under-versioned joiner");
        assert_eq!(err.required_min, 3);
        assert_eq!(err.max_supported, 2);
    }
}
