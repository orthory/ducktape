//! the chat module's public wire surface -- types only.
//!
//! writes go via [`ChatMsg`]; reads via [`ChatQuery`] -> [`ChatReply`]; hook
//! subscribers receive [`ChatEvent`] payloads. authorship is never part of a
//! write payload — the module derives it from the dispatch origin — so the
//! wire surface carries [`AuthorRef`] only in replies and events.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const DEFAULT_CHAT_TARGET: &str = "chat";

// ---- write-time caps (consensus constants) ---------------------------------
// enforced by the module BEFORE staging: the qmdb codec's 1 MiB cap is
// decode-only, so an oversized committed value would panic every validator on
// the next read. shared here so clients can pre-validate.

/// serialized [`MessageHead`] bound, per message record.
pub const MAX_MESSAGE_HEAD_BYTES: usize = 64 * 1024;
/// serialized [`Channel`] record bound (also applied to the membership index).
pub const MAX_CHANNEL_RECORD_BYTES: usize = 256 * 1024;
/// revisions per message; further edits are rejected.
pub const MAX_REVISIONS: u32 = 256;
/// emoji byte length bound.
pub const MAX_EMOJI_BYTES: usize = 64;
/// distinct emojis per message.
pub const MAX_REACTION_EMOJIS: usize = 64;
/// hook modules per channel.
pub const MAX_HOOKS_PER_CHANNEL: usize = 8;
/// replies per thread.
pub const MAX_THREAD_REPLIES: usize = 4096;
/// query page bound; larger limits are clamped down to this.
pub const MAX_QUERY_LIMIT: u64 = 256;
/// participants per channel huddle; further joins are rejected.
pub const MAX_HUDDLE_MEMBERS: usize = 32;
/// a huddle member's node key: raw ed25519 public key bytes.
pub const HUDDLE_NODE_KEY_BYTES: usize = 32;

/// who authored a message — derived from `Env.origin`, never from a payload.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AuthorRef {
    /// an external submitter's (non-empty) public key bytes.
    User(Vec<u8>),
    /// an individual agent hosted by a module. `module` always derives from
    /// the dispatch origin (`Origin::Module`); `agent_id` comes from the
    /// post's `as_agent` field, which ONLY a module origin may set — so an
    /// agent author is exactly as trustworthy as the genesis-fixed module
    /// code that claimed it, and each agent is individually addressable in
    /// mentions.
    Agent { module: String, agent_id: String },
    /// a module that emitted the write as a follow-up.
    Module(String),
    /// genesis / system-internal.
    System,
}

/// inline formatting applied to a [`Span`]. mentions are structured so
/// agent-trigger parsing stays deterministic.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mark {
    Bold,
    Italic,
    Link(String),
    Mention(AuthorRef),
}

/// a run of text with uniform marks.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub marks: Vec<Mark>,
}

impl Span {
    /// a plain, unmarked span.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            marks: Vec::new(),
        }
    }
}

/// one block of a message body.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Block {
    Paragraph(Vec<Span>),
    Code { lang: Option<String>, text: String },
    Quote(Vec<Span>),
    Divider,
}

impl Block {
    /// a single-span plain paragraph.
    pub fn paragraph(text: impl Into<String>) -> Self {
        Self::Paragraph(vec![Span::plain(text)])
    }
}

/// who may post (and react) in a channel.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PostPolicy {
    /// any authenticated author.
    Open,
    /// external users must be channel members; module/system authors always may.
    MembersOnly,
}

/// one participant of a channel's live huddle. `node` is the raw ed25519 key
/// of the member's node — where peers route this participant's voice frames
/// (the media plane authenticates by transport identity; this is routing, not
/// authorship). `user` derives from `Env.origin` like every chat author.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HuddleMember {
    pub user: Vec<u8>,
    pub node: Vec<u8>,
    pub joined_at: u64,
}

/// the per-channel record: metadata plus the head sequence counter that
/// assigns every message's position (P3 — gap-free, in-state, at execute time).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub created_at: u64,
    /// the last assigned message sequence; 0 = no messages yet.
    pub head_seq: u64,
    pub post_policy: PostPolicy,
    /// module ids notified (one follow-up msg each) on every successful post.
    pub hooks: Vec<String>,
    /// pinned message sequences (no pin op yet; carried for the record shape).
    pub pinned: Vec<u64>,
    /// the channel's live huddle roster, join order. empty = no huddle. the
    /// roster is consensus state (who is in the room); the audio itself rides
    /// the off-consensus voice plane.
    pub huddle: Vec<HuddleMember>,
    /// the user who created the channel (`AuthorRef::User` bytes). `None` for
    /// module/system-minted channels, which have no user owner. only the owner
    /// may rename/archive an owned channel; a `None` owner is open to any user.
    pub owner: Option<Vec<u8>>,
    /// archived channels reject posts, reactions, and huddle joins; membership,
    /// rename, and unarchive stay allowed.
    pub archived: bool,
}

/// the mutable head of one message. prior contents live in immutable revision
/// records; a delete tombstones the head but keeps the skeleton so thread
/// linkage and the per-channel sequence promise survive.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MessageHead {
    pub message_id: String,
    pub author: AuthorRef,
    pub blocks: Vec<Block>,
    pub created_at: u64,
    /// edit revision; 0 = original post.
    pub rev: u32,
    pub edited_at: Option<u64>,
    /// the revision the last edit CLAIMED to be based on. a stale base is
    /// recorded (base_rev != rev - 1), never rejected — head is last-write-wins
    /// under the consensus total order.
    pub base_rev: Option<u32>,
    pub deleted: bool,
    /// `Some(root_seq)` marks this message as a thread reply.
    pub thread: Option<u64>,
    pub reply_count: u64,
    pub last_reply_seq: Option<u64>,
}

/// one emoji's reactors on a message. set semantics per (emoji, author).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReactionSummary {
    pub emoji: String,
    pub reactors: BTreeSet<AuthorRef>,
}

/// a query-side message view: the head plus its reaction summary and the
/// channel's head-sequence watermark at read time.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MessageView {
    pub channel_id: String,
    pub seq: u64,
    pub head: MessageHead,
    pub reactions: Vec<ReactionSummary>,
    pub channel_head_seq: u64,
}

/// a thread: the root message plus one page of replies.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    pub root: MessageView,
    pub replies: Vec<MessageView>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatMsg {
    /// `channel_id`s containing `:` are a reserved module namespace: an
    /// external (user) origin may not create one, and a module origin `m`
    /// may only create ids prefixed `"{m}:"` (forge's per-issue/PR discussion
    /// channels are `forge:<repo>:<n>`). system origin is unrestricted.
    CreateChannel {
        channel_id: String,
        name: String,
        post_policy: PostPolicy,
    },
    /// rename a channel, reusing `CreateChannel`'s name validation (non-empty +
    /// the reserved `:` namespace gate + the record byte cap). only the
    /// channel's `owner` (a `User` origin) may rename an owned channel; an
    /// unowned (module/system-minted) channel admits any user, mirroring
    /// `SetMembership`. module and system origins pass as elsewhere.
    RenameChannel { channel_id: String, name: String },
    /// archive or unarchive a channel. an archived channel rejects posts,
    /// reactions, and huddle joins; membership, rename, and unarchive stay
    /// allowed. authorization mirrors `RenameChannel`.
    SetChannelArchived { channel_id: String, archived: bool },
    /// post a message; `thread` = `Some(root_seq)` posts a thread reply, which
    /// is a normal message record consuming its own channel sequence.
    /// `as_agent` refines a MODULE origin into an [`AuthorRef::Agent`] author
    /// (`{module: origin module, agent_id}`) — modules are genesis-trusted
    /// code, so a hosting module may attribute a post to one of its agents.
    /// an external or system submitter setting `as_agent` is REJECTED.
    PostMessage {
        channel_id: String,
        message_id: String,
        blocks: Vec<Block>,
        thread: Option<u64>,
        as_agent: Option<String>,
    },
    /// replace the head blocks; the prior head is appended to the immutable
    /// revision history. only the stored author may edit.
    EditMessage {
        channel_id: String,
        seq: u64,
        blocks: Vec<Block>,
        base_rev: Option<u32>,
    },
    /// tombstone: content and reactions cleared, skeleton kept. only the
    /// stored author may delete.
    DeleteMessage { channel_id: String, seq: u64 },
    /// idempotent per (emoji, author).
    AddReaction {
        channel_id: String,
        seq: u64,
        emoji: String,
    },
    /// exact remove of this author's reaction; absent = deterministic no-op.
    RemoveReaction {
        channel_id: String,
        seq: u64,
        emoji: String,
    },
    /// subscribe a module to this channel's post notifications. any non-empty
    /// origin may register for now — admin gating is future work.
    RegisterHook {
        channel_id: String,
        module_id: String,
    },
    UnregisterHook {
        channel_id: String,
        module_id: String,
    },
    /// add/remove an external user from the channel member set. any non-empty
    /// origin may modify for now — admin gating is future work.
    SetMembership {
        channel_id: String,
        user: Vec<u8>,
        member: bool,
    },
    /// join (or start) the channel's huddle. external users only — huddles are
    /// human affordances; members-only channels gate like posting. idempotent:
    /// re-joining updates `node` (the joiner's node key, [`HUDDLE_NODE_KEY_BYTES`]
    /// raw ed25519 bytes) and stages nothing when unchanged.
    JoinHuddle { channel_id: String, node: Vec<u8> },
    /// leave the channel's huddle. leaving a huddle one is not in is a
    /// deterministic no-op; an empty roster means no huddle.
    LeaveHuddle { channel_id: String },
    /// evict a huddle member — call liveness is not consensus-observable
    /// (a crashed client cannot leave), so cleanup is social: any author the
    /// channel's post policy admits may sweep a stale entry, mirroring
    /// `SetMembership`'s trust posture. sweeping an absent user is a
    /// deterministic no-op.
    SweepHuddle { channel_id: String, user: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatQuery {
    Channels,
    Channel {
        channel_id: String,
    },
    /// the newest `limit` messages (all sequences — replies and tombstones
    /// included, so pagination stays gap-free), ascending by sequence.
    MessagesLatest {
        channel_id: String,
        limit: u64,
    },
    /// `limit` messages starting at `from_seq`, ascending.
    MessagesRange {
        channel_id: String,
        from_seq: u64,
        limit: u64,
    },
    /// the window of `limit` messages CENTERED on `seq`, ascending — the
    /// jump-to-message read: a tag/search hit older than the newest `limit`
    /// is in no [`MessagesLatest`](ChatQuery::MessagesLatest) page. half the
    /// window sits before `seq`, the rest from `seq` on; both ends clamp to the
    /// channel's live range and `limit` to [`MAX_QUERY_LIMIT`]. carries every
    /// sequence (replies, tombstones) like the other pages.
    MessagesAround {
        channel_id: String,
        seq: u64,
        limit: u64,
    },
    /// global message-id lookup.
    Message {
        message_id: String,
    },
    /// the immutable edit history of one message, ascending by revision.
    Revisions {
        channel_id: String,
        seq: u64,
    },
    /// the thread root plus one page of replies; `from` is a 0-based offset
    /// into the reply list.
    Thread {
        channel_id: String,
        root_seq: u64,
        from: u64,
        limit: u64,
    },
    Reactions {
        channel_id: String,
        seq: u64,
    },
    Members {
        channel_id: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatReply {
    Channels(Vec<Channel>),
    Channel(Option<Channel>),
    Messages(Vec<MessageView>),
    Message(Option<MessageView>),
    Revisions(Vec<MessageHead>),
    Thread(Option<Thread>),
    Reactions(Vec<ReactionSummary>),
    Members(Vec<Vec<u8>>),
}

/// the hook notification payload: one follow-up [`sdk::Msg`]-shaped dispatch
/// per registered hook module, emitted in the same block as the post (P2).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatEvent {
    MessagePosted {
        channel_id: String,
        seq: u64,
        thread_root: Option<u64>,
        author: AuthorRef,
        mentions: Vec<AuthorRef>,
    },
}

pub fn encode_msg(m: &ChatMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<ChatMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_query(q: &ChatQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}

pub fn decode_query(b: &[u8]) -> Result<ChatQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_reply(r: &ChatReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_reply(b: &[u8]) -> Result<ChatReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_event(e: &ChatEvent) -> Vec<u8> {
    serde_json::to_vec(e).expect("serializable")
}

pub fn decode_event(b: &[u8]) -> Result<ChatEvent, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
