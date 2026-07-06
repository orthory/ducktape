//! the immutable object model — chunk/file/tree/snapshot records in the
//! content-addressed store. an id is sha256 over a domain-separating kind tag
//! byte followed by the object's canonical body, so one body means exactly one
//! thing under exactly one kind, and the id can never collide across kinds.
//!
//! every decode is strict by design: counts are capped before they are trusted,
//! strings are utf-8, tree names and meta keys strictly ascend, single-byte
//! enums and booleans reject any non-canonical byte, and a decode that does not
//! consume the whole input is rejected. canonical bytes in, canonical bytes out,
//! or an error — there is no lenient path.

use std::collections::BTreeMap;

use sha2::{Digest as _, Sha256};

use crate::{
    CHUNK_SIZE, MAX_CHUNKS_PER_FILE, MAX_DIR_ENTRIES, MAX_MESSAGE_BYTES, MAX_META_ENTRIES,
    MAX_META_KEY_BYTES, MAX_META_VALUE_BYTES, MAX_NAME_BYTES,
};

/// a raw 32-byte object id: sha256 over `tag ‖ body` (64-char lowercase hex on
/// the wire).
pub type ObjectId = [u8; 32];

/// object kind — the domain-separating tag byte in every id preimage and the
/// `kind` field of a sync-fetched object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Kind {
    Chunk = 0,
    File = 1,
    Tree = 2,
    Snapshot = 3,
}

impl Kind {
    /// the id-preimage / wire tag byte.
    pub fn tag(self) -> u8 {
        self as u8
    }

    /// the inverse of [`Kind::tag`] — an unknown tag is rejected, never coerced.
    pub fn from_u8(value: u8) -> Option<Kind> {
        match value {
            0 => Some(Kind::Chunk),
            1 => Some(Kind::File),
            2 => Some(Kind::Tree),
            3 => Some(Kind::Snapshot),
            _ => None,
        }
    }
}

/// derive an object id: sha256 over the kind tag byte followed by the body —
/// the same preimage every receiver re-derives before trusting fetched bytes.
pub fn object_id(kind: Kind, body: &[u8]) -> ObjectId {
    let mut h = Sha256::new();
    h.update([kind.tag()]);
    h.update(body);
    h.finalize().into()
}

/// a file: total byte size, the ordered chunk ids that reconstruct it, and up
/// to [`MAX_META_ENTRIES`] small string metadata pairs. chunking is fixed at
/// [`CHUNK_SIZE`]; [`verify_chunk_len`] pins the length of each chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileObj {
    pub size: u64,
    pub chunks: Vec<ObjectId>,
    pub meta: BTreeMap<String, String>,
}

impl FileObj {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.size.to_le_bytes());
        out.extend_from_slice(&(self.chunks.len() as u32).to_le_bytes());
        for id in &self.chunks {
            out.extend_from_slice(id);
        }
        out.extend_from_slice(&(self.meta.len() as u16).to_le_bytes());
        for (key, value) in &self.meta {
            push_string(&mut out, key);
            push_string(&mut out, value);
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut off = 0usize;
        let size = read_u64(bytes, &mut off)?;

        let chunk_count = read_u32(bytes, &mut off)? as usize;
        if chunk_count > MAX_CHUNKS_PER_FILE {
            return Err("files: file chunk count over cap".into());
        }
        // do not pre-reserve to the declared count: it is untrusted until the
        // ids are actually read, so growth stays amortized instead.
        let mut chunks = Vec::new();
        for _ in 0..chunk_count {
            chunks.push(read_bytes32(bytes, &mut off)?);
        }

        let meta_count = read_u16(bytes, &mut off)? as usize;
        if meta_count > MAX_META_ENTRIES {
            return Err("files: file meta count over cap".into());
        }
        let mut meta: BTreeMap<String, String> = BTreeMap::new();
        for _ in 0..meta_count {
            let key = read_string(bytes, &mut off)?;
            let value = read_string(bytes, &mut off)?;
            if key.len() > MAX_META_KEY_BYTES {
                return Err("files: file meta key over cap".into());
            }
            if value.len() > MAX_META_VALUE_BYTES {
                return Err("files: file meta value over cap".into());
            }
            // strictly ascending keys keep the encoding canonical: no duplicate,
            // no reordered pair can produce a second valid preimage.
            if meta
                .last_key_value()
                .is_some_and(|(last, _)| last.as_str() >= key.as_str())
            {
                return Err("files: file meta keys not strictly ascending".into());
            }
            meta.insert(key, value);
        }

        finish(bytes, off)?;
        Ok(FileObj { size, chunks, meta })
    }
}

/// what a tree entry points at — the tag byte prefixing each entry's id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EntryKind {
    File = 0,
    Dir = 1,
    Symlink = 2,
}

impl EntryKind {
    pub fn tag(self) -> u8 {
        self as u8
    }

    pub fn from_u8(value: u8) -> Option<EntryKind> {
        match value {
            0 => Some(EntryKind::File),
            1 => Some(EntryKind::Dir),
            2 => Some(EntryKind::Symlink),
            _ => None,
        }
    }
}

/// one entry in a directory tree: what it is, the object it names, whether it is
/// executable, and the resolved byte size (0 for a directory).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeEntry {
    pub kind: EntryKind,
    pub id: ObjectId,
    pub exec: bool,
    pub size: u64,
}

/// a directory: a name-keyed set of entries, encoded in strict ascending name
/// order so the same directory always hashes to the same id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeObj {
    pub entries: BTreeMap<String, TreeEntry>,
}

impl TreeObj {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        // BTreeMap iterates in ascending key order, which is exactly the
        // canonical entry order the decoder re-checks.
        for (name, entry) in &self.entries {
            push_string(&mut out, name);
            out.push(entry.kind.tag());
            out.extend_from_slice(&entry.id);
            out.push(u8::from(entry.exec));
            out.extend_from_slice(&entry.size.to_le_bytes());
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut off = 0usize;
        let entry_count = read_u32(bytes, &mut off)? as usize;
        if entry_count > MAX_DIR_ENTRIES {
            return Err("files: tree entry count over cap".into());
        }
        let mut entries: BTreeMap<String, TreeEntry> = BTreeMap::new();
        for _ in 0..entry_count {
            let name = read_string(bytes, &mut off)?;
            if name.len() > MAX_NAME_BYTES {
                return Err("files: tree entry name over cap".into());
            }
            let kind = EntryKind::from_u8(read_u8(bytes, &mut off)?)
                .ok_or_else(|| "files: tree entry has an unknown kind".to_string())?;
            let id = read_bytes32(bytes, &mut off)?;
            let exec = read_bool(bytes, &mut off)?;
            let size = read_u64(bytes, &mut off)?;
            if entries
                .last_key_value()
                .is_some_and(|(last, _)| last.as_str() >= name.as_str())
            {
                return Err("files: tree names not strictly ascending".into());
            }
            entries.insert(
                name,
                TreeEntry {
                    kind,
                    id,
                    exec,
                    size,
                },
            );
        }

        finish(bytes, off)?;
        Ok(TreeObj { entries })
    }
}

/// a snapshot: the committed root tree, the parent it descends from (absent
/// only for the first commit), and the consensus-witnessed authorship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotObj {
    pub root: ObjectId,
    pub parent: Option<ObjectId>,
    pub author: String,
    pub consensus_time: u64,
    pub height: u64,
    pub message: String,
}

impl SnapshotObj {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.root);
        match &self.parent {
            Some(parent) => {
                out.push(1);
                out.extend_from_slice(parent);
            }
            None => out.push(0),
        }
        push_string(&mut out, &self.author);
        out.extend_from_slice(&self.consensus_time.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        push_string(&mut out, &self.message);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut off = 0usize;
        let root = read_bytes32(bytes, &mut off)?;
        // has_parent is a canonical bool: the parent id is present iff it is 1.
        let parent = if read_bool(bytes, &mut off)? {
            Some(read_bytes32(bytes, &mut off)?)
        } else {
            None
        };
        let author = read_string(bytes, &mut off)?;
        let consensus_time = read_u64(bytes, &mut off)?;
        let height = read_u64(bytes, &mut off)?;
        let message = read_string(bytes, &mut off)?;
        if message.len() > MAX_MESSAGE_BYTES {
            return Err("files: snapshot message over cap".into());
        }

        finish(bytes, off)?;
        Ok(SnapshotObj {
            root,
            parent,
            author,
            consensus_time,
            height,
            message,
        })
    }
}

/// the load-bearing exact-length rule: every chunk but the last must be exactly
/// [`CHUNK_SIZE`]; the last carries the remainder `size - (n-1)*CHUNK_SIZE`,
/// which must land in `1..=CHUNK_SIZE`. a size that cannot produce such a
/// remainder is an inconsistent size/chunks pair and rejects — so a zero-length
/// "chunk" can never spoof a hole, and the byte length alone pins each chunk.
pub fn verify_chunk_len(file: &FileObj, index: usize, got_len: u64) -> Result<(), String> {
    let n = file.chunks.len();
    if index >= n {
        return Err("files: chunk index out of range".into());
    }
    let expected = if index + 1 == n {
        let prefix = (n as u64 - 1)
            .checked_mul(CHUNK_SIZE)
            .ok_or_else(|| "files: chunk prefix length overflows".to_string())?;
        let last = file
            .size
            .checked_sub(prefix)
            .ok_or_else(|| "files: file size smaller than its chunk count implies".to_string())?;
        if last == 0 || last > CHUNK_SIZE {
            return Err("files: file size inconsistent with chunk count".into());
        }
        last
    } else {
        CHUNK_SIZE
    };
    if got_len != expected {
        return Err("files: chunk length does not match the size rule".into());
    }
    Ok(())
}

// ---- canonical codec helpers ------------------------------------------------
//
// every read advances a cursor and bounds-checks against the input; a field
// that runs past the end is a truncation, and callers finish with `finish` so
// unconsumed trailing bytes reject.

fn push_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

/// every byte must be accounted for — a decode that stops short of the end saw
/// trailing bytes and is not canonical.
fn finish(bytes: &[u8], off: usize) -> Result<(), String> {
    if off != bytes.len() {
        return Err("files: object body has trailing bytes".into());
    }
    Ok(())
}

/// read `N` little-endian bytes, advancing the cursor; running past the end is a
/// truncation.
fn read_array<const N: usize>(bytes: &[u8], off: &mut usize) -> Result<[u8; N], String> {
    let end = off
        .checked_add(N)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| "files: object body truncated".to_string())?;
    let mut buf = [0u8; N];
    buf.copy_from_slice(&bytes[*off..end]);
    *off = end;
    Ok(buf)
}

fn read_u64(bytes: &[u8], off: &mut usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(read_array::<8>(bytes, off)?))
}

fn read_u32(bytes: &[u8], off: &mut usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(read_array::<4>(bytes, off)?))
}

fn read_u16(bytes: &[u8], off: &mut usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(read_array::<2>(bytes, off)?))
}

fn read_u8(bytes: &[u8], off: &mut usize) -> Result<u8, String> {
    Ok(read_array::<1>(bytes, off)?[0])
}

fn read_bytes32(bytes: &[u8], off: &mut usize) -> Result<[u8; 32], String> {
    read_array::<32>(bytes, off)
}

/// a single-byte boolean; only 0 and 1 are canonical, any other byte rejects.
fn read_bool(bytes: &[u8], off: &mut usize) -> Result<bool, String> {
    match read_u8(bytes, off)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err("files: expected a 0/1 boolean byte".into()),
    }
}

/// a `u64` length prefix followed by exactly that many utf-8 bytes. the length
/// is bounded by the remaining input before any allocation, so a bogus length
/// truncates rather than over-allocating.
fn read_string(bytes: &[u8], off: &mut usize) -> Result<String, String> {
    let len = read_u64(bytes, off)?;
    let len = usize::try_from(len).map_err(|_| "files: object body truncated".to_string())?;
    let end = off
        .checked_add(len)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| "files: object body truncated".to_string())?;
    let value = std::str::from_utf8(&bytes[*off..end])
        .map_err(|_| "files: object string is not utf-8".to_string())?;
    *off = end;
    Ok(value.to_owned())
}
