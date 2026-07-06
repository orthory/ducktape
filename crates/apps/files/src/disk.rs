//! disk persistence: the content-addressed odb as [`DiskStore`] (task 5) and
//! the refs-file envelope as [`DiskRefs`] (task 6, still a stub).
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

use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::objects::{Kind, ObjectId, object_id};
use crate::state::Refs;
use crate::store::{ObjectStore, RefsStore};
use crate::wire::{from_hex_32, to_hex};

fn unimplemented_err() -> String {
    "files: unimplemented".into()
}

/// content-addressed object database over `dir/<aa>/<hex[2..]>` files. each file
/// is `[kind u8] ‖ body`; the filename is the 64-char lowercase-hex id.
pub struct DiskStore {
    dir: PathBuf,
}

impl DiskStore {
    /// create the odb root and sweep any `*.tmp` crash debris under it. a fresh
    /// dir is created lazily; fanout subdirs appear on first `put` into them.
    pub fn open(dir: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        sweep_tmp(&dir)?;
        Ok(Self { dir })
    }

    /// the on-disk path for an id: `dir/<aa>/<hex[2..]>`. returned with the hex
    /// so callers can build error context without re-encoding.
    fn object_path(&self, id: &ObjectId) -> (String, PathBuf) {
        let hex = to_hex(id);
        let path = self.dir.join(&hex[..2]).join(&hex[2..]);
        (hex, path)
    }
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
        // content-addressed + idempotent: an existing file is already exactly
        // these bytes, so re-putting is a no-op (and cheap — no rewrite).
        if dest.exists() {
            return Ok(id);
        }
        let subdir = self.dir.join(&hex[..2]);
        std::fs::create_dir_all(&subdir).map_err(|e| format!("odb put {hex}: mkdir: {e}"))?;
        // tmp lives in the destination subdir so the rename below is same-dir
        // (and therefore atomic). the full hex keeps the tmp name unique.
        let tmp = subdir.join(format!("{hex}.tmp"));
        let mut buf = Vec::with_capacity(1 + body.len());
        buf.push(kind.tag());
        buf.extend_from_slice(body);
        // scope the file so it is closed before the rename on every platform.
        {
            let mut f = std::fs::File::create(&tmp)
                .map_err(|e| format!("odb put {hex}: create tmp: {e}"))?;
            f.write_all(&buf)
                .map_err(|e| format!("odb put {hex}: write tmp: {e}"))?;
            // durable before publish: the bytes must hit disk before the rename
            // makes them reachable under the content-addressed name.
            f.sync_all()
                .map_err(|e| format!("odb put {hex}: fsync tmp: {e}"))?;
        }
        std::fs::rename(&tmp, &dest).map_err(|e| {
            // a failed publish must not leave debris under the object name.
            let _ = std::fs::remove_file(&tmp);
            format!("odb put {hex}: rename: {e}")
        })?;
        Ok(id)
    }

    fn get(&self, id: &ObjectId) -> Result<Option<(Kind, Vec<u8>)>, String> {
        let (hex, path) = self.object_path(id);
        let raw = match std::fs::read(&path) {
            Ok(raw) => raw,
            // absent is Ok(None), sharply distinct from a corrupt Err below.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("odb get {hex}: {e}")),
        };
        let (tag, body) = raw
            .split_first()
            .ok_or_else(|| format!("odb get {hex}: object file is empty"))?;
        let kind =
            Kind::from_u8(*tag).ok_or_else(|| format!("odb get {hex}: unknown kind tag {tag}"))?;
        // re-derive and verify: the disk is untrusted, so a bit-flip must surface
        // as an error rather than return wrong bytes under a trusted id.
        if object_id(kind, body) != *id {
            return Err(format!(
                "odb get {hex}: content hash mismatch (corrupt object)"
            ));
        }
        Ok(Some((kind, body.to_vec())))
    }

    fn has(&self, id: &ObjectId) -> bool {
        self.object_path(id).1.exists()
    }

    fn remove(&mut self, id: &ObjectId) -> Result<(), String> {
        let (hex, path) = self.object_path(id);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            // a missing object is already removed — idempotent, not an error.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("odb remove {hex}: {e}")),
        }
    }

    fn list(&self) -> Result<Vec<ObjectId>, String> {
        let mut out = Vec::new();
        let top = match std::fs::read_dir(&self.dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(format!("odb list: {e}")),
        };
        for aa_entry in top {
            let aa_entry = aa_entry.map_err(|e| format!("odb list: {e}"))?;
            let aa_name = aa_entry.file_name();
            let Some(aa) = aa_name.to_str() else { continue };
            // fanout dirs are exactly two hex chars; anything else is foreign.
            if aa.len() != 2 {
                continue;
            }
            if !aa_entry
                .file_type()
                .map_err(|e| format!("odb list: {e}"))?
                .is_dir()
            {
                continue;
            }
            for f_entry in
                std::fs::read_dir(aa_entry.path()).map_err(|e| format!("odb list: {e}"))?
            {
                let f_entry = f_entry.map_err(|e| format!("odb list: {e}"))?;
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

/// the atomically-replaced refs file under `dir/refs` (task 6).
pub struct DiskRefs {
    /// module data dir holding the refs file; task 6 adds the envelope codec.
    #[allow(dead_code)]
    dir: PathBuf,
}

impl DiskRefs {
    pub fn open(dir: PathBuf) -> Result<Self, String> {
        Ok(Self { dir })
    }
}

impl RefsStore for DiskRefs {
    fn load(&self) -> Result<Option<(Refs, u64, u64)>, String> {
        Err(unimplemented_err())
    }

    fn save(&mut self, _refs: &Refs, _height: u64, _gc_watermark: u64) -> Result<(), String> {
        Err(unimplemented_err())
    }
}
