//! #219 hardening: the attempt-scoped statesync scratch for the duckfs dir.
//!
//! unlike the qmdb modules — whose `sync_from` lands under an attempt-scoped
//! runtime child (`{name}_scratch_a{attempt}`) so a failed join's partial store
//! never occupies the canonical namespace — the files module's odb is a plain
//! filesystem directory. this module gives it the same discipline:
//!
//! - [`SyncScratch::prepare`] carves out `<canonical>_scratch_a{attempt}` next
//!   to the canonical dir; the joiner opens its [`Files`](crate::Files) THERE,
//!   so install/ingest never touch the canonical dir. it also sweeps stale
//!   scratch siblings from earlier attempts and SEEDS the new scratch's odb
//!   from the canonical odb and from those stale scratches (hard links where
//!   the filesystem supports them — objects are immutable once published, so
//!   shared inodes are safe), keeping the object fetch incremental: a
//!   rejoining node refetches only the delta past its last promoted boundary,
//!   and a retry never refetches what a failed attempt already landed.
//! - [`SyncScratch::promote`] is the ONLY road into the canonical dir, and it
//!   is verify-then-replace: the scratch refs envelope is checksum-loaded and
//!   re-hashed against the caller's expected (app-hash-verified) root, the
//!   objects are published durably into the canonical odb FIRST (link-or-copy,
//!   touched-dir fsync — `commit_block`'s ordering contract), and only then is
//!   the canonical refs file atomically replaced at the scratch's sync-target
//!   height. a failed join simply never calls it: the canonical dir stays
//!   byte-untouched, and content-addressing makes a re-promotion at the same
//!   boundary a no-op walk (idempotent across retries).
//! - [`SyncScratch::sweep_stale`] is the boot-time janitor: a restarting node
//!   (validator or joiner) holds no live scratch handle, so any leftover
//!   `<canonical>_scratch_a<n>` dir — a crashed attempt, or a promoted scratch
//!   whose final removal was interrupted — is safely removed. it matches the
//!   attempt-dir shape STRICTLY and never touches the canonical dir itself.
//!
//! a rejoining node's superseded canonical objects are deliberately kept:
//! refs replacement never deletes data, so they linger as unreachable orphans
//! until a routine gc sweep — exactly the fate of any other orphan object.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::disk::{DiskRefs, fsync_dir};
use duckfs_core::state::root_bytes;
use duckfs_core::store::RefsStore as _;
use duckfs_core::{from_hex_32, to_hex};

/// an attempt-scoped scratch dir for the files statesync lane: sync into
/// [`dir`](Self::dir), then either [`promote`](Self::promote) on a verified
/// join or simply drop it (a later attempt / boot sweeps the leftovers).
pub struct SyncScratch {
    canonical: PathBuf,
    dir: PathBuf,
}

impl SyncScratch {
    /// carve out `<canonical>_scratch_a{attempt}` next to the canonical dir,
    /// sweep stale scratch siblings, and seed the fresh scratch's odb from
    /// every local object source (canonical + stale scratches). the canonical
    /// dir itself is never created or written here.
    ///
    /// safe to call whenever the caller holds no live scratch-backed module —
    /// which the node guarantees by dropping any served host before a sync
    /// attempt (attempt numbers never repeat within a process, and a same-name
    /// leftover can only be a dead prior run's: its stale refs are cleared, its
    /// objects are kept as seed).
    pub fn prepare(canonical: &Path, attempt: usize) -> Result<Self, String> {
        let dir = attempt_dir(canonical, attempt)?;
        let stale: Vec<PathBuf> = scratch_siblings(canonical)
            .map_err(|e| format!("files scratch: enumerate stale dirs: {e}"))?
            .into_iter()
            .filter(|p| *p != dir)
            .collect();
        std::fs::create_dir_all(dir.join("objects"))
            .map_err(|e| format!("files scratch: create {}: {e}", dir.display()))?;
        // a same-name leftover from a crashed prior run: its refs are stale
        // (the sync installs fresh, root-verified refs before anything reads
        // them) — drop them so `Files::open` starts from empty refs; its
        // objects are a valid seed (content-addressed, self-verifying).
        match std::fs::remove_file(dir.join("refs")) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("files scratch: clear stale refs: {e}")),
        }
        merge_objects(&canonical.join("objects"), &dir.join("objects"))?;
        for s in &stale {
            merge_objects(&s.join("objects"), &dir.join("objects"))?;
            // spent: everything reusable was just seeded. best-effort — a dir
            // that resists removal is retried by the next attempt / boot sweep.
            let _ = std::fs::remove_dir_all(s);
        }
        Ok(Self {
            canonical: canonical.to_path_buf(),
            dir,
        })
    }

    /// where the joiner's [`Files`](crate::Files) must open for this attempt.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// verify-then-replace promotion into the canonical dir — call ONLY after
    /// the join's composite app-hash gate has passed, with the files root that
    /// gate certified. ordering mirrors `commit_block`'s durability contract:
    ///
    /// 1. checksum-load the scratch refs and re-hash against `expected_root` —
    ///    a mismatch rejects before one byte reaches the canonical dir
    /// 2. publish every scratch object into the canonical odb (link-or-copy;
    ///    an existing destination is already exactly these bytes) and fsync
    ///    the touched fanout dirs + odb root — objects durable FIRST
    /// 3. atomically replace the canonical refs file at the scratch's
    ///    sync-target height (tmp → fsync → rename → dir fsync) — the commit
    ///    point; a crash on either side leaves a coherent canonical dir
    /// 4. remove the spent scratch (best-effort; the boot sweep backstops it)
    ///
    /// idempotent: re-promoting the same boundary re-verifies, walks objects
    /// that all already exist, and rewrites an identical refs file.
    pub fn promote(&self, expected_root: [u8; 32]) -> Result<(), String> {
        let (refs, height, gc_watermark) = DiskRefs::open(self.dir.clone())
            .map_err(|e| format!("files promote: open scratch refs: {e}"))?
            .load()
            .map_err(|e| format!("files promote: load scratch refs: {e}"))?
            .ok_or_else(|| "files promote: the scratch holds no synced refs image".to_string())?;
        let got = root_bytes(&refs);
        if got != expected_root {
            return Err(format!(
                "files promote: scratch refs root {} != expected root {}",
                to_hex(&got),
                to_hex(&expected_root),
            ));
        }
        let canonical_odb = self.canonical.join("objects");
        let touched = merge_objects(&self.dir.join("objects"), &canonical_odb)?;
        if !touched.is_empty() {
            for d in &touched {
                fsync_dir(d).map_err(|e| format!("files promote: {e}"))?;
            }
            fsync_dir(&canonical_odb).map_err(|e| format!("files promote: {e}"))?;
        }
        DiskRefs::open(self.canonical.clone())
            .map_err(|e| format!("files promote: open canonical refs: {e}"))?
            .save(&refs, height, gc_watermark)
            .map_err(|e| format!("files promote: save canonical refs: {e}"))?;
        let _ = std::fs::remove_dir_all(&self.dir);
        Ok(())
    }

    /// boot-time janitor: remove every `<canonical>_scratch_a<digits>` sibling.
    /// best-effort by design — cleanup must never stop a boot — and strictly
    /// scoped: the canonical dir and unrelated siblings are never touched.
    pub fn sweep_stale(canonical: &Path) {
        let Ok(stale) = scratch_siblings(canonical) else {
            return;
        };
        for dir in stale {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

/// the attempt-scoped dir name, mirroring the qmdb runtime-child convention
/// (`{name}_scratch_a{attempt}`), as a SIBLING of the canonical dir.
fn attempt_dir(canonical: &Path, attempt: usize) -> Result<PathBuf, String> {
    let name = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| {
            format!(
                "files scratch: canonical dir {} has no utf-8 name",
                canonical.display()
            )
        })?;
    Ok(canonical.with_file_name(format!("{name}_scratch_a{attempt}")))
}

/// every existing `<name>_scratch_a<digits>` sibling of the canonical dir.
/// strict shape match: anything else next to the canonical dir is foreign and
/// must never be swept.
fn scratch_siblings(canonical: &Path) -> std::io::Result<Vec<PathBuf>> {
    let (Some(parent), Some(name)) = (
        canonical.parent(),
        canonical.file_name().and_then(|n| n.to_str()),
    ) else {
        return Ok(Vec::new());
    };
    let prefix = format!("{name}_scratch_a");
    let entries = match std::fs::read_dir(parent) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(n) = file_name.to_str() else {
            continue;
        };
        let Some(rest) = n.strip_prefix(&prefix) else {
            continue;
        };
        if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
            out.push(entry.path());
        }
    }
    Ok(out)
}

/// link-or-copy every object in the `src` odb that the `dst` odb lacks, and
/// return the touched destination fanout dirs (the caller decides whether the
/// publication must be made durable — promotion fsyncs, seeding need not: a
/// scratch is disposable and every seeded link shares an already-durable
/// inode). a missing `src` is an empty odb, not an error.
fn merge_objects(src: &Path, dst: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let mut touched = BTreeSet::new();
    for_each_object(src, |aa, rest, src_path| {
        let sub = dst.join(aa);
        let dst_path = sub.join(rest);
        if dst_path.exists() {
            return Ok(()); // content-addressed: already exactly these bytes
        }
        std::fs::create_dir_all(&sub)
            .map_err(|e| format!("files scratch: mkdir {}: {e}", sub.display()))?;
        place_object(src_path, &dst_path)?;
        touched.insert(sub.clone());
        Ok(())
    })?;
    Ok(touched)
}

/// walk `src` as a duckfs odb — two-hex-char fanout dirs holding 62-hex-char
/// object files, exactly `DiskStore::list`'s shape — handing each object file
/// to `f`. tmp debris and foreign names are skipped, never propagated.
fn for_each_object(
    src: &Path,
    mut f: impl FnMut(&str, &str, &Path) -> Result<(), String>,
) -> Result<(), String> {
    let ctx = |e: std::io::Error| format!("files scratch: walk {}: {e}", src.display());
    let top = match std::fs::read_dir(src) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(ctx(e)),
    };
    for aa_entry in top {
        let aa_entry = aa_entry.map_err(ctx)?;
        let aa_name = aa_entry.file_name();
        let Some(aa) = aa_name.to_str() else { continue };
        if aa.len() != 2 || !aa_entry.file_type().map_err(ctx)?.is_dir() {
            continue;
        }
        for f_entry in std::fs::read_dir(aa_entry.path()).map_err(ctx)? {
            let f_entry = f_entry.map_err(ctx)?;
            let f_name = f_entry.file_name();
            let Some(rest) = f_name.to_str() else {
                continue;
            };
            if rest.len() != 62 || from_hex_32(&format!("{aa}{rest}")).is_none() {
                continue;
            }
            f(aa, rest, &f_entry.path())?;
        }
    }
    Ok(())
}

/// publish one immutable object file under its final content-addressed name:
/// a hard link where the filesystem supports it (atomic, and it shares the
/// source's already-durable inode), else tmp-copy → fsync → rename — either
/// way the object never appears half-written under its final name.
fn place_object(src: &Path, dst: &Path) -> Result<(), String> {
    match std::fs::hard_link(src, dst) {
        Ok(()) => Ok(()),
        // raced/pre-existing destination: content-addressed, already identical.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => {
            let tmp = dst.with_extension("tmp");
            std::fs::copy(src, &tmp)
                .map_err(|e| format!("files scratch: copy {}: {e}", src.display()))?;
            std::fs::File::open(&tmp)
                .and_then(|file| file.sync_all())
                .map_err(|e| format!("files scratch: fsync {}: {e}", tmp.display()))?;
            std::fs::rename(&tmp, dst).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                format!("files scratch: publish {}: {e}", dst.display())
            })
        }
    }
}
