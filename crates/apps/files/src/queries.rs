//! the read side (tasks 11/12): stat/ls/read/find/grep/history/diff/refs over
//! committed state, paged and byte-capped. task 9 lands `Stat` only; the rest
//! stay unimplemented until task 11. every read is over COMMITTED state — the
//! pending overlay never leaks into a query.

use std::collections::BTreeMap;

use crate::fs::{Fs, refs_contains_snapshot};
use crate::objects::{EntryKind, FileObj, Kind};
use crate::paths::canonical;
use crate::store::ObjectStore;
use crate::tree::{Store, entry_at, snapshot_root_tree};
use crate::wire::{EntryInfo, EntryKindWire, FilesQuery, FilesReply, from_hex_32, to_hex};

/// dispatch a committed-state query. task 9 serves `Stat`; the rest are wired in
/// task 11.
pub(crate) fn query<S: ObjectStore>(fs: &Fs<S>, q: FilesQuery) -> Result<FilesReply, String> {
    match q {
        FilesQuery::Stat { path, snapshot } => stat(fs, &path, snapshot.as_deref()),
        _ => Err("files: query unimplemented".into()),
    }
}

/// resolve one entry against committed state. `snapshot: None` reads the
/// committed head; `Some(hex)` must resolve to the head, the window, or a pin of
/// the COMMITTED refs, else `snapshot not resolvable`. the filesystem root
/// (empty segments) is a directory, not a tree ENTRY, so `stat("/")` is `None`.
fn stat<S: ObjectStore>(
    fs: &Fs<S>,
    path: &str,
    snapshot: Option<&str>,
) -> Result<FilesReply, String> {
    let refs = fs.refs_view();
    let head = match snapshot {
        None => refs.head,
        Some(hex) => {
            let id =
                from_hex_32(hex).ok_or_else(|| "files: snapshot not resolvable".to_string())?;
            if !refs_contains_snapshot(refs, &id) {
                return Err("files: snapshot not resolvable".into());
            }
            Some(id)
        }
    };
    let store = Store {
        store: fs.store_ref(),
        pending: &[],
    };
    // no head (empty filesystem, or an empty committed state) resolves nothing.
    let root_tree = match head {
        Some(snap) => Some(snapshot_root_tree(&store, &snap)?),
        None => None,
    };
    let segs = canonical(path)?;
    if segs.is_empty() {
        return Ok(FilesReply::Stat(None));
    }
    let Some(entry) = entry_at(&store, root_tree, &segs)? else {
        return Ok(FilesReply::Stat(None));
    };
    // a file/symlink carries meta in its FileObj; a directory has none.
    let meta = match entry.kind {
        EntryKind::File | EntryKind::Symlink => decode_file_meta(&store, &entry.id)?,
        EntryKind::Dir => BTreeMap::new(),
    };
    Ok(FilesReply::Stat(Some(EntryInfo {
        path: format!("/{}", segs.join("/")),
        kind: wire_kind(entry.kind),
        size: entry.size,
        exec: entry.exec,
        object: to_hex(&entry.id),
        meta,
    })))
}

/// decode the meta map of the FileObj at `id` (committed store).
fn decode_file_meta(
    store: &Store,
    id: &crate::objects::ObjectId,
) -> Result<BTreeMap<String, String>, String> {
    let (kind, body) = store
        .get(id)?
        .ok_or_else(|| "files: file object missing from store".to_string())?;
    if kind != Kind::File {
        return Err("files: expected a file object".into());
    }
    Ok(FileObj::decode(&body)?.meta)
}

fn wire_kind(kind: EntryKind) -> EntryKindWire {
    match kind {
        EntryKind::File => EntryKindWire::File,
        EntryKind::Dir => EntryKindWire::Dir,
        EntryKind::Symlink => EntryKindWire::Symlink,
    }
}
