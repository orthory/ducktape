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
//!
//! SCOPE IS FROZEN (2026-07-13 storage-plane review): this crate stays a
//! dumb node-local receipt store. anything needing replication, GC,
//! authority, or auditability belongs in duckfs (module id `files`) — two
//! prior attempts to grow shared-byte planes beside duckfs (the `memory`
//! module, the prompt-blob mesh lane) were both deleted after converging on
//! duckfs. don't start a third.

mod staging;
pub use staging::{LARGE_BLOB_CACHE_BYTES, StageError, StagedBlob};

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sha2::{Digest as _, Sha256};

/// total resident bytes the memory map may hold for DISK-BACKED blobs. the
/// map used to never evict, so every ≤[`LARGE_BLOB_CACHE_BYTES`] put stayed
/// resident for the process lifetime — and the explorer projection puts one
/// op payload per op per block, so a stream of 1 MiB file chunks (a photo
/// dropped on the Files tab) grew node RSS without bound. eviction is safe
/// exactly because these blobs live on disk: a miss re-reads and re-verifies
/// the file.
const CACHE_BUDGET_BYTES: usize = 64 * 1024 * 1024;

/// the node-local blob store contract: the content-addressed put/get surface
/// consumers (statesync blob serve, the resident/validator relays, the
/// explorer index fold, the code source) read and write through. the real arm
/// is [`BlobHandle`] (in-memory with optional write-through persistence); the
/// sim arm is [`MemBlobs`]. staging ([`BlobHandle::stage`]) is deliberately
/// NOT on the contract — it is disk-slot-coupled and stays on the concrete
/// handle.
pub trait Blobs: Send + Sync + 'static {
    /// stage bytes under their sha256 content address, returning the digest.
    fn put_chunk(&self, bytes: Vec<u8>) -> [u8; 32];
    /// the whole blob, or `None` on an honest miss.
    fn get_chunk(&self, digest: &[u8; 32]) -> Option<Vec<u8>>;
    /// whether the store holds (and can verify) the blob.
    fn has_chunk(&self, digest: &[u8; 32]) -> bool;
    /// the blob's total length without materializing it.
    fn chunk_len(&self, digest: &[u8; 32]) -> Option<u64>;
    /// one bounded window `[offset, offset+len)`, clamped to the blob's end;
    /// `None` when the blob is absent or `offset` lies past its end.
    fn read_range(&self, digest: &[u8; 32], offset: u64, len: usize) -> Option<Vec<u8>>;
}

// crate-internal: [`BlobHandle`] is the public type embedders share.
#[derive(Default)]
pub(crate) struct BlobStore {
    chunks: HashMap<[u8; 32], Vec<u8>>,
    /// insertion order of the DISK-BACKED entries in `chunks` — the eviction
    /// queue. entries whose only copy is memory (pure in-memory stores, or a
    /// failed write-through) are deliberately absent: evicting one would lose
    /// the blob.
    cache_order: VecDeque<[u8; 32]>,
    /// total bytes of the entries in `cache_order`.
    cache_bytes: usize,
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
            root: Some(root),
            ..Self::default()
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
                // process lifetime.
                Ok(()) if bytes.len() > LARGE_BLOB_CACHE_BYTES => return digest,
                // disk holds the truth — memory is a bounded read cache.
                Ok(()) => {
                    self.cache(digest, bytes);
                    return digest;
                }
                Err(why) => eprintln!(
                    "[blobstore] cannot persist blob {} under {}: {why}; kept in memory only",
                    hex(&digest),
                    root.display()
                ),
            }
        }
        // no disk copy exists (pure in-memory store, or the write just
        // failed): the map IS the store for this blob, never evicted.
        self.chunks.insert(digest, bytes);
        digest
    }

    /// memory first, then the persistence root. a verified disk hit is cached
    /// back into memory only when small, through the same bounded cache the
    /// put path fills.
    pub fn get_chunk(&mut self, digest: &[u8; 32]) -> Option<Vec<u8>> {
        if let Some(bytes) = self.chunks.get(digest) {
            return Some(bytes.clone());
        }
        let bytes = self.disk_chunk(digest)?;
        if bytes.len() <= LARGE_BLOB_CACHE_BYTES {
            self.cache(*digest, bytes.clone());
        }
        Some(bytes)
    }

    /// park a DISK-BACKED blob in the memory map and keep the cache's total
    /// resident bytes under [`CACHE_BUDGET_BYTES`], evicting oldest-in first.
    /// only blobs that verifiably live on disk enter here, so eviction can
    /// never lose data — a miss re-reads and re-verifies the file.
    // ponytail: FIFO, not LRU — a get does not refresh recency. the cache
    // exists to stop unbounded growth; switch to LRU if repeated hot-blob
    // reads ever measure as a problem.
    fn cache(&mut self, digest: [u8; 32], bytes: Vec<u8>) {
        // content-addressed: same digest = same bytes, already resident (and
        // possibly pinned as memory-only) — nothing to add or re-queue.
        if self.chunks.contains_key(&digest) {
            return;
        }
        self.cache_bytes += bytes.len();
        self.cache_order.push_back(digest);
        self.chunks.insert(digest, bytes);
        while self.cache_bytes > CACHE_BUDGET_BYTES {
            let Some(oldest) = self.cache_order.pop_front() else {
                break;
            };
            if let Some(evicted) = self.chunks.remove(&oldest) {
                self.cache_bytes -= evicted.len();
            }
        }
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

/// the real arm: delegate the contract to the handle's inherent methods, so
/// concrete callers keep the inherent surface and `dyn Blobs` consumers get
/// the identical behavior.
impl Blobs for BlobHandle {
    fn put_chunk(&self, bytes: Vec<u8>) -> [u8; 32] {
        BlobHandle::put_chunk(self, bytes)
    }
    fn get_chunk(&self, digest: &[u8; 32]) -> Option<Vec<u8>> {
        BlobHandle::get_chunk(self, digest)
    }
    fn has_chunk(&self, digest: &[u8; 32]) -> bool {
        BlobHandle::has_chunk(self, digest)
    }
    fn chunk_len(&self, digest: &[u8; 32]) -> Option<u64> {
        BlobHandle::chunk_len(self, digest)
    }
    fn read_range(&self, digest: &[u8; 32], offset: u64, len: usize) -> Option<Vec<u8>> {
        BlobHandle::read_range(self, digest, offset, len)
    }
}

/// the sim arm: a pure in-memory [`Blobs`] over a `BTreeMap`, for tests and
/// embedders that want to inject a controlled (or deliberately empty) blob
/// source without a disk root. content addressing is identical to the real
/// store — the same sha256 keys the same bytes.
#[cfg(any(test, feature = "sim"))]
#[derive(Default)]
pub struct MemBlobs(Mutex<std::collections::BTreeMap<[u8; 32], Vec<u8>>>);

#[cfg(any(test, feature = "sim"))]
impl MemBlobs {
    fn map(&self) -> std::sync::MutexGuard<'_, std::collections::BTreeMap<[u8; 32], Vec<u8>>> {
        self.0.lock().expect("mem blobs poisoned")
    }
}

#[cfg(any(test, feature = "sim"))]
impl Blobs for MemBlobs {
    fn put_chunk(&self, bytes: Vec<u8>) -> [u8; 32] {
        let digest = sha256(&bytes);
        self.map().insert(digest, bytes);
        digest
    }
    fn get_chunk(&self, digest: &[u8; 32]) -> Option<Vec<u8>> {
        self.map().get(digest).cloned()
    }
    fn has_chunk(&self, digest: &[u8; 32]) -> bool {
        self.map().contains_key(digest)
    }
    fn chunk_len(&self, digest: &[u8; 32]) -> Option<u64> {
        self.map().get(digest).map(|b| b.len() as u64)
    }
    fn read_range(&self, digest: &[u8; 32], offset: u64, len: usize) -> Option<Vec<u8>> {
        let map = self.map();
        let bytes = map.get(digest)?;
        if offset > bytes.len() as u64 {
            return None;
        }
        let end = (offset as usize).saturating_add(len).min(bytes.len());
        Some(bytes[offset as usize..end].to_vec())
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

    /// the ONE contract every [`Blobs`] arm must satisfy — content addressing,
    /// round-trip, honest misses, length, and ranged reads. run against BOTH
    /// the disk-backed [`BlobHandle`] and the sim [`MemBlobs`] so a mock can
    /// never silently diverge from the real store.
    fn exercise(blobs: &dyn Blobs) {
        let digest = blobs.put_chunk(b"hello".to_vec());
        // put_chunk keys the blob by sha256 — the digest IS the content address.
        assert_eq!(digest, sha256(b"hello"));
        assert_eq!(blobs.get_chunk(&digest).as_deref(), Some(b"hello".as_ref()));
        assert!(blobs.has_chunk(&digest));
        // an unknown digest is an honest miss on every surface.
        assert!(!blobs.has_chunk(&[0u8; 32]));
        assert_eq!(blobs.get_chunk(&[0u8; 32]), None);
        assert_eq!(blobs.chunk_len(&digest), Some(5));
        assert_eq!(blobs.chunk_len(&[0u8; 32]), None);
        // one bounded window, and an offset past the end is a miss.
        assert_eq!(
            blobs.read_range(&digest, 1, 3).as_deref(),
            Some(b"ell".as_ref())
        );
        assert_eq!(blobs.read_range(&digest, 6, 3), None);
    }

    #[test]
    fn blob_handle_satisfies_the_blobs_contract() {
        exercise(&BlobHandle::default());
    }

    #[test]
    fn mem_blobs_satisfies_the_blobs_contract() {
        exercise(&MemBlobs::default());
    }

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
    fn the_disk_backed_memory_cache_stays_under_budget_and_loses_nothing() {
        let root = tempfile::tempdir().unwrap();
        let store = BlobHandle::persistent(root.path()).unwrap();
        let payload = |seed: u8| vec![seed; 1024 * 1024];
        // 65 distinct 1 MiB puts against a 64 MiB budget: the oldest evicts.
        let first = store.put_chunk(payload(0));
        for seed in 1..=64u8 {
            store.put_chunk(payload(seed));
        }
        {
            let inner = store.0.lock().unwrap();
            assert!(
                inner.cache_bytes <= CACHE_BUDGET_BYTES,
                "resident cache bytes stay under budget"
            );
            assert!(
                !inner.chunks.contains_key(&first),
                "the oldest cached blob was evicted from memory"
            );
        }
        // eviction lost nothing: the disk copy re-serves (re-verified).
        assert_eq!(
            store.get_chunk(&first).as_deref(),
            Some(payload(0).as_slice())
        );
    }

    #[test]
    fn a_memory_only_store_never_evicts() {
        // no root: the map IS the store, so the cache budget must not apply —
        // every blob stays readable no matter how many follow it.
        let store = BlobHandle::default();
        let payload = |seed: u8| vec![seed; 1024 * 1024];
        let digests: Vec<_> = (0..=64u8).map(|seed| store.put_chunk(payload(seed))).collect();
        for (seed, digest) in digests.iter().enumerate() {
            assert_eq!(
                store.get_chunk(digest).as_deref(),
                Some(payload(seed as u8).as_slice())
            );
        }
    }

    #[test]
    fn an_unusable_root_fails_at_construction() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("not-a-dir");
        std::fs::write(&file, b"x").unwrap();
        assert!(BlobStore::persistent(file.join("sub")).is_err());
    }
}
