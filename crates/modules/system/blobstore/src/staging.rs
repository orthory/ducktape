//! streaming staging: land a large content-addressed blob on disk without
//! ever holding it whole in memory.
//!
//! a [`StagedBlob`] is an exclusive, append-only writer for one expected
//! `(digest, len)`. bytes stream into `<root>/staging/<hex>` and fold into a
//! running sha256 as they land; [`StagedBlob::finish`] verifies length and
//! hash and only then publishes the file under its content-addressed name
//! (atomic rename) — bytes are never addressable before they verify, so a
//! crashed or lying transfer can never leave a file whose name misdescribes
//! its content. rootless (in-memory) stores stage into a plain buffer so the
//! API is uniform for tests and embedders.
//!
//! resume: re-opening a digest's staging slot rebuilds the running hash from
//! the bytes already on disk and reports the high-water [`StagedBlob::offset`]
//! for the sender to continue from. exclusivity is the CALLER's admission
//! contract — one live transfer per digest — the slot itself only ever
//! appends.

use std::io::{Read as _, Seek as _, Write as _};
use std::path::PathBuf;

use sha2::{Digest as _, Sha256};

use crate::{BlobHandle, hex};

/// blobs at or below this size are cached in the store's memory map on put /
/// get; larger blobs live on disk only (persistent stores) so a 1 GB
/// component is never parked in RAM for the process lifetime.
pub const LARGE_BLOB_CACHE_BYTES: usize = 4 * 1024 * 1024;

/// why a staged transfer could not complete.
#[derive(Debug)]
pub enum StageError {
    /// appending past the declared length, or finishing short of it.
    Length { expected: u64, got: u64 },
    /// the finished bytes do not hash to the declared digest.
    HashMismatch,
    /// the staging file could not be written / published.
    Io(std::io::Error),
}

impl std::fmt::Display for StageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Length { expected, got } => {
                write!(f, "staged length {got} does not match declared {expected}")
            }
            Self::HashMismatch => write!(f, "staged bytes do not hash to the declared digest"),
            Self::Io(e) => write!(f, "staging io: {e}"),
        }
    }
}

impl std::error::Error for StageError {}

impl From<std::io::Error> for StageError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// where the in-flight bytes accumulate.
enum Sink {
    /// persistent store: `<root>/staging/<hex>`, appended in order.
    Disk { file: std::fs::File, path: PathBuf },
    /// rootless store: a plain buffer (tests, embedders — small blobs).
    Memory(Vec<u8>),
}

/// an exclusive append-only staging slot for one expected `(digest, len)`.
pub struct StagedBlob {
    store: BlobHandle,
    digest: [u8; 32],
    expected_len: u64,
    hasher: Sha256,
    written: u64,
    sink: Sink,
}

impl BlobHandle {
    /// open — or resume — the staging slot for `digest`/`len`. on a
    /// persistent store an existing staging file's bytes are folded back into
    /// the running hash and appending continues from [`StagedBlob::offset`];
    /// a leftover file longer than `len` (a superseded transfer under a lying
    /// length) is discarded and staging restarts clean.
    pub fn stage(&self, digest: [u8; 32], len: u64) -> Result<StagedBlob, StageError> {
        let root = self.persistence_root();
        let (sink, hasher, written) = match root {
            None => (Sink::Memory(Vec::new()), Sha256::new(), 0),
            Some(root) => {
                let dir = root.join("staging");
                std::fs::create_dir_all(&dir)?;
                let path = dir.join(hex(&digest));
                let mut hasher = Sha256::new();
                let mut written = 0u64;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .read(true)
                    .write(true)
                    .open(&path)?;
                let existing = file.metadata()?.len();
                if existing > len {
                    file.set_len(0)?;
                } else {
                    // rebuild the running hash over the resumable prefix.
                    let mut buf = vec![0u8; 256 * 1024];
                    loop {
                        let n = file.read(&mut buf)?;
                        if n == 0 {
                            break;
                        }
                        hasher.update(&buf[..n]);
                        written += n as u64;
                    }
                }
                file.seek(std::io::SeekFrom::End(0))?;
                (Sink::Disk { file, path }, hasher, written)
            }
        };
        Ok(StagedBlob {
            store: self.clone(),
            digest,
            expected_len: len,
            hasher,
            written,
            sink,
        })
    }
}

impl StagedBlob {
    /// the resume high-water: how many bytes are already staged. a sender
    /// continues from exactly here.
    pub fn offset(&self) -> u64 {
        self.written
    }

    /// append the next in-order bytes. rejects growth past the declared
    /// length so a lying sender is stopped at the boundary it declared.
    pub fn append(&mut self, bytes: &[u8]) -> Result<(), StageError> {
        let next = self.written + bytes.len() as u64;
        if next > self.expected_len {
            return Err(StageError::Length {
                expected: self.expected_len,
                got: next,
            });
        }
        match &mut self.sink {
            Sink::Disk { file, .. } => file.write_all(bytes)?,
            Sink::Memory(buf) => buf.extend_from_slice(bytes),
        }
        self.hasher.update(bytes);
        self.written = next;
        Ok(())
    }

    /// verify length + hash, then publish: the staging file renames onto its
    /// content-addressed name (or the buffer inserts via the store's put).
    /// on any error the staging bytes are kept for resume — the caller
    /// decides between retry and [`StagedBlob::abort`].
    pub fn finish(self) -> Result<[u8; 32], StageError> {
        if self.written != self.expected_len {
            return Err(StageError::Length {
                expected: self.expected_len,
                got: self.written,
            });
        }
        let got: [u8; 32] = self.hasher.finalize().into();
        if got != self.digest {
            // a hash mismatch is never resumable — drop the poisoned bytes.
            if let Sink::Disk { path, .. } = &self.sink {
                let _ = std::fs::remove_file(path);
            }
            return Err(StageError::HashMismatch);
        }
        match self.sink {
            Sink::Disk { file, path } => {
                file.sync_all()?;
                drop(file);
                self.store.publish_staged(&self.digest, &path)?;
            }
            Sink::Memory(buf) => {
                self.store.put_chunk(buf);
            }
        }
        Ok(self.digest)
    }

    /// discard the staged bytes (a refused or superseded transfer).
    pub fn abort(self) {
        if let Sink::Disk { path, .. } = &self.sink {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha(bytes: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(bytes);
        h.finalize().into()
    }

    #[test]
    fn staged_blob_streams_verifies_and_publishes() {
        let root = tempfile::tempdir().unwrap();
        let store = BlobHandle::persistent(root.path()).unwrap();
        let payload = vec![7u8; 3 * 256 * 1024 + 17];
        let digest = sha(&payload);

        let mut slot = store.stage(digest, payload.len() as u64).unwrap();
        assert_eq!(slot.offset(), 0);
        for chunk in payload.chunks(256 * 1024) {
            slot.append(chunk).unwrap();
        }
        assert_eq!(slot.finish().unwrap(), digest);
        assert!(store.has_chunk(&digest));
        assert_eq!(store.get_chunk(&digest).as_deref(), Some(payload.as_slice()));
        // published under the content-addressed name; staging slot gone.
        assert!(root.path().join(hex(&digest)).is_file());
        assert!(!root.path().join("staging").join(hex(&digest)).exists());
    }

    #[test]
    fn resume_continues_from_the_high_water() {
        let root = tempfile::tempdir().unwrap();
        let store = BlobHandle::persistent(root.path()).unwrap();
        let payload = b"first half|second half".to_vec();
        let digest = sha(&payload);

        let mut slot = store.stage(digest, payload.len() as u64).unwrap();
        slot.append(b"first half|").unwrap();
        drop(slot); // transfer died mid-flight — file stays for resume.

        let mut resumed = store.stage(digest, payload.len() as u64).unwrap();
        assert_eq!(resumed.offset(), b"first half|".len() as u64);
        resumed.append(b"second half").unwrap();
        assert_eq!(resumed.finish().unwrap(), digest);
        assert_eq!(store.get_chunk(&digest).as_deref(), Some(payload.as_slice()));
    }

    #[test]
    fn wrong_bytes_never_publish_and_do_not_resume() {
        let root = tempfile::tempdir().unwrap();
        let store = BlobHandle::persistent(root.path()).unwrap();
        let digest = sha(b"the true bytes");

        let mut slot = store.stage(digest, 14).unwrap();
        slot.append(b"the LIED bytes").unwrap();
        assert!(matches!(slot.finish(), Err(StageError::HashMismatch)));
        assert!(!store.has_chunk(&digest));
        // poisoned staging is dropped — a retry starts clean.
        let fresh = store.stage(digest, 14).unwrap();
        assert_eq!(fresh.offset(), 0);
        fresh.abort();
    }

    #[test]
    fn length_lies_are_rejected_at_the_boundary() {
        let store = BlobHandle::default();
        let digest = sha(b"xyz");
        let mut slot = store.stage(digest, 3).unwrap();
        assert!(matches!(
            slot.append(b"xyzz"),
            Err(StageError::Length { expected: 3, got: 4 })
        ));
        slot.append(b"xyz").unwrap();
        assert_eq!(slot.finish().unwrap(), digest);
        assert!(store.has_chunk(&digest));
    }

    #[test]
    fn short_finish_is_length_error_and_resumable() {
        let store = BlobHandle::default();
        let digest = sha(b"abcdef");
        let mut slot = store.stage(digest, 6).unwrap();
        slot.append(b"abc").unwrap();
        assert!(matches!(
            slot.finish(),
            Err(StageError::Length { expected: 6, got: 3 })
        ));
    }
}
