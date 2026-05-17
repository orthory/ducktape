#[derive(thiserror::Error, Debug)]
pub enum OperationError {
    #[error("asdf {0}")]
    NodeNotFound(String)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpId {
    client_id: u16,
    client_k: u32,
}

impl OpId {
    pub fn new(client_id: u16, client_k: u32) -> Self {
        Self { client_id, client_k }
    }

    pub fn client_id(&self) -> u16 { self.client_id }
    pub fn client_k(&self) -> u32 { self.client_k }
}

#[derive(Debug)]
pub enum Operation {
    // node-wise operations
    InsertAfter {
        op_id: OpId,
        anchor_id: uid::Uid,
        node: nodes::Nodes,
    },
    RemoveNode {
        op_id: OpId,
        anchor_id: uid::Uid,
        node_id: uid::Uid,
    },

    // intra-node operation
    OnUserWrite {
        op_id: OpId,
        node_id: uid::Uid,
        pos: usize,
        text: String,
    },

    OnUserDelete {
        op_id: OpId,
        node_id: uid::Uid,
        start: u32,
        len: usize,
    },

    // user-visible operations
    OnUserCaret {
        op_id: OpId,
        node_id: uid::Uid,
        pos: usize
    },
    OnUserBlur,
    
}
