use std::sync::Arc;

use uid::Identify;

use crate::{entry::Entry, op::Op, workspace::Workspace};

use hydration::Hydratable;

impl Hydratable for Workspace {
    type Op = crate::op::Op;

    // hydrate folds a stream of workspace ops into local tree state. this is the
    // ONE allowed `&mut self` method on the tree (it's the trait signature) — it
    // mutates the shared `Arc<Entry>` in place via `Arc::make_mut`, which clones
    // only if the arc is shared, otherwise mutates the unique owner directly.
    fn hydrate(&mut self, op: impl Iterator<Item = Self::Op>) {
        let root = Arc::make_mut(self.root_mut());
        for o in op {
            apply_one(root, o);
        }
    }
}

fn apply_one(root: &mut Entry, op: Op) {
    match op {
        // intra-document edit: find the file entry whose document carries
        // `entry_id` (= the document/frontmatter uid) and forward the op.
        Op::EntryMut { entry_id, op } => {
            if let Some(doc) = find_document_mut(root, entry_id) {
                // local apply; ignore the op result here — convergence is the
                // caller's concern, and a rejected op is a no-op on state.
                let _ = doc.apply(op);
            }
        }

        // structural: drop the new entry at an explicit slash-delimited path.
        // intermediate segments are find-or-created as directories; the final
        // segment is the basename the entry lands under. an empty path is a
        // no-op, and if an intermediate segment collides with an existing
        // `File` we bail rather than clobber it. callers mint the path upstream
        // so every node inserts at the same place — that's what keeps the tree
        // convergent across peers.
        Op::AddEntry { path, entry } => {
            let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            insert_at_path(root, &segments, entry);
        }

        // structural: remove the file entry whose document uid matches.
        Op::RemoveEntry { entry_id } => {
            remove_document(root, entry_id);
        }

        // structural: relocate the entry at `from` to `to`. paths are
        // slash-delimited, same shape `Workspace::get_entries` accepts.
        Op::MoveEntry { from, to, .. } => {
            if let Some((name, detached)) = detach_at_path(root, &from) {
                let to_name = to
                    .rsplit('/')
                    .find(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or(name);
                if let Entry::Directory(items) = root {
                    items.push((to_name, detached));
                }
            }
        }
    }
}

/// insert `entry` at a slash-delimited path under `current`, creating missing
/// intermediate directories as we go. the final segment is the basename. a
/// no-op if `segments` is empty, if `current` isn't a directory, or if an
/// intermediate segment collides with an existing `File` (we won't clobber it).
fn insert_at_path(current: &mut Entry, segments: &[&str], entry: Entry) {
    let Entry::Directory(items) = current else {
        return;
    };
    let Some((head, rest)) = segments.split_first() else {
        return;
    };

    // leaf: push the entry under its basename.
    if rest.is_empty() {
        items.push((head.to_string(), entry));
        return;
    }

    // intermediate: find-or-create the child directory, then recurse.
    let idx = match items.iter().position(|(name, _)| name == head) {
        Some(i) => i,
        None => {
            items.push((head.to_string(), Entry::Directory(Vec::new())));
            items.len() - 1
        }
    };
    insert_at_path(&mut items[idx].1, rest, entry);
}

/// depth-first search for the `Entry::File` whose document uid equals `id`,
/// returning a mutable borrow of the inner `Document`.
fn find_document_mut(entry: &mut Entry, id: uid::Uid) -> Option<&mut document::Document> {
    match entry {
        Entry::File(doc) => (doc.uid() == id).then_some(doc),
        Entry::Directory(items) => items
            .iter_mut()
            .find_map(|(_, child)| find_document_mut(child, id)),
    }
}

/// remove the `Entry::File` whose document uid equals `id` from anywhere in the
/// tree, mending the parent directory's child list. returns true if removed.
fn remove_document(entry: &mut Entry, id: uid::Uid) -> bool {
    let Entry::Directory(items) = entry else {
        return false;
    };
    if let Some(pos) = items.iter().position(
        |(_, child)| matches!(child, Entry::File(doc) if doc.uid() == id),
    ) {
        items.remove(pos);
        return true;
    }
    items.iter_mut().any(|(_, child)| remove_document(child, id))
}

/// detach (remove + return) the entry at a slash-delimited path, along with its
/// basename. only walks the directory whose child matches the final segment.
fn detach_at_path(root: &mut Entry, path: &str) -> Option<(String, Entry)> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    detach_in_recursion(root, &segments)
}

fn detach_in_recursion(current: &mut Entry, segments: &[&str]) -> Option<(String, Entry)> {
    let Entry::Directory(items) = current else {
        return None;
    };
    let head = *segments.first()?;
    let idx = items.iter().position(|(name, _)| name == head)?;

    if segments.len() == 1 {
        let (name, entry) = items.remove(idx);
        return Some((name, entry));
    }
    detach_in_recursion(&mut items[idx].1, &segments[1..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use document::Document;
    use document::op::{Op as DocOp, OpId};
    use nodes::Nodes;

    // a minimal valid document: frontmatter only. the parser mints the uids, so
    // callers read the real document uid back via `Document::uid()` rather than
    // choosing it. the frontmatter's `title` is the editable text buffer.
    const SAMPLE: &str = "---\ntitle: t\nauthor: @a\ncreated_at: 1\nupdated_at: 1\n---\n";

    fn sample_doc() -> Document {
        Document::from_reader(SAMPLE.as_bytes()).expect("parse")
    }

    fn title_of(ws: &Workspace, doc_id: uid::Uid) -> String {
        fn find(entry: &Entry, id: uid::Uid) -> Option<String> {
            match entry {
                Entry::File(doc) => {
                    if doc.uid() == id {
                        doc.nodes_iter().find_map(|n| match n {
                            Nodes::Frontmatter(f) => Some(f.title),
                            _ => None,
                        })
                    } else {
                        None
                    }
                }
                Entry::Directory(items) => {
                    items.iter().find_map(|(_, c)| find(c, id))
                }
            }
        }
        find(ws.root(), doc_id).expect("document present")
    }

    #[test]
    fn hydrate_entry_mut_edits_target_document() {
        let doc = sample_doc();
        let doc_id = doc.uid();

        let mut ws = Workspace::new_from_entry(Entry::Directory(vec![(
            "a.md".into(),
            Entry::File(doc),
        )]));

        // the frontmatter node IS the document root, so its node uid == doc uid.
        // writing into its editable text inserts at the front of the title "t".
        let ops = vec![Op::EntryMut {
            entry_id: doc_id,
            op: DocOp::OnUserWrite {
                op_id: OpId::new(1, 1),
                node_id: doc_id,
                pos: 0,
                text: "hello ".into(),
            },
        }];

        ws.hydrate(ops.into_iter());

        assert_eq!(title_of(&ws, doc_id), "hello t");
    }

    #[test]
    fn hydrate_add_entry_lands_at_nested_path() {
        let doc = sample_doc();
        let doc_id = doc.uid();
        // start from an empty root so the "a/" parent dir must be created.
        let mut ws = Workspace::new_from_entry(Entry::Directory(Vec::new()));

        ws.hydrate(std::iter::once(Op::AddEntry {
            path: "a/b.md".into(),
            entry: Entry::File(doc),
        }));

        // retrievable at exactly the path we asked for...
        let found = ws.get_entries("a/b.md".into()).expect("entry present");
        assert!(matches!(found, Entry::File(_)));
        // ...and it's the document we inserted.
        assert_eq!(title_of(&ws, doc_id), "t");
    }

    #[test]
    fn hydrate_remove_entry_drops_the_file() {
        let doc = sample_doc();
        let doc_id = doc.uid();
        let mut ws = Workspace::new_from_entry(Entry::Directory(vec![(
            "a.md".into(),
            Entry::File(doc),
        )]));

        ws.hydrate(std::iter::once(Op::RemoveEntry { entry_id: doc_id }));

        let Entry::Directory(items) = ws.root().as_ref() else {
            panic!("root is a directory");
        };
        assert!(items.is_empty());
    }
}
