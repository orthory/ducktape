use iced::widget::text_editor::Content;

use super::*;

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
            spans: plain_rich_spans(&text),
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

/// Whether the block editor holds work the node does not: a save in flight or
/// refused, or text that has drifted from the last text known saved. The tick
/// autosave means an edit is invisible to handlers until it fires, so the
/// text comparison — not the status — is what catches a fresh keystroke.
fn block_edit_unsaved(current: &str, saved: &str, autosave_status: &str) -> bool {
    let save_in_flight_or_refused = matches!(autosave_status, "saving" | "error");
    save_in_flight_or_refused || current != saved
}

/// The canonical replacement for the live-resync refresh: the block's
/// canonical text when the editor is clean, or `None` to keep the local
/// buffer (and its cursor) untouched.
fn refreshed_block_text(
    blocks: &[PageBlock],
    selected_id: &str,
    current: &str,
    saved: &str,
    autosave_status: &str,
) -> Option<String> {
    if selected_id.is_empty() || block_edit_unsaved(current, saved, autosave_status) {
        return None;
    }
    blocks
        .iter()
        .find(|block| block.id == selected_id)
        .map(|block| block.text.clone())
        .filter(|canonical| canonical != current)
}

/// The live-resync refresh of the block editor itself. The buffer is only
/// rebuilt when the canonical text actually differs — replacing an equal
/// `Content` would still throw the cursor to the origin mid-typing.
pub fn refreshed_block_editor(
    document: Content,
    blocks: Vec<PageBlock>,
    selected_id: String,
    saved: String,
    autosave_status: String,
) -> Content {
    let current = editor_block_text(&document);
    match refreshed_block_text(&blocks, &selected_id, &current, &saved, &autosave_status) {
        Some(canonical) => Content::with_text(&canonical),
        None => document,
    }
}

/// The saved-text mirror of [`refreshed_block_editor`] — the SAME decision on
/// the SAME inputs, so the editor and its dirty baseline move together.
pub fn refreshed_block_saved(
    current: String,
    blocks: Vec<PageBlock>,
    selected_id: String,
    saved: String,
    autosave_status: String,
) -> String {
    refreshed_block_text(&blocks, &selected_id, &current, &saved, &autosave_status).unwrap_or(saved)
}

/// [`retain_selected_string`] for the editor buffer: deselection empties it
/// without touching a live selection's buffer (identity pass-through keeps
/// the cursor).
pub fn retained_block_editor(document: Content, selected_id: String) -> Content {
    if selected_id.is_empty() {
        return Content::new();
    }
    document
}

/// One block's text as the wire sees it: `Content::text()` always appends a
/// trailing newline that was never typed, so it is stripped — and ONLY it, a
/// deliberate trailing space mid-edit must stay dirty-visible.
fn editor_block_text(document: &Content) -> String {
    let mut text = document.text();
    if text.ends_with('\n') {
        text.pop();
    }
    text
}

pub fn remember_orphaned_block_drafts(
    mut drafts: Vec<String>,
    blocks: Vec<PageBlock>,
    selected_id: String,
    current: String,
    saved: String,
    autosave_status: String,
) -> Vec<String> {
    let unsaved = block_edit_unsaved(&current, &saved, &autosave_status);
    if unsaved && selected_block_missing(&blocks, &selected_id) {
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

/// The nearest committed block ABOVE `id` in document order — where the
/// selection lands after a Backspace delete, so the typing flow walks up the
/// page instead of dropping to nothing.
pub fn previous_block_id(blocks: Vec<PageBlock>, id: String) -> String {
    let Some(index) = blocks.iter().position(|block| block.id == id) else {
        return String::new();
    };
    blocks[..index]
        .iter()
        .rev()
        .find(|block| !block.pending)
        .map(|block| block.id.clone())
        .unwrap_or_default()
}

/// The keyed-row key of one block, or -1 — a widget-target key that matches
/// no row, so a focus task built from it is a safe no-op.
pub fn block_key_of(blocks: Vec<PageBlock>, id: String) -> i64 {
    blocks
        .iter()
        .find(|block| block.id == id)
        .map_or(-1, |block| block.key)
}

/// The kind a fresh block continues with after Enter: list-like kinds repeat
/// themselves (a bullet's Enter makes the next bullet), everything else
/// starts a plain Text block.
pub fn follow_kind(kind: String) -> String {
    match kind.as_str() {
        "Bullet" | "Number" | "Todo" | "Toggle" => kind,
        _ => "Text".into(),
    }
}

/// The pure step behind the block editor's state-only keys. "split" opens the
/// insert row under the selected block with the continuing kind and names the
/// row to focus; "escape" drops the selection and orphans an unsaved draft.
/// Decide-fn only — the handler applies each field (decide-fns never write).
#[allow(clippy::too_many_arguments)]
pub fn block_key_step(
    action: String,
    blocks: Vec<PageBlock>,
    selected_id: String,
    selected_kind: String,
    selected_checked: bool,
    insert_open: bool,
    insert_after_id: String,
    insert_kind: String,
    current: String,
    saved: String,
    autosave_status: String,
    orphaned: Vec<String>,
) -> BlockKeyStep {
    let keep = BlockKeyStep {
        selected_id: selected_id.clone(),
        selected_kind: selected_kind.clone(),
        selected_checked,
        insert_open,
        insert_after_id: insert_after_id.clone(),
        insert_kind: insert_kind.clone(),
        focus_key: -1,
        orphaned: orphaned.clone(),
        autosave_bump: 0,
    };
    match action.as_str() {
        "split" => BlockKeyStep {
            insert_open: true,
            insert_after_id: selected_id.clone(),
            insert_kind: follow_kind(selected_kind),
            focus_key: block_key_of(blocks, selected_id),
            ..keep
        },
        "escape" => BlockKeyStep {
            selected_id: String::new(),
            selected_kind: String::new(),
            selected_checked: false,
            orphaned: remember_orphaned_block_drafts(
                orphaned,
                Vec::new(),
                selected_id,
                current,
                saved,
                autosave_status,
            ),
            autosave_bump: 1,
            ..keep
        },
        // classify_block_key routes only "split" and "escape" here.
        _ => keep,
    }
}

/// The insert draft's markdown-prefix pass, ordered longest-first so `### `
/// wins over `## ` over `# `.
const BLOCK_AUTOFORMATS: [(&str, &str); 11] = [
    ("### ", "Heading 3"),
    ("## ", "Heading 2"),
    ("# ", "Heading 1"),
    ("- ", "Bullet"),
    ("* ", "Bullet"),
    ("1. ", "Number"),
    ("[] ", "Todo"),
    ("[ ] ", "Todo"),
    ("> ", "Quote"),
    ("```", "Code"),
    ("---", "Divider"),
];

/// Converts a fresh insert draft's leading markdown shorthand into the block
/// kind it names, with the shorthand stripped from the draft.
pub fn autoformat_block_draft(draft: String, kind: String) -> BlockAutoformat {
    // Only the default kind converts — a "# " typed into a chosen Code or
    // Quote block is content, not a command.
    if kind != "Text" {
        return BlockAutoformat { kind, draft };
    }
    let matched = BLOCK_AUTOFORMATS
        .iter()
        .find_map(|(prefix, next)| draft.strip_prefix(prefix).map(|rest| (rest, *next)));
    let Some((rest, next)) = matched else {
        return BlockAutoformat { kind, draft };
    };
    BlockAutoformat {
        kind: next.into(),
        draft: rest.into(),
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

pub(crate) struct Tip {
    pub(crate) height: i64,
    pub(crate) status: String,
}

pub(crate) fn rpc_client(input: &str) -> Result<RpcClient, String> {
    let configured = if input.trim().is_empty() {
        std::env::var("DUCKTAPE_NODE")
            .ok()
            .or_else(|| registered_endpoint().map(str::to_string))
            .unwrap_or_else(|| DEFAULT_RPC.to_string())
    } else {
        input.trim().to_string()
    };
    RpcClient::new(&configured).map_err(Into::into)
}
