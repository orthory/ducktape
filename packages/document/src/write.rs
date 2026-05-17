use std::collections::hash_map::Entry::{Occupied, Vacant};

use uid::Identify;

use crate::{Document, journal::ContainerResult, operation::Operation};

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
    pub fn apply(&mut self, op: Operation) -> Result<(), Errors> {
        match &op {
            Operation::InsertAfter { anchor_id, node, .. } => {
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
            Operation::RemoveNode { anchor_id, node_id, .. } => {
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
            Operation::OnUserWrite { node_id, pos, text, .. } => {
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
            Operation::OnUserDelete { node_id, start, len, .. } => {
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
            Operation::OnUserCaret { .. } => todo!(),
            Operation::OnUserBlur => todo!(),
        }

        // insert to journal
        match self.journal.insert_op(op) {
            ContainerResult::Inserted => Ok(()),
            ContainerResult::Flush => {
                let _flushable = self.journal.drain_flushable();

                // at this point, journal is already flushed.
                // really apply the changes
                // TODO: that's not true
                Ok(())
            }
        }
    }

    fn resolve_next_uid(&self, anchor_id: &uid::Uid) -> Result<Option<uid::Uid>, Errors> {
        Ok(self.nodes_map.get(anchor_id).ok_or(Errors::InvalidAnchor)?.1)
    }
}
