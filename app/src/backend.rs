use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chat::index::{ChatViewQuery, ChatViewReply, MsgRow};
use chat::{ChatMsg, ChatQuery, ChatReply, PostPolicy};
use ducktape_rpc::{Client as RpcClient, ModuleEvent, Status as NodeStatus};
use iced::futures::StreamExt as _;
use pages::index::{PageRow, PagesViewQuery, PagesViewReply, ThreadRow};
use pages::{BlockKind, NewBlock, PageMsg, PageQuery, PageReply};
use tokio::io::AsyncWriteExt as _;
use zeroize::{Zeroize as _, Zeroizing};

// chat's client view model is module-owned (`chat::client`) — the rendered
// row types, the composer parsing, the optimistic merges, and the op-delta
// splices. re-exported here because the Ice externs resolve `crate::backend`.
pub use chat::client::{
    ChatBlock, ChatChannel, ChatDelta, ChatMember, ChatMessage, ChatReaction, ChatSpan,
    append_thread_page, apply_chat_channels, apply_chat_members, apply_chat_messages,
    apply_chat_thread, author_name, chat_message, contains_pending_message,
    mark_message_groups, merge_message_send_result, merge_pending_messages,
    merge_thread_reply, optimistic_message, paragraph_blocks, parse_message,
    rollback_pending_message, short_label, thread_offset_after_reply,
};
pub use pages::client::PagesDelta;

const DEFAULT_RPC: &str = "http://127.0.0.1:8844";
const MAX_SIGNED_PAYLOAD_BYTES: usize = 23 * 1024;
const MAX_KEY_FILE_BYTES: u64 = 64 * 1024;
const MAX_FRAME_HEX_BYTES: usize = 3 * 1024 * 1024;
const ENCRYPTED_KEY_PREFIX: &str = "ducktape-user-key-v1:";
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const CHAT_TIMELINE_ROOT_LIMIT: usize = 128;
/// The chat view clamps one message page to 256 rows (default 50, max 256), so
/// the timeline walk steps in 256-row pages.
const CHAT_VIEW_PAGE_LIMIT: u64 = 256;

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
struct ThreadData {
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
    pub mark_count: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PagesData {
    pub pages: Vec<PageItem>,
    pub blocks: Vec<PageBlock>,
    pub active_page: String,
    pub active_page_title: String,
    pub active_page_parent: String,
    pub selected_block_id: String,
    pub selected_block_kind: String,
    pub selected_block_text: String,
    pub selected_block_checked: bool,
    pub page_title_selected: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct BlockInsertResult {
    pub data: PagesData,
    pub operation_id: String,
    pub page_id: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageCommentThread {
    pub id: String,
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
pub struct BlockCommentsRefreshData {
    pub generation: i64,
    pub target: String,
    pub threads: Vec<PageCommentThread>,
    pub total: i64,
    pub threads_next_from: i64,
    pub threads_has_more: bool,
    pub thread_id: String,
    pub comments: Vec<PageComment>,
    pub comments_next_from: i64,
    pub comments_has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageSearchHit {
    pub page_id: String,
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
pub struct AutosaveResult {
    pub generation: i64,
    pub written: bool,
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
    pub channel_members: Vec<ChatMember>,
    pub pages: Vec<PageItem>,
    pub blocks: Vec<PageBlock>,
    pub active_page: String,
    pub active_page_title: String,
    pub active_page_parent: String,
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
            message,
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
}


/// Custom command the multiline composer's key bindings raise. It carries no
/// data: the only Custom binding is "send", routed straight to
/// `send_message_submit`, which reads the editor text itself.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ComposerCmd;

/// Editor key bindings for the message composer: plain Enter sends (raised as a
/// Custom command), every other key press — Shift+Enter included, which iced
/// maps to a newline insertion — keeps its native binding.
pub fn composer_keys(
    event: iced::widget::text_editor::KeyPress,
) -> Option<iced::widget::text_editor::Binding<ComposerCmd>> {
    let enter_pressed = matches!(
        event.key,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
    );
    let sends = enter_pressed && !event.modifiers.shift();
    if sends {
        return Some(iced::widget::text_editor::Binding::Custom(ComposerCmd));
    }
    iced::widget::text_editor::Binding::from_key_press(event)
}

pub fn fresh_operation_id(prefix: String) -> String {
    fresh_id(&prefix)
}

pub fn optimistic_block(
    mut blocks: Vec<PageBlock>,
    after_id: String,
    kind: String,
    text: String,
    id: String,
) -> Vec<PageBlock> {
    let selected_index = blocks.iter().position(|block| block.id == after_id);
    let (insert_at, parent, prefix) = match selected_index {
        Some(index) => {
            let selected = &blocks[index];
            let selected_depth = selected.prefix.len();
            let insert_at = blocks
                .iter()
                .enumerate()
                .skip(index + 1)
                .find(|(_, block)| block.prefix.len() <= selected_depth)
                .map_or(blocks.len(), |(index, _)| index);
            (insert_at, selected.parent.clone(), selected.prefix.clone())
        }
        None => (blocks.len(), String::new(), String::new()),
    };
    blocks.insert(
        insert_at,
        PageBlock {
            key: page_block_key(&id),
            id,
            parent,
            kind,
            text,
            pending: true,
            checked: false,
            prefix,
            child_count: 0,
            mark_count: 0,
        },
    );
    blocks
}

pub fn merge_pending_blocks(
    canonical: Vec<PageBlock>,
    current: Vec<PageBlock>,
    current_page: String,
    next_page: String,
    settled_id: String,
) -> Vec<PageBlock> {
    if current_page != next_page {
        return canonical;
    }
    let canonical_ids = canonical
        .iter()
        .map(|block| block.id.clone())
        .collect::<BTreeSet<_>>();
    let mut pending_by_anchor = BTreeMap::<String, Vec<PageBlock>>::new();
    let mut anchor = String::new();
    for block in current {
        if canonical_ids.contains(&block.id) {
            anchor = block.id;
        } else if block.pending && block.id != settled_id {
            pending_by_anchor
                .entry(anchor.clone())
                .or_default()
                .push(block);
        }
    }
    if pending_by_anchor.is_empty() {
        return canonical;
    }
    let mut merged = pending_by_anchor.remove("").unwrap_or_default();
    for block in canonical {
        let id = block.id.clone();
        merged.push(block);
        merged.extend(pending_by_anchor.remove(&id).unwrap_or_default());
    }
    merged.extend(pending_by_anchor.into_values().flatten());
    merged
}

pub fn merge_block_insert_result(
    canonical: Vec<PageBlock>,
    current: Vec<PageBlock>,
    current_page: String,
    next_page: String,
    settled_id: String,
) -> Vec<PageBlock> {
    if current_page != next_page {
        return canonical;
    }
    let canonical_ids = canonical
        .iter()
        .map(|block| block.id.clone())
        .collect::<BTreeSet<_>>();
    let mut extras_by_anchor = BTreeMap::<String, Vec<PageBlock>>::new();
    let mut anchor = String::new();
    for block in current {
        if canonical_ids.contains(&block.id) {
            anchor = block.id;
        } else if block.id != settled_id {
            extras_by_anchor
                .entry(anchor.clone())
                .or_default()
                .push(block);
        }
    }
    if extras_by_anchor.is_empty() {
        return canonical;
    }
    let mut merged = extras_by_anchor.remove("").unwrap_or_default();
    for block in canonical {
        let id = block.id.clone();
        merged.push(block);
        merged.extend(extras_by_anchor.remove(&id).unwrap_or_default());
    }
    merged.extend(extras_by_anchor.into_values().flatten());
    merged
}

pub fn rollback_pending_block(
    mut blocks: Vec<PageBlock>,
    pending_id: String,
    committed: bool,
) -> Vec<PageBlock> {
    if !committed {
        blocks.retain(|block| !block.pending || block.id != pending_id);
    }
    blocks
}

pub fn remember_failed_block(
    mut drafts: Vec<String>,
    current: String,
    pending: String,
    committed: bool,
) -> Vec<String> {
    if !committed && !current.is_empty() {
        append_recovered_draft(&mut drafts, pending);
    }
    drafts
}

pub fn rollback_blocks(mut blocks: Vec<PageBlock>, keep_pending: bool) -> Vec<PageBlock> {
    if keep_pending {
        return blocks;
    }
    blocks.retain(|block| !block.pending);
    blocks
}

pub fn append_page_comment_threads(
    threads: Vec<PageCommentThread>,
    next: Vec<PageCommentThread>,
) -> Vec<PageCommentThread> {
    threads
        .into_iter()
        .chain(next)
        .map(|thread| (thread.id.clone(), thread))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}

pub fn append_page_comments(
    comments: Vec<PageComment>,
    next: Vec<PageComment>,
) -> Vec<PageComment> {
    comments
        .into_iter()
        .chain(next)
        .map(|comment| (comment.ordinal, comment))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}

pub fn restore_draft(current: String, pending: String, keep_pending: bool) -> String {
    if keep_pending {
        return current;
    }
    if current.is_empty() { pending } else { current }
}

pub fn remember_failed_draft(
    existing: String,
    current: String,
    pending: String,
    committed: bool,
) -> String {
    if committed || current.is_empty() || pending.is_empty() {
        return existing;
    }
    if existing.is_empty() {
        return pending;
    }
    format!("{existing}\n{pending}")
}

pub fn retain_for_endpoint(value: String, current: String, next: String) -> String {
    if current == next {
        value
    } else {
        String::new()
    }
}

pub fn mutation_failure_phase(committed: bool) -> String {
    if committed { "recovering" } else { "idle" }.into()
}

fn committed_message_change(phase: &str, committed: bool) -> bool {
    committed && matches!(phase, "message-edit" | "message-delete")
}

pub fn message_seq_after_failure(current: i64, phase: String, committed: bool) -> i64 {
    if committed_message_change(&phase, committed) {
        0
    } else {
        current
    }
}

pub fn message_text_after_failure(current: String, phase: String, committed: bool) -> String {
    if committed_message_change(&phase, committed) {
        String::new()
    } else {
        current
    }
}

pub fn message_action_after_failure(current: String, phase: String, committed: bool) -> String {
    if committed_message_change(&phase, committed) {
        "toolbar".into()
    } else {
        current
    }
}

pub fn refreshed_required_message_seq(
    messages: Vec<ChatMessage>,
    current_channel: String,
    next_channel: String,
    value: i64,
) -> i64 {
    if current_channel != next_channel {
        return 0;
    }
    if messages
        .iter()
        .any(|message| message.seq == value && !message.deleted)
    {
        value
    } else {
        0
    }
}

pub fn refreshed_known_message_seq(
    messages: Vec<ChatMessage>,
    current_channel: String,
    next_channel: String,
    value: i64,
) -> i64 {
    if current_channel != next_channel
        || messages
            .iter()
            .any(|message| message.seq == value && message.deleted)
    {
        0
    } else {
        value
    }
}

pub fn refreshed_channel_value(current_channel: String, next_channel: String, value: i64) -> i64 {
    if current_channel == next_channel {
        value
    } else {
        0
    }
}

// --- Client-local unread tracking (no wire read-cursor) ------------------
//
// `channel_reads` is a per-channel last-seen `seq`. `unread_boundary` is that
// value FROZEN at the moment you entered the current channel, used only to
// place the in-channel "New messages" divider for this visit.

fn last_read_of(reads: &[ChannelRead], channel: &str) -> i64 {
    reads
        .iter()
        .find(|read| read.channel == channel)
        .map_or(0, |read| read.seq)
}

fn head_seq_of(channels: &[ChatChannel], channel: &str) -> i64 {
    channels
        .iter()
        .find(|entry| entry.id == channel)
        .map_or(0, |entry| entry.head_seq)
}

pub fn channel_last_read(reads: Vec<ChannelRead>, channel: String) -> i64 {
    last_read_of(&reads, &channel)
}

pub fn channel_head_seq(channels: Vec<ChatChannel>, channel: String) -> i64 {
    head_seq_of(&channels, &channel)
}

// active-channel scalars re-derived from the (delta-folded) channel list,
// keeping the current value when the channel is absent from the list.

pub fn channel_display_name(channels: Vec<ChatChannel>, channel: String, current: String) -> String {
    channels
        .iter()
        .find(|row| row.id == channel)
        .map_or(current, |row| row.name.clone())
}

pub fn channel_flag_archived(channels: Vec<ChatChannel>, channel: String, current: bool) -> bool {
    channels
        .iter()
        .find(|row| row.id == channel)
        .map_or(current, |row| row.archived)
}

pub fn channel_flag_members_only(
    channels: Vec<ChatChannel>,
    channel: String,
    current: bool,
) -> bool {
    channels
        .iter()
        .find(|row| row.id == channel)
        .map_or(current, |row| row.members_only)
}

pub fn channel_live_huddle_count(
    channels: Vec<ChatChannel>,
    channel: String,
    current: i64,
) -> i64 {
    channels
        .iter()
        .find(|row| row.id == channel)
        .map_or(current, |row| row.huddle_count)
}

/// Advance the open thread's next-reply offset when a reply delta for THAT
/// thread lands (the loaded page grew by one settled row).
pub fn thread_offset_after_live(
    offset: i64,
    has_more: bool,
    delta: ChatDelta,
    active_channel: String,
    root: i64,
) -> i64 {
    let is_open_thread_reply =
        delta.kind == "reply" && delta.channel_id == active_channel && delta.root_seq == root;
    if !is_open_thread_reply {
        return offset;
    }
    thread_offset_after_reply(offset, has_more, true)
}

/// Upsert `channel`'s read cursor to `max(existing, seq)`. An empty channel id
/// (no channel selected / disconnected) is inert.
pub fn mark_channel_read(
    mut reads: Vec<ChannelRead>,
    channel: String,
    seq: i64,
) -> Vec<ChannelRead> {
    if channel.is_empty() {
        return reads;
    }
    if let Some(read) = reads.iter_mut().find(|read| read.channel == channel) {
        read.seq = read.seq.max(seq);
        return reads;
    }
    reads.push(ChannelRead { channel, seq });
    reads
}

/// A channel is unread when its newest `seq` is past what this device has seen.
/// `initial_channel_reads` seeds every channel at connect, so a caught-up
/// channel has `last_read == head_seq` and never lights up spuriously; the
/// cursor only lags once new messages actually arrive.
pub fn channel_is_unread(reads: Vec<ChannelRead>, channel: String, head_seq: i64) -> bool {
    head_seq > last_read_of(&reads, &channel)
}

/// On first connect, seed each not-yet-tracked channel's cursor to its own head
/// so the session starts fully caught up. Existing entries are preserved.
pub fn initial_channel_reads(
    channels: Vec<ChatChannel>,
    existing: Vec<ChannelRead>,
) -> Vec<ChannelRead> {
    let mut reads = existing;
    for channel in channels {
        let tracked = reads.iter().any(|read| read.channel == channel.id);
        if !tracked {
            reads.push(ChannelRead {
                channel: channel.id,
                seq: channel.head_seq,
            });
        }
    }
    reads
}

/// Where to freeze the "New messages" divider when entering a channel. Only
/// re-freezes on an actual channel change — a same-channel refresh keeps the
/// divider still. Returns 0 (no divider) when arriving already caught up, so a
/// caught-up channel never grows a divider above your own later sends or live
/// arrivals during the visit.
pub fn frozen_unread_boundary(
    reads: Vec<ChannelRead>,
    channels: Vec<ChatChannel>,
    current_channel: String,
    next_channel: String,
    current_boundary: i64,
) -> i64 {
    if current_channel == next_channel {
        return current_boundary;
    }
    let last_read = last_read_of(&reads, &next_channel);
    let head = head_seq_of(&channels, &next_channel);
    let arrived_with_unread = head > last_read;
    if arrived_with_unread { last_read } else { 0 }
}

/// The `seq` of the first message past `boundary` (messages are seq-ascending),
/// or 0 when the visit started caught up (`boundary <= 0`) or nothing is unread.
/// Pending optimistic messages carry `seq == -1`, so they never anchor a divider.
pub fn first_unread_seq(messages: Vec<ChatMessage>, boundary: i64) -> i64 {
    if boundary <= 0 {
        return 0;
    }
    messages
        .iter()
        .find(|message| message.seq > boundary)
        .map_or(0, |message| message.seq)
}

pub fn thread_generation_after_refresh(
    generation: i64,
    current_channel: String,
    next_channel: String,
    previous_root: i64,
    next_root: i64,
) -> i64 {
    let context_unchanged = current_channel == next_channel && previous_root == next_root;
    if context_unchanged {
        generation
    } else {
        generation + 1
    }
}

pub fn thread_loading_after_refresh(
    loading: bool,
    current_channel: String,
    next_channel: String,
    previous_root: i64,
    next_root: i64,
) -> bool {
    let same_channel = current_channel == next_channel;
    let active_root_was_invalidated = previous_root > 0 && next_root <= 0;
    loading && same_channel && !active_root_was_invalidated
}

pub fn retain_thread_messages(messages: Vec<ChatMessage>, root_seq: i64) -> Vec<ChatMessage> {
    if root_seq > 0 { messages } else { Vec::new() }
}

pub fn refreshed_block_draft(
    blocks: Vec<PageBlock>,
    selected_id: String,
    current: String,
    autosave_status: String,
) -> String {
    let has_local_edit = matches!(autosave_status.as_str(), "saving" | "error");
    if selected_id.is_empty() || has_local_edit {
        return current;
    }
    blocks
        .into_iter()
        .find(|block| block.id == selected_id)
        .map_or(current, |block| block.text)
}

pub fn remember_orphaned_block_drafts(
    mut drafts: Vec<String>,
    blocks: Vec<PageBlock>,
    selected_id: String,
    current: String,
    autosave_status: String,
) -> Vec<String> {
    let has_local_edit = matches!(autosave_status.as_str(), "saving" | "error");
    if has_local_edit && selected_block_missing(&blocks, &selected_id) {
        append_recovered_draft(&mut drafts, current);
    }
    drafts
}

pub fn remember_orphaned_comment_drafts(
    mut drafts: Vec<String>,
    blocks: Vec<PageBlock>,
    selected_id: String,
    current: String,
) -> Vec<String> {
    if selected_block_missing(&blocks, &selected_id) {
        append_recovered_draft(&mut drafts, current);
    }
    drafts
}

pub fn remove_recovered_draft(mut drafts: Vec<String>, recovered: String) -> Vec<String> {
    if let Some(index) = drafts.iter().position(|draft| draft == &recovered) {
        drafts.remove(index);
    }
    drafts
}

pub fn retain_drafts_for_endpoint(
    drafts: Vec<String>,
    current: String,
    next: String,
) -> Vec<String> {
    if current == next { drafts } else { Vec::new() }
}

pub fn refreshed_selected_block(blocks: Vec<PageBlock>, selected_id: String) -> String {
    if selected_block_missing(&blocks, &selected_id) {
        String::new()
    } else {
        selected_id
    }
}

pub fn retain_selected_string(value: String, selected_id: String) -> String {
    if selected_id.is_empty() {
        String::new()
    } else {
        value
    }
}

pub fn retain_selected_i64(value: i64, selected_id: String) -> i64 {
    if selected_id.is_empty() { 0 } else { value }
}

pub fn retain_selected_comment_threads(
    threads: Vec<PageCommentThread>,
    selected_id: String,
) -> Vec<PageCommentThread> {
    if selected_id.is_empty() {
        Vec::new()
    } else {
        threads
    }
}

pub fn retain_selected_comments(
    comments: Vec<PageComment>,
    selected_id: String,
) -> Vec<PageComment> {
    if selected_id.is_empty() {
        Vec::new()
    } else {
        comments
    }
}

pub fn cancel_missing_block_autosave(
    rpc: String,
    generation: i64,
    blocks: Vec<PageBlock>,
    selected_id: String,
) -> i64 {
    if selected_block_missing(&blocks, &selected_id) {
        let key = scope_key(rpc.trim().to_string(), selected_id);
        autosaves()
            .lock()
            .expect("autosave lock poisoned")
            .remove(&key);
        return generation.saturating_add(1);
    }
    generation
}

fn selected_block_missing(blocks: &[PageBlock], selected_id: &str) -> bool {
    !selected_id.is_empty() && !blocks.iter().any(|block| block.id == selected_id)
}

fn append_recovered_draft(drafts: &mut Vec<String>, draft: String) {
    let should_append = !draft.is_empty() && !drafts.iter().any(|current| current == &draft);
    if should_append {
        drafts.push(draft);
    }
}

pub fn scope_key(scope: String, id: String) -> String {
    format!("{scope}\0{id}")
}

pub fn block_action_menu_y(pointer_y: f64, viewport_height: f64) -> f64 {
    let below = (pointer_y - 4.0).max(0.0);
    let below_fits = below + 190.0 <= viewport_height;
    if below_fits {
        below
    } else {
        (pointer_y - 190.0).max(0.0)
    }
}

struct Tip {
    height: i64,
    status: String,
}

fn rpc_client(input: &str) -> Result<RpcClient, String> {
    let configured = if input.trim().is_empty() {
        std::env::var("DUCKTAPE_NODE").unwrap_or_else(|_| DEFAULT_RPC.to_string())
    } else {
        input.trim().to_string()
    };
    RpcClient::new(&configured).map_err(Into::into)
}

pub fn canonical_endpoint(input: String) -> String {
    let configured = input.trim();
    rpc_client(configured)
        .map(|rpc| rpc.origin().to_string())
        .unwrap_or_else(|_| configured.to_string())
}

pub async fn connect(rpc: String) -> Result<WorkspaceData, AppError> {
    let result = async {
        let rpc = rpc_client(&rpc)?;
        load_workspace(&rpc, None, None, 0).await
    }
    .await;
    result.map_err(|_| AppError {
        message: "Could not connect. Check the endpoint and node.".into(),
        committed: false,
    })
}

pub fn live_events(rpc: String) -> iced::futures::stream::BoxStream<'static, LiveUpdate> {
    struct State {
        rpc: String,
        cursors: BTreeMap<String, String>,
        stream: Option<ducktape_rpc::ModuleEventStream>,
        retry_attempt: u32,
    }

    iced::futures::stream::unfold(
        State {
            rpc,
            cursors: BTreeMap::new(),
            stream: None,
            retry_attempt: 0,
        },
        |mut state| async move {
            if state.stream.is_none() && state.retry_attempt > 0 {
                tokio::time::sleep(retry_delay(state.retry_attempt)).await;
            }
            if state.stream.is_none() {
                let connected = async {
                    let rpc = rpc_client(&state.rpc)?;
                    rpc.module_events(
                        vec!["chat".to_string(), "pages".to_string()],
                        state.cursors.clone(),
                    )
                    .await
                    .map_err(Into::into)
                }
                .await;
                match connected {
                    Ok(stream) => state.stream = Some(stream),
                    Err(error) => {
                        state.retry_attempt = state.retry_attempt.saturating_add(1);
                        return Some((live_retry(error), state));
                    }
                }
            }
            loop {
                let event = state
                    .stream
                    .as_mut()
                    .expect("stream initialized above")
                    .next()
                    .await;
                let update = match event {
                    Some(Ok(ModuleEvent::Ready { cursors })) => {
                        state.cursors = cursors;
                        state.retry_attempt = 0;
                        live_update("ready", "Live", -1)
                    }
                    Some(Ok(ModuleEvent::Changed { module, cursor, op })) => {
                        state.cursors.insert(format!("module:{module}"), cursor);
                        match folded_update(&state.rpc, &module, op).await {
                            Some(update) => update,
                            // invisible to the UI (hook registration) — keep
                            // draining without emitting.
                            None => continue,
                        }
                    }
                    Some(Ok(ModuleEvent::Lagged { module, cursor })) => {
                        state.cursors.insert(format!("module:{module}"), cursor);
                        live_resync(&module, -1)
                    }
                    Some(Err(error)) => {
                        state.stream = None;
                        state.retry_attempt = state.retry_attempt.saturating_add(1);
                        live_retry(error.into())
                    }
                    None => {
                        state.stream = None;
                        state.retry_attempt = state.retry_attempt.saturating_add(1);
                        live_retry("RPC stream closed".into())
                    }
                };
                return Some((update, state));
            }
        },
    )
    .boxed()
}

/// Fold one applied op into a live update. A decode failure (payload or
/// stamp) degrades to a scoped resync of that module — a CLIENT reloads,
/// never wedges. `None` = the op is invisible to this UI.
async fn folded_update(
    rpc: &str,
    module: &str,
    op: ducktape_rpc::StreamOp,
) -> Option<LiveUpdate> {
    let height = i64::try_from(op.height).unwrap_or(i64::MAX);
    let Some(payload) = op
        .payload
        .as_ref()
        .and_then(|value| serde_json::to_vec(value).ok())
    else {
        return Some(live_resync(module, height));
    };
    match module {
        "chat" => {
            let current_user = local_user_key().await;
            let origin_kind = match op.origin.kind {
                ducktape_rpc::StreamOriginKind::External => "external",
                ducktape_rpc::StreamOriginKind::Module => "module",
                ducktape_rpc::StreamOriginKind::System => "system",
            };
            let folded = chat::client::delta_from_op(
                &payload,
                op.assigned.as_ref(),
                origin_kind,
                op.origin.id.as_deref(),
                current_user.as_deref(),
            );
            let mut delta = match folded {
                Ok(Some(delta)) => delta,
                Ok(None) => return None,
                Err(_) => return Some(live_resync("chat", height)),
            };
            // huddle membership is roster-derived — reload the one channel
            // row from its canonical record instead of guessing the count.
            if delta.kind == "channel-refresh" {
                match load_channel_row(rpc, &delta.channel_id).await {
                    Ok(channel) => {
                        delta.kind = "channel-updated".into();
                        delta.channel = channel;
                    }
                    Err(_) => return Some(live_resync("chat", height)),
                }
            }
            Some(LiveUpdate {
                kind: "chat".into(),
                status: format!("Live · block {height}"),
                height,
                module: "chat".into(),
                load_chat: false,
                load_pages: false,
                debounce: false,
                chat: delta,
                pages: PagesDelta::default(),
            })
        }
        "pages" => match pages::client::delta_from_op(&payload) {
            Ok(delta) => Some(LiveUpdate {
                kind: "pages".into(),
                status: format!("Live · block {height}"),
                height,
                module: "pages".into(),
                load_chat: false,
                load_pages: true,
                debounce: true,
                chat: ChatDelta::default(),
                pages: delta,
            }),
            Err(_) => Some(live_resync("pages", height)),
        },
        _ => None,
    }
}

/// One channel's row rebuilt from its canonical record (the dispatch-grade
/// point read) — the huddle roster length is not derivable from the op.
async fn load_channel_row(rpc: &str, channel_id: &str) -> Result<ChatChannel, String> {
    let rpc = rpc_client(rpc)?;
    let reply: ChatReply = rpc
        .query(
            "chat",
            &ChatQuery::Channel {
                channel_id: channel_id.to_string(),
            },
        )
        .await?;
    let ChatReply::Channel(Some(channel)) = reply else {
        return Err("channel record was not found".into());
    };
    Ok(ChatChannel {
        id: channel.id,
        name: channel.name,
        archived: channel.archived,
        members_only: channel.post_policy == PostPolicy::MembersOnly,
        huddle_count: count_i64(channel.huddle.len()),
        head_seq: number_i64(channel.head_seq),
    })
}

/// One scoped catch-up load, flag-selected per plane: the chat slices
/// (channel list + active window + members) and/or the pages slices (page
/// list + active blocks). Runs on stream `ready` (the subscribe→hydrate
/// ordering race), on a `resync` (lag or an unfoldable op), and debounced
/// after pages deltas — never per chat commit. Unloaded planes come back
/// with their `*_loaded` flag false and the handler keeps current state.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct LiveRefresh {
    pub generation: i64,
    pub chat_loaded: bool,
    pub channels: Vec<ChatChannel>,
    pub messages: Vec<ChatMessage>,
    pub active_channel: String,
    pub active_channel_name: String,
    pub active_channel_archived: bool,
    pub active_channel_members_only: bool,
    pub active_channel_huddle_count: i64,
    pub channel_members: Vec<ChatMember>,
    pub pages_loaded: bool,
    pub pages: Vec<PageItem>,
    pub blocks: Vec<PageBlock>,
    pub active_page: String,
    pub active_page_title: String,
    pub active_page_parent: String,
}

/// `planes` is `chat` | `pages` | `both` — the flat Ice surface's
/// discriminant for which slices to load ([`resync_planes`] builds it).
pub async fn live_resync_load(
    rpc: String,
    channel_id: String,
    page_id: String,
    planes: String,
    debounce: bool,
    generation: i64,
    attempt: i64,
) -> Result<LiveRefresh, HydrationError> {
    if debounce {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if attempt > 0 {
        tokio::time::sleep(retry_delay(u32::try_from(attempt).unwrap_or(u32::MAX))).await;
    }
    async {
        let rpc = rpc_client(&rpc)?;
        let mut refresh = LiveRefresh {
            generation,
            chat_loaded: false,
            channels: Vec::new(),
            messages: Vec::new(),
            active_channel: String::new(),
            active_channel_name: String::new(),
            active_channel_archived: false,
            active_channel_members_only: false,
            active_channel_huddle_count: 0,
            channel_members: Vec::new(),
            pages_loaded: false,
            pages: Vec::new(),
            blocks: Vec::new(),
            active_page: String::new(),
            active_page_title: String::new(),
            active_page_parent: String::new(),
        };
        let load_chat = planes == "chat" || planes == "both";
        let load_pages = planes == "pages" || planes == "both";
        if load_chat {
            let chat =
                load_chat_data(&rpc, (!channel_id.is_empty()).then_some(channel_id.as_str()))
                    .await?;
            refresh.chat_loaded = true;
            refresh.channels = chat.channels;
            refresh.messages = chat.messages;
            refresh.active_channel = chat.active_channel;
            refresh.active_channel_name = chat.active_channel_name;
            refresh.active_channel_archived = chat.active_channel_archived;
            refresh.active_channel_members_only = chat.active_channel_members_only;
            refresh.active_channel_huddle_count = chat.active_channel_huddle_count;
            refresh.channel_members = chat.channel_members;
        }
        if load_pages {
            let pages =
                load_pages_data(&rpc, (!page_id.is_empty()).then_some(page_id.as_str())).await?;
            refresh.pages_loaded = true;
            refresh.pages = pages.pages;
            refresh.blocks = pages.blocks;
            refresh.active_page = pages.active_page;
            refresh.active_page_title = pages.active_page_title;
            refresh.active_page_parent = pages.active_page_parent;
        }
        Ok(refresh)
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message,
    })
}

/// The planes discriminant for [`live_resync_load`].
pub fn resync_planes(load_chat: bool, load_pages: bool) -> String {
    match (load_chat, load_pages) {
        (true, true) => "both".into(),
        (true, false) => "chat".into(),
        (false, true) => "pages".into(),
        (false, false) => String::new(),
    }
}

// per-field keepers: apply a refreshed value only when its plane loaded —
// the Ice handler assigns every field unconditionally and these self-select.

pub fn keep_channels(
    loaded: bool,
    next: Vec<ChatChannel>,
    current: Vec<ChatChannel>,
) -> Vec<ChatChannel> {
    if loaded { next } else { current }
}

pub fn keep_messages(
    loaded: bool,
    next: Vec<ChatMessage>,
    current: Vec<ChatMessage>,
) -> Vec<ChatMessage> {
    if loaded { next } else { current }
}

pub fn keep_members(
    loaded: bool,
    next: Vec<ChatMember>,
    current: Vec<ChatMember>,
) -> Vec<ChatMember> {
    if loaded { next } else { current }
}

pub fn keep_pages(loaded: bool, next: Vec<PageItem>, current: Vec<PageItem>) -> Vec<PageItem> {
    if loaded { next } else { current }
}

pub fn keep_blocks(loaded: bool, next: Vec<PageBlock>, current: Vec<PageBlock>) -> Vec<PageBlock> {
    if loaded { next } else { current }
}

pub fn keep_str(loaded: bool, next: String, current: String) -> String {
    if loaded { next } else { current }
}

pub fn keep_bool(loaded: bool, next: bool, current: bool) -> bool {
    if loaded { next } else { current }
}

pub fn keep_i64(loaded: bool, next: i64, current: i64) -> i64 {
    if loaded { next } else { current }
}

pub async fn load_chat(rpc: String, channel_id: String) -> Result<ChatData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        load_chat_data(&rpc, Some(&channel_id)).await
    }
    .await
    .map_err(app_error)
}

pub async fn load_chat_hit(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    target_seq: i64,
) -> Result<ChatData, AppError> {
    async {
        let root_seq = positive_sequence(root_seq)?;
        let target_seq = positive_sequence(target_seq)?;
        let rpc = rpc_client(&rpc)?;
        let mut chat = load_chat_data(&rpc, Some(&channel_id)).await?;
        chat.messages = load_messages_around(&rpc, &channel_id, root_seq).await?;
        let root = chat
            .messages
            .iter()
            .find(|message| message.seq == number_i64(root_seq))
            .cloned()
            .ok_or_else(|| "message was not found".to_string())?;
        chat.selected_message_seq = root.seq;
        chat.selected_message_rev = root.rev;
        chat.selected_message_body.clone_from(&root.body);
        if target_seq == root_seq {
            return Ok(chat);
        }
        let reply = load_message_at(&rpc, &channel_id, target_seq).await?;
        if reply.thread != Some(root_seq) {
            return Err("search result does not belong to the selected thread".into());
        }
        let current_user = local_user_key().await;
        chat.active_thread_seq = root.seq;
        chat.thread_target_seq = number_i64(target_seq);
        chat.thread_messages = vec![root, chat_message(reply, current_user.as_deref())];
        chat.thread_next_reply_offset = -1;
        Ok(chat)
    }
    .await
    .map_err(app_error)
}

pub async fn create_channel(
    rpc: String,
    password: String,
    name: String,
) -> Result<ChatData, AppError> {
    async {
        let name = bounded_text(name, "channel name", 128)?;
        let channel_id = fresh_id("channel");
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::CreateChannel {
                channel_id: channel_id.clone(),
                name,
                post_policy: PostPolicy::Open,
            }),
            password,
        )
        .await?;
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn rename_channel(
    rpc: String,
    password: String,
    channel_id: String,
    name: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let name = bounded_text(name, "channel name", 128)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::RenameChannel {
                channel_id: channel_id.clone(),
                name,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn archive_channel(
    rpc: String,
    password: String,
    channel_id: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::SetChannelArchived {
                channel_id: channel_id.clone(),
                archived: true,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn unarchive_channel(
    rpc: String,
    password: String,
    channel_id: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::SetChannelArchived {
                channel_id: channel_id.clone(),
                archived: false,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn add_channel_member(
    rpc: String,
    password: String,
    channel_id: String,
    member_key: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let user = public_key(&member_key, "member public key")?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::SetMembership {
                channel_id: channel_id.clone(),
                user,
                member: true,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn remove_channel_member(
    rpc: String,
    password: String,
    channel_id: String,
    member_key: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let user = public_key(&member_key, "member public key")?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::SetMembership {
                channel_id: channel_id.clone(),
                user,
                member: false,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn join_huddle(
    rpc: String,
    password: String,
    channel_id: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let rpc = rpc_client(&rpc)?;
        let status = rpc.status().await.map_err(|error| error.to_string())?;
        let node = public_key(&status.public_key, "node public key")?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::JoinHuddle {
                channel_id: channel_id.clone(),
                node,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn leave_huddle(
    rpc: String,
    password: String,
    channel_id: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::LeaveHuddle {
                channel_id: channel_id.clone(),
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn send_message(
    rpc: String,
    password: String,
    channel_id: String,
    message_id: String,
    body: String,
) -> Result<SendReceipt, OptimisticMutationError> {
    let operation_id = message_id.clone();
    let operation_scope = channel_id.clone();
    let operation_body = body.clone();
    let result = async {
        if channel_id.is_empty() {
            return Err("choose a channel first".to_string().into());
        }
        let body = bounded_text(body, "message", 16 * 1024)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: channel_id.clone(),
                message_id: required_id(message_id, "message")?,
                blocks: parse_message(&body),
                thread: None,
                as_agent: None,
            }),
            password,
        )
        .await?;
        Ok(())
    }
    .await;
    result
        .map(|()| SendReceipt {
            operation_id: operation_id.clone(),
            channel_id: operation_scope.clone(),
        })
        .map_err(|cause: AppError| OptimisticMutationError {
            message: cause.message,
            committed: cause.committed,
            operation_id,
            scope_id: operation_scope,
            body: operation_body,
        })
}

pub async fn load_thread(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    target_seq: i64,
    through_reply_offset: i64,
    generation: i64,
) -> Result<ThreadLoadData, HydrationError> {
    let result = async {
        let root_seq = positive_sequence(root_seq)?;
        let target_seq = u64::try_from(target_seq).unwrap_or(0);
        let is_sparse_target = through_reply_offset < 0 && target_seq > 0;
        let through_reply_offset = u64::try_from(through_reply_offset)
            .unwrap_or(0)
            .min(chat::MAX_THREAD_REPLIES as u64);
        let rpc = rpc_client(&rpc)?;
        if is_sparse_target {
            return load_sparse_thread_data(&rpc, &channel_id, root_seq, target_seq).await;
        }
        let mut thread =
            load_thread_data(&rpc, &channel_id, root_seq, through_reply_offset).await?;
        let target_is_loaded = target_seq > 0
            && thread
                .messages
                .iter()
                .any(|message| message.seq == number_i64(target_seq));
        if target_is_loaded {
            thread.target_seq = number_i64(target_seq);
        }
        Ok(thread)
    }
    .await;
    result
        .map(|thread| ThreadLoadData {
            generation,
            root_seq: thread.root_seq,
            target_seq: thread.target_seq,
            messages: thread.messages,
            next_reply_offset: thread.next_reply_offset,
            has_more: thread.has_more,
        })
        .map_err(|message| HydrationError {
            generation,
            message,
        })
}

pub async fn load_thread_page(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    from: i64,
    generation: i64,
) -> Result<ThreadPageData, HydrationError> {
    let result = async {
        let root_seq = positive_sequence(root_seq)?;
        let from = u64::try_from(from).map_err(|_| "invalid thread offset".to_string())?;
        let rpc = rpc_client(&rpc)?;
        let thread = query_thread_page(&rpc, &channel_id, root_seq).await?;
        let total = thread.replies.len() as u64;
        let start = from.min(total);
        let page_len = CHAT_VIEW_PAGE_LIMIT.min(total - start);
        let next_reply_offset = start + page_len;
        let page_is_full = page_len == CHAT_VIEW_PAGE_LIMIT;
        let thread_cap_reached = next_reply_offset >= chat::MAX_THREAD_REPLIES as u64;
        let has_more = page_is_full && !thread_cap_reached;
        let current_user = local_user_key().await;
        let messages = thread
            .replies
            .into_iter()
            .skip(start as usize)
            .take(page_len as usize)
            .map(|row| chat_message(row, current_user.as_deref()))
            .collect();
        Ok(ThreadPageData {
            generation,
            messages,
            next_reply_offset: number_i64(next_reply_offset),
            has_more,
        })
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message,
    })
}

pub async fn refresh_live_thread(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    target_seq: i64,
    through_reply_offset: i64,
    generation: i64,
) -> Result<LiveThreadData, HydrationError> {
    if channel_id.is_empty() || root_seq <= 0 {
        return Ok(LiveThreadData {
            generation,
            channel_id,
            root_seq: 0,
            target_seq: 0,
            messages: Vec::new(),
            next_reply_offset: 0,
            has_more: false,
        });
    }
    load_thread(
        rpc,
        channel_id.clone(),
        root_seq,
        target_seq,
        through_reply_offset,
        generation,
    )
    .await
    .map(|thread| LiveThreadData {
        generation: thread.generation,
        channel_id,
        root_seq: thread.root_seq,
        target_seq: thread.target_seq,
        messages: thread.messages,
        next_reply_offset: thread.next_reply_offset,
        has_more: thread.has_more,
    })
}

pub async fn send_reply(
    rpc: String,
    password: String,
    channel_id: String,
    root_seq: i64,
    message_id: String,
    body: String,
) -> Result<SendReceipt, OptimisticMutationError> {
    let operation_id = message_id.clone();
    let operation_scope = channel_id.clone();
    let operation_body = body.clone();
    let result = async {
        let root_seq = positive_sequence(root_seq)?;
        let body = bounded_text(body, "reply", 16 * 1024)?;
        let rpc = rpc_client(&rpc)?;
        let message_id = required_id(message_id, "message")?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: channel_id.clone(),
                message_id: message_id.clone(),
                blocks: parse_message(&body),
                thread: Some(root_seq),
                as_agent: None,
            }),
            password,
        )
        .await?;
        Ok(())
    }
    .await;
    result
        .map(|()| SendReceipt {
            operation_id: operation_id.clone(),
            channel_id: operation_scope.clone(),
        })
        .map_err(|cause: AppError| OptimisticMutationError {
        message: cause.message,
        committed: cause.committed,
        operation_id,
        scope_id: operation_scope,
        body: operation_body,
    })
}

pub async fn edit_message(
    rpc: String,
    password: String,
    channel_id: String,
    seq: i64,
    base_rev: i64,
    body: String,
) -> Result<bool, AppError> {
    async {
        let seq = positive_sequence(seq)?;
        let base_rev =
            u32::try_from(base_rev).map_err(|_| "invalid message revision".to_string())?;
        let body = bounded_text(body, "message", 16 * 1024)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::EditMessage {
                channel_id: channel_id.clone(),
                seq,
                blocks: parse_message(&body),
                base_rev: Some(base_rev),
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn delete_message(
    rpc: String,
    password: String,
    channel_id: String,
    seq: i64,
) -> Result<bool, AppError> {
    async {
        let seq = positive_sequence(seq)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::DeleteMessage {
                channel_id: channel_id.clone(),
                seq,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn add_reaction(
    rpc: String,
    password: String,
    channel_id: String,
    seq: i64,
    emoji: String,
) -> Result<bool, AppError> {
    async {
        let seq = positive_sequence(seq)?;
        let emoji = bounded_text(emoji, "reaction", chat::MAX_EMOJI_BYTES)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::AddReaction {
                channel_id: channel_id.clone(),
                seq,
                emoji,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn remove_reaction(
    rpc: String,
    password: String,
    channel_id: String,
    seq: i64,
    emoji: String,
) -> Result<bool, AppError> {
    async {
        let seq = positive_sequence(seq)?;
        let emoji = bounded_text(emoji, "reaction", chat::MAX_EMOJI_BYTES)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::RemoveReaction {
                channel_id: channel_id.clone(),
                seq,
                emoji,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn search_chat(
    rpc: String,
    channel_id: String,
    text: String,
    generation: i64,
) -> Result<ChatSearchData, HydrationError> {
    let result = async {
        let text = bounded_text(text, "search", 512)?;
        let rpc = rpc_client(&rpc)?;
        let reply: chat::index::ChatViewReply = rpc
            .view(
                "chat",
                &serde_json::json!({
                    "search": {
                        "text": text,
                        "channel_id": (!channel_id.is_empty()).then_some(channel_id),
                        "limit": 50
                    }
                }),
            )
            .await?;
        let chat::index::ChatViewReply::Hits(hits) = reply else {
            return Err("chat search returned an invalid reply".into());
        };
        Ok(ChatSearchData {
            generation,
            hits: hits
                .into_iter()
                .map(|hit| ChatSearchHit {
                    channel_id: hit.channel_id,
                    seq: number_i64(hit.seq),
                    root_seq: number_i64(hit.thread.unwrap_or(hit.seq)),
                    author: hit.author,
                    text: hit.text,
                    meta: format!("#{}", hit.seq),
                })
                .collect(),
        })
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message,
    })
}

pub async fn load_page(
    rpc: String,
    page_id: String,
    selected_block_id: String,
) -> Result<PagesData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let pages = load_pages_data(&rpc, Some(&page_id)).await?;
        Ok(with_selected_block(pages, &selected_block_id))
    }
    .await
    .map_err(app_error)
}

pub async fn load_block_threads(
    rpc: String,
    target: String,
    from: i64,
    generation: i64,
) -> Result<BlockThreadListData, HydrationError> {
    let result = async {
        let target = required_id(target, "block")?;
        let from = u32::try_from(from).map_err(|_| "invalid comment offset".to_string())?;
        let rpc = rpc_client(&rpc)?;
        query_block_threads(&rpc, &target, from, generation).await
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message,
    })
}

pub async fn load_block_comment_page(
    rpc: String,
    target: String,
    thread_id: String,
    from: i64,
    generation: i64,
) -> Result<BlockCommentData, HydrationError> {
    let result = async {
        let target = required_id(target, "block")?;
        let thread_id = required_id(thread_id, "comment thread")?;
        let from = u32::try_from(from).map_err(|_| "invalid comment offset".to_string())?;
        let rpc = rpc_client(&rpc)?;
        query_block_comment_page(&rpc, &target, &thread_id, from, generation)
            .await?
            .ok_or_else(|| "comment thread was not found".to_string())
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message,
    })
}

pub async fn refresh_block_comments(
    rpc: String,
    target: String,
    thread_id: String,
    generation: i64,
) -> Result<BlockCommentsRefreshData, HydrationError> {
    let result = async {
        if target.is_empty() {
            return Ok(BlockCommentsRefreshData {
                generation,
                target,
                threads: Vec::new(),
                total: 0,
                threads_next_from: 0,
                threads_has_more: false,
                thread_id: String::new(),
                comments: Vec::new(),
                comments_next_from: 0,
                comments_has_more: false,
            });
        }
        let target = required_id(target, "block")?;
        let rpc = rpc_client(&rpc)?;
        let threads = query_block_threads(&rpc, &target, 0, generation).await?;
        let comments = if thread_id.is_empty() {
            BlockCommentData {
                generation,
                target: target.clone(),
                thread_id,
                from: 0,
                comments: Vec::new(),
                next_from: 0,
                has_more: false,
            }
        } else {
            let thread_id = required_id(thread_id, "comment thread")?;
            query_block_comment_page(&rpc, &target, &thread_id, 0, generation)
                .await?
                .unwrap_or(BlockCommentData {
                    generation,
                    target: target.clone(),
                    thread_id: String::new(),
                    from: 0,
                    comments: Vec::new(),
                    next_from: 0,
                    has_more: false,
                })
        };
        Ok(BlockCommentsRefreshData {
            generation,
            target,
            threads: threads.threads,
            total: threads.total,
            threads_next_from: threads.next_from,
            threads_has_more: threads.has_more,
            thread_id: comments.thread_id,
            comments: comments.comments,
            comments_next_from: comments.next_from,
            comments_has_more: comments.has_more,
        })
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message,
    })
}

pub async fn post_block_comment(
    rpc: String,
    password: String,
    target: String,
    thread_id: String,
    text: String,
    generation: i64,
) -> Result<BlockCommentData, AppError> {
    async {
        let target = required_id(target, "block")?;
        let text = bounded_text(text, "comment", 16 * 1024)?;
        let thread_id = comment_thread_id(thread_id)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::AddComment {
                thread_id: thread_id.clone(),
                comment_id: fresh_id("comment"),
                target: target.clone(),
                text,
                anchor: None,
                mentions: Vec::new(),
                as_agent: None,
            }),
            password,
        )
        .await?;
        query_block_comment_page(&rpc, &target, &thread_id, 0, generation)
            .await
            .and_then(|page| page.ok_or_else(|| "comment thread was not found".to_string()))
            .map_err(committed_error)
    }
    .await
}

fn comment_thread_id(thread_id: String) -> Result<String, String> {
    if thread_id.is_empty() {
        Ok(fresh_id("thread"))
    } else {
        required_id(thread_id, "comment thread")
    }
}

pub async fn create_page(
    rpc: String,
    password: String,
    title: String,
) -> Result<PagesData, AppError> {
    async {
        let title = bounded_text(title, "page title", 512)?;
        let page_id = fresh_id("page");
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::CreatePage {
                page_id: page_id.clone(),
                title,
            }),
            password,
        )
        .await?;
        load_pages_data(&rpc, Some(&page_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn autosave_page_title(
    rpc: String,
    password: String,
    page_id: String,
    title: String,
) -> Result<bool, AppError> {
    async {
        if page_id.is_empty() {
            return Err("choose a page first".to_string());
        }
        let title = bounded_exact_text(title, "page title", 512)?;
        debounced_page_text(rpc, password, page_id, title).await
    }
    .await
    .map_err(app_error)
}

pub async fn autosave_block_text(
    rpc: String,
    password: String,
    block_id: String,
    kind: String,
    text: String,
    generation: i64,
) -> Result<AutosaveResult, HydrationError> {
    let result = async {
        let kind = parse_block_kind(&kind)?;
        let text = bounded_updated_block_text(kind, text)?;
        debounced_page_text(rpc, password, block_id, text).await
    }
    .await;
    result
        .map(|written| AutosaveResult {
            generation,
            written,
        })
        .map_err(|message| HydrationError {
            generation,
            message,
        })
}

pub async fn delete_page(
    rpc: String,
    password: String,
    page_id: String,
) -> Result<PagesData, AppError> {
    async {
        if page_id.is_empty() {
            return Err("choose a page first".to_string().into());
        }
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::RemoveBlock {
                block_id: page_id.clone(),
            }),
            password,
        )
        .await?;
        load_pages_data(&rpc, None).await.map_err(committed_error)
    }
    .await
}

pub async fn add_block(
    rpc: String,
    password: String,
    page_id: String,
    after_id: String,
    kind: String,
    block_id: String,
    text: String,
) -> Result<BlockInsertResult, OptimisticMutationError> {
    let operation_id = block_id.clone();
    let operation_scope = page_id.clone();
    let operation_body = text.clone();
    let result = async {
        if page_id.is_empty() {
            return Err("choose a page first".to_string().into());
        }
        let kind = parse_block_kind(&kind)?;
        let text = bounded_new_block_text(kind, text)?;
        let rpc = rpc_client(&rpc)?;
        let blocks = load_page_blocks(&rpc, &page_id).await?;
        let root = blocks
            .first()
            .filter(|block| block.kind == BlockKind::Page)
            .ok_or_else(|| "page block was not found".to_string())?;
        let selected = blocks.iter().find(|block| block.id == after_id);
        let parent = selected
            .and_then(|block| block.parent.clone())
            .unwrap_or_else(|| page_id.clone());
        let after = selected
            .map(|block| block.id.clone())
            .or_else(|| root.children.last().cloned());
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::InsertBlock {
                parent,
                after,
                block: NewBlock {
                    id: required_id(block_id, "block")?,
                    kind,
                    text,
                    marks: Vec::new(),
                },
            }),
            password,
        )
        .await?;
        load_pages_data(&rpc, Some(&page_id))
            .await
            .map_err(committed_error)
    }
    .await;
    result
        .map(|data| BlockInsertResult {
            data,
            operation_id: operation_id.clone(),
            page_id: operation_scope.clone(),
        })
        .map_err(|cause: AppError| OptimisticMutationError {
            message: cause.message,
            committed: cause.committed,
            operation_id,
            scope_id: operation_scope,
            body: operation_body,
        })
}

pub async fn save_block(
    rpc: String,
    password: String,
    page_id: String,
    block_id: String,
    kind: String,
    text: String,
) -> Result<PagesData, AppError> {
    async {
        let kind = parse_block_kind(&kind)?;
        let text = bounded_updated_block_text(kind, text)?;
        let rpc = rpc_client(&rpc)?;
        let blocks = load_page_blocks(&rpc, &page_id).await?;
        let block = blocks
            .iter()
            .find(|block| block.id == block_id)
            .ok_or_else(|| "block was not found".to_string())?;
        let text_changed = block.text != text;
        let kind_changed = block.kind != kind;
        if text_changed {
            signed_write(
                &rpc,
                "pages",
                pages::encode_msg(&PageMsg::UpdateText {
                    block_id: block_id.clone(),
                    text,
                    marks: None,
                }),
                password.clone(),
            )
            .await?;
            if kind_changed {
                signed_write(
                    &rpc,
                    "pages",
                    pages::encode_msg(&PageMsg::SetKind {
                        block_id: block_id.clone(),
                        kind,
                    }),
                    password,
                )
                .await
                .map_err(committed_error)?;
            }
            return load_selected_page_data(&rpc, &page_id, &block_id)
                .await
                .map_err(committed_error);
        }
        if kind_changed {
            signed_write(
                &rpc,
                "pages",
                pages::encode_msg(&PageMsg::SetKind {
                    block_id: block_id.clone(),
                    kind,
                }),
                password,
            )
            .await?;
            return load_selected_page_data(&rpc, &page_id, &block_id)
                .await
                .map_err(committed_error);
        }
        load_selected_page_data(&rpc, &page_id, &block_id)
            .await
            .map_err(app_error)
    }
    .await
}

pub async fn set_block_checked(
    rpc: String,
    password: String,
    page_id: String,
    block_id: String,
    checked: bool,
) -> Result<PagesData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::SetChecked {
                block_id: block_id.clone(),
                checked,
            }),
            password,
        )
        .await?;
        load_selected_page_data(&rpc, &page_id, &block_id)
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn move_block(
    rpc: String,
    password: String,
    page_id: String,
    block_id: String,
    direction: String,
) -> Result<PagesData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let blocks = load_page_blocks(&rpc, &page_id).await?;
        let (parent, after) = block_move(&blocks, &block_id, &direction)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::MoveBlock {
                block_id: block_id.clone(),
                parent,
                after,
            }),
            password,
        )
        .await?;
        load_selected_page_data(&rpc, &page_id, &block_id)
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn remove_block(
    rpc: String,
    password: String,
    page_id: String,
    block_id: String,
) -> Result<PagesData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::RemoveBlock { block_id }),
            password,
        )
        .await?;
        load_pages_data(&rpc, Some(&page_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn search_pages(
    rpc: String,
    page_id: String,
    text: String,
    generation: i64,
) -> Result<PageSearchData, HydrationError> {
    let result = async {
        let text = bounded_text(text, "search", 512)?;
        let rpc = rpc_client(&rpc)?;
        let reply: pages::index::PagesViewReply = rpc
            .view(
                "pages",
                &serde_json::json!({
                    "search": {
                        "text": text,
                        "page_id": (!page_id.is_empty()).then_some(page_id),
                        "limit": 50
                    }
                }),
            )
            .await?;
        let pages::index::PagesViewReply::Hits(hits) = reply else {
            return Err("page search returned an invalid reply".into());
        };
        Ok(PageSearchData {
            generation,
            hits: hits
                .into_iter()
                .map(|hit| PageSearchHit {
                    page_id: hit.page_id,
                    block_id: hit.block_id,
                    kind: block_kind_name(hit.kind).into(),
                    text: hit.text,
                })
                .collect(),
        })
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message,
    })
}

async fn load_workspace(
    rpc: &RpcClient,
    channel_id: Option<&str>,
    page_id: Option<&str>,
    generation: i64,
) -> Result<WorkspaceData, String> {
    let tip = tip_from_status(rpc.status().await?)?;
    let chat = load_chat_data(rpc, channel_id).await?;
    let pages = load_pages_data(rpc, page_id).await?;
    Ok(WorkspaceData {
        generation,
        rpc: rpc.origin().to_string(),
        status: tip.status,
        height: tip.height,
        channels: chat.channels,
        messages: chat.messages,
        active_channel: chat.active_channel,
        active_channel_name: chat.active_channel_name,
        active_channel_archived: chat.active_channel_archived,
        active_channel_members_only: chat.active_channel_members_only,
        active_channel_huddle_count: chat.active_channel_huddle_count,
        channel_members: chat.channel_members,
        pages: pages.pages,
        blocks: pages.blocks,
        active_page: pages.active_page,
        active_page_title: pages.active_page_title,
        active_page_parent: pages.active_page_parent,
    })
}

fn tip_from_status(status: NodeStatus) -> Result<Tip, String> {
    let height = i64::try_from(status.height).map_err(|_| "node height exceeds i64")?;
    Ok(Tip {
        height,
        status: format!("Connected · block {height}"),
    })
}

async fn load_chat_data(rpc: &RpcClient, requested: Option<&str>) -> Result<ChatData, String> {
    let mut wire_channels = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let reply: ChatViewReply = rpc
            .view(
                "chat",
                &ChatViewQuery::Channels {
                    after: after.clone(),
                    limit: None,
                },
            )
            .await?;
        let ChatViewReply::Channels {
            channels: page,
            has_more,
            next_after,
        } = reply
        else {
            return Err("node returned an invalid channel list".into());
        };
        wire_channels.extend(page);
        if !has_more {
            break;
        }
        after = next_after;
        if after.is_none() {
            break;
        }
    }
    let channels = wire_channels
        .iter()
        .map(|info| ChatChannel {
            id: info.channel.id.clone(),
            name: info.channel.name.clone(),
            archived: info.channel.archived,
            members_only: info.channel.post_policy == PostPolicy::MembersOnly,
            huddle_count: count_i64(info.channel.huddle.len()),
            head_seq: number_i64(info.head_seq),
        })
        .collect::<Vec<_>>();
    let active_channel = requested
        .filter(|id| channels.iter().any(|channel| channel.id == *id))
        .map(str::to_string)
        .or_else(|| channels.first().map(|channel| channel.id.clone()))
        .unwrap_or_default();
    let active_channel_name = channels
        .iter()
        .find(|channel| channel.id == active_channel)
        .map(|channel| channel.name.clone())
        .unwrap_or_default();
    let active_wire_channel = wire_channels
        .iter()
        .find(|info| info.channel.id == active_channel);
    let active_channel_archived = active_wire_channel.is_some_and(|info| info.channel.archived);
    let active_channel_members_only = active_wire_channel
        .is_some_and(|info| info.channel.post_policy == PostPolicy::MembersOnly);
    let active_channel_huddle_count =
        active_wire_channel.map_or(0, |info| count_i64(info.channel.huddle.len()));
    let active_channel_head_seq = active_wire_channel.map_or(0, |info| info.head_seq);
    let channel_members = if active_channel.is_empty() {
        Vec::new()
    } else {
        load_channel_members(rpc, &active_channel).await?
    };
    let messages = if active_channel.is_empty() {
        Vec::new()
    } else {
        load_messages(rpc, &active_channel, active_channel_head_seq).await?
    };
    Ok(ChatData {
        channels,
        messages,
        active_channel,
        active_channel_name,
        active_channel_archived,
        active_channel_members_only,
        active_channel_huddle_count,
        channel_members,
        selected_message_seq: 0,
        selected_message_rev: 0,
        selected_message_body: String::new(),
        active_thread_seq: 0,
        thread_target_seq: 0,
        thread_messages: Vec::new(),
        thread_next_reply_offset: 0,
        thread_has_more: false,
    })
}

async fn load_channel_members(
    rpc: &RpcClient,
    channel_id: &str,
) -> Result<Vec<ChatMember>, String> {
    let mut members = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let reply: ChatViewReply = rpc
            .view(
                "chat",
                &ChatViewQuery::Members {
                    channel_id: channel_id.to_string(),
                    after: after.clone(),
                    limit: None,
                },
            )
            .await?;
        let ChatViewReply::Members {
            members: page,
            has_more,
            next_after,
        } = reply
        else {
            return Err("node returned an invalid channel member list".into());
        };
        members.extend(page);
        if !has_more {
            break;
        }
        after = next_after;
        if after.is_none() {
            break;
        }
    }
    Ok(members
        .into_iter()
        .map(|member| {
            let id = member_id(&member.user);
            ChatMember {
                label: short_label(id),
                key: id.to_string(),
            }
        })
        .collect())
}

/// The member's key id: the part after `user:` in a rendered member handle,
/// or the whole handle when it carries no such prefix.
fn member_id(user: &str) -> &str {
    user.strip_prefix("user:").unwrap_or(user)
}

pub async fn load_older_messages(
    rpc: String,
    channel_id: String,
    before_seq: i64,
    generation: i64,
) -> Result<HistoryPageData, HydrationError> {
    let result = async {
        let rpc = rpc_client(&rpc)?;
        let before = u64::try_from(before_seq).unwrap_or(0);
        let mut cursor = before.saturating_sub(1);
        let mut roots = Vec::new();
        while cursor > 0 && roots.len() < CHAT_TIMELINE_ROOT_LIMIT {
            let limit = cursor.min(CHAT_VIEW_PAGE_LIMIT);
            let from_seq = cursor - limit + 1;
            let reply: ChatViewReply = rpc
                .view(
                    "chat",
                    &ChatViewQuery::MessagesRange {
                        channel_id: channel_id.clone(),
                        from_seq,
                        limit: Some(limit as usize),
                    },
                )
                .await?;
            let ChatViewReply::Messages(rows) = reply else {
                return Err("node returned an invalid message list".to_string());
            };
            roots.extend(rows.into_iter().filter(|row| row.thread.is_none()));
            if from_seq == 1 {
                break;
            }
            cursor = from_seq - 1;
        }
        roots.sort_by_key(|row| row.seq);
        let excess = roots.len().saturating_sub(CHAT_TIMELINE_ROOT_LIMIT);
        roots.drain(..excess);
        let current_user = local_user_key().await;
        let messages: Vec<ChatMessage> = roots
            .into_iter()
            .map(|row| chat_message(row, current_user.as_deref()))
            .collect();
        Ok(messages)
    }
    .await;
    result
        .map(|messages| HistoryPageData {
            generation,
            messages,
        })
        .map_err(|message| HydrationError {
            generation,
            message,
        })
}

async fn load_messages_around(
    rpc: &RpcClient,
    channel_id: &str,
    seq: u64,
) -> Result<Vec<ChatMessage>, String> {
    let reply: ChatViewReply = rpc
        .view(
            "chat",
            &ChatViewQuery::MessagesAround {
                channel_id: channel_id.to_string(),
                seq,
                limit: Some(CHAT_VIEW_PAGE_LIMIT as usize),
            },
        )
        .await?;
    let ChatViewReply::Messages(rows) = reply else {
        return Err("node returned an invalid message window".into());
    };
    let current_user = local_user_key().await;
    Ok(rows
        .into_iter()
        .filter(|row| row.thread.is_none())
        .map(|row| chat_message(row, current_user.as_deref()))
        .collect())
}

async fn load_message_at(
    rpc: &RpcClient,
    channel_id: &str,
    seq: u64,
) -> Result<MsgRow, String> {
    let reply: ChatViewReply = rpc
        .view(
            "chat",
            &ChatViewQuery::MessagesAround {
                channel_id: channel_id.to_string(),
                seq,
                limit: Some(1),
            },
        )
        .await?;
    let ChatViewReply::Messages(rows) = reply else {
        return Err("node returned an invalid message window".into());
    };
    rows.into_iter()
        .find(|row| row.seq == seq)
        .ok_or_else(|| "message was not found".into())
}

async fn load_messages(
    rpc: &RpcClient,
    channel_id: &str,
    head_seq: u64,
) -> Result<Vec<ChatMessage>, String> {
    let mut cursor = head_seq;
    let mut roots = Vec::new();
    while cursor > 0 && roots.len() < CHAT_TIMELINE_ROOT_LIMIT {
        let limit = cursor.min(CHAT_VIEW_PAGE_LIMIT);
        let from_seq = cursor - limit + 1;
        let reply: ChatViewReply = rpc
            .view(
                "chat",
                &ChatViewQuery::MessagesRange {
                    channel_id: channel_id.to_string(),
                    from_seq,
                    limit: Some(limit as usize),
                },
            )
            .await?;
        let ChatViewReply::Messages(rows) = reply else {
            return Err("node returned an invalid message list".into());
        };
        roots.extend(rows.into_iter().filter(|row| row.thread.is_none()));
        if from_seq == 1 {
            break;
        }
        cursor = from_seq - 1;
    }
    roots.sort_by_key(|row| row.seq);
    let excess = roots.len().saturating_sub(CHAT_TIMELINE_ROOT_LIMIT);
    roots.drain(..excess);
    let current_user = local_user_key().await;
    let mut messages: Vec<ChatMessage> = roots
        .into_iter()
        .map(|row| chat_message(row, current_user.as_deref()))
        .collect();
    mark_message_groups(&mut messages);
    Ok(messages)
}

/// One page of older history, returned to the reducer with the generation that
/// requested it so a stale load can be discarded.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct HistoryPageData {
    pub generation: i64,
    pub messages: Vec<ChatMessage>,
}

/// True when the oldest loaded root is not the channel's first message, i.e.
/// there is older history to page in.
pub fn history_has_older(messages: Vec<ChatMessage>) -> bool {
    messages.first().is_some_and(|message| message.seq > 1)
}

/// The seq of the oldest loaded root (the ceiling for the next older page).
pub fn oldest_message_seq(messages: Vec<ChatMessage>) -> i64 {
    messages.first().map_or(0, |message| message.seq)
}

/// Prepend an older page ahead of the current timeline, de-duped by seq, sorted
/// oldest-first, and re-grouped so the seam between pages regroups correctly.
pub fn prepend_history(messages: Vec<ChatMessage>, older: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let known: BTreeSet<i64> = messages.iter().map(|message| message.seq).collect();
    let mut merged: Vec<ChatMessage> = older
        .into_iter()
        .filter(|message| !known.contains(&message.seq))
        .chain(messages)
        .collect();
    merged.sort_by_key(|message| message.seq);
    mark_message_groups(&mut merged);
    merged
}


/// One thread's root plus its complete reply run, walked over the view's reply
/// cursor to exhaustion.
struct ThreadPage {
    root: MsgRow,
    replies: Vec<MsgRow>,
}

async fn query_thread_page(
    rpc: &RpcClient,
    channel_id: &str,
    root_seq: u64,
) -> Result<ThreadPage, String> {
    let mut root = None;
    let mut replies = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let reply: ChatViewReply = rpc
            .view(
                "chat",
                &ChatViewQuery::Thread {
                    channel_id: channel_id.to_string(),
                    root_seq,
                    after: after.clone(),
                    limit: None,
                },
            )
            .await?;
        let ChatViewReply::Thread {
            root: page_root,
            replies: page_replies,
            has_more,
            next_after,
        } = reply
        else {
            return Err("thread was not found".into());
        };
        if root.is_none() {
            let Some(page_root) = page_root else {
                return Err("thread was not found".into());
            };
            root = Some(page_root);
        }
        replies.extend(page_replies);
        if !has_more {
            break;
        }
        after = next_after;
        if after.is_none() {
            break;
        }
    }
    let root = root.ok_or_else(|| "thread was not found".to_string())?;
    Ok(ThreadPage { root, replies })
}

async fn load_sparse_thread_data(
    rpc: &RpcClient,
    channel_id: &str,
    root_seq: u64,
    target_seq: u64,
) -> Result<ThreadData, String> {
    let root = load_message_at(rpc, channel_id, root_seq).await?;
    let target = load_message_at(rpc, channel_id, target_seq).await?;
    if target.thread != Some(root_seq) {
        return Err("search result does not belong to the selected thread".into());
    }
    let current_user = local_user_key().await;
    Ok(ThreadData {
        root_seq: number_i64(root_seq),
        target_seq: number_i64(target_seq),
        messages: vec![
            chat_message(root, current_user.as_deref()),
            chat_message(target, current_user.as_deref()),
        ],
        next_reply_offset: -1,
        has_more: false,
    })
}

async fn load_thread_data(
    rpc: &RpcClient,
    channel_id: &str,
    root_seq: u64,
    through_reply_offset: u64,
) -> Result<ThreadData, String> {
    if channel_id.is_empty() || root_seq == 0 {
        return Ok(ThreadData {
            root_seq: 0,
            target_seq: 0,
            messages: Vec::new(),
            next_reply_offset: 0,
            has_more: false,
        });
    }

    // the view walks the reply cursor to exhaustion, so the whole thread (up to
    // the module's reply cap) arrives in one call; re-page it into the UI's
    // MAX_QUERY_LIMIT windows so the reducer's cursor contract is unchanged.
    let thread = query_thread_page(rpc, channel_id, root_seq).await?;
    let (loaded, has_more) = thread_page_bound(thread.replies.len() as u64, through_reply_offset);
    let current_user = local_user_key().await;
    let messages = std::iter::once(thread.root)
        .chain(thread.replies.into_iter().take(loaded as usize))
        .map(|row| chat_message(row, current_user.as_deref()))
        .collect();
    Ok(ThreadData {
        root_seq: number_i64(root_seq),
        target_seq: 0,
        messages,
        next_reply_offset: number_i64(loaded),
        has_more,
    })
}

/// Re-page a fully-loaded thread into the branch's MAX_QUERY_LIMIT windows:
/// how many replies to surface for `through_reply_offset`, and whether more
/// remain. Mirrors the old page-walk (has_more keys on a full page, capped at
/// MAX_THREAD_REPLIES).
fn thread_page_bound(total: u64, through_reply_offset: u64) -> (u64, bool) {
    let cap = chat::MAX_THREAD_REPLIES as u64;
    let mut from = 0;
    loop {
        let page_len = CHAT_VIEW_PAGE_LIMIT.min(total - from);
        from += page_len;
        let page_is_full = page_len == CHAT_VIEW_PAGE_LIMIT;
        let thread_cap_reached = from >= cap;
        let has_more = page_is_full && !thread_cap_reached;
        let first_page_is_enough = through_reply_offset == 0;
        let requested_offset_is_loaded = from >= through_reply_offset;
        if !has_more || first_page_is_enough || requested_offset_is_loaded {
            return (from, has_more);
        }
    }
}

async fn query_block_threads(
    rpc: &RpcClient,
    target: &str,
    from: u32,
    generation: i64,
) -> Result<BlockThreadListData, String> {
    let reply: PagesViewReply = rpc
        .view(
            "pages",
            &PagesViewQuery::ThreadsForTargets {
                targets: vec![target.to_string()],
            },
        )
        .await?;
    let PagesViewReply::Threads(groups) = reply else {
        return Err("node returned an invalid comment thread page".into());
    };
    // threads-per-target is consensus-capped, so the whole list arrives in one
    // grouped reply — there is no offset paging to resume.
    let threads = groups
        .into_iter()
        .find(|group| group.target == target)
        .map(|group| group.threads)
        .unwrap_or_default();
    let total = count_i64(threads.len());
    Ok(BlockThreadListData {
        generation,
        target: target.to_string(),
        from: i64::from(from),
        threads: threads.into_iter().map(page_comment_thread).collect(),
        total,
        next_from: 0,
        has_more: false,
    })
}

async fn query_block_comment_page(
    rpc: &RpcClient,
    target: &str,
    thread_id: &str,
    from: u32,
    generation: i64,
) -> Result<Option<BlockCommentData>, String> {
    let reply: PageReply = rpc
        .query(
            "pages",
            &PageQuery::CommentThread {
                thread_id: thread_id.to_string(),
            },
        )
        .await?;
    let PageReply::CommentThread(thread) = reply else {
        return Err("node returned an invalid comment page".into());
    };
    let Some(view) = thread else {
        return Ok(None);
    };
    let is_expected_thread = view.thread.id == thread_id && view.thread.target == target;
    if !is_expected_thread {
        return Err("node returned comments for another block".into());
    }
    // the committed read returns the whole live comment list; the UI's page
    // ordinals are 1-based positions in it, sliced from `from`.
    let comments = view
        .comments
        .into_iter()
        .enumerate()
        .skip(from as usize)
        .map(|(index, comment)| page_comment(index + 1, comment))
        .collect();
    Ok(Some(BlockCommentData {
        generation,
        target: view.thread.target,
        thread_id: view.thread.id,
        from: i64::from(from),
        comments,
        next_from: 0,
        has_more: false,
    }))
}

fn page_comment_thread(thread: ThreadRow) -> PageCommentThread {
    let comment_count = count_i64(thread.comments.iter().filter(|c| !c.deleted).count());
    let count_label = if comment_count == 1 {
        "1 comment".to_string()
    } else {
        format!("{comment_count} comments")
    };
    PageCommentThread {
        id: thread.id,
        author: author_name(&thread.opener),
        meta: if thread.resolved {
            format!("{count_label} · resolved")
        } else {
            count_label
        },
        resolved: thread.resolved,
        comment_count,
    }
}

fn page_comment(ordinal: usize, comment: pages::Comment) -> PageComment {
    let edited = comment.edited_at.is_some();
    let ordinal = count_i64(ordinal);
    PageComment {
        id: comment.id,
        ordinal,
        author: page_author_name(&comment.author),
        meta: if edited {
            format!("#{ordinal} · edited")
        } else {
            format!("#{ordinal}")
        },
        text: comment.text,
    }
}

fn page_author_name(author: &pages::AuthorRef) -> String {
    match author {
        pages::AuthorRef::User(key) => format!("user {}", short_hex(key)),
        pages::AuthorRef::Agent { agent_id, .. } => format!("@{agent_id}"),
        pages::AuthorRef::Module(module) => module.clone(),
        pages::AuthorRef::System => "system".into(),
    }
}

async fn load_pages_data(rpc: &RpcClient, requested: Option<&str>) -> Result<PagesData, String> {
    let wire_pages = load_page_index(rpc).await?;
    let pages = page_items(wire_pages);
    let active_page = requested
        .filter(|id| pages.iter().any(|page| page.id == *id))
        .map(str::to_string)
        .or_else(|| pages.first().map(|page| page.id.clone()))
        .unwrap_or_default();
    let active_page_parent = pages
        .iter()
        .find(|page| page.id == active_page)
        .map(|page| page.parent.clone())
        .unwrap_or_default();
    if active_page.is_empty() {
        return Ok(PagesData {
            pages,
            blocks: Vec::new(),
            active_page,
            active_page_title: String::new(),
            active_page_parent,
            selected_block_id: String::new(),
            selected_block_kind: String::new(),
            selected_block_text: String::new(),
            selected_block_checked: false,
            page_title_selected: false,
        });
    }
    let wire_blocks = load_page_blocks(rpc, &active_page).await?;
    let active_page_title = wire_blocks
        .first()
        .map(|block| block.text.clone())
        .unwrap_or_default();
    let blocks = page_blocks(wire_blocks, &active_page);
    Ok(PagesData {
        pages,
        blocks,
        active_page,
        active_page_title,
        active_page_parent,
        selected_block_id: String::new(),
        selected_block_kind: String::new(),
        selected_block_text: String::new(),
        selected_block_checked: false,
        page_title_selected: false,
    })
}

fn page_blocks(wire_blocks: Vec<pages::Block>, active_page: &str) -> Vec<PageBlock> {
    let parents = wire_blocks
        .iter()
        .map(|block| (block.id.clone(), block.parent.clone()))
        .collect::<BTreeMap<_, _>>();
    wire_blocks
        .into_iter()
        .skip(1)
        .map(|block| PageBlock {
            key: page_block_key(&block.id),
            prefix: block_prefix(&block, active_page, &parents),
            id: block.id,
            parent: block.parent.unwrap_or_default(),
            kind: block_kind_name(block.kind).into(),
            text: block.text,
            pending: false,
            checked: block.checked,
            child_count: count_i64(block.children.len()),
            mark_count: count_i64(block.marks.len()),
        })
        .collect()
}

fn page_block_key(id: &str) -> i64 {
    // ponytail: session-wide interning is collision-free; scope it per workspace
    // only if retaining every visited block id becomes measurable.
    static KEYS: OnceLock<Mutex<BTreeMap<String, i64>>> = OnceLock::new();
    let mut keys = KEYS
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(key) = keys.get(id) {
        return *key;
    }
    let key = count_i64(keys.len());
    keys.insert(id.to_owned(), key);
    key
}

fn with_selected_block(mut pages: PagesData, selected_block_id: &str) -> PagesData {
    if !selected_block_id.is_empty() && selected_block_id == pages.active_page {
        pages.page_title_selected = true;
        return pages;
    }
    let Some(block) = pages
        .blocks
        .iter()
        .find(|block| block.id == selected_block_id)
    else {
        return pages;
    };
    pages.selected_block_id.clone_from(&block.id);
    pages.selected_block_kind.clone_from(&block.kind);
    pages.selected_block_text.clone_from(&block.text);
    pages.selected_block_checked = block.checked;
    pages
}

async fn load_selected_page_data(
    rpc: &RpcClient,
    page_id: &str,
    block_id: &str,
) -> Result<PagesData, String> {
    load_pages_data(rpc, Some(page_id))
        .await
        .map(|pages| with_selected_block(pages, block_id))
}

async fn load_page_blocks(rpc: &RpcClient, page_id: &str) -> Result<Vec<pages::Block>, String> {
    let mut blocks = Vec::new();
    let mut after = None;
    loop {
        let reply: PageReply = rpc
            .query(
                "pages",
                &PageQuery::GetPage {
                    page_id: page_id.to_string(),
                    after: after.clone(),
                    limit: 0,
                },
            )
            .await?;
        let page = match reply {
            PageReply::Page(Some(page)) => page,
            _ => return Err("page was not found".into()),
        };
        blocks.extend(page.blocks);
        let Some(next) = page.next_after else {
            return Ok(blocks);
        };
        if after.as_ref() == Some(&next) {
            return Err("node repeated the page cursor".into());
        }
        after = Some(next);
    }
}

async fn load_page_index(rpc: &RpcClient) -> Result<Vec<PageRow>, String> {
    let mut pages = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let reply: PagesViewReply = rpc
            .view(
                "pages",
                &PagesViewQuery::ListPages {
                    after: after.clone(),
                    limit: None,
                },
            )
            .await?;
        let PagesViewReply::Pages {
            pages: page,
            has_more,
            next_after,
        } = reply
        else {
            return Err("node returned an invalid page list".into());
        };
        pages.extend(page);
        if !has_more {
            return Ok(pages);
        }
        let Some(next) = next_after else {
            return Ok(pages);
        };
        if after.as_ref() == Some(&next) {
            return Err("node repeated the page-list cursor".into());
        }
        after = Some(next);
    }
}

fn page_items(wire_pages: Vec<PageRow>) -> Vec<PageItem> {
    let known = wire_pages
        .iter()
        .map(|page| page.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut children = BTreeMap::<Option<&str>, Vec<usize>>::new();
    for (index, page) in wire_pages.iter().enumerate() {
        let parent = page
            .parent
            .as_deref()
            .filter(|parent| known.contains(parent));
        children.entry(parent).or_default().push(index);
    }
    let mut stack = children
        .get(&None)
        .into_iter()
        .flatten()
        .rev()
        .map(|index| (*index, 0_usize))
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let mut pages = Vec::with_capacity(wire_pages.len());
    while pages.len() < wire_pages.len() {
        let Some((index, depth)) = stack.pop() else {
            let Some(index) = wire_pages
                .iter()
                .position(|page| !visited.contains(page.id.as_str()))
            else {
                break;
            };
            stack.push((index, 0));
            continue;
        };
        let page = &wire_pages[index];
        if !visited.insert(page.id.as_str()) {
            continue;
        }
        let page_children = children.get(&Some(page.id.as_str()));
        pages.push(PageItem {
            id: page.id.clone(),
            title: if page.title.is_empty() {
                "Untitled".into()
            } else {
                page.title.clone()
            },
            parent: page.parent.clone().unwrap_or_default(),
            prefix: "  ".repeat(depth),
            child_count: page_children.map_or(0, |children| count_i64(children.len())),
        });
        if let Some(page_children) = page_children {
            stack.extend(page_children.iter().rev().map(|index| (*index, depth + 1)));
        }
    }
    pages
}

fn block_prefix(
    block: &pages::Block,
    page_id: &str,
    parents: &BTreeMap<String, Option<String>>,
) -> String {
    let mut depth = 0;
    let mut parent = block.parent.as_deref();
    while let Some(parent_id) = parent {
        if parent_id == page_id || depth >= parents.len() {
            break;
        }
        depth += 1;
        parent = parents.get(parent_id).and_then(Option::as_deref);
    }
    "  ".repeat(depth)
}

async fn signed_write(
    rpc: &RpcClient,
    target: &str,
    payload: Vec<u8>,
    password: String,
) -> Result<(), String> {
    if payload.is_empty() || payload.len() > MAX_SIGNED_PAYLOAD_BYTES {
        return Err(format!(
            "{target} transaction exceeds the signed payload limit"
        ));
    }
    let frame = sign_frame(target, &payload, password).await?;
    rpc.submit_frame(frame).await.map_err(Into::into)
}

async fn sign_frame(target: &str, payload: &[u8], mut password: String) -> Result<Vec<u8>, String> {
    let key = user_key_path()?;
    require_encrypted_key(&key)?;
    let payload_hex = hex_encode(payload);
    let input = signing_input(&password, &payload_hex);
    password.zeroize();
    let input = Zeroizing::new(input?);
    let mut command = tokio::process::Command::new(ducktape_binary());
    command
        .arg("user")
        .arg("sign-frame")
        .arg("--key")
        .arg(&key)
        .arg("--target")
        .arg(target)
        .arg("--seq")
        .arg(next_sequence().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| {
        format!("could not start the ducktape signer ({error}); build node-bin or set DUCKTAPE_BIN")
    })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "ducktape signer stdin is unavailable".to_string())?;
    stdin
        .write_all(&input)
        .await
        .map_err(|error| format!("could not send payload to signer: {error}"))?;
    drop(stdin);
    let output = tokio::time::timeout(RPC_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "ducktape signer timed out".to_string())?
        .map_err(|error| format!("ducktape signer failed: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ducktape signer refused the transaction: {}",
            bounded_detail(&detail)
        ));
    }
    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|_| "ducktape signer returned non-UTF-8 output".to_string())?;
    let frame_hex = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "ducktape signer returned no frame".to_string())?;
    hex_decode(frame_hex.trim())
}

fn signing_input(password: &str, payload_hex: &str) -> Result<Vec<u8>, String> {
    let invalid_password = password.len() > 16 * 1024
        || password
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, 0 | b'\n' | b'\r'));
    if invalid_password {
        return Err("key password is too long or contains a line delimiter".into());
    }
    if password.is_empty() {
        return Err("the local user key is locked; enter its password".into());
    }
    let mut input = Vec::with_capacity(password.len() + payload_hex.len() + 2);
    input.extend_from_slice(password.as_bytes());
    input.push(b'\n');
    input.extend_from_slice(payload_hex.as_bytes());
    input.push(b'\n');
    Ok(input)
}

async fn local_user_key() -> Option<Vec<u8>> {
    // ponytail: cache the launch identity; restart the app after replacing user.key.
    static KEY: tokio::sync::OnceCell<Option<Vec<u8>>> = tokio::sync::OnceCell::const_new();
    KEY.get_or_init(read_local_user_key).await.clone()
}

async fn read_local_user_key() -> Option<Vec<u8>> {
    let key = user_key_path().ok()?;
    let mut command = tokio::process::Command::new(ducktape_binary());
    command
        .arg("user")
        .arg("key")
        .arg("status")
        .arg("--key")
        .arg(key)
        .kill_on_drop(true);
    let output = tokio::time::timeout(RPC_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() || output.stdout.len() > 256 {
        return None;
    }
    parse_user_key_status(std::str::from_utf8(&output.stdout).ok()?)
}

fn parse_user_key_status(status: &str) -> Option<Vec<u8>> {
    let mut fields = status.split_whitespace();
    match fields.next()? {
        "encrypted" => {}
        _ => return None,
    }
    let key = fields.next()?;
    if fields.next().is_some() {
        return None;
    }
    public_key(key, "local user key").ok()
}

fn user_key_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("DUCKTAPE_USER_KEY") {
        return Ok(path.into());
    }
    if let Some(root) = std::env::var_os("DUCKTAPE_HOME") {
        return Ok(PathBuf::from(root).join("user.key"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".ducktape/user.key"))
        .ok_or_else(|| "cannot locate local user.key; set DUCKTAPE_USER_KEY".to_string())
}

fn require_encrypted_key(path: &std::path::Path) -> Result<(), String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("cannot read local user key at {}: {error}", path.display()))?;
    if metadata.len() > MAX_KEY_FILE_BYTES {
        return Err("local user key file is unexpectedly large".into());
    }
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("cannot read local user key at {}: {error}", path.display()))?;
    let mut prefix = [0; ENCRYPTED_KEY_PREFIX.len()];
    let read = file
        .read(&mut prefix)
        .map_err(|error| format!("cannot read local user key at {}: {error}", path.display()))?;
    let encrypted = read == prefix.len() && prefix == ENCRYPTED_KEY_PREFIX.as_bytes();
    prefix.zeroize();
    if encrypted {
        Ok(())
    } else {
        Err("local user key must use the encrypted v1 format".into())
    }
}

fn ducktape_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("DUCKTAPE_BIN") {
        return path.into();
    }
    if let Ok(current) = std::env::current_exe()
        && let Some(sibling) = current.parent().map(|parent| parent.join("ducktape"))
        && sibling.is_file()
    {
        return sibling;
    }
    PathBuf::from("ducktape")
}

fn bounded_text(value: String, field: &str, limit: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > limit || value.chars().any(|character| character == '\0') {
        return Err(format!("{field} must be between 1 and {limit} bytes"));
    }
    Ok(value.to_string())
}

fn bounded_exact_text(value: String, field: &str, limit: usize) -> Result<String, String> {
    let invalid = value.len() > limit || value.chars().any(|character| character == '\0');
    if invalid {
        return Err(format!(
            "{field} must be at most {limit} bytes and contain no NUL"
        ));
    }
    Ok(value)
}

fn required_id(value: String, subject: &str) -> Result<String, String> {
    bounded_text(value, &format!("{subject} id"), 512)
}

fn public_key(value: &str, field: &str) -> Result<Vec<u8>, String> {
    let value = value.trim();
    let expected = chat::HUDDLE_NODE_KEY_BYTES * 2;
    if value.len() != expected {
        return Err(format!("{field} must be {expected} hexadecimal characters"));
    }
    hex_decode(value).map_err(|_| format!("{field} must be hexadecimal"))
}

fn positive_sequence(value: i64) -> Result<u64, String> {
    u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "message sequence must be positive".into())
}

fn app_error(message: String) -> AppError {
    message.into()
}

fn committed_error(message: String) -> AppError {
    AppError {
        message,
        committed: true,
    }
}

fn retry_delay(attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1).min(4);
    Duration::from_secs(1_u64 << exponent)
}

fn number_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn count_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn live_update(kind: &str, status: &str, height: i64) -> LiveUpdate {
    LiveUpdate {
        kind: kind.into(),
        status: status.into(),
        height,
        module: String::new(),
        load_chat: kind == "ready",
        load_pages: kind == "ready",
        debounce: false,
        chat: ChatDelta::default(),
        pages: PagesDelta::default(),
    }
}

fn live_retry(_message: String) -> LiveUpdate {
    live_update("retry", "Reconnecting…", -1)
}

/// A module's replay is unavailable or unfoldable — the handler reloads that
/// module's slices instead of folding.
fn live_resync(module: &str, height: i64) -> LiveUpdate {
    let mut update = live_update("resync", "Live · resyncing", height);
    update.module = module.to_string();
    update.load_chat = module == "chat";
    update.load_pages = module == "pages";
    update
}

 /// Container style for a per-author avatar: the data-driven tint fill with a
/// rounded-square radius and a soft drop shadow. `bg=` only takes static colors,
/// so the dynamic tint rides in through a `container-style` extern.
pub fn avatar_style(_theme: &iced::Theme, r: f64, g: f64, b: f64) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgb(
            r as f32, g as f32, b as f32,
        ))),
        border: iced::Border {
            radius: 10.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.30),
            offset: iced::Vector::new(0.0, 1.0),
            blur_radius: 4.0,
        },
        ..Default::default()
    }
}

fn short_hex(bytes: &[u8]) -> String {
    let mut output = String::new();
    for byte in bytes.iter().take(4) {
        let _ = write!(output, "{byte:02x}");
    }
    if bytes.len() > 4 {
        output.push('…');
    }
    output
}

/// A shortened display label for an id string: its first 8 characters, with an
/// ellipsis when more follow.
const fn block_kind_name(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Page => "Page",
        BlockKind::Paragraph => "Text",
        BlockKind::Heading1 => "Heading 1",
        BlockKind::Heading2 => "Heading 2",
        BlockKind::Heading3 => "Heading 3",
        BlockKind::Bulleted => "Bullet",
        BlockKind::Numbered => "Number",
        BlockKind::Todo => "Todo",
        BlockKind::Toggle => "Toggle",
        BlockKind::Quote => "Quote",
        BlockKind::Code => "Code",
        BlockKind::Callout => "Callout",
        BlockKind::Divider => "Divider",
    }
}

fn parse_block_kind(kind: &str) -> Result<BlockKind, String> {
    match kind {
        "Page" => Ok(BlockKind::Page),
        "Text" => Ok(BlockKind::Paragraph),
        "Heading 1" => Ok(BlockKind::Heading1),
        "Heading 2" => Ok(BlockKind::Heading2),
        "Heading 3" => Ok(BlockKind::Heading3),
        "Bullet" => Ok(BlockKind::Bulleted),
        "Number" => Ok(BlockKind::Numbered),
        "Todo" => Ok(BlockKind::Todo),
        "Toggle" => Ok(BlockKind::Toggle),
        "Quote" => Ok(BlockKind::Quote),
        "Code" => Ok(BlockKind::Code),
        "Callout" => Ok(BlockKind::Callout),
        "Divider" => Ok(BlockKind::Divider),
        _ => Err("choose a valid block type".into()),
    }
}

fn bounded_new_block_text(kind: BlockKind, text: String) -> Result<String, String> {
    if kind == BlockKind::Divider {
        return Ok(String::new());
    }
    let field = if kind == BlockKind::Page {
        "page title"
    } else {
        "block text"
    };
    let limit = if kind == BlockKind::Page {
        512
    } else {
        64 * 1024
    };
    if text.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    bounded_exact_text(text, field, limit)
}

fn bounded_updated_block_text(kind: BlockKind, text: String) -> Result<String, String> {
    if kind == BlockKind::Divider {
        return Ok(String::new());
    }
    if kind == BlockKind::Page {
        return bounded_exact_text(text, "page title", 512);
    }
    bounded_exact_text(text, "block text", 64 * 1024)
}

async fn debounced_page_text(
    rpc: String,
    mut password: String,
    block_id: String,
    text: String,
) -> Result<bool, String> {
    let key = format!("{rpc}\0{block_id}");
    let ticket = begin_autosave(&key);
    tokio::time::sleep(Duration::from_millis(400)).await;
    if !autosave_is_current(&key, ticket) {
        password.zeroize();
        return Ok(false);
    }
    // ponytail: one writer is enough before a live network; shard by RPC only if latency proves it.
    let _writer = autosave_writer().lock().await;
    if !autosave_is_current(&key, ticket) {
        password.zeroize();
        return Ok(false);
    }
    let result = async {
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::UpdateText {
                block_id,
                text: text.clone(),
                marks: None,
            }),
            password,
        )
        .await
    }
    .await;
    let current = finish_autosave(&key, ticket);
    if !current {
        return Ok(false);
    }
    result?;
    Ok(true)
}

fn autosaves() -> &'static std::sync::Mutex<BTreeMap<String, u64>> {
    static AUTOSAVES: OnceLock<std::sync::Mutex<BTreeMap<String, u64>>> = OnceLock::new();
    AUTOSAVES.get_or_init(Default::default)
}

fn autosave_writer() -> &'static tokio::sync::Mutex<()> {
    static WRITER: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    WRITER.get_or_init(Default::default)
}

pub fn cancel_autosaves(rpc: String, generation: i64) -> i64 {
    let prefix = format!("{}\0", rpc.trim());
    autosaves()
        .lock()
        .expect("autosave lock poisoned")
        .retain(|key, _| !key.starts_with(&prefix));
    generation.saturating_add(1)
}

fn begin_autosave(key: &str) -> u64 {
    static TICKET: AtomicU64 = AtomicU64::new(1);
    let ticket = TICKET.fetch_add(1, Ordering::Relaxed);
    autosaves()
        .lock()
        .expect("autosave lock poisoned")
        .insert(key.to_string(), ticket);
    ticket
}

fn autosave_is_current(key: &str, ticket: u64) -> bool {
    autosaves().lock().expect("autosave lock poisoned").get(key) == Some(&ticket)
}

fn finish_autosave(key: &str, ticket: u64) -> bool {
    let mut autosaves = autosaves().lock().expect("autosave lock poisoned");
    let is_current = autosaves.get(key) == Some(&ticket);
    if !is_current {
        return false;
    }
    autosaves.remove(key);
    true
}

fn block_move(
    blocks: &[pages::Block],
    block_id: &str,
    direction: &str,
) -> Result<(Option<String>, Option<String>), String> {
    let block = blocks
        .iter()
        .find(|block| block.id == block_id)
        .ok_or_else(|| "block was not found".to_string())?;
    let parent_id = block
        .parent
        .as_deref()
        .ok_or_else(|| "top-level pages cannot move inside their own document".to_string())?;
    let parent = blocks
        .iter()
        .find(|block| block.id == parent_id)
        .ok_or_else(|| "block parent was not found".to_string())?;
    let index = parent
        .children
        .iter()
        .position(|child| child == block_id)
        .ok_or_else(|| "block is missing from its parent".to_string())?;
    match direction {
        "up" if index > 0 => Ok((
            Some(parent.id.clone()),
            index
                .checked_sub(2)
                .map(|index| parent.children[index].clone()),
        )),
        "down" if index + 1 < parent.children.len() => Ok((
            Some(parent.id.clone()),
            Some(parent.children[index + 1].clone()),
        )),
        "indent" if index > 0 => {
            let new_parent = blocks
                .iter()
                .find(|block| block.id == parent.children[index - 1])
                .ok_or_else(|| "previous block was not found".to_string())?;
            Ok((
                Some(new_parent.id.clone()),
                new_parent.children.last().cloned(),
            ))
        }
        "outdent" => {
            let promotes_page = block.kind == BlockKind::Page && parent.parent.is_none();
            if promotes_page {
                return Ok((None, None));
            }
            let grandparent = parent
                .parent
                .clone()
                .ok_or_else(|| "block is already at the top level".to_string())?;
            Ok((Some(grandparent), Some(parent.id.clone())))
        }
        "up" => Err("block is already first".into()),
        "down" => Err("block is already last".into()),
        "indent" => Err("block needs a previous sibling to indent under".into()),
        _ => Err("choose a valid block move".into()),
    }
}

fn bounded_detail(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "no detail".into();
    }
    value.chars().take(300).collect()
}

fn next_sequence() -> u64 {
    static SEQUENCE: OnceLock<AtomicU64> = OnceLock::new();
    SEQUENCE
        .get_or_init(|| AtomicU64::new(epoch_nanos() as u64))
        .fetch_add(1, Ordering::Relaxed)
}

fn fresh_id(prefix: &str) -> String {
    format!("{prefix}-{}-{}", epoch_nanos(), next_sequence())
}

fn epoch_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    let valid = !value.is_empty()
        && value.len() <= MAX_FRAME_HEX_BYTES
        && value.len().is_multiple_of(2)
        && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !valid {
        return Err("ducktape signer returned an invalid frame".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("validated ASCII hex");
            u8::from_str_radix(pair, 16).map_err(|_| "ducktape signer returned invalid hex".into())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use commonware_cryptography::{Signer as _, ed25519};
    use iced::futures::StreamExt as _;

    use super::*;

    #[test]
    fn signer_requires_the_encrypted_v1_key_format() {
        assert_eq!(signing_input("secret", "00").unwrap(), b"secret\n00\n");
        assert!(signing_input("", "00").is_err());
        assert!(signing_input("bad\nsecret", "00").is_err());

        let directory = tempfile::tempdir().unwrap();
        let key = directory.path().join("user.key");
        std::fs::write(&key, format!("{ENCRYPTED_KEY_PREFIX}ciphertext")).unwrap();
        require_encrypted_key(&key).unwrap();
        std::fs::write(&key, "plaintext-key").unwrap();
        assert!(require_encrypted_key(&key).is_err());

        let public_key = "ab".repeat(32);
        assert_eq!(
            parse_user_key_status(&format!("encrypted {public_key}\n")),
            Some(vec![0xab; 32])
        );
        assert!(parse_user_key_status("absent\n").is_none());
        assert!(parse_user_key_status(&format!("plaintext {public_key}\n")).is_none());
    }


    #[test]
    fn post_commit_hydration_errors_are_not_retryable() {
        let error = committed_error("read failed".into());
        assert!(error.committed);
        assert_eq!(error.message, "read failed");
    }

    #[test]
    fn concurrent_optimistic_messages_settle_independently() {
        let pending = optimistic_message(
            optimistic_message(Vec::new(), "first".into(), "message-a".into()),
            "second".into(),
            "message-b".into(),
        );
        assert_eq!(
            pending
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            ["message-a", "message-b"]
        );

        let canonical = |id: &str, seq: i64, body: &str| ChatMessage {
            id: id.into(),
            seq,
            author: "You".into(),
            meta: format!("#{seq}"),
            body: body.into(),
            blocks: paragraph_blocks(body),
            pending: false,
            rev: 0,
            edited: false,
            deleted: false,
            reply_count: 0,
            thread_seq: 0,
            show_author: true,
            initial: "Y".into(),
            avatar_r: 0.4,
            avatar_g: 0.4,
            avatar_b: 0.9,
            reactions: Vec::new(),
        };
        let after_second = merge_message_send_result(
            vec![canonical("message-b", 1, "second")],
            pending,
            "general".into(),
            "general".into(),
            "message-b".into(),
        );
        assert_eq!(after_second.len(), 2);
        assert!(!after_second[0].pending);
        assert_eq!(after_second[1].id, "message-a");
        assert!(after_second[1].pending);

        let settled = merge_message_send_result(
            vec![
                canonical("message-b", 1, "second"),
                canonical("message-a", 2, "first"),
            ],
            after_second,
            "general".into(),
            "general".into(),
            "message-a".into(),
        );
        assert_eq!(
            settled
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            ["message-b", "message-a"]
        );
        assert!(settled.iter().all(|message| !message.pending));

        let after_stale_response = merge_message_send_result(
            vec![canonical("message-b", 1, "second")],
            settled,
            "general".into(),
            "general".into(),
            "message-b".into(),
        );
        assert_eq!(
            after_stale_response
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            ["message-b", "message-a"]
        );
    }

    #[test]
    fn message_groups_collapse_consecutive_authors() {
        let msg = |seq: i64, author: &str, deleted: bool| ChatMessage {
            id: format!("m{seq}"),
            seq,
            author: author.into(),
            meta: format!("#{seq}"),
            body: "body".into(),
            blocks: paragraph_blocks("body"),
            pending: false,
            rev: 0,
            edited: false,
            deleted,
            reply_count: 0,
            thread_seq: 0,
            show_author: false,
            initial: "A".into(),
            avatar_r: 0.4,
            avatar_g: 0.4,
            avatar_b: 0.9,
            reactions: Vec::new(),
        };
        let mut messages = vec![
            msg(1, "alice", false),
            msg(2, "alice", false),
            msg(3, "bob", false),
            msg(4, "bob", true),
            msg(5, "bob", false),
        ];
        mark_message_groups(&mut messages);
        let shown: Vec<bool> = messages.iter().map(|message| message.show_author).collect();
        // 1 opens the list; 2 shares alice -> continuation; 3 switches to bob -> header;
        // 4 is deleted -> header; 5 follows a deleted message -> header.
        assert_eq!(shown, vec![true, false, true, true, true]);
    }

    #[test]
    fn history_pagination_prepends_older_and_flags_more() {
        let msg = |seq: i64| ChatMessage {
            id: format!("m{seq}"),
            seq,
            author: "alice".into(),
            meta: format!("#{seq}"),
            body: "body".into(),
            blocks: paragraph_blocks("body"),
            pending: false,
            rev: 0,
            edited: false,
            deleted: false,
            reply_count: 0,
            thread_seq: 0,
            show_author: false,
            initial: "A".into(),
            avatar_r: 0.4,
            avatar_g: 0.4,
            avatar_b: 0.9,
            reactions: Vec::new(),
        };
        // oldest loaded root is seq 3 -> older history exists.
        let loaded = vec![msg(3), msg(4), msg(5)];
        assert!(history_has_older(loaded.clone()));
        assert_eq!(oldest_message_seq(loaded.clone()), 3);
        // prepend an older page whose last item (seq 3) duplicates the current head.
        let merged = prepend_history(loaded, vec![msg(1), msg(2), msg(3)]);
        assert_eq!(
            merged.iter().map(|message| message.seq).collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
        // now the oldest loaded root is seq 1 -> no more history to page.
        assert!(!history_has_older(merged));
    }

    #[test]
    fn thread_offsets_advance_only_for_loaded_commits() {
        assert_eq!(thread_offset_after_reply(3, false, true), 4);
        assert_eq!(thread_offset_after_reply(3, false, false), 3);
        assert_eq!(thread_offset_after_reply(256, true, true), 256);
        assert_eq!(thread_offset_after_reply(-1, false, true), -1);
    }

    #[test]
    fn block_comment_posts_reuse_the_selected_thread() {
        assert_eq!(comment_thread_id("thread-a".into()).unwrap(), "thread-a");
        assert!(
            comment_thread_id(String::new())
                .unwrap()
                .starts_with("thread-")
        );
        assert!(comment_thread_id(" ".into()).is_err());
    }

    #[test]
    fn concurrent_blocks_preserve_pending_position_then_accept_canonical_order() {
        let block = |id: &str, pending: bool| PageBlock {
            key: page_block_key(id),
            id: id.into(),
            parent: "page".into(),
            kind: "Text".into(),
            text: id.into(),
            pending,
            checked: false,
            prefix: String::new(),
            child_count: 0,
            mark_count: 0,
        };
        let current = vec![block("x", false), block("a", true), block("b", true)];
        let after_b = merge_block_insert_result(
            vec![block("x", false), block("b", false)],
            current,
            "page".into(),
            "page".into(),
            "b".into(),
        );
        assert_eq!(
            after_b
                .iter()
                .map(|block| block.id.as_str())
                .collect::<Vec<_>>(),
            ["x", "a", "b"]
        );
        assert!(after_b[1].pending);

        let settled = merge_block_insert_result(
            vec![block("x", false), block("b", false), block("a", false)],
            after_b,
            "page".into(),
            "page".into(),
            "a".into(),
        );
        assert_eq!(
            settled
                .iter()
                .map(|block| block.id.as_str())
                .collect::<Vec<_>>(),
            ["x", "b", "a"]
        );
        assert!(settled.iter().all(|block| !block.pending));

        let after_stale_response = merge_block_insert_result(
            vec![block("x", false), block("b", false)],
            settled,
            "page".into(),
            "page".into(),
            "b".into(),
        );
        assert_eq!(
            after_stale_response
                .iter()
                .map(|block| block.id.as_str())
                .collect::<Vec<_>>(),
            ["x", "b", "a"]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn chat_and_pages_round_trip_over_signed_frames() {
        let storage = tempfile::tempdir().unwrap();
        let sim = simnode::boot(
            storage.path(),
            "127.0.0.1:0".parse().unwrap(),
            simnode::SimOpts {
                auto: true,
                ..Default::default()
            },
        )
        .unwrap();
        let origin = format!("http://{}", sim.addr());
        let rpc = RpcClient::new(&origin).unwrap();
        let signer = ed25519::PrivateKey::from_seed(7);

        submit_test(
            &rpc,
            &signer,
            1,
            "chat",
            chat::encode_msg(&ChatMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
                post_policy: PostPolicy::Open,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            2,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "hello-1".into(),
                blocks: vec![chat::Block::paragraph("hello from the app")],
                thread: None,
                as_agent: None,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            3,
            "pages",
            pages::encode_msg(&PageMsg::CreatePage {
                page_id: "welcome".into(),
                title: "Welcome".into(),
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            4,
            "pages",
            pages::encode_msg(&PageMsg::InsertBlock {
                parent: "welcome".into(),
                after: None,
                block: NewBlock {
                    id: "intro".into(),
                    kind: BlockKind::Paragraph,
                    text: "A signed page block".into(),
                    marks: Vec::new(),
                },
            }),
        )
        .await;

        let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
        assert_eq!(chat.channels[0].name, "General");
        assert_eq!(chat.messages[0].body, "hello from the app");
        let pages = load_pages_data(&rpc, Some("welcome")).await.unwrap();
        assert_eq!(pages.active_page_title, "Welcome");
        assert_eq!(pages.blocks[0].text, "A signed page block");

        let origin = rpc.origin().to_string();
        let selected_page = load_page(origin.clone(), "welcome".into(), "intro".into())
            .await
            .unwrap();
        assert_eq!(selected_page.selected_block_id, "intro");
        assert_eq!(selected_page.selected_block_text, "A signed page block");
        let selected_title = load_page(origin.clone(), "welcome".into(), "welcome".into())
            .await
            .unwrap();
        assert!(selected_title.page_title_selected);
        assert!(selected_title.selected_block_id.is_empty());
        let workspace = connect(origin.clone()).await.unwrap();
        let mut live = live_events(origin.clone());
        let ready = live.next().await.unwrap();
        assert_eq!(ready.kind, "ready");
        submit_test(
            &rpc,
            &signer,
            5,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "hello-2".into(),
                blocks: vec![chat::Block::paragraph("arrived on the next block")],
                thread: None,
                as_agent: None,
            }),
        )
        .await;
        let changed = live.next().await.unwrap();
        assert_eq!(changed.kind, "chat", "a chat op folds into a chat delta");
        assert_eq!(changed.chat.kind, "posted");
        assert_eq!(changed.chat.channel_id, "general");
        assert_eq!(
            changed.chat.seq, 2,
            "the delta carries the module-assigned sequence from the feed stamp"
        );
        assert_eq!(changed.chat.message.body, "arrived on the next block");
        assert!(
            !changed.load_chat && !changed.load_pages,
            "a folded chat delta requires no reload"
        );
        assert!(changed.height > workspace.height);
        let base_height = changed.height;
        submit_test(
            &rpc,
            &signer,
            6,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "reply-1".into(),
                blocks: vec![chat::Block::paragraph("a threaded reply")],
                thread: Some(1),
                as_agent: None,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            7,
            "chat",
            chat::encode_msg(&ChatMsg::EditMessage {
                channel_id: "general".into(),
                seq: 1,
                blocks: vec![chat::Block::paragraph("hello, edited")],
                base_rev: Some(0),
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            8,
            "chat",
            chat::encode_msg(&ChatMsg::AddReaction {
                channel_id: "general".into(),
                seq: 1,
                emoji: "👍".into(),
            }),
        )
        .await;

        wait_for_block(&mut live, base_height + 3).await;
        let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
        assert_eq!(chat.active_channel_name, "General");
        assert_eq!(chat.messages[0].body, "hello, edited");
        assert!(chat.messages[0].edited);
        assert_eq!(chat.messages[0].reply_count, 1);
        assert_eq!(chat.messages[0].reactions[0].emoji, "👍");
        let thread = load_thread_data(&rpc, "general", 1, 0).await.unwrap();
        assert_eq!(thread.messages.len(), 2);
        assert_eq!(thread.messages[1].body, "a threaded reply");
        let hit = load_chat_hit(origin.clone(), "general".into(), 1, 3)
            .await
            .unwrap();
        assert_eq!(hit.selected_message_seq, 1);
        assert_eq!(hit.active_thread_seq, 1);
        assert_eq!(hit.thread_target_seq, 3);
        assert_eq!(hit.thread_messages[1].body, "a threaded reply");
        submit_test(
            &rpc,
            &signer,
            9,
            "pages",
            pages::encode_msg(&PageMsg::InsertBlock {
                parent: "welcome".into(),
                after: Some("intro".into()),
                block: NewBlock {
                    id: "heading".into(),
                    kind: BlockKind::Heading2,
                    text: "Nested work".into(),
                    marks: Vec::new(),
                },
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            10,
            "pages",
            pages::encode_msg(&PageMsg::InsertBlock {
                parent: "heading".into(),
                after: None,
                block: NewBlock {
                    id: "todo".into(),
                    kind: BlockKind::Todo,
                    text: "Ship the editor".into(),
                    marks: Vec::new(),
                },
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            11,
            "pages",
            pages::encode_msg(&PageMsg::SetChecked {
                block_id: "todo".into(),
                checked: true,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            12,
            "pages",
            pages::encode_msg(&PageMsg::InsertBlock {
                parent: "welcome".into(),
                after: Some("heading".into()),
                block: NewBlock {
                    id: "child".into(),
                    kind: BlockKind::Page,
                    text: "Child page".into(),
                    marks: Vec::new(),
                },
            }),
        )
        .await;

        wait_for_block(&mut live, base_height + 7).await;
        let pages = load_pages_data(&rpc, Some("welcome")).await.unwrap();
        assert_eq!(pages.pages[0].id, "welcome");
        assert_eq!(pages.pages[1].id, "child");
        assert_eq!(pages.pages[1].prefix, "  ");
        assert_eq!(pages.blocks[2].id, "todo");
        assert_eq!(pages.blocks[2].prefix, "  ");
        assert!(pages.blocks[2].checked);

        submit_test(
            &rpc,
            &signer,
            13,
            "pages",
            pages::encode_msg(&PageMsg::AddComment {
                thread_id: "thread-live".into(),
                comment_id: "comment-live".into(),
                target: "intro".into(),
                text: "temporary".into(),
                anchor: None,
                mentions: Vec::new(),
                as_agent: None,
            }),
        )
        .await;
        wait_for_block(&mut live, base_height + 8).await;
        let comments =
            refresh_block_comments(origin.clone(), "intro".into(), "thread-live".into(), 1)
                .await
                .unwrap();
        assert_eq!(comments.thread_id, "thread-live");
        submit_test(
            &rpc,
            &signer,
            14,
            "pages",
            pages::encode_msg(&PageMsg::DeleteComment {
                comment_id: "comment-live".into(),
            }),
        )
        .await;
        wait_for_block(&mut live, base_height + 9).await;
        let comments =
            refresh_block_comments(origin.clone(), "intro".into(), "thread-live".into(), 2)
                .await
                .unwrap();
        assert!(comments.thread_id.is_empty());
        assert!(comments.comments.is_empty());

        let refreshed = live_resync_load(
            origin,
            "general".into(),
            "welcome".into(),
            "both".into(),
            false,
            7,
            0,
        )
        .await
        .unwrap();
        assert_eq!(refreshed.generation, 7);
        assert!(refreshed.chat_loaded && refreshed.pages_loaded);
        assert_eq!(refreshed.messages[1].body, "arrived on the next block");
        assert_eq!(refreshed.active_page, "welcome");
        sim.shutdown();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn timeline_pages_past_thread_only_traffic() {
        let storage = tempfile::tempdir().unwrap();
        let sim = simnode::boot(
            storage.path(),
            "127.0.0.1:0".parse().unwrap(),
            simnode::SimOpts {
                auto: true,
                ..Default::default()
            },
        )
        .unwrap();
        let origin = format!("http://{}", sim.addr());
        let rpc = RpcClient::new(&origin).unwrap();
        let signer = ed25519::PrivateKey::from_seed(8);

        submit_test(
            &rpc,
            &signer,
            1,
            "chat",
            chat::encode_msg(&ChatMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
                post_policy: PostPolicy::Open,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            2,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "root".into(),
                blocks: vec![chat::Block::paragraph("root stays visible")],
                thread: None,
                as_agent: None,
            }),
        )
        .await;
        for index in 0_u64..257 {
            submit_test(
                &rpc,
                &signer,
                index + 3,
                "chat",
                chat::encode_msg(&ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: format!("reply-{index}"),
                    blocks: vec![chat::Block::paragraph(format!("reply {index}"))],
                    thread: Some(1),
                    as_agent: None,
                }),
            )
            .await;
        }

        let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].body, "root stays visible");
        let first = load_thread_data(&rpc, "general", 1, 0).await.unwrap();
        assert_eq!(first.messages.len(), 257);
        assert_eq!(first.next_reply_offset, 256);
        assert!(first.has_more);
        let last = load_thread_page(origin.clone(), "general".into(), 1, 256, 9)
            .await
            .unwrap();
        assert_eq!(last.messages.len(), 1);
        assert_eq!(last.messages[0].body, "reply 256");
        assert_eq!(last.next_reply_offset, 257);
        assert!(!last.has_more);
        let sparse = load_thread(origin, "general".into(), 1, 258, -1, 10)
            .await
            .unwrap();
        assert_eq!(sparse.target_seq, 258);
        assert_eq!(sparse.next_reply_offset, -1);
        assert_eq!(sparse.messages.len(), 2);
        assert_eq!(sparse.messages[1].body, "reply 256");
        sim.shutdown();
    }

    #[test]
    fn hydration_retry_is_capped() {
        assert_eq!(retry_delay(1), Duration::from_secs(1));
        assert_eq!(retry_delay(3), Duration::from_secs(4));
        assert_eq!(retry_delay(99), Duration::from_secs(16));
    }

    #[test]
    fn page_block_keys_survive_refresh_reordering() {
        let root = pages::Block {
            id: "page".into(),
            parent: None,
            page: "page".into(),
            kind: BlockKind::Page,
            text: "Page".into(),
            marks: Vec::new(),
            checked: false,
            children: Vec::new(),
        };
        let block = |id: &str| pages::Block {
            id: id.into(),
            parent: Some("page".into()),
            page: "page".into(),
            kind: BlockKind::Paragraph,
            text: id.into(),
            marks: Vec::new(),
            checked: false,
            children: Vec::new(),
        };
        let before = page_blocks(
            vec![root.clone(), block("editing"), block("trailing")],
            "page",
        );
        let editing_key = before
            .iter()
            .find(|block| block.id == "editing")
            .unwrap()
            .key;
        let nested = pages::Block {
            id: "nested".into(),
            parent: Some("inserted".into()),
            page: "page".into(),
            kind: BlockKind::Paragraph,
            text: "nested".into(),
            marks: Vec::new(),
            checked: false,
            children: Vec::new(),
        };
        let after = page_blocks(
            vec![
                root,
                block("inserted"),
                nested,
                block("trailing"),
                block("editing"),
            ],
            "page",
        );

        assert_eq!(
            after
                .iter()
                .find(|block| block.id == "editing")
                .unwrap()
                .key,
            editing_key
        );
        assert_eq!(
            after
                .iter()
                .map(|block| block.key)
                .collect::<BTreeSet<_>>()
                .len(),
            after.len()
        );

        let optimistic = optimistic_block(
            after,
            "inserted".into(),
            "Text".into(),
            "pending".into(),
            "block-pending".into(),
        );
        let pending = &optimistic[2];
        assert_eq!(pending.id, "block-pending");
        assert_eq!(pending.key, page_block_key(&pending.id));
        assert_eq!(pending.parent, "page");
        assert_eq!(optimistic[1].id, "nested");
        assert_eq!(optimistic[3].id, "trailing");
        assert_eq!(
            optimistic
                .iter()
                .map(|block| block.key)
                .collect::<BTreeSet<_>>()
                .len(),
            optimistic.len()
        );
    }

    #[test]
    fn block_action_menu_stays_inside_the_page_viewport() {
        assert_eq!(block_action_menu_y(100.0, 500.0), 96.0);
        assert_eq!(block_action_menu_y(450.0, 500.0), 260.0);
        assert_eq!(block_action_menu_y(2.0, 500.0), 0.0);
    }

    #[test]
    fn refresh_merges_only_clean_block_drafts() {
        let blocks = vec![PageBlock {
            key: 0,
            id: "block".into(),
            parent: "page".into(),
            kind: "Text".into(),
            text: "remote".into(),
            pending: false,
            checked: false,
            prefix: String::new(),
            child_count: 0,
            mark_count: 0,
        }];

        assert_eq!(
            refreshed_block_draft(
                blocks.clone(),
                "block".into(),
                "local".into(),
                "saving".into(),
            ),
            "local"
        );
        assert_eq!(
            refreshed_block_draft(blocks, "block".into(), "local".into(), "saved".into()),
            "remote"
        );
    }

    #[test]
    fn recovered_drafts_are_deduplicated_and_endpoint_scoped() {
        let drafts = remember_orphaned_block_drafts(
            vec!["local".into()],
            Vec::new(),
            "missing".into(),
            "local".into(),
            "error".into(),
        );
        assert_eq!(drafts, ["local"]);
        assert_eq!(
            retain_drafts_for_endpoint(drafts.clone(), "http://node".into(), "http://node".into(),),
            drafts
        );
        assert!(
            retain_drafts_for_endpoint(drafts, "http://node".into(), "http://other".into(),)
                .is_empty()
        );
    }

    #[test]
    fn page_updates_preserve_exact_text() {
        assert_eq!(
            bounded_updated_block_text(BlockKind::Code, "  code\n".into()).unwrap(),
            "  code\n"
        );
        assert_eq!(
            bounded_updated_block_text(BlockKind::Paragraph, String::new()).unwrap(),
            ""
        );
        assert_eq!(
            bounded_exact_text(String::new(), "page title", 512).unwrap(),
            ""
        );
    }

    #[test]
    fn autosave_keeps_only_the_latest_ticket() {
        let key = "autosave-test";
        let first = begin_autosave(key);
        let latest = begin_autosave(key);
        assert!(!autosave_is_current(key, first));
        assert!(autosave_is_current(key, latest));
        assert!(!finish_autosave(key, first));
        assert!(finish_autosave(key, latest));
        assert!(!autosave_is_current(key, latest));
    }

    #[test]
    fn reconnect_cancels_only_the_previous_endpoint_autosaves() {
        let old_key = "http://old\0page";
        let other_key = "http://other\0page";
        let old_ticket = begin_autosave(old_key);
        let other_ticket = begin_autosave(other_key);

        assert_eq!(cancel_autosaves("http://old".into(), 4), 5);
        assert!(!autosave_is_current(old_key, old_ticket));
        assert!(finish_autosave(other_key, other_ticket));
    }

    #[test]
    fn missing_block_cancels_only_its_own_autosave() {
        let title_key = "http://missing-block-test\0page";
        let block_key = "http://missing-block-test\0block";
        let title_ticket = begin_autosave(title_key);
        let block_ticket = begin_autosave(block_key);

        assert_eq!(
            cancel_missing_block_autosave(
                "http://missing-block-test".into(),
                4,
                Vec::new(),
                "block".into(),
            ),
            5
        );
        assert!(!autosave_is_current(block_key, block_ticket));
        assert!(finish_autosave(title_key, title_ticket));
    }

    #[test]
    fn block_moves_follow_visible_sibling_order() {
        let block = |id: &str, parent: Option<&str>, kind, children: &[&str]| pages::Block {
            id: id.into(),
            parent: parent.map(str::to_string),
            page: "page".into(),
            kind,
            text: id.into(),
            marks: Vec::new(),
            checked: false,
            children: children.iter().map(|child| (*child).into()).collect(),
        };
        let blocks = vec![
            block("page", None, BlockKind::Page, &["a", "b"]),
            block("a", Some("page"), BlockKind::Paragraph, &["c"]),
            block("c", Some("a"), BlockKind::Paragraph, &[]),
            block("b", Some("page"), BlockKind::Paragraph, &[]),
        ];

        assert_eq!(
            block_move(&blocks, "b", "up").unwrap(),
            (Some("page".into()), None)
        );
        assert_eq!(
            block_move(&blocks, "a", "down").unwrap(),
            (Some("page".into()), Some("b".into()))
        );
        assert_eq!(
            block_move(&blocks, "b", "indent").unwrap(),
            (Some("a".into()), Some("c".into()))
        );
        assert_eq!(
            block_move(&blocks, "c", "outdent").unwrap(),
            (Some("page".into()), Some("a".into()))
        );

        let page = block("child-page", Some("page"), BlockKind::Page, &[]);
        let parent = block("page", None, BlockKind::Page, &["child-page"]);
        assert_eq!(
            block_move(&[parent, page], "child-page", "outdent").unwrap(),
            (None, None)
        );
    }

    #[test]
    fn client_local_unread_tracking_seeds_marks_and_places_the_divider() {
        let channel = |id: &str, head: i64| ChatChannel {
            id: id.into(),
            name: id.into(),
            archived: false,
            members_only: false,
            huddle_count: 0,
            head_seq: head,
        };
        let read = |channel: &str, seq: i64| ChannelRead {
            channel: channel.into(),
            seq,
        };
        let message = |seq: i64, pending: bool| ChatMessage {
            id: format!("m{seq}"),
            seq: if pending { -1 } else { seq },
            author: "u".into(),
            meta: String::new(),
            body: String::new(),
            blocks: Vec::new(),
            pending,
            rev: 0,
            edited: false,
            deleted: false,
            reply_count: 0,
            thread_seq: 0,
            show_author: true,
            initial: "U".into(),
            avatar_r: 0.0,
            avatar_g: 0.0,
            avatar_b: 0.0,
            reactions: Vec::new(),
        };

        let reads = vec![read("general", 100), read("random", 30)];
        let channels = vec![channel("general", 100), channel("random", 50)];

        // channel_last_read / channel_head_seq: lookup, 0 when absent.
        assert_eq!(channel_last_read(reads.clone(), "random".into()), 30);
        assert_eq!(channel_last_read(reads.clone(), "missing".into()), 0);
        assert_eq!(channel_head_seq(channels.clone(), "random".into()), 50);
        assert_eq!(channel_head_seq(channels.clone(), "missing".into()), 0);

        // mark_channel_read upserts to the max, adds absent, ignores empty id.
        let marked = mark_channel_read(reads.clone(), "random".into(), 50);
        assert_eq!(channel_last_read(marked.clone(), "random".into()), 50);
        let lowered = mark_channel_read(marked, "random".into(), 40);
        assert_eq!(channel_last_read(lowered, "random".into()), 50);
        let added = mark_channel_read(reads.clone(), "new".into(), 7);
        assert_eq!(channel_last_read(added, "new".into()), 7);
        assert_eq!(
            mark_channel_read(reads.clone(), String::new(), 9).len(),
            reads.len()
        );

        // channel_is_unread: head past the seen cursor.
        assert!(channel_is_unread(reads.clone(), "random".into(), 50));
        assert!(!channel_is_unread(reads.clone(), "random".into(), 30));
        assert!(!channel_is_unread(reads.clone(), "general".into(), 100));

        // initial_channel_reads: seed absent channels to head, preserve existing.
        let seeded = initial_channel_reads(channels.clone(), vec![read("random", 30)]);
        assert_eq!(channel_last_read(seeded.clone(), "random".into()), 30);
        assert_eq!(channel_last_read(seeded.clone(), "general".into()), 100);
        assert!(!channel_is_unread(seeded, "general".into(), 100));

        // first_unread_seq: first message past the boundary; pending (seq -1)
        // never anchors it; 0 when caught up.
        let messages = vec![
            message(31, false),
            message(40, false),
            message(50, false),
            message(0, true),
        ];
        assert_eq!(first_unread_seq(messages.clone(), 30), 31);
        assert_eq!(first_unread_seq(messages.clone(), 45), 50);
        assert_eq!(first_unread_seq(messages.clone(), 50), 0);
        assert_eq!(first_unread_seq(messages, 0), 0);

        // frozen_unread_boundary: same channel is left untouched; a change
        // re-freezes at the arrived channel's last-read, or 0 when caught up.
        assert_eq!(
            frozen_unread_boundary(
                reads.clone(),
                channels.clone(),
                "random".into(),
                "random".into(),
                30
            ),
            30
        );
        assert_eq!(
            frozen_unread_boundary(
                reads.clone(),
                channels.clone(),
                "general".into(),
                "random".into(),
                999
            ),
            30
        );
        let caught_up = vec![read("general", 100), read("random", 50)];
        assert_eq!(
            frozen_unread_boundary(caught_up, channels, "general".into(), "random".into(), 999),
            0
        );
    }

    /// drain the live event stream until the index has folded the block at
    /// `min_height` — the system's own commit signal, never a timed poll.
    async fn wait_for_block(
        live: &mut iced::futures::stream::BoxStream<'static, LiveUpdate>,
        min_height: i64,
    ) {
        loop {
            let update = live.next().await.expect("live stream ended");
            let folded = update.kind == "chat" || update.kind == "pages";
            if folded && update.height >= min_height {
                return;
            }
        }
    }

    async fn submit_test(
        rpc: &RpcClient,
        signer: &ed25519::PrivateKey,
        sequence: u64,
        target: &str,
        payload: Vec<u8>,
    ) {
        let frame = node::encode_frame(
            signer,
            sequence,
            &sdk::Msg {
                target: target.into(),
                payload,
            },
            None,
        );
        rpc.submit_frame(frame).await.unwrap();
    }
}
