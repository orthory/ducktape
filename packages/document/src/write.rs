use std::collections::hash_map::Entry::{Occupied, Vacant};

use uid::Identify;

use crate::{Document, op::Op};

#[derive(thiserror::Error, Debug)]
pub enum Errors {

    #[error("unknown anchor")]
    InvalidAnchor,

    #[error("duplicate uid")]
    DuplicateUid,

    #[error("invalid node")]
    InvalidNode,

    #[error("invalid remove op")]
    InvalidRemoveOp,

    #[error("invalid position")]
    InvalidPosition,
}

impl Document {
    /// apply applies op to document construction
    /// 
    /// both in-memory and ops journal are wrriten to as soon as an op is seen,
    /// so that both reconstruction (from in-memory) and journal sync can be done
    /// 
    /// server-side serialization is done here, but leader must be elected elsewhere
    /// 
    /// note: this must be single producer
    pub fn apply(&mut self, op: Op) -> Result<(), Errors> {
        match &op {
            Op::InsertAfter { anchor_id, node, .. } => {
                let anchor_next_uid = self.resolve_next_uid(anchor_id)?;
                let node_uid = node.uid();
                let _ = match self.nodes_map.entry(node_uid) {
                    Occupied(_) => {
                        Err(Errors::DuplicateUid)
                    },
                    Vacant(v) => {
                        Ok(v.insert((node.clone(), anchor_next_uid)))
                    }
                }?;

                // modify anchor's next to current
                self.nodes_map
                    .get_mut(anchor_id)
                    .expect("anchor shouuld exist at this point")
                    .1 = Some(node_uid);
            },
            Op::RemoveNode { anchor_id, node_id, .. } => {
                let node_next = self.nodes_map
                    .get(node_id)
                    .ok_or(Errors::InvalidNode)?.1;
                
                let (_, anchor_next_uid) = self.nodes_map
                    .get_mut(anchor_id)
                    .ok_or(Errors::InvalidAnchor)?;

                // check if supplied anchor_id <> node_id is consecutive
                if *anchor_next_uid != Some(*node_id) {
                    return Err(Errors::InvalidRemoveOp)
                }

                *anchor_next_uid = node_next;

                self.nodes_map.remove(node_id);
            },
            Op::OnUserWrite { node_id, pos, text, .. } => {
                let (node, _) = self.nodes_map
                    .get_mut(node_id)
                    .ok_or(Errors::InvalidNode)?;

                node.with_editable_text(|s| -> Result<(), Errors> {
                    if *pos > s.len() || !s.is_char_boundary(*pos) {
                        return Err(Errors::InvalidPosition);
                    }
                    s.insert_str(*pos, text);
                    Ok(())
                })?;
            },
            Op::OnUserDelete { node_id, start, len, .. } => {
                let (node, _) = self.nodes_map
                    .get_mut(node_id)
                    .ok_or(Errors::InvalidNode)?;

                let start = *start as usize;
                let end = start.checked_add(*len).ok_or(Errors::InvalidPosition)?;

                node.with_editable_text(|s| -> Result<(), Errors> {
                    if end > s.len()
                        || !s.is_char_boundary(start)
                        || !s.is_char_boundary(end)
                    {
                        return Err(Errors::InvalidPosition);
                    }
                    s.replace_range(start..end, "");
                    Ok(())
                })?;
            },
            // caret / presence ops carry no document STATE — they're
            // ephemeral cursor/focus signals that don't affect convergence.
            // Document has no caret field, so there's nothing to mutate here.
            Op::OnUserCaret { .. } => {}
            Op::OnUserBlur => {}
        };

        Ok(())
    }

    fn resolve_next_uid(&self, anchor_id: &uid::Uid) -> Result<Option<uid::Uid>, Errors> {
        Ok(self.nodes_map.get(anchor_id).ok_or(Errors::InvalidAnchor)?.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::{Op, OpId};
    use nodes::{FrontmatterV1, Nodes};
    use std::collections::HashMap;

    fn opid() -> OpId {
        OpId::new(1, 1)
    }

    /// A single-node Document whose root frontmatter has a known uid and an
    /// empty title (the editable text buffer for a frontmatter node).
    fn single_node_doc() -> (Document, uid::Uid) {
        let root_uid = uid::new();
        let node = Nodes::Frontmatter(FrontmatterV1 {
            uid: root_uid,
            title: String::new(),
            ..Default::default()
        });
        let mut nodes_map = HashMap::new();
        nodes_map.insert(root_uid, (node, None));
        (Document { uid: root_uid, nodes_map }, root_uid)
    }

    fn title_of(doc: &Document, id: &uid::Uid) -> String {
        match &doc.nodes_map.get(id).unwrap().0 {
            Nodes::Frontmatter(f) => f.title.clone(),
            _ => panic!("expected frontmatter"),
        }
    }

    // insert-after, then write text into the inserted node, then delete a
    // range out of it. asserts both list structure and resulting text.
    #[test]
    fn insert_then_write_then_delete() {
        let (mut doc, root) = single_node_doc();

        let child_uid = uid::new();
        let child = Nodes::Frontmatter(FrontmatterV1 {
            uid: child_uid,
            title: String::new(),
            ..Default::default()
        });

        doc.apply(Op::InsertAfter {
            op_id: opid(),
            anchor_id: root,
            node: child,
        })
        .unwrap();

        // root -> child -> None
        assert_eq!(doc.nodes_map.get(&root).unwrap().1, Some(child_uid));
        assert_eq!(doc.nodes_map.get(&child_uid).unwrap().1, None);

        // write "hello world" at pos 0
        doc.apply(Op::OnUserWrite {
            op_id: opid(),
            node_id: child_uid,
            pos: 0,
            text: "hello world".into(),
        })
        .unwrap();
        assert_eq!(title_of(&doc, &child_uid), "hello world");

        // delete "hello " (chars [0,6)), leaving "world"
        doc.apply(Op::OnUserDelete {
            op_id: opid(),
            node_id: child_uid,
            start: 0,
            len: 6,
        })
        .unwrap();
        assert_eq!(title_of(&doc, &child_uid), "world");
    }

    #[test]
    fn remove_node_mends_the_list() {
        let (mut doc, root) = single_node_doc();

        let child_uid = uid::new();
        doc.apply(Op::InsertAfter {
            op_id: opid(),
            anchor_id: root,
            node: Nodes::Frontmatter(FrontmatterV1 {
                uid: child_uid,
                ..Default::default()
            }),
        })
        .unwrap();

        doc.apply(Op::RemoveNode {
            op_id: opid(),
            anchor_id: root,
            node_id: child_uid,
        })
        .unwrap();

        assert!(doc.nodes_map.get(&child_uid).is_none());
        assert_eq!(doc.nodes_map.get(&root).unwrap().1, None);
    }

    // caret / blur carry no document state — applying them must not error and
    // must not change structure.
    #[test]
    fn caret_and_blur_are_noops() {
        let (mut doc, root) = single_node_doc();
        let before = doc.nodes_map.len();

        doc.apply(Op::OnUserCaret { op_id: opid(), node_id: root, pos: 0 })
            .unwrap();
        doc.apply(Op::OnUserBlur).unwrap();

        assert_eq!(doc.nodes_map.len(), before);
    }
}
