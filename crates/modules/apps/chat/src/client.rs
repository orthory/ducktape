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
use sha2::{Digest, Sha256};

use crate::index::{self, MsgRow};
use crate::{AuthorRef, Block, ChatAssigned, ChatMsg, Mark, PostPolicy, Span, decode_msg};

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
    pub avatar_kind: String,
    /// the block this message settled in; 0 while pending.
    pub height: i64,
    /// that block's `consensus_time` — a block HEIGHT on a validator network
    /// (bin/noded/src/index.rs stamps `consensus_time = height`) and unix
    /// MILLIS on a single-writer noded (bin/noded/src/main.rs). NEVER unix
    /// seconds: render it as a height, never as a wall clock. 0 while pending.
    pub time: i64,
    pub reactions: Vec<ChatReaction>,
}

/// The lazy-row dependency hash. `body` and `blocks` are deliberately NOT
/// hashed: they are the bulk of the record, and every visible change to them
/// arrives with a field this impl does hash — an edit bumps `rev` (the
/// optimistic-concurrency token `edit_message` requires), a delete flips
/// `deleted`, and the optimistic settle flips `pending`/`height`. Hashing the
/// full text again made every view rebuild re-hash the whole conversation:
/// the stream's `lazy` rows compare exactly this hash once per rebuild.
impl std::hash::Hash for ChatMessage {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let Self {
            id,
            seq,
            author,
            meta,
            body: _,
            blocks: _,
            pending,
            rev,
            edited,
            deleted,
            reply_count,
            thread_seq,
            show_author,
            initial,
            avatar_kind,
            height,
            time,
            reactions,
        } = self;
        id.hash(state);
        seq.hash(state);
        author.hash(state);
        meta.hash(state);
        pending.hash(state);
        rev.hash(state);
        edited.hash(state);
        deleted.hash(state);
        reply_count.hash(state);
        thread_seq.hash(state);
        show_author.hash(state);
        initial.hash(state);
        avatar_kind.hash(state);
        height.hash(state);
        time.hash(state);
        reactions.hash(state);
    }
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
            avatar_kind: String::new(),
            height: 0,
            time: 0,
            reactions: Vec::new(),
        }
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
/// `height` is the block the op was applied in — the only stamp a chat row
/// carries, since the chain has no wall clock. The op payload does not hold it
/// (the validator assigns it), so it arrives from the live stream beside the
/// payload and is written onto the row here. Without it a live-arriving message
/// renders with no stamp at all until the next full load re-reads it from the
/// index, which is what your own message did the moment you sent it.
pub fn delta_from_op(
    payload: &[u8],
    assigned: Option<&serde_json::Value>,
    origin_kind: &str,
    origin_id: Option<&str>,
    current_user: Option<&[u8]>,
    height: u64,
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
                height,
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
                // An edit's stamp is the ORIGINAL post's block, never the
                // edit's — and `apply_edit_content` copies only body/blocks/
                // rev/edited/meta off this carrier, so the row on screen keeps
                // the height it was posted at. Left 0 deliberately.
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
        // the rail's root carries the reply summary its replies rule draws —
        // bump it here exactly as the stream's timeline arm does, then merge
        // the reply row itself.
        "reply" if delta.root_seq == active_thread_seq => {
            merge_thread_reply(bump_reply_summary(thread, delta.root_seq), delta.message)
        }
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

/// The optimistic half of a reaction tap: the SAME reactor-set fold the live
/// delta rides, applied locally before the op is submitted. `reactor` must be
/// the canonical rendered handle (`user:{hex}`) — set semantics then make the
/// real delta's replay a no-op instead of a double count.
pub fn optimistic_reaction(
    messages: Vec<ChatMessage>,
    seq: i64,
    emoji: String,
    added: bool,
    reactor: String,
) -> Vec<ChatMessage> {
    let delta = ChatDelta {
        kind: "reaction".into(),
        seq,
        emoji,
        added,
        reactor,
        by_me: true,
        ..ChatDelta::default()
    };
    apply_reaction(messages, &delta)
}

/// True when this delta settles one of OUR optimistic rows — the pop edge of
/// the timeline's transient ✓. Read BEFORE the delta is folded in: the match
/// is the pending row the canonical row is about to replace.
///
/// Borrowed, not owned: this is no longer an Ice extern (the app calls it
/// through the fused `chat_settle`, which owns its arguments once), so nothing
/// here has to pay the by-value boundary.
pub fn send_settled_by(messages: &[ChatMessage], delta: &ChatDelta, active_channel: &str) -> bool {
    delta.kind == "posted"
        && delta.channel_id == active_channel
        && messages
            .iter()
            .any(|message| message.pending && message.id == delta.message.id)
}

/// [`send_settled_by`] for the OPEN thread rail: a settling reply arrives as
/// a `reply` delta, not a `posted` one, and its pending row lives in
/// `thread_messages`. Read BEFORE `apply_chat_thread` folds the delta in.
pub fn reply_settled_by(thread: &[ChatMessage], delta: &ChatDelta, active_channel: &str) -> bool {
    delta.kind == "reply"
        && delta.channel_id == active_channel
        && thread
            .iter()
            .any(|message| message.pending && message.id == delta.message.id)
}

// ============================================================================
// optimistic sends — client-minted rows and their settle/rollback merges
// ============================================================================

/// The row a send paints before the block lands.
///
/// It is minted through the SAME author/avatar path [`chat_message`] renders a
/// committed row with, because [`mark_message_groups`] opens a run on an author
/// change and a hand-written label is always a change: a hard-coded `"You"`
/// against the canonical `"you"` gave every send of your own a full avatar +
/// header, which then vanished — shifting the row up by the header's height —
/// the moment the settle delta replaced it. Without a cached identity there is
/// no handle to render, so the row stays unattributed and simply opens its own
/// run, which is what an unknown author means everywhere else.
///
/// It does NOT re-mark the runs: the thread rail mints through here too, and
/// its vec is `[root] ++ replies` — the root renders as its own divided block,
/// so a whole-vec pass would fold the first reply under it and swallow that
/// reply's header (see `load_thread_data`, which marks the replies only). The
/// timeline re-marks at its call site, where the vec is a plain run.
pub fn optimistic_message(
    mut messages: Vec<ChatMessage>,
    body: String,
    message_id: String,
    current_user: Option<&[u8]>,
) -> Vec<ChatMessage> {
    let blocks = paragraph_blocks(&body);
    let handle = current_user.map(|key| format!("user:{}", hex_encode(key)));
    let (author, initial) = match handle.as_deref() {
        Some(handle) => (author_display(handle, current_user), avatar_initial(handle)),
        None => ("you".into(), "•".into()),
    };
    // EVERY PENDING ROW GETS ITS OWN SEQ, descending below zero. The timeline
    // is a keyed virtual column now (`by=message.seq`), so a shared sentinel is
    // a shared row identity: two sends in flight at once would collide on one
    // key and share one row's widget state and measured height. Nothing reads a
    // pending seq numerically — `oldest_committed` skips pending rows entirely,
    // `first_unread_seq` needs `seq > boundary > 0`, and `prepend_history` and
    // `merge_message_send_result` both partition pending out before sorting —
    // so the only thing this number has to be is unique and below every
    // committed seq.
    let lowest_seq = messages
        .iter()
        .map(|message| message.seq)
        .min()
        .unwrap_or_default();
    messages.push(ChatMessage {
        id: message_id,
        seq: lowest_seq.min(0) - 1,
        author,
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
        initial,
        avatar_kind: "human".into(),
        height: 0,
        time: 0,
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
    let blocks = if row.deleted {
        vec![deleted_block()]
    } else {
        blocks_view(&row.blocks)
    };
    ChatMessage {
        id: row.message_id,
        seq: number_i64(row.seq),
        author: author_display(&row.author, current_user),
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
        avatar_kind: avatar_kind(&row.author).into(),
        height: number_i64(row.height),
        time: number_i64(row.time),
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

/// True when the message's rendered author IS the local user — the same
/// `user:{hex}` comparison the reaction check makes, against the same key.
fn authored_by_user(author: &str, current_user: Option<&[u8]>) -> bool {
    current_user.is_some_and(|key| author == format!("user:{}", hex_encode(key)))
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

/// Flatten wire blocks back into composer text — the seed for an edit draft.
///
/// One `\n` per block boundary, because that is what a block boundary now MEANS
/// in the composer (`parse_message_with_members` makes every typed line its own
/// block). A `\n\n` here re-parsed to the same blocks, but it handed the editor
/// a blank line the author never typed, and every edit added another.
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
        .join("\n")
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

/// Word-level rich runs of one block of text — the pages renderer's view of
/// the chat inline grammar (no roster, so mentions stay plain ink). Empty when
/// the text carries no inline mark, keeping the plain single-`text` render;
/// multi-line text stays plain because a wrapping flex cannot force its line
/// breaks.
pub fn plain_rich_spans(text: &str) -> Vec<ChatSpan> {
    if text.contains('\n') {
        return Vec::new();
    }
    let spans = inline_spans(text, &[]);
    let marked = spans.iter().any(|span| !span.marks.is_empty());
    if !marked {
        return Vec::new();
    }
    word_spans(&spans)
}

// ============================================================================
// authorship + avatars — display identity derived from rendered handles
// ============================================================================

/// An [`AuthorRef`] as the rendered handle the display fns parse — the same
/// vocabulary the index stamps (`user:{hex}`, `agent:{module}/{agent}`,
/// `module:{id}`, `system`), so every module surface names an author
/// identically.
pub fn author_handle(author: &AuthorRef) -> String {
    match author {
        AuthorRef::User(key) => format!("user:{}", hex_encode(key)),
        AuthorRef::Agent { module, agent_id } => format!("agent:{module}/{agent_id}"),
        AuthorRef::Module(id) => format!("module:{id}"),
        AuthorRef::System => "system".into(),
    }
}

/// The display name for a rendered author string (`user:{id}`,
/// `agent:{module}/{agent}`, `module:{id}`, or `system`).
/// The author label a READER sees: `you` for the reader's own writing, the
/// rendered handle otherwise.
///
/// Every other surface in the shell already says `you` — the huddle roster, the
/// member roster — while the timeline printed the reader's own messages as
/// `user 3f8dc828…`, a hex nobody recognises as themselves. The plain
/// [`author_name`] stays for the places that render an author with no reader in
/// frame (a page comment's opener, a wire label).
pub fn author_display(author: &str, current_user: Option<&[u8]>) -> String {
    match authored_by_user(author, current_user) {
        true => "you".into(),
        false => author_name(author),
    }
}

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
/// the agent/module name otherwise.
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

fn avatar_kind(author: &str) -> &'static str {
    match author.split_once(':') {
        Some(("user", _)) => "human",
        Some(_) | None => "agent",
    }
}

fn initial_of(source: &str) -> String {
    source
        .chars()
        .find(char::is_ascii_alphanumeric)
        .map_or_else(|| "•".into(), |c| c.to_ascii_uppercase().to_string())
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
///
/// A SINGLE NEWLINE IS A HARD BREAK, not CommonMark's soft break. The composer
/// hint says `⇧↵ newline` and `⇧↵` really does put a `\n` in the buffer, so
/// folding consecutive lines into one paragraph with a space posted a typed
/// list as "- apples - bananas - pears" — and the fold happens on the way to
/// the CHAIN, so no renderer recovers it. Each line is therefore its own block.
/// A rendered break has to be a block boundary rather than a `\n` inside one:
/// a marked-up line renders as a wrapping flex of word tokens (`word_spans`),
/// and a flex cannot force a line break.
pub fn parse_message(input: &str) -> Vec<Block> {
    parse_message_with_members(input, &[])
}

/// [`parse_message`] with `@mention` resolution against the channel's member
/// roster: `@` followed by four or more characters that case-insensitively
/// prefix-match a member's key becomes a [`Mark::Mention`] span, which the
/// module fans out to hooks (inbox notifications) on apply. Non-matching
/// `@word`s stay plain text.
pub fn parse_message_with_members(input: &str, members: &[ChatMember]) -> Vec<Block> {
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
            index = push_quote_block(&lines, index, members, &mut blocks);
        } else if is_blank {
            index += 1;
        } else {
            index = push_paragraph_block(&lines, index, members, &mut blocks);
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

fn push_quote_block(
    lines: &[&str],
    start: usize,
    members: &[ChatMember],
    blocks: &mut Vec<Block>,
) -> usize {
    let mut index = start;
    while index < lines.len() && lines[index].trim().starts_with('>') {
        let stripped = lines[index].trim().trim_start_matches('>').trim_start();
        blocks.push(Block::Quote(inline_spans(stripped, members)));
        index += 1;
    }
    index
}

fn push_paragraph_block(
    lines: &[&str],
    start: usize,
    members: &[ChatMember],
    blocks: &mut Vec<Block>,
) -> usize {
    let mut index = start;
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
        blocks.push(Block::Paragraph(inline_spans(trimmed, members)));
        index += 1;
    }
    index
}

/// Scan a single line of text for inline marks, emitting marked `Span`s. Marks do
/// not nest; the first matching delimiter wins. Bare `http(s)://` runs become
/// `Link`s.
fn inline_spans(text: &str, members: &[ChatMember]) -> Vec<Span> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans: Vec<Span> = Vec::new();
    let mut plain = String::new();
    let mut index = 0;
    while index < chars.len() {
        let url = url_len(&chars, index);
        let bold = fenced(&chars, index, "**").or_else(|| fenced(&chars, index, "__"));
        let italic = fenced(&chars, index, "*").or_else(|| fenced(&chars, index, "_"));
        if let Some((member, len)) = mention_at(&chars, index, members) {
            flush_plain(&mut plain, &mut spans);
            let handle: String = chars[index..index + len].iter().collect();
            spans.push(Span {
                text: handle,
                marks: vec![Mark::Mention(AuthorRef::User(member_key_bytes(&member)))],
            });
            index += len;
        } else if let Some(len) = url {
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

/// If `chars[at..]` opens an `@mention` of a channel member — `@` plus four
/// or more word characters that case-insensitively prefix-match a member's
/// key — the matched member and the consumed length (the `@` included).
fn mention_at(chars: &[char], at: usize, members: &[ChatMember]) -> Option<(ChatMember, usize)> {
    let opens = chars[at] == '@' && (at == 0 || chars[at - 1].is_whitespace());
    if !opens || members.is_empty() {
        return None;
    }
    let word: String = chars[at + 1..]
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    if word.chars().count() < 4 {
        return None;
    }
    let needle = word.to_ascii_lowercase();
    let member = members
        .iter()
        .find(|member| member.key.to_ascii_lowercase().starts_with(&needle))?
        .clone();
    Some((member, 1 + word.chars().count()))
}

/// A member key back to `AuthorRef::User` bytes: keys are rendered by
/// [`user_handle`] — printable identities pass through verbatim, raw key
/// bytes arrive hex-encoded — so an even-length all-hex key decodes, and
/// anything else is the identity's own bytes.
fn member_key_bytes(member: &ChatMember) -> Vec<u8> {
    let key = &member.key;
    let looks_hex =
        !key.is_empty() && key.len().is_multiple_of(2) && key.bytes().all(|b| b.is_ascii_hexdigit());
    if looks_hex {
        let decode = |range: &str| u8::from_str_radix(range, 16).ok();
        let bytes: Option<Vec<u8>> = (0..key.len())
            .step_by(2)
            .map(|i| decode(&key[i..i + 2]))
            .collect();
        if let Some(bytes) = bytes {
            return bytes;
        }
    }
    key.as_bytes().to_vec()
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

// ============================================================================
// direct messages — the derived two-party channel id
// ============================================================================

/// The two-party channel id for a pair of member keys (lowercase hex), sorted
/// so both ends of the pair derive the same id and re-opening is idempotent.
///
/// The id is BARE by construction, and that is the whole point:
/// [`Chat::validate_channel_namespace`] refuses a user-authored id containing
/// ':' (that namespace belongs to module origins) and refuses '/' from anyone,
/// while a DM is created by one of the pair's own USER keys. Hashing also keeps
/// the id at 67 characters instead of the 130 the two keys spell out.
pub fn dm_channel_id(a: &str, b: &str) -> String {
    let (low, high) = match a <= b {
        true => (a, b),
        false => (b, a),
    };
    let mut digest = Sha256::new();
    digest.update(low.as_bytes());
    digest.update([0x1f]);
    digest.update(high.as_bytes());
    let mut id = String::from("dm-");
    for byte in digest.finalize() {
        let _ = write!(id, "{byte:02x}");
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_delta_bumps_the_open_thread_roots_summary() {
        let root = ChatMessage {
            seq: 1,
            id: "g1".into(),
            reply_count: 1,
            ..ChatMessage::default()
        };
        let reply = ChatMessage {
            seq: 3,
            id: "g3".into(),
            thread_seq: 1,
            ..ChatMessage::default()
        };
        let delta = ChatDelta {
            kind: "reply".into(),
            channel_id: "general".into(),
            seq: 3,
            root_seq: 1,
            message: reply,
            ..ChatDelta::default()
        };
        let thread = apply_chat_thread(vec![root], delta, "general".into(), 1);
        assert_eq!(thread[0].reply_count, 2, "the rail's replies rule reads this");
        assert!(thread.iter().any(|message| message.seq == 3));
    }

    #[test]
    fn a_reader_sees_their_own_writing_as_you() {
        let me = vec![0xab; 32];
        let mine = format!("user:{}", hex_encode(&me));
        let theirs = format!("user:{}", hex_encode(&[0xcd; 32]));

        assert_eq!(author_display(&mine, Some(&me)), "you");
        // The same row read by anyone else is still the handle.
        assert_eq!(author_display(&mine, Some(&[0xcd; 32])), author_name(&mine));
        assert_eq!(author_display(&theirs, Some(&me)), author_name(&theirs));
        // No local key (the boot race) renders nobody as `you`.
        assert_eq!(author_display(&mine, None), author_name(&mine));
        // An agent is never the reader.
        assert_eq!(
            author_display("agent:demo/quackbot", Some(&me)),
            "@quackbot"
        );
    }

    #[test]
    fn a_live_post_carries_the_block_it_was_applied_in() {
        // The block height is the only stamp a chat row has, and the op payload
        // does not carry it — a live-arriving message used to render with none
        // until a full reload re-read it from the index, which is what your own
        // message did the instant you sent it.
        let payload = serde_json::to_vec(&ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: "m1".into(),
            blocks: vec![Block::Paragraph(vec![Span {
                text: "hi".into(),
                marks: Vec::new(),
            }])],
            thread: None,
            as_agent: None,
        })
        .expect("a PostMessage encodes");
        let assigned = serde_json::json!({ "posted": { "seq": 7 } });
        let delta = delta_from_op(
            &payload,
            Some(&assigned),
            "external",
            Some("ext:ab"),
            None,
            276_199,
        )
        .expect("a well-formed op folds")
        .expect("a post is visible to the UI");

        assert_eq!(delta.kind, "posted");
        assert_eq!(delta.message.height, 276_199);
    }

    #[test]
    fn a_dm_id_is_pair_symmetric_and_survives_the_namespace_rule() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let id = dm_channel_id(&a, &b);
        assert_eq!(id, dm_channel_id(&b, &a));
        assert_ne!(id, dm_channel_id(&a, &a));

        // the round trip that matters: the DM is created by a USER author, and
        // this is the rule that author faces. A ':' here would reject every DM.
        crate::Chat::validate_channel_namespace(&AuthorRef::User(vec![0xab; 32]), &id)
            .expect("a user-authored DM channel id must pass the namespace rule");
    }

    #[test]
    fn parse_message_maps_markdown_onto_wire_blocks() {
        let input = "Hello **world** and *friends*\n\n> quote me\n> across lines\n\n```rust\nfn main() {}\n```\n\nvisit https://ducktape.example for more";
        let blocks = parse_message(input);
        assert_eq!(blocks.len(), 5);

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

        // a quote keeps its lines apart, exactly as a paragraph does — the
        // `⇧↵ newline` the composer advertises means the same thing inside a
        // `>` run as outside one.
        let Block::Quote(first) = &blocks[1] else {
            panic!("second block is a quote");
        };
        let Block::Quote(second) = &blocks[2] else {
            panic!("third block is a quote");
        };
        assert_eq!(span_text(first), "quote me");
        assert_eq!(span_text(second), "across lines");

        // fenced code keeps its language + raw text
        let Block::Code { lang, text } = &blocks[3] else {
            panic!("fourth block is code");
        };
        assert_eq!(lang.as_deref(), Some("rust"));
        assert_eq!(text, "fn main() {}");

        // bare url becomes a link mark
        let Block::Paragraph(spans) = &blocks[4] else {
            panic!("fifth block is a paragraph");
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
        assert_eq!(view[3].kind, "code");
        assert_eq!(view[3].text, "fn main() {}");

        // a plain message stays a single non-rich paragraph
        let plain = blocks_view(&parse_message("just text here"));
        assert_eq!(plain.len(), 1);
        assert!(!plain[0].rich);
        assert_eq!(plain[0].text, "just text here");
    }

    /// `⇧↵` PUTS A NEWLINE IN THE BUFFER AND THE COMPOSER SAYS SO.
    ///
    /// Consecutive lines were folded into one paragraph with a space, so a
    /// typed list posted as "- apples - bananas - pears" — and the fold happens
    /// on the way to the CHAIN, so no renderer recovers it. A rendered break
    /// has to be a block boundary: a marked-up line renders as a wrapping flex
    /// of word tokens, and a flex cannot force a line break inside one.
    #[test]
    fn a_single_newline_survives_the_trip_to_the_wire() {
        let blocks = parse_message("- apples\n- bananas\n- pears");
        assert_eq!(blocks.len(), 3, "one block per typed line");
        let lines: Vec<String> = blocks
            .iter()
            .map(|block| match block {
                Block::Paragraph(spans) => span_text(spans),
                other => panic!("every line is a paragraph, got {other:?}"),
            })
            .collect();
        assert_eq!(lines, ["- apples", "- bananas", "- pears"]);

        // The break survives inline marks, which is the case the old fold could
        // not have been fixed for on the render side.
        let marked = parse_message("**ship it**\nhttps://ducktape.example");
        assert_eq!(marked.len(), 2);
        let view = blocks_view(&marked);
        assert!(view[0].rich && view[1].rich);

        // And the edit draft the reader gets back is the text she typed, not
        // her text with a blank line inserted between every pair of lines.
        assert_eq!(message_body(&blocks), "- apples\n- bananas\n- pears");
    }

    #[test]
    fn mentions_resolve_against_the_member_roster() {
        let members = vec![
            ChatMember {
                key: "a1b2c3d4e5f6".into(),
                label: "a1b2c3d4…".into(),
            },
            ChatMember {
                key: "zoe".into(),
                label: "zoe".into(),
            },
        ];
        let blocks = parse_message_with_members("ping @a1b2 about the deploy", &members);
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("paragraph expected");
        };
        let mention = spans
            .iter()
            .find(|span| span.marks.iter().any(|m| matches!(m, Mark::Mention(_))))
            .expect("a mention span");
        assert_eq!(mention.text, "@a1b2");
        let Mark::Mention(AuthorRef::User(bytes)) = &mention.marks[0] else {
            panic!("user mention expected");
        };
        assert_eq!(bytes, &vec![0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6]);

        // a printable (non-hex) identity keeps its own bytes
        let blocks = parse_message_with_members("hey @zoe1 no — @zoea", &members);
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("paragraph expected");
        };
        // "@zoe1" prefix-matches nothing ("zoe" is shorter than the needle);
        // four-char rule also keeps short "@zoe" plain.
        assert!(
            spans
                .iter()
                .all(|span| span.marks.iter().all(|m| !matches!(m, Mark::Mention(_))))
        );

        // an unknown @word stays plain text
        let blocks = parse_message_with_members("email @someone", &members);
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("paragraph expected");
        };
        assert!(spans.iter().all(|span| span.marks.is_empty()));

        // rendered mentions highlight
        let view = blocks_view(&parse_message_with_members("cc @a1b2c3", &members));
        assert!(view[0].rich);
        assert!(
            view[0]
                .spans
                .iter()
                .any(|span| span.highlight && span.text.starts_with("@a1b2c3"))
        );
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

    #[test]
    fn avatars_distinguish_humans_from_software_authors() {
        assert_eq!(avatar_kind("user:deadbeef"), "human");
        for author in ["agent:chat/reviewer", "module:forge", "system"] {
            assert_eq!(avatar_kind(author), "agent");
        }
    }

    #[test]
    fn plain_rich_spans_mark_inline_runs_and_stay_empty_for_plain_text() {
        let spans = plain_rich_spans("say **hi** to https://duck.example/x");
        let bold: Vec<_> = spans.iter().filter(|span| span.bold).collect();
        assert_eq!(bold.len(), 1);
        assert_eq!(bold[0].text.trim_end(), "hi");
        assert!(spans.iter().any(|span| span.highlight));
        assert!(spans.iter().all(|span| !span.text.contains("**")));

        assert!(plain_rich_spans("no marks here").is_empty());
        assert!(plain_rich_spans("**multi**\nline").is_empty());
    }
}
