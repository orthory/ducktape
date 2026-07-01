//! the document module's public wire surface — types only, no logic, no sdk dep.
//!
//! documents are SIMPLE and block-based (no markdown): a document is an ordered
//! list of [`Block`]s keyed by `doc_id`. a consumer that writes documents depends
//! on THIS crate, never on the document impl.

use serde::{Deserialize, Serialize};

/// the kind of a block. a small enum — extend later (list, quote, image, ...).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading,
    Code,
}

/// one block of a document: a stable id, a kind, and its text payload.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: String,
    pub kind: BlockKind,
    pub text: String,
}

/// write intents the document module accepts (its `execute` payload).
///
/// `after` positioning rule (SAME in `InsertBlock` and `MoveBlock`):
/// `None` == "at the front" (index 0); `Some(id)` == "immediately after the
/// block with that id" (anchor must exist, else the op errors).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DocMsg {
    /// create an empty document at `doc_id`. idempotent: re-creating an existing
    /// doc is a benign no-op. required before any block op (blocks never
    /// auto-create the doc).
    CreateDoc { doc_id: String },
    /// insert `block` into `doc_id` after the given anchor (see the `after` rule).
    InsertBlock { doc_id: String, after: Option<String>, block: Block },
    /// replace the text of an existing block.
    UpdateBlock { doc_id: String, block_id: String, text: String },
    /// remove a block from the document.
    RemoveBlock { doc_id: String, block_id: String },
    /// move an existing block to a new position (see the `after` rule).
    MoveBlock { doc_id: String, block_id: String, after: Option<String> },
}

pub fn encode_msg(m: &DocMsg) -> Vec<u8> { serde_json::to_vec(m).expect("serializable") }
pub fn decode_msg(b: &[u8]) -> Result<DocMsg, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }

/// read requests the document module serves via `Module::query`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DocQuery {
    /// the whole document as its ordered `Vec<Block>` (`None` == doc absent).
    GetDoc { doc_id: String },
    /// a single block by id (`None` == doc or block absent).
    GetBlock { doc_id: String, block_id: String },
}

/// replies to a [`DocQuery`]. `Option` mirrors absence (doc/block not found).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DocReply {
    Doc(Option<Vec<Block>>),
    Block(Option<Block>),
}

pub fn encode_query(q: &DocQuery) -> Vec<u8> { serde_json::to_vec(q).expect("serializable") }
pub fn decode_query(b: &[u8]) -> Result<DocQuery, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
pub fn encode_reply(r: &DocReply) -> Vec<u8> { serde_json::to_vec(r).expect("serializable") }
pub fn decode_reply(b: &[u8]) -> Result<DocReply, String> { serde_json::from_slice(b).map_err(|e| e.to_string()) }
