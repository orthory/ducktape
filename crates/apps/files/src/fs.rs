//! the pure duckfs state machine — every consensus semantic lands here, over
//! the [`ObjectStore`] seam, with no sdk, no async, and no disk io anywhere.
//! the native glue (`module.rs`) maps origin/env in and notifications out.
//! tasks 7-14 fill the op/query/sync semantics; this skeleton pins the shapes.

use std::collections::{BTreeMap, BTreeSet};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::objects::{
    EntryKind, FileObj, Kind, ObjectId, SnapshotObj, TreeEntry, TreeObj, object_id,
    verify_file_shape,
};
use crate::paths::{canonical, check_authority};
use crate::state::{PinEntry, Refs, Staged, decode_refs, encode_refs, root_bytes};
use crate::store::ObjectStore;
use crate::tree::{Store, TreeEdit, entry_at, snapshot_root_tree};
use crate::wire::{
    CHUNK_SIZE, Change, Content, FilesQuery, FilesReply, FilesSyncReq, FilesSyncResp,
    HISTORY_WINDOW, MAX_CHANGES_PER_COMMIT, MAX_CHUNKS_PER_FILE, MAX_GREP_SCAN_BYTES,
    MAX_INLINE_COMMIT_BYTES, MAX_MESSAGE_BYTES, MAX_META_ENTRIES, MAX_META_KEY_BYTES,
    MAX_META_VALUE_BYTES, MAX_PIN_NAME_BYTES, MAX_PINS, MAX_STAGING_ENTRIES,
    MAX_STAGING_ENTRIES_PER_OWNER, MAX_SYMLINK_TARGET_BYTES, MAX_SYNC_IDS, MAX_SYNC_REPLY_BYTES,
    MAX_WATCH_MODULE_ID_BYTES, MAX_WATCHES, STAGING_QUOTA_BYTES, STAGING_TTL_BLOCKS, SyncObject,
    from_hex_32, to_hex,
};

pub struct Fs<S: ObjectStore> {
    pub(crate) store: S,
    pub(crate) refs: Refs,
    pub(crate) pending: Option<Pending>,
    /// per-owner staging byte ceiling — [`STAGING_QUOTA_BYTES`] in production,
    /// lowered only by the `#[doc(hidden)]` test override so the quota-boundary
    /// logic can be exercised without staging a full gibibyte per owner.
    pub(crate) quota: u64,
    /// per-call grep scan budget — [`MAX_GREP_SCAN_BYTES`] in production, lowered
    /// only by the `#[doc(hidden)]` test override so the budget-boundary + resume
    /// logic can be exercised without a multi-megabyte fixture per call.
    pub(crate) grep_budget: u64,
    /// bounded history-window capacity — [`HISTORY_WINDOW`] in production, shrunk
    /// only by the `#[doc(hidden)]` test override so gc's window-expiry sweep can
    /// be exercised with a handful of commits instead of thousands. commit's
    /// window pop keys on this.
    pub(crate) window_cap: usize,
    /// global staging-table entry ceiling — [`MAX_STAGING_ENTRIES`] in production,
    /// lowered only by the `#[doc(hidden)]` test override so the table-full
    /// boundary is exercised without staging 65_536 chunks. putblob refuses a
    /// stage that would grow `refs.staging` to this many entries: that
    /// execute-side rejection is what keeps every execute-produced [`Refs`] within
    /// [`decode_refs`](crate::state::decode_refs)'s staging ceiling, so the agreed
    /// image always re-decodes on reboot and installs on a joiner.
    pub(crate) staging_entry_cap: usize,
    /// per-owner staging-table entry cap — [`MAX_STAGING_ENTRIES_PER_OWNER`] in
    /// production, lowered only by the `#[doc(hidden)]` test override so the
    /// per-owner boundary is exercised cheaply. bounds one owner's share of the
    /// global table.
    pub(crate) staging_entry_cap_per_owner: usize,
}

/// a block's staged objects — `(kind, body)` pairs the glue flushes into the
/// odb at commit. named so the block-boundary signatures stay legible.
pub type StagedObjects = Vec<(Kind, Vec<u8>)>;

/// per-block overlay: refs-next plus objects awaiting the store flush.
pub(crate) struct Pending {
    pub refs: Refs,
    pub objects: StagedObjects,
    /// per-block index over `objects` — the id of every object buffered this
    /// block. maintained in lockstep with `objects` so availability checks
    /// (commit step 6) and putblob's no-op dedup stay O(log n) instead of
    /// re-hashing every buffered megabyte on each call. a chunk uploaded inline
    /// by an earlier commit THIS block (in `objects`, not in `staging`) is thus
    /// seen as available by a later commit and no-op'd by a later putblob.
    pub object_ids: BTreeSet<ObjectId>,
    pub height: u64,
}

impl Pending {
    /// buffer an object for this block's store flush and index its id. the id is
    /// the content-addressed `object_id(kind, body)`; the index dedups so the
    /// same bytes buffered twice cost one entry (the flush is idempotent anyway).
    fn push_object(&mut self, kind: Kind, body: Vec<u8>) {
        let id = object_id(kind, &body);
        self.object_ids.insert(id);
        self.objects.push((kind, body));
    }
}

/// one watch hit produced by a commit; the glue turns each into an emitted
/// follow-up msg (task 9).
pub struct Notification {
    pub module_id: String,
    pub prefix: String,
    pub path: String,
    pub snapshot: String,
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
            grep_budget: MAX_GREP_SCAN_BYTES,
            window_cap: HISTORY_WINDOW,
            staging_entry_cap: MAX_STAGING_ENTRIES,
            staging_entry_cap_per_owner: MAX_STAGING_ENTRIES_PER_OWNER,
        }
    }

    /// `#[doc(hidden)]` test seam: shrink the per-owner staging quota so the
    /// boundary logic is exercised without staging a full gibibyte. production
    /// never calls this — the quota stays [`STAGING_QUOTA_BYTES`].
    #[doc(hidden)]
    pub fn set_staging_quota_for_tests(&mut self, quota: u64) {
        self.quota = quota;
    }

    /// `#[doc(hidden)]` test seam: shrink the per-call grep scan budget so the
    /// budget-boundary + resume-cursor logic is exercised without a multi-MiB
    /// fixture. production never calls this — the budget stays
    /// [`MAX_GREP_SCAN_BYTES`]. the read side ([`crate::queries`]) reads it via
    /// [`Fs::grep_budget`].
    #[doc(hidden)]
    pub fn set_grep_budget_for_tests(&mut self, budget: u64) {
        self.grep_budget = budget;
    }

    /// `#[doc(hidden)]` test seam: shrink the bounded history window so gc's
    /// window-expiry sweep can be driven with a few commits. production never
    /// calls this — the window stays [`HISTORY_WINDOW`].
    #[doc(hidden)]
    pub fn set_history_window_for_tests(&mut self, n: usize) {
        self.window_cap = n;
    }

    /// `#[doc(hidden)]` test seam: shrink the staging-table entry caps (global,
    /// then per-owner) so putblob's table-full and per-owner-flood boundaries are
    /// exercised in a handful of ops instead of staging tens of thousands of
    /// chunks. production never calls this — the caps stay [`MAX_STAGING_ENTRIES`]
    /// / [`MAX_STAGING_ENTRIES_PER_OWNER`].
    #[doc(hidden)]
    pub fn set_staging_entry_caps_for_tests(&mut self, global: usize, per_owner: usize) {
        self.staging_entry_cap = global;
        self.staging_entry_cap_per_owner = per_owner;
    }

    /// `#[doc(hidden)]` test seam: the gc mark set over COMMITTED refs — the
    /// reachable-object set a reachability test asserts every member of resolves
    /// after a sweep. panics on a corrupt store (a reachable object missing),
    /// which is itself the negative signal the reachability tests want.
    #[doc(hidden)]
    pub fn gc_mark_for_test(&self) -> BTreeSet<ObjectId> {
        crate::gc::mark(&self.refs, &self.store).expect("mark over a consistent store")
    }

    /// `#[doc(hidden)]` test seam: does the committed odb hold `id`?
    #[doc(hidden)]
    pub fn odb_has_for_test(&self, id: &ObjectId) -> bool {
        self.store.has(id)
    }

    /// `#[doc(hidden)]` test seam: the committed odb object count.
    #[doc(hidden)]
    pub fn odb_len_for_test(&self) -> usize {
        self.store.list().map(|ids| ids.len()).unwrap_or(0)
    }

    /// the per-call grep scan budget the read side charges each scanned file
    /// against (pre-scan, by declared size).
    pub(crate) fn grep_budget(&self) -> u64 {
        self.grep_budget
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
                object_ids: BTreeSet::new(),
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
        let object_ids = objects.iter().map(|(k, b)| object_id(*k, b)).collect();
        self.pending = Some(Pending {
            refs,
            objects,
            object_ids,
            height,
        });
    }

    /// `#[doc(hidden)]` test seam: insert a watch straight into COMMITTED refs so
    /// commit-time watch fan-out can be exercised before the watch op semantics
    /// (task 10) land. moves the committed root (watches are refs state) — that is
    /// fine for a test that only asserts the emitted notifications.
    #[doc(hidden)]
    pub fn insert_watch_for_test(&mut self, prefix: String, module_id: String) {
        self.refs.watches.insert((prefix, module_id));
    }

    /// `#[doc(hidden)]` test seam: the committed head snapshot as 64-char hex —
    /// the base a per-path-CAS test threads into a follow-up commit.
    #[doc(hidden)]
    pub fn committed_head_for_test(&self) -> Option<String> {
        self.refs.head.as_ref().map(|h| to_hex(h))
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
        // copy the entry caps out before the field borrows below, same as `quota`.
        let entry_cap = self.staging_entry_cap;
        let entry_cap_per_owner = self.staging_entry_cap_per_owner;
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
        // it, or an earlier op THIS block already buffered it — whether a prior
        // putblob (also in staging) OR a prior commit that chunked the same bytes
        // inline (in objects, NOT in staging). the per-block object index covers
        // both cases, so a putblob after an inline commit of the same chunk
        // no-ops instead of double-staging.
        if store.has(&digest) || pending.object_ids.contains(&digest) {
            return Ok(());
        }

        // the caps below gate the ADD path only — the dedup no-op above already
        // returned for an already-reachable chunk, so a full table can never block
        // re-staging a durable chunk.

        // global table cap — the load-bearing consensus-safety check. refusing to
        // grow `refs.staging` to `entry_cap` ([`MAX_STAGING_ENTRIES`]) is exactly
        // what keeps every execute-produced refs within `decode_refs`'s staging
        // ceiling, so the agreed image always re-decodes on reboot and installs on
        // a joiner. the byte quota does NOT bound the count (distinct tiny chunks
        // cost almost no quota), so the count needs its own cap here.
        if pending.refs.staging.len() >= entry_cap {
            return Err("files: staging table is full".into());
        }

        // per-owner caps over the PENDING staging view (same-block stages count):
        // the byte quota and the entry share are tallied in one pass.
        let len = bytes.len() as u64;
        let (used, owner_entries) = pending
            .refs
            .staging
            .values()
            .filter(|s| s.owner == actor)
            .fold((0u64, 0usize), |(used, n), s| {
                (used.saturating_add(s.len), n + 1)
            });
        // per-owner entry share — one owner may not monopolize the global table.
        // 4096 × 1 MiB > the byte quota, so an honest large upload trips the quota
        // first; this bites only a tiny-chunk flood.
        if owner_entries >= entry_cap_per_owner {
            return Err("files: staging entry quota exceeded".into());
        }
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
        pending.push_object(Kind::Chunk, bytes.to_vec());
        Ok(())
    }

    /// the atomic write path. validates the whole op against the base + the
    /// effective head (per-path CAS), applies every change through a single lazy
    /// [`TreeEdit`], builds the new tree + snapshot, and merges the result into
    /// this block's pending overlay — all-or-nothing: nothing touches
    /// `self.pending` until full success, so a rejected commit leaves the block
    /// exactly as it was. returns the watch notifications the glue emits.
    pub fn commit(
        &mut self,
        actor: &str,
        height: u64,
        time: u64,
        base: Option<String>,
        message: String,
        changes: Vec<Change>,
    ) -> Result<Vec<Notification>, String> {
        self.require_pending(height);
        // read the window cap before borrowing pending — commit_apply needs it to
        // bound the history window, and it is a plain Copy field.
        let window_cap = self.window_cap;
        // the scratch refs every commit mutation lands in — a clone of the pending
        // view, swept at the top. merged back into pending only on full success.
        let scratch = self
            .pending
            .as_ref()
            .expect("require_pending set it")
            .refs
            .clone();

        // all reads run over the committed odb PLUS this block's prior pending
        // objects (in-block chaining: a later commit reads a snapshot an earlier
        // commit produced this block). all writes accumulate into `built`, which
        // is discarded untouched on any reject.
        let built = {
            let pending = self.pending.as_ref().expect("require_pending set it");
            let store = Store {
                store: &self.store,
                pending: &pending.objects,
            };
            commit_apply(
                &store,
                &pending.object_ids,
                scratch,
                actor,
                height,
                time,
                base,
                message,
                &changes,
                window_cap,
            )?
        };

        // merge on success — the single mutation point.
        let pending = self.pending.as_mut().expect("require_pending set it");
        pending.refs = built.refs;
        for (kind, body) in built.objects {
            pending.push_object(kind, body);
        }
        Ok(built.notifications)
    }

    /// pin a resolvable snapshot under `name`, protecting it from gc. mutates the
    /// PENDING refs view only — the committed root moves at `commit_block` +
    /// `adopt_refs`, never here (the established discipline). validate-then-mutate:
    /// every check is a pre-check, so a reject leaves pending as the sweep left it.
    ///
    /// `height` rides the same `(actor, height, ..)` shape putblob uses: it is the
    /// block height the sweep and `require_pending` key on — there is no other
    /// source of the current height in the pure core (the refs preimage carries
    /// none), and the binding sweep-first rule below needs it.
    pub fn pin(
        &mut self,
        actor: &str,
        height: u64,
        snapshot: String,
        name: String,
    ) -> Result<(), String> {
        self.require_pending(height);
        let pending = self.pending.as_mut().expect("require_pending set it");
        // sweep-first, on the pending view, like putblob. the deterministic staging
        // sweep ticks at every mutating verb so expiry stays a pure function of the
        // op stream even in a block whose only op is a pin. a reject AFTER the sweep
        // is harmless: the kernel aborts the whole block on any execute error
        // (verified in task 9's review — the host aborts every module on any drain
        // failure), so `abort_block` erases the swept-but-rejected pending; it is
        // never observed.
        sweep_expired(&mut pending.refs, height);

        // validate fully before the single mutation (all pre-checks).
        if name.is_empty() {
            return Err("files: pin name must not be empty".into());
        }
        if name.len() > MAX_PIN_NAME_BYTES {
            return Err("files: pin name exceeds the byte cap".into());
        }
        if pending.refs.pins.len() >= MAX_PINS {
            return Err("files: pin table is full".into());
        }
        if pending.refs.pins.contains_key(&name) {
            return Err("files: pin name already exists".into());
        }
        // the id must hex-parse AND resolve in the PENDING view (head, window, or an
        // already-pinned id). a gc'd / unknown id is unpinnable — naming an
        // unreachable snapshot cannot revive it.
        let id =
            from_hex_32(&snapshot).ok_or_else(|| "files: snapshot not resolvable".to_string())?;
        if !refs_contains_snapshot(&pending.refs, &id) {
            return Err("files: snapshot not resolvable".into());
        }

        pending.refs.pins.insert(
            name,
            PinEntry {
                snapshot: id,
                owner: actor.to_string(),
            },
        );
        Ok(())
    }

    /// remove a pin by name — owner-gated: only the pin's creator or system.
    /// mutates the PENDING view only (see [`Fs::pin`] for the height/sweep rules).
    pub fn unpin(&mut self, actor: &str, height: u64, name: String) -> Result<(), String> {
        self.require_pending(height);
        let pending = self.pending.as_mut().expect("require_pending set it");
        // sweep-first (see `pin`): `abort_block` erases a swept-but-rejected pending.
        sweep_expired(&mut pending.refs, height);

        let owner = match pending.refs.pins.get(&name) {
            Some(entry) => entry.owner.clone(),
            None => return Err("files: pin not found".into()),
        };
        // owner-gated: the creator or system may remove it; nobody else.
        if actor != owner && actor != "system" {
            return Err("files: only the pin owner may unpin".into());
        }
        pending.refs.pins.remove(&name);
        Ok(())
    }

    /// register a `(prefix, module_id)` watch. origin-gated: watches are
    /// module-origin only and a module may only watch for itself; system may
    /// register for any module. mutates the PENDING view only (see [`Fs::pin`]).
    pub fn watch(
        &mut self,
        actor: &str,
        height: u64,
        is_module: bool,
        prefix: String,
        module_id: String,
    ) -> Result<(), String> {
        self.require_pending(height);
        let pending = self.pending.as_mut().expect("require_pending set it");
        // sweep-first (see `pin`): `abort_block` erases a swept-but-rejected pending.
        sweep_expired(&mut pending.refs, height);

        watch_origin_gate(actor, is_module, &module_id)?;
        if module_id.is_empty() {
            return Err("files: watch module id must not be empty".into());
        }
        if module_id.len() > MAX_WATCH_MODULE_ID_BYTES {
            return Err("files: watch module id exceeds the byte cap".into());
        }
        // canonicalize the prefix so registration and the commit fan-out key on the
        // SAME bytes, and so matching is segment-boundary (not substring): a watch on
        // "/shared" must fire for "/shared/x" but NOT for "/sharedsecret/x".
        let prefix = canonical_watch_prefix(&prefix)?;
        if pending.refs.watches.len() >= MAX_WATCHES {
            return Err("files: watch table is full".into());
        }
        let key = (prefix, module_id);
        if pending.refs.watches.contains(&key) {
            return Err("files: watch already registered".into());
        }
        pending.refs.watches.insert(key);
        Ok(())
    }

    /// remove a `(prefix, module_id)` watch — same origin gate as [`Fs::watch`].
    /// mutates the PENDING view only.
    pub fn unwatch(
        &mut self,
        actor: &str,
        height: u64,
        is_module: bool,
        prefix: String,
        module_id: String,
    ) -> Result<(), String> {
        self.require_pending(height);
        let pending = self.pending.as_mut().expect("require_pending set it");
        // sweep-first (see `pin`): `abort_block` erases a swept-but-rejected pending.
        sweep_expired(&mut pending.refs, height);

        // gate first — never leak whether a watch exists to an unauthorized caller.
        watch_origin_gate(actor, is_module, &module_id)?;
        // key on the same canonical prefix registration stored.
        let prefix = canonical_watch_prefix(&prefix)?;
        let key = (prefix, module_id);
        if !pending.refs.watches.remove(&key) {
            return Err("files: watch not found".into());
        }
        Ok(())
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

    /// committed state only — never the pending overlay. delegates to the pure
    /// read side in `queries.rs`.
    pub fn query(&self, q: FilesQuery) -> Result<FilesReply, String> {
        crate::queries::query(self, q)
    }

    /// the committed object store — the read side (`queries.rs`) opens a
    /// committed-only [`Store`] over it (no pending overlay).
    pub(crate) fn store_ref(&self) -> &S {
        &self.store
    }

    /// committed refs — the read side resolves snapshots against this view.
    pub(crate) fn refs_view(&self) -> &Refs {
        &self.refs
    }

    /// answer an off-block object fetch from the COMMITTED odb. `GetObjects`
    /// returns, in request order, a [`SyncObject`] per id: absent ids come back
    /// `present: false` (the caller keeps them queued for a later round), present
    /// ids carry the kind tag byte and the standard-base64 body. strictness is
    /// uniform — beyond [`MAX_SYNC_IDS`] rejects, and any non-hex id rejects the
    /// WHOLE request (a malformed batch is a client bug, not a per-id absence).
    pub fn serve_sync(&self, req: FilesSyncReq) -> Result<FilesSyncResp, String> {
        match req {
            FilesSyncReq::GetObjects { ids } => {
                if ids.len() > MAX_SYNC_IDS {
                    return Err("files: too many ids".into());
                }
                let mut out = Vec::with_capacity(ids.len());
                // the reply BYTE budget (see [`MAX_SYNC_REPLY_BYTES`]): a full
                // batch of 1 MiB chunks would encode ~350 MiB, far over the p2p
                // message cap the reply rides under (whose sender ASSERTS on
                // size). once the budget is spent the rest of the batch comes
                // back "absent" — the possession driver re-requests whatever is
                // still missing next round, so a truncated page is progress,
                // never a lie about possession. the FIRST present object is
                // served unconditionally: every round then lands at least one
                // object, keeping the driver's anti-livelock invariant
                // ("landed == 0" means pruned) intact.
                let mut spent = 0usize;
                let mut served = 0usize;
                for hex in &ids {
                    // one bad id rejects the batch — the same all-or-nothing
                    // strictness the object/refs codecs use, so a caller can never
                    // silently drop a mistyped id as a phantom "absent".
                    let id =
                        from_hex_32(hex).ok_or_else(|| "files: sync id is not hex".to_string())?;
                    // re-render the id so the reply is canonical lowercase hex
                    // regardless of how the request framed it.
                    let absent = SyncObject {
                        id: to_hex(&id),
                        present: false,
                        kind: 0,
                        b64: String::new(),
                    };
                    match self.store.get(&id)? {
                        Some((kind, body)) => {
                            let b64 = STANDARD.encode(&body);
                            // hex id + base64 body + the fixed json fields.
                            let cost = 64 + b64.len() + 48;
                            if served > 0 && spent + cost > MAX_SYNC_REPLY_BYTES {
                                out.push(absent);
                                continue;
                            }
                            spent += cost;
                            served += 1;
                            out.push(SyncObject {
                                id: to_hex(&id),
                                present: true,
                                kind: kind.tag(),
                                b64,
                            });
                        }
                        None => out.push(absent),
                    }
                }
                Ok(FilesSyncResp::Objects(out))
            }
            // the refs image is the `root()` preimage — served over the same
            // resolver lane so a duckfs-odb joiner installs refs then walks
            // `missing_objects` without ever touching the snapshot/chunk lane.
            FilesSyncReq::GetRefs => Ok(FilesSyncResp::Refs {
                b64: STANDARD.encode(self.snapshot_refs()),
            }),
        }
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

    /// the ids of up to `limit` objects reachable from the committed gc roots but
    /// NOT yet in the odb, in deterministic (sorted) order. it is the exact gc
    /// reachability walk (head/window/pins/staging), but it COLLECTS absent ids
    /// instead of erroring on them — that is the point of the self-heal lane.
    ///
    /// children of a missing parent are undiscoverable this round (their ids live
    /// inside the parent's not-yet-fetched body), so the caller iterates: install
    /// refs, then loop { missing -> GetObjects -> ingest } until this returns
    /// empty. each round fetches at least the newly-revealed layer, so the store
    /// strictly grows and the loop cannot livelock. a present-but-corrupt object
    /// is NOT absence — it surfaces as an Err (the same signal gc's mark raises),
    /// never as a phantom "missing".
    pub fn missing_objects(&self, limit: usize) -> Result<Vec<ObjectId>, String> {
        let missing = crate::gc::collect_missing(&self.refs, &self.store)?;
        // sorted (BTreeSet) then truncated: two calls over the same state return
        // the identical prefix, so the caller never livelocks on order churn.
        Ok(missing.into_iter().take(limit).collect())
    }

    /// verify-then-store one fetched object. the id must re-derive from the bytes
    /// (`object_id(kind, body) == *id`) or the object is rejected — the
    /// dishonest-server rule: a peer cannot smuggle bytes under an id they do not
    /// hash to. a `File` object's declared size/chunk-COUNT shape is validated
    /// here too (fix 2a): content-addressing alone does NOT capture the
    /// size/chunk-count invariant, so a self-consistent FileObj could otherwise
    /// claim a size its chunk list cannot cover. chunk LENGTHS stay unvalidated
    /// here (the chunks may not have arrived yet) — the read side closes that hole
    /// per-chunk. pure: the caller (glue) fsyncs the batch for durability.
    pub fn ingest_object(&mut self, id: &ObjectId, kind: u8, body: &[u8]) -> Result<(), String> {
        let kind = Kind::from_u8(kind).ok_or_else(|| "files: ingest unknown kind".to_string())?;
        if object_id(kind, body) != *id {
            return Err("files: object id mismatch".into());
        }
        if kind == Kind::File {
            let file = FileObj::decode(body)?;
            verify_file_shape(file.size, file.chunks.len())
                .map_err(|_| "ingest: file object size/chunk shape invalid".to_string())?;
        }
        self.store.put(kind, body)?;
        Ok(())
    }

    /// mark + sweep over COMMITTED state now; the CALLER (the glue) decides WHEN
    /// via the watermark trigger. returns the number of objects removed.
    ///
    /// consensus-neutral: `gc::mark` reads only committed refs and the
    /// sweep removes only unreachable objects from the store — `self.refs`, hence
    /// the module root, is NEVER touched, so gc cannot move the root and every
    /// node converges to the same object set whenever it happens to run.
    ///
    /// all-or-nothing on corruption: if any reachable object is missing, mark
    /// returns Err and this removes NOTHING (a partial mark must never drive a
    /// sweep — that would delete live-but-unreadable data).
    pub fn gc(&mut self) -> Result<u64, String> {
        let live = crate::gc::mark(&self.refs, &self.store)?;
        // list() is owned, so its immutable borrow ends before the removes below;
        // remove every stored id the mark did not reach.
        let mut removed = 0u64;
        for id in self.store.list()? {
            if !live.contains(&id) {
                self.store.remove(&id)?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

// ---- commit internals -------------------------------------------------------

/// whether `id` names a snapshot resolvable in `refs`: the head, anywhere in the
/// bounded history window, or a pinned snapshot. shared by base resolution
/// (commit) and snapshot resolution (the stat query) — same membership rule.
pub(crate) fn refs_contains_snapshot(refs: &Refs, id: &ObjectId) -> bool {
    refs.head.as_ref() == Some(id)
        || refs.window.contains(id)
        || refs.pins.values().any(|p| &p.snapshot == id)
}

/// the fully-built result of a successful commit validation+apply pass, held in
/// locals until the single merge into pending (all-or-nothing).
struct CommitBuilt {
    refs: Refs,
    objects: StagedObjects,
    notifications: Vec<Notification>,
}

/// one planned tree edit: validated up front, replayed onto the effective head
/// in apply order. `Put` also carries the symlink case (with `EntryKind::Symlink`).
enum EditOp {
    Put { segs: Vec<String>, entry: TreeEntry },
    Mkdir { segs: Vec<String> },
    Rm { segs: Vec<String> },
    Mv { from: Vec<String>, to: Vec<String> },
}

/// the pure commit engine — steps 1-11 of the brief over a read `store`, this
/// block's prior-object index `pending_ids`, and a swept scratch `refs`. every
/// write accumulates into locals (`objects`, the returned refs), so the caller
/// discards the whole thing untouched on any reject: all-or-nothing by
/// construction — `self.pending` is never touched here.
#[allow(clippy::too_many_arguments)]
fn commit_apply(
    store: &Store,
    pending_ids: &BTreeSet<ObjectId>,
    mut refs: Refs,
    actor: &str,
    height: u64,
    time: u64,
    base: Option<String>,
    message: String,
    changes: &[Change],
    window_cap: usize,
) -> Result<CommitBuilt, String> {
    // step 0: the deterministic staging sweep, over the scratch. a reject never
    // persists a half-applied sweep — production aborts the whole block, and an
    // idempotent later op re-sweeps if the block continues.
    sweep_expired(&mut refs, height);

    // step 1: message + change-count bounds.
    if message.len() > MAX_MESSAGE_BYTES {
        return Err("files: commit message exceeds the byte cap".into());
    }
    if changes.is_empty() {
        return Err("files: commit must carry at least one change".into());
    }
    if changes.len() > MAX_CHANGES_PER_COMMIT {
        return Err("files: commit exceeds the change cap".into());
    }

    // step 2: resolve base -> its root tree. None = the empty tree (first commit /
    // create-only). Some(hex) must resolve in the PENDING refs view (in-block
    // chaining can base onto a snapshot committed earlier this same block).
    let base_root: Option<ObjectId> = match &base {
        None => None,
        Some(hex) => {
            let id = from_hex_32(hex)
                .ok_or_else(|| "files: base snapshot not resolvable".to_string())?;
            if !refs_contains_snapshot(&refs, &id) {
                return Err("files: base snapshot not resolvable".into());
            }
            Some(snapshot_root_tree(store, &id)?)
        }
    };

    // effective head = the pending refs head (in-block chaining): the CAS-compare
    // and apply target, and the parent of the new snapshot.
    let effective_head: Option<ObjectId> = refs.head;
    let effective_root: Option<ObjectId> = match effective_head {
        Some(h) => Some(snapshot_root_tree(store, &h)?),
        None => None,
    };

    // steps 3-6: validate + plan every change. new objects accumulate in
    // `objects`/`staged_ids`; `touched` (canonical joined path + segments) drives
    // dedup, CAS and watch fan-out; `chunk_refs` drives staged-quota reclaim.
    let mut objects: StagedObjects = Vec::new();
    let mut staged_ids: BTreeSet<ObjectId> = BTreeSet::new();
    let mut plan: Vec<EditOp> = Vec::with_capacity(changes.len());
    let mut touched: Vec<(String, Vec<String>)> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut chunk_refs: Vec<ObjectId> = Vec::new();
    let mut chunks_to_check: Vec<ObjectId> = Vec::new();
    let mut inline_bytes: usize = 0;

    for change in changes {
        match change {
            Change::Put {
                path,
                exec,
                meta,
                content,
            } => {
                // step 3: canonicalize + authority-check the written path.
                let segs = canon_authorized(actor, path)?;
                let joined = join_segs(&segs);
                dedup(&mut seen, &joined)?; // step 4
                validate_meta(meta)?; // step 8 meta caps, enforced before staging
                let (fileobj_id, chunk_ids, size) = match content {
                    Content::Inline { b64 } => {
                        // step 5: strict base64, budget-summed.
                        let bytes = STANDARD
                            .decode(b64.as_bytes())
                            .map_err(|_| "files: inline content is not valid base64".to_string())?;
                        inline_bytes = inline_bytes
                            .checked_add(bytes.len())
                            .ok_or_else(|| "files: inline commit budget overflows".to_string())?;
                        if inline_bytes > MAX_INLINE_COMMIT_BYTES {
                            return Err("files: inline commit budget exceeded".into());
                        }
                        let chunk_ids =
                            chunk_bytes(&bytes, store, pending_ids, &mut objects, &mut staged_ids);
                        let fileobj_id = stage_fileobj(
                            bytes.len() as u64,
                            &chunk_ids,
                            meta,
                            store,
                            pending_ids,
                            &mut objects,
                            &mut staged_ids,
                        );
                        (fileobj_id, chunk_ids, bytes.len() as u64)
                    }
                    Content::Chunks { size, chunks } => {
                        // step 6: size/chunk consistency + hex parse; availability
                        // is checked below once all inline chunks are known.
                        let ids = validate_chunks(*size, chunks)?;
                        chunks_to_check.extend_from_slice(&ids);
                        let fileobj_id = stage_fileobj(
                            *size,
                            &ids,
                            meta,
                            store,
                            pending_ids,
                            &mut objects,
                            &mut staged_ids,
                        );
                        (fileobj_id, ids, *size)
                    }
                };
                chunk_refs.extend(chunk_ids);
                touched.push((joined, segs.clone()));
                plan.push(EditOp::Put {
                    segs,
                    entry: TreeEntry {
                        kind: EntryKind::File,
                        id: fileobj_id,
                        exec: *exec,
                        size,
                    },
                });
            }
            Change::Mkdir { path } => {
                let segs = canon_authorized(actor, path)?;
                let joined = join_segs(&segs);
                dedup(&mut seen, &joined)?;
                touched.push((joined, segs.clone()));
                plan.push(EditOp::Mkdir { segs });
            }
            Change::Rm { path } => {
                let segs = canon_authorized(actor, path)?;
                let joined = join_segs(&segs);
                dedup(&mut seen, &joined)?;
                touched.push((joined, segs.clone()));
                plan.push(EditOp::Rm { segs });
            }
            Change::Mv { from, to } => {
                // both endpoints are written paths: canonicalized, authority-checked
                // and dedup'd, and both feed CAS + watch fan-out.
                let from_segs = canon_authorized(actor, from)?;
                let to_segs = canon_authorized(actor, to)?;
                let from_joined = join_segs(&from_segs);
                let to_joined = join_segs(&to_segs);
                dedup(&mut seen, &from_joined)?;
                dedup(&mut seen, &to_joined)?;
                touched.push((from_joined, from_segs.clone()));
                touched.push((to_joined, to_segs.clone()));
                plan.push(EditOp::Mv {
                    from: from_segs,
                    to: to_segs,
                });
            }
            Change::Symlink { path, target } => {
                let segs = canon_authorized(actor, path)?;
                let joined = join_segs(&segs);
                dedup(&mut seen, &joined)?;
                if target.len() > MAX_SYMLINK_TARGET_BYTES {
                    return Err("files: symlink target exceeds the byte cap".into());
                }
                // one chunk holds the target bytes; the FileObj points at it with
                // the symlink's entry kind and size = target length.
                let chunk_id = stage_object(
                    Kind::Chunk,
                    target.as_bytes().to_vec(),
                    store,
                    pending_ids,
                    &mut objects,
                    &mut staged_ids,
                );
                let fileobj = FileObj {
                    size: target.len() as u64,
                    chunks: vec![chunk_id],
                    meta: BTreeMap::new(),
                };
                let fileobj_id = stage_object(
                    Kind::File,
                    fileobj.encode(),
                    store,
                    pending_ids,
                    &mut objects,
                    &mut staged_ids,
                );
                chunk_refs.push(chunk_id);
                touched.push((joined, segs.clone()));
                plan.push(EditOp::Put {
                    segs,
                    entry: TreeEntry {
                        kind: EntryKind::Symlink,
                        id: fileobj_id,
                        exec: false,
                        size: target.len() as u64,
                    },
                });
            }
        }
    }

    // step 6 (availability): every referenced chunk must be reachable — staged
    // via putblob, durable in the odb, produced by a prior commit this block, or
    // produced inline by THIS commit. checked after the plan pass so an inline
    // chunk in a later change is visible to an earlier `Chunks` reference.
    for id in &chunks_to_check {
        let available = refs.staging.contains_key(id)
            || pending_ids.contains(id)
            || staged_ids.contains(id)
            || store.store.has(id);
        if !available {
            return Err("files: chunk not available".into());
        }
    }

    // step 7 (per-path CAS): every touched path must be byte-identical between the
    // base tree and the effective head tree (full `TreeEntry` compare), else a
    // concurrent change moved it since base — reject the whole commit.
    for (joined, segs) in &touched {
        if entry_at(store, base_root, segs)? != entry_at(store, effective_root, segs)? {
            return Err(format!("files: conflict: {joined} changed since base"));
        }
    }

    // step 8 (apply): replay the plan onto ONE lazy edit over the effective head —
    // intra-commit directory reads ride the overlay, so the store view (committed
    // odb + prior-block pending) is enough.
    let mut edit = TreeEdit::load(store, effective_root);
    for op in &plan {
        match op {
            EditOp::Put { segs, entry } => edit.put(store, segs, *entry)?,
            EditOp::Mkdir { segs } => edit.mkdir(store, segs)?,
            EditOp::Rm { segs } => edit.rm(store, segs)?,
            EditOp::Mv { from, to } => edit.mv(store, from, to)?,
        }
    }

    // step 9 (build + snapshot): finalize the tree — a fully-empty filesystem
    // still needs a hashable root, so stage the canonical empty tree for it.
    let root = match edit.build(&mut objects)? {
        Some(id) => id,
        None => {
            let body = TreeObj {
                entries: BTreeMap::new(),
            }
            .encode();
            let id = object_id(Kind::Tree, &body);
            if !staged_ids.contains(&id) && !pending_ids.contains(&id) && !store.store.has(&id) {
                objects.push((Kind::Tree, body));
                staged_ids.insert(id);
            }
            id
        }
    };
    let snapshot = SnapshotObj {
        root,
        parent: effective_head,
        author: actor.to_string(),
        consensus_time: time,
        height,
        message,
    };
    let snap_body = snapshot.encode();
    let snap_id = object_id(Kind::Snapshot, &snap_body);
    objects.push((Kind::Snapshot, snap_body));

    // step 10 (refs): advance head, push into the bounded window, and reclaim
    // staged quota for every referenced chunk that WAS staged — its bytes are now
    // tree-reachable. a chunk referenced straight from the odb was never staged,
    // so it reclaims nothing.
    refs.head = Some(snap_id);
    refs.window.push_back(snap_id);
    // bound the window at the (test-overridable) cap — evicted snapshots fall out
    // of the gc root set, so their exclusive objects become sweepable.
    while refs.window.len() > window_cap {
        refs.window.pop_front();
    }
    for id in &chunk_refs {
        refs.staging.remove(id);
    }

    // step 11 (watch fan-out): one notification per (touched path, watch) where
    // the path is under the watch prefix. deterministic: touched paths in change
    // order, watches in the refs BTreeSet order.
    let snapshot_hex = to_hex(&snap_id);
    let mut notifications = Vec::new();
    for (joined, _segs) in &touched {
        for (prefix, module_id) in &refs.watches {
            if watch_matches(prefix, joined) {
                notifications.push(Notification {
                    module_id: module_id.clone(),
                    prefix: prefix.clone(),
                    path: joined.clone(),
                    snapshot: snapshot_hex.clone(),
                });
            }
        }
    }

    Ok(CommitBuilt {
        refs,
        objects,
        notifications,
    })
}

/// canonicalize a written path and authority-check it for `actor`.
fn canon_authorized(actor: &str, path: &str) -> Result<Vec<String>, String> {
    let segs = canonical(path)?;
    check_authority(actor, &segs)?;
    Ok(segs)
}

/// the canonical joined form of a path's segments — the CAS/dedup/watch key.
fn join_segs(segs: &[String]) -> String {
    format!("/{}", segs.join("/"))
}

/// the watch origin gate, shared by [`Fs::watch`]/[`Fs::unwatch`]: watches are
/// module-origin only (external submitters cannot register), and a module may act
/// only for itself. system (also `is_module`) may act for any module_id — it is
/// the arbitrary-authority origin. one function so both ends enforce it identically.
fn watch_origin_gate(actor: &str, is_module: bool, module_id: &str) -> Result<(), String> {
    if !is_module {
        return Err("files: watch registration is module-origin only".into());
    }
    if actor != "system" && actor != module_id {
        return Err("files: a module may only watch for itself".into());
    }
    Ok(())
}

/// canonicalize a watch prefix to its stored joined form, run at BOTH ends
/// (registration + removal) so the two key on identical bytes. `canonical`
/// enforces absolute + NFC + the path byte cap and rejects empty / dot / trailing
/// segments, so a stored prefix is always a clean segment path ("/" for the
/// everything-watch, "/a/b" otherwise) — which is exactly what makes the commit
/// fan-out's segment-boundary test exact rather than a substring match.
fn canonical_watch_prefix(prefix: &str) -> Result<String, String> {
    let segs = canonical(prefix)?;
    Ok(join_segs(&segs))
}

/// segment-boundary watch match: a watch prefix `p` matches path `q` iff `p` is
/// the everything-watch "/", or `q == p`, or `q` descends from `p` across a "/"
/// boundary. so "/shared" fires for "/shared/x" but NOT for "/sharedsecret/x" —
/// the binding fix from task 9's review (the old `starts_with` leaked across the
/// boundary). stored prefixes are canonical (no trailing slash), so the byte at
/// `p.len()` is always the delimiter, never part of a segment name.
fn watch_matches(prefix: &str, path: &str) -> bool {
    if prefix == "/" {
        return true;
    }
    if path == prefix {
        return true;
    }
    path.starts_with(prefix) && path.as_bytes().get(prefix.len()) == Some(&b'/')
}

/// record a touched path for dedup; a second touch of the same path rejects
/// (order-independence for CAS and apply). Mv touches two paths.
fn dedup(seen: &mut BTreeSet<String>, joined: &str) -> Result<(), String> {
    if !seen.insert(joined.to_string()) {
        return Err("files: duplicate path in commit".into());
    }
    Ok(())
}

/// enforce the meta caps BEFORE any object is staged — reject, never truncate
/// (the objects codec would also reject at decode, but we fail early and loudly).
fn validate_meta(meta: &BTreeMap<String, String>) -> Result<(), String> {
    if meta.len() > MAX_META_ENTRIES {
        return Err("files: commit meta entry count over cap".into());
    }
    for (key, value) in meta {
        if key.len() > MAX_META_KEY_BYTES {
            return Err("files: commit meta key over cap".into());
        }
        if value.len() > MAX_META_VALUE_BYTES {
            return Err("files: commit meta value over cap".into());
        }
    }
    Ok(())
}

/// validate a `Content::Chunks` size/list pair and hex-parse every digest. size 0
/// requires an EMPTY chunk list (empty files are legal in duckfs); otherwise the
/// list length is pinned to ceil(size / CHUNK_SIZE) by checked span bounds
/// `(n-1)*CHUNK_SIZE < size <= n*CHUNK_SIZE`.
fn validate_chunks(size: u64, chunks: &[String]) -> Result<Vec<ObjectId>, String> {
    if chunks.len() > MAX_CHUNKS_PER_FILE {
        return Err("files: file chunk count over cap".into());
    }
    // the size/chunk-count invariant is shared with sync ingest, so it lives in
    // one place ([`verify_file_shape`]) rather than being duplicated here.
    verify_file_shape(size, chunks.len())?;
    chunks
        .iter()
        .map(|hex| {
            from_hex_32(hex).ok_or_else(|| "files: chunk digest is not valid hex".to_string())
        })
        .collect()
}

/// chunk `bytes` at CHUNK_SIZE boundaries into staged chunk objects, returning the
/// ordered chunk ids. an empty file yields NO chunks — the FileObj carries size 0
/// and an empty chunk list (empty files are legal).
fn chunk_bytes(
    bytes: &[u8],
    store: &Store,
    pending_ids: &BTreeSet<ObjectId>,
    objects: &mut StagedObjects,
    staged_ids: &mut BTreeSet<ObjectId>,
) -> Vec<ObjectId> {
    if bytes.is_empty() {
        return Vec::new();
    }
    bytes
        .chunks(CHUNK_SIZE as usize)
        .map(|c| {
            stage_object(
                Kind::Chunk,
                c.to_vec(),
                store,
                pending_ids,
                objects,
                staged_ids,
            )
        })
        .collect()
}

/// build + stage a FileObj over `chunks`, returning its id.
fn stage_fileobj(
    size: u64,
    chunks: &[ObjectId],
    meta: &BTreeMap<String, String>,
    store: &Store,
    pending_ids: &BTreeSet<ObjectId>,
    objects: &mut StagedObjects,
    staged_ids: &mut BTreeSet<ObjectId>,
) -> ObjectId {
    let fileobj = FileObj {
        size,
        chunks: chunks.to_vec(),
        meta: meta.clone(),
    };
    stage_object(
        Kind::File,
        fileobj.encode(),
        store,
        pending_ids,
        objects,
        staged_ids,
    )
}

/// stage a content-addressed object into the local buffer, returning its id.
/// skips the push when the bytes are already reachable (odb, a prior block-pending
/// object, or already staged by this commit) — so an inline chunk that matches a
/// putblob'd or prior-committed chunk is never buffered twice. its quota is still
/// reclaimed by the caller via `chunk_refs`.
fn stage_object(
    kind: Kind,
    body: Vec<u8>,
    store: &Store,
    pending_ids: &BTreeSet<ObjectId>,
    objects: &mut StagedObjects,
    staged_ids: &mut BTreeSet<ObjectId>,
) -> ObjectId {
    let id = object_id(kind, &body);
    if !staged_ids.contains(&id) && !pending_ids.contains(&id) && !store.store.has(&id) {
        objects.push((kind, body));
        staged_ids.insert(id);
    }
    id
}
