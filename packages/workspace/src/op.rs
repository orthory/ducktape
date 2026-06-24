use serde::{Deserialize, Serialize};

use crate::entry;

#[derive(Serialize, Deserialize, Debug)]
pub enum Op {
    EntryMut { 
        entry_id: uid::Uid,
        op: document::op::Op
    },

    AddEntry {
        entry: entry::Entry
    },

    RemoveEntry {
        entry_id: uid::Uid
    },

    MoveEntry {
        entry_id: uid::Uid,
        from: String,
        to: String,
    }
}
