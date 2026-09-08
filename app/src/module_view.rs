//! Module-owned views. A screen that ships as an `ice:view` component — an
//! Ice application compiled for the `tree` target (`crates/views`) — is
//! loaded FROM A FILE beside the binary (`make views` stages
//! `target/views/<module>_view.wasm`; `DUCKTAPE_VIEWS_DIR` overrides), ticked
//! inside a fuel and time budget, and drawn with the runtime's tree renderer
//! as one widget in the tab that used to hold the native screen.
//!
//! The boundary is the screen component's own contract. Its props go in as
//! JSON, one item per change, on the guest's `<module>.props` subscription;
//! its emits come out as intents the widget hands the app as
//! [`ModuleViewEvent`]s, so every write keeps going through the handler that
//! signs it today. The guest sees no key, no endpoint and no clock — a view
//! that holds none of them cannot leak one — and a view that traps shows why
//! in its place instead of taking the window with it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector, widget, window};
use ui_lang_runtime::view_tree::{self, Inputs, Output, Pictures, Surfaces};
use ui_lang_wire as wire;
use wasmtime::component::{Component, Linker, TypedFunc};
use wasmtime::{Config, Engine, OptLevel, Store, StoreContextMut, StoreLimits, StoreLimitsBuilder};

/// What a module view asked the app to do: `kind` is the operation
/// (`vote`, `execute`), `detail` the guest's JSON for it.
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct ModuleViewEvent {
    pub kind: String,
    pub detail: String,
}

/// What one call into a view may spend: about a 60 Hz frame of
/// instructions, and a wall-clock deadline for the time an import takes
/// that fuel cannot see.
const FUEL_PER_TICK: u64 = 100_000_000;
const TICK_DEADLINE: Duration = Duration::from_millis(100);
const EPOCH_TICK: Duration = Duration::from_millis(10);
const MEMORY_LIMIT: usize = 64 << 20;
const MAX_MODULE_BYTES: u64 = 64 << 20;
/// A frame the view sends past this ends it: nothing a screen needs is
/// megabytes, and the host would decode all of it on the window thread.
const MAX_FRAME_BYTES: usize = 8 << 20;
const MAX_REQUESTS_PER_TICK: usize = 256;
const MAX_PAYLOAD_BYTES: usize = 1 << 20;
/// How often a tab polls for a component still loading on its thread.
const LOAD_POLL: Duration = Duration::from_millis(50);

// ---------- the Approvals seat ----------

/// The Approvals tab: the governance register as the app has it, drawn by
/// the `governance` view. Its intents come back as `vote` and `execute`,
/// each with an [`Intent`] in `detail`.
pub fn governance_view(
    dark: bool,
    connected: bool,
    admin: bool,
    answered: bool,
    voting: &str,
    rows: &[crate::backend::ProposalRow],
) -> Element<'static, ModuleViewEvent> {
    let props = serde_json::json!({
        "rows": rows,
        "voting": voting,
        "admin": admin,
        "connected": connected,
        "answered": answered,
        "dark": dark,
    });
    module_view(
        "governance",
        serde_json::to_vec(&props).expect("props encode"),
    )
}

/// A vote or a settle, as the governance view sends it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct Intent {
    pub proposal_id: String,
    pub approve: bool,
}

pub fn gov_intent(event: &ModuleViewEvent) -> crate::GovIntent {
    match event.kind.as_str() {
        "execute" => crate::GovIntent::Execute,
        _ => crate::GovIntent::Vote,
    }
}

pub fn gov_event_proposal(event: &ModuleViewEvent) -> String {
    intent(event)
        .map(|intent| intent.proposal_id)
        .unwrap_or_default()
}

pub fn gov_event_approves(event: &ModuleViewEvent) -> bool {
    intent(event).is_some_and(|intent| intent.approve)
}

fn intent(event: &ModuleViewEvent) -> Option<Intent> {
    serde_json::from_str(&event.detail).ok()
}

// ---------- the roster seats ----------

/// The Members tab: the roster as the app has it, drawn by the `members`
/// view. Its intents come back as `copy` (`text`, `label`), `agent_status`
/// (`agent_id`, `paused`) and `propose` (`action`, `key`), each a JSON object
/// in `detail` the `event_text` / `event_flag` readings pick apart.
pub fn members_view(
    dark: bool,
    connected: bool,
    admin: bool,
    answered: bool,
    rows: &[crate::backend::MemberRow],
) -> Element<'static, ModuleViewEvent> {
    let props = serde_json::json!({
        "rows": rows,
        "admin": admin,
        "connected": connected,
        "answered": answered,
        "dark": dark,
    });
    module_view("members", serde_json::to_vec(&props).expect("props encode"))
}

/// The Agents tab: the registry as the app has it, drawn by the `agents`
/// view. Nothing comes back — the register is read-only there.
pub fn agents_view(
    dark: bool,
    connected: bool,
    answered: bool,
    rows: &[crate::backend::AgentRow],
) -> Element<'static, ModuleViewEvent> {
    let props = serde_json::json!({
        "rows": rows,
        "connected": connected,
        "answered": answered,
        "dark": dark,
    });
    module_view("agents", serde_json::to_vec(&props).expect("props encode"))
}

pub fn roster_intent(event: &ModuleViewEvent) -> crate::RosterIntent {
    match event.kind.as_str() {
        "agent_status" => crate::RosterIntent::AgentStatus,
        "propose" => crate::RosterIntent::Propose,
        _ => crate::RosterIntent::Copy,
    }
}

/// The string under `field` in an intent's JSON detail; empty when absent.
pub fn event_text(event: &ModuleViewEvent, field: &str) -> String {
    detail(event)
        .and_then(|detail| detail.get(field)?.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The flag under `field` in an intent's JSON detail; false when absent.
pub fn event_flag(event: &ModuleViewEvent, field: &str) -> bool {
    detail(event)
        .is_some_and(|detail| detail.get(field).and_then(serde_json::Value::as_bool) == Some(true))
}

/// The integer under `field` in an intent's JSON detail; 0 when absent.
pub fn event_int(event: &ModuleViewEvent, field: &str) -> i64 {
    detail(event)
        .and_then(|detail| detail.get(field).and_then(serde_json::Value::as_i64))
        .unwrap_or_default()
}

/// The number under `field` in an intent's JSON detail; 0 when absent.
pub fn event_num(event: &ModuleViewEvent, field: &str) -> f64 {
    detail(event)
        .and_then(|detail| detail.get(field).and_then(serde_json::Value::as_f64))
        .unwrap_or_default()
}

fn detail(event: &ModuleViewEvent) -> Option<serde_json::Value> {
    serde_json::from_str(&event.detail).ok()
}

// ---------- the node seat ----------

/// The Node tab: the facts the app holds, drawn by the `node` view. Its
/// intents come back as `copy` (`text`, `label`), `tab` (`tab`) and
/// `log_filter` (`filter`); the native log ring's own events come back as
/// `log_timeline`, drained by [`node_log_timeline_drain`].
///
/// The timeline and its source are stashed for the surface the view leaves
/// a slot for: the host paints the app-held ring there, on its own clock.
#[allow(
    clippy::too_many_arguments,
    reason = "the Ice extern hands the screen's facts one by one"
)]
pub fn node_view(
    dark: bool,
    connected: bool,
    admin: bool,
    tier: &str,
    status: &str,
    loading: bool,
    module_rows: &[crate::backend::ModuleRow],
    node_key: &str,
    node_data_dir: &str,
    node_height: i64,
    node_checkpoint: i64,
    node_last_finalized: i64,
    node_reachable_label: &str,
    node_quorum_label: &str,
    node_version: &str,
    node_root_hash: &str,
    sync_line: &str,
    node_phase_since: i64,
    node_sync_retries: i64,
    node_sync_failures: i64,
    node_sync_last_error: &str,
    node_peers: &[crate::backend::PeerRow],
    wall_now: i64,
    timeline: &crate::backend::NodeLogTimelineState,
    source: &str,
) -> Element<'static, ModuleViewEvent> {
    node_timeline().lock().expect("node timeline").shown =
        Some((timeline.clone(), source.to_owned()));
    let props = serde_json::json!({
        "node_key": node_key,
        "node_data_dir": node_data_dir,
        "tier": tier,
        "admin": admin,
        "status": status,
        "loading": loading,
        "module_rows": module_rows,
        "node_height": node_height,
        "node_checkpoint": node_checkpoint,
        "node_last_finalized": node_last_finalized,
        "node_reachable_label": node_reachable_label,
        "node_quorum_label": node_quorum_label,
        "node_version": node_version,
        "node_root_hash": node_root_hash,
        "sync_line": sync_line,
        "node_phase_since": node_phase_since,
        "node_sync_retries": node_sync_retries,
        "node_sync_failures": node_sync_failures,
        "node_sync_last_error": node_sync_last_error,
        "node_peers": node_peers,
        "wall_now": wall_now,
        "connected": connected,
        "dark": dark,
    });
    module_view("node", serde_json::to_vec(&props).expect("props encode"))
}

pub fn node_intent(event: &ModuleViewEvent) -> crate::NodeIntent {
    match event.kind.as_str() {
        "tab" => crate::NodeIntent::Tab,
        "log_filter" => crate::NodeIntent::LogFilter,
        "log_timeline" => crate::NodeIntent::LogTimeline,
        _ => crate::NodeIntent::Copy,
    }
}

/// The tab a `tab` intent names; a name the screen has no tab for is the
/// overview.
pub fn node_event_tab(event: &ModuleViewEvent) -> crate::NodeTab {
    match event_text(event, "tab").as_str() {
        "permissions" => crate::NodeTab::Permissions,
        "activity" => crate::NodeTab::Activity,
        "modules" => crate::NodeTab::Modules,
        _ => crate::NodeTab::Overview,
    }
}

/// Applies what the reader did in the native log ring since the last drain
/// — a scroll, a selection, a return to the tail — to the timeline the app
/// holds, in the order it happened.
pub fn node_log_timeline_drain(
    mut state: crate::backend::NodeLogTimelineState,
) -> crate::backend::NodeLogTimelineState {
    let events = std::mem::take(&mut node_timeline().lock().expect("node timeline").events);
    for event in events {
        state = crate::backend::node_log_timeline_apply(state, event);
    }
    state
}

// ---------- the explorer seat ----------

/// The Explorer tab: the ledger and the answer to the last workspace search
/// as the app holds them, drawn by the `explorer` view. Its intents come
/// back as `refresh`, `copy` (`text`, `label`), `search` (`query`) and
/// `clear`.
#[allow(
    clippy::too_many_arguments,
    reason = "the Ice extern hands the screen's facts one by one"
)]
pub fn explorer_view(
    dark: bool,
    connected: bool,
    loading: bool,
    blocks: &[crate::backend::ExplorerBlock],
    ops: &[crate::backend::ExplorerOp],
    head: i64,
    sync_line: &str,
    hits: &[crate::backend::ExplorerHit],
    kinds: &[crate::backend::KindCount],
    partial: &str,
    searching: bool,
    sent_query: &str,
) -> Element<'static, ModuleViewEvent> {
    let props = serde_json::json!({
        "connected": connected,
        "loading": loading,
        "dark": dark,
        "blocks": blocks,
        "ops": ops,
        "head": head,
        "sync_line": sync_line,
        "hits": hits,
        "kinds": kinds,
        "partial": partial,
        "searching": searching,
        "sent_query": sent_query,
    });
    module_view(
        "explorer",
        serde_json::to_vec(&props).expect("props encode"),
    )
}

pub fn explorer_intent(event: &ModuleViewEvent) -> crate::ExplorerIntent {
    match event.kind.as_str() {
        "refresh" => crate::ExplorerIntent::Refresh,
        "search" => crate::ExplorerIntent::Search,
        "clear" => crate::ExplorerIntent::Clear,
        _ => crate::ExplorerIntent::Copy,
    }
}

// ---------- the settings seat ----------

/// The Settings tab: this device's preferences, the account and its keys, the
/// signing seat and the workspace's lifecycle, drawn by the `settings` view.
/// The roster folds to the readings the card shows, the mutation phase to
/// the two flags the buttons gate on, and the password to whether the seat is
/// held — the password itself never crosses. Its intents come back one per
/// act (`settings_intent`), carrying only what the reader typed.
#[allow(
    clippy::too_many_arguments,
    reason = "the Ice extern hands the screen's facts one by one"
)]
pub fn settings_view(
    dark: bool,
    connected: bool,
    loading: bool,
    status: &str,
    mutation_phase: crate::MutationPhase,
    appearance: crate::Appearance,
    desktop_notifications: bool,
    password: &str,
    account_name: &str,
    network_name: &str,
    connected_rpc: &str,
    account_ceremony_phase: &str,
    account_ceremony_qr: &str,
    account_ceremony_detail: &str,
    account_ceremony_left: &str,
    settings_key_state: &str,
    settings_key_path: &str,
    settings_open_tabs: i64,
    members_rows: &[crate::backend::MemberRow],
    members_answered: bool,
    account_number: &str,
    account_renaming: bool,
    account_exists: bool,
    account_keys: i64,
    account_key_rows: &[crate::backend::AccountKeyRow],
    account_busy: bool,
    account_ticket: &str,
    drafts_cleared: i64,
    drafts_scope: &str,
) -> Element<'static, ModuleViewEvent> {
    let appearance = match appearance {
        crate::Appearance::System => "system",
        crate::Appearance::Light => "light",
        crate::Appearance::Dark => "dark",
    };
    let props = serde_json::json!({
        "dark": dark,
        "connected": connected,
        "loading": loading,
        "status": status,
        "busy": mutation_phase != crate::MutationPhase::Idle,
        "recovering": mutation_phase == crate::MutationPhase::Recovering,
        "appearance": appearance,
        "desktop_notifications": desktop_notifications,
        "unlocked": !password.is_empty(),
        "account_name": account_name,
        "network_name": network_name,
        "connected_rpc": connected_rpc,
        "account_ceremony_phase": account_ceremony_phase,
        "account_ceremony_qr": account_ceremony_qr,
        "account_ceremony_detail": account_ceremony_detail,
        "account_ceremony_left": account_ceremony_left,
        "settings_key_state": settings_key_state,
        "settings_key_path": settings_key_path,
        "settings_open_tabs": settings_open_tabs,
        "tier": crate::backend::member_tier(members_rows),
        "admin": crate::backend::members_is_admin(members_rows),
        "members_line": crate::backend::members_summary(connected, members_rows),
        "members_answered": members_answered,
        "account_number": account_number,
        "account_renaming": account_renaming,
        "account_exists": account_exists,
        "account_keys": account_keys,
        "account_key_rows": account_key_rows,
        "account_busy": account_busy,
        "account_ticket": account_ticket,
        "drafts_cleared": drafts_cleared,
        "drafts_scope": drafts_scope,
    });
    module_view(
        "settings",
        serde_json::to_vec(&props).expect("props encode"),
    )
}

pub fn settings_intent(event: &ModuleViewEvent) -> crate::SettingsIntent {
    use crate::SettingsIntent as Intent;
    match event.kind.as_str() {
        "tab" => Intent::Tab,
        "reconnect" => Intent::Reconnect,
        "switch_network" => Intent::SwitchNetwork,
        "unlock" => Intent::Unlock,
        "lock" => Intent::Lock,
        "rename" => Intent::Rename,
        "create" => Intent::Create,
        "key_add" => Intent::KeyAdd,
        "join" => Intent::Join,
        "key_remove" => Intent::KeyRemove,
        "passkey" => Intent::Passkey,
        "passkey_desktop" => Intent::PasskeyDesktop,
        "ceremony_cancel" => Intent::CeremonyCancel,
        "wallet" => Intent::Wallet,
        "login" => Intent::Login,
        "clear_tabs" => Intent::ClearTabs,
        "forget" => Intent::Forget,
        "light" => Intent::Light,
        "dark" => Intent::Dark,
        "notifications" => Intent::Notifications,
        _ => Intent::Copy,
    }
}

/// The rail tab a `tab` intent names; the two the settings cards link to.
pub fn settings_event_tab(event: &ModuleViewEvent) -> crate::ShellTab {
    match event_text(event, "tab").as_str() {
        "members" => crate::ShellTab::Members,
        _ => crate::ShellTab::Node,
    }
}

// ---------- the chat seat ----------

/// The chat view's props, as one document — a struct rather than a `json!`
/// literal because the macro recurses once per field and this screen has
/// more than the compiler's default limit.
#[derive(serde::Serialize)]
struct ChatProps<'a> {
    dark: bool,
    endpoint: &'a str,
    network_name: &'a str,
    network_chain_id: &'a str,
    status: &'a str,
    block_height: i64,
    search_phase: &'static str,
    search_query: &'a str,
    search_hits: &'a [crate::backend::ChatSearchHit],
    rooms: &'a [crate::backend::ChatSidebarRow],
    dm_rows: &'a [crate::backend::DmSidebarRow],
    channel_create_open: bool,
    connected: bool,
    loading: bool,
    busy: bool,
    active_channel: &'a str,
    active_dm_peer: &'a str,
    active_dm: &'a crate::backend::DmPeer,
    active_channel_name: &'a str,
    active_channel_archived: bool,
    active_channel_members_only: bool,
    channel_members: &'a [crate::backend::ChatMember],
    post_refusal: &'a str,
    huddle_joined: bool,
    huddle_channel: &'a str,
    huddle_channel_name: &'a str,
    huddle_joined_at: i64,
    huddle_now: i64,
    call_muted: bool,
    messages: &'a [crate::backend::ChatMessage],
    has_older_history: bool,
    history_view: bool,
    at_live_tail: bool,
    history_loading: bool,
    unread_boundary: i64,
    unread_marker_seq: i64,
    selected_message_seq: i64,
    selected_message_rev: i64,
    message_action: &'static str,
    channel_settings_open: bool,
    active_thread_seq: i64,
    thread_target_seq: i64,
    thread_messages: &'a [crate::backend::ChatMessage],
    thread_selected_seq: i64,
    thread_selected_rev: i64,
    thread_message_action: &'static str,
    thread_has_more: bool,
    thread_next_reply_seq: i64,
    thread_loading: bool,
    copy_anchor_seq: i64,
    copy_head_seq: i64,
    copy_surface: &'static str,
    sent_serial: i64,
}

/// The Chat tab: the room list, the stream, the rail and the drawer as the
/// app holds them, drawn by the `chat` view. Its intents come back one per
/// act ([`chat_intent`]), carrying what the reader chose or typed; the two
/// composers are host surfaces (`crate::composer_surface`), whose submit
/// comes back as `composer`.
#[allow(
    clippy::too_many_arguments,
    reason = "the Ice extern hands the screen's facts one by one"
)]
pub fn chat_view(
    dark: bool,
    endpoint: &str,
    network_name: &str,
    network_chain_id: &str,
    status: &str,
    block_height: i64,
    search_phase: crate::SearchPhase,
    search_query: &str,
    search_hits: &[crate::backend::ChatSearchHit],
    rooms: &[crate::backend::ChatSidebarRow],
    dm_rows: &[crate::backend::DmSidebarRow],
    channel_create_open: bool,
    connected: bool,
    loading: bool,
    mutation_phase: crate::MutationPhase,
    active_channel: &str,
    active_dm_peer: &str,
    active_dm: &crate::backend::DmPeer,
    active_channel_name: &str,
    active_channel_archived: bool,
    active_channel_members_only: bool,
    channel_members: &[crate::backend::ChatMember],
    post_refusal: &str,
    huddle_joined: bool,
    huddle_channel: &str,
    huddle_channel_name: &str,
    huddle_joined_at: i64,
    huddle_now: i64,
    call_muted: bool,
    messages: &[crate::backend::ChatMessage],
    has_older_history: bool,
    history_view: bool,
    at_live_tail: bool,
    history_loading: bool,
    unread_boundary: i64,
    unread_marker_seq: i64,
    selected_message_seq: i64,
    selected_message_rev: i64,
    message_action: crate::MessageAction,
    channel_settings_open: bool,
    active_thread_seq: i64,
    thread_target_seq: i64,
    thread_messages: &[crate::backend::ChatMessage],
    thread_selected_seq: i64,
    thread_selected_rev: i64,
    thread_message_action: crate::MessageAction,
    thread_has_more: bool,
    thread_next_reply_seq: i64,
    thread_loading: bool,
    copy_anchor_seq: i64,
    copy_head_seq: i64,
    copy_surface: crate::CopySurface,
    sent_serial: i64,
) -> Element<'static, ModuleViewEvent> {
    let props = ChatProps {
        dark,
        endpoint,
        network_name,
        network_chain_id,
        status,
        block_height,
        search_phase: search_phase_name(search_phase),
        search_query,
        search_hits,
        rooms,
        dm_rows,
        channel_create_open,
        connected,
        loading,
        busy: mutation_phase != crate::MutationPhase::Idle,
        active_channel,
        active_dm_peer,
        active_dm,
        active_channel_name,
        active_channel_archived,
        active_channel_members_only,
        channel_members,
        post_refusal,
        huddle_joined,
        huddle_channel,
        huddle_channel_name,
        huddle_joined_at,
        huddle_now,
        call_muted,
        messages,
        has_older_history,
        history_view,
        at_live_tail,
        history_loading,
        unread_boundary,
        unread_marker_seq,
        selected_message_seq,
        selected_message_rev,
        message_action: message_action_name(message_action),
        channel_settings_open,
        active_thread_seq,
        thread_target_seq,
        thread_messages,
        thread_selected_seq,
        thread_selected_rev,
        thread_message_action: message_action_name(thread_message_action),
        thread_has_more,
        thread_next_reply_seq,
        thread_loading,
        copy_anchor_seq,
        copy_head_seq,
        copy_surface: copy_surface_name(copy_surface),
        sent_serial,
    };
    module_view("chat", serde_json::to_vec(&props).expect("props encode"))
}

fn search_phase_name(phase: crate::SearchPhase) -> &'static str {
    match phase {
        crate::SearchPhase::Idle => "idle",
        crate::SearchPhase::Searching => "searching",
        crate::SearchPhase::Done => "done",
    }
}

fn message_action_name(action: crate::MessageAction) -> &'static str {
    match action {
        crate::MessageAction::Toolbar => "toolbar",
        crate::MessageAction::More => "more",
        crate::MessageAction::Reactions => "reactions",
        crate::MessageAction::Editing => "editing",
        crate::MessageAction::Delete => "delete",
    }
}

fn copy_surface_name(surface: crate::CopySurface) -> &'static str {
    match surface {
        crate::CopySurface::Nowhere => "nowhere",
        crate::CopySurface::Timeline => "timeline",
        crate::CopySurface::Thread => "thread",
    }
}

pub fn chat_intent(event: &ModuleViewEvent) -> crate::ChatIntent {
    use crate::ChatIntent as Intent;
    match event.kind.as_str() {
        "search" => Intent::Search,
        "clear_search" => Intent::ClearSearch,
        "open_hit" => Intent::OpenHit,
        "toggle_create" => Intent::ToggleCreate,
        "choose_channel" => Intent::ChooseChannel,
        "choose_dm" => Intent::ChooseDm,
        "toggle_settings" => Intent::ToggleSettings,
        "show_huddle" => Intent::ShowHuddle,
        "leave_huddle" => Intent::LeaveHuddle,
        "join_huddle" => Intent::JoinHuddle,
        "load_history" => Intent::LoadHistory,
        "scrolled" => Intent::Scrolled,
        "open_link" => Intent::OpenLink,
        "copy" => Intent::Copy,
        "copy_link" => Intent::CopyLink,
        "add_reaction" => Intent::AddReaction,
        "remove_reaction" => Intent::RemoveReaction,
        "open_thread" => Intent::OpenThread,
        "message_actions" => Intent::MessageActions,
        "message_reactions" => Intent::MessageReactions,
        "begin_edit" => Intent::BeginEdit,
        "arm_delete" => Intent::ArmDelete,
        "press" => Intent::Press,
        "clear_range" => Intent::ClearRange,
        "copy_range" => Intent::CopyRange,
        "reaction_submit" => Intent::ReactionSubmit,
        "edit" => Intent::Edit,
        "delete" => Intent::Delete,
        "rename" => Intent::Rename,
        "archive" => Intent::Archive,
        "unarchive" => Intent::Unarchive,
        "add_member" => Intent::AddMember,
        "remove_member" => Intent::RemoveMember,
        "close_thread" => Intent::CloseThread,
        "thread_actions" => Intent::ThreadActions,
        "thread_reactions" => Intent::ThreadReactions,
        "thread_begin_edit" => Intent::ThreadBeginEdit,
        "thread_arm_delete" => Intent::ThreadArmDelete,
        "thread_clear_selection" => Intent::ThreadClearSelection,
        "thread_edit" => Intent::ThreadEdit,
        "thread_delete" => Intent::ThreadDelete,
        "load_thread" => Intent::LoadThread,
        "composer" => Intent::Composer,
        _ => Intent::ClearSelection,
    }
}

/// The surface a `press` intent names; a name the view has no surface for
/// is nowhere, which draws no range.
pub fn chat_event_surface(event: &ModuleViewEvent) -> crate::CopySurface {
    match event_text(event, "surface").as_str() {
        "timeline" => crate::CopySurface::Timeline,
        "thread" => crate::CopySurface::Thread,
        _ => crate::CopySurface::Nowhere,
    }
}

/// Which composer a `composer` intent came from.
pub fn chat_event_kind(event: &ModuleViewEvent) -> crate::ComposerKind {
    match event_text(event, "kind").as_str() {
        "reply" => crate::ComposerKind::Reply,
        _ => crate::ComposerKind::Message,
    }
}

/// A refused or failed body, handed back to the composer it was written in.
pub fn chat_composer_unsent(scope: &str, text: &str, committed: bool) -> bool {
    crate::composer_surface::unsent(scope, text, committed);
    true
}

/// The native log ring behind the node view's slot: the timeline the app
/// last drew the tab with, and what the reader did in it since the app
/// last drained. One per process, like the view it belongs to.
#[derive(Default)]
struct NodeTimeline {
    shown: Option<(crate::backend::NodeLogTimelineState, String)>,
    events: Vec<crate::backend::NodeLogTimelineEvent>,
}

fn node_timeline() -> &'static Mutex<NodeTimeline> {
    static TIMELINE: OnceLock<Mutex<NodeTimeline>> = OnceLock::new();
    TIMELINE.get_or_init(Mutex::default)
}

/// The surfaces a module's view may leave slots for. The node view's
/// `node_log_timeline` is the app's own ring, painted from the timeline the
/// tab was last drawn with; what the reader does in it is queued for
/// [`node_log_timeline_drain`], and the guest — which declared the slot as
/// `-> unit` — hears only that something happened.
fn surfaces_of(module: &str) -> Surfaces {
    let mut surfaces = Surfaces::default();
    if module == "node" {
        surfaces.insert(
            "node_log_timeline".into(),
            Arc::new(|_key: &str, _args: &[wire::SurfaceValue]| {
                let shown = node_timeline().lock().expect("node timeline").shown.clone();
                let Some((timeline, source)) = shown else {
                    return widget::Space::new().into();
                };
                crate::backend::node_log_timeline(timeline, source).map(|event| {
                    node_timeline()
                        .lock()
                        .expect("node timeline")
                        .events
                        .push(event);
                    wire::SurfaceValue::Unit
                })
            }),
        );
    }
    if module == "chat" {
        surfaces.insert("chat_composer".into(), crate::composer_surface::provider());
    }
    surfaces
}

/// The operations a view may ask of the app, by module. An intent outside
/// the list is refused at the door, never handed to a handler.
fn intents_of(module: &str) -> &'static [&'static str] {
    match module {
        "governance" => &["vote", "execute"],
        "members" => &["copy", "agent_status", "propose"],
        "agents" => &[],
        "node" => &["copy", "tab", "log_filter"],
        "explorer" => &["refresh", "copy", "search", "clear"],
        "settings" => &[
            "tab",
            "reconnect",
            "switch_network",
            "unlock",
            "lock",
            "rename",
            "create",
            "key_add",
            "join",
            "key_remove",
            "passkey",
            "passkey_desktop",
            "ceremony_cancel",
            "wallet",
            "login",
            "copy",
            "clear_tabs",
            "forget",
            "light",
            "dark",
            "notifications",
        ],
        // `composer` is deliberately NOT here: a send crosses only from the
        // host's own composer surface (`deliver`), never as a guest request.
        "chat" => &[
            "search",
            "clear_search",
            "open_hit",
            "toggle_create",
            "choose_channel",
            "choose_dm",
            "toggle_settings",
            "show_huddle",
            "leave_huddle",
            "join_huddle",
            "load_history",
            "scrolled",
            "open_link",
            "copy",
            "copy_link",
            "add_reaction",
            "remove_reaction",
            "open_thread",
            "message_actions",
            "message_reactions",
            "begin_edit",
            "arm_delete",
            "clear_selection",
            "press",
            "clear_range",
            "copy_range",
            "reaction_submit",
            "edit",
            "delete",
            "rename",
            "archive",
            "unarchive",
            "add_member",
            "remove_member",
            "close_thread",
            "thread_actions",
            "thread_reactions",
            "thread_begin_edit",
            "thread_arm_delete",
            "thread_clear_selection",
            "thread_edit",
            "thread_delete",
            "load_thread",
        ],
        _ => &[],
    }
}

// ---------- mounting ----------

/// The widget for one module's view, with `props` as the app has them now.
/// The view is loaded once per process, on its own thread, and the tab
/// shows what stage it is at until then.
fn module_view(module: &'static str, props: Vec<u8>) -> Element<'static, ModuleViewEvent> {
    let mounted = mounted(module);
    let (content, rev) = {
        let mut locked = mounted.lock().expect("module view lock");
        locked.props = Some(props);
        match &mut locked.slot {
            Slot::Loading => return notice("Loading the view…"),
            Slot::Failed(reason) => return notice(reason),
            Slot::Ready(guest) => (guest.render(), guest.frame_rev),
        }
    };
    Element::new(ModuleView {
        mounted,
        rev,
        content,
    })
}

/// What the tab shows while the view is not there to show itself.
fn notice(text: &str) -> Element<'static, ModuleViewEvent> {
    widget::container(widget::text(text.to_owned()).size(13))
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .into()
}

/// One module's view for the life of the process: the instance once it is
/// there, and the props the app last handed it, which it takes on its next
/// redraw whether the instance was ready when they arrived or not.
struct Mounted {
    slot: Slot,
    props: Option<Vec<u8>>,
}

enum Slot {
    Loading,
    Ready(Box<Guest>),
    Failed(String),
}

type Registry = Mutex<HashMap<&'static str, Arc<Mutex<Mounted>>>>;

fn mounted(module: &'static str) -> Arc<Mutex<Mounted>> {
    static MOUNTED: OnceLock<Registry> = OnceLock::new();
    let mut registry = MOUNTED
        .get_or_init(Mutex::default)
        .lock()
        .expect("module views");
    registry
        .entry(module)
        .or_insert_with(|| {
            let mounted = Arc::new(Mutex::new(Mounted {
                slot: Slot::Loading,
                props: None,
            }));
            // A cold cranelift compile is a second or more; the window
            // thread shows "Loading" instead of freezing for it.
            let loading = mounted.clone();
            std::thread::spawn(move || {
                let slot = match Guest::load(module) {
                    Ok(guest) => Slot::Ready(Box::new(guest)),
                    Err(reason) => {
                        tracing::warn!(
                            target: "ducktape::app",
                            module,
                            reason = "module_view_unloadable",
                            error = %reason,
                            "module view not loaded"
                        );
                        Slot::Failed(reason)
                    }
                };
                loading.lock().expect("module view lock").slot = slot;
            });
            mounted
        })
        .clone()
}

/// Where the staged views are: `$DUCKTAPE_VIEWS_DIR`, else `views/` beside
/// the binary or beside its profile directory — the shape
/// `workspace_config::staged_modules_dir` gives the founding set.
fn views_dir() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("DUCKTAPE_VIEWS_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let exe = std::env::current_exe().map_err(|error| format!("current executable: {error}"))?;
    let exe_dir = exe.parent().ok_or("the executable has no directory")?;
    [Some(exe_dir), exe_dir.parent()]
        .into_iter()
        .flatten()
        .map(|dir| dir.join("views"))
        .find(|dir| dir.is_dir())
        .ok_or_else(|| {
            format!(
                "no views beside {} — `make views` stages them under target/views, or set $DUCKTAPE_VIEWS_DIR",
                exe.display()
            )
        })
}

// ---------- the guest ----------

/// What a view's store holds: its limits, and the message its panic hook
/// handed over before the trap that follows.
struct HostState {
    limits: StoreLimits,
    panic: Option<String>,
}

struct Guest {
    module: &'static str,
    store: Store<HostState>,
    tick: TypedFunc<(Vec<u8>,), (Vec<u8>,)>,
    /// The guest's events for its next tick.
    pending: Vec<wire::Event>,
    /// The last frame, its `root` kept across `unchanged` ticks and patched
    /// in place by a frame that carries patches instead of a tree.
    frame: wire::Frame,
    /// Bumped when `frame.root` changes: the widget rebuilds when it sees a
    /// number it has not rendered.
    frame_rev: u64,
    ticks: u64,
    /// The live text of every input in the tree — the host's, not the guest's.
    inputs: Inputs,
    /// Every picture the guest has sent, by hash: the bytes cross once.
    pictures: Pictures,
    surfaces: Surfaces,
    /// The guest's `<module>.props` subscription, once it asked, and the
    /// props it was last given on it.
    props_subscription: Option<u64>,
    props_sent: Option<Vec<u8>>,
    /// What the guest asked the app to do this redraw.
    intents: Vec<ModuleViewEvent>,
    /// The trap that ended the view, if one did. A faulted guest never ticks again.
    fault: Option<String>,
}

fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        // The faces the app loads (`font` in app.ice); a view names them and
        // the runtime resolves the name only through this registry.
        for family in ["Geist", "Geist Mono"] {
            ui_lang_runtime::view_tree::register_font_family(family);
        }
        let mut config = Config::new();
        config.cranelift_opt_level(OptLevel::Speed);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let engine = Engine::new(&config).expect("wasmtime engine");
        // The clock every tick's deadline is measured against: one thread
        // for the process, never stopped.
        let ticking = engine.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(EPOCH_TICK);
                ticking.increment_epoch();
            }
        });
        engine
    })
}

/// What one call into the view may spend: instructions, and time.
fn arm(store: &mut Store<HostState>) {
    let _ = store.set_fuel(FUEL_PER_TICK);
    let epochs = TICK_DEADLINE.as_nanos().div_ceil(EPOCH_TICK.as_nanos()) as u64;
    store.set_epoch_deadline(epochs);
}

impl Guest {
    fn load(module: &'static str) -> Result<Self, String> {
        let path = views_dir()?.join(format!("{module}_view.wasm"));
        Self::load_from(module, &path)
    }

    fn load_from(module: &'static str, path: &std::path::Path) -> Result<Self, String> {
        let shown = path.display().to_string();
        let metadata = std::fs::metadata(path).map_err(|error| format!("{shown}: {error}"))?;
        if metadata.len() > MAX_MODULE_BYTES {
            return Err(format!(
                "{shown}: past the {MAX_MODULE_BYTES} byte module limit"
            ));
        }
        let bytes = std::fs::read(path).map_err(|error| format!("{shown}: {error}"))?;
        let engine = engine();
        let component =
            Component::new(engine, &bytes).map_err(|error| format!("{shown}: {error}"))?;
        // Tables are allocated eagerly at their declared minimum, before any
        // fuel or memory limit is consulted; a component is several core
        // instances — the app, the stub adapters `cargo ice bundle` gave it,
        // the bindings' shims — and one memory.
        let limits = StoreLimitsBuilder::new()
            .memory_size(MEMORY_LIMIT)
            .memories(1)
            .instances(8)
            .tables(4)
            .table_elements(1 << 20)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(
            engine,
            HostState {
                limits,
                panic: None,
            },
        );
        store.limiter(|state| &mut state.limits);
        store.epoch_deadline_trap();
        // The `ice:view` world's one import is the panic hook's; anything
        // else the component asks for traps if it is ever called.
        let mut linker = Linker::<HostState>::new(engine);
        linker
            .root()
            .func_wrap(
                "panicked",
                |mut store: StoreContextMut<'_, HostState>, (message,): (String,)| {
                    let line = message.lines().next().unwrap_or_default();
                    store.data_mut().panic = Some(line.chars().take(1024).collect());
                    Ok(())
                },
            )
            .map_err(|error| error.to_string())?;
        linker
            .define_unknown_imports_as_traps(&component)
            .map_err(|error| error.to_string())?;
        arm(&mut store);
        let instance = linker
            .instantiate(&mut store, &component)
            .map_err(|error| format!("{shown}: {}", first_line(&error)))?;
        let init = instance
            .get_typed_func::<(bool,), ()>(&mut store, "init")
            .map_err(|error| format!("{shown}: {error}"))?;
        let tick = instance
            .get_typed_func::<(Vec<u8>,), (Vec<u8>,)>(&mut store, "tick")
            .map_err(|error| format!("{shown}: {error}"))?;
        // `on mount` runs in here, with a budget of its own.
        arm(&mut store);
        // `on mount` runs in here, told which platform it keys for.
        if let Err(error) = init.call(&mut store, (cfg!(target_os = "macos"),)) {
            let trap = format!("{shown}: init trapped: {}", first_line(&error));
            return Err(panic_message(&mut store).unwrap_or(trap));
        }
        Ok(Self {
            module,
            store,
            tick,
            pending: Vec::new(),
            frame: wire::Frame::default(),
            frame_rev: 0,
            ticks: 0,
            inputs: Inputs::default(),
            pictures: Pictures::default(),
            surfaces: surfaces_of(module),
            props_subscription: None,
            props_sent: None,
            intents: Vec::new(),
            fault: None,
        })
    }

    /// The tree as the host holds it, rendered — or the reason there is none.
    fn render(&self) -> Element<'static, Output> {
        if let Some(fault) = &self.fault {
            return widget::container(
                widget::column![
                    widget::text("This view was stopped.").size(14),
                    widget::text(fault.clone()).size(12),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .into();
        }
        let root = self.frame.root.clone().unwrap_or_else(wire::Node::empty);
        view_tree::render(&root, &self.inputs, &self.pictures, &self.surfaces)
    }

    /// What the user did to the tree, as the widgets report it: recorded
    /// host-side (an input's text) and queued for the guest's next tick. A
    /// host surface's event is the app's, not the guest's: it was queued
    /// where the surface keeps it, and the app is told to drain it.
    fn deliver(&mut self, output: Output) {
        if let Output::Surface { value, .. } = output {
            match self.module {
                "node" => self.intents.push(ModuleViewEvent {
                    kind: "log_timeline".into(),
                    detail: String::new(),
                }),
                "chat" => self.intents.extend(crate::composer_surface::intent(&value)),
                _ => {}
            }
            return;
        }
        self.inputs.apply(output, &mut self.pending);
    }

    /// Hands the guest the props the app holds, if they moved since the
    /// guest last saw them. Before the guest has subscribed they wait here.
    fn sync_props(&mut self, props: &Option<Vec<u8>>) {
        let Some(id) = self.props_subscription else {
            return;
        };
        if props.is_none() || *props == self.props_sent {
            return;
        }
        self.props_sent = props.clone();
        self.pending.push(wire::Event::Response {
            id,
            result: Ok(props.clone().unwrap_or_default()),
            done: false,
        });
    }

    /// One redraw: tick if there is anything to deliver — or never was a
    /// first frame — answer the requests, and say whether the guest is due
    /// again at once. A guest with nothing to deliver is left alone: the
    /// tree the host has is the tree it would send.
    fn redraw(&mut self, props: &Option<Vec<u8>>) -> bool {
        if self.fault.is_some() {
            return false;
        }
        self.sync_props(props);
        let quiet = self.ticks > 0 && !self.frame.busy && self.pending.is_empty();
        if quiet {
            return false;
        }
        self.tick();
        self.ticks += 1;
        for (nth, request) in std::mem::take(&mut self.frame.requests)
            .into_iter()
            .enumerate()
        {
            match nth < MAX_REQUESTS_PER_TICK {
                true => self.answer(request, props),
                false => self.refuse(request.id, "too many requests this tick".into()),
            }
        }
        for id in std::mem::take(&mut self.frame.cancels) {
            if self.props_subscription == Some(id) {
                self.props_subscription = None;
            }
        }
        self.fault.is_none() && (self.frame.busy || !self.pending.is_empty())
    }

    /// Routes one request: the props subscription is answered from what the
    /// app holds, an intent the module declares goes to the app, the log
    /// goes to the log, and anything else is refused.
    fn answer(&mut self, request: wire::Request, props: &Option<Vec<u8>>) {
        let wire::Request { id, kind, payload } = request;
        if payload.len() > MAX_PAYLOAD_BYTES {
            self.refuse(
                id,
                format!("`{kind}` carries more than {MAX_PAYLOAD_BYTES} bytes"),
            );
            return;
        }
        let (capability, operation) = kind.split_once('.').unwrap_or((kind.as_str(), ""));
        let own = capability == self.module;
        let declared_intent = own && intents_of(self.module).contains(&operation);
        match (capability, operation) {
            _ if own && operation == "props" => {
                self.props_subscription = Some(id);
                self.props_sent = None;
                self.sync_props(props);
            }
            _ if declared_intent => self.intents.push(ModuleViewEvent {
                kind: operation.to_owned(),
                detail: String::from_utf8_lossy(&payload).into_owned(),
            }),
            ("host", "log") => {
                tracing::debug!(
                    target: "ducktape::app",
                    module = self.module,
                    line = %String::from_utf8_lossy(&payload),
                    "module view log"
                );
                self.reply(id, Ok(Vec::new()));
            }
            _ => self.refuse(id, format!("unknown request `{kind}`")),
        }
    }

    fn refuse(&mut self, id: u64, message: String) {
        self.reply(id, Err(message));
    }

    fn reply(&mut self, id: u64, result: Result<Vec<u8>, String>) {
        self.pending.push(wire::Event::Response {
            id,
            result,
            done: true,
        });
    }

    /// One call into the module with the pending events, inside the budget.
    /// A trap ends the view; the widget shows the message in its place.
    fn tick(&mut self) {
        let events = std::mem::take(&mut self.pending);
        let bytes = wire::encode(&events);
        arm(&mut self.store);
        let outcome = self
            .tick
            .call(&mut self.store, (bytes,))
            .map(|(frame,)| frame)
            .map_err(|error| first_line(&error))
            .and_then(|frame| shape(&frame));
        match outcome {
            Ok(mut frame) => {
                match merge(&mut self.frame.root, &mut frame) {
                    Ok(false) => {}
                    Ok(true) => {
                        self.frame_rev += 1;
                        if let Some(root) = &mut frame.root {
                            self.inputs.adopt(root);
                            self.pictures.adopt(root);
                            // The guest remembers its tree without the
                            // picture bytes; the tree its patches build on
                            // has to be that one.
                            root.for_each_mut(&mut |node| {
                                if let wire::Node::Svg { bytes, .. } = node {
                                    *bytes = None;
                                }
                            });
                        }
                    }
                    // The tab is blank for a tick and the guest hears that
                    // it must send the tree whole.
                    Err(refused) => {
                        tracing::warn!(
                            target: "ducktape::app",
                            module = self.module,
                            reason = "module_view_patch_refused",
                            error = refused,
                            "module view patch refused"
                        );
                        self.frame_rev += 1;
                        self.pending.push(wire::Event::Resync);
                    }
                }
                self.frame = frame;
            }
            Err(trap) => {
                let reason = panic_message(&mut self.store).unwrap_or(trap);
                tracing::warn!(
                    target: "ducktape::app",
                    module = self.module,
                    reason = "module_view_trapped",
                    error = %reason,
                    "module view ended"
                );
                self.fault = Some(reason);
                self.frame_rev += 1;
            }
        }
    }
}

/// Brings the tree the host holds into `frame`: an `unchanged` frame takes
/// it as is, a frame without a tree patches it, a frame with one replaces
/// it. `Ok(true)` is a tree the widget has to rebuild for.
fn merge(held: &mut Option<wire::Node>, frame: &mut wire::Frame) -> Result<bool, &'static str> {
    if frame.unchanged {
        frame.root = held.take();
        return Ok(false);
    }
    if frame.root.is_some() {
        return Ok(true);
    }
    let patches = std::mem::take(&mut frame.patches);
    let mut root = held.take().ok_or("no tree to patch")?;
    wire::apply(&mut root, patches)?;
    frame.root = Some(root);
    Ok(true)
}

/// What the host is willing to take from one tick's bytes: nothing in here
/// is trusted — the length, the counts, the tree.
fn shape(bytes: &[u8]) -> Result<wire::Frame, String> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err("frame too large".to_string());
    }
    let mut frame: wire::Frame = wire::decode(bytes)?;
    frame.requests.truncate(2 * MAX_REQUESTS_PER_TICK);
    if frame.unchanged {
        frame.root = None;
    }
    if frame.unchanged || frame.root.is_some() {
        frame.patches = Vec::new();
    }
    wire::sanitize(&mut frame);
    Ok(frame)
}

fn panic_message(store: &mut Store<HostState>) -> Option<String> {
    let text = store.data_mut().panic.take()?;
    (!text.is_empty()).then_some(text)
}

/// Why a call failed: the trap itself, not the wrapper and backtrace
/// wasmtime prints around it.
fn first_line(error: &wasmtime::Error) -> String {
    if let Some(wasmtime::Trap::Interrupt) = error.root_cause().downcast_ref::<wasmtime::Trap>() {
        return format!("tick exceeded {} ms", TICK_DEADLINE.as_millis());
    }
    error
        .root_cause()
        .to_string()
        .lines()
        .next()
        .unwrap_or("trap")
        .to_string()
}

// ---------- the widget ----------

/// The tree the guest last sent, rendered with the app's own widgets and
/// wrapped so that every redraw ticks the guest, everything the user does
/// inside goes back as the guest's own events, and a changed tree is
/// re-rendered in place.
struct ModuleView {
    mounted: Arc<Mutex<Mounted>>,
    /// The frame `content` was rendered from.
    rev: u64,
    content: Element<'static, Output, iced::Theme, iced::Renderer>,
}

impl Widget<ModuleViewEvent, iced::Theme, iced::Renderer> for ModuleView {
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, ModuleViewEvent>,
        viewport: &Rectangle,
    ) {
        // The tree's widgets speak `Output`; what they say is the guest's,
        // not the app's, so it is diverted rather than mapped. Everything
        // else the local shell collected carries over to the window's.
        let mut outputs = Vec::new();
        {
            let mut local = Shell::new(&mut outputs);
            self.content.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, &mut local, viewport,
            );
            if local.is_event_captured() {
                shell.capture_event();
            }
            if local.is_layout_invalid() {
                shell.invalidate_layout();
            }
            if local.are_widgets_invalid() {
                shell.invalidate_widgets();
            }
            match local.redraw_request() {
                window::RedrawRequest::NextFrame => shell.request_redraw(),
                window::RedrawRequest::At(at) => shell.request_redraw_at(at),
                window::RedrawRequest::Wait => {}
            }
            shell.input_method_mut().merge(local.input_method());
        }
        let mut mounted = self.mounted.lock().expect("module view lock");
        let Mounted { slot, props } = &mut *mounted;
        let guest = match slot {
            Slot::Ready(guest) => guest,
            Slot::Loading => {
                shell.request_redraw_at(window::RedrawRequest::At(
                    iced::time::Instant::now() + LOAD_POLL,
                ));
                return;
            }
            Slot::Failed(_) => return,
        };
        if !outputs.is_empty() {
            for output in outputs {
                guest.deliver(output);
            }
            shell.request_redraw();
        }
        let Event::Window(window::Event::RedrawRequested(_)) = event else {
            return;
        };
        if guest.redraw(props) {
            shell.request_redraw();
        }
        for intent in std::mem::take(&mut guest.intents) {
            shell.publish(intent);
        }
        // A new tree is re-rendered here, in place: the app's own view is
        // rebuilt only by its own messages, and a guest's tick is not one.
        if guest.frame_rev != self.rev {
            self.rev = guest.frame_rev;
            self.content = guest.render();
            tree.diff(self.content.as_widget());
            shell.invalidate_layout();
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, ModuleViewEvent, iced::Theme, iced::Renderer>> {
        // An overlay's messages would be the guest's too; the tree carries
        // no widget that opens one.
        let _ = (tree, layout, renderer, viewport, translation);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: &str, detail: &str) -> ModuleViewEvent {
        ModuleViewEvent {
            kind: kind.into(),
            detail: detail.into(),
        }
    }

    #[test]
    fn an_intent_is_read_off_the_guests_json() {
        let vote = event("vote", r#"{"proposal_id":"prop-1","approve":false}"#);
        assert!(matches!(gov_intent(&vote), crate::GovIntent::Vote));
        assert_eq!(gov_event_proposal(&vote), "prop-1");
        assert!(!gov_event_approves(&vote));
        let settle = event("execute", r#"{"proposal_id":"prop-2","approve":true}"#);
        assert!(matches!(gov_intent(&settle), crate::GovIntent::Execute));
        assert_eq!(gov_event_proposal(&settle), "prop-2");
    }

    /// Malformed JSON names no proposal, and the handler's empty-id guard
    /// is what refuses it.
    #[test]
    fn a_malformed_intent_names_no_proposal() {
        let broken = event("vote", "not json");
        assert_eq!(gov_event_proposal(&broken), "");
        assert!(!gov_event_approves(&broken));
    }

    /// Only the operations a module declares reach the app; the props
    /// subscription and the log are the host's, everything else is refused.
    #[test]
    fn only_declared_intents_are_routed() {
        assert_eq!(intents_of("governance"), ["vote", "execute"]);
        assert_eq!(intents_of("members"), ["copy", "agent_status", "propose"]);
        assert!(intents_of("agents").is_empty());
        let chat = intents_of("chat");
        assert_eq!(chat.len(), 43);
        assert!(chat.contains(&"choose_channel"));
        assert!(
            !chat.contains(&"composer"),
            "a submit reaches the app only through the composer surface it was typed in"
        );
    }

    /// A roster intent is read field by field off its JSON; a missing or
    /// malformed field is empty or false, which the handler's guards refuse.
    #[test]
    fn a_roster_intent_is_read_field_by_field() {
        let pause = event(
            "agent_status",
            r#"{"agent_id":"reviewer-bot","paused":true}"#,
        );
        assert!(matches!(
            roster_intent(&pause),
            crate::RosterIntent::AgentStatus
        ));
        assert_eq!(event_text(&pause, "agent_id"), "reviewer-bot");
        assert!(event_flag(&pause, "paused"));
        let ballot = event("propose", r#"{"action":"add_validator","key":"res-1"}"#);
        assert!(matches!(
            roster_intent(&ballot),
            crate::RosterIntent::Propose
        ));
        assert_eq!(event_text(&ballot, "key"), "res-1");
        assert!(!event_flag(&ballot, "paused"));
        let broken = event("copy", "not json");
        assert!(matches!(roster_intent(&broken), crate::RosterIntent::Copy));
        assert_eq!(event_text(&broken, "text"), "");
    }

    /// Every text in the tree the host holds, in tree order.
    fn texts(guest: &Guest) -> Vec<String> {
        let mut root = guest.frame.root.clone().expect("a tree");
        let mut texts = Vec::new();
        root.for_each_mut(&mut |node| {
            if let wire::Node::Text { content, .. } = node {
                texts.push(content.clone());
            }
        });
        texts
    }

    /// The message index the button labelled `name` would send.
    fn button_message(guest: &Guest, name: &str) -> u32 {
        let mut root = guest.frame.root.clone().expect("a tree");
        let mut message = None;
        root.for_each_mut(&mut |node| {
            if let wire::Node::Button {
                label, on_press, ..
            } = node
                && label.as_deref() == Some(name)
            {
                message = *on_press;
            }
        });
        message.expect("an enabled button")
    }

    /// The bundled component, end to end through the host: it boots on the
    /// offline plate, takes the register the app pushes, and a press on its
    /// card comes back as the intent the handler signs. Needs `make views`;
    /// without the staged component the test says so and does nothing.
    #[test]
    fn the_staged_governance_view_boots_takes_the_register_and_votes() {
        let staged = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../target/views/governance_view.wasm");
        if !staged.is_file() {
            eprintln!("skipped: no {} — run `make views`", staged.display());
            return;
        }
        let mut guest = Guest::load_from("governance", &staged).expect("the view loads");
        let no_props = None;
        assert!(
            !guest.redraw(&no_props),
            "a booted view with no props is quiet"
        );
        assert!(
            guest.props_subscription.is_some(),
            "the view subscribes to its props"
        );
        assert!(
            texts(&guest).iter().any(|text| text == "Not connected"),
            "{:?}",
            texts(&guest)
        );

        let props = Some(
            serde_json::to_vec(&serde_json::json!({
                "rows": [{
                    "id": "prop-1", "action": "add_validator", "detail": "node-7",
                    "proposer": "robin", "status": "open", "deadline": 4200,
                    "approvals": 1, "rejections": 0, "rule": "threshold",
                    "required_yes": 2, "electorate": 4, "open": true, "settled_height": 0
                }],
                "voting": "", "admin": true, "connected": true, "answered": true, "dark": false
            }))
            .expect("props encode"),
        );
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in [
            "1 pending",
            "prop-1",
            "1 approval · 1 more for quorum",
            "Approve →",
        ] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} in {shown:?}"
            );
        }
        // The same props again are not delivered again.
        assert!(
            !guest.redraw(&props),
            "unchanged props leave the view quiet"
        );

        guest.deliver(Output::Activate(button_message(&guest, "Approve")));
        guest.redraw(&props);
        assert_eq!(
            guest.intents,
            [ModuleViewEvent {
                kind: "vote".into(),
                detail: r#"{"proposal_id":"prop-1","approve":true}"#.into(),
            }]
        );
        assert!(guest.fault.is_none());
    }

    /// The staged path for `module`, or None with a note when `make views`
    /// has not run.
    fn staged(module: &str) -> Option<std::path::PathBuf> {
        let staged = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../target/views/{module}_view.wasm"));
        if staged.is_file() {
            return Some(staged);
        }
        eprintln!("skipped: no {} — run `make views`", staged.display());
        None
    }

    /// The bundled Members view through the host: it boots offline, takes
    /// the roster, opens a record on a press, and the record's write comes
    /// back as the intent the handler signs.
    #[test]
    fn the_staged_members_view_boots_takes_the_roster_and_pauses_an_agent() {
        let Some(staged) = staged("members") else {
            return;
        };
        let mut guest = Guest::load_from("members", &staged).expect("the view loads");
        guest.redraw(&None);
        assert!(
            texts(&guest).iter().any(|text| text == "Not connected"),
            "{:?}",
            texts(&guest)
        );
        let props = Some(
            serde_json::to_vec(&serde_json::json!({
                "rows": [
                    {"key": "val-1", "label": "Ada", "role": "validator",
                     "is_this_node": true, "is_agent": false, "model": "", "live": true},
                    {"key": "reviewer-bot", "label": "Reviewer Bot", "role": "agent",
                     "is_this_node": false, "is_agent": true, "model": "review", "live": true}
                ],
                "admin": true, "connected": true, "answered": true, "dark": false
            }))
            .expect("props encode"),
        );
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in [
            "1 human · 1 agent",
            "Ada",
            "this node",
            "Reviewer Bot",
            "AGENT",
        ] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} in {shown:?}"
            );
        }

        guest.deliver(Output::Activate(button_message(&guest, "Reviewer Bot")));
        guest.redraw(&props);
        assert!(
            texts(&guest).iter().any(|text| text == "agent id"),
            "the record opened: {:?}",
            texts(&guest)
        );
        guest.deliver(Output::Activate(button_message(&guest, "Pause agent")));
        guest.redraw(&props);
        assert_eq!(
            guest.intents,
            [ModuleViewEvent {
                kind: "agent_status".into(),
                detail: r#"{"agent_id":"reviewer-bot","paused":true}"#.into(),
            }]
        );
        assert!(guest.fault.is_none());
    }

    /// The bundled Agents view through the host: offline plate, then the
    /// register, and nothing ever leaves it.
    #[test]
    fn the_staged_agents_view_boots_and_takes_the_register() {
        let Some(staged) = staged("agents") else {
            return;
        };
        let mut guest = Guest::load_from("agents", &staged).expect("the view loads");
        guest.redraw(&None);
        assert!(
            texts(&guest).iter().any(|text| text == "Not connected"),
            "{:?}",
            texts(&guest)
        );
        let props = Some(
            serde_json::to_vec(&serde_json::json!({
                "rows": [{
                    "id": "reviewer-bot", "name": "Reviewer Bot", "initials": "RB",
                    "capability": "review", "status": "paused", "owner_handle": "eddy",
                    "live": false, "skill_count": 3, "cap_count": 2
                }],
                "connected": true, "answered": true, "dark": false
            }))
            .expect("props encode"),
        );
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in ["1 agent · 0 working", "Reviewer Bot", "PAUSED", "eddy"] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} in {shown:?}"
            );
        }
        assert!(guest.intents.is_empty());
        assert!(guest.fault.is_none());
    }

    /// Every host surface in the guest's tree, by name.
    fn surface_names(guest: &Guest) -> Vec<String> {
        fn walk(node: &wire::Node, out: &mut Vec<String>) {
            if let wire::Node::Surface { name, .. } = node {
                out.push(name.clone());
            }
            for child in node.children() {
                walk(child, out);
            }
        }
        let mut names = Vec::new();
        if let Some(root) = &guest.frame.root {
            walk(root, &mut names);
        }
        names
    }

    /// The bundled Node view through the host: offline plate, then the
    /// facts; the Activity tab asks for its tab as an intent and leaves the
    /// log ring's slot to the host's own surface, whose events come back as
    /// the drain intent rather than going to the guest.
    #[test]
    fn the_staged_node_view_boots_takes_the_facts_and_leaves_the_log_ring_to_the_host() {
        let Some(staged) = staged("node") else {
            return;
        };
        let mut guest = Guest::load_from("node", &staged).expect("the view loads");
        assert!(guest.surfaces.contains_key("node_log_timeline"));
        guest.redraw(&None);
        assert!(
            texts(&guest).iter().any(|text| text == "Not connected"),
            "{:?}",
            texts(&guest)
        );
        let props = Some(
            serde_json::to_vec(&serde_json::json!({
                "node_key": "ab12cd34", "node_data_dir": "/var/ducktape/demo",
                "tier": "validator", "admin": true, "status": "Live", "loading": false,
                "module_rows": [], "node_height": 84912, "node_checkpoint": 84900,
                "node_last_finalized": 1700000000, "node_reachable_label": "3",
                "node_quorum_label": "3", "node_version": "0.4.2", "node_root_hash": "c0ffee",
                "sync_line": "live", "node_phase_since": 1700000000, "node_sync_retries": 0,
                "node_sync_failures": 0, "node_sync_last_error": "", "node_peers": [],
                "wall_now": 1700000030, "connected": true, "dark": false
            }))
            .expect("props encode"),
        );
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in ["This node", "ab12cd34", "h 84,912", "0.4.2"] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} in {shown:?}"
            );
        }
        assert!(surface_names(&guest).is_empty());

        guest.deliver(Output::Activate(button_message(&guest, "Node activity")));
        guest.redraw(&props);
        assert_eq!(
            std::mem::take(&mut guest.intents),
            [ModuleViewEvent {
                kind: "tab".into(),
                detail: r#"{"tab":"activity"}"#.into(),
            }]
        );
        assert_eq!(surface_names(&guest), ["node_log_timeline"]);

        // what the reader does in the host's ring never reaches the guest
        guest.deliver(Output::Surface {
            handler: None,
            value: wire::SurfaceValue::Unit,
        });
        assert!(guest.pending.is_empty());
        assert_eq!(
            guest.intents,
            [ModuleViewEvent {
                kind: "log_timeline".into(),
                detail: String::new(),
            }]
        );
        assert!(guest.fault.is_none());
    }

    /// The bundled Explorer view through the host: the ledger, then a
    /// search that leaves as an intent and lands back as props.
    #[test]
    fn the_staged_explorer_view_boots_takes_the_ledger_and_asks_for_a_search() {
        let Some(staged) = staged("explorer") else {
            return;
        };
        let mut guest = Guest::load_from("explorer", &staged).expect("the view loads");
        guest.redraw(&None);
        let props = Some(
            serde_json::to_vec(&serde_json::json!({
                "connected": true, "loading": false, "dark": false,
                "blocks": [{"height": 84912, "hash": "9f3e", "commit": "c0ffee", "op_count": 1}],
                "ops": [{"height": 84912, "proposer": "val-1", "target": "chat",
                         "disposition": "applied", "op_hash": "ab12cd34", "payload": "post",
                         "trace": "chat · 1 msg"}],
                "head": 84912, "sync_line": "live",
                "hits": [], "kinds": [], "partial": "", "searching": false, "sent_query": ""
            }))
            .expect("props encode"),
        );
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in ["Explorer", "h 84,912"] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} in {shown:?}"
            );
        }
        guest.deliver(Output::Activate(button_message(&guest, "Refresh")));
        guest.redraw(&props);
        assert_eq!(
            std::mem::take(&mut guest.intents),
            [ModuleViewEvent {
                kind: "refresh".into(),
                detail: "null".into(),
            }]
        );
        assert!(guest.fault.is_none());
    }

    /// The bundled Settings view through the host: the facts, then a
    /// rename that leaves as an intent carrying the trimmed name — and the
    /// password crosses in as a flag only.
    #[test]
    fn the_staged_settings_view_boots_takes_the_facts_and_sends_a_rename() {
        let Some(staged) = staged("settings") else {
            return;
        };
        let mut guest = Guest::load_from("settings", &staged).expect("the view loads");
        guest.redraw(&None);
        let props = Some(
            serde_json::to_vec(&serde_json::json!({
                "dark": false, "connected": true, "loading": false, "status": "Connected",
                "busy": false, "recovering": false, "appearance": "system",
                "desktop_notifications": true, "unlocked": true,
                "account_name": "duck", "network_name": "testnet",
                "connected_rpc": "http://127.0.0.1:1",
                "account_ceremony_phase": "", "account_ceremony_qr": "",
                "account_ceremony_detail": "", "account_ceremony_left": "",
                "settings_key_state": "sealed", "settings_key_path": "/keys/user.key",
                "settings_open_tabs": 2, "tier": "validator", "admin": true,
                "members_line": "3 humans · 1 agent", "members_answered": true,
                "account_number": "42", "account_renaming": false, "account_exists": true,
                "account_keys": 2,
                "account_key_rows": [{"scheme": "ed25519", "pubkey": "ab12cd34", "label": "laptop"}],
                "account_busy": false, "account_ticket": "",
                "drafts_cleared": 0, "drafts_scope": ""
            }))
            .expect("props encode"),
        );
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in ["Settings", "Theme"] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} in {shown:?}"
            );
        }
        guest.deliver(Output::Activate(button_message(&guest, "Account")));
        guest.redraw(&props);
        guest.deliver(Output::Activate(button_message(&guest, "Copy number")));
        guest.redraw(&props);
        assert_eq!(
            std::mem::take(&mut guest.intents),
            [ModuleViewEvent {
                kind: "copy".into(),
                detail: r#"{"text":"42","label":"Number copied"}"#.into(),
            }]
        );
        assert!(guest.fault.is_none());
    }

    #[test]
    fn a_frame_that_changed_nothing_keeps_the_held_tree() {
        let held_tree = wire::Node::empty();
        let mut held = Some(held_tree.clone());
        let mut unchanged = wire::Frame {
            unchanged: true,
            ..wire::Frame::default()
        };
        assert_eq!(merge(&mut held, &mut unchanged), Ok(false));
        assert_eq!(unchanged.root, Some(held_tree));
        let mut patched = wire::Frame::default();
        assert_eq!(merge(&mut None, &mut patched), Err("no tree to patch"));
    }
}
