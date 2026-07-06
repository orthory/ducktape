//! the read side (tasks 11/12): stat/ls/read/find/grep/history/diff/refs over
//! committed state, paged and byte-capped. task 9 lands `Stat`; task 11 adds
//! `Ls`/`Read`/`Refs`; the rest stay unimplemented until task 12. every read is
//! over COMMITTED state — the pending overlay never leaks into a query.

use std::collections::BTreeMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::fs::{Fs, refs_contains_snapshot};
use crate::objects::{EntryKind, FileObj, Kind, ObjectId, TreeObj};
use crate::paths::canonical;
use crate::store::ObjectStore;
use crate::tree::{Store, entry_at, snapshot_root_tree};
use crate::wire::{
    CHUNK_SIZE, EntryInfo, EntryKindWire, FilesQuery, FilesReply, MAX_PAGE, MAX_READ_BYTES,
    RefsInfo, from_hex_32, to_hex,
};

/// dispatch a committed-state query. task 9 serves `Stat`; task 11 serves
/// `Ls`/`Read`/`Refs`; the search/history reads land in task 12.
pub(crate) fn query<S: ObjectStore>(fs: &Fs<S>, q: FilesQuery) -> Result<FilesReply, String> {
    match q {
        FilesQuery::Stat { path, snapshot } => stat(fs, &path, snapshot.as_deref()),
        FilesQuery::Ls {
            path,
            snapshot,
            after,
            limit,
        } => ls(fs, &path, snapshot.as_deref(), after.as_deref(), limit),
        FilesQuery::Read {
            path,
            snapshot,
            offset,
            len,
        } => read(fs, &path, snapshot.as_deref(), offset, len),
        FilesQuery::Refs {} => refs(fs),
        _ => Err("files: query unimplemented".into()),
    }
}

/// resolve the committed snapshot a read runs against. `None` reads the committed
/// head; `Some(hex)` must resolve to the head, the bounded history window, or a
/// pin of the COMMITTED refs, else `snapshot not resolvable`. shared by every
/// snapshot-addressable read (stat/ls/read) so they agree on the membership rule.
fn resolve_head<S: ObjectStore>(
    fs: &Fs<S>,
    snapshot: Option<&str>,
) -> Result<Option<ObjectId>, String> {
    let refs = fs.refs_view();
    match snapshot {
        None => Ok(refs.head),
        Some(hex) => {
            let id =
                from_hex_32(hex).ok_or_else(|| "files: snapshot not resolvable".to_string())?;
            if !refs_contains_snapshot(refs, &id) {
                return Err("files: snapshot not resolvable".into());
            }
            Ok(Some(id))
        }
    }
}

/// open the committed-only read view (no pending overlay) plus the root tree of
/// the resolved snapshot. a `None` head (empty filesystem) yields `None` root.
fn committed_view<'a, S: ObjectStore>(
    fs: &'a Fs<S>,
    snapshot: Option<&str>,
) -> Result<(Store<'a>, Option<ObjectId>), String> {
    let head = resolve_head(fs, snapshot)?;
    let store = Store {
        store: fs.store_ref(),
        pending: &[],
    };
    let root_tree = match head {
        Some(snap) => Some(snapshot_root_tree(&store, &snap)?),
        None => None,
    };
    Ok((store, root_tree))
}

/// resolve one entry against committed state. the filesystem root (empty
/// segments) is a directory, not a tree ENTRY, so `stat("/")` is `None`.
fn stat<S: ObjectStore>(
    fs: &Fs<S>,
    path: &str,
    snapshot: Option<&str>,
) -> Result<FilesReply, String> {
    let (store, root_tree) = committed_view(fs, snapshot)?;
    let segs = canonical(path)?;
    if segs.is_empty() {
        return Ok(FilesReply::Stat(None));
    }
    let Some(entry) = entry_at(&store, root_tree, &segs)? else {
        return Ok(FilesReply::Stat(None));
    };
    Ok(FilesReply::Stat(Some(entry_info(
        &store,
        &format!("/{}", segs.join("/")),
        &entry,
    )?)))
}

/// list a directory's entries in name order, paged by a strictly-after cursor.
/// the path must resolve to a directory (or the root `/`, whose empty segment
/// list names the root dir listing directly); a file/symlink path is `not a
/// directory` and an absent path is `path not found`.
fn ls<S: ObjectStore>(
    fs: &Fs<S>,
    path: &str,
    snapshot: Option<&str>,
    after: Option<&str>,
    limit: u64,
) -> Result<FilesReply, String> {
    let (store, root_tree) = committed_view(fs, snapshot)?;
    let segs = canonical(path)?;

    // resolve the directory whose entries we list, and the joined prefix each
    // child path is built under. the root (empty segs) lists the root tree, which
    // is `None` (empty listing) on a fresh filesystem.
    let (dir_tree, base) = if segs.is_empty() {
        (root_tree, String::new())
    } else {
        match entry_at(&store, root_tree, &segs)? {
            None => return Err("files: path not found".into()),
            Some(entry) => match entry.kind {
                EntryKind::Dir => (Some(entry.id), format!("/{}", segs.join("/"))),
                // a file or symlink has no directory listing.
                EntryKind::File | EntryKind::Symlink => {
                    return Err("files: not a directory".into());
                }
            },
        }
    };

    // 0 is a useless page; the honest clamp is 1..=MAX_PAGE. BTreeMap iteration is
    // already strict ascending name order — exactly the cursor order.
    let limit = limit.min(MAX_PAGE).max(1) as usize;
    let entries_map = dir_entries(&store, dir_tree)?;
    let mut iter = entries_map.iter().filter(|(name, _)| match after {
        Some(a) => name.as_str() > a,
        None => true,
    });

    let mut entries = Vec::new();
    let mut last_name: Option<String> = None;
    for (name, entry) in iter.by_ref().take(limit) {
        last_name = Some(name.clone());
        entries.push(entry_info(&store, &format!("{base}/{name}"), entry)?);
    }
    // next is the last returned NAME iff at least one more entry follows the page
    // — never a phantom cursor when the page ends exactly at the listing's end.
    let next = if iter.next().is_some() {
        last_name
    } else {
        None
    };
    Ok(FilesReply::Ls { entries, next })
}

/// read a byte range of a file (or symlink target). a directory is `not a file`
/// and an absent path is `path not found`. `len` is clamped to [`MAX_READ_BYTES`]
/// (a 0-byte read is legal — empty result); a read at or past EOF returns the
/// empty suffix. `eof` is `offset + returned == size` (so it is true whenever the
/// read reaches the end, including any offset into an empty file).
fn read<S: ObjectStore>(
    fs: &Fs<S>,
    path: &str,
    snapshot: Option<&str>,
    offset: u64,
    len: u64,
) -> Result<FilesReply, String> {
    let (store, root_tree) = committed_view(fs, snapshot)?;
    let segs = canonical(path)?;
    // the filesystem root is a directory, never a file.
    if segs.is_empty() {
        return Err("files: not a file".into());
    }
    let entry = match entry_at(&store, root_tree, &segs)? {
        None => return Err("files: path not found".into()),
        Some(entry) => entry,
    };
    match entry.kind {
        EntryKind::File | EntryKind::Symlink => {}
        EntryKind::Dir => return Err("files: not a file".into()),
    }

    let file = load_fileobj(&store, &entry.id)?;
    let size = file.size;
    let len = len.min(MAX_READ_BYTES);
    let bytes = read_range(&store, &file, offset, len)?;
    let eof = offset.saturating_add(bytes.len() as u64) >= size;
    Ok(FilesReply::Read {
        b64: STANDARD.encode(&bytes),
        eof,
    })
}

/// the committed refs summary: head hex, name→snapshot pin map, and the current
/// bounded history window length.
fn refs<S: ObjectStore>(fs: &Fs<S>) -> Result<FilesReply, String> {
    let refs = fs.refs_view();
    let pins = refs
        .pins
        .iter()
        .map(|(name, entry)| (name.clone(), to_hex(&entry.snapshot)))
        .collect();
    Ok(FilesReply::Refs(RefsInfo {
        head: refs.head.as_ref().map(|h| to_hex(h)),
        pins,
        window_len: refs.window.len() as u64,
    }))
}

// ---- helpers ----------------------------------------------------------------

/// build the wire [`EntryInfo`] for a tree entry at `path`: kind mapped, size and
/// exec verbatim, id as hex, and — for a file/symlink — the meta decoded from its
/// committed FileObj (a directory carries no FileObj, so its meta is empty).
fn entry_info(
    store: &Store,
    path: &str,
    entry: &crate::objects::TreeEntry,
) -> Result<EntryInfo, String> {
    let meta = match entry.kind {
        EntryKind::File | EntryKind::Symlink => load_fileobj(store, &entry.id)?.meta,
        EntryKind::Dir => BTreeMap::new(),
    };
    Ok(EntryInfo {
        path: path.to_string(),
        kind: wire_kind(entry.kind),
        size: entry.size,
        exec: entry.exec,
        object: to_hex(&entry.id),
        meta,
    })
}

/// decode the directory tree at `dir_tree` into its name-keyed entries. a `None`
/// tree (the root of a fresh filesystem) lists nothing.
fn dir_entries(
    store: &Store,
    dir_tree: Option<ObjectId>,
) -> Result<BTreeMap<String, crate::objects::TreeEntry>, String> {
    let Some(id) = dir_tree else {
        return Ok(BTreeMap::new());
    };
    let (kind, body) = store
        .get(&id)?
        .ok_or_else(|| "files: tree object missing from store".to_string())?;
    if kind != Kind::Tree {
        return Err("files: expected a tree object".into());
    }
    Ok(TreeObj::decode(&body)?.entries)
}

/// decode the FileObj at `id` from the committed store (a file or symlink leaf).
fn load_fileobj(store: &Store, id: &ObjectId) -> Result<FileObj, String> {
    let (kind, body) = store
        .get(id)?
        .ok_or_else(|| "files: file object missing from store".to_string())?;
    if kind != Kind::File {
        return Err("files: expected a file object".into());
    }
    FileObj::decode(&body)
}

/// reassemble the byte range `[offset, offset+len)` of `file`, clipped to the
/// file size, by fetching only the chunks the range spans (chunk index =
/// byte / CHUNK_SIZE). a read at or past EOF is empty. a chunk object that is
/// absent from the committed store is a hard error (never silent truncation —
/// full replication means every committed chunk is present).
fn read_range(store: &Store, file: &FileObj, offset: u64, len: u64) -> Result<Vec<u8>, String> {
    let size = file.size;
    if offset >= size || len == 0 {
        return Ok(Vec::new());
    }
    let end = offset.saturating_add(len).min(size); // exclusive, clipped to EOF
    let first = (offset / CHUNK_SIZE) as usize;
    let last = ((end - 1) / CHUNK_SIZE) as usize;
    let mut out = Vec::with_capacity((end - offset) as usize);
    for index in first..=last {
        let chunk_id = file.chunks.get(index).ok_or_else(|| {
            "files: file references fewer chunks than its size implies".to_string()
        })?;
        let (kind, body) = store
            .get(chunk_id)?
            .ok_or_else(|| format!("files: chunk object missing: {}", to_hex(chunk_id)))?;
        if kind != Kind::Chunk {
            return Err("files: expected a chunk object".into());
        }
        // intersect the requested range with this chunk's byte span and copy it.
        let chunk_start = index as u64 * CHUNK_SIZE;
        let chunk_end = chunk_start + body.len() as u64;
        let lo = offset.max(chunk_start);
        let hi = end.min(chunk_end);
        if lo < hi {
            out.extend_from_slice(&body[(lo - chunk_start) as usize..(hi - chunk_start) as usize]);
        }
    }
    Ok(out)
}

fn wire_kind(kind: EntryKind) -> EntryKindWire {
    match kind {
        EntryKind::File => EntryKindWire::File,
        EntryKind::Dir => EntryKindWire::Dir,
        EntryKind::Symlink => EntryKindWire::Symlink,
    }
}
