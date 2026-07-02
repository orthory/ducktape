//! network state sync: how a joiner rebuilds every module from a RUNNING node.
//!
//! ## protocol
//!
//! three request shapes ride one request/response transport (any transport —
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
//! ## wire format
//!
//! compact hand-rolled binary (u64-le length prefixes, strict bounds checks,
//! no trailing bytes) — NOT serde_json: snapshot payloads are bulk bytes and
//! json inflates raw bytes ~3.7x, which would silently shrink the usable
//! chunk size under a transport frame cap.

use std::collections::BTreeMap;

use host::{FinalizedBlock, Host};
use sdk::{ModuleId, StateRoot, StateSyncHandle, ROOT_LEN};

pub mod p2p;
pub mod qmdb;
pub mod wire;

use wire::WireError;

/// max snapshot bytes per [`SyncResponse::Chunk`]. sized so a chunk plus
/// framing stays far under the mesh's 1 MiB message cap.
pub const CHUNK_LEN: usize = 256 * 1024;

/// how many boundary captures a server retains. more than one lets a second
/// joiner start syncing without invalidating the first joiner's in-flight
/// capture when the boundary advances between their manifest fetches.
pub const MAX_CAPTURES: usize = 4;

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
    #[error("app-hash mismatch after rebuild: manifest {expected}, composed {actual}")]
    AppHashMismatch { expected: String, actual: String },
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

/// one module's row in a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub module_id: ModuleId,
    pub root: StateRoot,
    pub kind: PayloadKind,
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
    /// the scheme-encoded finalization certificate for exactly `height`,
    /// when the serving node holds one (`None` right after a cutover, when
    /// the epoch has not finalized past its base — the joiner then spawns on
    /// the epoch's genesis floor instead).
    pub floor_cert: Option<Vec<u8>>,
    pub entries: Vec<ManifestEntry>,
}

impl Manifest {
    pub fn entry(&self, id: &str) -> Option<&ManifestEntry> {
        self.entries.iter().find(|e| e.module_id == id)
    }
}

/// a state-sync request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncRequest {
    /// capture (or reuse) the latest finalized boundary and describe it.
    Manifest,
    /// fetch a chunk of a captured module's snapshot payload.
    Chunk {
        height: u64,
        module_id: ModuleId,
        offset: u64,
    },
    /// route module-defined bytes to the live module's `serve_sync`.
    Module { module_id: ModuleId, body: Vec<u8> },
}

/// a state-sync response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncResponse {
    Manifest(Manifest),
    Chunk { total: u64, bytes: Vec<u8> },
    Module(Vec<u8>),
    Error(String),
}

impl SyncResponse {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Manifest(_) => "Manifest",
            Self::Chunk { .. } => "Chunk",
            Self::Module(_) => "Module",
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
            height,
            module_id,
            offset,
        } => {
            out.push(1u8);
            out.extend_from_slice(&height.to_le_bytes());
            wire::put_str(&mut out, module_id);
            out.extend_from_slice(&offset.to_le_bytes());
        }
        SyncRequest::Module { module_id, body } => {
            out.push(2u8);
            wire::put_str(&mut out, module_id);
            wire::put_bytes(&mut out, body);
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
            height: wire::take_u64(&mut buf)?,
            module_id: wire::take_str(&mut buf)?,
            offset: wire::take_u64(&mut buf)?,
        },
        2 => SyncRequest::Module {
            module_id: wire::take_str(&mut buf)?,
            body: wire::take_bytes(&mut buf)?.to_vec(),
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
            out.extend_from_slice(&(m.entries.len() as u64).to_le_bytes());
            for e in &m.entries {
                wire::put_str(&mut out, &e.module_id);
                out.extend_from_slice(e.root.as_bytes());
                out.push(e.kind.to_u8());
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
        SyncResponse::Error(msg) => {
            out.push(3u8);
            wire::put_str(&mut out, msg);
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
                entries.push(ManifestEntry {
                    module_id: wire::take_str(&mut buf)?,
                    root: StateRoot(wire::take_array::<ROOT_LEN>(&mut buf)?),
                    kind: PayloadKind::from_u8(wire::take_u8(&mut buf)?)?,
                });
            }
            SyncResponse::Manifest(Manifest {
                height,
                app_hash,
                epoch,
                view_base,
                participants,
                floor_cert,
                entries,
            })
        }
        1 => SyncResponse::Chunk {
            total: wire::take_u64(&mut buf)?,
            bytes: wire::take_bytes(&mut buf)?.to_vec(),
        },
        2 => SyncResponse::Module(wire::take_bytes(&mut buf)?.to_vec()),
        3 => SyncResponse::Error(wire::take_str(&mut buf)?),
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
    /// the module serves its own resolver lane live; the capture only records
    /// that fact (and the boundary root, in the entry).
    Resolver,
    Unsupported,
}

impl CapturedPayload {
    fn kind(&self) -> PayloadKind {
        match self {
            Self::Stateless => PayloadKind::Stateless,
            Self::Snapshot(_) => PayloadKind::Snapshot,
            Self::Resolver => PayloadKind::Resolver,
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
#[derive(Debug, Clone, Default)]
pub struct BoundaryCoords {
    pub epoch: u64,
    pub view_base: u64,
    pub participants: Vec<Vec<u8>>,
    pub floor_cert: Option<Vec<u8>>,
}

/// a consistent boundary capture: every payload from ONE finalized boundary.
#[derive(Debug, Clone)]
struct Capture {
    app_hash: StateRoot,
    coords: BoundaryCoords,
    modules: BTreeMap<ModuleId, CapturedModule>,
}

/// the server side of the protocol: capture consistent boundary views on
/// demand, cache a few, and answer manifest/chunk requests from them; route
/// module-lane requests to the live host. hold one per node; drive it from the
/// same task that owns the host (answers between drains are automatically
/// consistent — no locks, no torn reads).
#[derive(Default)]
pub struct SyncServer {
    captures: BTreeMap<u64, Capture>,
}

impl SyncServer {
    pub fn new() -> Self {
        Self::default()
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

    async fn try_handle(
        &mut self,
        host: &Host,
        finalized: Option<FinalizedBlock>,
        coords: &BoundaryCoords,
        req: SyncRequest,
    ) -> Result<SyncResponse, String> {
        match req {
            SyncRequest::Manifest => {
                let finalized = finalized.ok_or("no finalized boundary to serve yet")?;
                self.ensure_capture(host, finalized, coords).await?;
                let capture = self
                    .captures
                    .get(&finalized.height)
                    .expect("ensure_capture inserted this height");
                Ok(SyncResponse::Manifest(Manifest {
                    height: finalized.height,
                    app_hash: capture.app_hash,
                    epoch: capture.coords.epoch,
                    view_base: capture.coords.view_base,
                    participants: capture.coords.participants.clone(),
                    floor_cert: capture.coords.floor_cert.clone(),
                    entries: capture
                        .modules
                        .iter()
                        .map(|(id, m)| ManifestEntry {
                            module_id: id.clone(),
                            root: m.root,
                            kind: m.payload.kind(),
                        })
                        .collect(),
                }))
            }
            SyncRequest::Chunk {
                height,
                module_id,
                offset,
            } => {
                let capture = self
                    .captures
                    .get(&height)
                    .ok_or_else(|| format!("no capture at height {height} (refetch manifest)"))?;
                let module = capture
                    .modules
                    .get(&module_id)
                    .ok_or_else(|| format!("no module {module_id} in capture {height}"))?;
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
                Ok(SyncResponse::Chunk {
                    total,
                    bytes: bytes[start..end].to_vec(),
                })
            }
            SyncRequest::Module { module_id, body } => host
                .serve_sync(&module_id, &body)
                .await
                .map(SyncResponse::Module)
                .map_err(|e| format!("module {module_id} serve_sync: {e}")),
        }
    }

    /// capture the registry at `finalized` if not already cached; evict the
    /// oldest capture past [`MAX_CAPTURES`].
    async fn ensure_capture(
        &mut self,
        host: &Host,
        finalized: FinalizedBlock,
        coords: &BoundaryCoords,
    ) -> Result<(), String> {
        if self.captures.contains_key(&finalized.height) {
            return Ok(());
        }
        let snapshot = host
            .capture_finalized_snapshot(finalized)
            .map_err(|e| format!("capture failed: {e}"))?;

        let mut modules = BTreeMap::new();
        for m in snapshot.modules {
            let payload = match m.state_sync {
                StateSyncHandle::Stateless => CapturedPayload::Stateless,
                StateSyncHandle::SnapshotBytes(bytes) => CapturedPayload::Snapshot(bytes),
                StateSyncHandle::ResolverBacked { .. } => CapturedPayload::Resolver,
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
        self.captures.insert(
            finalized.height,
            Capture {
                app_hash: snapshot.app_hash,
                coords: coords.clone(),
                modules,
            },
        );
        while self.captures.len() > MAX_CAPTURES {
            let oldest = *self
                .captures
                .keys()
                .next()
                .expect("len > MAX_CAPTURES implies non-empty");
            self.captures.remove(&oldest);
        }
        Ok(())
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

/// fetch a captured module's full snapshot payload, chunk by chunk.
pub async fn fetch_snapshot<C: SyncClient>(
    client: &C,
    height: u64,
    module_id: &str,
) -> Result<Vec<u8>, SyncError> {
    let mut out: Vec<u8> = Vec::new();
    loop {
        let resp = client
            .request(SyncRequest::Chunk {
                height,
                module_id: module_id.to_string(),
                offset: out.len() as u64,
            })
            .await?;
        match resp {
            SyncResponse::Chunk { total, bytes } => {
                if bytes.is_empty() && out.len() < total as usize {
                    return Err(SyncError::Module {
                        module: module_id.to_string(),
                        reason: "server returned an empty chunk mid-payload".into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_frames_round_trip() {
        for req in [
            SyncRequest::Manifest,
            SyncRequest::Chunk {
                height: 42,
                module_id: "forge".into(),
                offset: 1 << 20,
            },
            SyncRequest::Module {
                module_id: "kv".into(),
                body: vec![1, 2, 3],
            },
        ] {
            let bytes = encode_request(&req);
            assert_eq!(decode_request(&bytes).unwrap(), req);
        }
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
                floor_cert: Some(vec![0xCC; 96]),
                entries: vec![
                    ManifestEntry {
                        module_id: "kv".into(),
                        root: StateRoot([1u8; ROOT_LEN]),
                        kind: PayloadKind::Resolver,
                    },
                    ManifestEntry {
                        module_id: "valset".into(),
                        root: StateRoot([2u8; ROOT_LEN]),
                        kind: PayloadKind::Snapshot,
                    },
                ],
            }),
            // a fresh-epoch boundary: no finalization past the base yet, so
            // no floor certificate — the joiner spawns on the genesis floor.
            SyncResponse::Manifest(Manifest {
                height: 12,
                app_hash: StateRoot([8u8; ROOT_LEN]),
                epoch: 1,
                view_base: 12,
                participants: vec![vec![3u8; 32]],
                floor_cert: None,
                entries: vec![],
            }),
            SyncResponse::Chunk {
                total: 10,
                bytes: vec![0xAB; 10],
            },
            SyncResponse::Module(vec![4, 5]),
            SyncResponse::Error("nope".into()),
        ] {
            let bytes = encode_response(&resp);
            assert_eq!(decode_response(&bytes).unwrap(), resp);
        }
    }

    #[test]
    fn truncated_and_trailing_frames_reject() {
        let bytes = encode_request(&SyncRequest::Chunk {
            height: 1,
            module_id: "m".into(),
            offset: 0,
        });
        assert!(decode_request(&bytes[..bytes.len() - 1]).is_err(), "truncation rejects");
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

        // same header, zero participants + no floor cert, then a forged
        // ENTRY count.
        let mut bytes = vec![0u8];
        bytes.extend_from_slice(&1u64.to_le_bytes());
        bytes.extend_from_slice(&[0u8; ROOT_LEN]);
        bytes.extend_from_slice(&2u64.to_le_bytes());
        bytes.extend_from_slice(&3u64.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());
        assert!(decode_response(&bytes).is_err());
    }
}
