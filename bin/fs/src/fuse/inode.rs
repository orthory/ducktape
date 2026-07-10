//! the inode table shared by the read-only and read-write mounts: a stable
//! `ino <-> (parent, name)` map from which each mount builds its own path (a
//! duckfs path for the RO mount, a backing-dir path for the RW mount).
//!
//! inodes are assigned lazily as `lookup`/`readdir` discover names, and once
//! assigned they never move — the kernel caches an ino and expects it to keep
//! meaning the same object. a name is interned under its parent, so the same
//! (parent, name) always resolves to the same ino (idempotent lookups). storing
//! parent+name rather than an absolute path makes `rename` a single-node edit: a
//! directory's descendants recompute their paths through the parent chain, so
//! moving a dir moves its whole subtree with no bookkeeping.
//!
//! there is no inode GC (`forget` is ignored): the table grows with the number of
//! distinct paths touched during a mount's lifetime, which is bounded and cheap
//! for the wave-1 mount. the root is always ino 1.

use std::collections::HashMap;

/// the fixed root inode number the kernel starts every path resolution from.
pub const ROOT_INO: u64 = 1;

/// one interned node: the parent it hangs under and its own name. the root has
/// itself as parent and an empty name.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Node {
    parent: u64,
    name: String,
}

/// a lazily-grown `ino <-> (parent, name)` table. root is [`ROOT_INO`]; children
/// are interned on first sight and keep their ino for the mount's lifetime.
#[derive(Debug)]
pub struct Inodes {
    nodes: HashMap<u64, Node>,
    /// (parent ino, name) -> child ino, so interning the same name twice is
    /// idempotent.
    by_name: HashMap<(u64, String), u64>,
    next: u64,
}

impl Default for Inodes {
    fn default() -> Self {
        Self::new()
    }
}

impl Inodes {
    /// a fresh table holding only the root.
    pub fn new() -> Self {
        let mut nodes = HashMap::new();
        nodes.insert(
            ROOT_INO,
            Node {
                parent: ROOT_INO,
                name: String::new(),
            },
        );
        Inodes {
            nodes,
            by_name: HashMap::new(),
            next: ROOT_INO + 1,
        }
    }

    /// intern `name` under `parent`, returning its (stable) ino. the same
    /// (parent, name) always maps back to the same ino. `parent` must already
    /// exist (it does — you only ever intern under an ino the kernel handed you).
    pub fn intern(&mut self, parent: u64, name: &str) -> u64 {
        if let Some(&ino) = self.by_name.get(&(parent, name.to_string())) {
            return ino;
        }
        let ino = self.next;
        self.next += 1;
        self.nodes.insert(
            ino,
            Node {
                parent,
                name: name.to_string(),
            },
        );
        self.by_name.insert((parent, name.to_string()), ino);
        ino
    }

    /// does `ino` name a known node?
    #[allow(dead_code)]
    pub fn contains(&self, ino: u64) -> bool {
        self.nodes.contains_key(&ino)
    }

    /// the parent ino of `ino` (the root is its own parent), or `None` if `ino`
    /// is unknown.
    pub fn parent_of(&self, ino: u64) -> Option<u64> {
        self.nodes.get(&ino).map(|n| n.parent)
    }

    /// the root-to-leaf name segments of `ino` (empty for the root), or `None`
    /// when `ino` is unknown. each mount joins these onto its own base.
    pub fn segments(&self, ino: u64) -> Option<Vec<String>> {
        let mut out = Vec::new();
        let mut cur = ino;
        // walk to the root, collecting names; guard against a corrupt cycle with
        // a depth cap far above any real tree (path depth is capped at 128).
        for _ in 0..4096 {
            let node = self.nodes.get(&cur)?;
            if cur == ROOT_INO {
                out.reverse();
                return Some(out);
            }
            out.push(node.name.clone());
            cur = node.parent;
        }
        None
    }

    /// re-parent/rename an interned node (a `rename` across or within dirs). the
    /// node keeps its ino (the kernel's handle stays valid); its descendants
    /// recompute their paths through the new parent chain automatically. any node
    /// already interned at the destination name is evicted from the name index so
    /// the destination resolves to the moved node.
    pub fn rename(&mut self, ino: u64, new_parent: u64, new_name: &str) {
        let Some(node) = self.nodes.get(&ino) else {
            return;
        };
        let old_key = (node.parent, node.name.clone());
        // drop a clobbered destination entry (overwrite rename) from the name map.
        if let Some(&clobbered) = self.by_name.get(&(new_parent, new_name.to_string()))
            && clobbered != ino
        {
            self.by_name.remove(&(new_parent, new_name.to_string()));
        }
        self.by_name.remove(&old_key);
        self.by_name.insert((new_parent, new_name.to_string()), ino);
        if let Some(node) = self.nodes.get_mut(&ino) {
            node.parent = new_parent;
            node.name = new_name.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_ino_1_with_no_segments() {
        let t = Inodes::new();
        assert!(t.contains(ROOT_INO));
        assert_eq!(t.segments(ROOT_INO), Some(Vec::new()));
        assert_eq!(t.parent_of(ROOT_INO), Some(ROOT_INO));
    }

    #[test]
    fn intern_is_stable_and_idempotent() {
        let mut t = Inodes::new();
        let a = t.intern(ROOT_INO, "sub");
        let a2 = t.intern(ROOT_INO, "sub");
        assert_eq!(a, a2, "same (parent,name) → same ino");
        let b = t.intern(ROOT_INO, "other");
        assert_ne!(a, b, "distinct names → distinct inos");
        // a nested path threads through the parent chain.
        let child = t.intern(a, "child.txt");
        assert_eq!(
            t.segments(child),
            Some(vec!["sub".to_string(), "child.txt".to_string()])
        );
        assert_eq!(t.parent_of(child), Some(a));
    }

    #[test]
    fn unknown_ino_has_no_segments() {
        let t = Inodes::new();
        assert_eq!(t.segments(999), None);
        assert!(!t.contains(999));
    }

    #[test]
    fn rename_moves_a_node_and_its_subtree() {
        let mut t = Inodes::new();
        let dir_a = t.intern(ROOT_INO, "a");
        let dir_b = t.intern(ROOT_INO, "b");
        let child = t.intern(dir_a, "f.txt");
        assert_eq!(
            t.segments(child),
            Some(vec!["a".to_string(), "f.txt".to_string()])
        );

        // move a/ under b/ as a2/ — the child follows via the parent chain.
        t.rename(dir_a, dir_b, "a2");
        assert_eq!(
            t.segments(dir_a),
            Some(vec!["b".to_string(), "a2".to_string()])
        );
        assert_eq!(
            t.segments(child),
            Some(vec!["b".to_string(), "a2".to_string(), "f.txt".to_string()]),
            "a descendant recomputes its path through the moved parent"
        );
        // the old (root,"a") name no longer resolves; the new one does.
        assert_eq!(t.intern(dir_b, "a2"), dir_a, "destination resolves to it");
    }

    #[test]
    fn rename_clobbers_the_destination_name() {
        let mut t = Inodes::new();
        let src = t.intern(ROOT_INO, "src");
        let dst = t.intern(ROOT_INO, "dst");
        // overwrite dst with src: (root,"dst") must now map to src's ino.
        t.rename(src, ROOT_INO, "dst");
        assert_eq!(t.intern(ROOT_INO, "dst"), src);
        assert_ne!(src, dst);
    }
}
