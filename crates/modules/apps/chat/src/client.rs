//! chat's CLIENT view model — the rendered row types, the composer's
//! markdown→wire parsing, the optimistic-send merge discipline, and the
//! op-delta fold a feed-following UI splices its state with.
//!
//! this is module-owned deliberately: a client folds the same applied-op
//! feed the index guest folds, so the fold vocabulary (rows, authorship
//! rendering, stamp decoding) lives beside `index.rs` — never pinned inside
//! a particular app shell. the desktop shell consumes it today via plain
//! linkage; the module-bundled-UI lane compiles this same module into the
//! shipped `ui.wasm`.
//!
//! everything here is PURE data-in/data-out: no IO, no async, no renderer
//! types. shell-side effects (rpc loads, iced styling, editors) stay in the
//! shell.

use index_guest::{OriginTag, user_handle};

use crate::index::{self, MsgRow};
use crate::{Block, ChatAssigned, ChatMsg, Mark, PostPolicy, Span, decode_msg};

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

// ============================================================================
// rendered row types — what a chat view iterates over
// ============================================================================

#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct ChatChannel {
    pub id: String,
    pub name: String,
    pub archived: bool,
    pub members_only: bool,
    pub huddle_count: i64,
    pub head_seq: i64,
}

#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct ChatReaction {
    pub emoji: String,
    pub count: i64,
    pub reacted_by_me: bool,
    /// rendered reactor handles — the dedupe set delta folds need for exact
    /// counts (mirrors the index row's reactor list; not rendered).
    pub reactors: Vec<String>,
}

#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct ChatMember {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub id: String,
    pub seq: i64,
    pub author: String,
    pub meta: String,
    pub body: String,
    pub blocks: Vec<ChatBlock>,
    pub pending: bool,
    pub rev: i64,
    pub edited: bool,
    pub deleted: bool,
    pub reply_count: i64,
    pub thread_seq: i64,
    pub show_author: bool,
    pub initial: String,
    pub avatar_r: f64,
    pub avatar_g: f64,
    pub avatar_b: f64,
    pub reactions: Vec<ChatReaction>,
}

impl Default for ChatMessage {
    fn default() -> Self {
        Self {
            id: String::new(),
            seq: 0,
            author: String::new(),
            meta: String::new(),
            body: String::new(),
            blocks: Vec::new(),
            pending: false,
            rev: 0,
            edited: false,
            deleted: false,
            reply_count: 0,
            thread_seq: 0,
            show_author: true,
            initial: String::new(),
            avatar_r: 0.0,
            avatar_g: 0.0,
            avatar_b: 0.0,
            reactions: Vec::new(),
        }
    }
}

// `f64` avatar tint keeps `Hash` off the derive; hash the bit pattern instead
// so aggregates over messages can still derive `Hash` for view memoization.
impl std::hash::Hash for ChatMessage {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.seq.hash(state);
        self.author.hash(state);
        self.meta.hash(state);
        self.body.hash(state);
        self.blocks.hash(state);
        self.pending.hash(state);
        self.rev.hash(state);
        self.edited.hash(state);
        self.deleted.hash(state);
        self.reply_count.hash(state);
        self.thread_seq.hash(state);
        self.show_author.hash(state);
        self.initial.hash(state);
        self.avatar_r.to_bits().hash(state);
        self.avatar_g.to_bits().hash(state);
        self.avatar_b.to_bits().hash(state);
        self.reactions.hash(state);
    }
}

/// One rendered block of a message body. `kind` is `paragraph` | `code` |
/// `quote` | `divider`. Plain paragraphs/quotes carry their exact text in
/// `text` (`rich=false`); formatted ones carry word-level `spans` for a
/// wrapping flex render (`rich=true`).
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct ChatBlock {
    pub kind: String,
    pub text: String,
    pub lang: String,
    pub rich: bool,
    pub spans: Vec<ChatSpan>,
}

/// A word-level run of a rich paragraph/quote, carrying its inline marks.
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct ChatSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub highlight: bool,
    pub link: String,
}

// ============================================================================
// the op-delta — one folded applied op, pre-rendered for state splicing
// ============================================================================

/// One folded chat op. `kind` picks the arm; unrelated fields sit at their
/// empty defaults. Rows are built by the SAME renderers hydration uses
/// ([`chat_message`] over a [`MsgRow`]), so a folded row and a fetched row
/// are indistinguishable.
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct ChatDelta {
    /// `posted` | `reply` | `edited` | `deleted` | `reaction` |
    /// `channel-created` | `channel-renamed` | `channel-archived` |
    /// `membership` | `channel-refresh` (huddle changed — the shell reloads
    /// the one channel row and re-emits it as `channel-updated`) |
    /// `channel-updated`.
    pub kind: String,
    pub channel_id: String,
    /// the target message (edited/deleted/reaction) or the new row's seq.
    pub seq: i64,
    /// the thread root (`reply`).
    pub root_seq: i64,
    /// the rendered row: full for `posted`/`reply`; content carrier
    /// (body/blocks/rev/meta) for `edited`.
    pub message: ChatMessage,
    /// the full channel row (`channel-created` / `channel-updated`).
    pub channel: ChatChannel,
    pub name: String,
    pub archived: bool,
    pub emoji: String,
    /// reaction/membership direction: added vs removed.
    pub added: bool,
    /// the reacting author's rendered handle (set-semantics dedupe).
    pub reactor: String,
    /// the reactor is this device's user.
    pub by_me: bool,
    pub member: ChatMember,
}

/// Translate one applied chat op (its feed-row parts) into a [`ChatDelta`].
/// `Ok(None)` = invisible to a chat UI (hook registration). `Err` = the op or
/// its stamp did not decode — the caller's signal to fall back to a scoped
/// resync (a CLIENT never wedges like the index fold does; it reloads).
///
/// `origin_kind` is `external` | `module` | `system` with `origin_id` as the
/// feed row carries it (external ids arrive as rendered user handles).
pub fn delta_from_op(
    payload: &[u8],
    assigned: Option<&serde_json::Value>,
    origin_kind: &str,
    origin_id: Option<&str>,
    current_user: Option<&[u8]>,
) -> Result<Option<ChatDelta>, String> {
    let msg = decode_msg(payload)?;
    let origin = origin_tag(origin_kind, origin_id);
    let delta = match msg {
        ChatMsg::CreateChannel {
            channel_id,
            name,
            post_policy,
        } => ChatDelta {
            kind: "channel-created".into(),
            channel_id: channel_id.clone(),
            channel: ChatChannel {
                id: channel_id,
                name,
                archived: false,
                members_only: post_policy == PostPolicy::MembersOnly,
                huddle_count: 0,
                head_seq: 0,
            },
            ..ChatDelta::default()
        },
        ChatMsg::RenameChannel { channel_id, name } => ChatDelta {
            kind: "channel-renamed".into(),
            channel_id,
            name,
            ..ChatDelta::default()
        },
        ChatMsg::SetChannelArchived {
            channel_id,
            archived,
        } => ChatDelta {
            kind: "channel-archived".into(),
            channel_id,
            archived,
            ..ChatDelta::default()
        },
        ChatMsg::PostMessage {
            channel_id,
            message_id,
            blocks,
            thread,
            as_agent,
        } => {
            let ChatAssigned::Posted { seq } = decode_stamp(assigned)? else {
                return Err("applied PostMessage carried a non-Posted stamp".into());
            };
            let row = MsgRow {
                channel_id: channel_id.clone(),
                seq,
                message_id,
                author: index::author(&origin, as_agent.as_deref()),
                height: 0,
                time: 0,
                blocks,
                text: String::new(),
                deleted: false,
                edited: false,
                rev: 0,
                edited_at: None,
                base_rev: None,
                thread,
                reply_count: 0,
                last_reply_seq: None,
                reactions: Vec::new(),
                tags: Vec::new(),
            };
            let kind = match thread {
                Some(_) => "reply",
                None => "posted",
            };
            ChatDelta {
                kind: kind.into(),
                channel_id,
                seq: number_i64(seq),
                root_seq: number_i64(thread.unwrap_or(0)),
                message: chat_message(row, current_user),
                ..ChatDelta::default()
            }
        }
        ChatMsg::EditMessage {
            channel_id,
            seq,
            blocks,
            base_rev,
        } => {
            let ChatAssigned::Edited { rev } = decode_stamp(assigned)? else {
                return Err("applied EditMessage carried a non-Edited stamp".into());
            };
            let carrier = MsgRow {
                channel_id: channel_id.clone(),
                seq,
                message_id: String::new(),
                author: index::author(&origin, None),
                height: 0,
                time: 0,
                blocks,
                text: String::new(),
                deleted: false,
                edited: true,
                rev,
                edited_at: None,
                base_rev,
                thread: None,
                reply_count: 0,
                last_reply_seq: None,
                reactions: Vec::new(),
                tags: Vec::new(),
            };
            ChatDelta {
                kind: "edited".into(),
                channel_id,
                seq: number_i64(seq),
                message: chat_message(carrier, current_user),
                ..ChatDelta::default()
            }
        }
        ChatMsg::DeleteMessage { channel_id, seq } => ChatDelta {
            kind: "deleted".into(),
            channel_id,
            seq: number_i64(seq),
            ..ChatDelta::default()
        },
        ChatMsg::AddReaction {
            channel_id,
            seq,
            emoji,
        } => reaction_delta(channel_id, seq, emoji, true, &origin, current_user),
        ChatMsg::RemoveReaction {
            channel_id,
            seq,
            emoji,
        } => reaction_delta(channel_id, seq, emoji, false, &origin, current_user),
        ChatMsg::RegisterHook { .. } | ChatMsg::UnregisterHook { .. } => return Ok(None),
        ChatMsg::SetMembership {
            channel_id,
            user,
            member,
        } => {
            let id = user_handle(&user);
            ChatDelta {
                kind: "membership".into(),
                channel_id,
                added: member,
                member: ChatMember {
                    label: short_label(&id),
                    key: id,
                },
                ..ChatDelta::default()
            }
        }
        ChatMsg::JoinHuddle { channel_id, .. }
        | ChatMsg::LeaveHuddle { channel_id }
        | ChatMsg::SweepHuddle { channel_id, .. } => ChatDelta {
            kind: "channel-refresh".into(),
            channel_id,
            ..ChatDelta::default()
        },
    };
    Ok(Some(delta))
}

fn reaction_delta(
    channel_id: String,
    seq: u64,
    emoji: String,
    added: bool,
    origin: &OriginTag,
    current_user: Option<&[u8]>,
) -> ChatDelta {
    let reactor = index::author(origin, None);
    let by_me = current_user.is_some_and(|key| reactor == format!("user:{}", hex_encode(key)));
    ChatDelta {
        kind: "reaction".into(),
        channel_id,
        seq: number_i64(seq),
        emoji,
        added,
        reactor,
        by_me,
        ..ChatDelta::default()
    }
}

fn origin_tag(kind: &str, id: Option<&str>) -> OriginTag {
    match kind {
        "external" => OriginTag::external(id.unwrap_or_default()),
        "module" => OriginTag::module(id.unwrap_or_default()),
        _ => OriginTag::system(),
    }
}

fn decode_stamp(assigned: Option<&serde_json::Value>) -> Result<ChatAssigned, String> {
    let value = assigned.ok_or("applied assigning op carried no stamp")?;
    serde_json::from_value(value.clone()).map_err(|e| e.to_string())
}

// ============================================================================
// delta splices — pure list surgery for one folded op. every helper is
// idempotent on its target key (seq / message id / reactor handle), so a
// delta that raced a resync applies as a no-op instead of double-counting.
// ============================================================================

/// Fold one chat delta into the channel list.
pub fn apply_chat_channels(mut channels: Vec<ChatChannel>, delta: ChatDelta) -> Vec<ChatChannel> {
    match delta.kind.as_str() {
        "channel-created" => {
            let exists = channels.iter().any(|channel| channel.id == delta.channel.id);
            if !exists {
                channels.push(delta.channel);
            }
        }
        "channel-updated" => match channels
            .iter_mut()
            .find(|channel| channel.id == delta.channel.id)
        {
            Some(channel) => *channel = delta.channel,
            None => channels.push(delta.channel),
        },
        "channel-renamed" => {
            if let Some(channel) = channels.iter_mut().find(|c| c.id == delta.channel_id) {
                channel.name = delta.name;
            }
        }
        "channel-archived" => {
            if let Some(channel) = channels.iter_mut().find(|c| c.id == delta.channel_id) {
                channel.archived = delta.archived;
            }
        }
        "posted" | "reply" => {
            if let Some(channel) = channels.iter_mut().find(|c| c.id == delta.channel_id) {
                channel.head_seq = channel.head_seq.max(delta.seq);
            }
        }
        _ => {}
    }
    channels
}

/// Fold one chat delta into the ACTIVE channel's root timeline.
pub fn apply_chat_messages(
    messages: Vec<ChatMessage>,
    delta: ChatDelta,
    active_channel: String,
) -> Vec<ChatMessage> {
    if delta.channel_id != active_channel {
        return messages;
    }
    match delta.kind.as_str() {
        "posted" => insert_committed_root(messages, delta.message),
        "reply" => bump_reply_summary(messages, delta.root_seq),
        "edited" => apply_edit_content(messages, delta.seq, &delta.message),
        "deleted" => apply_tombstone(messages, delta.seq),
        "reaction" => apply_reaction(messages, &delta),
        _ => messages,
    }
}

/// Fold one chat delta into the OPEN thread panel (root + loaded replies).
pub fn apply_chat_thread(
    thread: Vec<ChatMessage>,
    delta: ChatDelta,
    active_channel: String,
    active_thread_seq: i64,
) -> Vec<ChatMessage> {
    if delta.channel_id != active_channel || active_thread_seq <= 0 {
        return thread;
    }
    match delta.kind.as_str() {
        "reply" if delta.root_seq == active_thread_seq => merge_thread_reply(thread, delta.message),
        "edited" => apply_edit_content(thread, delta.seq, &delta.message),
        "deleted" => apply_tombstone(thread, delta.seq),
        "reaction" => apply_reaction(thread, &delta),
        _ => thread,
    }
}

/// Fold one chat delta into the ACTIVE channel's member panel.
pub fn apply_chat_members(
    mut members: Vec<ChatMember>,
    delta: ChatDelta,
    active_channel: String,
) -> Vec<ChatMember> {
    let is_membership = delta.kind == "membership" && delta.channel_id == active_channel;
    if !is_membership {
        return members;
    }
    members.retain(|member| member.key != delta.member.key);
    if delta.added {
        members.push(delta.member);
    }
    members
}

/// A committed root row lands: settle the matching pending row in place, or
/// append in seq order. Skips replies (the timeline is roots-only) and rows
/// already present.
fn insert_committed_root(mut messages: Vec<ChatMessage>, row: ChatMessage) -> Vec<ChatMessage> {
    if row.thread_seq > 0 {
        return messages;
    }
    if let Some(pending) = messages
        .iter_mut()
        .find(|message| message.pending && message.id == row.id)
    {
        *pending = row;
        mark_message_groups(&mut messages);
        return messages;
    }
    let already_present = messages
        .iter()
        .any(|message| !message.pending && message.seq == row.seq);
    if already_present {
        return messages;
    }
    // committed rows stay seq-sorted; pending rows tail the list.
    let insert_at = messages
        .iter()
        .position(|message| message.pending || message.seq > row.seq)
        .unwrap_or(messages.len());
    messages.insert(insert_at, row);
    mark_message_groups(&mut messages);
    messages
}

fn bump_reply_summary(mut messages: Vec<ChatMessage>, root_seq: i64) -> Vec<ChatMessage> {
    if let Some(root) = messages
        .iter_mut()
        .find(|message| !message.pending && message.seq == root_seq)
    {
        // not replay-proof on its own (the row carries no last-reply
        // high-water); the stream delivers each op once per cursor, and a
        // reconnect runs the ready-resync which reloads the canonical count.
        root.reply_count += 1;
    }
    messages
}

/// Copy an edit's content fields onto the target row, keeping identity fields
/// (author, reactions, reply summary) intact. An older or replayed revision
/// applies as a no-op.
fn apply_edit_content(
    mut messages: Vec<ChatMessage>,
    seq: i64,
    content: &ChatMessage,
) -> Vec<ChatMessage> {
    if let Some(row) = messages
        .iter_mut()
        .find(|message| !message.pending && message.seq == seq)
    {
        let stale = row.deleted || row.rev >= content.rev;
        if !stale {
            row.body = content.body.clone();
            row.blocks = content.blocks.clone();
            row.rev = content.rev;
            row.edited = true;
            row.meta = content.meta.clone();
        }
    }
    messages
}

/// The canonical tombstone shape, exactly as a hydrated deleted row renders.
fn apply_tombstone(mut messages: Vec<ChatMessage>, seq: i64) -> Vec<ChatMessage> {
    if let Some(row) = messages
        .iter_mut()
        .find(|message| !message.pending && message.seq == seq)
    {
        row.deleted = true;
        row.body = "Message deleted".into();
        row.blocks = vec![deleted_block()];
        row.reactions = Vec::new();
    }
    mark_message_groups(&mut messages);
    messages
}

/// Reactor-set semantics, mirroring the index fold: a reactor appears at most
/// once per emoji, so replayed or double-submitted reactions cannot drift the
/// count.
fn apply_reaction(mut messages: Vec<ChatMessage>, delta: &ChatDelta) -> Vec<ChatMessage> {
    let Some(row) = messages
        .iter_mut()
        .find(|message| !message.pending && message.seq == delta.seq)
    else {
        return messages;
    };
    if row.deleted {
        return messages;
    }
    match row
        .reactions
        .iter_mut()
        .find(|reaction| reaction.emoji == delta.emoji)
    {
        Some(reaction) => {
            reaction.reactors.retain(|reactor| *reactor != delta.reactor);
            if delta.added {
                reaction.reactors.push(delta.reactor.clone());
            }
            reaction.count = count_i64(reaction.reactors.len());
            reaction.reacted_by_me = match delta.by_me {
                true => delta.added,
                false => reaction.reacted_by_me,
            };
        }
        None if delta.added => row.reactions.push(ChatReaction {
            emoji: delta.emoji.clone(),
            count: 1,
            reacted_by_me: delta.by_me,
            reactors: vec![delta.reactor.clone()],
        }),
        None => {}
    }
    row.reactions.retain(|reaction| reaction.count > 0);
    messages
}

// ============================================================================
// optimistic sends — client-minted rows and their settle/rollback merges
// ============================================================================

pub fn optimistic_message(
    mut messages: Vec<ChatMessage>,
    body: String,
    message_id: String,
) -> Vec<ChatMessage> {
    let (avatar_r, avatar_g, avatar_b) = avatar_rgb_for("You");
    let blocks = paragraph_blocks(&body);
    messages.push(ChatMessage {
        id: message_id,
        seq: -1,
        author: "You".into(),
        meta: "Sending…".into(),
        body,
        blocks,
        pending: true,
        rev: 0,
        edited: false,
        deleted: false,
        reply_count: 0,
        thread_seq: 0,
        show_author: true,
        initial: "Y".into(),
        avatar_r,
        avatar_g,
        avatar_b,
        reactions: Vec::new(),
    });
    messages
}

pub fn merge_pending_messages(
    mut canonical: Vec<ChatMessage>,
    current: Vec<ChatMessage>,
    current_channel: String,
    next_channel: String,
    settled_id: String,
) -> Vec<ChatMessage> {
    if current_channel != next_channel {
        return canonical;
    }
    let canonical_ids = canonical
        .iter()
        .map(|message| message.id.clone())
        .collect::<BTreeSet<_>>();
    canonical.extend(current.into_iter().filter(|message| {
        message.pending && message.id != settled_id && !canonical_ids.contains(&message.id)
    }));
    canonical
}

pub fn merge_message_send_result(
    canonical: Vec<ChatMessage>,
    current: Vec<ChatMessage>,
    current_channel: String,
    next_channel: String,
    settled_id: String,
) -> Vec<ChatMessage> {
    if current_channel != next_channel {
        return canonical;
    }
    let canonical_ids = canonical
        .iter()
        .map(|message| message.id.clone())
        .collect::<BTreeSet<_>>();
    let mut committed = current
        .iter()
        .filter(|message| !message.pending && message.seq > 0)
        .map(|message| (message.seq, message.clone()))
        .collect::<BTreeMap<_, _>>();
    for message in canonical {
        let replace = committed
            .get(&message.seq)
            .is_none_or(|current| message.rev >= current.rev);
        if replace {
            committed.insert(message.seq, message);
        }
    }
    let mut merged = committed.into_values().collect::<Vec<_>>();
    merged.extend(current.into_iter().filter(|message| {
        message.pending && message.id != settled_id && !canonical_ids.contains(&message.id)
    }));
    merged
}

pub fn rollback_pending_message(
    mut messages: Vec<ChatMessage>,
    pending_id: String,
    committed: bool,
) -> Vec<ChatMessage> {
    if !committed {
        messages.retain(|message| !message.pending || message.id != pending_id);
    }
    messages
}

pub fn contains_pending_message(messages: Vec<ChatMessage>, pending_id: String) -> bool {
    messages
        .iter()
        .any(|message| message.pending && message.id == pending_id)
}

pub fn append_thread_page(messages: Vec<ChatMessage>, next: Vec<ChatMessage>) -> Vec<ChatMessage> {
    merge_message_send_result(next, messages, String::new(), String::new(), String::new())
}

pub fn merge_thread_reply(messages: Vec<ChatMessage>, reply: ChatMessage) -> Vec<ChatMessage> {
    let settled_id = reply.id.clone();
    merge_message_send_result(
        vec![reply],
        messages,
        String::new(),
        String::new(),
        settled_id,
    )
}

pub fn thread_offset_after_reply(offset: i64, has_more: bool, committed: bool) -> i64 {
    if !committed || offset < 0 || has_more {
        offset
    } else {
        offset.saturating_add(1)
    }
}

// ============================================================================
// row rendering — MsgRow (the index/feed shape) → the rendered ChatMessage
// ============================================================================

pub fn chat_message(row: MsgRow, current_user: Option<&[u8]>) -> ChatMessage {
    let edited = row.rev > 0;
    let meta = if edited {
        format!("#{} · edited", row.seq)
    } else {
        format!("#{}", row.seq)
    };
    let (avatar_r, avatar_g, avatar_b) = avatar_rgb(&row.author);
    let blocks = if row.deleted {
        vec![deleted_block()]
    } else {
        blocks_view(&row.blocks)
    };
    ChatMessage {
        id: row.message_id,
        seq: number_i64(row.seq),
        author: author_name(&row.author),
        meta,
        body: if row.deleted {
            "Message deleted".into()
        } else {
            message_body(&row.blocks)
        },
        blocks,
        pending: false,
        rev: i64::from(row.rev),
        edited,
        deleted: row.deleted,
        reply_count: number_i64(row.reply_count),
        thread_seq: number_i64(row.thread.unwrap_or(0)),
        show_author: true,
        initial: avatar_initial(&row.author),
        avatar_r,
        avatar_g,
        avatar_b,
        reactions: row
            .reactions
            .into_iter()
            .map(|reaction| {
                let reacted_by_me = reacted_by_user(&reaction.reactors, current_user);
                ChatReaction {
                    emoji: reaction.emoji,
                    count: count_i64(reaction.reactors.len()),
                    reacted_by_me,
                    reactors: reaction.reactors,
                }
            })
            .collect(),
    }
}

/// True when the local user's rendered author string (`user:{hex}`) is among a
/// reaction's reactors.
fn reacted_by_user(reactors: &[String], current_user: Option<&[u8]>) -> bool {
    current_user.is_some_and(|key| {
        let handle = format!("user:{}", hex_encode(key));
        reactors.contains(&handle)
    })
}

/// Slack-style grouping: a message shows its avatar + author header only when it
/// opens a run — the first message, or one whose author differs from the message
/// above it. Deleted messages always break a run (neither joins nor extends one).
pub fn mark_message_groups(messages: &mut [ChatMessage]) {
    let opens_run: Vec<bool> = messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            index == 0
                || message.deleted
                || messages[index - 1].deleted
                || messages[index - 1].author != message.author
        })
        .collect();
    for (message, show) in messages.iter_mut().zip(opens_run) {
        message.show_author = show;
    }
}

pub fn message_body(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(|block| match block {
            Block::Paragraph(spans) => span_text(spans),
            Block::Code { lang, text } => match lang {
                Some(lang) => format!("{lang}\n{text}"),
                None => text.clone(),
            },
            Block::Quote(spans) => format!("“{}”", span_text(spans)),
            Block::Divider => "────────".into(),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn span_text(spans: &[Span]) -> String {
    spans.iter().map(|span| span.text.as_str()).collect()
}

/// Convert wire `Block`s into the render model the view iterates over.
pub fn blocks_view(blocks: &[Block]) -> Vec<ChatBlock> {
    blocks.iter().map(block_view).collect()
}

/// A single plain paragraph render block — for optimistic sends and fixtures
/// that never carry inline marks.
pub fn paragraph_blocks(text: &str) -> Vec<ChatBlock> {
    vec![ChatBlock {
        kind: "paragraph".into(),
        text: text.to_string(),
        lang: String::new(),
        rich: false,
        spans: Vec::new(),
    }]
}

fn deleted_block() -> ChatBlock {
    ChatBlock {
        kind: "paragraph".into(),
        text: "Message deleted".into(),
        lang: String::new(),
        rich: false,
        spans: Vec::new(),
    }
}

fn block_view(block: &Block) -> ChatBlock {
    match block {
        Block::Paragraph(spans) => rich_block("paragraph", spans),
        Block::Quote(spans) => rich_block("quote", spans),
        Block::Code { lang, text } => ChatBlock {
            kind: "code".into(),
            text: text.clone(),
            lang: lang.clone().unwrap_or_default(),
            rich: false,
            spans: Vec::new(),
        },
        Block::Divider => ChatBlock {
            kind: "divider".into(),
            text: String::new(),
            lang: String::new(),
            rich: false,
            spans: Vec::new(),
        },
    }
}

/// A paragraph/quote block. Plain runs keep their exact text for a single
/// wrapping `text`; any inline mark switches to word-level `spans` a wrapping
/// flex can reflow.
fn rich_block(kind: &str, spans: &[Span]) -> ChatBlock {
    let marked = spans.iter().any(|span| !span.marks.is_empty());
    ChatBlock {
        kind: kind.into(),
        text: span_text(spans),
        lang: String::new(),
        rich: marked,
        spans: if marked { word_spans(spans) } else { Vec::new() },
    }
}

fn word_spans(spans: &[Span]) -> Vec<ChatSpan> {
    let mut out = Vec::new();
    for span in spans {
        let bold = span.marks.iter().any(|m| matches!(m, Mark::Bold));
        let italic = span.marks.iter().any(|m| matches!(m, Mark::Italic));
        let link = span.marks.iter().find_map(|mark| match mark {
            Mark::Link(url) => Some(url.clone()),
            _ => None,
        });
        let mention = span.marks.iter().any(|m| matches!(m, Mark::Mention(_)));
        let highlight = link.is_some() || mention;
        // Keep the trailing space baked into each token so a wrapping flex with
        // zero column-gap reproduces exact spacing around mark boundaries (a
        // comma right after a bold run stays attached, not " ,").
        for token in span.text.split_inclusive(' ') {
            if token.is_empty() {
                continue;
            }
            out.push(ChatSpan {
                text: token.to_string(),
                bold,
                italic,
                highlight,
                link: link.clone().unwrap_or_default(),
            });
        }
    }
    out
}

// ============================================================================
// authorship + avatars — display identity derived from rendered handles
// ============================================================================

/// The display name for a rendered author string (`user:{id}`,
/// `agent:{module}/{agent}`, `module:{id}`, or `system`).
pub fn author_name(author: &str) -> String {
    match author.split_once(':') {
        Some(("user", id)) => format!("user {}", short_label(id)),
        Some(("agent", path)) => {
            let name = path.rsplit('/').next().unwrap_or(path);
            format!("@{name}")
        }
        Some(("module", id)) => id.to_string(),
        _ => "system".into(),
    }
}

/// The stable identity an avatar is derived from: the shortened id for a user,
/// the agent/module name otherwise. Both the initial glyph and the tint hash
/// off this so a given author always looks the same.
fn avatar_source(author: &str) -> String {
    match author.split_once(':') {
        Some(("user", id)) => short_label(id),
        Some(("agent", path)) => path.rsplit('/').next().unwrap_or(path).to_string(),
        Some(("module", id)) => id.to_string(),
        _ => "system".into(),
    }
}

/// The single-glyph avatar label for an author: the first alphanumeric character
/// of its identity, uppercased. Falls back to a neutral dot when there is
/// nothing to show.
fn avatar_initial(author: &str) -> String {
    initial_of(&avatar_source(author))
}

fn initial_of(source: &str) -> String {
    source
        .chars()
        .find(char::is_ascii_alphanumeric)
        .map_or_else(|| "•".into(), |c| c.to_ascii_uppercase().to_string())
}

/// Curated avatar tints — saturated mid-tones that keep near-white initials
/// legible, indexed by a stable hash of the author identity.
const AVATAR_PALETTE: [(u8, u8, u8); 8] = [
    (0x60, 0x62, 0xe8), // indigo
    (0x8a, 0x5c, 0xf0), // violet
    (0xc7, 0x4a, 0xae), // fuchsia
    (0xdc, 0x4f, 0x66), // rose
    (0xc2, 0x6a, 0x24), // amber
    (0x1a, 0x9d, 0x6e), // emerald
    (0x0e, 0x8f, 0x96), // teal
    (0x2f, 0x7a, 0xe0), // blue
];

/// The avatar tint for an author as linear 0..1 RGB.
pub fn avatar_rgb(author: &str) -> (f64, f64, f64) {
    avatar_rgb_for(&avatar_source(author))
}

/// The avatar tint for a bare identity string (used for optimistic/local authors).
pub fn avatar_rgb_for(source: &str) -> (f64, f64, f64) {
    let hash = source
        .bytes()
        .fold(0u32, |acc, byte| acc.wrapping_mul(31).wrapping_add(u32::from(byte)));
    let (r, g, b) = AVATAR_PALETTE[(hash as usize) % AVATAR_PALETTE.len()];
    (
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
    )
}

pub fn short_label(id: &str) -> String {
    let mut label: String = id.chars().take(8).collect();
    if id.chars().count() > 8 {
        label.push('…');
    }
    label
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn number_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn count_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

// ============================================================================
// composer parsing — markdown → wire blocks
// ============================================================================

/// Parse composer text into wire `Block`s: fenced ```code``` (optional language),
/// `>` quotes, `---`/`***` dividers, and paragraphs with inline `**bold**` /
/// `__bold__`, `*italic*` / `_italic_`, and bare `http(s)` links. Everything the
/// `chat` wire enums can round-trip — nothing client-only.
pub fn parse_message(input: &str) -> Vec<Block> {
    let lines: Vec<&str> = input.lines().collect();
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim();
        let opens_fence = trimmed.starts_with("```");
        let is_divider = trimmed == "---" || trimmed == "***";
        let is_quote = trimmed.starts_with('>');
        let is_blank = trimmed.is_empty();
        if opens_fence {
            index = push_code_block(&lines, index, trimmed, &mut blocks);
        } else if is_divider {
            blocks.push(Block::Divider);
            index += 1;
        } else if is_quote {
            index = push_quote_block(&lines, index, &mut blocks);
        } else if is_blank {
            index += 1;
        } else {
            index = push_paragraph_block(&lines, index, &mut blocks);
        }
    }
    if blocks.is_empty() {
        blocks.push(Block::paragraph(input.trim().to_string()));
    }
    blocks
}

fn push_code_block(lines: &[&str], start: usize, opener: &str, blocks: &mut Vec<Block>) -> usize {
    let lang = opener.trim_start_matches('`').trim().to_string();
    let mut index = start + 1;
    let mut code = Vec::new();
    while index < lines.len() && lines[index].trim() != "```" {
        code.push(lines[index]);
        index += 1;
    }
    let closed = index < lines.len();
    blocks.push(Block::Code {
        lang: (!lang.is_empty()).then_some(lang),
        text: code.join("\n"),
    });
    if closed { index + 1 } else { index }
}

fn push_quote_block(lines: &[&str], start: usize, blocks: &mut Vec<Block>) -> usize {
    let mut index = start;
    let mut quoted = Vec::new();
    while index < lines.len() && lines[index].trim().starts_with('>') {
        let stripped = lines[index].trim().trim_start_matches('>').trim_start();
        quoted.push(stripped.to_string());
        index += 1;
    }
    blocks.push(Block::Quote(inline_spans(&quoted.join(" "))));
    index
}

fn push_paragraph_block(lines: &[&str], start: usize, blocks: &mut Vec<Block>) -> usize {
    let mut index = start;
    let mut paragraph = Vec::new();
    while index < lines.len() {
        let trimmed = lines[index].trim();
        let breaks = trimmed.is_empty()
            || trimmed.starts_with('>')
            || trimmed.starts_with("```")
            || trimmed == "---"
            || trimmed == "***";
        if breaks {
            break;
        }
        paragraph.push(trimmed);
        index += 1;
    }
    blocks.push(Block::Paragraph(inline_spans(&paragraph.join(" "))));
    index
}

/// Scan a single line of text for inline marks, emitting marked `Span`s. Marks do
/// not nest; the first matching delimiter wins. Bare `http(s)://` runs become
/// `Link`s.
fn inline_spans(text: &str) -> Vec<Span> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans: Vec<Span> = Vec::new();
    let mut plain = String::new();
    let mut index = 0;
    while index < chars.len() {
        let url = url_len(&chars, index);
        let bold = fenced(&chars, index, "**").or_else(|| fenced(&chars, index, "__"));
        let italic = fenced(&chars, index, "*").or_else(|| fenced(&chars, index, "_"));
        if let Some(len) = url {
            flush_plain(&mut plain, &mut spans);
            let target: String = chars[index..index + len].iter().collect();
            spans.push(Span {
                text: target.clone(),
                marks: vec![Mark::Link(target)],
            });
            index += len;
        } else if let Some((inner, len)) = bold {
            flush_plain(&mut plain, &mut spans);
            spans.push(Span {
                text: inner,
                marks: vec![Mark::Bold],
            });
            index += len;
        } else if let Some((inner, len)) = italic {
            flush_plain(&mut plain, &mut spans);
            spans.push(Span {
                text: inner,
                marks: vec![Mark::Italic],
            });
            index += len;
        } else {
            plain.push(chars[index]);
            index += 1;
        }
    }
    flush_plain(&mut plain, &mut spans);
    if spans.is_empty() {
        spans.push(Span::plain(String::new()));
    }
    spans
}

fn flush_plain(plain: &mut String, spans: &mut Vec<Span>) {
    if !plain.is_empty() {
        spans.push(Span::plain(std::mem::take(plain)));
    }
}

/// If `chars[at..]` opens a bare link, its length in chars; else `None`.
fn url_len(chars: &[char], at: usize) -> Option<usize> {
    let rest: String = chars[at..].iter().collect();
    let starts_link = rest.starts_with("http://") || rest.starts_with("https://");
    if !starts_link {
        return None;
    }
    let len = chars[at..]
        .iter()
        .take_while(|c| !c.is_whitespace())
        .count();
    (len > 0).then_some(len)
}

/// If `chars[at..]` opens with `marker` and has a later closing `marker`, the
/// enclosed text and the total consumed length (markers included).
fn fenced(chars: &[char], at: usize, marker: &str) -> Option<(String, usize)> {
    let marks: Vec<char> = marker.chars().collect();
    let opens = chars[at..].starts_with(marks.as_slice());
    if !opens {
        return None;
    }
    let body_start = at + marks.len();
    let mut cursor = body_start;
    while cursor + marks.len() <= chars.len() {
        if chars[cursor..].starts_with(marks.as_slice()) {
            let inner: String = chars[body_start..cursor].iter().collect();
            if inner.is_empty() {
                return None;
            }
            return Some((inner, cursor + marks.len() - at));
        }
        cursor += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_message_maps_markdown_onto_wire_blocks() {
        let input = "Hello **world** and *friends*\n\n> quote me\n> across lines\n\n```rust\nfn main() {}\n```\n\nvisit https://ducktape.example for more";
        let blocks = parse_message(input);
        assert_eq!(blocks.len(), 4);

        // paragraph 1: plain / bold / plain / italic runs
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("first block is a paragraph");
        };
        assert_eq!(spans[0].text, "Hello ");
        assert!(spans[0].marks.is_empty());
        assert_eq!(spans[1].text, "world");
        assert_eq!(spans[1].marks, vec![Mark::Bold]);
        assert_eq!(spans[3].text, "friends");
        assert_eq!(spans[3].marks, vec![Mark::Italic]);

        // quote joins its lines
        let Block::Quote(quote) = &blocks[1] else {
            panic!("second block is a quote");
        };
        assert_eq!(span_text(quote), "quote me across lines");

        // fenced code keeps its language + raw text
        let Block::Code { lang, text } = &blocks[2] else {
            panic!("third block is code");
        };
        assert_eq!(lang.as_deref(), Some("rust"));
        assert_eq!(text, "fn main() {}");

        // bare url becomes a link mark
        let Block::Paragraph(spans) = &blocks[3] else {
            panic!("fourth block is a paragraph");
        };
        let link = spans
            .iter()
            .find(|span| matches!(span.marks.first(), Some(Mark::Link(_))))
            .expect("a link span");
        assert_eq!(link.text, "https://ducktape.example");
        assert_eq!(
            link.marks,
            vec![Mark::Link("https://ducktape.example".into())]
        );

        // round-trips into the flattened body + render model
        assert!(message_body(&blocks).contains("Hello world and friends"));
        let view = blocks_view(&blocks);
        assert_eq!(view[0].kind, "paragraph");
        assert!(view[0].rich, "formatted paragraph renders as spans");
        assert!(view[0].spans.iter().any(|span| span.bold && span.text == "world"));
        assert_eq!(view[2].kind, "code");
        assert_eq!(view[2].text, "fn main() {}");

        // a plain message stays a single non-rich paragraph
        let plain = blocks_view(&parse_message("just text here"));
        assert_eq!(plain.len(), 1);
        assert!(!plain[0].rich);
        assert_eq!(plain[0].text, "just text here");
    }

    #[test]
    fn reactions_know_the_local_reactor() {
        let reactors = vec![
            format!("user:{}", hex_encode(&[0xab; 32])),
            "system".to_string(),
        ];
        assert!(reacted_by_user(&reactors, Some(&[0xab; 32])));
        assert!(!reacted_by_user(&reactors, Some(&[0xcd; 32])));
        assert!(!reacted_by_user(&reactors, None));
    }
}
