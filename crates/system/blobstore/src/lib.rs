//! node-local content-addressed byte store for the daemon's receipt lane:
//! op payloads staged at submit time and served back over
//! `GET /v1/files/blob/{digest}`. never consensus state, never in any root.
//!
//! by default the store is pure in-memory (tests, embedders, forge's private
//! fallback store). a store built with [`BlobStore::persistent`] additionally
//! writes every chunk through to `<root>/<sha256-hex>` (atomic tmp+rename)
//! and falls back from memory to disk on a miss, so blobs — e.g. an agent's
//! registered prompt, pinned by hash in the runs envelope — survive a daemon
//! restart. content addressing makes disk blobs self-verifying: a file whose
//! bytes no longer hash to its name is a miss (plus a warning), never bad
//! bytes served. persistence is additive durability only — blobs stay
//! node-local staging, and nothing consensus-visible changes.

mod staging;
pub use staging::{LARGE_BLOB_CACHE_BYTES, StageError, StagedBlob};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sha2::{Digest as _, Sha256};

// crate-internal: [`BlobHandle`] is the public type embedders share.
#[derive(Default)]
pub(crate) struct BlobStore {
    chunks: HashMap<[u8; 32], Vec<u8>>,
    /// write-through persistence root; `None` = pure in-memory.
    root: Option<PathBuf>,
}

impl BlobStore {
    /// a store that writes every chunk through to `<root>/<sha256-hex>` and
    /// falls back to disk on a memory miss. creates the directory up front so
    /// an unusable root fails loudly at construction, not on the first put.
    pub fn persistent(root: impl Into<PathBuf>) -> std::io::Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            chunks: HashMap::new(),
            root: Some(root),
        })
    }

    pub fn put_chunk(&mut self, bytes: Vec<u8>) -> [u8; 32] {
        let digest = sha256(&bytes);
        // write-through BEFORE the memory insert, always (re-putting heals a
        // corrupt file on disk). a disk failure degrades to memory-only with
        // a warning — put stays infallible, exactly the old semantics.
        if let Some(root) = &self.root {
            match write_through(root, &digest, &bytes) {
                // a persisted LARGE blob lives on disk only: parking it in the
                // map would grow resident memory by the blob size for the
                // process lifetime (the map never evicts).
                Ok(()) if bytes.len() > LARGE_BLOB_CACHE_BYTES => return digest,
                Ok(()) => {}
                Err(why) => eprintln!(
                    "[blobstore] cannot persist blob {} under {}: {why}; kept in memory only",
                    hex(&digest),
                    root.display()
                ),
            }
        }
        self.chunks.insert(digest, bytes);
        digest
    }

    /// memory first, then the persistence root. a verified disk hit is cached
    /// back into memory only when small — the map never evicts, so a large
    /// blob would otherwise stay resident for the process lifetime.
    pub fn get_chunk(&mut self, digest: &[u8; 32]) -> Option<Vec<u8>> {
        if let Some(bytes) = self.chunks.get(digest) {
            return Some(bytes.clone());
        }
        let bytes = self.disk_chunk(digest)?;
        if bytes.len() <= LARGE_BLOB_CACHE_BYTES {
            self.chunks.insert(*digest, bytes.clone());
        }
        Some(bytes)
    }

    /// the blob's total length: memory hit, else the published file's size.
    /// range serving trusts publish-time verification (the atomic rename is
    /// the receipt) — the requester re-verifies the assembled whole.
    pub fn chunk_len(&self, digest: &[u8; 32]) -> Option<u64> {
        if let Some(bytes) = self.chunks.get(digest) {
            return Some(bytes.len() as u64);
        }
        let path = self.root.as_ref()?.join(hex(digest));
        std::fs::metadata(path).ok().map(|m| m.len())
    }

    /// one bounded window of a blob, without reading the whole file: memory
    /// hit slices, disk hit seeks. `None` when the blob is absent or `offset`
    /// lies past its end. ranged bytes are NOT re-verified per read (that
    /// would read the whole blob every range) — the assembling requester
    /// verifies the whole against the digest, fail-closed.
    pub fn read_range(&self, digest: &[u8; 32], offset: u64, len: usize) -> Option<Vec<u8>> {
        if let Some(bytes) = self.chunks.get(digest) {
            let total = bytes.len() as u64;
            if offset > total {
                return None;
            }
            let end = (offset as usize).saturating_add(len).min(bytes.len());
            return Some(bytes[offset as usize..end].to_vec());
        }
        let path = self.root.as_ref()?.join(hex(digest));
        let mut file = std::fs::File::open(path).ok()?;
        let total = file.metadata().ok()?.len();
        if offset > total {
            return None;
        }
        use std::io::{Read as _, Seek as _};
        file.seek(std::io::SeekFrom::Start(offset)).ok()?;
        let want = len.min((total - offset) as usize);
        let mut buf = vec![0u8; want];
        file.read_exact(&mut buf).ok()?;
        Some(buf)
    }

    pub fn has_chunk(&self, digest: &[u8; 32]) -> bool {
        self.chunks.contains_key(digest) || self.disk_chunk(digest).is_some()
    }

    /// the disk fallback: read `<root>/<hex>` and REVERIFY the content hash.
    /// content-addressed blobs are self-verifying — a corrupt or truncated
    /// file is treated as absent (with a warning), never served.
    fn disk_chunk(&self, digest: &[u8; 32]) -> Option<Vec<u8>> {
        let path = self.root.as_ref()?.join(hex(digest));
        let bytes = std::fs::read(&path).ok()?;
        if sha256(&bytes) != *digest {
            eprintln!(
                "[blobstore] blob file {} fails its content hash; treating it as absent",
                path.display()
            );
            return None;
        }
        Some(bytes)
    }
}

#[derive(Clone, Default)]
pub struct BlobHandle(Arc<Mutex<BlobStore>>);

impl BlobHandle {
    /// a shared handle over a [`BlobStore::persistent`] store — see there.
    pub fn persistent(root: impl Into<PathBuf>) -> std::io::Result<Self> {
        Ok(Self(Arc::new(Mutex::new(BlobStore::persistent(root)?))))
    }

    pub fn put_chunk(&self, bytes: Vec<u8>) -> [u8; 32] {
        self.0.lock().expect("blob store poisoned").put_chunk(bytes)
    }

    pub fn get_chunk(&self, digest: &[u8; 32]) -> Option<Vec<u8>> {
        self.0
            .lock()
            .expect("blob store poisoned")
            .get_chunk(digest)
    }

    pub fn has_chunk(&self, digest: &[u8; 32]) -> bool {
        self.0
            .lock()
            .expect("blob store poisoned")
            .has_chunk(digest)
    }

    /// see [`BlobStore::chunk_len`].
    pub fn chunk_len(&self, digest: &[u8; 32]) -> Option<u64> {
        self.0
            .lock()
            .expect("blob store poisoned")
            .chunk_len(digest)
    }

    /// see [`BlobStore::read_range`].
    pub fn read_range(&self, digest: &[u8; 32], offset: u64, len: usize) -> Option<Vec<u8>> {
        self.0
            .lock()
            .expect("blob store poisoned")
            .read_range(digest, offset, len)
    }

    /// the write-through root, when persistent — where staging slots live.
    pub(crate) fn persistence_root(&self) -> Option<PathBuf> {
        self.0.lock().expect("blob store poisoned").root.clone()
    }

    /// publish a VERIFIED staging file under its content-addressed name. the
    /// rename is atomic: the blob is either fully addressable or absent.
    pub(crate) fn publish_staged(&self, digest: &[u8; 32], staged: &Path) -> std::io::Result<()> {
        let root = self
            .persistence_root()
            .expect("publish_staged is only reachable from a disk staging sink");
        std::fs::rename(staged, root.join(hex(digest)))
    }
}

/// atomic write-through: land the bytes in a temp file, then rename onto the
/// content-addressed name — a crash mid-write leaves a stray tmp file, never
/// a half-written blob under a valid name.
fn write_through(root: &Path, digest: &[u8; 32], bytes: &[u8]) -> std::io::Result<()> {
    let tmp = root.join(format!(".tmp-{}-{}", hex(digest), std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, root.join(hex(digest))).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

pub(crate) fn hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_store_round_trips_and_stays_off_disk() {
        let store = BlobHandle::default();
        let digest = store.put_chunk(b"hello".to_vec());
        assert_eq!(digest, sha256(b"hello"));
        assert_eq!(store.get_chunk(&digest).as_deref(), Some(b"hello".as_ref()));
        assert!(store.has_chunk(&digest));
        assert!(!store.has_chunk(&[0u8; 32]));
        assert_eq!(store.get_chunk(&[0u8; 32]), None);
    }

    #[test]
    fn persistent_blobs_survive_a_restart() {
        let root = tempfile::tempdir().unwrap();
        let digest = {
            // "store A": the daemon before a restart.
            let a = BlobHandle::persistent(root.path()).unwrap();
            a.put_chunk(b"You are Bot, the release captain.".to_vec())
        };
        // "store B": a fresh process over the same root — cold memory.
        let b = BlobHandle::persistent(root.path()).unwrap();
        assert!(b.has_chunk(&digest));
        assert_eq!(
            b.get_chunk(&digest).as_deref(),
            Some(b"You are Bot, the release captain.".as_ref())
        );
        // the blob landed under its content-addressed name, no tmp strays.
        assert!(root.path().join(hex(&digest)).is_file());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_corrupt_disk_blob_is_a_miss_never_bad_bytes() {
        let root = tempfile::tempdir().unwrap();
        let digest = BlobHandle::persistent(root.path())
            .unwrap()
            .put_chunk(b"original".to_vec());
        std::fs::write(root.path().join(hex(&digest)), b"tampered").unwrap();

        let fresh = BlobHandle::persistent(root.path()).unwrap();
        assert_eq!(fresh.get_chunk(&digest), None);
        assert!(!fresh.has_chunk(&digest));

        // re-putting the true bytes heals the file (the F1 remedy flow:
        // re-save the prompt from the app).
        let healed = fresh.put_chunk(b"original".to_vec());
        assert_eq!(healed, digest);
        assert_eq!(
            BlobHandle::persistent(root.path())
                .unwrap()
                .get_chunk(&digest)
                .as_deref(),
            Some(b"original".as_ref())
        );
    }

    #[test]
    fn memory_wins_over_a_corrupted_disk_copy() {
        let root = tempfile::tempdir().unwrap();
        let store = BlobHandle::persistent(root.path()).unwrap();
        let digest = store.put_chunk(b"payload".to_vec());
        std::fs::write(root.path().join(hex(&digest)), b"garbage").unwrap();
        // the live process still holds the true bytes in memory.
        assert_eq!(
            store.get_chunk(&digest).as_deref(),
            Some(b"payload".as_ref())
        );
    }

    #[test]
    fn a_verified_disk_hit_is_cached_back_into_memory() {
        let root = tempfile::tempdir().unwrap();
        let digest = BlobHandle::persistent(root.path())
            .unwrap()
            .put_chunk(b"warm me".to_vec());

        let fresh = BlobHandle::persistent(root.path()).unwrap();
        assert_eq!(
            fresh.get_chunk(&digest).as_deref(),
            Some(b"warm me".as_ref())
        );
        // deleting the file after the first get: the cached copy still serves.
        std::fs::remove_file(root.path().join(hex(&digest))).unwrap();
        assert_eq!(
            fresh.get_chunk(&digest).as_deref(),
            Some(b"warm me".as_ref())
        );
    }

    #[test]
    fn an_unusable_root_fails_at_construction() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("not-a-dir");
        std::fs::write(&file, b"x").unwrap();
        assert!(BlobStore::persistent(file.join("sub")).is_err());
    }
}
