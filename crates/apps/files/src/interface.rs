//! the files module's public wire surface -- types only.
//!
//! writes go via [`FilesMsg`]; reads via [`FilesQuery`] -> [`FilesReply`]; the
//! off-consensus chunk transfer speaks [`FilesSyncReq`] -> [`FilesSyncResp`].
//!
//! a [`Manifest`] is the CONSENSUS truth about a file: its identity, size, and
//! the ordered list of chunk digests. the chunk BYTES are never on the wire
//! here and never in consensus state — they live in a node-local blob store and
//! move peer-to-peer, verified by the receiver against these committed digests.
//! authorship (`owner`) is derived from the dispatch origin, never a payload
//! field, so a submitter cannot claim someone else's ownership.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// a sha256 digest rendered as a 64-character lowercase hex string. this is how
/// a `[u8; 32]` digest crosses the JSON wire.
pub type DigestHex = String;

// ---- write-time caps (consensus constants) ---------------------------------
// the module enforces every one of these at execute time and REJECTS on breach,
// so oversized bytes never enter the `root()` preimage. shared here so clients
// can pre-validate before submitting.

/// `file_id` byte-length bound.
pub const MAX_FILE_ID_BYTES: usize = 256;
/// `name` byte-length bound.
pub const MAX_NAME_BYTES: usize = 512;
/// `mime` byte-length bound.
pub const MAX_MIME_BYTES: usize = 128;
/// smallest permitted chunk size (4 KiB).
pub const MIN_CHUNK_SIZE: u64 = 4 * 1024;
/// largest permitted chunk size (4 MiB).
pub const MAX_CHUNK_SIZE: u64 = 4 * 1024 * 1024;
/// chunks per manifest; a longer chunk list is rejected.
pub const MAX_CHUNKS: usize = 4096;
/// manifests per module; a further add is rejected.
pub const MAX_MANIFESTS: usize = 65_536;
/// list-query page bound; larger limits are clamped down to this.
pub const MAX_LIST_LIMIT: u64 = 256;

/// a content-addressed file manifest — the consensus commitment to one file.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub file_id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub chunk_size: u64,
    /// per-chunk sha256 digests, in file order.
    pub chunks: Vec<DigestHex>,
    /// a whole-file commitment: sha256 over the concatenation of the chunk
    /// digest RAW bytes in order. this is a digest-of-digests — NOT a hash of
    /// the raw file bytes — computed by the module, never caller-supplied.
    ///
    /// digest equality alone is NOT a sufficient receiver-side check: a
    /// manifest may commit `sha256("")` for a chunk while claiming a non-zero
    /// size, and a dishonest server can then serve zero bytes that hash
    /// correctly. every fetched chunk must go through [`verify_chunk`], which
    /// also pins the exact byte length the manifest implies.
    pub digest: DigestHex,
    /// derived from the dispatch origin: a module id verbatim, `"ext:"` +
    /// lowercase hex of the external submitter bytes, or `"system"`; never
    /// taken from a payload field. the `ext:` prefix domain-separates external
    /// identities so a hex-looking module id can never collide with one.
    pub owner: String,
    pub created_at_height: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FilesMsg {
    /// register a manifest. the module computes `digest` and `owner`; the
    /// submitter supplies only the identity, shape, and chunk list.
    AddManifest {
        file_id: String,
        name: String,
        mime: String,
        size: u64,
        chunk_size: u64,
        chunks: Vec<DigestHex>,
    },
    /// remove a manifest. owner-gated: only the stored owner origin may remove.
    RemoveManifest { file_id: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FilesQuery {
    Stat { file_id: String },
    List { prefix: String, limit: u64 },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FilesReply {
    Stat(Option<Manifest>),
    List(Vec<Manifest>),
}

/// the off-consensus chunk-serve request. the module answers this from its
/// node-local blob store, outside any block.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FilesSyncReq {
    GetChunk { digest: DigestHex },
}

/// the off-consensus chunk-serve response. `present == false` carries empty
/// `bytes`. the caller MUST re-hash `bytes` and check it against the digest
/// committed in a manifest before trusting them — a dishonest server can flip a
/// byte, but the receiver detects it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FilesSyncResp {
    Chunk { present: bool, bytes: Vec<u8> },
}

/// receiver-side verification of one fetched chunk against a committed
/// manifest. every fetcher MUST run this before trusting served bytes.
///
/// checks BOTH properties the manifest commits:
/// - `sha256(bytes)` equals `chunks[index]`, and
/// - `bytes.len()` equals the exact length the manifest implies for that
///   position — `chunk_size` for every chunk except the last, and
///   `size - (n - 1) * chunk_size` for the last.
///
/// the length check is load-bearing, not belt-and-braces: a manifest with
/// `size = 1` and `chunks = [sha256("")]` validates at execute, and a
/// dishonest server can serve zero bytes whose digest matches — digest
/// equality alone would accept a chunk that reconstructs the wrong file.
pub fn verify_chunk(manifest: &Manifest, index: usize, bytes: &[u8]) -> Result<(), String> {
    let n = manifest.chunks.len();
    if index >= n {
        return Err(format!("chunk index {index} out of range ({n} chunks)"));
    }
    let expected_len = if index == n - 1 {
        (n as u64 - 1)
            .checked_mul(manifest.chunk_size)
            .and_then(|prev| manifest.size.checked_sub(prev))
            .ok_or_else(|| "manifest size/chunk_size are inconsistent".to_string())?
    } else {
        manifest.chunk_size
    };
    if bytes.len() as u64 != expected_len {
        return Err(format!(
            "chunk {index} length mismatch: manifest implies {expected_len} bytes, got {}",
            bytes.len()
        ));
    }
    let got = digest_hex(bytes);
    if got != manifest.chunks[index] {
        return Err(format!(
            "chunk {index} digest mismatch: committed {}, got {got}",
            manifest.chunks[index]
        ));
    }
    Ok(())
}

/// sha256 of `bytes` as a 64-char lowercase hex string — the [`DigestHex`]
/// rendering used everywhere on this wire.
pub fn digest_hex(bytes: &[u8]) -> DigestHex {
    let mut h = Sha256::new();
    h.update(bytes);
    let raw: [u8; 32] = h.finalize().into();
    let mut s = String::with_capacity(64);
    for b in raw {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

pub fn encode_msg(m: &FilesMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<FilesMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_query(q: &FilesQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}

pub fn decode_query(b: &[u8]) -> Result<FilesQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_reply(r: &FilesReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_reply(b: &[u8]) -> Result<FilesReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_sync_req(r: &FilesSyncReq) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_sync_req(b: &[u8]) -> Result<FilesSyncReq, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_sync_resp(r: &FilesSyncResp) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_sync_resp(b: &[u8]) -> Result<FilesSyncResp, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
