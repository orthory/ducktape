use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ::chat::index::{ChatViewQuery, ChatViewReply, MsgRow};
// No `ChatQuery`/`ChatReply` here on purpose: every chat read in this app goes
// through `/v1/index/chat/view`, off the node's select loop. A dispatch query
// import reappearing is the signal that one crawled back onto it.
use ::chat::{ChatMsg, PostPolicy};
// this device's signing key, opened in-process by `keystore` and signing op
// frames through `::node::encode_frame` — see `rpc::Signer`.
use commonware_cryptography::{Signer as _, ed25519};
use ducktape_rpc::{Client as RpcClient, ModuleEvent, Status as NodeStatus};
use iced::futures::{FutureExt as _, StreamExt as _};
use pages::index::{PageRow, PagesViewQuery, PagesViewReply, ThreadRow};
use pages::{BlockKind, NewBlock, PageMsg, PageQuery, PageReply};
use tokio::sync::OwnedSemaphorePermit;
use zeroize::Zeroizing;

// chat's client view model is module-owned (`chat::client`) — the rendered
// row types, the composer parsing, the optimistic merges, and the op-delta
// splices. re-exported here because the Ice externs resolve `crate::backend`.
pub use ::chat::client::{
    CHAT_HOT_WINDOW_LIMIT, ChatBlock, ChatChannel, ChatDelta, ChatMember, ChatMessage,
    ChatReaction, ChatSpan, append_thread_page, author_display, author_name, bounded_chat_window,
    bounded_thread_window, chat_message, contains_pending_message, mark_message_groups,
    merge_landing_messages, merge_message_send_result, merge_pending_messages,
    merge_thread_refresh, parse_message_with_members, rollback_pending_message, short_label,
};
// the composer's block splitter is not called by the shipping binary — only by
// the app's own test helpers, which build message rows the way a send does.
#[cfg(test)]
pub use ::chat::client::{THREAD_HOT_WINDOW_LIMIT, paragraph_blocks};
// forge's client view model, same arrangement: the tracker rows, the item
// pane (reviews + merge-box tallies), and the op-refresh classification.
pub use ::forge::client::{
    ForgeRefresh, ItemRow as ForgeItem, ReviewCommentRow as ForgeReviewComment,
    ReviewRow as ForgeReview,
};
pub use inbox::client::{BellDelta, BellItem, apply_bell_items as fold_bell_items};
pub use pages::client::PagesDelta;
const DEFAULT_RPC: &str = "http://127.0.0.1:8844";
/// How many one-second polls the provisioning screen waits before it says the
/// node is not running and names the command that starts it.
const PROVISION_PATIENCE: u32 = 8;
/// The voting window a membership proposal opens with, in consensus seconds —
/// the same value the CLI's membership ceremony uses.
const GOVERNANCE_VOTING_PERIOD: u64 = 1_000_000;
/// One index view page fills the entire bounded render window. Timeline roots
/// have their own index keyspace, so this is always one RPC regardless of how
/// many thread replies sit between roots.
const CHAT_VIEW_PAGE_LIMIT: usize = CHAT_HOT_WINDOW_LIMIT;

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
    /// The switch this window answers for. Every route that moves the reader
    /// bumps `chat_generation` and stamps it here, so a room she has already
    /// clicked past cannot land on top of the one she is looking at. Thread,
    /// history, and search reads own separate compiler delivery lanes.
    pub generation: i64,
    pub channels: Vec<ChatChannel>,
    pub messages: Vec<ChatMessage>,
    pub has_older_history: bool,
    pub active_channel: String,
    pub active_channel_name: String,
    pub active_channel_archived: bool,
    pub active_channel_members_only: bool,
    /// the huddle's roster, not just its length — the faces and the tiles.
    pub huddle_roster: Vec<HuddleParticipant>,
    pub channel_members: Vec<ChatMember>,
    pub selected_message_seq: i64,
    pub selected_message_rev: i64,
    pub selected_message_body: String,
    pub active_thread_seq: i64,
    pub thread_target_seq: i64,
    pub thread_messages: Vec<ChatMessage>,
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
    pub next_reply_seq: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ThreadLoadData {
    pub generation: i64,
    pub root_seq: i64,
    pub target_seq: i64,
    pub messages: Vec<ChatMessage>,
    pub next_reply_seq: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ThreadPageData {
    pub generation: i64,
    pub messages: Vec<ChatMessage>,
    pub next_reply_seq: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct LiveThreadData {
    pub channel_id: String,
    pub root_seq: i64,
    pub messages: Vec<ChatMessage>,
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
    pub has_older_history: bool,
    pub active_channel: String,
    pub active_channel_name: String,
    pub active_channel_archived: bool,
    pub active_channel_members_only: bool,
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
    /// The thread a REPLY was for, `0` for a message. The composer that let
    /// the body go is keyed by its room and its thread, so a failure can only
    /// be handed back to the box it came from if it says which one that was.
    pub thread_seq: i64,
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

#[derive(Clone, Debug, PartialEq)]
pub struct LiveUpdate {
    /// `ready` (topics subscribed — run the catch-up resync), `retry`
    /// (stream down, reconnecting), `chat` (one ordered bounded delta batch),
    /// `pages` (one folded delta), `resync` (this module's replay lagged —
    /// reload its slices).
    pub kind: crate::LiveKind,
    pub status: String,
    pub height: i64,
    /// the module needing a scoped resync (`kind == LiveKind::Resync`).
    pub module: String,
    /// which plane(s) the handler must reload (`ready` = both after the
    /// subscribe→hydrate ordering race; `resync` = the lagged plane; a pages
    /// delta = the pages plane, debounced). chat deltas set neither.
    pub load_chat: bool,
    pub load_pages: bool,
    /// trail 100ms so a burst of pages ops coalesces into one reload.
    pub debounce: bool,
    /// Ordered chat deltas. Consecutive, already-ready chat frames are
    /// published together so one network burst costs one reducer pass and one
    /// view rebuild per bounded batch, not one of each per operation.
    pub chat: Vec<ChatDelta>,
    pub pages: PagesDelta,
    pub bell: BellDelta,
    /// one committed forge op's invalidation scope (`kind == LiveKind::Forge`).
    pub forge: ForgeRefresh,
    /// Subscription backpressure, not UI state. The next socket publication
    /// cannot be read until the generated app message carrying this token has
    /// finished its update and all of its clones have been dropped.
    pub(crate) permit: LivePermit,
}

#[derive(Clone, Default)]
pub(crate) struct LivePermit(Option<Arc<OwnedSemaphorePermit>>);

impl LivePermit {
    pub(crate) fn held(permit: OwnedSemaphorePermit) -> Self {
        Self(Some(Arc::new(permit)))
    }

    #[cfg(test)]
    pub(crate) fn is_held(&self) -> bool {
        self.0.is_some()
    }
}

impl std::fmt::Debug for LivePermit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("LivePermit")
            .field(&self.0.is_some())
            .finish()
    }
}

impl PartialEq for LivePermit {
    fn eq(&self, _other: &Self) -> bool {
        // The permit changes scheduling only; it is not part of a publication's
        // domain value and must not perturb reducer/test equality.
        true
    }
}

impl Default for LiveUpdate {
    fn default() -> Self {
        Self {
            kind: crate::LiveKind::Retry,
            status: String::new(),
            height: 0,
            module: String::new(),
            load_chat: false,
            load_pages: false,
            debounce: false,
            chat: Vec::new(),
            pages: PagesDelta::default(),
            bell: BellDelta::default(),
            forge: ForgeRefresh::default(),
            permit: LivePermit::default(),
        }
    }
}

mod agent;
mod bell;
mod chat;
mod document;
mod duck_uri;
mod explorer;
mod forge;
mod hub;
mod live;
mod load;
mod model;
mod node;
mod picture;
mod roster;
mod rpc;
mod search;
mod shell;
mod storage;
mod style;

pub use agent::*;
pub use bell::*;
pub use chat::*;
pub use document::*;
pub use duck_uri::*;
pub use explorer::*;
pub use forge::*;
pub use hub::*;
pub use live::*;
pub use load::*;
pub use model::*;
pub use node::*;
pub use picture::*;
pub use roster::*;
pub use rpc::*;
pub use search::*;
pub use shell::*;
pub use storage::*;
pub use style::*;

#[cfg(test)]
mod tests;
