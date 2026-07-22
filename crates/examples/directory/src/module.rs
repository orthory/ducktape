//! the native, in-memory implementation of the directory module.
//!
//! deliberately NOT qmdb: its state is a `BTreeMap`, so `root()` and `query()`
//! are SYNC — which is what lets a peer module do a live cross-module `ctx.query`
//! of it without opening the async-query question. its root is a state-based hash
//! (order-independent + idempotent), the correct shape for a module commitment.
//!
//! the canonical encoding here (le-u64 entry count, then sorted le-u64
//! length-prefixed pairs) is BYTE-IDENTICAL to the wasm host store's — the
//! `directory` guest carries the same keys/values, so root(), snapshot(),
//! and install() stay continuous across a native↔wasm cutover. change either
//! side only in lockstep with the other.

use crate::interface::*;

use std::collections::BTreeMap;

use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};

pub struct Directory {
    id: ModuleId,
    /// committed state — what `root()` and the app-hash commit to.
    entries: BTreeMap<String, String>,
    /// writes staged during the current block: read ahead of `entries` (read-
    /// your-writes) but merged in — and reflected in `root()` — only when the
    /// host calls `commit_block`.
    pending: BTreeMap<String, String>,
}

impl Directory {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            entries: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    /// direct sync write (used by `execute` and handy for tests/genesis seeding).
    pub fn set(&mut self, key: String, value: String) {
        self.entries.insert(key, value);
    }

    /// stage `key -> value` for this block WITHOUT committing — visible to reads
    /// at once (read-your-writes), merged into committed state (and `root()`)
    /// only when the host calls `commit_block` at the block boundary.
    pub fn stage(&mut self, key: String, value: String) {
        self.pending.insert(key, value);
    }

    /// read `key`: a STAGED (this-block) write shadows committed state.
    pub fn get(&self, key: &str) -> Option<&String> {
        self.pending.get(key).or_else(|| self.entries.get(key))
    }

    /// the state-based commitment for `entries`: a length-prefixed sha256 over
    /// the sorted pairs. shared by `root()` and `install` so the root recomputed
    /// from a decoded snapshot can never drift from the live algorithm.
    fn root_of(entries: &BTreeMap<String, String>) -> StateRoot {
        let mut h = Sha256::new();
        h.update((entries.len() as u64).to_le_bytes());
        for (k, v) in entries {
            h.update((k.len() as u64).to_le_bytes());
            h.update(k.as_bytes());
            h.update((v.len() as u64).to_le_bytes());
            h.update(v.as_bytes());
        }
        StateRoot(h.finalize().into())
    }

    // ---- state-sync ---------------------------------------------------------
    // rebuild committed state from a peer's snapshot. the peer is untrusted —
    // the expected root (obtained from consensus, not from the peer) is the
    // trust anchor, and install fully verifies before it mutates.

    /// deterministic canonical encoding of COMMITTED state: exactly the byte
    /// stream `root()` hashes (le-u64 entry count, then sorted le-u64
    /// length-prefixed key/value pairs), so `sha256(snapshot()) == root()` by
    /// construction. staged writes are excluded — a snapshot is a
    /// committed-state artifact.
    pub fn snapshot(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.entries.len() as u64).to_le_bytes());
        for (k, v) in &self.entries {
            out.extend_from_slice(&(k.len() as u64).to_le_bytes());
            out.extend_from_slice(k.as_bytes());
            out.extend_from_slice(&(v.len() as u64).to_le_bytes());
            out.extend_from_slice(v.as_bytes());
        }
        out
    }

    /// replace COMMITTED state with a peer-provided snapshot, gated on
    /// `expected`. the bytes arrive from a byzantine peer, so decode is strict:
    /// every declared length is checked against the remaining buffer before
    /// anything is allocated, keys must be strictly ascending (the one order
    /// `snapshot` emits — rejects duplicates and re-encodings), trailing bytes
    /// are rejected, and the decoded state's recomputed root must equal
    /// `expected`. verification completes BEFORE any mutation: on Err this
    /// module's state (and `root()`) is byte-identical to before the call.
    /// success also drops the staged overlay — the snapshot is the whole truth.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let mut off = 0usize;
        let count = read_u64(bytes, &mut off)?;
        // an entry costs at least its two 8-byte length prefixes, so a count the
        // remaining buffer cannot possibly hold is rejected before the loop.
        if count > ((bytes.len() - off) / 16) as u64 {
            return Err(Error::Module("snapshot truncated".into()));
        }
        let mut entries: BTreeMap<String, String> = BTreeMap::new();
        for _ in 0..count {
            let key = read_string(bytes, &mut off)?;
            let value = read_string(bytes, &mut off)?;
            if entries
                .last_key_value()
                .is_some_and(|(last, _)| *last >= key)
            {
                return Err(Error::Module("snapshot keys not strictly ascending".into()));
            }
            entries.insert(key, value);
        }
        if off != bytes.len() {
            return Err(Error::Module("snapshot has trailing bytes".into()));
        }
        if Self::root_of(&entries) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        self.entries = entries;
        self.pending.clear();
        Ok(())
    }
}

/// read a le-u64 at `*off`, advancing it. the buffer is untrusted: truncation
/// is an Err, never a panic.
fn read_u64(bytes: &[u8], off: &mut usize) -> Result<u64, Error> {
    let end = off
        .checked_add(8)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[*off..end]);
    *off = end;
    Ok(u64::from_le_bytes(buf))
}

/// read a length-prefixed utf-8 string at `*off`. the declared length is
/// validated against the REMAINING buffer before any allocation, so a forged
/// length can neither oversize-allocate nor read out of bounds.
fn read_string(bytes: &[u8], off: &mut usize) -> Result<String, Error> {
    let len = read_u64(bytes, off)?;
    let len = usize::try_from(len).map_err(|_| Error::Module("snapshot truncated".into()))?;
    if len > bytes.len() - *off {
        return Err(Error::Module("snapshot truncated".into()));
    }
    let s = std::str::from_utf8(&bytes[*off..*off + len])
        .map_err(|_| Error::Module("snapshot string is not utf-8".into()))?;
    *off += len;
    Ok(s.to_owned())
}

#[async_trait::async_trait(?Send)]
impl Module for Directory {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment: a length-prefixed sha256 over the sorted entries.
    /// order-independent (BTreeMap) and idempotent — f(current state), unlike qmdb.
    fn root(&self) -> StateRoot {
        Directory::root_of(&self.entries)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            DirMsg::Set { key, value } => self.stage(key, value),
        }
        Ok(())
    }

    /// read projection — serves other modules' `ctx.query` + external reads.
    /// async per the trait, though the in-memory body has nothing to await.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            DirQuery::Get { key } => Ok(encode_reply(&DirReply::Value(self.get(&key).cloned()))),
        }
    }

    /// merge the block's staged writes into committed state — `root()` now
    /// reflects them. no-op if nothing was staged.
    async fn commit_block(&mut self) -> Result<(), Error> {
        for (k, v) in std::mem::take(&mut self.pending) {
            self.entries.insert(k, v);
        }
        Ok(())
    }

    /// discard the block's staged writes — committed state (and `root()`) is
    /// unchanged.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_query;

    #[test]
    fn set_query_and_state_based_root() {
        let mut d = Directory::new("directory");
        let r0 = d.root();
        d.set("a".into(), "1".into());
        let r1 = d.root();
        assert_ne!(r0, r1, "a write must move the root");

        let reply =
            futures::executor::block_on(d.query(&encode_query(&DirQuery::Get { key: "a".into() })))
                .unwrap();
        assert_eq!(
            crate::decode_reply(&reply).unwrap(),
            DirReply::Value(Some("1".into()))
        );

        // state-based: same final content -> same root regardless of history.
        let mut e = Directory::new("directory");
        e.set("a".into(), "1".into());
        assert_eq!(
            r1,
            e.root(),
            "root must be f(state), order/history-independent"
        );
    }
}
