//! the duckfs wire surface — types only. writes go via [`FilesMsg`] (json)
//! or the binary putblob frame; reads via [`FilesQuery`] -> [`FilesReply`];
//! the off-block object fetch speaks [`FilesSyncReq`] -> [`FilesSyncResp`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// a sha256-derived object id rendered as 64-char lowercase hex on the wire.
pub type DigestHex = String;

// ---- network constants (consensus; execute-time rejection on breach) ----
pub const CHUNK_SIZE: u64 = 1024 * 1024;
pub const MAX_NAME_BYTES: usize = 255;
pub const MAX_PATH_BYTES: usize = 4096;
pub const MAX_DEPTH: usize = 128;
pub const MAX_DIR_ENTRIES: usize = 65_536;
pub const MAX_INLINE_COMMIT_BYTES: usize = 256 * 1024;
pub const MAX_CHANGES_PER_COMMIT: usize = 4096;
pub const MAX_MESSAGE_BYTES: usize = 4096;
pub const MAX_META_ENTRIES: usize = 16;
pub const MAX_META_KEY_BYTES: usize = 64;
pub const MAX_META_VALUE_BYTES: usize = 256;
pub const MAX_CHUNKS_PER_FILE: usize = 4_194_304;
pub const MAX_SYMLINK_TARGET_BYTES: usize = 4096;
pub const STAGING_QUOTA_BYTES: u64 = 1024 * 1024 * 1024;
pub const STAGING_TTL_BLOCKS: u64 = 4096;
pub const MAX_PINS: usize = 1024;
/// the global staging-table entry ceiling — [`MAX_PINS`] × 64 = 65_536. it is a
/// consensus constant shared by BOTH the canonical decode and the execute path:
/// [`decode_refs`](crate::state::decode_refs) rejects any refs image whose
/// staging section exceeds it, and `putblob` rejects a stage that would grow the
/// table to it. keeping ONE definition across the two sides is load-bearing —
/// because putblob refuses to stage past this bound, EVERY `Refs` an execute path
/// can produce stays within the decode ceiling, so the agreed image always
/// re-decodes on reboot and installs on a joiner. the per-owner byte quota does
/// NOT bound the entry count (distinct tiny chunks cost almost no quota), so
/// without this cap one owner could grow the table past the decode limit, commit
/// it as agreed consensus state, then brick the whole cluster the next time each
/// node loads its refs file (`decode_refs` would reject the honest, agreed
/// image everywhere at once).
pub const MAX_STAGING_ENTRIES: usize = MAX_PINS * 64;
/// one owner's share of the [`MAX_STAGING_ENTRIES`] staging table — the count
/// analogue of [`STAGING_QUOTA_BYTES`], bounding how many outstanding entries a
/// single owner may hold so no owner can monopolize the shared table. 4096 ×
/// [`CHUNK_SIZE`] (1 MiB) = 4 GiB, four times the 1 GiB byte quota, so an honest
/// large upload always trips the byte quota first and is never limited by this
/// cap; it bites only a hostile flood of tiny (sub-quota) chunks.
pub const MAX_STAGING_ENTRIES_PER_OWNER: usize = 4096;
pub const MAX_PIN_NAME_BYTES: usize = 128;
pub const MAX_WATCHES: usize = 256;
pub const MAX_WATCH_MODULE_ID_BYTES: usize = 128;
pub const HISTORY_WINDOW: usize = 1024;
pub const GC_PERIOD_BLOCKS: u64 = 1024;
pub const MAX_PAGE: u64 = 256;
/// the `GetObjects` request id cap — a sync fetch beyond this rejects ("too many
/// ids"). one batch is a page of ids, so it mirrors [`MAX_PAGE`]; the fetch loop
/// pages through larger missing sets a batch at a time.
pub const MAX_SYNC_IDS: usize = MAX_PAGE as usize;
pub const MAX_READ_BYTES: u64 = 1024 * 1024;
pub const MAX_GREP_SCAN_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_GREP_LINE_BYTES: usize = 256;
/// hard ceiling on hits emitted by ONE grep call (MAX_PAGE * 16 — the same
/// bounded-reply convention as the diff cap). the scan budget bounds bytes
/// SCANNED, not hits EMITTED, so without this a single in-budget file of
/// pathologically many matching lines could amplify into an unbounded reply;
/// with it a reply is bounded at roughly 4096 hits x ~0.5 KiB each (path +
/// capped line text + uri) — a couple of MiB worst case.
pub const MAX_GREP_HITS_PER_CALL: usize = MAX_PAGE as usize * 16;
/// first byte of the binary putblob op frame. json msgs start with b'{',
/// so one leading byte disambiguates the whole op space.
pub const PUTBLOB_FRAME_TAG: u8 = 0x00;

// ---- write wire ----

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilesMsg {
    /// atomic multi-path commit. `base_snapshot: None` means the empty tree
    /// (first commit). per-path CAS: every changed path must be identical
    /// between base and the live head or the whole commit rejects.
    Commit {
        base_snapshot: Option<DigestHex>,
        message: String,
        changes: Vec<Change>,
    },
    Pin {
        snapshot: DigestHex,
        name: String,
    },
    /// owner-gated: only the pin's creator (or system) may unpin.
    Unpin {
        name: String,
    },
    Watch {
        prefix: String,
        module_id: String,
    },
    /// gated to the module that registered the watch.
    Unwatch {
        prefix: String,
        module_id: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Put {
        path: String,
        exec: bool,
        #[serde(default)]
        meta: BTreeMap<String, String>,
        content: Content,
    },
    Mkdir {
        path: String,
    },
    /// removes the entry at `path` (file, symlink, or whole subtree).
    Rm {
        path: String,
    },
    Mv {
        from: String,
        to: String,
    },
    Symlink {
        path: String,
        target: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Content {
    /// small files ride inside the commit op; the module chunks + hashes.
    Inline { b64: String },
    /// large files reference chunks staged via putblob (or already present).
    Chunks { size: u64, chunks: Vec<DigestHex> },
}

// ---- read wire ----

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilesQuery {
    Stat {
        path: String,
        snapshot: Option<DigestHex>,
    },
    Ls {
        path: String,
        snapshot: Option<DigestHex>,
        after: Option<String>,
        limit: u64,
    },
    Read {
        path: String,
        snapshot: Option<DigestHex>,
        offset: u64,
        len: u64,
    },
    Find {
        prefix: String,
        snapshot: Option<DigestHex>,
        after: Option<String>,
        limit: u64,
    },
    Grep {
        pattern: String,
        prefix: String,
        snapshot: Option<DigestHex>,
        cursor: Option<String>,
        limit: u64,
    },
    History {
        limit: u64,
    },
    Diff {
        from: DigestHex,
        to: DigestHex,
        prefix: String,
    },
    Refs {},
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKindWire {
    File,
    Dir,
    Symlink,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    pub path: String,
    pub kind: EntryKindWire,
    pub size: u64,
    pub exec: bool,
    pub object: DigestHex,
    #[serde(default)]
    pub meta: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SnapshotInfo {
    pub id: DigestHex,
    pub parent: Option<DigestHex>,
    pub root_tree: DigestHex,
    pub author: String,
    pub height: u64,
    pub consensus_time: u64,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Added,
    Removed,
    Modified,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DiffEntry {
    pub path: String,
    pub kind: DiffKind,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GrepHit {
    pub path: String,
    pub line: u64,
    pub text: String,
    /// `duck://files<path>@<snapshot>#L<line>` — the absolute path brings its
    /// own leading slash (see [`evidence_uri`]).
    pub uri: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RefsInfo {
    pub head: Option<DigestHex>,
    pub pins: BTreeMap<String, DigestHex>,
    pub window_len: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilesReply {
    Stat(Option<EntryInfo>),
    Ls {
        entries: Vec<EntryInfo>,
        next: Option<String>,
    },
    Read {
        b64: String,
        eof: bool,
    },
    Find {
        entries: Vec<EntryInfo>,
        next: Option<String>,
    },
    Grep {
        hits: Vec<GrepHit>,
        next: Option<String>,
    },
    History(Vec<SnapshotInfo>),
    Diff(Vec<DiffEntry>),
    Refs(RefsInfo),
}

// ---- off-block object fetch (state sync / self-heal lane) ----

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilesSyncReq {
    /// batched fetch; response order matches request order.
    GetObjects { ids: Vec<DigestHex> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SyncObject {
    pub id: DigestHex,
    pub present: bool,
    /// object kind tag byte (`objects::Kind as u8`) — receivers re-derive the
    /// id as sha256(tag ‖ body) and reject mismatches.
    pub kind: u8,
    pub b64: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilesSyncResp {
    Objects(Vec<SyncObject>),
}

// ---- codecs (same shape as every module in this repo) ----

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

/// build a putblob op: `[PUTBLOB_FRAME_TAG] ++ raw chunk bytes`.
pub fn encode_putblob(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + bytes.len());
    out.push(PUTBLOB_FRAME_TAG);
    out.extend_from_slice(bytes);
    out
}

/// grep-hit evidence uri: `duck://files<path>@<snapshot>#L<line>`. the path is
/// absolute and brings its own leading slash, so the authority is joined
/// bare — same rule as memory's `duck://memory<path>` uris (a separator slash
/// here would double it).
pub fn evidence_uri(path: &str, snapshot: &str, line: u64) -> String {
    format!("duck://files{path}@{snapshot}#L{line}")
}

/// decode exactly 64 lowercase-hex chars into 32 bytes (uppercase rejected).
pub fn from_hex_32(s: &str) -> Option<[u8; 32]> {
    let bytes = s.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = hex_val(bytes[2 * i])?;
        let lo = hex_val(bytes[2 * i + 1])?;
        *slot = (hi << 4) | lo;
    }
    Some(out)
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

pub fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}
