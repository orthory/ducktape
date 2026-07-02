//! content-addressed file manifests as consensus state, with body bytes kept
//! OFF consensus (the NoKV metadata/body split).
//!
//! ## the split (the part that must never blur)
//!
//! two planes:
//!
//! - the **metadata control plane** is consensus state: a `BTreeMap` of
//!   [`Manifest`]s keyed by `file_id`. a manifest names the file and pins the
//!   ordered list of per-chunk sha256 digests plus a whole-file digest-of-
//!   digests. this is the ONLY files state folded into `root()`, and the only
//!   thing state sync transfers.
//! - the **body store** is a node-local, in-memory [`BlobStore`] of chunk bytes
//!   keyed by their own sha256. it is NOT part of `root()`, NOT consensus state,
//!   and NOT part of state sync. possession of bytes is per-node; the manifest
//!   is the shared truth about what those bytes must hash to.
//!
//! chunk bytes never enter the op stream — no upload op goes through consensus.
//! the daemon/RPC layer feeds uploaded bytes into [`Files::put_chunk`] and reads
//! them back with [`Files::get_chunk`]; only the resulting digests reach an
//! `AddManifest`. because a receiver re-hashes every fetched chunk against the
//! digest a manifest committed, a dishonest server can never install bad bytes.
//!
//! ## staging
//!
//! writes STAGE into a pending overlay during `execute`, publish at
//! `commit_block`, and discard at `abort_block`; `root()` is sha256 over the
//! canonical encoding of COMMITTED manifests only. `snapshot`/`install` ship
//! exactly that preimage and verify against the expected root before adopting.
//! every size cap is enforced at execute time with rejection, so an oversized
//! value never enters the root preimage.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest as _, Sha256};

use files_interface::{
    FilesMsg, FilesQuery, FilesReply, FilesSyncReq, FilesSyncResp, MAX_CHUNK_SIZE, MAX_CHUNKS,
    MAX_FILE_ID_BYTES, MAX_LIST_LIMIT, MAX_MANIFESTS, MAX_MIME_BYTES, MAX_NAME_BYTES,
    MIN_CHUNK_SIZE, Manifest, decode_msg, decode_query, decode_sync_req, encode_reply,
    encode_sync_resp,
};

/// node-local chunk bytes, keyed by their sha256 digest. explicitly NOT part of
/// `root()` or state sync — durability of bytes is a per-node concern.
#[derive(Default)]
pub struct BlobStore {
    chunks: HashMap<[u8; 32], Vec<u8>>,
}

impl BlobStore {
    pub fn put_chunk(&mut self, bytes: Vec<u8>) -> [u8; 32] {
        let digest = sha256(&bytes);
        self.chunks.insert(digest, bytes);
        digest
    }

    pub fn get_chunk(&self, digest: &[u8; 32]) -> Option<&[u8]> {
        self.chunks.get(digest).map(Vec::as_slice)
    }

    pub fn has_chunk(&self, digest: &[u8; 32]) -> bool {
        self.chunks.contains_key(digest)
    }
}

pub struct Files {
    id: ModuleId,
    manifests: BTreeMap<String, Manifest>,
    /// per-block overlay: `Some(m)` stages an upsert, `None` stages a delete.
    pending: BTreeMap<String, Option<Manifest>>,
    blobs: BlobStore,
}

impl Files {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            manifests: BTreeMap::new(),
            pending: BTreeMap::new(),
            blobs: BlobStore::default(),
        }
    }

    // ---- node-local blob store seam (never touches consensus state) --------

    /// store one chunk and return its sha256 digest. called by the daemon/RPC
    /// layer as bytes are uploaded — bytes never enter the op stream.
    pub fn put_chunk(&mut self, bytes: Vec<u8>) -> [u8; 32] {
        self.blobs.put_chunk(bytes)
    }

    /// read one chunk's bytes back out of the local blob store.
    pub fn get_chunk(&self, digest: &[u8; 32]) -> Option<&[u8]> {
        self.blobs.get_chunk(digest)
    }

    /// whether this node currently holds a chunk's bytes.
    pub fn has_chunk(&self, digest: &[u8; 32]) -> bool {
        self.blobs.has_chunk(digest)
    }

    // ---- consensus state (manifests) ---------------------------------------

    fn get(&self, file_id: &str) -> Option<&Manifest> {
        match self.pending.get(file_id) {
            Some(staged) => staged.as_ref(),
            None => self.manifests.get(file_id),
        }
    }

    /// distinct `file_id` count after applying the pending overlay — the
    /// projection `MAX_MANIFESTS` is checked against. counted incrementally
    /// off the committed length so a full-capacity module stays O(pending).
    fn effective_len(&self) -> usize {
        let mut len = self.manifests.len();
        for (id, staged) in &self.pending {
            match (staged.is_some(), self.manifests.contains_key(id)) {
                (true, false) => len += 1,
                (false, true) => len -= 1,
                _ => {}
            }
        }
        len
    }

    /// derive the owner string from the dispatch origin: a module id verbatim,
    /// `"ext:"` + lowercase hex for an external submitter (the prefix
    /// domain-separates external identities from hex-looking module ids), or
    /// `"system"`. never taken from the payload.
    fn owner_of(origin: &Origin) -> String {
        match origin {
            Origin::Module(id) => id.clone(),
            Origin::External(bytes) => format!("ext:{}", to_hex(bytes)),
            Origin::System => "system".to_string(),
        }
    }

    fn stage_add(
        &mut self,
        origin: &Origin,
        height: u64,
        file_id: String,
        name: String,
        mime: String,
        size: u64,
        chunk_size: u64,
        chunks: Vec<String>,
    ) -> Result<(), Error> {
        require(!file_id.is_empty(), "file_id must not be empty")?;
        require(!name.is_empty(), "name must not be empty")?;
        require(
            file_id.len() <= MAX_FILE_ID_BYTES,
            "file_id exceeds size cap",
        )?;
        require(name.len() <= MAX_NAME_BYTES, "name exceeds size cap")?;
        require(mime.len() <= MAX_MIME_BYTES, "mime exceeds size cap")?;
        require(
            (MIN_CHUNK_SIZE..=MAX_CHUNK_SIZE).contains(&chunk_size),
            "chunk_size out of range",
        )?;
        require(!chunks.is_empty(), "chunks must not be empty")?;
        require(chunks.len() <= MAX_CHUNKS, "too many chunks")?;

        let n = chunks.len() as u64;
        let expected_chunks = size.div_ceil(chunk_size);
        require(expected_chunks == n, "chunk count does not match size")?;
        let span = n
            .checked_mul(chunk_size)
            .ok_or_else(|| Error::Module("files: chunk span overflow".into()))?;
        let prev_span = (n - 1)
            .checked_mul(chunk_size)
            .ok_or_else(|| Error::Module("files: chunk span overflow".into()))?;
        require(size <= span, "size exceeds chunk span")?;
        require(size > prev_span, "size underfills the last chunk")?;

        let mut raw = Vec::with_capacity(chunks.len() * 32);
        for chunk in &chunks {
            let digest = from_hex_32(chunk).ok_or_else(|| {
                Error::Module("files: chunk digest not 64-char lowercase hex".into())
            })?;
            raw.extend_from_slice(&digest);
        }

        require(self.get(&file_id).is_none(), "file_id already exists")?;
        require(
            self.effective_len() < MAX_MANIFESTS,
            "manifest limit reached",
        )?;

        let digest = to_hex(&sha256(&raw));
        let manifest = Manifest {
            file_id: file_id.clone(),
            name,
            mime,
            size,
            chunk_size,
            chunks,
            digest,
            owner: Self::owner_of(origin),
            created_at_height: height,
        };
        self.pending.insert(file_id, Some(manifest));
        Ok(())
    }

    fn stage_remove(&mut self, origin: &Origin, file_id: String) -> Result<(), Error> {
        require(!file_id.is_empty(), "file_id must not be empty")?;
        let manifest = self
            .get(&file_id)
            .ok_or_else(|| Error::Module(format!("files: manifest not found: {file_id}")))?;
        let who = Self::owner_of(origin);
        require(
            manifest.owner == who,
            "only the stored owner may remove this manifest",
        )?;
        self.pending.insert(file_id, None);
        Ok(())
    }

    fn stat(&self, file_id: &str) -> Option<Manifest> {
        self.manifests.get(file_id).cloned()
    }

    fn list(&self, prefix: &str, limit: u64) -> Vec<Manifest> {
        let limit = limit.min(MAX_LIST_LIMIT) as usize;
        self.manifests
            .values()
            .filter(|m| m.file_id.starts_with(prefix))
            .take(limit)
            .cloned()
            .collect()
    }

    // ---- root / snapshot / install -----------------------------------------

    fn root_of(manifests: &BTreeMap<String, Manifest>) -> StateRoot {
        StateRoot(sha256(&encode_manifests(manifests)))
    }

    pub fn snapshot(&self) -> Vec<u8> {
        encode_manifests(&self.manifests)
    }

    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let manifests = decode_manifests(bytes)?;
        if Self::root_of(&manifests) != expected {
            return Err(Error::Module("files: snapshot root mismatch".into()));
        }
        self.manifests = manifests;
        self.pending.clear();
        Ok(())
    }
}

fn require(ok: bool, why: &str) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::Module(format!("files: {why}")))
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// decode exactly 64 lowercase-hex chars into 32 bytes. rejects any other
/// length and any non-`[0-9a-f]` byte (uppercase included) — so this doubles as
/// the "valid 64-char lowercase hex" digest check.
fn from_hex_32(s: &str) -> Option<[u8; 32]> {
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

fn encode_manifests(manifests: &BTreeMap<String, Manifest>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(manifests.len() as u64).to_le_bytes());
    for m in manifests.values() {
        push_string(&mut out, &m.file_id);
        push_string(&mut out, &m.name);
        push_string(&mut out, &m.mime);
        out.extend_from_slice(&m.size.to_le_bytes());
        out.extend_from_slice(&m.chunk_size.to_le_bytes());
        out.extend_from_slice(&(m.chunks.len() as u64).to_le_bytes());
        for chunk in &m.chunks {
            push_string(&mut out, chunk);
        }
        push_string(&mut out, &m.digest);
        push_string(&mut out, &m.owner);
        out.extend_from_slice(&m.created_at_height.to_le_bytes());
    }
    out
}

fn decode_manifests(bytes: &[u8]) -> Result<BTreeMap<String, Manifest>, Error> {
    let mut off = 0usize;
    let count = read_u64(bytes, &mut off)?;
    if count > MAX_MANIFESTS as u64 {
        return Err(Error::Module(
            "files: snapshot manifest count too large".into(),
        ));
    }

    let mut manifests: BTreeMap<String, Manifest> = BTreeMap::new();
    for _ in 0..count {
        let file_id = read_string(bytes, &mut off)?;
        let name = read_string(bytes, &mut off)?;
        let mime = read_string(bytes, &mut off)?;
        let size = read_u64(bytes, &mut off)?;
        let chunk_size = read_u64(bytes, &mut off)?;
        let chunk_count = read_u64(bytes, &mut off)?;
        if chunk_count > MAX_CHUNKS as u64 {
            return Err(Error::Module(
                "files: snapshot chunk count too large".into(),
            ));
        }
        let mut chunks = Vec::with_capacity(chunk_count as usize);
        for _ in 0..chunk_count {
            chunks.push(read_string(bytes, &mut off)?);
        }
        let digest = read_string(bytes, &mut off)?;
        let owner = read_string(bytes, &mut off)?;
        let created_at_height = read_u64(bytes, &mut off)?;

        if manifests
            .last_key_value()
            .is_some_and(|(last, _)| last.as_str() >= file_id.as_str())
        {
            return Err(Error::Module(
                "files: snapshot file ids not strictly ascending".into(),
            ));
        }

        manifests.insert(
            file_id.clone(),
            Manifest {
                file_id,
                name,
                mime,
                size,
                chunk_size,
                chunks,
                digest,
                owner,
                created_at_height,
            },
        );
    }
    if off != bytes.len() {
        return Err(Error::Module("files: snapshot has trailing bytes".into()));
    }
    Ok(manifests)
}

fn push_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn read_u64(bytes: &[u8], off: &mut usize) -> Result<u64, Error> {
    let end = off
        .checked_add(8)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Module("files: snapshot truncated".into()))?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[*off..end]);
    *off = end;
    Ok(u64::from_le_bytes(buf))
}

fn read_string(bytes: &[u8], off: &mut usize) -> Result<String, Error> {
    let len = read_u64(bytes, off)?;
    let len =
        usize::try_from(len).map_err(|_| Error::Module("files: snapshot truncated".into()))?;
    if len > bytes.len() - *off {
        return Err(Error::Module("files: snapshot truncated".into()));
    }
    let value = std::str::from_utf8(&bytes[*off..*off + len])
        .map_err(|_| Error::Module("files: snapshot string is not utf-8".into()))?;
    *off += len;
    Ok(value.to_owned())
}

#[async_trait::async_trait(?Send)]
impl Module for Files {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.manifests)
    }

    /// advertise the snapshot lane over MANIFESTS ONLY. blob-store bytes are
    /// explicitly not part of state sync — a joiner rebuilds the namespace truth
    /// here and fetches any bytes it needs separately, verifying each against
    /// the committed digest.
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    /// serve one chunk out of the node-local blob store. legal under the
    /// serve_sync contract because the CALLER verifies received bytes against
    /// the digest committed in the manifest — a dishonest response can never
    /// install. answered from the local blob store, outside any block.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_sync_req(req).map_err(Error::Module)? {
            FilesSyncReq::GetChunk { digest } => {
                let resp = match from_hex_32(&digest).and_then(|d| self.get_chunk(&d)) {
                    Some(bytes) => FilesSyncResp::Chunk {
                        present: true,
                        bytes: bytes.to_vec(),
                    },
                    None => FilesSyncResp::Chunk {
                        present: false,
                        bytes: Vec::new(),
                    },
                };
                Ok(encode_sync_resp(&resp))
            }
        }
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let env = ctx.env().clone();
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            FilesMsg::AddManifest {
                file_id,
                name,
                mime,
                size,
                chunk_size,
                chunks,
            } => self.stage_add(
                &env.origin,
                env.height,
                file_id,
                name,
                mime,
                size,
                chunk_size,
                chunks,
            ),
            FilesMsg::RemoveManifest { file_id } => self.stage_remove(&env.origin, file_id),
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            FilesQuery::Stat { file_id } => {
                Ok(encode_reply(&FilesReply::Stat(self.stat(&file_id))))
            }
            FilesQuery::List { prefix, limit } => {
                Ok(encode_reply(&FilesReply::List(self.list(&prefix, limit))))
            }
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, staged) in std::mem::take(&mut self.pending) {
            match staged {
                Some(manifest) => {
                    self.manifests.insert(id, manifest);
                }
                None => {
                    self.manifests.remove(&id);
                }
            }
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}
