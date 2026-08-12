use super::*;

pub fn fresh_operation_id(prefix: String) -> String {
    fresh_id(&prefix)
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

/// FOLD A LOAD'S ROWS INTO THE LIST ON SCREEN — do not replace it with them.
///
/// The switch loader is handed the list the reader is already looking at and
/// answers with the one row it refreshed (`load_channel_window_data`), so
/// assigning its list back would revert every delta the live stream folded
/// during the round trip: a peer's post in a THIRD room and the unread badge it
/// lit, a channel created, renamed or archived. Nothing re-pages the list
/// afterwards — `load_chat` is raised only by a reconnect — so that loss is
/// permanent, not a frame of staleness.
///
/// `head_seq` only moves FORWARD. The row was read mid-flight; a delta folded
/// after that read is the newer fact, and letting the row walk it back relights
/// a badge the reader has already cleared.
pub fn upsert_channel_rows(
    mut channels: Vec<ChatChannel>,
    refreshed: Vec<ChatChannel>,
) -> Vec<ChatChannel> {
    for mut row in refreshed {
        let Some(current) = channels.iter_mut().find(|current| current.id == row.id) else {
            channels.push(row);
            continue;
        };
        row.head_seq = row.head_seq.max(current.head_seq);
        *current = row;
    }
    channels
}

// active-channel scalars re-derived from the (delta-folded) channel list,
// keeping the current value when the channel is absent from the list.

pub fn channel_display_name(
    channels: Vec<ChatChannel>,
    channel: String,
    current: String,
) -> String {
    channels
        .iter()
        .find(|row| row.id == channel)
        .map_or(current, |row| row.name.clone())
}

/// Is the clicked channel archived / members-only, per the list the sidebar is
/// already drawn from? Both ride the click, not the round trip: `post_refusal`
/// is recomputed the moment the room changes, and reading the room she LEFT for
/// one round trip is how a public channel came up refusing her post.
pub fn channel_is_archived(channels: Vec<ChatChannel>, channel: String) -> bool {
    channels.iter().any(|row| row.id == channel && row.archived)
}

pub fn channel_is_members_only(channels: Vec<ChatChannel>, channel: String) -> bool {
    channels
        .iter()
        .any(|row| row.id == channel && row.members_only)
}

/// Is the reader inside the last tenth of the loaded scrollback?
///
/// The stream is bottom-anchored, so a scrollable reports its offset relative
/// to the END — 1.0 is the TOP of the history in hand, which is where the next
/// older page belongs.
///
/// A NaN offset (content that fits reports `0/0`) compares false against
/// everything, which is the answer this wants anyway — iced does not publish a
/// viewport at all in that case, so it is a belt, not the braces.
pub fn near_scroll_top(relative_offset: f64) -> bool {
    relative_offset >= 0.9
}

/// The last rows and member roll seen in one channel, kept so a switch back
/// paints in one frame instead of on the network.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChannelWindow {
    pub channel_id: String,
    pub messages: Vec<ChatMessage>,
    pub members: Vec<ChatMember>,
}

/// How many rooms the cache remembers. Alternating between two rooms is the
/// motion this pays for; a third covers "back out through the room you came
/// from". Past that it is memory held for windows the refetch would replace
/// anyway.
const CHANNEL_WINDOW_CACHE: usize = 3;

/// Park the room being left, most-recent first.
///
/// PENDING ROWS DO NOT GO IN. An in-flight send settles against `messages` for
/// the room the reader is IN — `message_sent`/`message_send_failed` both drop
/// their timeline surgery once she has moved — so a parked pending row has no
/// writer left to retire it and would come back as a permanent "Sending…".
///
/// A HISTORY WINDOW DOES NOT GO IN EITHER. Those rows are a page around one old
/// message, not the tail; restoring them under a cleared `history_view` would
/// paint months-old scrollback as the live conversation.
pub fn cache_channel_window(
    cache: Vec<ChannelWindow>,
    channel_id: String,
    messages: Vec<ChatMessage>,
    members: Vec<ChatMember>,
    history_view: bool,
) -> Vec<ChannelWindow> {
    let committed: Vec<ChatMessage> = messages
        .into_iter()
        .filter(|message| !message.pending)
        .collect();
    let worth_keeping = !history_view && !channel_id.is_empty() && !committed.is_empty();
    if !worth_keeping {
        return cache;
    }
    let mut kept: Vec<ChannelWindow> = vec![ChannelWindow {
        channel_id: channel_id.clone(),
        messages: committed,
        members,
    }];
    kept.extend(
        cache
            .into_iter()
            .filter(|window| window.channel_id != channel_id)
            .take(CHANNEL_WINDOW_CACHE - 1),
    );
    kept
}

pub fn cached_window_messages(cache: Vec<ChannelWindow>, channel_id: String) -> Vec<ChatMessage> {
    cache
        .into_iter()
        .find(|window| window.channel_id == channel_id)
        .map(|window| window.messages)
        .unwrap_or_default()
}

pub fn cached_window_members(cache: Vec<ChannelWindow>, channel_id: String) -> Vec<ChatMember> {
    cache
        .into_iter()
        .find(|window| window.channel_id == channel_id)
        .map(|window| window.members)
        .unwrap_or_default()
}

/// The clicked page's title, from the index the sidebar is already drawn from
/// — the header has to move with the click, not with the round trip. Falls
/// back to the current title while the id is not in the list yet.
pub fn page_display_title(pages: Vec<PageItem>, page: String, current: String) -> String {
    pages
        .iter()
        .find(|row| row.id == page)
        .map_or(current, |row| row.title.clone())
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

pub fn channel_live_huddle_count(channels: Vec<ChatChannel>, channel: String, current: i64) -> i64 {
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
fn channel_is_unread(reads: &[ChannelRead], channel: &str, head_seq: i64) -> bool {
    head_seq > last_read_of(reads, channel)
}

/// EVERY unread channel's id, in one pass — the sidebar's badge, mirrored.
///
/// The by-value extern ABI is why this is a batch: called per row the way
/// `channel_is_unread` was, it deep-cloned the whole read-cursor list once per
/// channel, which is `O(channels × reads)` String allocations on EVERY frame
/// (~900 at thirty rooms, and it grows quadratically with the workspace). The
/// answer moves only where `channel_reads` or `channels` move, so it is
/// computed there and read from `unread_channel_ids` state.
pub fn unread_channels(reads: Vec<ChannelRead>, channels: Vec<ChatChannel>) -> Vec<String> {
    channels
        .into_iter()
        .filter(|channel| channel_is_unread(&reads, &channel.id, channel.head_seq))
        .map(|channel| channel.id)
        .collect()
}

/// The sidebar row's own reading of that mirror. Still one clone per row, but
/// of a list holding only the channels that ARE unread — usually empty, never
/// wider than the room list, and carrying one String each instead of a cursor.
pub fn is_unread_channel(unread: Vec<String>, channel: String) -> bool {
    unread.contains(&channel)
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

/// The live-resync twin of [`remember_orphaned_comment_drafts`]: the rail's
/// anchor is the PAGE, so the half-typed comment is orphaned only when that
/// page itself vanished from the index — never merely because a resync ran.
pub fn remember_orphaned_page_comment(
    mut drafts: Vec<String>,
    pages: Vec<PageItem>,
    target: String,
    draft: String,
) -> Vec<String> {
    let page_gone = !target.is_empty() && !pages.iter().any(|page| page.id == target);
    if page_gone {
        append_recovered_draft(&mut drafts, draft);
    }
    drafts
}

pub fn remove_recovered_draft(mut drafts: Vec<String>, recovered: String) -> Vec<String> {
    if let Some(index) = drafts.iter().position(|draft| draft == &recovered) {
        drafts.remove(index);
    }
    drafts
}

/// The commented BLOCK ids in a thread list — the page's own id marks no line.
pub fn commented_targets_of(threads: Vec<PageCommentThread>, page_id: String) -> Vec<String> {
    let mut targets: Vec<String> = threads
        .into_iter()
        .filter(|thread| !thread.resolved && thread.target != page_id)
        .map(|thread| thread.target)
        .collect();
    // NOT deduplicated — see `load::commented_targets`. The repetition is the
    // per-line thread count the margin chip spells.
    targets.sort();
    targets
}

/// The open thread's resolved flag, read off the rail's own list.
pub fn thread_is_resolved(threads: Vec<PageCommentThread>, id: String) -> bool {
    threads
        .iter()
        .find(|thread| thread.id == id)
        .is_some_and(|thread| thread.resolved)
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

pub(crate) struct Tip {
    pub(crate) height: i64,
    pub(crate) status: String,
}

pub(crate) fn rpc_client(input: &str) -> Result<RpcClient, String> {
    let configured = if input.trim().is_empty() {
        std::env::var("DUCKTAPE_NODE")
            .ok()
            .or_else(registered_endpoint)
            .unwrap_or_else(|| DEFAULT_RPC.to_string())
    } else {
        input.trim().to_string()
    };
    // One client per origin for the process's life: the reqwest pool and TLS
    // setup survive across externs instead of being rebuilt by every `run`
    // (one hydrate fans out 13 of them in a single parallel).
    // ponytail: never evicts — the map holds one entry per endpoint the user
    // has ever pointed this session at, which is their handful of networks.
    static CLIENTS: std::sync::Mutex<std::collections::BTreeMap<String, RpcClient>> =
        std::sync::Mutex::new(std::collections::BTreeMap::new());
    let mut clients = CLIENTS.lock().expect("rpc client cache");
    if let Some(client) = clients.get(&configured) {
        return Ok(client.clone());
    }
    let client = RpcClient::new(&configured).map_err(String::from)?;
    clients.insert(configured, client.clone());
    Ok(client)
}
