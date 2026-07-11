//! disk persistence: the content-addressed odb as [`DiskStore`] (task 5) and
//! the durable refs-file envelope as [`DiskRefs`] (task 6 — the block commit
//! point; its docblock carries the load-bearing durability-ordering contract).
//!
//! WHY tmp-in-same-dir + rename: a write must publish atomically — a reader (or
//! a crash) must see either no object or the whole object, never a half-written
//! file under its final content-addressed name. `rename(2)` is atomic only
//! within a single directory, so the tmp is created in the SAME `<aa>/` fanout
//! subdir as its destination; `sync_all` flushes the bytes before the rename
//! makes them reachable. tmp names are `<hex>.tmp`, and `open` sweeps `*.tmp`
//! at any depth so a crash between create and rename leaves no debris behind.
//!
//! WHY `get` re-verifies: the filename is the claimed id, but the disk is
//! untrusted (bit-rot, a torn write, a hostile edit). `get` re-derives
//! `object_id(kind, body)` and errors on mismatch — a corrupt object surfaces
//! as `Err`, never as silently-wrong bytes. that error is the signal the later
//! self-heal lane keys on to re-fetch the object from a peer.

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use duckfs_core::objects::{Kind, ObjectId, object_id};
use duckfs_core::state::{Refs, decode_refs, encode_refs};
use duckfs_core::store::ObjectStore;
use duckfs_core::{from_hex_32, to_hex};

/// content-addressed object database over `dir/<aa>/<hex[2..]>` files. each file
/// is `[kind u8] ‖ body`; the filename is the 64-char lowercase-hex id.
pub struct DiskStore {
    dir: PathBuf,
    /// fanout subdirs that received a new object since the last [`DiskStore::sync_dirs`].
    /// `put` makes an object's BYTES durable (fsync of the tmp before rename) but
    /// not its directory ENTRY (the rename); the block glue fsyncs these dirs at
    /// the commit boundary so every published object is fully durable BEFORE the
    /// refs file — the object side of the torn-commit fix (task 5 review).
    dirty: BTreeSet<PathBuf>,
}

impl DiskStore {
    /// create the odb root and sweep any `*.tmp` crash debris under it. a fresh
    /// dir is created lazily; fanout subdirs appear on first `put` into them.
    pub fn open(dir: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        sweep_tmp(&dir)?;
        Ok(Self {
            dir,
            dirty: BTreeSet::new(),
        })
    }

    /// fsync every fanout dir that received an object since the last call, plus
    /// the odb root (so a freshly-created `<aa>/` dir-entry is itself durable).
    /// the block glue calls this AFTER flushing the block's objects and BEFORE
    /// persisting the refs file, so a crash after the refs commit point can
    /// never reference an object whose directory entry never reached disk.
    ///
    /// why not just fsync the odb root: fsync of a directory persists only THAT
    /// directory's own name→inode entries, not its children's — so the object
    /// files inside `<aa>/` need `<aa>/` itself fsynced. we fsync each touched
    /// fanout dir for the file entries, then the root for the fanout-dir entries.
    /// the reverse gap (objects durable, refs not) is harmless: objects are
    /// content-addressed and idempotently re-put on replay.
    pub fn sync_dirs(&mut self) -> Result<(), String> {
        if self.dirty.is_empty() {
            return Ok(()); // nothing was published this block
        }
        for dir in &self.dirty {
            fsync_dir(dir)?;
        }
        fsync_dir(&self.dir)?;
        self.dirty.clear();
        Ok(())
    }

    /// the on-disk path for an id: `dir/<aa>/<hex[2..]>`. returned with the hex
    /// so callers can build error context without re-encoding.
    fn object_path(&self, id: &ObjectId) -> (String, PathBuf) {
        let hex = to_hex(id);
        let path = self.dir.join(&hex[..2]).join(&hex[2..]);
        (hex, path)
    }
}

/// fsync a directory so its name→inode entries are durable — the barrier that
/// makes a `rename` into (or a fresh subdir under) `dir` survive a crash. opened
/// read-only and `sync_all`'d; this is a unix/macos operation (the deploy
/// targets), where a directory can be opened as a file for fsync. `pub(crate)`:
/// the statesync scratch promotion (`scratch.rs`) shares this barrier.
pub(crate) fn fsync_dir(dir: &Path) -> Result<(), String> {
    std::fs::File::open(dir)
        .and_then(|f| f.sync_all())
        .map_err(|e| format!("files: fsync dir {}: {e}", dir.display()))
}

/// recursively remove every `*.tmp` file under `dir` (crash debris from a write
/// that died between create-tmp and rename).
fn sweep_tmp(dir: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            sweep_tmp(&path)?;
        } else if entry.file_name().to_string_lossy().ends_with(".tmp") {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

impl ObjectStore for DiskStore {
    fn put(&mut self, kind: Kind, body: &[u8]) -> Result<ObjectId, String> {
        let id = object_id(kind, body);
        let (hex, dest) = self.object_path(&id);
        // content-addressed + idempotent: an INTACT existing file already holds
        // exactly these bytes, so re-putting is a no-op (and cheap — no rewrite).
        // but the disk is untrusted: a corrupt existing object (a torn/truncated
        // external write) must be REPLACED, not trusted as "already present", or a
        // "possessed" object stays permanently unreadable and put could never
        // repair it (finding #2). cheap guard, no body read on the hot path: the
        // stored file must be exactly `[kind] ‖ body` = `body.len() + 1` bytes, so
        // a length mismatch is definitely corrupt — fall through and atomically
        // overwrite it via the same tmp+rename below. (a same-LENGTH bit-flip is
        // caught by `verify` on the possession path, which deletes it so the next
        // put lands on an absent name.)
        let intact_len = (body.len() as u64) + 1;
        if dest.exists()
            && std::fs::metadata(&dest)
                .map(|m| m.len() == intact_len)
                .unwrap_or(false)
        {
            return Ok(id);
        }
        let subdir = self.dir.join(&hex[..2]);
        std::fs::create_dir_all(&subdir)
            .map_err(|e| format!("files: odb put {hex}: mkdir: {e}"))?;
        // tmp lives in the destination subdir so the rename below is same-dir
        // (and therefore atomic). the full hex keeps the tmp name unique.
        let tmp = subdir.join(format!("{hex}.tmp"));
        let mut buf = Vec::with_capacity(1 + body.len());
        buf.push(kind.tag());
        buf.extend_from_slice(body);
        // scope the file so it is closed before the rename on every platform.
        {
            let mut f = std::fs::File::create(&tmp)
                .map_err(|e| format!("files: odb put {hex}: create tmp: {e}"))?;
            f.write_all(&buf)
                .map_err(|e| format!("files: odb put {hex}: write tmp: {e}"))?;
            // durable before publish: the bytes must hit disk before the rename
            // makes them reachable under the content-addressed name.
            f.sync_all()
                .map_err(|e| format!("files: odb put {hex}: fsync tmp: {e}"))?;
        }
        std::fs::rename(&tmp, &dest).map_err(|e| {
            // a failed publish must not leave debris under the object name.
            let _ = std::fs::remove_file(&tmp);
            format!("files: odb put {hex}: rename: {e}")
        })?;
        // the rename made a new dir-entry in `subdir`; remember it so the block
        // glue can fsync it before the refs commit point (see `sync_dirs`).
        self.dirty.insert(subdir);
        Ok(id)
    }

    fn get(&self, id: &ObjectId) -> Result<Option<(Kind, Vec<u8>)>, String> {
        let (hex, path) = self.object_path(id);
        let raw = match std::fs::read(&path) {
            Ok(raw) => raw,
            // absent is Ok(None), sharply distinct from a corrupt Err below.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("files: odb get {hex}: {e}")),
        };
        let (tag, body) = raw
            .split_first()
            .ok_or_else(|| format!("files: odb get {hex}: object file is empty"))?;
        let kind = Kind::from_u8(*tag)
            .ok_or_else(|| format!("files: odb get {hex}: unknown kind tag {tag}"))?;
        // re-derive and verify: the disk is untrusted, so a bit-flip must surface
        // as an error rather than return wrong bytes under a trusted id.
        if object_id(kind, body) != *id {
            return Err(format!(
                "files: odb get {hex}: content hash mismatch (corrupt object)"
            ));
        }
        Ok(Some((kind, body.to_vec())))
    }

    fn has(&self, id: &ObjectId) -> bool {
        self.object_path(id).1.exists()
    }

    fn verify(&self, id: &ObjectId) -> Result<bool, String> {
        // integrity-verified presence (finding #2): read the file, re-derive the
        // id, and on a PROVEN mismatch (empty file, unknown kind tag, or hash
        // mismatch) DELETE the corrupt object so it reads as absent and the
        // self-heal fetch loop re-fetches a good copy (which `put` then lands). a
        // NotFound is a clean `Ok(false)`; a genuine read error (not a proven
        // corruption) propagates — we never delete on a transient io fault.
        // removal is best-effort + path-based (idempotent), so verify stays
        // `&self` like the rest of the read surface.
        let (hex, path) = self.object_path(id);
        let raw = match std::fs::read(&path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(format!("files: odb verify {hex}: {e}")),
        };
        let intact = match raw.split_first() {
            Some((tag, body)) => match Kind::from_u8(*tag) {
                Some(kind) => object_id(kind, body) == *id,
                None => false,
            },
            None => false, // an empty file cannot be any object
        };
        if !intact {
            // a racing writer or fs error here is harmless — the next verify
            // re-checks, and an absent name is exactly the self-heal signal.
            let _ = std::fs::remove_file(&path);
        }
        Ok(intact)
    }

    fn stat(&self, id: &ObjectId) -> Result<Option<(Kind, u64)>, String> {
        // metadata-only by contract: one open, a 1-byte kind-tag read, and an
        // fstat — the body (file length minus the tag byte) is never read, so
        // commit's chunk-length check stays cheap on the execute path.
        let (hex, path) = self.object_path(id);
        let mut f = match std::fs::File::open(&path) {
            Ok(f) => f,
            // absent is Ok(None), sharply distinct from a corrupt Err below.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("files: odb stat {hex}: {e}")),
        };
        let len = f
            .metadata()
            .map_err(|e| format!("files: odb stat {hex}: {e}"))?
            .len();
        if len == 0 {
            return Err(format!("files: odb stat {hex}: object file is empty"));
        }
        let mut tag = [0u8; 1];
        std::io::Read::read_exact(&mut f, &mut tag)
            .map_err(|e| format!("files: odb stat {hex}: {e}"))?;
        let kind = Kind::from_u8(tag[0])
            .ok_or_else(|| format!("files: odb stat {hex}: unknown kind tag {}", tag[0]))?;
        Ok(Some((kind, len - 1)))
    }

    fn remove(&mut self, id: &ObjectId) -> Result<(), String> {
        let (hex, path) = self.object_path(id);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            // a missing object is already removed — idempotent, not an error.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("files: odb remove {hex}: {e}")),
        }
    }

    fn list(&self) -> Result<Vec<ObjectId>, String> {
        let mut out = Vec::new();
        let top = match std::fs::read_dir(&self.dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(format!("files: odb list: {e}")),
        };
        for aa_entry in top {
            let aa_entry = aa_entry.map_err(|e| format!("files: odb list: {e}"))?;
            let aa_name = aa_entry.file_name();
            let Some(aa) = aa_name.to_str() else { continue };
            // fanout dirs are exactly two hex chars; anything else is foreign.
            if aa.len() != 2 {
                continue;
            }
            if !aa_entry
                .file_type()
                .map_err(|e| format!("files: odb list: {e}"))?
                .is_dir()
            {
                continue;
            }
            for f_entry in
                std::fs::read_dir(aa_entry.path()).map_err(|e| format!("files: odb list: {e}"))?
            {
                let f_entry = f_entry.map_err(|e| format!("files: odb list: {e}"))?;
                let f_name = f_entry.file_name();
                let Some(rest) = f_name.to_str() else {
                    continue;
                };
                // an object filename is the remaining 62 hex chars; this drops
                // `<hex>.tmp` debris and any stray non-object name.
                if rest.len() != 62 {
                    continue;
                }
                let hex = format!("{aa}{rest}");
                if let Some(id) = from_hex_32(&hex) {
                    out.push(id);
                }
            }
        }
        // deterministic and identical to MemStore's BTreeMap key order.
        out.sort();
        Ok(out)
    }
}

/// the durable refs file — the module's commit point — at `dir/refs`.
///
/// # the recovery contract (load-bearing)
///
/// the refs file is the SINGLE durable commit point of a block. its atomic
/// replacement is the moment the module's committed state advances, and the
/// block glue (`module.rs commit_block`) orders everything else around it:
///
/// 1. flush the block's objects into the odb (idempotent, content-addressed)
/// 2. `DiskStore::sync_dirs` — the object dir-entries are now durable
/// 3. `DiskRefs::save` — tmp → fsync(file) → rename → fsync(parent dir); the
///    rename is atomic, and the parent-dir fsync makes the rename's dir-entry
///    itself durable, so the file is either wholly the old image or wholly the
///    new one, never half-written and never a dangling dir-entry
/// 4. only now does the in-memory root adopt the new refs
///
/// so a crash BEFORE the step-3 rename leaves the OLD refs file, the OLD root,
/// and at worst some orphan objects (harmless — re-put idempotently on replay,
/// swept by a later gc). a crash AFTER it has the NEW refs and — because step 2
/// preceded it — every object the new refs names. there is no window in which
/// the durable root and the durable objects disagree. this is exactly the
/// non-atomic disk-vs-memory ordering that produced this repo's historic
/// torn-commit brick, closed here by construction.
///
/// # the envelope
///
/// `b"DUCKFS1\n" ‖ height u64 ‖ gc_watermark u64 ‖ payload_len u64 ‖ payload ‖
/// sha256(payload) 32 B`, all little-endian, where `payload = encode_refs`.
/// height and gc_watermark are per-node recovery bookkeeping and live ONLY here,
/// never in the root preimage. `load` returns `Ok(None)` for a fresh dir (no
/// file) but ERRORS on any corruption — a bad magic, a checksum mismatch, a
/// truncation, or trailing bytes. a corrupt refs file must brick LOUDLY (the
/// node re-syncs from a peer); silently defaulting would fork state.
pub struct DiskRefs {
    /// module data dir; the refs file is `dir/refs`, its tmp `dir/refs.tmp`.
    dir: PathBuf,
}

/// the refs-file magic — bump on any envelope layout change (flag-day rule: no
/// migrations, fresh genesis).
const REFS_MAGIC: &[u8; 8] = b"DUCKFS1\n";
/// magic(8) + height(8) + gc_watermark(8) + payload_len(8) + checksum(32).
const REFS_FIXED_LEN: usize = 8 + 8 + 8 + 8 + 32;

impl DiskRefs {
    /// open over the module data dir, creating it if absent (the refs file and
    /// its tmp are written here, and the dir is fsync'd on save).
    pub fn open(dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("files: refs open {}: {e}", dir.display()))?;
        Ok(Self { dir })
    }

    fn refs_path(&self) -> PathBuf {
        self.dir.join("refs")
    }
}

impl DiskRefs {
    /// `None` = fresh dir. `Ok(Some((refs, height, gc_watermark)))` otherwise.
    pub fn load(&self) -> Result<Option<(Refs, u64, u64)>, String> {
        let path = self.refs_path();
        let raw = match std::fs::read(&path) {
            Ok(raw) => raw,
            // absent = a fresh node, sharply distinct from a corrupt Err below.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("files: refs load: {e}")),
        };
        if raw.len() < REFS_FIXED_LEN {
            return Err("files: refs file shorter than its envelope".into());
        }
        if &raw[..8] != REFS_MAGIC {
            return Err("files: refs file magic mismatch".into());
        }
        let height = u64::from_le_bytes(raw[8..16].try_into().expect("8 bytes"));
        let gc_watermark = u64::from_le_bytes(raw[16..24].try_into().expect("8 bytes"));
        let payload_len = u64::from_le_bytes(raw[24..32].try_into().expect("8 bytes"));
        let payload_len = usize::try_from(payload_len)
            .map_err(|_| "files: refs payload_len overflow".to_string())?;
        // the total length must be EXACTLY the envelope + declared payload — a
        // short file is a truncation, a long one carries trailing bytes; both
        // reject rather than decode a partial/padded image.
        let want = REFS_FIXED_LEN
            .checked_add(payload_len)
            .ok_or_else(|| "files: refs payload_len overflow".to_string())?;
        if raw.len() != want {
            return Err("files: refs file length does not match payload_len".into());
        }
        let payload = &raw[32..32 + payload_len];
        let stored_sum = &raw[32 + payload_len..];
        let mut h = Sha256::new();
        h.update(payload);
        let sum: [u8; 32] = h.finalize().into();
        if stored_sum != sum {
            return Err("files: refs file checksum mismatch (corrupt)".into());
        }
        // strict decode: even a checksum-valid but non-canonical payload rejects.
        let refs = decode_refs(payload)?;
        Ok(Some((refs, height, gc_watermark)))
    }

    pub fn save(&mut self, refs: &Refs, height: u64, gc_watermark: u64) -> Result<(), String> {
        let payload = encode_refs(refs);
        let mut buf = Vec::with_capacity(REFS_FIXED_LEN + payload.len());
        buf.extend_from_slice(REFS_MAGIC);
        buf.extend_from_slice(&height.to_le_bytes());
        buf.extend_from_slice(&gc_watermark.to_le_bytes());
        buf.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        buf.extend_from_slice(&payload);
        let mut h = Sha256::new();
        h.update(&payload);
        let sum: [u8; 32] = h.finalize().into();
        buf.extend_from_slice(&sum);

        let dest = self.refs_path();
        let tmp = self.dir.join("refs.tmp");
        // scope the file so it is closed before the rename on every platform.
        {
            let mut f = std::fs::File::create(&tmp)
                .map_err(|e| format!("files: refs save: create tmp: {e}"))?;
            f.write_all(&buf)
                .map_err(|e| format!("files: refs save: write tmp: {e}"))?;
            // the bytes must be durable before the rename publishes them.
            f.sync_all()
                .map_err(|e| format!("files: refs save: fsync tmp: {e}"))?;
        }
        std::fs::rename(&tmp, &dest).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("files: refs save: rename: {e}")
        })?;
        // the rename is the commit point; fsync the parent dir so its dir-entry
        // (pointing at the new refs file) is itself durable — otherwise a crash
        // could lose the rename even though the file bytes were synced.
        fsync_dir(&self.dir).map_err(|e| format!("files: refs save: {e}"))?;
        Ok(())
    }
}
