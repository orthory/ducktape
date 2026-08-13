use super::*;

/// A selected loader call. Ice task-flow transforms may read only their input,
/// so the optional carries every argument the chosen effect needs.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct LoadRequest {
    pub rpc: String,
    pub key: String,
    pub generation: i64,
}

/// Select a loader without launching an offscreen refusal. `try` turns
/// `None` into `Task::none`, leaving any unrelated in-flight lane untouched.
pub fn load_request(
    condition: bool,
    rpc: String,
    key: String,
    generation: i64,
) -> Option<LoadRequest> {
    condition.then_some(LoadRequest {
        rpc,
        key,
        generation,
    })
}

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

pub fn mutation_failure_phase(committed: bool) -> crate::MutationPhase {
    if committed {
        crate::MutationPhase::Recovering
    } else {
        crate::MutationPhase::Idle
    }
}

pub fn mutation_phase_after_recovery(current: crate::MutationPhase) -> crate::MutationPhase {
    if current == crate::MutationPhase::Recovering {
        crate::MutationPhase::Idle
    } else {
        current
    }
}

fn committed_message_change(phase: crate::MutationPhase, committed: bool) -> bool {
    if !committed {
        return false;
    }
    match phase {
        crate::MutationPhase::MessageDelete | crate::MutationPhase::MessageEdit => true,
        crate::MutationPhase::Idle
        | crate::MutationPhase::Recovering
        | crate::MutationPhase::BlockComment
        | crate::MutationPhase::Channel
        | crate::MutationPhase::ChannelArchive
        | crate::MutationPhase::ChannelMember
        | crate::MutationPhase::ChannelRename
        | crate::MutationPhase::ChannelUnarchive
        | crate::MutationPhase::CommentResolve
        | crate::MutationPhase::ForgetWorkspace
        | crate::MutationPhase::Huddle
        | crate::MutationPhase::Onboarding
        | crate::MutationPhase::Page
        | crate::MutationPhase::PageDelete => false,
    }
}

pub fn message_seq_after_failure(
    current: i64,
    phase: crate::MutationPhase,
    committed: bool,
) -> i64 {
    if committed_message_change(phase, committed) {
        0
    } else {
        current
    }
}

pub fn message_text_after_failure(
    current: String,
    phase: crate::MutationPhase,
    committed: bool,
) -> String {
    if committed_message_change(phase, committed) {
        String::new()
    } else {
        current
    }
}

pub fn message_action_after_failure(
    current: crate::MessageAction,
    phase: crate::MutationPhase,
    committed: bool,
) -> crate::MessageAction {
    if committed_message_change(phase, committed) {
        crate::MessageAction::Toolbar
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

#[derive(Clone, Debug, PartialEq)]
pub struct MessageSelection {
    pub seq: i64,
    pub rev: i64,
    pub action: crate::MessageAction,
    pub draft: String,
}

pub(crate) fn message_selection_after_window_ref(
    messages: &[ChatMessage],
    seq: i64,
    rev: i64,
    action: crate::MessageAction,
    draft: String,
) -> MessageSelection {
    let visible = seq > 0
        && messages
            .iter()
            .any(|message| message.seq == seq && !message.deleted);
    if visible {
        MessageSelection {
            seq,
            rev,
            action,
            draft,
        }
    } else {
        MessageSelection {
            seq: 0,
            rev: 0,
            action: crate::MessageAction::Toolbar,
            draft: String::new(),
        }
    }
}

pub fn message_selection_after_window(
    messages: Vec<ChatMessage>,
    seq: i64,
    rev: i64,
    action: crate::MessageAction,
    draft: String,
) -> MessageSelection {
    message_selection_after_window_ref(&messages, seq, rev, action, draft)
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

/// Everything a room click projects from the channel list, computed in one
/// ownership crossing. The old shape cloned and scanned the whole workspace
/// four times before the load task could even start.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct ChannelSwitchFacts {
    pub unread_boundary: i64,
    pub name: String,
    pub archived: bool,
    pub members_only: bool,
}

pub fn channel_switch_facts(
    reads: Vec<ChannelRead>,
    channels: Vec<ChatChannel>,
    current_channel: String,
    next_channel: String,
    current_boundary: i64,
    current_name: String,
) -> ChannelSwitchFacts {
    let row = channels.iter().find(|row| row.id == next_channel);
    let head_seq = row.map_or(0, |row| row.head_seq);
    let unread_boundary = if current_channel == next_channel {
        current_boundary
    } else {
        let last_read = last_read_of(&reads, &next_channel);
        if head_seq > last_read { last_read } else { 0 }
    };
    ChannelSwitchFacts {
        unread_boundary,
        name: row.map_or(current_name, |row| row.name.clone()),
        archived: row.is_some_and(|row| row.archived),
        members_only: row.is_some_and(|row| row.members_only),
    }
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

/// One room's unsent composer text, parked while the reader is somewhere else.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct ChannelDraft {
    pub channel_id: String,
    pub text: String,
}

/// Park the composer of the room being left, and drop the entry when there is
/// nothing to park.
///
/// Drafts are tiny and self-clearing: empty text removes the entry, a send
/// empties the composer, and the next park removes it. This list therefore holds
/// one short string per room with unsent words and needs no cap.
pub fn park_message_draft(
    drafts: Vec<ChannelDraft>,
    channel_id: String,
    text: String,
) -> Vec<ChannelDraft> {
    let mut kept: Vec<ChannelDraft> = drafts
        .into_iter()
        .filter(|draft| draft.channel_id != channel_id)
        .collect();
    let worth_keeping = !channel_id.is_empty() && !text.is_empty();
    if worth_keeping {
        kept.push(ChannelDraft { channel_id, text });
    }
    kept
}

/// The parked composer text for one room, or nothing.
pub fn parked_message_draft(drafts: Vec<ChannelDraft>, channel_id: String) -> String {
    drafts
        .into_iter()
        .find(|draft| draft.channel_id == channel_id)
        .map(|draft| draft.text)
        .unwrap_or_default()
}

/// A thread's park key: the rail belongs to a room AND a root, and the same seq
/// under two rooms is two different threads.
fn thread_draft_key(channel_id: &str, thread_seq: i64) -> String {
    format!("{channel_id}#{thread_seq}")
}

/// Park the reply composer of the thread being left, in its OWN store.
///
/// NOT a harvest into `failed_reply_draft`: that field is channel-scoped, so a
/// reply typed in thread A would raise its "Unsent reply" plate over every later
/// thread of the room and Restore would arm A's words to post in B. Parked text
/// is invisible until she opens the thread it belongs to, which is the only
/// place it can be posted.
pub fn park_reply_draft(
    drafts: Vec<ChannelDraft>,
    channel_id: String,
    thread_seq: i64,
    text: String,
) -> Vec<ChannelDraft> {
    // NO OPEN RAIL, NOTHING TO PARK. The composite key is never empty on its
    // own, so `park_message_draft`'s empty-room check cannot stand in for this.
    let open_rail = !channel_id.is_empty() && thread_seq > 0;
    if !open_rail {
        return drafts;
    }
    park_message_draft(drafts, thread_draft_key(&channel_id, thread_seq), text)
}

/// The parked reply text for one thread, or nothing.
pub fn parked_reply_draft(
    drafts: Vec<ChannelDraft>,
    channel_id: String,
    thread_seq: i64,
) -> String {
    parked_message_draft(drafts, thread_draft_key(&channel_id, thread_seq))
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

/// One channel row with the unread decision already attached. Ice externs take
/// lists by value, so a view-time lookup cloned the unread list once per row.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatSidebarRow {
    pub channel: ChatChannel,
    pub unread: bool,
}

/// The CHANNELS section, prepared when its source state moves.
pub fn chat_sidebar_rooms(
    channels: Vec<ChatChannel>,
    peers: Vec<DmPeer>,
    me: String,
    reads: Vec<ChannelRead>,
) -> Vec<ChatSidebarRow> {
    let read_seqs: BTreeMap<&str, i64> = reads
        .iter()
        .map(|read| (read.channel.as_str(), read.seq))
        .collect();
    let dm_ids: BTreeSet<String> = peers
        .iter()
        .filter_map(|peer| {
            if me.is_empty() {
                None
            } else {
                Some(dm_channel_id(me.clone(), peer.key.clone()))
            }
        })
        .collect();
    channels
        .into_iter()
        .filter(|channel| !dm_ids.contains(&channel.id))
        .map(|channel| ChatSidebarRow {
            unread: channel.head_seq
                > read_seqs
                    .get(channel.id.as_str())
                    .copied()
                    .unwrap_or_default(),
            channel,
        })
        .collect()
}

/// One DIRECT row with the unread decision already attached.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct DmSidebarRow {
    pub peer: DmPeer,
    pub unread: bool,
}

/// The DIRECT section, prepared when its directory, channels, or read cursors
/// move. Channel heads are indexed once so the projection itself stays linear.
pub fn chat_sidebar_dms(
    channels: Vec<ChatChannel>,
    peers: Vec<DmPeer>,
    reads: Vec<ChannelRead>,
) -> Vec<DmSidebarRow> {
    let read_seqs: BTreeMap<&str, i64> = reads
        .iter()
        .map(|read| (read.channel.as_str(), read.seq))
        .collect();
    let heads: BTreeMap<String, i64> = channels
        .into_iter()
        .map(|channel| (channel.id, channel.head_seq))
        .collect();
    peers
        .into_iter()
        .map(|peer| {
            let head_seq = heads.get(&peer.channel_id).copied().unwrap_or_default();
            DmSidebarRow {
                unread: head_seq
                    > read_seqs
                        .get(peer.channel_id.as_str())
                        .copied()
                        .unwrap_or_default(),
                peer,
            }
        })
        .collect()
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
/// Pending optimistic messages carry a negative seq, so they never anchor a divider.
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

/// The clicked message as the rail's first row, so a thread opens on the
/// message it is ABOUT instead of a blank 330px plate for the whole round trip.
/// `thread_loaded` replaces the vec wholesale on arrival, and a load that FAILS
/// leaves the root standing rather than a permanently empty pane.
///
/// BOTH LISTS, because `open_thread_for` is emitted from inside the rail too: a
/// re-root onto a reply names a seq that lives in `thread`, never in the
/// timeline. Answers empty when neither holds it — the honest state, and the
/// one the rail drew before.
pub fn thread_root_seed(
    messages: Vec<ChatMessage>,
    thread: Vec<ChatMessage>,
    seq: i64,
) -> Vec<ChatMessage> {
    messages
        .into_iter()
        .chain(thread)
        .find(|message| message.seq == seq)
        .into_iter()
        .collect()
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
