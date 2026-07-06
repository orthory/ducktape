//! the comments module's public wire surface — types only, no logic, no sdk dep.
//!
//! a comment thread anchors to one addressable record ([`Anchor`] = module +
//! target id — a pages block id or page id) and holds an ordered list of
//! [`Comment`]s. authorship is derived from the dispatch origin, never a
//! payload (mirrors the chat module). the anchor makes a thread resolvable from
//! whatever holds the target, so a page render can batch-fetch every visible
//! block's threads with one [`CommentQuery::ThreadsForAnchors`].

use serde::{Deserialize, Serialize};

pub const DEFAULT_COMMENTS_TARGET: &str = "comments";

// write-time caps (consensus constants) — enforced BEFORE staging; the qmdb
// codec's 1 MiB cap is decode-only, so an oversized committed value poisons
// every validator's next read.
pub const MAX_COMMENT_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_COMMENTS_PER_THREAD: usize = 4096;
pub const MAX_THREADS_PER_ANCHOR: usize = 1024;
pub const MAX_QUERY_TARGETS: usize = 512;

/// what a thread is attached to: a module id plus a target record id (a pages
/// block id or page id). general so comments can anchor to any addressable
/// record later.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Anchor {
    pub module: String,
    pub target: String,
}

/// who authored a comment — derived from `Env.origin`, never a payload. own
/// copy of chat's shape (each module's interface is self-contained).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AuthorRef {
    User(Vec<u8>),
    Agent { module: String, agent_id: String },
    Module(String),
    System,
}

/// a comment thread: an anchor, its opener, resolve state, and the ordered ids
/// of its comments (tombstoned comments stay listed until the whole thread is
/// removed on last-live-delete).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    pub id: String,
    pub anchor: Anchor,
    pub opener: AuthorRef,
    pub created_at: u64,
    pub resolved: bool,
    pub resolved_by: Option<AuthorRef>,
    pub comment_ids: Vec<String>,
}

/// one comment. `deleted` tombstones content but keeps the record so ordering
/// and the thread skeleton survive until the thread is removed.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub id: String,
    pub thread_id: String,
    pub author: AuthorRef,
    pub text: String,
    pub created_at: u64,
    pub edited_at: Option<u64>,
    pub deleted: bool,
}

/// write intents. author + timestamp are derived by the module, never here.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommentMsg {
    /// open a thread (when `thread_id` is new) with `anchor` and this first
    /// comment, or append `comment_id` to an existing thread (whose anchor must
    /// match). author = origin.
    AddComment {
        thread_id: String,
        comment_id: String,
        anchor: Anchor,
        text: String,
    },
    /// replace a comment's text; stored-author-only. rejected on a tombstone.
    EditComment { comment_id: String, text: String },
    /// tombstone a comment; stored-author-only. when it was the thread's last
    /// live comment, the whole thread record is removed.
    DeleteComment { comment_id: String },
    /// toggle a thread's resolved flag; records the resolver as origin.
    ResolveThread { thread_id: String, resolved: bool },
}

/// read requests.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommentQuery {
    /// every thread anchored to any of `targets` in `module`, grouped by
    /// target. a page render calls this once with all visible block ids + the
    /// page id. `targets` beyond [`MAX_QUERY_TARGETS`] are rejected.
    ThreadsForAnchors { module: String, targets: Vec<String> },
    /// one thread with its live comments.
    Thread { thread_id: String },
}

/// a thread plus its live (non-tombstoned) comments in order.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ThreadView {
    pub thread: Thread,
    pub comments: Vec<Comment>,
}

/// the threads anchored to one target.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AnchorThreads {
    pub target: String,
    pub threads: Vec<ThreadView>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommentReply {
    Anchored(Vec<AnchorThreads>),
    Thread(Option<ThreadView>),
}

pub fn encode_msg(m: &CommentMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<CommentMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &CommentQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<CommentQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &CommentReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<CommentReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

#[cfg(test)]
mod interface_tests {
    use super::*;

    #[test]
    fn msg_round_trips() {
        let m = CommentMsg::AddComment {
            thread_id: "t1".into(),
            comment_id: "c1".into(),
            anchor: Anchor { module: "pages".into(), target: "b1".into() },
            text: "hi".into(),
        };
        assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
    }

    #[test]
    fn reply_round_trips() {
        let r = CommentReply::Anchored(vec![AnchorThreads {
            target: "b1".into(),
            threads: vec![],
        }]);
        assert_eq!(decode_reply(&encode_reply(&r)).unwrap(), r);
    }
}
