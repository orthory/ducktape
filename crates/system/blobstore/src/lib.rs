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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sha2::{Digest as _, Sha256};

#[derive(Default)]
pub struct BlobStore {
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
        if let Some(root) = &self.root
            && let Err(why) = write_through(root, &digest, &bytes)
        {
            eprintln!(
                "[blobstore] cannot persist blob {} under {}: {why}; kept in memory only",
                hex(&digest),
                root.display()
            );
        }
        self.chunks.insert(digest, bytes);
        digest
    }

    /// memory first, then the persistence root; a verified disk hit is cached
    /// back into memory so the borrow it returns lives in the map either way.
    pub fn get_chunk(&mut self, digest: &[u8; 32]) -> Option<&[u8]> {
        if !self.chunks.contains_key(digest) {
            let bytes = self.disk_chunk(digest)?;
            self.chunks.insert(*digest, bytes);
        }
        self.chunks.get(digest).map(Vec::as_slice)
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
            .map(<[u8]>::to_vec)
    }

    pub fn has_chunk(&self, digest: &[u8; 32]) -> bool {
        self.0
            .lock()
            .expect("blob store poisoned")
            .has_chunk(digest)
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

fn hex(digest: &[u8; 32]) -> String {
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
