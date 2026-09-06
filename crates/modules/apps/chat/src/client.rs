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

use sha2::{Digest, Sha256};

use crate::index::{self, MsgRow};
use crate::{Block, ChatAssigned, ChatMsg, Mark, Party, PostPolicy, Span, decode_msg};

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// Maximum number of roots materialized in one desktop chat window. The
/// archive remains cursor-queryable; this bounds only the hot read model that
/// every update and view build touches.
pub const CHAT_HOT_WINDOW_LIMIT: usize = 256;
/// One canonical root plus one reply page. Older replies remain queryable by
/// sequence cursor; keeping every page mounted made each live reply rebuild up
/// to 4,097 rich rows on the UI thread.
pub const THREAD_HOT_WINDOW_LIMIT: usize = CHAT_HOT_WINDOW_LIMIT + 1;

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
    /// Numeric identity for Ice's keyed virtual timeline. The language cannot
    /// key this column by the string message ID, so merges carry this value
    /// forward when a row with the same ID is replaced.
    pub view_key: i64,
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
    /// (crates/noded/src/index.rs stamps `consensus_time = height`) and unix
    /// MILLIS on a single-writer noded (bin/noded/src/main.rs). NEVER unix
    /// seconds: render it as a height, never as a wall clock. 0 while pending.
    pub time: i64,
    pub reactions: Vec<ChatReaction>,
    /// CLIENT-side render revision — the view's cheap lazy key beside `seq`
    /// (`lazy message by message.seq, message.render_rev`). Bumped by every
    /// in-place row mutation and SEEDED from the rendered-content hash at
    /// construction, so a wholesale replacement — a resync reloading a row
    /// with reactions the displayed copy never saw — moves the key with no
    /// in-place mutation having run. Identical content seeds identically,
    /// which is the case where keeping the cached subtree is correct.
    pub render_rev: i64,
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
            view_key: _,
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
            render_rev,
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
        render_rev.hash(state);
    }
}

impl Default for ChatMessage {
    fn default() -> Self {
        Self {
            id: String::new(),
            view_key: 0,
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
            render_rev: 0,
        }
    }
}

impl ChatMessage {
    /// The construction seed: a deterministic hash of the rendered content
    /// (exactly the fields the manual [`Hash`] covers, `render_rev` still at
    /// its zero default), so a replacement row carrying content the displayed
    /// copy never saw arrives with a moved key.
    fn seed_render_rev(mut self) -> Self {
        use std::hash::{Hash as _, Hasher as _};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        self.render_rev = i64::from_ne_bytes(hasher.finish().to_ne_bytes());
        self
    }

    /// One in-place row mutation = one bump; the keyed lazy repaints the row
    /// exactly when this (or `seq`) moves.
    fn bump_render_rev(&mut self) {
        self.render_rev = self.render_rev.wrapping_add(1);
    }
}

fn next_message_view_key() -> i64 {
    use std::sync::atomic::{AtomicI64, Ordering};

    static NEXT: AtomicI64 = AtomicI64::new(1);
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |key| {
        key.checked_add(1)
    })
    .expect("a process cannot render more than i64::MAX rows")
}

/// One rendered block of a message body. `kind` is `paragraph` | `code` |
/// `quote` | `divider`. Plain paragraphs/quotes carry their exact text in
/// `text` (`rich=false`); formatted ones carry run-level `spans` the view's
/// single rich-text paragraph renders (`rich=true`).
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct ChatBlock {
    pub kind: String,
    pub text: String,
    pub lang: String,
    pub rich: bool,
    pub spans: Vec<ChatSpan>,
}

/// One inline run of a rich paragraph/quote, pre-sorted into the style arm
/// the paragraph's span template renders it with. EXACTLY ONE of the text
/// fields is non-empty per span (`link` rides `link_text`): ice's rich-text
/// `for` expands a fixed span template per item with no conditionals, so the
/// arm choice has to be data — the view emits every arm for every run and an
/// empty span draws no glyphs. A run landing in two fields renders twice; a
/// run landing in none vanishes ([`span_arm`] owns the decision).
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct ChatSpan {
    pub mention: String,
    pub link_text: String,
    pub link: String,
    pub bold_italic: String,
    pub bold: String,
    pub italic: String,
    pub plain: String,
}

// ============================================================================
// the op-delta — one folded applied op, pre-rendered for state splicing
// ============================================================================

/// One folded chat op with exactly the payload its transition consumes. Rows
/// are built by the SAME renderers hydration uses ([`chat_message`] over a
/// [`MsgRow`]), so a folded row and a fetched row are indistinguishable.
#[derive(Clone, Debug, Hash, PartialEq)]
pub enum ChatDelta {
    ChannelCreated {
        channel: ChatChannel,
    },
    ChannelRenamed {
        channel_id: String,
        name: String,
    },
    ChannelArchived {
        channel_id: String,
        archived: bool,
    },
    Posted {
        channel_id: String,
        seq: i64,
        message: ChatMessage,
    },
    Reply {
        channel_id: String,
        seq: i64,
        root_seq: i64,
        message: ChatMessage,
    },
    Edited {
        channel_id: String,
        seq: i64,
        message: ChatMessage,
    },
    Deleted {
        channel_id: String,
        seq: i64,
    },
    Reaction {
        channel_id: String,
        seq: i64,
        emoji: String,
        added: bool,
        reactor: String,
        by_me: bool,
    },
    Membership {
        channel_id: String,
        added: bool,
        member: ChatMember,
    },
    /// A huddle change first produces this directive; the shell reloads the
    /// canonical row and replaces it with `ChannelUpdated` before publishing.
    ChannelRefresh {
        channel_id: String,
    },
    ChannelUpdated {
        channel_id: String,
        channel: ChatChannel,
    },
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
    _origin_kind: &str,
    _origin_id: Option<&str>,
    current_user: Option<&[u8]>,
    names: &AuthorNames,
    height: u64,
) -> Result<Option<ChatDelta>, String> {
    let msg = decode_msg(payload)?;
    let actor = decode_stamp(assigned)?.actor().clone();
    let delta = match msg {
        ChatMsg::CreateChannel {
            channel_id,
            name,
            post_policy,
        } => ChatDelta::ChannelCreated {
            channel: ChatChannel {
                id: channel_id,
                name,
                archived: false,
                members_only: post_policy == PostPolicy::MembersOnly,
                huddle_count: 0,
                head_seq: 0,
            },
        },
        ChatMsg::CreateDmChannel {
            counterpart: _,
            name,
        } => {
            let ChatAssigned::DmChannel { channel_id, .. } = decode_stamp(assigned)? else {
                return Err("applied CreateDmChannel carried a non-DmChannel stamp".into());
            };
            ChatDelta::ChannelCreated {
                channel: ChatChannel {
                    id: channel_id,
                    name,
                    archived: false,
                    members_only: true,
                    huddle_count: 0,
                    head_seq: 0,
                },
            }
        }
        ChatMsg::RenameChannel { channel_id, name } => {
            ChatDelta::ChannelRenamed { channel_id, name }
        }
        ChatMsg::SetChannelArchived {
            channel_id,
            archived,
        } => ChatDelta::ChannelArchived {
            channel_id,
            archived,
        },
        ChatMsg::PostMessage {
            channel_id,
            message_id,
            blocks: _,
            thread,
        } => {
            let ChatAssigned::Posted { seq, blocks, .. } = decode_stamp(assigned)? else {
                return Err("applied PostMessage carried a non-Posted stamp".into());
            };
            let row = MsgRow {
                channel_id: channel_id.clone(),
                seq,
                message_id,
                author: index::party_handle(&actor),
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
            let message = chat_message(row, current_user, names);
            match thread {
                Some(root_seq) => ChatDelta::Reply {
                    channel_id,
                    seq: number_i64(seq),
                    root_seq: number_i64(root_seq),
                    message,
                },
                None => ChatDelta::Posted {
                    channel_id,
                    seq: number_i64(seq),
                    message,
                },
            }
        }
        ChatMsg::EditMessage {
            channel_id,
            seq,
            blocks: _,
            base_rev,
        } => {
            let ChatAssigned::Edited { rev, blocks, .. } = decode_stamp(assigned)? else {
                return Err("applied EditMessage carried a non-Edited stamp".into());
            };
            let carrier = MsgRow {
                channel_id: channel_id.clone(),
                seq,
                message_id: String::new(),
                author: index::party_handle(&actor),
                // An edit's stamp is the ORIGINAL post's block, never the
                // edit's — and `merge_message_edit` copies only body/blocks/
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
            ChatDelta::Edited {
                channel_id,
                seq: number_i64(seq),
                message: chat_message(carrier, current_user, names),
            }
        }
        ChatMsg::DeleteMessage { channel_id, seq } => ChatDelta::Deleted {
            channel_id,
            seq: number_i64(seq),
        },
        ChatMsg::AddReaction {
            channel_id,
            seq,
            emoji,
        } => reaction_delta(
            channel_id,
            seq,
            emoji,
            true,
            decode_stamp(assigned)?.participant()?,
            current_user,
            names,
        ),
        ChatMsg::RemoveReaction {
            channel_id,
            seq,
            emoji,
        } => reaction_delta(
            channel_id,
            seq,
            emoji,
            false,
            decode_stamp(assigned)?.participant()?,
            current_user,
            names,
        ),
        ChatMsg::RegisterHook { .. } | ChatMsg::UnregisterHook { .. } => return Ok(None),
        ChatMsg::SetMembership {
            channel_id,
            party,
            member,
        } => {
            let id = index::party_handle(&party);
            ChatDelta::Membership {
                channel_id,
                added: member,
                member: ChatMember {
                    label: short_label(&id),
                    key: id,
                },
            }
        }
        ChatMsg::JoinHuddle { channel_id, .. }
        | ChatMsg::LeaveHuddle { channel_id }
        | ChatMsg::SweepHuddle { channel_id, .. } => ChatDelta::ChannelRefresh { channel_id },
    };
    Ok(Some(delta))
}

fn reaction_delta(
    channel_id: String,
    seq: u64,
    emoji: String,
    added: bool,
    actor: &Party,
    current_user: Option<&[u8]>,
    names: &AuthorNames,
) -> ChatDelta {
    let reactor = index::party_handle(actor);
    let by_me = current_user.is_some_and(|key| names.owns_handle(&reactor, key));
    ChatDelta::Reaction {
        channel_id,
        seq: number_i64(seq),
        emoji,
        added,
        reactor,
        by_me,
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

pub fn insert_channel(mut channels: Vec<ChatChannel>, channel: ChatChannel) -> Vec<ChatChannel> {
    let exists = channels.iter().any(|current| current.id == channel.id);
    if !exists {
        channels.push(channel);
    }
    channels
}

pub fn replace_channel(
    mut channels: Vec<ChatChannel>,
    channel_id: &str,
    channel: ChatChannel,
) -> Vec<ChatChannel> {
    match channels.iter_mut().find(|current| current.id == channel_id) {
        Some(current) => *current = channel,
        None => channels.push(channel),
    }
    channels
}

pub fn rename_channel(
    mut channels: Vec<ChatChannel>,
    channel_id: &str,
    name: String,
) -> Vec<ChatChannel> {
    if let Some(channel) = channels.iter_mut().find(|channel| channel.id == channel_id) {
        channel.name = name;
    }
    channels
}

pub fn archive_channel(
    mut channels: Vec<ChatChannel>,
    channel_id: &str,
    archived: bool,
) -> Vec<ChatChannel> {
    if let Some(channel) = channels.iter_mut().find(|channel| channel.id == channel_id) {
        channel.archived = archived;
    }
    channels
}

pub fn advance_channel_head(
    mut channels: Vec<ChatChannel>,
    channel_id: &str,
    seq: i64,
) -> Vec<ChatChannel> {
    if let Some(channel) = channels.iter_mut().find(|channel| channel.id == channel_id) {
        channel.head_seq = channel.head_seq.max(seq);
    }
    channels
}

pub fn apply_membership(
    mut members: Vec<ChatMember>,
    added: bool,
    member: ChatMember,
) -> Vec<ChatMember> {
    members.retain(|current| current.key != member.key);
    if added {
        members.push(member);
    }
    members
}

/// A committed root row lands: remove its matching pending placeholder, then
/// insert it in canonical seq order while preserving the placeholder's stable
/// virtual key. Skips replies (the timeline is roots-only) and duplicate rows.
pub fn merge_posted_message(mut messages: Vec<ChatMessage>, row: ChatMessage) -> Vec<ChatMessage> {
    if row.thread_seq > 0 {
        return messages;
    }
    let pending_key = messages
        .iter()
        .find(|message| message.pending && message.id == row.id)
        .map(|message| message.view_key);
    if pending_key.is_some() {
        messages.retain(|message| !message.pending || message.id != row.id);
    }
    let already_present = messages
        .iter()
        .any(|message| !message.pending && message.seq == row.seq);
    if already_present {
        return messages;
    }
    let mut row = row;
    if let Some(view_key) = pending_key {
        row.view_key = view_key;
    }
    // committed rows stay seq-sorted; pending rows tail the list.
    let insert_at = messages
        .iter()
        .position(|message| message.pending || message.seq > row.seq)
        .unwrap_or(messages.len());
    messages.insert(insert_at, row);
    mark_message_groups(&mut messages);
    bounded_chat_window(messages)
}

pub fn bump_reply_summary(mut messages: Vec<ChatMessage>, root_seq: i64) -> Vec<ChatMessage> {
    if let Some(root) = messages
        .iter_mut()
        .find(|message| !message.pending && message.seq == root_seq)
    {
        // not replay-proof on its own (the row carries no last-reply
        // high-water); the stream delivers each op once per cursor, and a
        // reconnect runs the ready-resync which reloads the canonical count.
        root.reply_count += 1;
        root.bump_render_rev();
    }
    messages
}

/// Copy an edit's content fields onto the target row, keeping identity fields
/// (author, reactions, reply summary) intact. An older or replayed revision
/// applies as a no-op.
pub fn merge_message_edit(
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
            row.bump_render_rev();
        }
    }
    messages
}

/// The canonical tombstone shape, exactly as a hydrated deleted row renders.
pub fn tombstone_message(mut messages: Vec<ChatMessage>, seq: i64) -> Vec<ChatMessage> {
    if let Some(row) = messages
        .iter_mut()
        .find(|message| !message.pending && message.seq == seq)
    {
        row.deleted = true;
        row.body = "Message deleted".into();
        row.blocks = vec![deleted_block()];
        row.reactions = Vec::new();
        row.bump_render_rev();
    }
    mark_message_groups(&mut messages);
    messages
}

/// Reactor-set semantics, mirroring the index fold: a reactor appears at most
/// once per emoji, so replayed or double-submitted reactions cannot drift the
/// count.
pub fn merge_message_reaction(
    mut messages: Vec<ChatMessage>,
    seq: i64,
    emoji: &str,
    added: bool,
    reactor: &str,
    by_me: bool,
) -> Vec<ChatMessage> {
    let Some(row) = messages
        .iter_mut()
        .find(|message| !message.pending && message.seq == seq)
    else {
        return messages;
    };
    if row.deleted {
        return messages;
    }
    match row
        .reactions
        .iter_mut()
        .find(|reaction| reaction.emoji == emoji)
    {
        Some(reaction) => {
            reaction.reactors.retain(|current| current != reactor);
            if added {
                reaction.reactors.push(reactor.into());
            }
            reaction.count = count_i64(reaction.reactors.len());
            reaction.reacted_by_me = match by_me {
                true => added,
                false => reaction.reacted_by_me,
            };
        }
        None if added => row.reactions.push(ChatReaction {
            emoji: emoji.into(),
            count: 1,
            reacted_by_me: by_me,
            reactors: vec![reactor.into()],
        }),
        // a remove of an emoji the row never had touched nothing: no rescan,
        // no bump.
        None => return messages,
    }
    row.reactions.retain(|reaction| reaction.count > 0);
    row.bump_render_rev();
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
    merge_message_reaction(messages, seq, &emoji, added, &reactor, true)
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
    names: &AuthorNames,
) -> Vec<ChatMessage> {
    let blocks = paragraph_blocks(&body);
    let handle = current_user.map(|key| format!("user:{}", hex_encode(key)));
    let (author, initial) = match handle.as_deref() {
        Some(handle) => (
            author_display(handle, current_user, names),
            avatar_initial(handle, names),
        ),
        None => ("you".into(), "•".into()),
    };
    // Pending sequences remain descending negatives for the existing numeric
    // guards and ordering. Each concurrent placeholder must be unique inside
    // the keyed virtual timeline.
    let next_pending_seq = messages
        .iter()
        .map(|message| message.seq)
        .min()
        .unwrap_or_default()
        .min(0)
        .saturating_sub(1);
    let view_key = next_message_view_key();
    messages.push(
        ChatMessage {
            id: message_id,
            view_key,
            seq: next_pending_seq,
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
            render_rev: 0,
        }
        .seed_render_rev(),
    );
    messages
}

/// Keep one bounded, ordered render window. Committed roots are evicted from
/// the oldest edge first; optimistic rows stay at the tail and are evicted
/// only in the degenerate case where more than the whole window is in flight.
/// A later canonical settle still enters by seq even when its placeholder was
/// outside the retained window.
pub fn bounded_chat_window(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    if messages.len() <= CHAT_HOT_WINDOW_LIMIT {
        return messages;
    }
    let (mut pending, mut committed): (Vec<_>, Vec<_>) =
        messages.into_iter().partition(|message| message.pending);
    let pending_limit = if committed.is_empty() {
        CHAT_HOT_WINDOW_LIMIT
    } else {
        CHAT_HOT_WINDOW_LIMIT - 1
    };
    if pending.len() > pending_limit {
        pending.drain(..pending.len() - pending_limit);
    }
    let committed_limit = CHAT_HOT_WINDOW_LIMIT - pending.len();
    if committed.len() > committed_limit {
        committed.drain(..committed.len() - committed_limit);
    }
    committed.extend(pending);
    mark_message_groups(&mut committed);
    committed
}

/// Keep the thread root and one sliding reply page, with the replies' author
/// runs marked. Pending replies live at the newest edge and displace the
/// oldest committed replies before they are ever discarded themselves. The
/// root is held out of the marking pass: it renders as its own divided block,
/// so the first reply always opens a run.
pub fn bounded_thread_window(mut messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let root_index = messages
        .iter()
        .position(|message| !message.pending && message.seq > 0 && message.thread_seq == 0);
    let root = root_index.map(|index| messages.remove(index));
    let reply_limit = THREAD_HOT_WINDOW_LIMIT - usize::from(root.is_some());
    let (mut pending, mut committed): (Vec<_>, Vec<_>) =
        messages.into_iter().partition(|message| message.pending);
    if pending.len() > reply_limit {
        pending.drain(..pending.len() - reply_limit);
    }
    let committed_limit = reply_limit - pending.len();
    if committed.len() > committed_limit {
        committed.drain(..committed.len() - committed_limit);
    }
    committed.extend(pending);
    mark_message_groups(&mut committed);
    root.into_iter().chain(committed).collect()
}

fn merge_pending_rows(
    mut canonical: Vec<ChatMessage>,
    current: Vec<ChatMessage>,
    current_channel: String,
    next_channel: String,
) -> Vec<ChatMessage> {
    if current_channel != next_channel {
        return canonical;
    }
    retain_client_row_identity(&mut canonical, &current);
    let canonical_ids = canonical
        .iter()
        .map(|message| message.id.clone())
        .collect::<BTreeSet<_>>();
    canonical.extend(
        current
            .into_iter()
            .filter(|message| message.pending && !canonical_ids.contains(&message.id)),
    );
    canonical
}

/// Install one root timeline snapshot and keep its render window bounded.
pub fn merge_pending_messages(
    canonical: Vec<ChatMessage>,
    current: Vec<ChatMessage>,
    current_channel: String,
    next_channel: String,
) -> Vec<ChatMessage> {
    bounded_chat_window(merge_pending_rows(
        canonical,
        current,
        current_channel,
        next_channel,
    ))
}

/// Install a room window without losing committed rows that arrived after the
/// read began. Navigation clears the previous room before launching the read,
/// so any same-room committed row in `current` is newer live traffic, not
/// stale cache state.
pub fn merge_landing_messages(
    canonical: Vec<ChatMessage>,
    current: Vec<ChatMessage>,
    current_channel: String,
    next_channel: String,
) -> Vec<ChatMessage> {
    let mut merged = merge_message_send_result(canonical, current, current_channel, next_channel);
    mark_message_groups(&mut merged);
    bounded_chat_window(merged)
}

/// Refresh the canonical prefix of an open thread without discarding pages the
/// reader already loaded. The refresh query returns one page; the pagination
/// cursor and the rest of the mounted rail remain owned by the UI state.
pub fn merge_thread_refresh(
    canonical: Vec<ChatMessage>,
    current: Vec<ChatMessage>,
    current_channel: String,
    next_channel: String,
) -> Vec<ChatMessage> {
    bounded_thread_window(merge_message_send_result(
        canonical,
        current,
        current_channel,
        next_channel,
    ))
}

pub fn merge_message_send_result(
    mut canonical: Vec<ChatMessage>,
    current: Vec<ChatMessage>,
    current_channel: String,
    next_channel: String,
) -> Vec<ChatMessage> {
    if current_channel != next_channel {
        return canonical;
    }
    retain_client_row_identity(&mut canonical, &current);
    let canonical_ids = canonical
        .iter()
        .map(|message| message.id.clone())
        .collect::<BTreeSet<_>>();
    let (pending, committed): (Vec<_>, Vec<_>) = current
        .into_iter()
        .partition(|message| message.pending || message.seq <= 0);
    let mut committed = committed
        .into_iter()
        .map(|message| (message.seq, message))
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
    merged.extend(
        pending
            .into_iter()
            .filter(|message| !canonical_ids.contains(&message.id)),
    );
    merged
}

fn retain_client_row_identity(canonical: &mut [ChatMessage], current: &[ChatMessage]) {
    let current_by_id = current
        .iter()
        .map(|message| (message.id.as_str(), message.view_key))
        .collect::<BTreeMap<_, _>>();
    for message in canonical {
        let Some(view_key) = current_by_id.get(message.id.as_str()) else {
            continue;
        };
        message.view_key = *view_key;
    }
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
    bounded_thread_window(merge_message_send_result(
        next,
        messages,
        String::new(),
        String::new(),
    ))
}

pub fn merge_thread_reply(
    mut messages: Vec<ChatMessage>,
    mut reply: ChatMessage,
) -> Vec<ChatMessage> {
    let pending_key = messages
        .iter()
        .find(|message| message.pending && message.id == reply.id)
        .map(|message| message.view_key);
    if pending_key.is_some() {
        messages.retain(|message| !message.pending || message.id != reply.id);
    }
    let existing = messages
        .iter()
        .position(|message| !message.pending && message.seq == reply.seq);
    if let Some(index) = existing {
        if messages[index].rev <= reply.rev {
            reply.view_key = messages[index].view_key;
            messages[index] = reply;
        }
        // still through the window fold: a replace that flips `deleted`
        // re-breaks the author runs around it.
        return bounded_thread_window(messages);
    }
    if let Some(view_key) = pending_key {
        reply.view_key = view_key;
    }
    let insert_at = messages
        .iter()
        .position(|message| message.pending || message.seq > reply.seq)
        .unwrap_or(messages.len());
    messages.insert(insert_at, reply);
    bounded_thread_window(messages)
}

// ============================================================================
// row rendering — MsgRow (the index/feed shape) → the rendered ChatMessage
// ============================================================================

pub fn chat_message(row: MsgRow, current_user: Option<&[u8]>, names: &AuthorNames) -> ChatMessage {
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
    let view_key = next_message_view_key();
    ChatMessage {
        id: row.message_id,
        view_key,
        seq: number_i64(row.seq),
        author: author_display(&row.author, current_user, names),
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
        initial: avatar_initial(&row.author, names),
        avatar_kind: avatar_kind(&row.author).into(),
        height: number_i64(row.height),
        time: number_i64(row.time),
        reactions: row
            .reactions
            .into_iter()
            .map(|reaction| {
                let reacted_by_me = reacted_by_user(&reaction.reactors, current_user, names);
                ChatReaction {
                    emoji: reaction.emoji,
                    count: count_i64(reaction.reactors.len()),
                    reacted_by_me,
                    reactors: reaction.reactors,
                }
            })
            .collect(),
        render_rev: 0,
    }
    .seed_render_rev()
}

/// True when a reactor is the reader's account or their exact signed key.
fn reacted_by_user(reactors: &[String], current_user: Option<&[u8]>, names: &AuthorNames) -> bool {
    current_user.is_some_and(|key| reactors.iter().any(|handle| names.owns_handle(handle, key)))
}

/// Account authorship and the reader's own historic key authorship both
/// belong to the reader; another key's old records never transfer with it.
fn authored_by_user(author: &str, current_user: Option<&[u8]>, names: &AuthorNames) -> bool {
    current_user.is_some_and(|key| names.owns_handle(author, key))
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
        // flip-only: this re-runs on every merge, and an unmoved header must
        // not move the row's render key.
        let flipped = message.show_author != show;
        if flipped {
            message.show_author = show;
            message.bump_render_rev();
        }
    }
}

/// Flatten wire blocks back into composer text — the seed for an edit draft.
///
/// One `\n` per block boundary, because that is what a block boundary now MEANS
/// in the composer (`parse_message_with_mentions` makes every typed line its own
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

/// The optimistic row's render blocks: the SAME grammar the send commits
/// ([`parse_message`]), so a pending row previews what will land instead of
/// showing raw `**marks**` until the settle replaces it. Roster mentions are
/// the one divergence — they need the channel members the send resolves.
pub fn paragraph_blocks(text: &str) -> Vec<ChatBlock> {
    blocks_view(&parse_message(text))
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
/// wrapping `text`; any inline mark switches to run-level `spans` the view's
/// single rich-text paragraph expands with its `for`.
fn rich_block(kind: &str, spans: &[Span]) -> ChatBlock {
    let marked = spans.iter().any(|span| !span.marks.is_empty());
    ChatBlock {
        kind: kind.into(),
        text: span_text(spans),
        lang: String::new(),
        rich: marked,
        spans: if marked { run_spans(spans) } else { Vec::new() },
    }
}

/// The one style arm a run renders through. The view's rich-text `for`
/// expands a fixed span template per item with no conditionals, so the arm
/// decision cannot live view-side — it is made here, once, and encoded as
/// WHICH [`ChatSpan`] text field carries the run.
enum SpanArm {
    Link(String),
    Mention,
    BoldItalic,
    Bold,
    Italic,
    Plain,
}

/// A link outranks every other mark (a bold link is still a destination), a
/// mention outranks emphasis, and emphasis resolves on the (bold, italic)
/// pair — the same precedence the per-token view arms encoded.
fn span_arm(span: &Span) -> SpanArm {
    let link = span.marks.iter().find_map(|mark| match mark {
        Mark::Link(url) => Some(url.clone()),
        _ => None,
    });
    if let Some(url) = link {
        return SpanArm::Link(url);
    }
    let mention = span.marks.iter().any(|m| matches!(m, Mark::Mention(_)));
    if mention {
        return SpanArm::Mention;
    }
    let bold = span.marks.iter().any(|m| matches!(m, Mark::Bold));
    let italic = span.marks.iter().any(|m| matches!(m, Mark::Italic));
    match (bold, italic) {
        (true, true) => SpanArm::BoldItalic,
        (true, false) => SpanArm::Bold,
        (false, true) => SpanArm::Italic,
        (false, false) => SpanArm::Plain,
    }
}

/// One [`ChatSpan`] per inline run, exact text preserved — the paragraph
/// widget wraps natively, so no word splitting happens here anymore.
fn run_spans(spans: &[Span]) -> Vec<ChatSpan> {
    let mut out = Vec::new();
    for span in spans {
        if span.text.is_empty() {
            continue;
        }
        let mut rendered = ChatSpan::default();
        match span_arm(span) {
            SpanArm::Link(url) => {
                rendered.link_text = span.text.clone();
                rendered.link = url;
            }
            SpanArm::Mention => rendered.mention = span.text.clone(),
            SpanArm::BoldItalic => rendered.bold_italic = span.text.clone(),
            SpanArm::Bold => rendered.bold = span.text.clone(),
            SpanArm::Italic => rendered.italic = span.text.clone(),
            SpanArm::Plain => rendered.plain = span.text.clone(),
        }
        out.push(rendered);
    }
    out
}

/// Rich runs of one block of text — the pages renderer's view of the chat
/// inline grammar (no roster, so mentions stay plain ink). Empty when the
/// text carries no inline mark, keeping the plain single-`text` render;
/// multi-line text stays plain because a rendered break is a block boundary,
/// never a `\n` inside one paragraph.
pub fn plain_rich_spans(text: &str) -> Vec<ChatSpan> {
    if text.contains('\n') {
        return Vec::new();
    }
    let spans = inline_spans(text, &MentionCandidates::default());
    let marked = spans.iter().any(|span| !span.marks.is_empty());
    if !marked {
        return Vec::new();
    }
    run_spans(&spans)
}

// ============================================================================
// authorship + avatars — display identity derived from rendered handles
// ============================================================================

/// A [`Party`] as the rendered handle the display fns parse — the same
/// vocabulary the index stamps (`user:{hex}`, `acct:{number}`,
/// `module:{id}`, `system`), so every module surface names an author
/// identically.
pub fn author_handle(author: &Party) -> String {
    index::party_handle(author)
}

/// The display name for a rendered author string (`user:{id}`,
/// `acct:{number}`, `module:{id}`, or `system`).
/// The author label a READER sees: `you` for the reader's own writing, the
/// rendered handle otherwise.
///
/// Every other surface in the shell already says `you` — the huddle roster, the
/// member roster — while the timeline printed the reader's own messages as
/// `user 3f8dc828…`, a hex nobody recognises as themselves. The plain
/// [`author_name`] stays for the places that render an author with no reader in
/// frame (a page comment's opener, a wire label).
pub fn author_display(author: &str, current_user: Option<&[u8]>, names: &AuthorNames) -> String {
    match authored_by_user(author, current_user, names) {
        true => "you".into(),
        false => author_name(author, names),
    }
}

/// Account names and verified key memberships used to render stable authors.
/// Program accounts have directory entries without keys. A key whose account
/// is unknown remains a key handle, so missing directory data cannot fabricate
/// membership or merge two people with the same display name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthorNames {
    by_key: BTreeMap<String, String>,
    by_account: BTreeMap<u64, String>,
    account_by_key: BTreeMap<String, u64>,
}

impl AuthorNames {
    /// `(member public key hex, account name)` pairs. Keys are lowercased on
    /// the way in, so a caller's hex casing cannot decide whether a name is
    /// found.
    pub fn new(entries: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            by_key: entries
                .into_iter()
                .map(|(key, name)| (key.to_ascii_lowercase(), name))
                .filter(|(key, name)| !key.is_empty() && !name.is_empty())
                .collect(),
            by_account: BTreeMap::new(),
            account_by_key: BTreeMap::new(),
        }
    }

    /// Authoritative account records supply stable handles and key membership.
    pub fn from_accounts<'a>(
        accounts: impl IntoIterator<Item = &'a identity::AccountView>,
    ) -> Self {
        let mut names = Self::default();
        for account in accounts {
            names
                .by_account
                .insert(account.number, account.name.clone());
            for key in &account.keys {
                let key = hex_encode(&key.pubkey);
                names.by_key.insert(key.clone(), account.name.clone());
                names.account_by_key.insert(key, account.number);
            }
        }
        names
    }

    pub fn party_of(&self, key: &[u8]) -> Party {
        match self.account_by_key.get(&hex_encode(key)) {
            Some(account) => Party::Account(*account),
            None => Party::Key(key.to_vec()),
        }
    }

    pub fn parties_of(&self, key: &[u8]) -> Vec<Party> {
        vec![self.party_of(key)]
    }

    pub fn owns_handle(&self, handle: &str, key: &[u8]) -> bool {
        let is_account = handle == self.handle_of(key);
        let is_original_key = handle == index::party_handle(&Party::Key(key.to_vec()));
        is_account || is_original_key
    }

    pub fn handle_of(&self, key: &[u8]) -> String {
        index::party_handle(&self.party_of(key))
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty() && self.by_account.is_empty()
    }

    /// The account name a raw key is registered under.
    pub fn name_of(&self, key: &[u8]) -> Option<&str> {
        self.by_key.get(&hex_encode(key)).map(String::as_str)
    }

    /// EVERY key of the account `name` names — an account is a set of keys
    /// (a phone and a laptop sign with different ones), so anything that has
    /// to REACH a person addresses all of them, not the one it happened to
    /// see. Case-insensitive on the name; ascending by key hex, so the answer
    /// is stable across two readings of the same directory.
    pub fn keys_named(&self, name: &str) -> Vec<Vec<u8>> {
        let wanted = name.to_ascii_lowercase();
        self.by_key
            .iter()
            .filter(|(_key, registered)| registered.to_ascii_lowercase() == wanted)
            .filter_map(|(key, _registered)| hex_bytes(key))
            .collect()
    }

    /// Every key of the account THIS key belongs to, the key itself included —
    /// the set a mention of that person must address. An unregistered key
    /// answers with itself alone: it is still a real identity, just an unnamed
    /// one.
    pub fn account_keys_of(&self, key: &[u8]) -> Vec<Vec<u8>> {
        let Some(account) = self.account_by_key.get(&hex_encode(key)) else {
            return vec![key.to_vec()];
        };
        self.account_by_key
            .iter()
            .filter(|(_, number)| *number == account)
            .filter_map(|(key, _)| hex_bytes(key))
            .collect()
    }

    /// `(account name, its keys)` for every account in the directory,
    /// ascending by name.
    fn accounts(&self) -> BTreeMap<String, Vec<Vec<u8>>> {
        let mut accounts: BTreeMap<String, Vec<Vec<u8>>> = BTreeMap::new();
        for (key, name) in &self.by_key {
            let Some(bytes) = hex_bytes(key) else {
                continue;
            };
            accounts
                .entry(name.to_ascii_lowercase())
                .or_default()
                .push(bytes);
        }
        accounts
    }

    /// The directory name behind either an account or signed-key handle.
    fn of_handle(&self, author: &str) -> Option<&str> {
        match author.split_once(':') {
            Some(("acct", number)) => self
                .by_account
                .get(&number.parse::<u64>().ok()?)
                .map(String::as_str),
            Some(("user", key)) => self
                .by_key
                .get(&key.to_ascii_lowercase())
                .map(String::as_str),
            _ => None,
        }
    }
}

pub fn author_name(author: &str, names: &AuthorNames) -> String {
    match names.of_handle(author) {
        Some(name) => name.to_string(),
        None => unnamed_author(author),
    }
}

/// The handle itself, rendered — what an author with no account entry reads as.
fn unnamed_author(author: &str) -> String {
    match author.split_once(':') {
        Some(("user", id)) => format!("user {}", short_label(id)),
        Some(("acct", account)) => format!("account {account}"),
        Some(("module", id)) => id.to_string(),
        _ => "system".into(),
    }
}

/// The stable identity an avatar is derived from: the shortened id for a user,
/// the agent/module name otherwise.
fn avatar_source(author: &str, names: &AuthorNames) -> String {
    if let Some(name) = names.of_handle(author) {
        return name.to_string();
    }
    match author.split_once(':') {
        Some(("user", id)) => short_label(id),
        Some(("acct", account)) => account.to_string(),
        Some(("module", id)) => id.to_string(),
        _ => "system".into(),
    }
}

/// The single-glyph avatar label for an author: the first alphanumeric character
/// of its identity, uppercased. Falls back to a neutral dot when there is
/// nothing to show.
fn avatar_initial(author: &str, names: &AuthorNames) -> String {
    initial_of(&avatar_source(author, names))
}

fn avatar_kind(author: &str) -> &'static str {
    match author.split_once(':') {
        Some(("user" | "acct", _)) => "human",
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

/// An even-length all-hex string back to its bytes; anything else is not hex.
fn hex_bytes(hex: &str) -> Option<Vec<u8>> {
    let looks_hex = !hex.is_empty()
        && hex.len().is_multiple_of(2)
        && hex.bytes().all(|b| b.is_ascii_hexdigit());
    if !looks_hex {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).ok())
        .collect()
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
/// a marked-up line renders as a single rich-text paragraph (`run_spans`),
/// one paragraph widget per typed line.
pub fn parse_message(input: &str) -> Vec<Block> {
    parse_message_with_mentions(input, &MentionCandidates::default())
}

/// [`parse_message`] with `@mention` resolution against [`MentionCandidates`]:
/// a resolved `@handle` becomes a [`Mark::Mention`] span, which the module
/// fans out to hooks (inbox notifications) on apply. Non-matching `@word`s
/// stay plain text.
pub fn parse_message_with_mentions(input: &str, mentions: &MentionCandidates) -> Vec<Block> {
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
            index = push_quote_block(&lines, index, mentions, &mut blocks);
        } else if is_blank {
            index += 1;
        } else {
            index = push_paragraph_block(&lines, index, mentions, &mut blocks);
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
    mentions: &MentionCandidates,
    blocks: &mut Vec<Block>,
) -> usize {
    let mut index = start;
    while index < lines.len() && lines[index].trim().starts_with('>') {
        let stripped = lines[index].trim().trim_start_matches('>').trim_start();
        blocks.push(Block::Quote(inline_spans(stripped, mentions)));
        index += 1;
    }
    index
}

fn push_paragraph_block(
    lines: &[&str],
    start: usize,
    mentions: &MentionCandidates,
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
        blocks.push(Block::Paragraph(inline_spans(trimmed, mentions)));
        index += 1;
    }
    index
}

/// Scan a single line of text for inline marks, emitting marked `Span`s. Marks do
/// not nest; the first matching delimiter wins. Bare `http(s)://` and `duck://`
/// runs become `Link`s, as does a `[label](url)` reference — one span whose
/// text is the label and whose mark carries the target.
fn inline_spans(text: &str, mentions: &MentionCandidates) -> Vec<Span> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans: Vec<Span> = Vec::new();
    let mut plain = String::new();
    let mut index = 0;
    while index < chars.len() {
        let url = url_len(&chars, index);
        let reference = reference_at(&chars, index);
        let bold = fenced(&chars, index, "**").or_else(|| fenced(&chars, index, "__"));
        let italic = fenced(&chars, index, "*").or_else(|| fenced(&chars, index, "_"));
        if let Some((target, len)) = mention_at(&chars, index, mentions) {
            flush_plain(&mut plain, &mut spans);
            let handle: String = chars[index..index + len].iter().collect();
            spans.push(Span {
                text: handle,
                // A directory account is one mark, regardless of its keys.
                marks: target.parties.iter().cloned().map(Mark::Mention).collect(),
            });
            index += len;
        } else if let Some((label, target, len)) = reference {
            flush_plain(&mut plain, &mut spans);
            spans.push(Span {
                text: label,
                marks: vec![Mark::Link(target)],
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
///
/// `duck://` is a link scheme here exactly as `http(s)://` is: the app
/// classifies a pressed link through its own module table
/// (`backend/duck_uri.rs`) and refuses what it cannot open, so the tokenizer
/// marks the run and decides nothing about where it points.
fn url_len(chars: &[char], at: usize) -> Option<usize> {
    let rest: String = chars[at..].iter().collect();
    let starts_link = LINK_SCHEMES.iter().any(|scheme| rest.starts_with(scheme));
    if !starts_link {
        return None;
    }
    let mut len = chars[at..]
        .iter()
        .take_while(|c| !c.is_whitespace())
        .count();
    // A run stops at whitespace, but the `)` that closes `[x](duck://page/p1)`
    // or `(see https://x)` belongs to the prose around the address, not to it.
    while dangling_close(&chars[at..at + len]) {
        len -= 1;
    }
    (len > 0).then_some(len)
}

/// Does this run end in a `)` that opens nowhere inside it? A balanced one
/// (`…/wiki/Foo_(bar)`) is part of the address and stays.
fn dangling_close(run: &[char]) -> bool {
    let closed = run.last() == Some(&')');
    let opens = run.iter().filter(|c| **c == '(').count();
    let closes = run.iter().filter(|c| **c == ')').count();
    closed && closes > opens
}

/// The schemes a bare run and a `[label](url)` target may carry. Anything
/// else stays plain text.
const LINK_SCHEMES: [&str; 3] = ["http://", "https://", "duck://"];

/// How many hex characters `mint_chain_id` puts after the `#`.
const CHAIN_DIGEST_HEX: usize = 8;

/// The chain id's hash half — `mint_chain_id` spells a chain id
/// `<name>#<8 hex>` and only the hex rides a URI. "" for an unnamed chain.
///
/// Split from the RIGHT: `node init --name` validates nothing, so a network
/// named `my#net` mints the chain id `my#net#a1b2c3d4`, and only the LAST `#`
/// is the minted separator.
pub fn chain_digest(chain_id: &str) -> &str {
    chain_id.rsplit_once('#').map(|(_, hex)| hex).unwrap_or("")
}

/// The `?net=` a produced `duck://` link carries, or "" when the producer has
/// no chain id. THIS IS THE ONE PLACE THE QUERY IS SPELLED: the app's link
/// builders (`backend/duck_uri.rs`) and the in-consensus producer
/// (`runs::inject`) both go through here, so a produced link cannot carry a
/// second dialect of the half that makes a foreign-network refusal possible.
/// It lives beside the tokenizer that PARSES the form for the same reason.
pub fn duck_net_query(chain_id: &str) -> String {
    let digest = chain_digest(chain_id);
    match digest.is_empty() {
        true => String::new(),
        false => format!("?net={digest}"),
    }
}

/// Is `digest` a minted chain-id hash half — exactly [`CHAIN_DIGEST_HEX`]
/// lowercase hex? The reader's side of [`duck_net_query`].
pub fn is_chain_digest(digest: &str) -> bool {
    digest.len() == CHAIN_DIGEST_HEX
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// If `chars[at..]` opens a `[label](url)` reference — the form agents already
/// emit for `duck://` refs (`runs::inject`) — the label, the target, and the
/// total consumed length. The label is one line with no nested brackets and
/// the target is one whitespace-free run in a known scheme; anything else is
/// not a reference and stays the plain text it was typed as.
fn reference_at(chars: &[char], at: usize) -> Option<(String, String, usize)> {
    if chars[at] != '[' {
        return None;
    }
    let label_end = chars[at + 1..]
        .iter()
        .position(|c| *c == ']' || *c == '[')?
        + at
        + 1;
    let labelled = chars[label_end] == ']' && chars.get(label_end + 1) == Some(&'(');
    if !labelled {
        return None;
    }
    let url_start = label_end + 2;
    let url_end = chars[url_start..]
        .iter()
        .position(|c| *c == ')' || c.is_whitespace())?
        + url_start;
    let closed = chars[url_end] == ')';
    if !closed {
        return None;
    }
    let label: String = chars[at + 1..label_end].iter().collect();
    let target: String = chars[url_start..url_end].iter().collect();
    let linkable = !label.is_empty() && LINK_SCHEMES.iter().any(|s| target.starts_with(s));
    linkable.then(|| (label, target, url_end + 1 - at))
}

/// WHO A TYPED `@handle` CAN REACH: the network's ACCOUNT DIRECTORY, by name,
/// unioned with the channel's own member roster, by key.
///
/// The roster alone used to be the whole answer, and it is empty in every
/// open-policy channel — `#general` has no member rows, so `@orthory` in the
/// room everybody actually writes in resolved to nothing, no `Mark::Mention`
/// was minted, and the module's `collect_mentions` had nothing to fan out.
/// A person is nameable in EVERY room they can be read in, so the directory
/// is the candidate set and the roster only adds keys the directory has not
/// registered (an unnamed device, an invited key).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MentionCandidates {
    /// longest handle first, so a greedy scan settles `@orthory-ops` on the
    /// account of that name instead of on `@orthory`.
    targets: Vec<MentionTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MentionTarget {
    /// what the writer types after `@`, lowercased.
    pub handle: String,
    /// The stable account or explicitly named key this handle addresses.
    pub parties: Vec<Party>,
}

/// The shortest `@key` fragment that may stand for a member key. A name is
/// matched WHOLE, so this bounds only the hex-prefix form.
const MIN_KEY_PREFIX: usize = 4;

impl MentionCandidates {
    /// The directory (by account name) unioned with `members` (by key). A key
    /// the directory already names is not re-added under its hex.
    pub fn new(directory: &AuthorNames, members: &[ChatMember]) -> Self {
        let mut targets: Vec<MentionTarget> = directory
            .accounts()
            .into_iter()
            .map(|(handle, keys)| MentionTarget {
                handle,
                parties: keys
                    .into_iter()
                    .filter(|key| !directory.account_by_key.contains_key(&hex_encode(key)))
                    .map(Party::Key)
                    .collect(),
            })
            .filter(|target| !target.parties.is_empty())
            .collect();
        let mut counts = BTreeMap::<String, usize>::new();
        for name in directory.by_account.values() {
            *counts.entry(name.to_ascii_lowercase()).or_default() += 1;
        }
        for (account, name) in &directory.by_account {
            let name = name.to_ascii_lowercase();
            let ambiguous = name.is_empty() || counts.get(&name).copied().unwrap_or(0) > 1;
            let handle = if ambiguous {
                format!("account-{account}")
            } else {
                name
            };
            targets.push(MentionTarget {
                handle,
                parties: vec![Party::Account(*account)],
            });
        }
        for member in members {
            if let Some(number) = member
                .key
                .strip_prefix("acct:")
                .and_then(|id| id.parse::<u64>().ok())
            {
                if !directory.by_account.contains_key(&number) {
                    targets.push(MentionTarget {
                        handle: format!("account-{number}"),
                        parties: vec![Party::Account(number)],
                    });
                }
                continue;
            }
            let key = member_key_bytes(member);
            if directory.name_of(&key).is_some() {
                continue;
            }
            targets.push(MentionTarget {
                handle: member
                    .key
                    .strip_prefix("user:")
                    .unwrap_or(&member.key)
                    .to_ascii_lowercase(),
                parties: vec![Party::Key(key)],
            });
        }
        targets.sort_by(|left, right| {
            right
                .handle
                .len()
                .cmp(&left.handle.len())
                .then_with(|| left.handle.cmp(&right.handle))
        });
        Self { targets }
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Every handle a composer could offer, longest first — the autocomplete's
    /// source and the parser's.
    pub fn handles(&self) -> Vec<&str> {
        self.targets
            .iter()
            .map(|target| target.handle.as_str())
            .collect()
    }

    /// The target a typed word names: a WHOLE account name, else a key the
    /// word prefixes. Both case-insensitive.
    fn resolve(&self, word: &str) -> Option<&MentionTarget> {
        let needle = word.to_ascii_lowercase();
        let names_an_account = |target: &&MentionTarget| target.handle == needle;
        let prefixes_a_key = |target: &&MentionTarget| {
            needle.len() >= MIN_KEY_PREFIX && target.handle.starts_with(&needle)
        };
        self.targets
            .iter()
            .find(names_an_account)
            .or_else(|| self.targets.iter().find(prefixes_a_key))
    }
}

/// A character that may sit inside a typed handle. Account names carry `-`,
/// `_` and `.` (`orthory-ops` is one account), so the scan cannot stop at the
/// first non-alphanumeric — it takes them all and then backs off.
fn handle_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')
}

/// If `chars[at..]` opens an `@mention` — `@` plus a handle
/// [`MentionCandidates`] resolves — the matched target and the consumed
/// length (the `@` included).
///
/// LONGEST MATCH FIRST, then shed TRAILING PUNCTUATION one character at a
/// time: `@orthory-ops.` scans `orthory-ops.`, which names nobody, then
/// `orthory-ops`, which does. Only `-`, `_` and `.` are shed — backing off
/// through letters instead would let `@zoe1` mention `zoe` and leave a
/// stray `1`.
fn mention_at<'a>(
    chars: &[char],
    at: usize,
    mentions: &'a MentionCandidates,
) -> Option<(&'a MentionTarget, usize)> {
    let opens = chars[at] == '@' && (at == 0 || chars[at - 1].is_whitespace());
    if !opens || mentions.is_empty() {
        return None;
    }
    let mut word: Vec<char> = chars[at + 1..]
        .iter()
        .copied()
        .take_while(|c| handle_char(*c))
        .collect();
    loop {
        let candidate: String = word.iter().collect();
        if let Some(target) = mentions.resolve(&candidate) {
            return Some((target, 1 + word.len()));
        }
        let sheds = word
            .last()
            .is_some_and(|last| !last.is_ascii_alphanumeric());
        if !sheds {
            return None;
        }
        word.pop();
    }
}

/// True when any `Mark::Mention` in `blocks` addresses one of `keys` — the
/// reader's own account keys, so "was I mentioned?" is answered for the
/// PERSON and not for the one device that happens to be signing here.
pub fn mentions_reach(blocks: &[Block], parties: &[Party]) -> bool {
    let addressed = |mark: &Mark| match mark {
        Mark::Mention(party) => parties.contains(party),
        Mark::Bold | Mark::Italic | Mark::Link(_) => false,
    };
    blocks.iter().any(|block| {
        let spans = match block {
            Block::Paragraph(spans) | Block::Quote(spans) => spans,
            Block::Code { .. } | Block::Divider => return false,
        };
        spans.iter().any(|span| span.marks.iter().any(addressed))
    })
}

/// A member key back to `Party::Key` bytes: keys are rendered by
/// [`user_handle`] — printable identities pass through verbatim, raw key
/// bytes arrive hex-encoded — so an even-length all-hex key decodes, and
/// anything else is the identity's own bytes.
fn member_key_bytes(member: &ChatMember) -> Vec<u8> {
    let key = member.key.strip_prefix("user:").unwrap_or(&member.key);
    hex_bytes(key).unwrap_or_else(|| key.as_bytes().to_vec())
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
    let mut id = String::from(DM_CHANNEL_PREFIX);
    for byte in digest.finalize() {
        let _ = write!(id, "{byte:02x}");
    }
    id
}

/// What every derived two-party room id opens with.
pub const DM_CHANNEL_PREFIX: &str = "dm-";

/// The byte length of the SHA-256 digest [`dm_channel_id`] renders as hex.
const DM_CHANNEL_DIGEST_HEX: usize = 64;

/// True when `id` has the exact SHAPE [`dm_channel_id`] mints — the prefix and
/// a full lowercase-hex digest.
///
/// A DM record is a NETWORK-VISIBLE channel row: the chat index serves every
/// member every channel, so a DM between two other people arrives in this
/// device's channel list like any room. Recognising the shape is what lets the
/// sidebar drop the ones that are none of this reader's business. It is a
/// shape test and not a `starts_with("dm-")` test on purpose: a person may
/// legitimately name a channel `dm-standup`, and that room is a room.
pub fn is_derived_dm_channel(id: &str) -> bool {
    let Some(digest) = id.strip_prefix(DM_CHANNEL_PREFIX) else {
        return false;
    };
    digest.len() == DM_CHANNEL_DIGEST_HEX
        && digest.bytes().all(|byte| {
            byte.is_ascii_digit() || byte.is_ascii_lowercase() && byte.is_ascii_hexdigit()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn committed(seq: i64, author: &str) -> ChatMessage {
        ChatMessage {
            seq,
            id: format!("g{seq}"),
            author: author.into(),
            ..ChatMessage::default()
        }
    }

    #[test]
    fn account_directory_names_programs_and_unifies_a_readers_keys() {
        let account = |number, name: &str, keys: Vec<Vec<u8>>| identity::AccountView {
            number,
            name: name.into(),
            control: identity::Control::Keys,
            keys: keys
                .into_iter()
                .map(|pubkey| identity::KeyView {
                    scheme: identity::KeyScheme::Ed25519,
                    pubkey,
                    label: None,
                    added_at: 0,
                })
                .collect(),
            avatar: None,
            bio: None,
            updated_at: 0,
        };
        let human = account(1, "same", vec![vec![1; 32], vec![2; 32]]);
        let mut program = account(2, "bot", Vec::new());
        program.control = identity::Control::Program {
            controller: 1,
            executor: "agent".into(),
            generation: 0,
            standing: identity::ProgramStanding::Active,
        };
        let names = AuthorNames::from_accounts([&human, &program]);
        assert_eq!(author_display("acct:1", Some(&[2; 32]), &names), "you");
        assert_eq!(author_name("acct:2", &names), "bot");
        let blocks = parse_message_with_mentions("ping @bot", &MentionCandidates::new(&names, &[]));
        assert!(mentions_reach(&blocks, &[Party::Account(2)]));
        let collision = account(3, "same", vec![vec![3; 32]]);
        let names = AuthorNames::from_accounts([&human, &collision]);
        let candidates = MentionCandidates::new(&names, &[]);
        assert!(candidates.handles().contains(&"account-1"));
        assert!(candidates.handles().contains(&"account-3"));
        assert!(!candidates.handles().contains(&"same"));
    }

    #[test]
    fn a_thread_window_keeps_its_root_pending_tail_and_one_reply_page() {
        let root = committed(1, "alice");
        let replies = (2..=300).map(|seq| ChatMessage {
            thread_seq: 1,
            ..committed(seq, "alice")
        });
        let pending = ChatMessage {
            id: "pending".into(),
            seq: -1,
            pending: true,
            thread_seq: 0,
            author: "alice".into(),
            ..ChatMessage::default()
        };
        let window = bounded_thread_window(
            std::iter::once(root)
                .chain(replies)
                .chain(std::iter::once(pending))
                .collect(),
        );

        assert_eq!(window.len(), THREAD_HOT_WINDOW_LIMIT);
        assert_eq!(window[0].seq, 1, "the thread root never leaves the rail");
        assert_eq!(window[1].seq, 46, "the oldest reply edge slides forward");
        assert!(
            window[1].show_author,
            "the new visible edge opens an author run"
        );
        assert!(window.last().is_some_and(|message| message.pending));
    }

    #[test]
    fn a_room_landing_keeps_commits_that_arrived_after_its_snapshot() {
        let canonical = (1..=20).map(|seq| committed(seq, "alice")).collect();
        let current = vec![committed(21, "bob")];

        let landed = merge_landing_messages(canonical, current, "general".into(), "general".into());

        assert_eq!(
            landed.iter().map(|message| message.seq).collect::<Vec<_>>(),
            (1..=21).collect::<Vec<_>>()
        );
    }

    #[test]
    fn an_initial_thread_landing_keeps_replies_that_arrived_during_its_read() {
        let canonical = vec![
            committed(1, "alice"),
            ChatMessage {
                thread_seq: 1,
                ..committed(2, "alice")
            },
        ];
        let current = vec![
            committed(1, "alice"),
            ChatMessage {
                thread_seq: 1,
                ..committed(3, "bob")
            },
        ];

        let landed = merge_thread_refresh(canonical, current, "general".into(), "general".into());

        assert_eq!(
            landed.iter().map(|message| message.seq).collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }

    /// Every in-place row mutation is a render-key move — the view's keyed
    /// lazy (`by message.seq, message.render_rev`) repaints a row ONLY when a
    /// key changes, so a path missing its bump is a stale row on screen.
    #[test]
    fn every_in_place_row_mutation_bumps_render_rev() {
        // an edit with a newer rev bumps; its stale replay does not.
        let messages = vec![committed(1, "a")];
        let before = messages[0].render_rev;
        let content = ChatMessage {
            rev: 1,
            body: "new".into(),
            ..ChatMessage::default()
        };
        let edited = merge_message_edit(messages, 1, &content);
        assert_ne!(edited[0].render_rev, before, "an edit bumps");
        let replayed = merge_message_edit(edited.clone(), 1, &content);
        assert_eq!(
            replayed[0].render_rev, edited[0].render_rev,
            "a stale edit replay is a no-op"
        );

        // a tombstone bumps.
        let messages = vec![committed(2, "a")];
        let before = messages[0].render_rev;
        let deleted = tombstone_message(messages, 2);
        assert_ne!(deleted[0].render_rev, before, "a tombstone bumps");

        // a reaction add bumps, its remove bumps again, and a remove of an
        // emoji the row never had touches nothing.
        let messages = vec![committed(3, "a")];
        let before = messages[0].render_rev;
        let added = optimistic_reaction(messages, 3, "👍".into(), true, "user:ab".into());
        assert_ne!(added[0].render_rev, before, "a reaction add bumps");
        let mid = added[0].render_rev;
        let removed = optimistic_reaction(added, 3, "👍".into(), false, "user:ab".into());
        assert_ne!(removed[0].render_rev, mid, "a reaction remove bumps");
        let untouched =
            optimistic_reaction(removed.clone(), 3, "🎉".into(), false, "user:ab".into());
        assert_eq!(
            untouched[0].render_rev, removed[0].render_rev,
            "a remove of an absent emoji is a no-op"
        );

        // a reply-summary bump bumps.
        let messages = vec![committed(4, "a")];
        let before = messages[0].render_rev;
        let bumped = bump_reply_summary(messages, 4);
        assert_ne!(bumped[0].render_rev, before, "a reply summary bumps");

        // a grouping flip bumps — and ONLY a flip: the re-mark that runs on
        // every merge must not move unmoved headers.
        let mut messages = vec![committed(5, "a"), committed(6, "a")];
        mark_message_groups(&mut messages);
        assert!(
            !messages[1].show_author,
            "the second row folds under the run"
        );
        let after_flip = messages[1].render_rev;
        let first_row = messages[0].render_rev;
        mark_message_groups(&mut messages);
        assert_eq!(
            messages[1].render_rev, after_flip,
            "a re-mark without a flip does not bump"
        );
        assert_eq!(
            messages[0].render_rev, first_row,
            "an unflipped row never bumps"
        );
    }

    /// Construction seeds `render_rev` from the rendered content, so a
    /// wholesale replacement (a resync) moves the key exactly when the
    /// replacement row renders differently — and keeps it when it does not.
    #[test]
    fn construction_seeds_render_rev_from_rendered_content() {
        let row = |reactions: Vec<index::ReactionRow>| MsgRow {
            channel_id: "general".into(),
            seq: 7,
            message_id: "m7".into(),
            author: "user:ab".into(),
            height: 12,
            time: 0,
            blocks: vec![Block::paragraph("hi")],
            text: String::new(),
            deleted: false,
            edited: false,
            rev: 0,
            edited_at: None,
            base_rev: None,
            thread: None,
            reply_count: 0,
            last_reply_seq: None,
            reactions,
            tags: Vec::new(),
        };
        let anon = AuthorNames::default();
        let plain = chat_message(row(Vec::new()), None, &anon);
        let identical = chat_message(row(Vec::new()), None, &anon);
        assert_eq!(
            plain.render_rev, identical.render_rev,
            "identical content seeds identically — the cached subtree is kept"
        );
        let reacted = chat_message(
            row(vec![index::ReactionRow {
                emoji: "👍".into(),
                reactors: vec!["user:cd".into()],
            }]),
            None,
            &anon,
        );
        assert_ne!(
            plain.render_rev, reacted.render_rev,
            "a replacement row with reactions the displayed copy never saw moves the key"
        );

        // the optimistic mint seeds too. NOTE the seed follows the manual
        // `Hash` contract, which excludes body/blocks: under ONE id a pending
        // row's body never changes (the settle REPLACES the row and moves
        // `seq`), and every fresh send mints a fresh id — the field that does
        // move the seed.
        let minted = optimistic_message(Vec::new(), "hello".into(), "op1".into(), None, &anon);
        let re_minted = optimistic_message(Vec::new(), "hello".into(), "op1".into(), None, &anon);
        assert_eq!(minted[0].render_rev, re_minted[0].render_rev);
        let other = optimistic_message(Vec::new(), "hello".into(), "op2".into(), None, &anon);
        assert_ne!(minted[0].render_rev, other[0].render_rev);
    }

    #[test]
    fn reply_writers_bump_the_root_and_insert_the_reply() {
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
        let thread = merge_thread_reply(bump_reply_summary(vec![root], 1), reply);
        assert_eq!(
            thread[0].reply_count, 2,
            "the rail's replies rule reads this"
        );
        assert!(thread.iter().any(|message| message.seq == 3));
    }

    #[test]
    fn a_reader_sees_their_own_writing_as_you() {
        let me = vec![0xab; 32];
        let mine = format!("user:{}", hex_encode(&me));
        let theirs = format!("user:{}", hex_encode(&[0xcd; 32]));

        let anon = AuthorNames::default();
        assert_eq!(author_display(&mine, Some(&me), &anon), "you");
        // The same row read by anyone else is still the handle.
        assert_eq!(
            author_display(&mine, Some(&[0xcd; 32]), &anon),
            author_name(&mine, &anon)
        );
        assert_eq!(
            author_display(&theirs, Some(&me), &anon),
            author_name(&theirs, &anon)
        );
        // No local key (the boot race) renders nobody as `you`.
        assert_eq!(
            author_display(&mine, None, &anon),
            author_name(&mine, &anon)
        );
        // An agent is never the reader.
        assert_eq!(author_display("acct:5", Some(&me), &anon), "account 5");
    }

    /// A KEY IS NOT A NAME, AND THE READER NEVER ASKED FOR ONE. `user:{hex}` is
    /// everything a chat row carries; the account name behind that key lives in
    /// the identity module, so a timeline read without its directory prints hex
    /// at people who are named one pane away in the DIRECT list.
    #[test]
    fn a_registered_account_renders_by_name() {
        let key = vec![0xbf; 32];
        let handle = format!("user:{}", hex_encode(&key));
        let names = AuthorNames::new([(hex_encode(&key).to_ascii_uppercase(), "orthory".into())]);

        // Hex casing is the directory's problem, not the caller's.
        assert_eq!(author_name(&handle, &names), "orthory");
        // The avatar follows the name — an "O", not the first hex nibble.
        assert_eq!(avatar_initial(&handle, &names), "O");
        // A reader still sees their own writing as `you`, name or no name.
        assert_eq!(author_display(&handle, Some(&key), &names), "you");
        // And a key with no account is still honestly its short hex.
        let stranger = format!("user:{}", hex_encode(&[0x11; 32]));
        assert_eq!(author_name(&stranger, &names), "user 11111111…");
        // A module or agent names itself; the directory has no say.
        assert_eq!(author_name("acct:5", &names), "account 5");
        assert_eq!(author_name("module:runs", &names), "runs");
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
        })
        .expect("a PostMessage encodes");
        let assigned = serde_json::json!({ "posted": { "seq": 7, "actor": { "key": [171, 171] }, "blocks": [] } });
        let delta = delta_from_op(
            &payload,
            Some(&assigned),
            "external",
            Some("ext:ab"),
            None,
            &AuthorNames::default(),
            276_199,
        )
        .expect("a well-formed op folds")
        .expect("a post is visible to the UI");

        let ChatDelta::Posted { message, .. } = delta else {
            panic!("a post must decode to the posted transition")
        };
        assert_eq!(message.height, 276_199);
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
        crate::Chat::validate_channel_namespace(&Party::Key(vec![0xab; 32]), &id)
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
        assert!(view[0].spans.iter().any(|span| span.bold == "world"));
        assert_eq!(view[3].kind, "code");
        assert_eq!(view[3].text, "fn main() {}");

        // a plain message stays a single non-rich paragraph
        let plain = blocks_view(&parse_message("just text here"));
        assert_eq!(plain.len(), 1);
        assert!(!plain[0].rich);
        assert_eq!(plain[0].text, "just text here");
    }

    /// A `duck://` address is a link, in both forms a member or an agent
    /// actually types one: the reference form the run injector emits
    /// (`runs::inject`) and the bare run. Without these the app's open plane
    /// is unreachable from chat — the protocol's whole last mile.
    #[test]
    fn a_duck_reference_and_a_bare_duck_url_both_become_one_link_span() {
        let Block::Paragraph(spans) = &parse_message("[x](duck://page/p1)")[0] else {
            panic!("a paragraph");
        };
        assert_eq!(spans.len(), 1, "label and target are one span: {spans:?}");
        assert_eq!(spans[0].text, "x");
        assert_eq!(spans[0].marks, vec![Mark::Link("duck://page/p1".into())]);

        let Block::Paragraph(bare) = &parse_message("duck://forge/r/1")[0] else {
            panic!("a paragraph");
        };
        assert_eq!(bare.len(), 1);
        assert_eq!(bare[0].text, "duck://forge/r/1");
        assert_eq!(bare[0].marks, vec![Mark::Link("duck://forge/r/1".into())]);

        // a produced link carries its network, and the query rides along
        let Block::Paragraph(net) = &parse_message("see [58](duck://forge/d/58?net=d0cdf950)")[0]
        else {
            panic!("a paragraph");
        };
        assert_eq!(net[0].text, "see ");
        assert_eq!(net[1].text, "58");
        assert_eq!(
            net[1].marks,
            vec![Mark::Link("duck://forge/d/58?net=d0cdf950".into())]
        );

        // not references, and not links either: an unknown scheme in the
        // target, and a bracket that opens no reference at all.
        for typed in ["[x](mailto:a@b)", "a [bracketed] word"] {
            let Block::Paragraph(spans) = &parse_message(typed)[0] else {
                panic!("a paragraph");
            };
            assert!(
                !spans
                    .iter()
                    .any(|span| matches!(span.marks.first(), Some(Mark::Link(_)))),
                "{typed} carries no link"
            );
            assert_eq!(
                span_text(spans),
                typed,
                "{typed} stays the text it was typed as"
            );
        }

        // an empty label is no reference — the bare target inside is still a
        // link, labelled by itself, and the prose's `)` is not part of it.
        let Block::Paragraph(unlabelled) = &parse_message("[](duck://page/p1)")[0] else {
            panic!("a paragraph");
        };
        let link = unlabelled
            .iter()
            .find(|span| matches!(span.marks.first(), Some(Mark::Link(_))))
            .expect("a link span");
        assert_eq!(link.text, "duck://page/p1");
        assert_eq!(span_text(unlabelled), "[](duck://page/p1)", "no text lost");

        // a `)` the address itself opened stays in it.
        let Block::Paragraph(balanced) = &parse_message("https://x/Foo_(bar)")[0] else {
            panic!("a paragraph");
        };
        assert_eq!(balanced[0].text, "https://x/Foo_(bar)");
    }

    /// `⇧↵` PUTS A NEWLINE IN THE BUFFER AND THE COMPOSER SAYS SO.
    ///
    /// Consecutive lines were folded into one paragraph with a space, so a
    /// typed list posted as "- apples - bananas - pears" — and the fold happens
    /// on the way to the CHAIN, so no renderer recovers it. A rendered break
    /// has to be a block boundary: a marked-up line renders as one rich-text
    /// paragraph, and a break belongs between paragraphs, not inside one.
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

    fn directory() -> AuthorNames {
        AuthorNames::new([
            ("aa11".into(), "eddy".into()),
            ("bb22".into(), "orthory".into()),
            ("cc33".into(), "orthory".into()),
            ("dd44".into(), "orthory-ops".into()),
        ])
    }

    fn mention_keys(blocks: &[Block]) -> Vec<Vec<u8>> {
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("paragraph expected");
        };
        spans
            .iter()
            .flat_map(|span| span.marks.iter())
            .filter_map(|mark| match mark {
                Mark::Mention(Party::Key(key)) => Some(key.clone()),
                _ => None,
            })
            .collect()
    }

    /// The bug: `#general` is an open channel with an EMPTY member roster, so
    /// the roster-only candidate set resolved no name at all in the one room
    /// everybody writes in. The directory is the candidate set now.
    #[test]
    fn an_account_name_mentions_in_a_channel_with_no_roster() {
        let mentions = MentionCandidates::new(&directory(), &[]);
        let blocks = parse_message_with_mentions("ping @orthory about it", &mentions);
        // both of that account's keys, so the mention reaches the PERSON.
        assert_eq!(
            mention_keys(&blocks),
            vec![vec![0xbb, 0x22], vec![0xcc, 0x33]]
        );
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("paragraph expected");
        };
        assert!(spans.iter().any(|span| span.text == "@orthory"));
    }

    /// A hyphenated name is one handle, and the longer one wins: `@orthory-ops`
    /// must not settle on `orthory` and leave `-ops` as text.
    #[test]
    fn a_hyphenated_name_wins_over_its_own_prefix() {
        let mentions = MentionCandidates::new(&directory(), &[]);
        let blocks = parse_message_with_mentions("cc @orthory-ops", &mentions);
        assert_eq!(mention_keys(&blocks), vec![vec![0xdd, 0x44]]);
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("paragraph expected");
        };
        assert!(spans.iter().any(|span| span.text == "@orthory-ops"));
    }

    /// Trailing punctuation is sentence, not handle.
    #[test]
    fn a_trailing_dot_is_shed_from_a_handle() {
        let mentions = MentionCandidates::new(&directory(), &[]);
        let blocks = parse_message_with_mentions("done @orthory-ops. thanks", &mentions);
        assert_eq!(mention_keys(&blocks), vec![vec![0xdd, 0x44]]);
        assert_eq!(message_body(&blocks), "done @orthory-ops. thanks");
    }

    #[test]
    fn a_roster_key_still_resolves_beside_the_directory() {
        let members = vec![ChatMember {
            key: "f00dbeef".into(),
            label: "f00dbeef".into(),
        }];
        let mentions = MentionCandidates::new(&directory(), &members);
        let blocks = parse_message_with_mentions("hi @f00d and @eddy", &mentions);
        assert_eq!(
            mention_keys(&blocks),
            vec![vec![0xf0, 0x0d, 0xbe, 0xef], vec![0xaa, 0x11]]
        );
    }

    #[test]
    fn a_mention_reaches_every_key_of_the_account_it_names() {
        let names = directory();
        let blocks =
            parse_message_with_mentions("@orthory ping", &MentionCandidates::new(&names, &[]));
        // orthory reads this on either device.
        assert!(mentions_reach(
            &blocks,
            &names
                .keys_named("orthory")
                .into_iter()
                .map(Party::Key)
                .collect::<Vec<_>>()
        ));
        assert!(mentions_reach(
            &blocks,
            &names
                .keys_named("orthory")
                .into_iter()
                .map(Party::Key)
                .collect::<Vec<_>>()
        ));
        // orthory-ops is a different account and is not addressed.
        assert!(!mentions_reach(
            &blocks,
            &names
                .account_keys_of(&[0xdd, 0x44])
                .into_iter()
                .map(Party::Key)
                .collect::<Vec<_>>()
        ));
    }

    #[test]
    fn an_unregistered_key_is_its_own_account() {
        assert_eq!(
            directory().account_keys_of(&[0x99]),
            vec![vec![0x99]],
            "a key nobody registered is still one identity"
        );
    }

    #[test]
    fn only_a_derived_id_reads_as_a_dm_room() {
        let derived = dm_channel_id("1", "2");
        assert!(is_derived_dm_channel(&derived));
        assert!(!is_derived_dm_channel("general"));
        // a person may name a channel this and it stays a channel.
        assert!(!is_derived_dm_channel("dm-standup"));
        assert!(!is_derived_dm_channel(&derived.to_ascii_uppercase()));
    }

    #[test]
    fn mentions_resolve_against_the_member_roster() {
        let roster =
            |members: &[ChatMember]| MentionCandidates::new(&AuthorNames::default(), members);
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
        let blocks = parse_message_with_mentions("ping @a1b2 about the deploy", &roster(&members));
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("paragraph expected");
        };
        let mention = spans
            .iter()
            .find(|span| span.marks.iter().any(|m| matches!(m, Mark::Mention(_))))
            .expect("a mention span");
        assert_eq!(mention.text, "@a1b2");
        let Mark::Mention(Party::Key(bytes)) = &mention.marks[0] else {
            panic!("user mention expected");
        };
        assert_eq!(bytes, &vec![0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6]);

        // a printable (non-hex) identity keeps its own bytes
        let blocks = parse_message_with_mentions("hey @zoe1 no — @zoea", &roster(&members));
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
        let blocks = parse_message_with_mentions("email @someone", &roster(&members));
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("paragraph expected");
        };
        assert!(spans.iter().all(|span| span.marks.is_empty()));

        // rendered mentions land in the mention arm
        let view = blocks_view(&parse_message_with_mentions(
            "cc @a1b2c3",
            &roster(&members),
        ));
        assert!(view[0].rich);
        assert!(
            view[0]
                .spans
                .iter()
                .any(|span| span.mention.starts_with("@a1b2c3"))
        );
    }

    #[test]
    fn reactions_know_the_local_reactor() {
        let reactors = vec![
            format!("user:{}", hex_encode(&[0xab; 32])),
            "system".to_string(),
        ];
        assert!(reacted_by_user(
            &reactors,
            Some(&[0xab; 32]),
            &AuthorNames::default()
        ));
        assert!(!reacted_by_user(
            &reactors,
            Some(&[0xcd; 32]),
            &AuthorNames::default()
        ));
        assert!(!reacted_by_user(&reactors, None, &AuthorNames::default()));
    }

    #[test]
    fn avatars_distinguish_humans_from_software_authors() {
        assert_eq!(avatar_kind("user:deadbeef"), "human");
        for author in ["module:reviewer", "module:forge", "system"] {
            assert_eq!(avatar_kind(author), "agent");
        }
    }

    /// The arm fields of one span, in template order.
    fn arm_texts(span: &ChatSpan) -> [&String; 6] {
        [
            &span.mention,
            &span.link_text,
            &span.bold_italic,
            &span.bold,
            &span.italic,
            &span.plain,
        ]
    }

    #[test]
    fn plain_rich_spans_mark_inline_runs_and_stay_empty_for_plain_text() {
        let spans = plain_rich_spans("say **hi** to https://duck.example/x");
        let bold: Vec<_> = spans.iter().filter(|span| !span.bold.is_empty()).collect();
        assert_eq!(bold.len(), 1);
        assert_eq!(bold[0].bold, "hi");
        let link = spans
            .iter()
            .find(|span| !span.link.is_empty())
            .expect("the bare url becomes a link span");
        assert_eq!(link.link_text, "https://duck.example/x");
        assert_eq!(link.link, "https://duck.example/x");

        // EXACTLY ONE ARM PER RUN — the view's rich-text `for` emits every
        // template span for every run, so a run filed under two arms renders
        // twice and a run filed under none vanishes from the paragraph.
        for span in &spans {
            let filled = arm_texts(span)
                .iter()
                .filter(|text| !text.is_empty())
                .count();
            assert_eq!(filled, 1, "one style arm per run: {span:?}");
        }

        // And the arms concatenate back to the typed text minus the marks —
        // the same wholeness the one-paragraph render shows the reader.
        let rendered: String = spans
            .iter()
            .flat_map(arm_texts)
            .map(String::as_str)
            .collect();
        assert_eq!(rendered, "say hi to https://duck.example/x");

        assert!(plain_rich_spans("no marks here").is_empty());
        assert!(plain_rich_spans("**multi**\nline").is_empty());
    }
}
