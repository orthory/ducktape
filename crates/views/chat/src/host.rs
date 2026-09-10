//! The chat facts the host pushes, the readings folded off them, and the
//! writes that leave as intents — one per act the screen offers. The
//! composers are the host's own surfaces: a submit never passes through
//! here.

use iced::futures::StreamExt;
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

// The chat module's own rows, as the desktop app serializes them
// (`chat::client`, `app/src/backend`). Fields the view never reads are not
// mirrored; serde leaves them on the floor.

#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ChatChannel {
    pub id: String,
    pub name: String,
    pub archived: bool,
    pub members_only: bool,
    pub huddle_count: i64,
    pub head_seq: i64,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ChatReaction {
    pub emoji: String,
    pub count: i64,
    pub reacted_by_me: bool,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ChatMember {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ChatSpan {
    pub mention: String,
    pub mention_link: String,
    pub link_text: String,
    pub link: String,
    pub bold_italic: String,
    pub bold: String,
    pub italic: String,
    pub plain: String,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ChatBlock {
    pub kind: String,
    pub text: String,
    pub lang: String,
    pub rich: bool,
    pub spans: Vec<ChatSpan>,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
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
    pub height: i64,
    pub time: i64,
    pub reactions: Vec<ChatReaction>,
    pub render_rev: i64,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ChatSidebarRow {
    pub channel: ChatChannel,
    pub unread: bool,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct DmPeer {
    pub key: String,
    pub name: String,
    pub initials: String,
    pub is_agent: bool,
    pub channel_id: String,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct DmSidebarRow {
    pub peer: DmPeer,
    pub unread: bool,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ChatSearchHit {
    pub channel_id: String,
    pub seq: i64,
    pub root_seq: i64,
    pub author: String,
    pub text: String,
    pub meta: String,
}

/// The screen's facts, as the app holds them: one document per change. The
/// enums cross by name (`search_phase`, `message_action`,
/// `thread_message_action`, `copy_surface`); the mutation lock crosses as
/// `busy`. `sent_serial` moves once per admitted send, which is the view's
/// cue to snap the stream to its tail.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatProps {
    pub dark: bool,
    pub endpoint: String,
    pub network_name: String,
    pub network_chain_id: String,
    pub status: String,
    pub block_height: i64,
    pub search_phase: String,
    pub search_query: String,
    pub search_hits: Vec<ChatSearchHit>,
    pub rooms: Vec<ChatSidebarRow>,
    pub dm_rows: Vec<DmSidebarRow>,
    pub channel_create_open: bool,
    pub connected: bool,
    pub loading: bool,
    pub busy: bool,
    pub active_channel: String,
    pub active_dm_peer: String,
    pub active_dm: DmPeer,
    pub active_channel_name: String,
    pub active_channel_archived: bool,
    pub active_channel_members_only: bool,
    pub channel_members: Vec<ChatMember>,
    pub post_refusal: String,
    pub huddle_joined: bool,
    pub huddle_channel: String,
    pub huddle_channel_name: String,
    pub huddle_joined_at: i64,
    pub huddle_now: i64,
    pub call_muted: bool,
    pub messages: Vec<ChatMessage>,
    pub has_older_history: bool,
    pub history_view: bool,
    pub at_live_tail: bool,
    pub history_loading: bool,
    pub unread_boundary: i64,
    pub unread_marker_seq: i64,
    pub selected_message_seq: i64,
    pub selected_message_rev: i64,
    pub message_action: String,
    pub channel_settings_open: bool,
    pub active_thread_seq: i64,
    pub thread_target_seq: i64,
    pub thread_messages: Vec<ChatMessage>,
    pub thread_selected_seq: i64,
    pub thread_selected_rev: i64,
    pub thread_message_action: String,
    pub thread_has_more: bool,
    pub thread_next_reply_seq: i64,
    pub thread_loading: bool,
    pub copy_anchor_seq: i64,
    pub copy_head_seq: i64,
    pub copy_surface: String,
    pub sent_serial: i64,
    /// The agent runs anchored in THIS room, live while they run; the
    /// committed reply takes a row's place. The host filters by room before
    /// encoding, so a row here is always one of this room's.
    pub live_agents: Vec<LiveRunHint>,
}

/// An agent run in flight under its anchor message: whose it is, where it
/// stands, and which run to open for its progress. The host's row also
/// names the room; it is not mirrored, because this screen draws one room.
/// The run's activity and answer preview are the run panel's to draw, so the
/// stream never carries them.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct LiveRunHint {
    pub anchor_seq: i64,
    pub thread_root: i64,
    pub run_id: String,
    /// the run's address: what `open_run` hands the app
    pub dispatch_id: String,
    pub agent: String,
    pub status: String,
}

/// The run a committed message was posted by, off the message id the runs
/// module mints for a run's replies: `agent/<dispatch_id>` for the run's one
/// final reply, `agent/<dispatch_id>/post/<slot>` for a post it staged. ""
/// for any other message.
pub fn run_of_message(id: &str) -> String {
    let Some(rest) = id.strip_prefix("agent/") else {
        return String::new();
    };
    let dispatch = rest.split_once('/').map_or(rest, |(dispatch, _)| dispatch);
    let is_dispatch_id = dispatch.len() == 64
        && dispatch
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    match is_dispatch_id {
        true => dispatch.to_owned(),
        false => String::new(),
    }
}

/// One item of the facts subscription: the facts, or why not.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropsItem {
    pub next: ChatProps,
    pub error: String,
}

/// The facts now, and again on every change the host sees — a
/// subscription, so a view restored from a snapshot asks again on its own.
pub fn props() -> iced::Subscription<PropsItem> {
    iced::Subscription::run(|| {
        host::subscribe("chat.props", &[]).map(|answer| {
            let read = answer.and_then(|bytes| {
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())
            });
            match read {
                Ok(next) => PropsItem {
                    next,
                    error: String::new(),
                },
                Err(error) => PropsItem {
                    next: ChatProps::default(),
                    error,
                },
            }
        })
    })
}

// ---------- the enums, by name ----------

pub(crate) fn search_phase_of(name: &str) -> crate::SearchPhase {
    match name {
        "searching" => crate::SearchPhase::Searching,
        "done" => crate::SearchPhase::Done,
        _ => crate::SearchPhase::Idle,
    }
}

pub(crate) fn message_action_of(name: &str) -> crate::MessageAction {
    match name {
        "more" => crate::MessageAction::More,
        "reactions" => crate::MessageAction::Reactions,
        "editing" => crate::MessageAction::Editing,
        "delete" => crate::MessageAction::Delete,
        _ => crate::MessageAction::Toolbar,
    }
}

pub(crate) fn copy_surface_of(name: &str) -> crate::CopySurface {
    match name {
        "timeline" => crate::CopySurface::Timeline,
        "thread" => crate::CopySurface::Thread,
        _ => crate::CopySurface::Nowhere,
    }
}

pub(crate) fn surface_name(surface: crate::CopySurface) -> String {
    match surface {
        crate::CopySurface::Timeline => "timeline",
        crate::CopySurface::Thread => "thread",
        crate::CopySurface::Nowhere => "nowhere",
    }
    .to_owned()
}

// ---------- the intents ----------

/// `chat.search` — run a workspace-wide message search.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    pub query: String,
}

/// `chat.open_hit` — land on a search hit: its channel, thread root and target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hit {
    pub channel: String,
    pub root_seq: i64,
    pub target_seq: i64,
}

/// `chat.choose_channel` — the room to open.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
}

/// `chat.choose_dm`, `chat.add_member`, `chat.remove_member` — a key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub key: String,
}

/// `chat.scrolled` — the stream's offsets, relative to its end anchor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scrolled {
    pub absolute_x: f64,
    pub absolute_y: f64,
    pub relative_x: f64,
    pub relative_y: f64,
}

/// `chat.open_link` — a pressed link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Url {
    pub url: String,
}

/// `chat.copy` — the host puts `text` on the clipboard and toasts `label`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copy {
    pub text: String,
    pub label: String,
}

/// `chat.copy_link` — a built message link for the clipboard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub link: String,
}

/// `chat.add_reaction`, `chat.remove_reaction` — one emoji on one message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaction {
    pub seq: i64,
    pub emoji: String,
}

/// `chat.open_thread` — the root to open the rail on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seq {
    pub seq: i64,
}

/// The run a Stop names.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunId {
    pub run_id: String,
}

/// The run a "View run" or a message's run chip opens.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DispatchId {
    pub dispatch_id: String,
}

/// The message-menu openers: which message, its body and its revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub seq: i64,
    pub body: String,
    pub rev: i64,
}

/// `chat.press` — a press on a message's prose, in which surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Press {
    pub seq: i64,
    pub surface: String,
}

/// `chat.reaction_submit` — the picker's choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Emoji {
    pub emoji: String,
}

/// `chat.edit`, `chat.thread_edit` — the edited body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    pub text: String,
}

/// `chat.rename` — the channel's new name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Name {
    pub name: String,
}

fn notify<T: Serialize>(operation: &str, payload: &T) -> bool {
    let bytes = serde_json::to_vec(payload).expect("an intent encodes");
    host::notify(operation, &bytes);
    true
}

pub fn send_search(query: &str) -> bool {
    notify(
        "chat.search",
        &Query {
            query: query.into(),
        },
    )
}

pub fn send_clear_search() -> bool {
    notify("chat.clear_search", &())
}

pub fn send_open_hit(channel: &str, root_seq: i64, target_seq: i64) -> bool {
    notify(
        "chat.open_hit",
        &Hit {
            channel: channel.into(),
            root_seq,
            target_seq,
        },
    )
}

pub fn send_toggle_create() -> bool {
    notify("chat.toggle_create", &())
}

pub fn send_choose_channel(id: &str) -> bool {
    notify("chat.choose_channel", &Channel { id: id.into() })
}

pub fn send_choose_dm(key: &str) -> bool {
    notify("chat.choose_dm", &Key { key: key.into() })
}

pub fn send_toggle_settings() -> bool {
    notify("chat.toggle_settings", &())
}

pub fn send_show_huddle() -> bool {
    notify("chat.show_huddle", &())
}

pub fn send_leave_huddle() -> bool {
    notify("chat.leave_huddle", &())
}

pub fn send_join_huddle() -> bool {
    notify("chat.join_huddle", &())
}

pub fn send_load_history() -> bool {
    notify("chat.load_history", &())
}

pub fn send_scrolled(absolute_x: f64, absolute_y: f64, relative_x: f64, relative_y: f64) -> bool {
    notify(
        "chat.scrolled",
        &Scrolled {
            absolute_x,
            absolute_y,
            relative_x,
            relative_y,
        },
    )
}

pub fn send_open_link(url: &str) -> bool {
    notify("chat.open_link", &Url { url: url.into() })
}

pub fn send_copy(text: &str, label: &str) -> bool {
    notify(
        "chat.copy",
        &Copy {
            text: text.into(),
            label: label.into(),
        },
    )
}

pub fn send_copy_link(link: &str) -> bool {
    notify("chat.copy_link", &Link { link: link.into() })
}

pub fn send_add_reaction(seq: i64, emoji: &str) -> bool {
    notify(
        "chat.add_reaction",
        &Reaction {
            seq,
            emoji: emoji.into(),
        },
    )
}

pub fn send_remove_reaction(seq: i64, emoji: &str) -> bool {
    notify(
        "chat.remove_reaction",
        &Reaction {
            seq,
            emoji: emoji.into(),
        },
    )
}

pub fn send_open_thread(seq: i64) -> bool {
    notify("chat.open_thread", &Seq { seq })
}

pub fn send_cancel_run(run_id: &str) -> bool {
    notify(
        "chat.cancel_run",
        &RunId {
            run_id: run_id.into(),
        },
    )
}

/// Take the reader to a run's panel: the live hint's "View run", or the run
/// chip on a message a run posted.
pub fn send_open_run(dispatch_id: &str) -> bool {
    notify(
        "chat.open_run",
        &DispatchId {
            dispatch_id: dispatch_id.into(),
        },
    )
}

fn selection(operation: &str, seq: i64, body: &str, rev: i64) -> bool {
    notify(
        operation,
        &Selection {
            seq,
            body: body.into(),
            rev,
        },
    )
}

pub fn send_message_actions(seq: i64, body: &str, rev: i64) -> bool {
    selection("chat.message_actions", seq, body, rev)
}

pub fn send_message_reactions(seq: i64, body: &str, rev: i64) -> bool {
    selection("chat.message_reactions", seq, body, rev)
}

pub fn send_begin_edit(seq: i64, body: &str, rev: i64) -> bool {
    selection("chat.begin_edit", seq, body, rev)
}

pub fn send_arm_delete(seq: i64, body: &str, rev: i64) -> bool {
    selection("chat.arm_delete", seq, body, rev)
}

pub fn send_clear_selection() -> bool {
    notify("chat.clear_selection", &())
}

pub(crate) fn send_press(seq: i64, surface: crate::CopySurface) -> bool {
    notify(
        "chat.press",
        &Press {
            seq,
            surface: surface_name(surface),
        },
    )
}

pub fn send_clear_range() -> bool {
    notify("chat.clear_range", &())
}

pub fn send_copy_range() -> bool {
    notify("chat.copy_range", &())
}

pub fn send_reaction_submit(emoji: &str) -> bool {
    notify(
        "chat.reaction_submit",
        &Emoji {
            emoji: emoji.into(),
        },
    )
}

pub fn send_edit(text: &str) -> bool {
    notify("chat.edit", &Text { text: text.into() })
}

pub fn send_delete() -> bool {
    notify("chat.delete", &())
}

pub fn send_rename(name: &str) -> bool {
    notify("chat.rename", &Name { name: name.into() })
}

pub fn send_archive() -> bool {
    notify("chat.archive", &())
}

pub fn send_unarchive() -> bool {
    notify("chat.unarchive", &())
}

pub fn send_add_member(key: &str) -> bool {
    notify("chat.add_member", &Key { key: key.into() })
}

pub fn send_remove_member(key: &str) -> bool {
    notify("chat.remove_member", &Key { key: key.into() })
}

pub fn send_close_thread() -> bool {
    notify("chat.close_thread", &())
}

pub fn send_thread_actions(seq: i64, body: &str, rev: i64) -> bool {
    selection("chat.thread_actions", seq, body, rev)
}

pub fn send_thread_reactions(seq: i64, body: &str, rev: i64) -> bool {
    selection("chat.thread_reactions", seq, body, rev)
}

pub fn send_thread_begin_edit(seq: i64, body: &str, rev: i64) -> bool {
    selection("chat.thread_begin_edit", seq, body, rev)
}

pub fn send_thread_arm_delete(seq: i64, body: &str, rev: i64) -> bool {
    selection("chat.thread_arm_delete", seq, body, rev)
}

pub fn send_thread_clear_selection() -> bool {
    notify("chat.thread_clear_selection", &())
}

pub fn send_thread_edit(text: &str) -> bool {
    notify("chat.thread_edit", &Text { text: text.into() })
}

pub fn send_thread_delete() -> bool {
    notify("chat.thread_delete", &())
}

pub fn send_load_thread() -> bool {
    notify("chat.load_thread", &())
}

// The desktop app's own readings (app/src/backend, chat::client), repeated
// here because the view is its own crate: the wire carries the words, not
// the functions.

pub fn icon(name: &str) -> Vec<u8> {
    design::icons::svg(name).as_bytes().to_vec()
}

pub fn connection_degraded(status: &str) -> bool {
    status == "Offline"
        || status == "Sync delayed"
        || status == "Reconnecting…"
        || status == "Live · resyncing"
}

pub(crate) fn message_plate(deleted: bool, selected: bool, in_range: bool) -> crate::RowPlate {
    match (deleted, selected, in_range) {
        (true, _, _) => crate::RowPlate::Plain,
        (false, true, _) => crate::RowPlate::Selected,
        (false, false, true) => crate::RowPlate::Ranged,
        (false, false, false) => crate::RowPlate::Plain,
    }
}

fn range_seqs(anchor: i64, head: i64) -> Option<(i64, i64)> {
    (anchor > 0 && head > 0).then(|| (anchor.min(head), anchor.max(head)))
}

pub(crate) fn seq_in_copy_range(
    seq: i64,
    anchor: i64,
    head: i64,
    surface: crate::CopySurface,
    mine: crate::CopySurface,
) -> bool {
    if surface != mine {
        return false;
    }
    range_seqs(anchor, head).is_some_and(|(low, high)| seq >= low && seq <= high)
}

/// The run whose answer posts into the thread rooted at `active_thread_seq` —
/// either because that root IS its anchor, or because its anchor is a reply
/// inside that thread.
///
/// Only ask it with a thread actually open: at `active_thread_seq == 0` every
/// top-level run answers it, since an unreplied run's `thread_root` is 0 too.
/// Both callers are already inside the rail's visibility gate.
pub fn run_in_thread(live: &LiveRunHint, active_thread_seq: i64) -> bool {
    live.anchor_seq == active_thread_seq || live.thread_root == active_thread_seq
}

/// Whether the thread RAIL owns this run's card, and so the stream must not draw
/// one for it.
///
/// ONE RUN, ONE CARD. The stream draws a card under the anchor and the rail draws
/// one at its foot — and when the open thread IS the run's thread, both
/// conditions held: two cards and two Stops for a single run. The rail wins
/// while it is on screen, because that is the surface the reader is working in,
/// and the stream takes the card back the moment the rail closes.
///
/// `rail_shown` is the rail's OWN visibility — `active_thread_seq > 0 &&
/// !channel_settings_open`, the gate it is drawn under — and not merely "a thread
/// is open". The channel-settings drawer replaces the rail while
/// `active_thread_seq` still stands, so suppressing on the seq alone would have
/// hidden BOTH cards for as long as the drawer was open.
pub fn rail_owns_run(live: &LiveRunHint, rail_shown: bool, active_thread_seq: i64) -> bool {
    rail_shown && run_in_thread(live, active_thread_seq)
}

/// The stream and the runs live in it, as one value: the timeline memo hashes
/// its one dependency, so the two lists that draw together must cross the
/// boundary together — a run's status folded into `live_agents` alone would
/// leave the memo's key unmoved and the hint would never repaint.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct Timeline {
    pub messages: Vec<ChatMessage>,
    pub live_agents: Vec<LiveRunHint>,
}

pub fn timeline_of(messages: &[ChatMessage], live_agents: &[LiveRunHint]) -> Timeline {
    Timeline {
        messages: messages.to_vec(),
        live_agents: live_agents.to_vec(),
    }
}

pub fn copy_range_count(messages: &[ChatMessage], anchor: i64, head: i64) -> i64 {
    let Some((low, high)) = range_seqs(anchor, head) else {
        return 0;
    };
    messages
        .iter()
        .filter(|message| message.seq >= low && message.seq <= high)
        .count() as i64
}

pub fn copy_range_label(count: i64) -> String {
    match count {
        1 => "1 message selected".to_owned(),
        count => format!("{count} messages selected"),
    }
}

pub fn thread_width_after_delta(width: f64, delta: f64, viewport: f64) -> f64 {
    // Keep the channel list, divider and a readable conversation alongside it.
    let maximum = (viewport - 236.0 - 10.0 - 320.0).clamp(280.0, 640.0);
    (width + delta).clamp(280.0, maximum)
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

pub fn search_answer_stands(query: &str, draft: &str, searching: bool) -> bool {
    !searching && !query.is_empty() && draft.trim() == query
}

pub fn reaction_palette() -> Vec<String> {
    [
        "👍", "❤️", "😄", "😂", "😮", "😢", "🎉", "👀", //
        "🙌", "🔥", "✅", "❌", "💯", "🚀", "🤔", "😅", //
        "🙏", "👏", "💪", "✨", "⚡", "🐛", "📌", "❓", //
        "🦆", "🤝", "😴", "🧠", "➕", "🎯", "🚧", "🏁",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub fn plural(count: i64, one: &str, many: &str) -> String {
    let noun = if count == 1 { one } else { many };
    format!("{count} {noun}")
}

fn grouped_digits(value: i64) -> String {
    let digits = value.max(0).to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        let boundary = index > 0 && (digits.len() - index).is_multiple_of(3);
        if boundary {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

pub fn height_label(height: i64) -> String {
    if height < 0 {
        return "h —".into();
    }
    format!("h {}", grouped_digits(height))
}

pub fn height_label_short(height: i64) -> String {
    height_label(height)
}

fn net_query(chain_id: &str) -> String {
    let digest = chain_id.rsplit_once('#').map(|(_, hex)| hex).unwrap_or("");
    match digest.is_empty() {
        true => String::new(),
        false => format!("?net={digest}"),
    }
}

pub fn duck_channel_link(channel: String, chain_id: String) -> String {
    format!("duck://channel/{channel}{}", net_query(&chain_id))
}

pub fn duck_channel_message_link(channel: String, seq: i64, chain_id: String) -> String {
    format!("duck://channel/{channel}{}#{seq}", net_query(&chain_id))
}

pub fn mmss(seconds: i64) -> String {
    let seconds = seconds.max(0);
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

pub fn count_label(count: i64) -> String {
    match count > 0 {
        true => count.to_string(),
        false => String::new(),
    }
}

/// The composer instances' keys, as the app spells them (`backend/model.rs`):
/// the endpoint is in both, because a channel id is a user-chosen string and
/// two networks' `#general` are two rooms.
pub fn composer_scope(endpoint: &str, channel_id: &str) -> String {
    format!("{endpoint}\u{1f}{channel_id}")
}

pub fn thread_scope(endpoint: &str, channel_id: &str, thread_seq: i64) -> String {
    format!("{endpoint}\u{1f}{channel_id}#{thread_seq}")
}

pub(crate) fn tone_of(dark: bool) -> crate::Tone {
    if dark {
        crate::Tone::Dark
    } else {
        crate::Tone::Light
    }
}

pub fn no_dm_peer() -> DmPeer {
    DmPeer::default()
}
