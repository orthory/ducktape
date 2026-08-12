use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ::chat::index::{ChatViewQuery, ChatViewReply, MsgRow};
// No `ChatQuery`/`ChatReply` here on purpose: every chat read in this app goes
// through `/v1/index/chat/view`, off the node's select loop. A dispatch query
// import reappearing is the signal that one crawled back onto it.
use ::chat::{ChatMsg, PostPolicy};
use ducktape_rpc::{Client as RpcClient, ModuleEvent, Status as NodeStatus};
use iced::futures::StreamExt as _;
use pages::index::{PageRow, PagesViewQuery, PagesViewReply, ThreadRow};
use pages::{BlockKind, NewBlock, PageMsg, PageQuery, PageReply};
use tokio::io::AsyncWriteExt as _;
use zeroize::{Zeroize as _, Zeroizing};

// chat's client view model is module-owned (`chat::client`) — the rendered
// row types, the composer parsing, the optimistic merges, and the op-delta
// splices. re-exported here because the Ice externs resolve `crate::backend`.
pub use ::chat::client::{
    ChatBlock, ChatChannel, ChatDelta, ChatMember, ChatMessage, ChatReaction, ChatSpan,
    append_thread_page, apply_chat_channels, apply_chat_members, apply_chat_messages,
    apply_chat_thread, author_display, author_name, chat_message, contains_pending_message,
    mark_message_groups, merge_message_send_result, merge_pending_messages, merge_thread_reply,
    parse_message_with_members, reply_settled_by, rollback_pending_message, send_settled_by,
    settled_reply_id, settled_send_id, short_label, thread_offset_after_reply,
};
// the composer's block splitter is not called by the shipping binary — only by
// the app's own test helpers, which build message rows the way a send does.
#[cfg(test)]
pub use ::chat::client::paragraph_blocks;
// forge's client view model, same arrangement: the tracker rows, the item
// pane (reviews + merge-box tallies), and the op-refresh classification.
pub use ::forge::client::{
    ForgeRefresh, ItemRow as ForgeItem, ReviewCommentRow as ForgeReviewComment,
    ReviewRow as ForgeReview,
};
pub use inbox::client::{BellDelta, BellItem, apply_bell_items as fold_bell_items};
pub use pages::client::PagesDelta;
const DEFAULT_RPC: &str = "http://127.0.0.1:8844";
const MAX_SIGNED_PAYLOAD_BYTES: usize = 23 * 1024;
const MAX_KEY_FILE_BYTES: u64 = 64 * 1024;
const MAX_FRAME_HEX_BYTES: usize = 3 * 1024 * 1024;
const ENCRYPTED_KEY_PREFIX: &str = "ducktape-user-key-v1:";
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
/// `node init` mints a key and writes a workspace; `node join` fetches an
/// invite's fronts. Both are slower than an rpc round-trip and both are
/// interactive-blocking, so they get their own ceiling.
const CLI_TIMEOUT: Duration = Duration::from_secs(120);
/// How many one-second polls the provisioning screen waits before it says the
/// node is not running and names the command that starts it.
const PROVISION_PATIENCE: u32 = 8;
/// The voting window a membership proposal opens with, in consensus seconds —
/// the same value the CLI's membership ceremony uses.
const GOVERNANCE_VOTING_PERIOD: u64 = 1_000_000;
/// How many thread roots a timeline load asks for before it stops spending
/// round trips. This is a REQUEST bound, not a render bound: the timeline is
/// virtualized (`virtual-row` on the message column), so rows the viewport
/// cannot see are never laid out and mounting more of them is free. What is
/// not free is the walk that fetches them — each step is a `MessagesRange`
/// RPC. Once a walk has this many roots in hand it has enough to fill several
/// screens, so it stops asking and leaves the rest to "Load older messages".
///
/// Whatever the page that crossed the quota carried over it is kept: the rows
/// already came over the wire, and discarding them only to fetch them again on
/// the next click is pure waste.
const CHAT_TIMELINE_ROOT_QUOTA: usize = 40;
/// The chat view clamps one message page to 256 rows (default 50, max 256), so
/// the timeline walk steps in 256-row pages.
const CHAT_VIEW_PAGE_LIMIT: u64 = 256;
/// How many such pages one backward walk may spend hunting roots. The walk
/// filters thread replies client-side, so a thread-heavy channel yields few
/// roots per page and would otherwise crawl head→seq 1 in 256-row hops before
/// chat paints anything. Bounded, a load costs at most this many round trips
/// and leaves the rest to "Load older messages".
const CHAT_TIMELINE_MAX_PAGES: u32 = 4;

/// Client-local read cursor for one channel: the newest `seq` this device has
/// "seen". There is no wire read-cursor — this list lives only in app state and
/// is never sent to the node.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChannelRead {
    pub channel: String,
    pub seq: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatData {
    pub channels: Vec<ChatChannel>,
    pub messages: Vec<ChatMessage>,
    pub active_channel: String,
    pub active_channel_name: String,
    pub active_channel_archived: bool,
    pub active_channel_members_only: bool,
    pub active_channel_huddle_count: i64,
    /// the huddle's roster, not just its length — the faces and the tiles.
    pub huddle_roster: Vec<HuddleParticipant>,
    pub channel_members: Vec<ChatMember>,
    pub selected_message_seq: i64,
    pub selected_message_rev: i64,
    pub selected_message_body: String,
    pub active_thread_seq: i64,
    pub thread_target_seq: i64,
    pub thread_messages: Vec<ChatMessage>,
    pub thread_next_reply_offset: i64,
    pub thread_has_more: bool,
}

/// The submit receipt of an optimistic send: the client-minted operation id
/// and its channel. The committed row arrives on the delta stream and settles
/// the pending row by id — there is no snapshot to merge.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct SendReceipt {
    pub operation_id: String,
    pub channel_id: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub(crate) struct ThreadData {
    pub root_seq: i64,
    pub target_seq: i64,
    pub messages: Vec<ChatMessage>,
    pub next_reply_offset: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ThreadLoadData {
    pub generation: i64,
    pub root_seq: i64,
    pub target_seq: i64,
    pub messages: Vec<ChatMessage>,
    pub next_reply_offset: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ThreadPageData {
    pub generation: i64,
    pub messages: Vec<ChatMessage>,
    pub next_reply_offset: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct LiveThreadData {
    pub generation: i64,
    pub channel_id: String,
    pub root_seq: i64,
    pub target_seq: i64,
    pub messages: Vec<ChatMessage>,
    pub next_reply_offset: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatSearchHit {
    pub channel_id: String,
    pub seq: i64,
    pub root_seq: i64,
    pub author: String,
    pub text: String,
    pub meta: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatSearchData {
    pub generation: i64,
    pub hits: Vec<ChatSearchHit>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageItem {
    pub id: String,
    pub title: String,
    pub parent: String,
    pub prefix: String,
    pub child_count: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageBlock {
    pub key: i64,
    pub id: String,
    pub parent: String,
    pub kind: String,
    pub text: String,
    pub pending: bool,
    pub checked: bool,
    pub prefix: String,
    pub child_count: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PagesData {
    pub pages: Vec<PageItem>,
    pub blocks: Vec<PageBlock>,
    pub active_page: String,
    pub active_page_title: String,
    pub active_page_parent: String,
    /// Every open comment thread on the page or its blocks — the header count
    /// the surface wears BEFORE the rail is ever opened.
    pub comment_thread_total: i64,
    /// The block ids carrying at least one unresolved thread, for the
    /// commented-line washes in the document.
    pub commented_block_hits: Vec<String>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageCommentThread {
    pub id: String,
    /// The block (or page) id the thread anchors to — the wire always carried
    /// it; dropping it here was what made block-anchored threads unopenable.
    pub target: String,
    pub author: String,
    pub meta: String,
    pub resolved: bool,
    pub comment_count: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageComment {
    pub id: String,
    pub ordinal: i64,
    pub author: String,
    pub meta: String,
    pub text: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct BlockThreadListData {
    pub generation: i64,
    pub target: String,
    pub from: i64,
    pub threads: Vec<PageCommentThread>,
    pub total: i64,
    pub next_from: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct BlockCommentData {
    pub generation: i64,
    pub target: String,
    pub thread_id: String,
    pub from: i64,
    pub comments: Vec<PageComment>,
    pub next_from: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageSearchHit {
    pub page_id: String,
    /// The title of the page the block lives in. The index's hit row carries
    /// only `page_id`, so without this join no surface could name the page a
    /// match came from — see [`titled_page_hits`].
    pub page_title: String,
    pub block_id: String,
    pub kind: String,
    pub text: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageSearchData {
    pub generation: i64,
    pub hits: Vec<PageSearchHit>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct WorkspaceData {
    pub generation: i64,
    pub rpc: String,
    pub status: String,
    pub height: i64,
    pub channels: Vec<ChatChannel>,
    pub messages: Vec<ChatMessage>,
    pub active_channel: String,
    pub active_channel_name: String,
    pub active_channel_archived: bool,
    pub active_channel_members_only: bool,
    pub active_channel_huddle_count: i64,
    pub huddle_roster: Vec<HuddleParticipant>,
    pub channel_members: Vec<ChatMember>,
    pub pages: Vec<PageItem>,
    pub blocks: Vec<PageBlock>,
    pub active_page: String,
    pub active_page_title: String,
    pub active_page_parent: String,
    pub comment_thread_total: i64,
    pub commented_block_hits: Vec<String>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AppError {
    pub message: String,
    pub committed: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct OptimisticMutationError {
    pub message: String,
    pub committed: bool,
    pub operation_id: String,
    pub scope_id: String,
    pub body: String,
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self {
            message: user_error(message),
            committed: false,
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct HydrationError {
    pub generation: i64,
    pub message: String,
}

#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct LiveUpdate {
    /// `ready` (topics subscribed — run the catch-up resync), `retry`
    /// (stream down, reconnecting), `chat` / `pages` (one folded delta),
    /// `resync` (this module's replay lagged — reload its slices).
    pub kind: String,
    pub status: String,
    pub height: i64,
    /// the module needing a scoped resync (`kind == "resync"`).
    pub module: String,
    /// which plane(s) the handler must reload (`ready` = both after the
    /// subscribe→hydrate ordering race; `resync` = the lagged plane; a pages
    /// delta = the pages plane, debounced). chat deltas set neither.
    pub load_chat: bool,
    pub load_pages: bool,
    /// trail 100ms so a burst of pages ops coalesces into one reload.
    pub debounce: bool,
    pub chat: ChatDelta,
    pub pages: PagesDelta,
    pub bell: BellDelta,
    /// one committed forge op's invalidation scope (`kind == "forge"`).
    pub forge: ForgeRefresh,
}

mod bell;
mod chat;
mod document;
mod explorer;
mod forge;
mod hub;
mod live;
mod load;
mod model;
mod node;
mod roster;
mod rpc;
mod search;
mod shell;
mod storage;
mod style;

pub use bell::*;
pub use chat::*;
pub use document::*;
pub use explorer::*;
pub use forge::*;
pub use hub::*;
pub use live::*;
pub use load::*;
pub use model::*;
pub use node::*;
pub use roster::*;
pub use rpc::*;
pub use search::*;
pub use shell::*;
pub use storage::*;
pub use style::*;

#[cfg(test)]
mod tests;
