//! copy-on-write tree editing (task 8): path walks over tree objects, rewrites
//! along the touched spine only, and the lazy overlay the commit executor (task
//! 9) drives.
//!
//! why lazy matters — the whole point of this engine. a [`TreeEdit`] loads the
//! root as a single unexpanded reference and only ever decodes the directories
//! on a path an op actually touches. an untouched subtree stays a
//! [`Node::Ref`], and at [`TreeEdit::build`] a `Ref` re-emits its existing
//! object id verbatim — never re-encoded. so an edit's cost is O(touched
//! spine), never O(namespace): touching one file in a million-entry tree
//! rewrites only that file's chain of parent directories, and every sibling
//! subtree is shared across versions by hash. that structural sharing is the
//! bedrock the storage model and gc (task 13) stand on — the untouched bytes of
//! version N are literally the same objects as version N-1.
//!
//! purity: no sdk, no `std::fs`, no async — this compiles under
//! `--no-default-features` as part of the future wasm core.

use std::collections::BTreeMap;

use crate::MAX_DIR_ENTRIES;
use crate::objects::{EntryKind, Kind, ObjectId, SnapshotObj, TreeEntry, TreeObj, object_id};
use crate::store::ObjectStore;

/// the read view onto stored objects for a single edit: the backing object
/// store plus the block's not-yet-flushed staged objects. `pending` is checked
/// FIRST — an in-block chained commit (task 9 applies changes in order, and a
/// later change reads a tree a prior change in the SAME block just produced)
/// references objects that have not been flushed to the store yet.
///
/// `&dyn ObjectStore` (not a generic) keeps this type — and every walk over it —
/// monomorphization-free and object-safe across `MemStore`/`DiskStore`; the
/// trait is object-safe (no generics, no `Self`-returning methods) so the `dyn`
/// costs nothing here but a vtable hop on a cold path.
pub struct Store<'a> {
    pub store: &'a dyn ObjectStore,
    pub pending: &'a [(Kind, Vec<u8>)],
}

impl Store<'_> {
    /// fetch by id, pending buffer first. the pending scan re-derives each id
    /// with [`object_id`] — a linear scan, fine at this stage because a block's
    /// pending set is small; task 9 may want an index here if a single block
    /// ever chains many commits (coordinate with putblob's staging dedup, which
    /// has the same "hash the buffered bytes" shape).
    pub(crate) fn get(&self, id: &ObjectId) -> Result<Option<(Kind, Vec<u8>)>, String> {
        for (kind, body) in self.pending {
            if object_id(*kind, body) == *id {
                return Ok(Some((*kind, body.clone())));
            }
        }
        self.store.get(id)
    }
}

/// decode the snapshot object and hand back the root tree it commits.
pub fn snapshot_root_tree(store: &Store, snapshot: &ObjectId) -> Result<ObjectId, String> {
    let (kind, body) = store
        .get(snapshot)?
        .ok_or_else(|| "files: snapshot object missing from store".to_string())?;
    if kind != Kind::Snapshot {
        return Err("files: expected a snapshot object".into());
    }
    Ok(SnapshotObj::decode(&body)?.root)
}

/// resolve `segs` against the committed tree rooted at `root_tree`, decoding one
/// directory per segment. `None` root is the empty tree (nothing resolves).
/// empty `segs` names the root directory itself, which is not a tree ENTRY (it
/// has no parent record) — callers that need the root handle it separately, so
/// this returns `None`.
pub fn entry_at(
    store: &Store,
    root_tree: Option<ObjectId>,
    segs: &[String],
) -> Result<Option<TreeEntry>, String> {
    let Some(root) = root_tree else {
        return Ok(None);
    };
    if segs.is_empty() {
        return Ok(None);
    }
    let mut dir = root;
    for (i, seg) in segs.iter().enumerate() {
        let entries = fetch_tree(store, &dir)?;
        let Some(entry) = entries.get(seg) else {
            return Ok(None);
        };
        if i + 1 == segs.len() {
            return Ok(Some(*entry));
        }
        // more segments remain: descent is only possible through a directory.
        if entry.kind != EntryKind::Dir {
            return Ok(None);
        }
        dir = entry.id;
    }
    // unreachable — `segs` is non-empty, so the last iteration always returns.
    Ok(None)
}

/// one node in the in-memory edit overlay. a `Ref` is a not-yet-loaded entry
/// pointing at a stored object by id (a file/symlink leaf, or a whole directory
/// subtree still on disk); a `Dir` is a directory that some op forced into
/// memory, holding its children (each itself a `Ref` until touched). only nodes
/// on a touched path are ever `Dir`.
pub enum Node {
    Ref(TreeEntry),
    Dir(BTreeMap<String, Node>),
}

/// a lazy copy-on-write edit over a tree. the root starts as a single `Node`
/// (an unexpanded `Ref` to the base root tree, or an empty `Dir` for a fresh
/// filesystem) and materializes only along touched paths.
pub struct TreeEdit {
    root: Node,
}

impl TreeEdit {
    /// open an edit over the tree rooted at `root_tree`. this decodes NOTHING —
    /// the root is held as a single unexpanded `Ref` and the first op that
    /// touches a path drives the decode. a `None` root is a fresh, empty
    /// filesystem.
    pub fn load(_store: &Store, root_tree: Option<ObjectId>) -> TreeEdit {
        let root = match root_tree {
            // the root's `size` is a placeholder: the root has no parent tree
            // entry, so this field is never encoded or read — `build` returns
            // the root id, not an entry.
            Some(id) => Node::Ref(TreeEntry {
                kind: EntryKind::Dir,
                id,
                exec: false,
                size: 0,
            }),
            None => Node::Dir(BTreeMap::new()),
        };
        TreeEdit { root }
    }

    /// place `entry` at `segs`, auto-creating any missing parent directories.
    /// rejects if a non-directory sits on the path to the target (a file in the
    /// way of a descent). replaces whatever entry currently sits at the final
    /// segment — the commit executor (task 9) layers any higher-level policy.
    pub fn put(&mut self, store: &Store, segs: &[String], entry: TreeEntry) -> Result<(), String> {
        let (name, dirs) = segs
            .split_last()
            .ok_or_else(|| "files: cannot put the root itself".to_string())?;
        let parent = navigate(&mut self.root, store, dirs, true)?;
        parent.insert(name.clone(), Node::Ref(entry));
        Ok(())
    }

    /// create an empty directory at `segs`, auto-creating missing parents.
    /// rejects if anything already exists at the target.
    pub fn mkdir(&mut self, store: &Store, segs: &[String]) -> Result<(), String> {
        let (name, dirs) = segs
            .split_last()
            .ok_or_else(|| "files: cannot mkdir the root itself".to_string())?;
        let parent = navigate(&mut self.root, store, dirs, true)?;
        if parent.contains_key(name) {
            return Err("files: mkdir target already exists".into());
        }
        parent.insert(name.clone(), Node::Dir(BTreeMap::new()));
        Ok(())
    }

    /// remove the entry at `segs` — a file, a symlink, or a whole subtree.
    /// rejects if the target (or any parent on the way) does not exist; missing
    /// parents are NOT created for a removal.
    pub fn rm(&mut self, store: &Store, segs: &[String]) -> Result<(), String> {
        let (name, dirs) = segs
            .split_last()
            .ok_or_else(|| "files: cannot rm the root itself".to_string())?;
        let parent = navigate(&mut self.root, store, dirs, false)?;
        if parent.remove(name).is_none() {
            return Err("files: rm target does not exist".into());
        }
        Ok(())
    }

    /// read the entry at `segs` as the edit currently sees it (overlay first,
    /// then decode on descent). `None` for an absent path or the root itself.
    /// the commit executor composes `Mv` as get + rm + put on top of this.
    pub fn get(&self, store: &Store, segs: &[String]) -> Result<Option<TreeEntry>, String> {
        if segs.is_empty() {
            return Ok(None);
        }
        walk_get(store, &self.root, segs)
    }

    /// finalize the edit into stored tree objects. post-order: every `Dir` on a
    /// touched path is encoded as a [`TreeObj`] (deterministic — `BTreeMap`
    /// yields the strict ascending name order the codec re-checks), capped at
    /// [`MAX_DIR_ENTRIES`], and pushed to `out` as `(Kind::Tree, body)`; every
    /// untouched `Ref` re-emits its existing id with NO re-encode. returns the
    /// new root tree id, or `None` for a completely empty root (the empty
    /// filesystem, which has no root object).
    pub fn build(self, out: &mut Vec<(Kind, Vec<u8>)>) -> Result<Option<ObjectId>, String> {
        match self.root {
            // nothing was touched: the whole tree is reused by its existing id.
            Node::Ref(entry) => Ok(Some(entry.id)),
            Node::Dir(children) => {
                if children.is_empty() {
                    // an empty root is the empty filesystem — no root object.
                    return Ok(None);
                }
                let entry = build_dir(children, out)?;
                Ok(Some(entry.id))
            }
        }
    }
}

// ---- internals --------------------------------------------------------------

/// decode the tree object at `id` into its entries.
fn fetch_tree(store: &Store, id: &ObjectId) -> Result<BTreeMap<String, TreeEntry>, String> {
    let (kind, body) = store
        .get(id)?
        .ok_or_else(|| "files: tree object missing from store".to_string())?;
    if kind != Kind::Tree {
        return Err("files: expected a tree object".into());
    }
    Ok(TreeObj::decode(&body)?.entries)
}

/// decode the directory at `id` into overlay nodes — every child a lazy `Ref`.
fn load_children(store: &Store, id: &ObjectId) -> Result<BTreeMap<String, Node>, String> {
    Ok(fetch_tree(store, id)?
        .into_iter()
        .map(|(name, entry)| (name, Node::Ref(entry)))
        .collect())
}

/// force `node` to be a `Dir` in place: a `Ref` to a directory is decoded into
/// its children; a `Ref` to a file or symlink is a descent into a non-directory
/// and rejects; an already-materialized `Dir` is left alone.
fn materialize(store: &Store, node: &mut Node) -> Result<(), String> {
    if let Node::Ref(entry) = node {
        if entry.kind != EntryKind::Dir {
            return Err("files: a file or symlink is in the way of a directory path".into());
        }
        let children = load_children(store, &entry.id)?;
        *node = Node::Dir(children);
    }
    Ok(())
}

/// descend to the directory that should CONTAIN the final path segment,
/// materializing (decoding) each directory on the way. with `create`, a missing
/// intermediate directory is created empty (put/mkdir semantics); without it, a
/// missing one rejects (rm semantics). returns the parent directory's children
/// map.
fn navigate<'e>(
    root: &'e mut Node,
    store: &Store,
    dirs: &[String],
    create: bool,
) -> Result<&'e mut BTreeMap<String, Node>, String> {
    let mut cur: &'e mut Node = root;
    for seg in dirs {
        materialize(store, cur)?;
        // inline match (not a helper returning the borrow) so the reborrow that
        // reassigns `cur` below threads cleanly through the loop under NLL.
        let map = match cur {
            Node::Dir(map) => map,
            Node::Ref(_) => unreachable!("materialize made this a Dir"),
        };
        if !map.contains_key(seg) {
            if !create {
                return Err("files: a directory on the path does not exist".into());
            }
            map.insert(seg.clone(), Node::Dir(BTreeMap::new()));
        }
        cur = map.get_mut(seg).expect("ensured present above");
    }
    materialize(store, cur)?;
    match cur {
        Node::Dir(map) => Ok(map),
        Node::Ref(_) => unreachable!("materialize made this a Dir"),
    }
}

/// read-only descent for `get`: resolve `segs` against `node` without mutating
/// the overlay, decoding `Ref` directories transiently as needed.
fn walk_get(store: &Store, node: &Node, segs: &[String]) -> Result<Option<TreeEntry>, String> {
    let Some((head, rest)) = segs.split_first() else {
        return Ok(None);
    };
    match node {
        Node::Dir(map) => match map.get(head) {
            Some(child) => step_get(store, child, rest),
            None => Ok(None),
        },
        Node::Ref(entry) => {
            // a leaf mid-path cannot be descended into.
            if entry.kind != EntryKind::Dir {
                return Ok(None);
            }
            let children = load_children(store, &entry.id)?;
            match children.get(head) {
                Some(child) => step_get(store, child, rest),
                None => Ok(None),
            }
        }
    }
}

/// either return the resolved entry (`rest` empty) or keep descending.
fn step_get(store: &Store, child: &Node, rest: &[String]) -> Result<Option<TreeEntry>, String> {
    if rest.is_empty() {
        match child {
            Node::Ref(entry) => Ok(Some(*entry)),
            // a directory the edit already materialized: report its content id
            // as it currently stands (a pure read-only re-encode, nothing
            // pushed). the objects behind this id are only staged at `build`, so
            // the executor must move a modified subtree by node, not by
            // get-then-put — see the module note in task 9.
            Node::Dir(_) => Ok(Some(dir_entry_readonly(child)?)),
        }
    } else {
        walk_get(store, child, rest)
    }
}

/// consume a materialized directory into a stored [`TreeObj`], pushing it (and,
/// post-order, every materialized directory beneath it) to `out`. an untouched
/// `Ref` child is re-emitted by id with no re-encode — the CoW reuse. returns
/// the directory's own tree entry (`size` = its direct entry count).
fn build_dir(
    children: BTreeMap<String, Node>,
    out: &mut Vec<(Kind, Vec<u8>)>,
) -> Result<TreeEntry, String> {
    let mut entries: BTreeMap<String, TreeEntry> = BTreeMap::new();
    for (name, child) in children {
        let entry = match child {
            Node::Ref(entry) => entry,
            Node::Dir(grandchildren) => build_dir(grandchildren, out)?,
        };
        entries.insert(name, entry);
    }
    if entries.len() > MAX_DIR_ENTRIES {
        return Err("files: directory exceeds the maximum entry count".into());
    }
    let size = entries.len() as u64;
    let body = TreeObj { entries }.encode();
    let id = object_id(Kind::Tree, &body);
    out.push((Kind::Tree, body));
    Ok(TreeEntry {
        kind: EntryKind::Dir,
        id,
        exec: false,
        size,
    })
}

/// the id/size a materialized directory would encode to, WITHOUT staging any
/// object — the read-only twin of [`build_dir`] used only by `get`.
fn dir_entry_readonly(node: &Node) -> Result<TreeEntry, String> {
    match node {
        Node::Ref(entry) => Ok(*entry),
        Node::Dir(map) => {
            let mut entries: BTreeMap<String, TreeEntry> = BTreeMap::new();
            for (name, child) in map {
                entries.insert(name.clone(), dir_entry_readonly(child)?);
            }
            if entries.len() > MAX_DIR_ENTRIES {
                return Err("files: directory exceeds the maximum entry count".into());
            }
            let size = entries.len() as u64;
            let body = TreeObj { entries }.encode();
            let id = object_id(Kind::Tree, &body);
            Ok(TreeEntry {
                kind: EntryKind::Dir,
                id,
                exec: false,
                size,
            })
        }
    }
}
