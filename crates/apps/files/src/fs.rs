//! the pure duckfs state machine — every consensus semantic lands here, over
//! the [`ObjectStore`] seam, with no sdk, no async, and no disk io anywhere.
//! the native glue (`module.rs`) maps origin/env in and notifications out.
//! tasks 7-14 fill the op/query/sync semantics; this skeleton pins the shapes.

use crate::objects::{Kind, ObjectId, object_id};
use crate::state::{Refs, Staged, decode_refs, encode_refs, root_bytes};
use crate::store::ObjectStore;
use crate::wire::{
    CHUNK_SIZE, Change, FilesQuery, FilesReply, FilesSyncReq, FilesSyncResp, STAGING_QUOTA_BYTES,
    STAGING_TTL_BLOCKS,
};

pub struct Fs<S: ObjectStore> {
    pub(crate) store: S,
    pub(crate) refs: Refs,
    pub(crate) pending: Option<Pending>,
    /// per-owner staging byte ceiling — [`STAGING_QUOTA_BYTES`] in production,
    /// lowered only by the `#[doc(hidden)]` test override so the quota-boundary
    /// logic can be exercised without staging a full gibibyte per owner.
    pub(crate) quota: u64,
}

/// a block's staged objects — `(kind, body)` pairs the glue flushes into the
/// odb at commit. named so the block-boundary signatures stay legible.
pub type StagedObjects = Vec<(Kind, Vec<u8>)>;

/// per-block overlay: refs-next plus objects awaiting the store flush.
pub(crate) struct Pending {
    pub refs: Refs,
    pub objects: StagedObjects,
    pub height: u64,
}

/// one watch hit produced by a commit; the glue turns each into an emitted
/// follow-up msg (task 9).
pub struct Notification {
    pub module_id: String,
    pub prefix: String,
    pub path: String,
    pub snapshot: String,
}

fn unimplemented_err() -> String {
    "files: unimplemented".into()
}

/// remove every staging entry whose ttl has elapsed at `height`. the condition
/// is `expires_at <= height` (encoded as the `> height` retain predicate): a
/// chunk staged at block h with ttl T (so `expires_at = h + T`) is swept the
/// first time files is active at-or-after block h + T — never a block late.
///
/// this is the deterministic, op-stream-driven staging sweep. run at the top of
/// every mutating verb, it makes expiry a pure function of the op stream: it
/// lands at the first files-activity block at-or-after `expires_at`, identically
/// on every validator (no wall clock, no per-node timer). swept chunks lose
/// their staging root and fall to the next gc. `pub(crate)` so tasks 9/10 reuse
/// it from commit/pin/unpin/watch/unwatch.
pub(crate) fn sweep_expired(refs: &mut Refs, height: u64) {
    refs.staging
        .retain(|_digest, staged| staged.expires_at > height);
}

impl<S: ObjectStore> Fs<S> {
    pub fn new(store: S, refs: Refs) -> Self {
        Self {
            store,
            refs,
            pending: None,
            quota: STAGING_QUOTA_BYTES,
        }
    }

    /// `#[doc(hidden)]` test seam: shrink the per-owner staging quota so the
    /// boundary logic is exercised without staging a full gibibyte. production
    /// never calls this — the quota stays [`STAGING_QUOTA_BYTES`].
    #[doc(hidden)]
    pub fn set_staging_quota_for_tests(&mut self, quota: u64) {
        self.quota = quota;
    }

    /// fork committed refs into this block's pending overlay on first touch, so a
    /// mutating verb edits the pending view while the committed root stays put
    /// until `commit_block` + `adopt_refs`. reused by every mutating verb (tasks
    /// 9/10); callers grab `self.pending` afterward so the field borrow stays
    /// disjoint from `self.store` (a `&mut Pending` return would alias `self`).
    pub(crate) fn require_pending(&mut self, height: u64) {
        if self.pending.is_none() {
            self.pending = Some(Pending {
                refs: self.refs.clone(),
                objects: Vec::new(),
                height,
            });
        }
    }

    /// committed refs only — the pending overlay never leaks into the root.
    pub fn root_bytes(&self) -> [u8; 32] {
        root_bytes(&self.refs)
    }

    pub fn refs(&self) -> &Refs {
        &self.refs
    }

    /// direct access to the object store — the native glue (`module.rs`) needs
    /// the concrete `S` after [`Fs::commit_block`] to flush the block's objects
    /// and fsync their odb dirs, and by-hand durability tests drive the same
    /// seam. hidden because it is glue/test plumbing, not part of the semantic
    /// surface (all consensus reads/writes go through the typed methods).
    #[doc(hidden)]
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    /// stage a pending block directly — a `#[doc(hidden)]` seam so durability
    /// tests can drive the block boundary before the op semantics (tasks 7/9/10)
    /// land. production staging happens inside the op methods.
    #[doc(hidden)]
    pub fn stage_pending(&mut self, refs: Refs, height: u64, objects: StagedObjects) {
        self.pending = Some(Pending {
            refs,
            objects,
            height,
        });
    }

    // ---- op surface (semantics land in tasks 7/9/10) ------------------------

    /// stage a raw chunk for a later commit to reference. bytes are consensus
    /// state: staged now, durable at THIS block's commit, gc-reachable via the
    /// staging table until referenced or expired.
    pub fn putblob(&mut self, actor: &str, height: u64, bytes: &[u8]) -> Result<(), String> {
        // tick the deterministic staging sweep first, over the pending view, so
        // same-block ops and the quota below see the post-sweep state.
        self.require_pending(height);
        let quota = self.quota;
        // disjoint field borrows: the sweep/stage touch `pending`, the dedup
        // reads `store` — held at once only because they are distinct fields.
        let store = &self.store;
        let pending = self.pending.as_mut().expect("require_pending set it");
        sweep_expired(&mut pending.refs, height);

        // a malformed frame is not a stageable object. (a rejected op aborts the
        // whole block in production, so this never leaves the sweep half-applied;
        // the direct-execute tests likewise keep earlier same-block stages.)
        if bytes.is_empty() {
            return Err("chunk must not be empty".into());
        }
        if bytes.len() as u64 > CHUNK_SIZE {
            return Err("chunk exceeds CHUNK_SIZE".into());
        }

        let digest = object_id(Kind::Chunk, bytes);

        // already durable → no-op, no quota charge. either the committed odb holds
        // it, or an earlier op THIS block already staged it: every chunk putblob
        // buffers into pending.objects it inserts into staging in the same breath,
        // so this O(1) staging membership check subsumes a pending.objects scan
        // (and avoids re-hashing every buffered megabyte on each call).
        if store.has(&digest) || pending.refs.staging.contains_key(&digest) {
            return Ok(());
        }

        // per-owner quota over the PENDING staging view (same-block stages count).
        let len = bytes.len() as u64;
        let used = pending
            .refs
            .staging
            .values()
            .filter(|s| s.owner == actor)
            .fold(0u64, |acc, s| acc.saturating_add(s.len));
        if used.saturating_add(len) > quota {
            return Err("staging quota exceeded".into());
        }

        // stage: the entry makes the chunk gc-reachable (task 13 marks staging
        // digests as roots), and the bytes ride pending.objects so they are
        // durable at this block's commit.
        pending.refs.staging.insert(
            digest,
            Staged {
                owner: actor.to_string(),
                len,
                expires_at: height.saturating_add(STAGING_TTL_BLOCKS),
            },
        );
        pending.objects.push((Kind::Chunk, bytes.to_vec()));
        Ok(())
    }

    pub fn commit(
        &mut self,
        _actor: &str,
        _height: u64,
        _time: u64,
        _base: Option<String>,
        _message: String,
        _changes: Vec<Change>,
    ) -> Result<Vec<Notification>, String> {
        Err(unimplemented_err())
    }

    pub fn pin(&mut self, _actor: &str, _snapshot: String, _name: String) -> Result<(), String> {
        Err(unimplemented_err())
    }

    pub fn unpin(&mut self, _actor: &str, _name: String) -> Result<(), String> {
        Err(unimplemented_err())
    }

    pub fn watch(
        &mut self,
        _actor: &str,
        _is_module: bool,
        _prefix: String,
        _module_id: String,
    ) -> Result<(), String> {
        Err(unimplemented_err())
    }

    pub fn unwatch(
        &mut self,
        _actor: &str,
        _is_module: bool,
        _prefix: String,
        _module_id: String,
    ) -> Result<(), String> {
        Err(unimplemented_err())
    }

    // ---- block boundary ------------------------------------------------------

    /// hand the block's staged `(refs, height, objects)` to the caller WITHOUT
    /// touching committed state — no object flush, no `self.refs` swap, no
    /// root movement. `None` when the block staged nothing.
    ///
    /// this pure hand-off is the whole point of task 6's durability ordering.
    /// the committed root must never run ahead of the durable refs file, or a
    /// crash mid-commit reproduces this repo's historic torn-commit brick (a
    /// disk module already at its post-root while its refs are still pre). so
    /// the caller (the native glue) must, in exactly this order:
    ///
    /// 1. `store_mut().put` every returned object (idempotent, content-addressed)
    /// 2. fsync the touched odb dirs (object dir-entries durable)
    /// 3. persist the refs file via `RefsStore::save` (the commit point)
    /// 4. only THEN [`Fs::adopt_refs`] — root moves here and nowhere else
    ///
    /// a crash before step 3 leaves the old refs file, the old root, and at
    /// worst some orphan objects (harmless: content-addressed, idempotently
    /// re-put on replay, swept by a later gc). a crash after step 3 has the new
    /// refs and — because step 2 preceded it — every object it names, durable.
    /// there is no torn window.
    pub fn commit_block(&mut self) -> Option<(Refs, u64, StagedObjects)> {
        let pending = self.pending.take()?;
        Some((pending.refs, pending.height, pending.objects))
    }

    /// adopt the block's refs as committed — the caller invokes this ONLY after
    /// the refs file is durably saved (see [`Fs::commit_block`]). this is the
    /// single place the committed root moves.
    pub fn adopt_refs(&mut self, refs: Refs) {
        self.refs = refs;
    }

    pub fn abort_block(&mut self) {
        self.pending = None;
    }

    // ---- read + sync surface (tasks 11/12/14) --------------------------------

    /// committed state only — never the pending overlay.
    pub fn query(&self, _q: FilesQuery) -> Result<FilesReply, String> {
        Err(unimplemented_err())
    }

    pub fn serve_sync(&self, _req: FilesSyncReq) -> Result<FilesSyncResp, String> {
        Err(unimplemented_err())
    }

    /// the exact `root_bytes` preimage — what the snapshot lane ships.
    pub fn snapshot_refs(&self) -> Vec<u8> {
        encode_refs(&self.refs)
    }

    /// verify-then-adopt: strict-decode a peer's refs image, check it against
    /// the expected root, then swap committed refs in and drop any staged block.
    pub fn install_refs(&mut self, bytes: &[u8], expected_root: [u8; 32]) -> Result<(), String> {
        let refs = decode_refs(bytes)?;
        if root_bytes(&refs) != expected_root {
            return Err("files: refs image does not match the expected root".into());
        }
        self.refs = refs;
        self.pending = None;
        Ok(())
    }

    pub fn missing_objects(&self, _limit: usize) -> Result<Vec<ObjectId>, String> {
        Err(unimplemented_err())
    }

    pub fn ingest_object(&mut self, _id: &ObjectId, _kind: u8, _body: &[u8]) -> Result<(), String> {
        Err(unimplemented_err())
    }

    /// mark + sweep now; the CALLER decides when (task 13 wires the trigger).
    pub fn gc(&mut self) -> Result<u64, String> {
        Err(unimplemented_err())
    }
}
