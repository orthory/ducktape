//! deterministic in-memory `memory` module — a shared agent workspace shaped
//! like a filesystem (the "NoKV" philosophy).
//!
//! like `tasks`, this is a state-based (not qmdb-backed) module: it stages all
//! mutations into a pending working copy during `execute`, publishes them at
//! `commit_block`, discards them at `abort_block`, and computes `root()` as a
//! sha256 over a canonical byte encoding of the COMMITTED state.
//! `snapshot()`/`install()` use that exact preimage so a joiner can verify a
//! peer image against the expected root before adopting it.
//!
//! ## namespace model
//!
//! the namespace is a set of absolute `/`-separated file paths. directories are
//! purely implicit — a file at `/a/b/c` implies dirs `/a` and `/a/b`; there is
//! no `mkdir`. every file is a stack of immutable [`Generation`]s; a publish
//! appends generation `latest + 1` (1 for a brand-new file). a `(path,
//! generation)` pair is a stable, hash-pinned reference.
//!
//! ## snapshots & the retention mechanism (design decision)
//!
//! `Snapshot { name }` pins the CURRENT `path -> latest generation` mapping of
//! the whole namespace — it stores the mapping only, never a copy of the bodies
//! ("a copy of nothing"). generation records live in one shared store keyed by
//! `(path, generation)`. `Delete` removes a file from the live index but RETAINS
//! any of its generation records that a snapshot still pins; a generation record
//! is dropped exactly when it is referenced by neither a live file nor any
//! snapshot. this "recomputed reference" GC (a mark-and-keep, evaluated on demand
//! rather than as a stored counter) runs on `Delete` and `DropSnapshot`, so a
//! snapshot read survives deletion and `DropSnapshot` releases the retained data.
//!
//! generation numbers are strictly increasing per path and NEVER reused while
//! any record for that path survives: a new publish takes `1 + max existing
//! generation for the path` (across live + retained), so a re-created path can
//! never collide with a still-pinned generation of its deleted predecessor. only
//! once a path is fully forgotten (deleted with nothing retained) does a fresh
//! publish restart at generation 1.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use memory_interface::{
    FileStat, Generation, GrepHit, LsEntry, MAX_BODY_BYTES, MAX_FILES, MAX_GENERATIONS_PER_PATH,
    MAX_GREP_LINE_BYTES, MAX_META_ENTRIES, MAX_META_KEY_BYTES, MAX_META_VALUE_BYTES,
    MAX_MODULE_ID_BYTES, MAX_PATH_BYTES, MAX_QUERY_LIMIT, MAX_SEGMENT_BYTES,
    MAX_SNAPSHOT_NAME_BYTES, MAX_SNAPSHOTS, MAX_WATCHES, MemoryEvent, MemoryMsg, MemoryQuery,
    MemoryReply, Meta, decode_msg, decode_query, encode_event, encode_reply,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};

/// the head of a live file: the contiguous generation range it currently owns.
/// `first` is where the current incarnation of the path started (1 for a fresh
/// file; higher if the path was re-created above still-pinned generations).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LiveHead {
    first: u64,
    latest: u64,
}

/// the committed (or staged) state. cloning this is how a block stages: the
/// pending copy is mutated during `execute`; `commit_block` promotes it and
/// `abort_block` drops it, leaving `root()` byte-identical.
#[derive(Clone, Default)]
struct Store {
    /// live index: path -> the current file's generation range.
    live: BTreeMap<String, LiveHead>,
    /// every retained generation record, keyed by `(path, generation)`.
    gens: BTreeMap<(String, u64), Generation>,
    /// pinned snapshots: name -> (path -> pinned generation).
    snapshots: BTreeMap<String, BTreeMap<String, u64>>,
    /// registered watches: a `(prefix, module_id)` set.
    watches: BTreeSet<(String, String)>,
}

pub struct Memory {
    id: ModuleId,
    /// what `root()` / `query` observe.
    committed: Store,
    /// the per-block working copy; `None` until the block's first mutation.
    pending: Option<Store>,
}

impl Memory {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            committed: Store::default(),
            pending: None,
        }
    }

    /// the staged working copy — cloned from committed on the block's first write.
    fn store_mut(&mut self) -> &mut Store {
        if self.pending.is_none() {
            self.pending = Some(self.committed.clone());
        }
        self.pending.as_mut().expect("just populated")
    }

    // ---- mutations (staged) ------------------------------------------------

    fn publish(
        &mut self,
        ctx: &mut dyn Ctx,
        author: String,
        height: u64,
        path: String,
        body: String,
        meta: Meta,
    ) -> Result<(), Error> {
        let path = validate_file_path(&path)?;
        // caps enforced BEFORE staging, with rejection: an oversized value must
        // never enter the root preimage (the poison-value lesson).
        if body.len() > MAX_BODY_BYTES {
            return Err(Error::Module("body exceeds 64 KiB cap".into()));
        }
        validate_meta(&meta)?;

        let (generation, targets) = {
            let store = self.store_mut();
            let generation = match store.live.get(&path) {
                Some(head) => {
                    if head.latest - head.first + 1 >= MAX_GENERATIONS_PER_PATH {
                        return Err(Error::Module(format!("generation cap reached: {path}")));
                    }
                    head.latest + 1
                }
                None => {
                    if store.live.len() >= MAX_FILES {
                        return Err(Error::Module("file cap reached".into()));
                    }
                    store.next_generation(&path)
                }
            };
            store.gens.insert(
                (path.clone(), generation),
                Generation {
                    generation,
                    body,
                    meta: meta.clone(),
                    author: author.clone(),
                    published_at_height: height,
                },
            );
            match store.live.get_mut(&path) {
                Some(head) => head.latest = generation,
                None => {
                    store.live.insert(
                        path.clone(),
                        LiveHead {
                            first: generation,
                            latest: generation,
                        },
                    );
                }
            }
            // one follow-up per matching WATCHER MODULE (deduped across
            // overlapping prefixes), in sorted order.
            let mut targets: BTreeSet<String> = BTreeSet::new();
            for (prefix, module_id) in &store.watches {
                if watch_matches(prefix, &path) {
                    targets.insert(module_id.clone());
                }
            }
            (generation, targets)
        };

        // watch fan-out (P2): each notification commits or aborts atomically with
        // the publish, and counts toward the block's dispatch budget.
        if !targets.is_empty() {
            let payload = encode_event(&MemoryEvent::Published {
                path,
                generation,
                meta,
                author,
            });
            for module_id in targets {
                ctx.emit_msg(Msg {
                    target: module_id,
                    payload: payload.clone(),
                });
            }
        }
        Ok(())
    }

    fn delete(&mut self, path: String) -> Result<(), Error> {
        let path = validate_file_path(&path)?;
        let store = self.store_mut();
        if store.live.remove(&path).is_none() {
            return Err(Error::Module(format!("file not found: {path}")));
        }
        // drop every generation of this path no snapshot still pins.
        let victims: Vec<u64> = store
            .generations_of(&path)
            .filter(|g| !store.is_referenced(&path, *g))
            .collect();
        for g in victims {
            store.gens.remove(&(path.clone(), g));
        }
        Ok(())
    }

    fn create_snapshot(&mut self, name: String) -> Result<(), Error> {
        if name.is_empty() {
            return Err(Error::Module("snapshot name must not be empty".into()));
        }
        if name.len() > MAX_SNAPSHOT_NAME_BYTES {
            return Err(Error::Module("snapshot name exceeds byte cap".into()));
        }
        let store = self.store_mut();
        if store.snapshots.contains_key(&name) {
            return Err(Error::Module(format!("snapshot already exists: {name}")));
        }
        if store.snapshots.len() >= MAX_SNAPSHOTS {
            return Err(Error::Module("snapshot cap reached".into()));
        }
        let pins: BTreeMap<String, u64> = store
            .live
            .iter()
            .map(|(path, head)| (path.clone(), head.latest))
            .collect();
        store.snapshots.insert(name, pins);
        Ok(())
    }

    fn drop_snapshot(&mut self, name: String) -> Result<(), Error> {
        let store = self.store_mut();
        let pins = store
            .snapshots
            .remove(&name)
            .ok_or_else(|| Error::Module(format!("snapshot not found: {name}")))?;
        // a generation pinned ONLY by the dropped snapshot is now unreferenced.
        let victims: Vec<(String, u64)> = pins
            .into_iter()
            .filter(|(path, g)| !store.is_referenced(path, *g))
            .collect();
        for key in victims {
            store.gens.remove(&key);
        }
        Ok(())
    }

    fn register_watch(
        &mut self,
        ctx: &mut dyn Ctx,
        prefix: String,
        module_id: String,
    ) -> Result<(), Error> {
        validate_prefix(&prefix)?;
        if module_id.is_empty() {
            return Err(Error::Module("watch module_id must not be empty".into()));
        }
        if module_id.len() > MAX_MODULE_ID_BYTES {
            return Err(Error::Module("watch module_id exceeds byte cap".into()));
        }
        // a dead follow-up would fail the whole block, so gate the target the way
        // chat gates its channel hooks: known module, and never self.
        if module_id == ctx.env().me {
            return Err(Error::Module("a module cannot watch itself".into()));
        }
        if ctx.module_root(&module_id).is_none() {
            return Err(Error::Module(format!("unknown watch target: {module_id}")));
        }
        let store = self.store_mut();
        let key = (prefix, module_id);
        if store.watches.contains(&key) {
            return Ok(()); // idempotent
        }
        if store.watches.len() >= MAX_WATCHES {
            return Err(Error::Module("watch cap reached".into()));
        }
        store.watches.insert(key);
        Ok(())
    }

    fn unregister_watch(&mut self, prefix: String, module_id: String) -> Result<(), Error> {
        // absent watch = deterministic no-op (chat's unregister-hook semantics).
        self.store_mut().watches.remove(&(prefix, module_id));
        Ok(())
    }

    // ---- root / snapshot / install -----------------------------------------

    fn root_of(store: &Store) -> StateRoot {
        let mut h = Sha256::new();
        h.update(store.encode());
        StateRoot(h.finalize().into())
    }

    /// the exact `root()` preimage — the self-contained bytes a joiner installs.
    pub fn snapshot(&self) -> Vec<u8> {
        self.committed.encode()
    }

    /// adopt a peer image only after verifying it against `expected` (the
    /// consensus-committed root).
    ///
    /// the hash authenticates the BYTES, not the decoded state: `snapshot()` is
    /// the exact root preimage, so honest bytes hash to the committed root
    /// directly, and any non-canonical re-encoding (e.g. duplicate-key sections
    /// a lenient decode would collapse via insert-overwrite) is rejected before
    /// it is ever parsed. the strict [`Store::decode`] behind it rejects
    /// execute-unreachable states outright, so even a colluding root recomputed
    /// from evil bytes cannot smuggle one in.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let mut h = Sha256::new();
        h.update(bytes);
        if StateRoot(h.finalize().into()) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        self.committed = Store::decode(bytes)?;
        self.pending = None;
        Ok(())
    }
}

impl Store {
    /// every generation number currently held for `path`, ascending.
    fn generations_of<'a>(&'a self, path: &'a str) -> impl Iterator<Item = u64> + 'a {
        self.gens
            .range((path.to_string(), u64::MIN)..=(path.to_string(), u64::MAX))
            .map(|((_, g), _)| *g)
    }

    /// the next generation to assign — strictly above any existing (live OR
    /// retained) generation for the path, so a re-created path never collides
    /// with a still-pinned generation of its predecessor.
    fn next_generation(&self, path: &str) -> u64 {
        self.gens
            .range((path.to_string(), u64::MIN)..=(path.to_string(), u64::MAX))
            .next_back()
            .map_or(1, |((_, g), _)| g + 1)
    }

    /// is `(path, g)` still referenced by the live file or any snapshot?
    fn is_referenced(&self, path: &str, g: u64) -> bool {
        if let Some(head) = self.live.get(path)
            && head.first <= g
            && g <= head.latest
        {
            return true;
        }
        self.snapshots
            .values()
            .any(|pins| pins.get(path) == Some(&g))
    }

    /// the [`FileStat`] of a live head, or `None` if its latest record is
    /// missing. strict decode makes that state unreachable, but a query path
    /// must NEVER panic (a panic here is a validator crash), so the miss
    /// degrades to "not visible" instead.
    fn stat_of(&self, path: &str, head: &LiveHead) -> Option<FileStat> {
        let latest = self.gens.get(&(path.to_string(), head.latest))?;
        Some(FileStat {
            path: path.to_string(),
            latest_generation: head.latest,
            generations: head.latest - head.first + 1,
            latest_meta: latest.meta.clone(),
            latest_author: latest.author.clone(),
            latest_published_at_height: latest.published_at_height,
            body_len: latest.body.len() as u64,
        })
    }

    // ---- read verbs (all against this committed store) ---------------------

    fn ls(&self, path: &str, limit: u64) -> Result<Vec<LsEntry>, Error> {
        let dir = validate_query_path(path)?;
        let depth = dir.len();
        let limit = limit.min(MAX_QUERY_LIMIT) as usize;
        // File wins over Dir on a colliding child path (a concrete file shadows
        // the implied directory in the listing; the dir is still Ls-able directly).
        let mut entries: BTreeMap<String, LsEntry> = BTreeMap::new();
        let mut dirs: Vec<String> = Vec::new();
        for (fpath, head) in &self.live {
            let fseg = split_segments(fpath);
            if fseg.len() <= depth || fseg[..depth] != dir[..] {
                continue;
            }
            if fseg.len() == depth + 1 {
                let Some(stat) = self.stat_of(fpath, head) else {
                    continue;
                };
                entries.insert(fpath.clone(), LsEntry::File(stat));
            } else {
                dirs.push(canonical_path(&fseg[..depth + 1]));
            }
        }
        for dir_path in dirs {
            entries
                .entry(dir_path.clone())
                .or_insert(LsEntry::Dir { path: dir_path });
        }
        Ok(entries.into_values().take(limit).collect())
    }

    fn stat(&self, path: &str) -> Result<Option<FileStat>, Error> {
        let path = validate_file_path(path)?;
        Ok(self
            .live
            .get(&path)
            .and_then(|head| self.stat_of(&path, head)))
    }

    fn read(
        &self,
        path: &str,
        generation: Option<u64>,
        snapshot: Option<String>,
    ) -> Result<Option<Generation>, Error> {
        if generation.is_some() && snapshot.is_some() {
            return Err(Error::Module(
                "read: generation and snapshot are mutually exclusive".into(),
            ));
        }
        let path = validate_file_path(path)?;
        if let Some(name) = snapshot {
            let Some(g) = self.snapshots.get(&name).and_then(|pins| pins.get(&path)) else {
                return Ok(None);
            };
            return Ok(self.gens.get(&(path, *g)).cloned());
        }
        let Some(head) = self.live.get(&path) else {
            return Ok(None);
        };
        let g = match generation {
            Some(g) if g < head.first || g > head.latest => return Ok(None),
            Some(g) => g,
            None => head.latest,
        };
        Ok(self.gens.get(&(path, g)).cloned())
    }

    fn find(
        &self,
        prefix: &str,
        meta_filter: &BTreeMap<String, String>,
        limit: u64,
    ) -> Vec<FileStat> {
        let limit = limit.min(MAX_QUERY_LIMIT) as usize;
        let mut out = Vec::new();
        for (path, head) in &self.live {
            if out.len() >= limit {
                break;
            }
            if !path.starts_with(prefix) {
                continue;
            }
            let Some(stat) = self.stat_of(path, head) else {
                continue;
            };
            if meta_filter
                .iter()
                .all(|(k, v)| stat.latest_meta.get(k) == Some(v))
            {
                out.push(stat);
            }
        }
        out
    }

    fn grep(&self, prefix: &str, pattern: &str, limit: u64) -> Vec<GrepHit> {
        let limit = limit.min(MAX_QUERY_LIMIT) as usize;
        let mut hits = Vec::new();
        for (path, head) in &self.live {
            if !path.starts_with(prefix) {
                continue;
            }
            let generation = head.latest;
            // like stat_of: a missing record is unreachable past strict decode,
            // but a query must degrade gracefully rather than panic.
            let Some(record) = self.gens.get(&(path.clone(), generation)) else {
                continue;
            };
            for (idx, line) in record.body.split('\n').enumerate() {
                if hits.len() >= limit {
                    return hits;
                }
                if line.contains(pattern) {
                    let line_no = idx as u64 + 1;
                    hits.push(GrepHit {
                        uri: format!("duck://memory{path}@{generation}#L{line_no}"),
                        path: path.clone(),
                        generation,
                        line: line_no,
                        text: truncate_utf8(line, MAX_GREP_LINE_BYTES),
                    });
                }
            }
        }
        hits
    }

    // ---- canonical encode / decode (the root preimage) ---------------------

    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.live.len() as u64).to_le_bytes());
        for (path, head) in &self.live {
            push_str(&mut out, path);
            out.extend_from_slice(&head.first.to_le_bytes());
            out.extend_from_slice(&head.latest.to_le_bytes());
        }
        out.extend_from_slice(&(self.gens.len() as u64).to_le_bytes());
        for ((path, g), rec) in &self.gens {
            push_str(&mut out, path);
            out.extend_from_slice(&g.to_le_bytes());
            push_str(&mut out, &rec.body);
            push_meta(&mut out, &rec.meta);
            push_str(&mut out, &rec.author);
            out.extend_from_slice(&rec.published_at_height.to_le_bytes());
        }
        out.extend_from_slice(&(self.snapshots.len() as u64).to_le_bytes());
        for (name, pins) in &self.snapshots {
            push_str(&mut out, name);
            out.extend_from_slice(&(pins.len() as u64).to_le_bytes());
            for (path, g) in pins {
                push_str(&mut out, path);
                out.extend_from_slice(&g.to_le_bytes());
            }
        }
        out.extend_from_slice(&(self.watches.len() as u64).to_le_bytes());
        for (prefix, module_id) in &self.watches {
            push_str(&mut out, prefix);
            push_str(&mut out, module_id);
        }
        out
    }

    /// strict decode: only canonical encodings of execute-reachable states are
    /// accepted. every section demands strictly-ascending unique keys (the only
    /// order [`Store::encode`] emits), canonical paths, execute-time caps, live
    /// heads whose FULL generation range is present (publish inserts each
    /// record; delete only removes records when the head goes), and snapshot
    /// pins that resolve to a present record. anything else is rejected — an
    /// honest validator can never have committed it.
    fn decode(bytes: &[u8]) -> Result<Store, Error> {
        let mut off = 0usize;
        let mut store = Store::default();

        for _ in 0..read_count(bytes, &mut off)? {
            let path = read_string(bytes, &mut off)?;
            validate_file_path(&path)?;
            let first = read_u64(bytes, &mut off)?;
            let latest = read_u64(bytes, &mut off)?;
            // generation numbering starts at 1, and one incarnation never holds
            // more than the generation cap (also bounds the range walk below).
            if first == 0 || first > latest || latest - first >= MAX_GENERATIONS_PER_PATH {
                return Err(Error::Module("snapshot live head range is invalid".into()));
            }
            if store
                .live
                .last_key_value()
                .is_some_and(|(last, _)| last.as_str() >= path.as_str())
            {
                return Err(Error::Module(
                    "snapshot live paths not strictly ascending".into(),
                ));
            }
            store.live.insert(path, LiveHead { first, latest });
        }

        for _ in 0..read_count(bytes, &mut off)? {
            let path = read_string(bytes, &mut off)?;
            validate_file_path(&path)?;
            let generation = read_u64(bytes, &mut off)?;
            if generation == 0 {
                return Err(Error::Module("snapshot generation must be >= 1".into()));
            }
            let body = read_string(bytes, &mut off)?;
            if body.len() > MAX_BODY_BYTES {
                return Err(Error::Module("snapshot body exceeds cap".into()));
            }
            let meta = read_meta(bytes, &mut off)?;
            validate_meta(&meta)?;
            let author = read_string(bytes, &mut off)?;
            let published_at_height = read_u64(bytes, &mut off)?;
            let key = (path, generation);
            if store
                .gens
                .last_key_value()
                .is_some_and(|(last, _)| *last >= key)
            {
                return Err(Error::Module(
                    "snapshot generation keys not strictly ascending".into(),
                ));
            }
            store.gens.insert(
                key,
                Generation {
                    generation,
                    body,
                    meta,
                    author,
                    published_at_height,
                },
            );
        }

        // every live head's full [first..=latest] range must be present.
        for (path, head) in &store.live {
            for g in head.first..=head.latest {
                if !store.gens.contains_key(&(path.clone(), g)) {
                    return Err(Error::Module(
                        "snapshot live head missing a generation record".into(),
                    ));
                }
            }
        }

        for _ in 0..read_count(bytes, &mut off)? {
            let name = read_string(bytes, &mut off)?;
            if name.is_empty() || name.len() > MAX_SNAPSHOT_NAME_BYTES {
                return Err(Error::Module("snapshot name is invalid".into()));
            }
            if store
                .snapshots
                .last_key_value()
                .is_some_and(|(last, _)| last.as_str() >= name.as_str())
            {
                return Err(Error::Module(
                    "snapshot names not strictly ascending".into(),
                ));
            }
            let mut pins: BTreeMap<String, u64> = BTreeMap::new();
            for _ in 0..read_count(bytes, &mut off)? {
                let path = read_string(bytes, &mut off)?;
                validate_file_path(&path)?;
                let g = read_u64(bytes, &mut off)?;
                if pins
                    .last_key_value()
                    .is_some_and(|(last, _)| last.as_str() >= path.as_str())
                {
                    return Err(Error::Module(
                        "snapshot pin paths not strictly ascending".into(),
                    ));
                }
                if !store.gens.contains_key(&(path.clone(), g)) {
                    return Err(Error::Module(
                        "snapshot pin references a missing generation".into(),
                    ));
                }
                pins.insert(path, g);
            }
            store.snapshots.insert(name, pins);
        }

        for _ in 0..read_count(bytes, &mut off)? {
            let prefix = read_string(bytes, &mut off)?;
            validate_prefix(&prefix)?;
            let module_id = read_string(bytes, &mut off)?;
            if module_id.is_empty() || module_id.len() > MAX_MODULE_ID_BYTES {
                return Err(Error::Module("snapshot watch module id is invalid".into()));
            }
            let key = (prefix, module_id);
            if store.watches.last().is_some_and(|last| *last >= key) {
                return Err(Error::Module(
                    "snapshot watches not strictly ascending".into(),
                ));
            }
            store.watches.insert(key);
        }

        if off != bytes.len() {
            return Err(Error::Module("snapshot has trailing bytes".into()));
        }
        Ok(store)
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Memory {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.committed)
    }

    /// advertise the snapshot lane: [`Memory::snapshot`] is the exact `root()`
    /// preimage and [`Memory::install`] verifies before adopting (tasks pattern).
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // every write requires an authenticated origin, even ops that store no
        // author — the empty demo-default external origin never passes.
        let author = author_from_origin(&ctx.env().origin)?;
        let height = ctx.env().height;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            MemoryMsg::Publish { path, body, meta } => {
                self.publish(ctx, author, height, path, body, meta)
            }
            MemoryMsg::Delete { path } => self.delete(path),
            MemoryMsg::Snapshot { name } => self.create_snapshot(name),
            MemoryMsg::DropSnapshot { name } => self.drop_snapshot(name),
            MemoryMsg::RegisterWatch { prefix, module_id } => {
                self.register_watch(ctx, prefix, module_id)
            }
            MemoryMsg::UnregisterWatch { prefix, module_id } => {
                self.unregister_watch(prefix, module_id)
            }
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let reply = match decode_query(req).map_err(Error::Module)? {
            MemoryQuery::Ls { path, limit } => MemoryReply::Ls(self.committed.ls(&path, limit)?),
            MemoryQuery::Stat { path } => MemoryReply::Stat(self.committed.stat(&path)?),
            MemoryQuery::Read {
                path,
                generation,
                snapshot,
            } => MemoryReply::Read(self.committed.read(&path, generation, snapshot)?),
            MemoryQuery::Find {
                prefix,
                meta_filter,
                limit,
            } => MemoryReply::Find(self.committed.find(&prefix, &meta_filter, limit)),
            MemoryQuery::Grep {
                prefix,
                pattern,
                limit,
            } => MemoryReply::Grep(self.committed.grep(&prefix, &pattern, limit)),
        };
        Ok(encode_reply(&reply))
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        if let Some(pending) = self.pending.take() {
            self.committed = pending;
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending = None;
        Ok(())
    }
}

// ---- validation & helpers --------------------------------------------------

/// derive the stored author from the dispatch origin — never from a payload.
/// external identities are domain-separated as `"ext:"` + lowercase hex (the
/// cross-module convention shared with jobs/inbox/files), so a hex-shaped
/// module id can never collide with an external identity.
fn author_from_origin(origin: &Origin) -> Result<String, Error> {
    match origin {
        Origin::External(bytes) if bytes.is_empty() => {
            Err(Error::Module("unauthenticated external origin".into()))
        }
        Origin::External(bytes) => Ok(format!("ext:{}", hex(bytes))),
        Origin::Module(id) => Ok(id.clone()),
        Origin::System => Ok("system".into()),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// validate + canonicalize a path that must name a FILE (never the root).
fn validate_file_path(path: &str) -> Result<String, Error> {
    let segments = normalize_segments(path)?;
    if segments.is_empty() {
        return Err(Error::Module("path must name a file, not the root".into()));
    }
    Ok(canonical_path(&segments))
}

/// validate a path used for listing; the root `/` is allowed.
fn validate_query_path(path: &str) -> Result<Vec<String>, Error> {
    normalize_segments(path)
}

/// strict normalization: reject anything not already in canonical form (no empty
/// segments, no `.`/`..`, no trailing slash except root), enforcing byte caps.
fn normalize_segments(path: &str) -> Result<Vec<String>, Error> {
    if path.is_empty() {
        return Err(Error::Module("path must not be empty".into()));
    }
    if path.len() > MAX_PATH_BYTES {
        return Err(Error::Module("path exceeds byte cap".into()));
    }
    let Some(rest) = path.strip_prefix('/') else {
        return Err(Error::Module(
            "path must be absolute (start with '/')".into(),
        ));
    };
    if rest.is_empty() {
        return Ok(Vec::new()); // the root "/"
    }
    if rest.ends_with('/') {
        return Err(Error::Module("path must not have a trailing slash".into()));
    }
    let mut segments = Vec::new();
    for seg in rest.split('/') {
        if seg.is_empty() {
            return Err(Error::Module("path must not contain empty segments".into()));
        }
        if seg == "." || seg == ".." {
            return Err(Error::Module("path must not contain '.' or '..'".into()));
        }
        if seg.len() > MAX_SEGMENT_BYTES {
            return Err(Error::Module("path segment exceeds byte cap".into()));
        }
        segments.push(seg.to_string());
    }
    Ok(segments)
}

fn split_segments(path: &str) -> Vec<String> {
    // `path` here is always a stored canonical file path, so this is the inverse
    // of `canonical_path` and never yields empty segments.
    path.strip_prefix('/')
        .unwrap_or(path)
        .split('/')
        .map(str::to_string)
        .collect()
}

fn canonical_path(segments: &[String]) -> String {
    let mut s = String::new();
    for seg in segments {
        s.push('/');
        s.push_str(seg);
    }
    s
}

fn validate_meta(meta: &Meta) -> Result<(), Error> {
    if meta.len() > MAX_META_ENTRIES {
        return Err(Error::Module("meta exceeds entry cap".into()));
    }
    for (key, value) in meta {
        if key.len() > MAX_META_KEY_BYTES {
            return Err(Error::Module("meta key exceeds byte cap".into()));
        }
        if value.len() > MAX_META_VALUE_BYTES {
            return Err(Error::Module("meta value exceeds byte cap".into()));
        }
    }
    Ok(())
}

/// a watch prefix is a CANONICAL absolute path — the same segment validation as
/// file paths, with the root `"/"` additionally allowed (watch everything).
fn validate_prefix(prefix: &str) -> Result<(), Error> {
    normalize_segments(prefix).map(|_| ())
}

/// segment-aware subtree match: the root watches everything; otherwise the
/// published path must BE the prefix or live strictly below it — a `/a` watch
/// matches `/a` and `/a/b` but never `/ab`.
fn watch_matches(prefix: &str, path: &str) -> bool {
    prefix == "/"
        || path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// truncate to at most `max_bytes`, backing off to the previous char boundary so
/// the result is always valid UTF-8.
fn truncate_utf8(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

fn push_str(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn push_meta(out: &mut Vec<u8>, meta: &Meta) {
    out.extend_from_slice(&(meta.len() as u64).to_le_bytes());
    for (key, value) in meta {
        push_str(out, key);
        push_str(out, value);
    }
}

/// a length-prefixed collection count, guarded so a corrupt count can never make
/// the decoder loop or allocate unboundedly (each entry costs >= 1 byte).
fn read_count(bytes: &[u8], off: &mut usize) -> Result<u64, Error> {
    let n = read_u64(bytes, off)?;
    if n > (bytes.len() - *off) as u64 {
        return Err(Error::Module("snapshot truncated".into()));
    }
    Ok(n)
}

fn read_meta(bytes: &[u8], off: &mut usize) -> Result<Meta, Error> {
    let mut meta = BTreeMap::new();
    for _ in 0..read_count(bytes, off)? {
        let key = read_string(bytes, off)?;
        let value = read_string(bytes, off)?;
        meta.insert(key, value);
    }
    Ok(meta)
}

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

fn read_string(bytes: &[u8], off: &mut usize) -> Result<String, Error> {
    let len = read_u64(bytes, off)?;
    let len = usize::try_from(len).map_err(|_| Error::Module("snapshot truncated".into()))?;
    if len > bytes.len() - *off {
        return Err(Error::Module("snapshot truncated".into()));
    }
    let value = std::str::from_utf8(&bytes[*off..*off + len])
        .map_err(|_| Error::Module("snapshot string is not utf-8".into()))?;
    *off += len;
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// strict decode makes a live head without its generation record
    /// unreachable, but if such a state ever existed in memory the read verbs
    /// must degrade gracefully (skip / None) — a panic in a query path is a
    /// validator crash.
    #[test]
    fn read_verbs_degrade_gracefully_on_a_missing_head_record() {
        let mut store = Store::default();
        let head = LiveHead {
            first: 1,
            latest: 1,
        };
        store.live.insert("/ghost".into(), head);

        assert_eq!(store.stat_of("/ghost", &head), None);
        assert_eq!(store.stat("/ghost").unwrap(), None);
        assert!(store.ls("/", 256).unwrap().is_empty());
        assert!(store.find("/", &BTreeMap::new(), 256).is_empty());
        assert!(store.grep("/", "", 256).is_empty());
    }
}
