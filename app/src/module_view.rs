//! Module-owned views. A screen that ships as an `ice:view` component — an
//! Ice application compiled for the `tree` target (`crates/views`) — is
//! loaded either from the deployed artifact of the module it belongs to
//! (`backend::view_source`: the registry's ACTIVE code hash, fetched and
//! verified, never a desktop substitute) or, for the desktop's own views,
//! FROM A FILE beside the binary (`make views` stages
//! `target/views/<module>_view.wasm`; `DUCKTAPE_VIEWS_DIR` overrides); it is
//! ticked inside a fuel and time budget, and drawn with the runtime's tree
//! renderer as one widget in the tab that used to hold the native screen.
//!
//! The boundary is the screen component's own contract. Its props go in as
//! JSON, one item per change, on the guest's `<module>.props` subscription;
//! its emits come out as intents the widget hands the app as
//! [`ModuleViewEvent`]s, so every write keeps going through the handler that
//! signs it today. The guest sees no key, no endpoint and no clock — a view
//! that holds none of them cannot leak one — and a view that traps shows why
//! in its place instead of taking the window with it.

mod display_budget;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector, widget, window};
use ui_lang_runtime::view_tree::{self, Inputs, Output, Pictures, Surfaces};
use ui_lang_wire as wire;
use wasmtime::component::{Component, Linker, TypedFunc};
use wasmtime::{
    Cache, CacheConfig, Config, Engine, OptLevel, Store, StoreContextMut, StoreLimits,
    StoreLimitsBuilder,
};

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

/// The Agents tab: the register as the app has it, drawn by the `agents`
/// view — every record whole, the capability tags the network announces,
/// the action vocabulary, and the signing account (`account`, its decimal
/// number) so the view offers the editor to a record's controller. Its
/// intents come back as `status` (`agent_id`, `paused`), `save` and
/// `register` (both the whole draft record as JSON, `AgentDraft`). Every
/// committed write bumps `committed`, which tells the view its drafts were
/// consumed.
#[allow(clippy::too_many_arguments)]
pub fn agents_view(
    dark: bool,
    connected: bool,
    answered: bool,
    account: &str,
    committed: i64,
    rows: &[crate::backend::AgentRow],
    capabilities: &[String],
    actions: &[String],
) -> Element<'static, ModuleViewEvent> {
    let props = serde_json::json!({
        "rows": rows,
        "capabilities": capabilities,
        "actions": actions,
        "account": account,
        "committed": committed,
        "connected": connected,
        "answered": answered,
        "dark": dark,
    });
    module_view("agents", serde_json::to_vec(&props).expect("props encode"))
}

pub fn agents_intent(event: &ModuleViewEvent) -> crate::AgentsIntent {
    match event.kind.as_str() {
        "save" => crate::AgentsIntent::Save,
        "register" => crate::AgentsIntent::Register,
        _ => crate::AgentsIntent::Status,
    }
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

fn detail(event: &ModuleViewEvent) -> Option<serde_json::Value> {
    serde_json::from_str(&event.detail).ok()
}

/// The number in one field of an intent's detail, 0 when absent or not one.
pub fn event_number(event: &ModuleViewEvent, field: &str) -> i64 {
    detail(event)
        .and_then(|detail| detail.get(field)?.as_i64())
        .unwrap_or_default()
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

// ---------- the forge seat ----------

/// The forge view's props as one document: borrowed where the app holds the
/// fact, folded where the view wants a word or a painted row.
#[derive(serde::Serialize)]
struct ForgeProps<'a> {
    dark: bool,
    connected: bool,
    org: &'a str,
    about: &'a str,
    tier: &'a str,
    network_chain_id: &'a str,
    connected_rpc: &'a str,
    repos: &'a [crate::backend::ForgeRepo],
    list_phase: &'static str,
    open_repo: &'a str,
    repo_menu: bool,
    repo_phase: &'static str,
    branches: &'a [String],
    tab: &'static str,
    items: &'a [crate::backend::ForgeItem],
    forge_item_number: i64,
    item_phase: &'static str,
    forge_item_kind: &'a str,
    forge_item_title: &'a str,
    forge_item_state: &'a str,
    forge_item_author: &'a str,
    forge_item_branches: &'a str,
    forge_item_body: &'a str,
    forge_item_blocks: &'a [crate::backend::ChatBlock],
    forge_item_files_changed: i64,
    forge_item_additions: i64,
    forge_item_deletions: i64,
    diff_rows: Vec<crate::backend::DiffLine>,
    forge_item_diff_truncated: bool,
    forge_item_merge_oid: &'a str,
    forge_item_source_oid: &'a str,
    forge_item_approvals: i64,
    forge_item_change_requests: i64,
    forge_item_reviews: &'a [crate::backend::ForgeReview],
    merge_conflicts: &'a [String],
    merge_busy: bool,
    review_verdict: &'static str,
    review_busy: bool,
    staged_comments: &'a [crate::backend::ForgeDraftComment],
    comment_cap_reached: bool,
    discussion: &'a [crate::backend::ChatMessage],
    linked_note: &'a [crate::backend::ChatMessage],
    discussion_clipped: bool,
    landed_seq: i64,
    landed_tick: i64,
    tree_path: &'a str,
    tree_rev: &'a str,
    tree_entries: &'a [crate::backend::TreeEntry],
    tree_born: bool,
    tree_truncated: bool,
    tree_phase: &'static str,
    file_path: &'a str,
    file_text: &'a str,
    file_binary: bool,
    file_truncated: bool,
    file_picture: bool,
    file_width: i64,
    file_height: i64,
    file_note: &'a str,
    file_header: &'a str,
    file_phase: &'static str,
    drafts_cleared: i64,
    drafts_scope: &'a str,
    note_scope: &'a str,
    note_blocked: bool,
}

/// The Forge tab: the register the app holds, the repo and item it has
/// open, the code browse's listing and file, and the discussion, drawn by
/// the `forge` view. The phases, the tab and the verdict cross as their
/// enum words; the patch crosses as painted rows; the optional landed note
/// as a list of at most one. Its intents come back one per act
/// (`forge_intent`), carrying only what the reader picked or typed.
#[allow(
    clippy::too_many_arguments,
    reason = "the Ice extern hands the screen's facts one by one"
)]
pub fn forge_view(
    dark: bool,
    connected: bool,
    org: &str,
    about: &str,
    tier: &str,
    network_chain_id: &str,
    connected_rpc: &str,
    repos: &[crate::backend::ForgeRepo],
    list_phase: crate::ForgePhase,
    open_repo: &str,
    repo_menu: bool,
    repo_phase: crate::ForgePhase,
    branches: &[String],
    tab: crate::ForgeTab,
    items: &[crate::backend::ForgeItem],
    item_number: i64,
    item_phase: crate::ForgePhase,
    item_kind: &str,
    item_title: &str,
    item_state: &str,
    item_author: &str,
    item_branches: &str,
    item_body: &str,
    item_blocks: &[crate::backend::ChatBlock],
    files_changed: i64,
    additions: i64,
    deletions: i64,
    diff: &str,
    diff_truncated: bool,
    merge_oid: &str,
    source_oid: &str,
    approvals: i64,
    change_requests: i64,
    reviews: &[crate::backend::ForgeReview],
    merge_conflicts: &[String],
    merge_busy: bool,
    review_verdict: crate::ForgeReviewVerdict,
    review_busy: bool,
    staged_comments: &[crate::backend::ForgeDraftComment],
    discussion: &[crate::backend::ChatMessage],
    linked_note: Option<crate::backend::ChatMessage>,
    landed_seq: i64,
    landed_tick: i64,
    tree_path: &str,
    tree_rev: &str,
    tree_entries: &[crate::backend::TreeEntry],
    tree_born: bool,
    tree_truncated: bool,
    tree_phase: crate::ForgeTreePhase,
    file_path: &str,
    file_text: &str,
    file_binary: bool,
    file_truncated: bool,
    file_picture: bool,
    file_width: i64,
    file_height: i64,
    file_note: &str,
    file_header: &str,
    file_phase: crate::ForgeFilePhase,
    drafts_cleared: i64,
    drafts_scope: &str,
    note_scope: &str,
    note_blocked: bool,
) -> Element<'static, ModuleViewEvent> {
    let props = ForgeProps {
        dark,
        connected,
        org,
        about,
        tier,
        network_chain_id,
        connected_rpc,
        repos,
        list_phase: forge_phase_word(list_phase),
        open_repo,
        repo_menu,
        repo_phase: forge_phase_word(repo_phase),
        branches,
        tab: match tab {
            crate::ForgeTab::Code => "code",
            crate::ForgeTab::Pulls => "pulls",
            crate::ForgeTab::Issues => "issues",
        },
        items,
        forge_item_number: item_number,
        item_phase: forge_phase_word(item_phase),
        forge_item_kind: item_kind,
        forge_item_title: item_title,
        forge_item_state: item_state,
        forge_item_author: item_author,
        forge_item_branches: item_branches,
        forge_item_body: item_body,
        forge_item_blocks: item_blocks,
        forge_item_files_changed: files_changed,
        forge_item_additions: additions,
        forge_item_deletions: deletions,
        diff_rows: crate::backend::diff_lines(diff),
        forge_item_diff_truncated: diff_truncated,
        forge_item_merge_oid: merge_oid,
        forge_item_source_oid: source_oid,
        forge_item_approvals: approvals,
        forge_item_change_requests: change_requests,
        forge_item_reviews: reviews,
        merge_conflicts,
        merge_busy,
        review_verdict: forge_verdict_word(review_verdict),
        review_busy,
        staged_comments,
        comment_cap_reached: crate::backend::forge_comment_cap_reached(staged_comments),
        discussion,
        linked_note: linked_note.as_slice(),
        discussion_clipped: false,
        landed_seq,
        landed_tick,
        tree_path,
        tree_rev,
        tree_entries,
        tree_born,
        tree_truncated,
        tree_phase: match tree_phase {
            crate::ForgeTreePhase::Loading => "loading",
            crate::ForgeTreePhase::Ready => "ready",
            crate::ForgeTreePhase::Failed => "failed",
        },
        file_path,
        file_text,
        file_binary,
        file_truncated,
        file_picture,
        file_width,
        file_height,
        file_note,
        file_header,
        file_phase: match file_phase {
            crate::ForgeFilePhase::Idle => "idle",
            crate::ForgeFilePhase::Loading => "loading",
            crate::ForgeFilePhase::Ready => "ready",
            crate::ForgeFilePhase::Failed => "failed",
        },
        drafts_cleared,
        drafts_scope,
        note_scope,
        note_blocked,
    };
    module_view(
        "forge",
        display_budget::forge(serde_json::to_value(&props).expect("props encode")),
    )
}

fn forge_phase_word(phase: crate::ForgePhase) -> &'static str {
    match phase {
        crate::ForgePhase::Idle => "idle",
        crate::ForgePhase::Loading => "loading",
        crate::ForgePhase::Ready => "ready",
        crate::ForgePhase::Failed => "failed",
    }
}

fn forge_verdict_word(verdict: crate::ForgeReviewVerdict) -> &'static str {
    match verdict {
        crate::ForgeReviewVerdict::Comment => "comment",
        crate::ForgeReviewVerdict::Approve => "approve",
        crate::ForgeReviewVerdict::RequestChanges => "request_changes",
    }
}

pub fn forge_intent(event: &ModuleViewEvent) -> crate::ForgeIntent {
    use crate::ForgeIntent as Intent;
    match event.kind.as_str() {
        "open_repo" => Intent::OpenRepo,
        "close_repo" => Intent::CloseRepo,
        "toggle_repo_menu" => Intent::ToggleRepoMenu,
        "tab" => Intent::Tab,
        "open_item" => Intent::OpenItem,
        "close_item" => Intent::CloseItem,
        "merge" => Intent::Merge,
        "review_pick" => Intent::ReviewPick,
        "review_submit" => Intent::ReviewSubmit,
        "comment_stage" => Intent::CommentStage,
        "comment_drop" => Intent::CommentDrop,
        "tree" => Intent::Tree,
        "blob" => Intent::Blob,
        "open_link" => Intent::OpenLink,
        "composer" => Intent::Composer,
        _ => Intent::Copy,
    }
}

/// The repo seat a `tab` intent names; a word the screen has no seat for is
/// the code browse.
pub fn forge_event_tab(event: &ModuleViewEvent) -> crate::ForgeTab {
    match event_text(event, "tab").as_str() {
        "pulls" => crate::ForgeTab::Pulls,
        "issues" => crate::ForgeTab::Issues,
        _ => crate::ForgeTab::Code,
    }
}

/// The verdict a `review_pick` intent names; an unknown word is a comment.
pub fn forge_event_verdict(event: &ModuleViewEvent) -> crate::ForgeReviewVerdict {
    match event_text(event, "verdict").as_str() {
        "approve" => crate::ForgeReviewVerdict::Approve,
        "request_changes" => crate::ForgeReviewVerdict::RequestChanges,
        _ => crate::ForgeReviewVerdict::Comment,
    }
}

// ---------- the shell seat ----------

/// The Shell tab: the app's agent picks, the terminal it holds and the run
/// it is watching, drawn by the `shell` view. The provider's wording — the
/// header line, the grant note, the terminal note, the blurbs — is folded
/// here, so the view names no provider; the terminal session is parked for
/// the `agent_terminal_surface` slot. Intents come back as `surface`,
/// `setup`, `identity`, `host_node`, `refresh`, `terminal_start`,
/// `terminal_stop`, `reset`, `detach`, `reopen`, `discard`, `open_link`,
/// and — from the host's own composer surface — `send` (`body`).
#[allow(
    clippy::too_many_arguments,
    reason = "the Ice extern hands the screen's facts one by one"
)]
pub fn shell_view(
    dark: bool,
    connected: bool,
    surface: crate::ShellSurface,
    setup_open: bool,
    identity_options: &[String],
    identity: &str,
    provider: &str,
    credential: &str,
    host_node_options: &[String],
    host_node: &str,
    credentials_loading: bool,
    terminal: &crate::backend::AgentTerminalSession,
    terminal_running: bool,
    terminal_busy: bool,
    terminal_title: &str,
    terminal_error: &str,
    entries: &[crate::backend::AgentChatEntry],
    activity: &[crate::backend::AgentActivity],
    chat_busy: bool,
    chat_status: &str,
    chat_detail: &str,
    live: &str,
    saga_id: &str,
    detached_saga: &str,
) -> Element<'static, ModuleViewEvent> {
    use crate::backend as b;
    *shell_terminal().lock().expect("shell terminal") = Some(terminal.clone());
    let entries: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "id": entry.id,
                "role": entry.role,
                "body": entry.body,
                "provider_label": b::agent_provider_label(&entry.provider),
                "provider_initial": b::agent_provider_initial(&entry.provider),
                "status": entry.status,
                "run_label": b::agent_run_label(&entry.saga_id),
                "steps": entry.steps,
                "steps_label": entry.steps_label,
            })
        })
        .collect();
    let props = serde_json::json!({
        "dark": dark,
        "connected": connected,
        "surface": match surface {
            crate::ShellSurface::Tasks => "tasks",
            crate::ShellSurface::Terminal => "terminal",
        },
        "setup_open": setup_open,
        "identity_options": identity_options,
        "identity": identity,
        "provider_initial": b::agent_provider_initial(provider),
        "credential": credential,
        "host_node_options": host_node_options,
        "host_node": host_node,
        "credentials_loading": credentials_loading,
        "terminal_running": terminal_running,
        "terminal_busy": terminal_busy,
        "terminal_title": terminal_title,
        "terminal_error": terminal_error,
        "entries": entries,
        "activity": activity,
        "chat_busy": chat_busy,
        "chat_status": chat_status,
        "chat_detail": chat_detail,
        "live": live,
        "saga_id": saga_id,
        "detached_saga": detached_saga,
        "run_line": b::agent_run_line(identity, host_node),
        "grant_note": b::agent_host_grant_note(host_node, credential),
        "terminal_note": b::agent_terminal_note(provider, credential),
        "composer_hint": b::agent_composer_hint(provider),
        "task_blurb": b::agent_task_blurb(host_node),
        "register_hint": b::agent_register_hint(provider),
    });
    module_view("shell", serde_json::to_vec(&props).expect("props encode"))
}

pub fn shell_intent(event: &ModuleViewEvent) -> crate::ShellIntent {
    use crate::ShellIntent as Intent;
    match event.kind.as_str() {
        "surface" => Intent::Surface,
        "setup" => Intent::Setup,
        "identity" => Intent::Identity,
        "host_node" => Intent::HostNode,
        "refresh" => Intent::Refresh,
        "terminal_start" => Intent::TerminalStart,
        "terminal_stop" => Intent::TerminalStop,
        "send" => Intent::Send,
        "reset" => Intent::Reset,
        "detach" => Intent::Detach,
        "reopen" => Intent::Reopen,
        "discard" => Intent::Discard,
        _ => Intent::OpenLink,
    }
}

/// The surface a `surface` intent names; a word the screen has no surface
/// for is the tasks.
pub fn shell_event_surface(event: &ModuleViewEvent) -> crate::ShellSurface {
    match event_text(event, "surface").as_str() {
        "terminal" => crate::ShellSurface::Terminal,
        _ => crate::ShellSurface::Tasks,
    }
}

/// Empties the host-side shell composer.
pub fn shell_composer_clear() -> bool {
    crate::shell_composer::clear();
    true
}

/// The terminal session behind the shell view's slot: the one the app
/// last drew the tab with. One per process, like the view it belongs to.
fn shell_terminal() -> &'static Mutex<Option<crate::backend::AgentTerminalSession>> {
    static TERMINAL: OnceLock<Mutex<Option<crate::backend::AgentTerminalSession>>> =
        OnceLock::new();
    TERMINAL.get_or_init(Mutex::default)
}

// ---------- the pages seat ----------

/// The Pages tab: the facts the app holds, drawn by the `pages` view. The
/// document is NOT among them — it is the app's editor, stashed for the
/// `page_document` surface the view leaves a slot for (`crate::pages::surface`)
/// and painted there by the host. Its intents come back one per act
/// (`pages_intent`); the drafts the view holds cross only with the act that
/// reads them, and `seed_rev` moving tells the view to take `page_draft` /
/// `block_comment_draft` back as its own (a recovered comment, a refused
/// post or create handed back).
#[allow(
    clippy::too_many_arguments,
    reason = "the Ice extern hands the screen's facts one by one"
)]
pub fn pages_view(
    dark: bool,
    connected: bool,
    loading: bool,
    mutation_phase: crate::MutationPhase,
    network_chain_id: &str,
    pages: &[crate::backend::PageItem],
    page_create_open: bool,
    page_draft: &str,
    block_comment_draft: &str,
    seed_rev: i64,
    active_page: &str,
    active_page_title: &str,
    active_page_parent: &str,
    page_searching: bool,
    page_search_hits: &[crate::backend::PageSearchHit],
    page_search_query: &str,
    page_delete_armed: bool,
    autosave: crate::AutosaveStatus,
    page_refusal: &str,
    doc_tabs: &[String],
    blocks: &[crate::backend::PageBlock],
    commented_block_hits: &[String],
    caret_comment_target: &str,
    active_thread_anchor: &str,
    orphaned_comment_drafts: &[String],
    page_editor: &iced::widget::text_editor::Content,
    block_comments_open: bool,
    thread_total: i64,
    threads: &[crate::backend::PageCommentThread],
    comment_rows: &[crate::pages::PageCommentThreadRow],
    threads_loading: bool,
    threads_has_more: bool,
    active_thread: &str,
    comments: &[crate::backend::PageComment],
    comments_loading: bool,
    comments_has_more: bool,
) -> Element<'static, ModuleViewEvent> {
    crate::pages::surface::show(
        page_editor,
        dark,
        loading || !connected,
        blocks,
        commented_block_hits,
    );
    let autosave = match autosave {
        crate::AutosaveStatus::Idle => "idle",
        crate::AutosaveStatus::Saving => "saving",
        crate::AutosaveStatus::Saved => "saved",
        crate::AutosaveStatus::Error => "error",
    };
    let subpages: Vec<serde_json::Value> = crate::backend::subpage_blocks(blocks)
        .into_iter()
        .map(|block| serde_json::json!({ "id": block.id, "title": block.text }))
        .collect();
    let props = serde_json::json!({
        "dark": dark,
        "connected": connected,
        "loading": loading,
        "busy": mutation_phase != crate::MutationPhase::Idle,
        "page_link": crate::backend::duck_page_link(active_page.to_owned(), network_chain_id.to_owned()),
        "pages": pages,
        "page_create_open": page_create_open,
        "active_page": active_page,
        "active_page_title": active_page_title,
        "active_page_parent": active_page_parent,
        "page_searching": page_searching,
        "page_search_hits": page_search_hits,
        "page_search_query": page_search_query,
        "page_delete_armed": page_delete_armed,
        "autosave": autosave,
        "page_refusal": page_refusal,
        "doc_tabs": crate::backend::doc_tab_rows(doc_tabs, pages, active_page),
        "subpages": subpages,
        "orphaned_comment_drafts": orphaned_comment_drafts,
        "block_comments_open": block_comments_open,
        "thread_total": thread_total,
        "comment_rows": comment_rows,
        "threads_loading": threads_loading,
        "threads_has_more": threads_has_more,
        "active_thread": active_thread,
        "thread_resolved": crate::backend::thread_is_resolved(threads, active_thread),
        "active_thread_anchor": active_thread_anchor,
        "comments": comments,
        "comments_loading": comments_loading,
        "comments_has_more": comments_has_more,
        "compose_hint": crate::pages::comment_compose_hint(blocks, caret_comment_target, active_page),
        "seed_rev": seed_rev,
        "page_seed": page_draft,
        "comment_seed": block_comment_draft,
    });
    module_view("pages", serde_json::to_vec(&props).expect("props encode"))
}

pub fn pages_intent(event: &ModuleViewEvent) -> crate::PagesIntent {
    use crate::PagesIntent as Intent;
    match event.kind.as_str() {
        "toggle_create" => Intent::ToggleCreate,
        "create" => Intent::Create,
        "choose" => Intent::Choose,
        "search" => Intent::Search,
        "clear_search" => Intent::ClearSearch,
        "arm_delete" => Intent::ArmDelete,
        "disarm_delete" => Intent::DisarmDelete,
        "delete" => Intent::Delete,
        "close_tab" => Intent::CloseTab,
        "open_hit" => Intent::OpenHit,
        "use_draft" => Intent::UseDraft,
        "discard_draft" => Intent::DiscardDraft,
        "edited" => Intent::Edited,
        "toggle_comments" => Intent::ToggleComments,
        "close_comments" => Intent::CloseComments,
        "open_thread" => Intent::OpenThread,
        "resolve" => Intent::Resolve,
        "more_threads" => Intent::MoreThreads,
        "close_thread" => Intent::CloseThread,
        "more_comments" => Intent::MoreComments,
        "post" => Intent::Post,
        _ => Intent::Copy,
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
    thread_messages: std::borrow::Cow<'a, [crate::backend::ChatMessage]>,
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
        thread_messages: std::borrow::Cow::Borrowed(thread_messages),
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
    module_view("chat", encode_chat_props(props))
}

/// Text bytes a guest's big list or blob may put on one frame: the wire
/// spends 64 KiB of text per frame and EMPTIES whatever comes after, and the
/// newest messages come last — a busy room's hot window (256 rows) drew its
/// newest messages blank. The rest of the frame (rooms, names, times, the
/// rail) lives in the headroom.
const TIMELINE_TEXT_BUDGET: usize = 48 << 10;

/// The facts encoded for the view, the timelines held to
/// [`TIMELINE_TEXT_BUDGET`]: the newest messages that fit, oldest dropped
/// first, and a clipped stream says so through `has_older_history` (the
/// thread through `thread_has_more`, its root always kept) so the view still
/// offers what was left behind as history.
fn encode_chat_props(mut props: ChatProps<'_>) -> Vec<u8> {
    let (stream, stream_clipped) = newest_within(props.messages, TIMELINE_TEXT_BUDGET);
    let stream_spent: usize = stream.iter().map(text_bytes).sum();
    props.messages = stream;
    props.has_older_history |= stream_clipped;
    let thread = &*props.thread_messages;
    // the root is drawn as its own block above the replies: it stays
    let root = usize::from(
        thread
            .first()
            .is_some_and(|message| message.thread_seq == 0),
    );
    let (replies, thread_clipped) = newest_within(
        &thread[root..],
        TIMELINE_TEXT_BUDGET
            .saturating_sub(stream_spent + thread[..root].iter().map(text_bytes).sum::<usize>()),
    );
    if thread_clipped {
        props.thread_messages =
            std::borrow::Cow::Owned(thread[..root].iter().chain(replies).cloned().collect());
        props.thread_has_more = true;
    }
    serde_json::to_vec(&props).expect("props encode")
}

/// The bytes a message puts on the wire as text.
fn text_bytes(message: &crate::backend::ChatMessage) -> usize {
    message.body.len() + message.author.len() + message.meta.len()
}

/// The newest tail of `messages` whose text fits `budget`, and whether
/// anything older was left out.
fn newest_within(
    messages: &[crate::backend::ChatMessage],
    budget: usize,
) -> (&[crate::backend::ChatMessage], bool) {
    let mut spent = 0;
    let mut start = messages.len();
    while start > 0 && spent + text_bytes(&messages[start - 1]) <= budget {
        spent += text_bytes(&messages[start - 1]);
        start -= 1;
    }
    (&messages[start..], start > 0)
}

/// The head of `text` that fits `budget`, cut on a char boundary, and
/// whether anything was cut.
fn head_within(text: &str, budget: usize) -> (&str, bool) {
    if text.len() <= budget {
        return (text, false);
    }
    let mut end = budget;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], true)
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

/// The room's explicit roster, as the app just read it, handed to the
/// composers over that room (and its threads) for their mention menu.
pub fn chat_composer_roster(scope: &str, members: &[crate::backend::ChatMember]) -> bool {
    crate::composer_surface::roster(scope, members);
    true
}

// ---------- the files seat ----------

/// The Files tab: one directory's listing, the preview open in it, the
/// snapshot history and the module's write rule, drawn by the `files` view.
/// Its intents come back one per act (`files_intent`): a navigation carries
/// a `path`, a write the `name` the reader typed or the `text` of the body
/// they edited — the drafts are the view's, and `writes` moving tells it a
/// committed write consumed one. The picture viewer, the highlighted reader
/// and the Markdown document are host surfaces (`surfaces_of("files")`).
#[allow(
    clippy::too_many_arguments,
    reason = "the Ice extern hands the screen's facts one by one"
)]
pub fn files_view(
    dark: bool,
    connected: bool,
    path: &str,
    listed: bool,
    entries: &[crate::backend::FsEntry],
    loading: bool,
    preview_path: &str,
    preview_entry: &crate::backend::FsEntry,
    delete_target: &str,
    diff_from: &str,
    diff: &[crate::backend::FsDiffEntry],
    history: &[crate::backend::FsSnapshot],
    preview_truncated: bool,
    preview_binary: bool,
    preview_picture: bool,
    preview_width: i64,
    preview_height: i64,
    preview_text: &str,
    write_refusal: &str,
    writes: i64,
    rpc: &str,
    chain: &str,
    connection: i64,
    preview_base: &str,
    save_reply: &crate::backend::FsSaveHistory,
) -> Element<'static, ModuleViewEvent> {
    let props = serde_json::json!({
        "network_scope": crate::backend::files_network_scope(rpc.into(), chain.into()),
        "context": crate::backend::files_context(rpc.into(), chain.into(), connection),
        "preview_base": preview_base,
        "save_reply": save_reply,
        "path": path,
        "listed": listed,
        "entries": entries,
        "directories": crate::backend::fs_directories(entries),
        "connected": connected,
        "loading": loading,
        "preview_path": preview_path,
        "preview_entry": preview_entry,
        "delete_target": delete_target,
        "diff_from": diff_from,
        "diff": diff,
        "history": history,
        "preview_truncated": preview_truncated,
        "preview_binary": preview_binary,
        "preview_picture": preview_picture,
        "preview_width": preview_width,
        "preview_height": preview_height,
        "preview_text": preview_text,
        "dark": dark,
        "write_refusal": write_refusal,
        "writes": writes,
    });
    module_view("files", display_budget::files(props))
}

pub fn files_intent(event: &ModuleViewEvent) -> crate::FilesIntent {
    use crate::FilesIntent as Intent;
    match event.kind.as_str() {
        "open_dir" => Intent::OpenDir,
        "open_file" => Intent::OpenFile,
        "open_parent" => Intent::OpenParent,
        "mkdir" => Intent::Mkdir,
        "new_file" => Intent::NewFile,
        "arm_delete" => Intent::ArmDelete,
        "delete" => Intent::Delete,
        "save" => Intent::Save,
        "show_diff" => Intent::ShowDiff,
        "close_diff" => Intent::CloseDiff,
        "open_link" => Intent::OpenLink,
        _ => Intent::DisarmDelete,
    }
}

/// The string argument at `index` of a surface's args; "" when the guest
/// sent something else.
fn surface_str(args: &[wire::SurfaceValue], index: usize) -> String {
    match args.get(index) {
        Some(wire::SurfaceValue::Str(text)) => text.clone(),
        _ => String::new(),
    }
}

fn surface_bool(args: &[wire::SurfaceValue], index: usize) -> bool {
    matches!(args.get(index), Some(wire::SurfaceValue::Bool(true)))
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
/// `-> unit` — hears only that something happened. The files view's three
/// are the preview's readers: the picture viewer over the Files surface's
/// store, the highlighted code reader, and the Markdown document, whose
/// activated link goes back to the guest's own handler as a string.
fn surfaces_of(module: &str) -> Surfaces {
    let mut surfaces = Surfaces::default();
    // the shell view's three: the terminal for the session the app parked,
    // the answer Markdown (a link it opens goes back to the guest's
    // handler), and the composer, whose submit is the app's `send` intent
    if module == "shell" {
        surfaces.insert(
            "agent_terminal_surface".into(),
            Arc::new(|_key: &str, _args: &[wire::SurfaceValue]| {
                let session = shell_terminal().lock().expect("shell terminal").clone();
                let Some(session) = session else {
                    return widget::Space::new().into();
                };
                crate::backend::agent_terminal_surface(&session).map(|()| wire::SurfaceValue::Unit)
            }),
        );
        surfaces.insert(
            "agent_markdown".into(),
            Arc::new(|_key: &str, args: &[wire::SurfaceValue]| {
                crate::backend::agent_markdown(surface_str(args, 0), surface_bool(args, 1))
                    .map(wire::SurfaceValue::Str)
            }),
        );
        surfaces.insert("shell_composer".into(), crate::shell_composer::provider());
    }
    if module == "chat" {
        surfaces.insert("chat_composer".into(), crate::composer_surface::provider());
    }
    if module == "forge" {
        // the discussion note is the chat composer over its own scope, the
        // item's channel — same document rule, same `composer` intent
        surfaces.insert("forge_composer".into(), crate::composer_surface::provider());
        surfaces.insert(
            "picture".into(),
            Arc::new(|_key: &str, args: &[wire::SurfaceValue]| {
                let [
                    wire::SurfaceValue::Str(surface),
                    wire::SurfaceValue::Str(path),
                ] = args
                else {
                    return widget::Space::new().into();
                };
                crate::backend::picture(surface.clone(), path.clone())
                    .map(|()| wire::SurfaceValue::Unit)
            }),
        );
        surfaces.insert(
            "forge_markdown".into(),
            Arc::new(|_key: &str, args: &[wire::SurfaceValue]| {
                let [
                    wire::SurfaceValue::Str(source),
                    wire::SurfaceValue::Str(doc),
                    wire::SurfaceValue::Bool(dark),
                ] = args
                else {
                    return widget::Space::new().into();
                };
                crate::backend::forge_markdown(source.clone(), doc.clone(), *dark)
                    .map(wire::SurfaceValue::Str)
            }),
        );
        surfaces.insert(
            "forge_code".into(),
            Arc::new(|_key: &str, args: &[wire::SurfaceValue]| {
                let [
                    wire::SurfaceValue::Str(source),
                    wire::SurfaceValue::Str(path),
                    wire::SurfaceValue::Bool(dark),
                ] = args
                else {
                    return widget::Space::new().into();
                };
                crate::backend::forge_code(source.clone(), path.clone(), *dark)
                    .map(|()| wire::SurfaceValue::Unit)
            }),
        );
    }
    if module == "files" {
        surfaces.insert(
            "picture".into(),
            Arc::new(|_key: &str, args: &[wire::SurfaceValue]| {
                crate::backend::picture(surface_str(args, 0), surface_str(args, 1))
                    .map(|()| wire::SurfaceValue::Unit)
            }),
        );
        surfaces.insert(
            "forge_code".into(),
            Arc::new(|_key: &str, args: &[wire::SurfaceValue]| {
                crate::backend::forge_code(
                    surface_str(args, 0),
                    surface_str(args, 1),
                    surface_bool(args, 2),
                )
                .map(|()| wire::SurfaceValue::Unit)
            }),
        );
        surfaces.insert(
            "agent_markdown".into(),
            Arc::new(|_key: &str, args: &[wire::SurfaceValue]| {
                crate::backend::agent_markdown(surface_str(args, 0), surface_bool(args, 1))
                    .map(wire::SurfaceValue::Str)
            }),
        );
    }
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
    if module == "pages" {
        surfaces.insert("page_document".into(), crate::pages::surface::provider());
    }
    surfaces
}

/// The intent a host surface's event comes back as, by module: the node's
/// log ring, the pages document.
fn surface_intent(module: &str) -> &'static str {
    match module {
        "pages" => "edited",
        _ => "log_timeline",
    }
}

/// The operations a view may ask of the app, by module. An intent outside
/// the list is refused at the door, never handed to a handler.
fn intents_of(module: &str) -> &'static [&'static str] {
    match module {
        "governance" => &["vote", "execute"],
        "members" => &["copy", "agent_status", "propose"],
        "agents" => &["status", "save", "register"],
        "node" => &["copy", "tab", "log_filter"],
        "explorer" => &["refresh", "copy", "search", "clear"],
        // `send` is deliberately NOT here: a send crosses only from the
        // host's own composer surface (`deliver`), never as a guest request.
        "shell" => &[
            "surface",
            "setup",
            "identity",
            "host_node",
            "refresh",
            "terminal_start",
            "terminal_stop",
            "reset",
            "detach",
            "reopen",
            "discard",
            "open_link",
        ],
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
        "forge" => &[
            "open_repo",
            "close_repo",
            "toggle_repo_menu",
            "tab",
            "open_item",
            "close_item",
            "merge",
            "review_pick",
            "review_submit",
            "comment_stage",
            "comment_drop",
            "tree",
            "blob",
            "open_link",
            "copy",
            "composer",
        ],
        "files" => &[
            "open_dir",
            "open_file",
            "open_parent",
            "mkdir",
            "new_file",
            "arm_delete",
            "disarm_delete",
            "delete",
            "save",
            "show_diff",
            "close_diff",
            "open_link",
        ],
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
            "light",
            "dark",
            "notifications",
        ],
        "pages" => &[
            "toggle_create",
            "create",
            "choose",
            "search",
            "clear_search",
            "arm_delete",
            "disarm_delete",
            "delete",
            "close_tab",
            "open_hit",
            "use_draft",
            "discard_draft",
            "toggle_comments",
            "close_comments",
            "open_thread",
            "resolve",
            "more_threads",
            "close_thread",
            "more_comments",
            "post",
            "copy",
        ],
        _ => &[],
    }
}

// ---------- mounting ----------

/// The widget for one module's view, with `props` as the app has them now.
/// The view is loaded on its own thread, and the tab shows what stage it is
/// at until then. A load still going or failed when the app points at
/// another node is started over on that node ([`connected`]); a view that
/// loaded stays for the process (the swap-in-place on a new deployment is
/// the next step, behind the `generation` this registry already keeps).
fn module_view(module: &'static str, props: Vec<u8>) -> Element<'static, ModuleViewEvent> {
    mounted(module).lock().expect("module view lock").props = Some(props);
    drawn(module)
}

/// The mounted view as it stands — its current frame, or the notice for a
/// slot without one — with no props pushed.
pub(crate) fn drawn(module: &'static str) -> Element<'static, ModuleViewEvent> {
    let mounted = mounted(module);
    let (content, rev, generation) = {
        let mut locked = mounted.lock().expect("module view lock");
        let generation = locked.generation;
        match &mut locked.slot {
            Slot::Loading => return notice("Loading the view…"),
            Slot::Empty => {
                return notice(&format!(
                    "This network has no {module} view yet. An admin activates a {module} deployment that ships one."
                ));
            }
            Slot::Failed(reason) => return notice(reason),
            Slot::Ready(guest) => (guest.render(), guest.frame_rev, generation),
        }
    };
    Element::new(ModuleView {
        mounted,
        generation,
        rev,
        content,
    })
}

/// The node the app is connected to, for the views that come from its
/// deployments. Told by `backend::connect`; every module view still
/// loading or failed off the previous node starts over on this one, under
/// a new generation, so an answer the previous node is still composing
/// lands nowhere. Returns the loads it started, for a test to wait on.
pub fn connected(client: &ducktape_rpc::Client) -> Vec<std::thread::JoinHandle<()>> {
    // THE REGISTRY LOCK FIRST: a connection change and the restart of the
    // views under it are one step. Two callers — `backend::connect` runs on
    // the executor's threads, and two connects can overlap — otherwise
    // interleave into loads asked of one node under the other's revision,
    // every one of which dies at install, and the views stay "Loading".
    let registry = registry().lock().expect("module views");
    // the client and its revision move as one, and their lock is let go
    // before any view is touched: a load installs under it (see
    // `spawn_load`), and takes the view's own lock inside it. Lock order,
    // everywhere: registry, then connection, then a view.
    let snapshot = {
        let mut connection = connection().lock().expect("views rpc");
        connection.rev += 1;
        connection.client = Some(client.clone());
        connection.clone()
    };
    registry
        .iter()
        .filter_map(|(module, mounted)| {
            let mut locked = mounted.lock().expect("module view lock");
            let owned = crate::backend::view_source::module_owned(module);
            match locked.slot {
                // the desktop's own view is the same on every node
                Slot::Ready(_) if !owned => return None,
                // a module-owned view is reloaded from this node's
                // deployment and swapped in place; the tab keeps showing
                // the one it has until then
                Slot::Ready(_) => {}
                _ => locked.slot = Slot::Loading,
            }
            let generation = locked.start(None);
            Some(spawn_load(module, mounted, generation, snapshot.clone()))
        })
        .collect()
}

/// The node's deployments moved (a new block): every module-owned view
/// whose module's active code is not the one it was drawn from is loaded
/// again, under a new generation, and swapped in place when it is ready.
/// One check in flight at a time; a block that lands during one is
/// covered by the next. Returns the loads it started, for a test to wait on.
pub async fn deployments_checked() -> Vec<std::thread::JoinHandle<()>> {
    static IN_FLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    use std::sync::atomic::Ordering;
    if IN_FLIGHT.swap(true, Ordering::SeqCst) {
        return Vec::new();
    }
    let started = deployments_check().await;
    IN_FLIGHT.store(false, Ordering::SeqCst);
    started
}

async fn deployments_check() -> Vec<std::thread::JoinHandle<()>> {
    let asked_of = connection().lock().expect("views rpc").clone();
    let Some(client) = &asked_of.client else {
        return Vec::new();
    };
    let hashes = match crate::backend::view_source::active_hashes(client).await {
        Ok(hashes) => hashes,
        Err(error) => {
            tracing::warn!(
                target: "ducktape::app",
                reason = "module_status_unreadable",
                error = %error,
                "module deployments not checked"
            );
            return Vec::new();
        }
    };
    let registry = registry().lock().expect("module views");
    // the connection may have moved while the registry answered: a load
    // asked of the old one dies at install, and `connected` restarted
    // everything under the new one
    if connection().lock().expect("views rpc").rev != asked_of.rev {
        return Vec::new();
    }
    registry
        .iter()
        .filter(|(module, _)| crate::backend::view_source::module_owned(module))
        .filter_map(|(module, mounted)| {
            let mut locked = mounted.lock().expect("module view lock");
            // a module the node does not run has no deployment to move
            // to: the view stays as `connected` left it
            let active = hashes.get(*module).copied()?;
            // a load already after this deployment — or after whatever is
            // active, as a reconnect's is — lands or fails on its own;
            // starting over on every block would never let it land
            let waited_for =
                locked.in_flight && locked.wanted.is_none_or(|wanted| Some(wanted) == active);
            if waited_for || active == locked.hash {
                return None;
            }
            let generation = locked.start(active);
            Some(spawn_load(module, mounted, generation, asked_of.clone()))
        })
        .collect()
}

/// The node the module-owned views load from, and how many times the app
/// has moved: a load is asked of one snapshot of this and installs only
/// while it is still the one.
#[derive(Clone, Default)]
struct Connection {
    client: Option<ducktape_rpc::Client>,
    rev: u64,
}

fn connection() -> &'static Mutex<Connection> {
    static CONNECTION: OnceLock<Mutex<Connection>> = OnceLock::new();
    CONNECTION.get_or_init(Mutex::default)
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
/// `generation` moves with every load asked for; a load answering for an
/// earlier one is dropped, and so is what the reader did in a tree of one.
struct Mounted {
    slot: Slot,
    props: Option<Vec<u8>>,
    generation: u64,
    /// The deployment the slot answers for — the view drawn, the empty
    /// slot of a deployment without one, or the one that failed to load —
    /// so a block moves it only when the active code moved.
    hash: Option<[u8; 32]>,
    /// A load is on its way for `generation`, and the deployment it is
    /// after when a block named one: a block that names it again waits
    /// for it instead of starting over.
    in_flight: bool,
    wanted: Option<[u8; 32]>,
}

impl Mounted {
    /// Opens the next generation for a load after `wanted` (None: whatever
    /// the node holds active), and names it.
    fn start(&mut self, wanted: Option<[u8; 32]>) -> u64 {
        self.generation += 1;
        self.in_flight = true;
        self.wanted = wanted;
        self.generation
    }
}

enum Slot {
    Loading,
    Ready(Box<Guest>),
    /// The active deployment verified, and it ships no view.
    Empty,
    Failed(String),
}

type Registry = Mutex<HashMap<&'static str, Arc<Mutex<Mounted>>>>;

fn registry() -> &'static Registry {
    static MOUNTED: OnceLock<Registry> = OnceLock::new();
    MOUNTED.get_or_init(Mutex::default)
}

fn mounted(module: &'static str) -> Arc<Mutex<Mounted>> {
    let mut registry = registry().lock().expect("module views");
    registry
        .entry(module)
        .or_insert_with(|| {
            let snapshot = connection().lock().expect("views rpc").clone();
            let mounted = Arc::new(Mutex::new(Mounted {
                slot: Slot::Loading,
                props: None,
                generation: 0,
                hash: None,
                in_flight: true,
                wanted: None,
            }));
            spawn_load(module, &mounted, 0, snapshot);
            mounted
        })
        .clone()
}

/// Loads the view on its own thread — a cold cranelift compile is a second
/// or more; the window thread shows "Loading" instead of freezing for it —
/// and installs it only if `mounted` still waits for this very load AND the
/// app is still on the node it was asked of, both checked under the
/// connection lock so a move cannot slip between the check and the seat.
fn spawn_load(
    module: &'static str,
    mounted: &Arc<Mutex<Mounted>>,
    generation: u64,
    asked_of: Connection,
) -> std::thread::JoinHandle<()> {
    let loading = mounted.clone();
    std::thread::spawn(move || {
        let loaded = Guest::load(module, asked_of.client.as_ref(), generation, &loading);
        let connection = connection().lock().expect("views rpc");
        let mut locked = loading.lock().expect("module view lock");
        if locked.generation != generation {
            return;
        }
        locked.in_flight = false;
        if connection.rev != asked_of.rev {
            return;
        }
        let Mounted { slot, hash, .. } = &mut *locked;
        match loaded {
            Ok(Loaded::Fresh(guest)) => {
                *hash = guest.hash;
                *slot = Slot::Ready(guest);
            }
            Ok(Loaded::Unchanged) => {}
            Ok(Loaded::Empty(removed)) => {
                *hash = Some(removed);
                *slot = Slot::Empty;
            }
            // the same tab, the same surface handle, the same host-side
            // input text and pictures: only the instance behind them moves
            Ok(Loaded::Swap {
                mut fresh,
                alive,
                ticks,
            }) => {
                let Slot::Ready(old) = slot else {
                    log_source(
                        module,
                        fresh.hash.as_ref(),
                        "Failed",
                        generation,
                        "the view left while its replacement was prepared",
                    );
                    return;
                };
                if !Arc::ptr_eq(&old.alive, &alive) || old.ticks != ticks || !old.settled() {
                    log_source(
                        module,
                        fresh.hash.as_ref(),
                        "Failed",
                        generation,
                        "the view moved while its replacement was prepared",
                    );
                    return;
                }
                fresh.frame_rev = old.frame_rev + 1;
                fresh.inputs = std::mem::take(&mut old.inputs);
                fresh.pictures = std::mem::take(&mut old.pictures);
                if let Some(root) = &mut fresh.frame.root {
                    fresh.inputs.adopt(root);
                    fresh.pictures.adopt(root);
                    root.for_each_mut(&mut |node| match node {
                        wire::Node::Svg { bytes, .. } => *bytes = None,
                        wire::Node::Image { data, .. } | wire::Node::ImageViewer { data, .. } => {
                            *data = None
                        }
                        _ => {}
                    });
                }
                log_source(module, fresh.hash.as_ref(), "Swapped", generation, "");
                *hash = fresh.hash;
                *slot = Slot::Ready(fresh);
            }
            Err(reason) => {
                tracing::warn!(
                    target: "ducktape::app",
                    module,
                    reason = "module_view_unloadable",
                    error = %reason,
                    "module view not loaded"
                );
                // a view that is there stays, with its hash: the failure is
                // the replacement's, not its own
                if !matches!(slot, Slot::Ready(_)) {
                    *slot = Slot::Failed(reason);
                }
            }
        }
    })
}

/// What a load came back with.
enum Loaded {
    /// A view where there was none.
    Fresh(Box<Guest>),
    /// The view already drawn is the active deployment's.
    Unchanged,
    /// A prepared replacement for the view drawn: restored from its
    /// snapshot, its first tree verified; `alive` and `ticks` name the
    /// instance it was prepared against.
    Swap {
        fresh: Box<Guest>,
        alive: Arc<()>,
        ticks: u64,
    },
    /// The deployment (this hash) ships no view.
    Empty([u8; 32]),
}

/// Whether `hash` is still the module's active code, asked of the node
/// right before a load's result is taken as the deployment's word.
async fn still_active(
    client: &ducktape_rpc::Client,
    module: &str,
    hash: [u8; 32],
) -> Result<bool, String> {
    crate::backend::view_source::active_hash(client, module)
        .await
        .map(|active| active == Some(hash))
        .map_err(|error| error.to_string())
}

/// One `view_source` line per outcome, with the same fields every time —
/// the canary greps them.
fn log_source(module: &str, hash: Option<&[u8; 32]>, state: &str, generation: u64, reason: &str) {
    tracing::info!(
        target: "ducktape::app",
        module,
        hash = %hash.map_or_else(|| "-".to_owned(), |hash| crate::backend::hex_encode(hash)),
        state,
        gen = generation,
        reason = %if reason.is_empty() { "-" } else { reason },
        "view_source"
    );
    #[cfg(test)]
    {
        let line = format!(
            "module={module} hash={} state={state} gen={generation} reason={}",
            hash.map_or_else(|| "-".to_owned(), |hash| crate::backend::hex_encode(hash)),
            if reason.is_empty() { "-" } else { reason }
        );
        canary::TAPS
            .lock()
            .expect("view_source taps")
            .retain(|tap| tap.send(line.clone()).is_ok());
    }
}

/// How long each stage of one module-owned load took: `status` and
/// `fetch` are the node's answers, `compile` is cranelift, `init` the
/// instance and its `on mount` or restore, `first_frame` the tree a
/// replacement proves (a fresh view draws its first on the window thread),
/// `check` the second look at the registry before the seat. `path` is
/// `tab` for a mount (the tab's first draw), `first` for a load the
/// connection asked for over an empty slot, `swap` over a view drawn.
#[derive(Default)]
struct LoadTiming {
    path: &'static str,
    status: Duration,
    fetch: Duration,
    compile: Duration,
    init: Duration,
    first_frame: Option<Duration>,
    check: Duration,
}

impl LoadTiming {
    /// One `view_load` line per load, every field every time, through the
    /// same logger and test tap as `view_source`.
    fn log(&self, module: &str, hash: Option<&[u8; 32]>, started: Instant, state: &str) {
        let ms = |duration: Duration| duration.as_millis();
        let hash = hash.map_or_else(|| "-".to_owned(), |hash| crate::backend::hex_encode(hash));
        let first_frame = self
            .first_frame
            .map_or_else(|| "-".to_owned(), |frame| ms(frame).to_string());
        let total = ms(started.elapsed());
        tracing::info!(
            target: "ducktape::app",
            module,
            hash = %hash,
            path = self.path,
            outcome = state,
            status_ms = ms(self.status),
            fetch_ms = ms(self.fetch),
            compile_ms = ms(self.compile),
            init_ms = ms(self.init),
            first_frame_ms = %first_frame,
            check_ms = ms(self.check),
            total_ms = total,
            "view_load"
        );
        #[cfg(test)]
        {
            let line = format!(
                "view_load module={module} hash={hash} path={} outcome={state} status_ms={} fetch_ms={} compile_ms={} init_ms={} first_frame_ms={first_frame} check_ms={} total_ms={total}",
                self.path,
                ms(self.status),
                ms(self.fetch),
                ms(self.compile),
                ms(self.init),
                ms(self.check),
            );
            canary::TAPS
                .lock()
                .expect("view_source taps")
                .retain(|tap| tap.send(line.clone()).is_ok());
        }
    }
}

/// What the canary runner (`tests::canary`) needs of this registry: the
/// `view_source` lines as they are logged, the module-owned views mounted
/// without an app around them, and the hash a slot answers for.
#[cfg(test)]
pub(crate) mod canary {
    use std::sync::{Mutex, mpsc};

    pub(crate) use super::drawn;
    pub(crate) use super::tests::connection_turn;

    pub(crate) static TAPS: Mutex<Vec<mpsc::Sender<String>>> = Mutex::new(Vec::new());

    pub(crate) fn tap() -> mpsc::Receiver<String> {
        let (sender, receiver) = mpsc::channel();
        TAPS.lock().expect("view_source taps").push(sender);
        receiver
    }

    pub(crate) fn mount_module_owned() {
        for module in crate::backend::view_source::MODULE_OWNED {
            super::mounted(module);
        }
    }

    pub(crate) fn seated_hash(module: &'static str) -> Option<[u8; 32]> {
        super::mounted(module)
            .lock()
            .expect("module view lock")
            .hash
    }

    /// Every text in the view's tree, or none where no tree is drawn yet.
    pub(crate) fn texts(module: &'static str) -> Vec<String> {
        let mut texts = Vec::new();
        let root = match &super::mounted(module)
            .lock()
            .expect("module view lock")
            .slot
        {
            super::Slot::Ready(guest) => guest.frame.root.clone(),
            _ => None,
        };
        if let Some(mut root) = root {
            root.for_each_mut(&mut |node| {
                if let super::wire::Node::Text { content, .. } = node {
                    texts.push(content.clone());
                }
            });
        }
        texts
    }
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

/// The guest's `restore(state, macos)` export.
type Restore = TypedFunc<(Vec<u8>, bool), (Result<(), String>,)>;

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
    /// Separates Files save acknowledgements across fresh guest instances.
    files_save_namespace: Option<String>,
    /// What the guest asked the app to do this redraw.
    intents: Vec<ModuleViewEvent>,
    /// The trap that ended the view, if one did. A faulted guest never ticks again.
    fault: Option<String>,
    /// The assets the deployment shipped beside this view, for the host
    /// surfaces that paint them by canonical relative path; swapped with the
    /// instance as one unit. Empty for a staged desktop view.
    assets: Arc<crate::backend::view_source::Assets>,
    /// The deployment this instance came from; none for a staged view.
    hash: Option<[u8; 32]>,
    /// This instance's identity: a replacement prepared against it is
    /// installed only over it.
    alive: Arc<()>,
    /// A replacement's first tree is in `frame` with its requests still
    /// to dispatch — the first redraw does that, without another tick.
    staged: bool,
    snapshot: TypedFunc<(), (Result<Vec<u8>, String>,)>,
    restore: Restore,
    init: TypedFunc<(bool,), ()>,
}

/// The asset at `path` in a deployment's map: the canonical relative path,
/// exactly — no normalisation, no file, no network.
fn artifact_asset<'a>(
    assets: &'a crate::backend::view_source::Assets,
    path: &str,
) -> Option<&'a [u8]> {
    assets.get(path).map(Vec::as_slice)
}

/// How many distinct missing asset paths a deployment's view is told
/// about: past this a guest asking a new path every frame is one line.
const MAX_MISSING_ASSETS: usize = 32;

fn hex_short(hash: &[u8; 32]) -> String {
    hash[..6].iter().map(|byte| format!("{byte:02x}")).collect()
}

const COMPILED_VIEW_LIMIT: usize = 16;
const COMPILED_VIEW_SOURCE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Default)]
struct ViewCodeCache {
    entries: VecDeque<CompiledView>,
}

struct CompiledView {
    hash: [u8; 32],
    source_bytes: usize,
    component: Arc<Component>,
}

impl ViewCodeCache {
    fn get(&mut self, hash: &[u8; 32]) -> Option<Arc<Component>> {
        let index = self.entries.iter().position(|entry| &entry.hash == hash)?;
        let entry = self.entries.remove(index)?;
        let component = entry.component.clone();
        self.entries.push_back(entry);
        Some(component)
    }

    fn insert(&mut self, entry: CompiledView) {
        if entry.source_bytes > COMPILED_VIEW_SOURCE_BYTES {
            return;
        }
        let mut bytes: usize = self.entries.iter().map(|entry| entry.source_bytes).sum();
        loop {
            let fits = self.entries.len() < COMPILED_VIEW_LIMIT
                && bytes + entry.source_bytes <= COMPILED_VIEW_SOURCE_BYTES;
            if fits {
                break;
            }
            let Some(old) = self.entries.pop_front() else {
                break;
            };
            bytes -= old.source_bytes;
        }
        self.entries.push_back(entry);
    }
}

fn compiled_view(bytes: &[u8]) -> Result<Arc<Component>, String> {
    static CODE: OnceLock<Mutex<ViewCodeCache>> = OnceLock::new();
    compile_view(engine(), CODE.get_or_init(Mutex::default), bytes)
}

// Cache code only: every load still creates its own Store, instance and assets.
// The cache belongs to this one Engine. Compile outside the lock so unrelated
// views can prepare concurrently; a competing result adopts the existing entry.
fn compile_view(
    engine: &Engine,
    cache: &Mutex<ViewCodeCache>,
    bytes: &[u8],
) -> Result<Arc<Component>, String> {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(bytes).into();
    if let Some(component) = cache.lock().expect("view code cache").get(&hash) {
        return Ok(component);
    }
    let component = Arc::new(Component::new(engine, bytes).map_err(|error| error.to_string())?);
    let mut cache = cache.lock().expect("view code cache");
    if let Some(existing) = cache.get(&hash) {
        return Ok(existing);
    }
    cache.insert(CompiledView {
        hash,
        source_bytes: bytes.len(),
        component: component.clone(),
    });
    Ok(component)
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
        match crate::backend::cache_dir() {
            Ok(directory) => {
                let mut cache = CacheConfig::new();
                cache.with_directory(directory.join("view-code"));
                match Cache::new(cache) {
                    Ok(cache) => {
                        config.cache(Some(cache));
                    }
                    Err(error) => tracing::warn!(reason = "view_cache_unavailable", %error),
                }
            }
            Err(error) => tracing::warn!(reason = "view_cache_directory_unavailable", %error),
        }
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
    /// A module-owned view comes from the module's active deployment on the
    /// connected node — nothing else, so with no node there is nothing to
    /// load yet, and no staged file is ever opened for it; the desktop's own
    /// views come from the staged file. With a view of the module already
    /// drawn, the deployment's view is prepared as its replacement:
    /// instantiated without `init`, restored from the drawn view's snapshot,
    /// and its first tree verified — and the active code is read again
    /// right before it is handed over, so a deployment that moved meanwhile
    /// is not installed. Every outcome for a module-owned view is one
    /// `view_source` log line with stable fields.
    fn load(
        module: &'static str,
        client: Option<&ducktape_rpc::Client>,
        generation: u64,
        mounted: &Arc<Mutex<Mounted>>,
    ) -> Result<Loaded, String> {
        use crate::backend::view_source::{self, ViewSource};
        if !view_source::module_owned(module) {
            let path = views_dir()?.join(format!("{module}_view.wasm"));
            return Self::load_from(module, &path).map(|guest| Loaded::Fresh(Box::new(guest)));
        }
        let logged = |hash: Option<&[u8; 32]>, state: &str, reason: &str| {
            log_source(module, hash, state, generation, reason);
        };
        let client = match client {
            Some(client) => client,
            None => {
                let reason = "not connected to a node yet";
                logged(None, "Failed", reason);
                return Err(reason.to_owned());
            }
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let started = Instant::now();
        let mut timing = LoadTiming {
            // the mount is the tab's first draw; a later load is one the
            // connection asked for, over a view drawn or not
            path: if generation == 0 { "tab" } else { "first" },
            ..LoadTiming::default()
        };
        let mut asked = view_source::Asked::default();
        let source = runtime.block_on(view_source::resolve(client, module, &mut asked));
        timing.status = asked.status;
        timing.fetch = asked.fetch;
        let source = match source {
            Ok(source) => source,
            Err(error) => {
                let reason = error.to_string();
                logged(None, "Failed", &reason);
                timing.log(module, None, started, "Failed");
                return Err(reason);
            }
        };
        let (hash, component, assets) = match source {
            ViewSource::NotActivated => {
                logged(None, "NotActivated", "");
                timing.log(module, None, started, "NotActivated");
                return Err(format!("the {module} module is not activated yet"));
            }
            ViewSource::Missing { hash } => {
                // a removal answered late, after the code moved on, is not
                // the current deployment's word
                let checked = Instant::now();
                let active = runtime.block_on(still_active(client, module, hash));
                timing.check = checked.elapsed();
                let active = match active {
                    Ok(active) => active,
                    Err(reason) => {
                        logged(Some(&hash), "Failed", &reason);
                        timing.log(module, Some(&hash), started, "Failed");
                        return Err(reason);
                    }
                };
                if !active {
                    let reason = "the active code moved while the view was prepared";
                    logged(Some(&hash), "Failed", reason);
                    timing.log(module, Some(&hash), started, "Failed");
                    return Err(reason.to_owned());
                }
                logged(Some(&hash), "Missing", "");
                timing.log(module, Some(&hash), started, "Missing");
                return Ok(Loaded::Empty(hash));
            }
            ViewSource::Ready {
                hash,
                component,
                assets,
            } => (hash, component, assets),
        };
        let shown = format!("{module} view @ {}", hex_short(&hash));
        let outcome = (|| -> Result<Loaded, String> {
            // the instance in the slot, if the deployment is a new one for
            // it: the replacement is seated only against that very
            // instance at that very tick count
            let against = {
                let locked = mounted.lock().expect("module view lock");
                match &locked.slot {
                    Slot::Ready(old) if old.hash == Some(hash) => return Ok(Loaded::Unchanged),
                    Slot::Ready(old) => Some((old.alive.clone(), old.ticks)),
                    _ => None,
                }
            };
            if against.is_some() {
                timing.path = "swap";
            }
            let compiled = Instant::now();
            let component = Self::compile(&component, &shown);
            timing.compile = compiled.elapsed();
            let component = component?;
            let seated = Instant::now();
            let prepared = (|| -> Result<Self, String> {
                let mut fresh = Self::instantiate(module, &component, &shown)?;
                fresh.deployed(hash, assets);
                match against {
                    // a view drawn carries its state over
                    Some((_, ticks)) if ticks > 0 => {
                        let snapshot = {
                            let mut locked = mounted.lock().expect("module view lock");
                            let Slot::Ready(old) = &mut locked.slot else {
                                return Err(
                                    "the view left while its replacement was prepared".into()
                                );
                            };
                            if !old.settled() {
                                return Err(
                                    "the view has pending work; its replacement waits".into()
                                );
                            }
                            old.snapshot()?
                        };
                        wire::Snapshot::decode(&snapshot)?;
                        fresh.restore(&snapshot, &shown)?;
                        let framed = Instant::now();
                        let frame = fresh.first_frame(&shown);
                        timing.first_frame = Some(framed.elapsed());
                        frame?;
                    }
                    // one mounted but never ticked has no state worth carrying;
                    // its replacement still proves its first tree before it
                    // takes the slot
                    Some(_) => {
                        fresh.init(&shown)?;
                        let framed = Instant::now();
                        let frame = fresh.first_frame(&shown);
                        timing.first_frame = Some(framed.elapsed());
                        frame?;
                    }
                    None => {
                        fresh.init(&shown)?;
                    }
                }
                Ok(fresh)
            })();
            // Include failed instantiate/restore/init work as well as success.
            timing.init = seated
                .elapsed()
                .saturating_sub(timing.first_frame.unwrap_or_default());
            let fresh = prepared?;
            // the deployment may have moved while this one was prepared;
            // the block that moved it starts another load
            let checked = Instant::now();
            let active = runtime.block_on(still_active(client, module, hash));
            timing.check = checked.elapsed();
            if !active? {
                return Err("the active code moved while the view was prepared".into());
            }
            Ok(match against {
                Some((alive, ticks)) => Loaded::Swap {
                    fresh: Box::new(fresh),
                    alive,
                    ticks,
                },
                None => Loaded::Fresh(Box::new(fresh)),
            })
        })();
        let state = match &outcome {
            Ok(Loaded::Fresh(_)) => {
                logged(Some(&hash), "Ready", "");
                "Ready"
            }
            Ok(Loaded::Unchanged) => "Unchanged",
            // `Swapped` is logged at the seat: the swap can still be refused there
            Ok(_) => "Swap",
            Err(reason) => {
                logged(Some(&hash), "Failed", reason);
                "Failed"
            }
        };
        timing.log(module, Some(&hash), started, state);
        outcome
    }

    /// The deployment's own: its assets and the surfaces that paint them
    /// by path, namespaced to this module and hash — a path the deployment
    /// does not ship leaves the slot empty and says so once.
    fn deployed(&mut self, hash: [u8; 32], assets: Arc<crate::backend::view_source::Assets>) {
        self.hash = Some(hash);
        self.assets = assets.clone();
        let module = self.module;
        // the paths said to be missing, once each — up to a budget, past
        // which one line says the guest keeps asking and nothing more is
        // kept or logged
        let missing: Arc<Mutex<std::collections::HashSet<String>>> = Arc::default();
        let lookup = move |args: &[wire::SurfaceValue]| -> Option<Vec<u8>> {
            let path = match args.first() {
                Some(wire::SurfaceValue::Str(path)) => path.clone(),
                _ => String::new(),
            };
            let found = artifact_asset(&assets, &path).map(<[u8]>::to_vec);
            if found.is_none() {
                let mut missing = missing.lock().expect("missing assets");
                let (path, reason) = match missing.len() {
                    n if n < MAX_MISSING_ASSETS => (path, "asset_missing"),
                    MAX_MISSING_ASSETS => (String::new(), "asset_missing_budget"),
                    _ => return None,
                };
                if missing.insert(path.clone()) {
                    tracing::info!(
                        target: "ducktape::app",
                        module,
                        hash = %crate::backend::hex_encode(&hash),
                        state = "Ready",
                        path,
                        reason,
                        "view_source"
                    );
                }
            }
            found
        };
        let svg = lookup.clone();
        self.surfaces.insert(
            "artifact_svg".into(),
            Arc::new(
                move |_key: &str, args: &[wire::SurfaceValue]| match svg(args) {
                    Some(bytes) => widget::svg(widget::svg::Handle::from_memory(bytes))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into(),
                    None => widget::Space::new().into(),
                },
            ),
        );
        self.surfaces.insert(
            "artifact_image".into(),
            Arc::new(
                move |_key: &str, args: &[wire::SurfaceValue]| match lookup(args) {
                    Some(bytes) => widget::image(widget::image::Handle::from_bytes(bytes))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into(),
                    None => widget::Space::new().into(),
                },
            ),
        );
    }

    /// Everything this instance was asked to do is done: nothing pending,
    /// no request the host has yet to route, no trap. A replacement not
    /// yet redrawn is settled too: the only requests its first tree
    /// carries are the subscriptions its restore rebuilt, which its own
    /// replacement rebuilds again — a tab not shown between two
    /// deployments is not stuck on the first.
    fn settled(&self) -> bool {
        self.fault.is_none()
            && self.pending.is_empty()
            && (self.staged || self.frame.requests.is_empty())
    }

    fn snapshot(&mut self) -> Result<Vec<u8>, String> {
        arm(&mut self.store);
        self.snapshot
            .call(&mut self.store, ())
            .map_err(|error| first_line(&error))?
            .0
    }

    fn restore(&mut self, snapshot: &[u8], shown: &str) -> Result<(), String> {
        arm(&mut self.store);
        self.restore
            .call(
                &mut self.store,
                (snapshot.to_vec(), cfg!(target_os = "macos")),
            )
            .map_err(|error| format!("{shown}: restore trapped: {}", first_line(&error)))?
            .0
            .map_err(|error| format!("{shown}: {error}"))
    }

    /// `on mount` runs in here, told which platform it keys for.
    fn init(&mut self, shown: &str) -> Result<(), String> {
        arm(&mut self.store);
        if let Err(error) = self
            .init
            .call(&mut self.store, (cfg!(target_os = "macos"),))
        {
            let trap = format!("{shown}: init trapped: {}", first_line(&error));
            return Err(panic_message(&mut self.store).unwrap_or(trap));
        }
        Ok(())
    }

    /// A restored instance's first tick: its whole tree, or it is no
    /// replacement. Its requests wait for the first redraw.
    fn first_frame(&mut self, shown: &str) -> Result<(), String> {
        self.tick();
        #[cfg(test)]
        if tests::FIRST_FRAME_TRAPS.swap(false, std::sync::atomic::Ordering::SeqCst) {
            self.fault = Some("the first frame trapped (test)".into());
        }
        if let Some(fault) = &self.fault {
            return Err(format!("{shown}: {fault}"));
        }
        if self.frame.root.is_none() {
            return Err(format!(
                "{shown}: the replacement did not publish a complete tree"
            ));
        }
        self.ticks += 1;
        self.staged = true;
        Ok(())
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
        Self::from_bytes(module, &bytes, &shown)
    }

    /// The component instantiated and mounted; `shown` names it in errors.
    fn from_bytes(module: &'static str, bytes: &[u8], shown: &str) -> Result<Self, String> {
        let component = Self::compile(bytes, shown)?;
        let mut guest = Self::instantiate(module, &component, shown)?;
        guest.init(shown)?;
        Ok(guest)
    }

    /// The component's bytes checked and compiled — the cranelift stage of
    /// a load, measured on its own.
    fn compile(bytes: &[u8], shown: &str) -> Result<Arc<Component>, String> {
        if bytes.len() as u64 > MAX_MODULE_BYTES {
            return Err(format!(
                "{shown}: past the {MAX_MODULE_BYTES} byte module limit"
            ));
        }
        // its preferred size is for placing a new window; the tab embeds
        ui_lang_wire::manifest::read_manifest(bytes)
            .ok_or_else(|| format!("{shown}: the component's manifest cannot be read"))?;
        compiled_view(bytes).map_err(|error| format!("{shown}: {error}"))
    }

    /// The component instantiated, its exports bound, nothing run yet: a
    /// fresh view is `init`ed, a replacement `restore`d.
    fn instantiate(
        module: &'static str,
        component: &Component,
        shown: &str,
    ) -> Result<Self, String> {
        let engine = engine();
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
            .define_unknown_imports_as_traps(component)
            .map_err(|error| error.to_string())?;
        arm(&mut store);
        let instance = linker
            .instantiate(&mut store, component)
            .map_err(|error| format!("{shown}: {}", first_line(&error)))?;
        let init = instance
            .get_typed_func::<(bool,), ()>(&mut store, "init")
            .map_err(|error| format!("{shown}: {error}"))?;
        let tick = instance
            .get_typed_func::<(Vec<u8>,), (Vec<u8>,)>(&mut store, "tick")
            .map_err(|error| format!("{shown}: {error}"))?;
        let snapshot = instance
            .get_typed_func::<(), (Result<Vec<u8>, String>,)>(&mut store, "snapshot")
            .map_err(|error| format!("{shown}: {error}"))?;
        let restore = instance
            .get_typed_func::<(Vec<u8>, bool), (Result<(), String>,)>(&mut store, "restore")
            .map_err(|error| format!("{shown}: {error}"))?;
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
            files_save_namespace: (module == "files")
                .then(|| crate::backend::fresh_operation_id("files-view".into())),
            intents: Vec::new(),
            fault: None,
            assets: Arc::default(),
            hash: None,
            alive: Arc::new(()),
            staged: false,
            snapshot,
            restore,
            init,
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
    /// host surface the guest routed (`-> handler _`) reaches that handler
    /// with the value the surface produced; one it left unrouted is the
    /// app's own ring, whose event was queued where the surface keeps it,
    /// and the app is told to drain it.
    fn deliver(&mut self, output: Output) {
        if let Output::Surface {
            handler: None,
            value,
        } = output
        {
            // the shell composer's submit carries its body; the other
            // unrouted surfaces only say that something happened
            if self.module == "shell" {
                self.intents.extend(crate::shell_composer::intent(&value));
            } else if self.module == "chat" || self.module == "forge" {
                self.intents.extend(crate::composer_surface::intent(&value));
            } else {
                self.intents.push(ModuleViewEvent {
                    kind: surface_intent(self.module).into(),
                    detail: String::new(),
                });
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
        let mut bytes = props.clone().unwrap_or_default();
        if let Some(namespace) = &self.files_save_namespace
            && let Ok(serde_json::Value::Object(mut facts)) = serde_json::from_slice(&bytes)
        {
            facts.insert("save_namespace".into(), namespace.clone().into());
            bytes = serde_json::to_vec(&facts).expect("JSON object serializes");
        }
        self.pending.push(wire::Event::Response {
            id,
            result: Ok(bytes),
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
        if self.staged {
            // a replacement's first tree is already here; only its
            // requests and cancels are still to route
            self.staged = false;
        } else {
            let quiet = self.ticks > 0 && !self.frame.busy && self.pending.is_empty();
            if quiet {
                return false;
            }
            self.tick();
            self.ticks += 1;
        }
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
                            root.for_each_mut(&mut |node| match node {
                                wire::Node::Svg { bytes, .. } => *bytes = None,
                                wire::Node::Image { data, .. }
                                | wire::Node::ImageViewer { data, .. } => *data = None,
                                _ => {}
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
    /// The load `content` was rendered under: what the reader does in a
    /// tree of an earlier one is not handed to the view of a later one.
    generation: u64,
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
        let Mounted {
            slot,
            props,
            generation,
            ..
        } = &mut *mounted;
        let guest = match slot {
            Slot::Ready(guest) => guest,
            Slot::Loading => {
                shell.request_redraw_at(window::RedrawRequest::At(
                    iced::time::Instant::now() + LOAD_POLL,
                ));
                return;
            }
            Slot::Failed(_) | Slot::Empty => return,
        };
        let outputs = if *generation == self.generation {
            outputs
        } else {
            // a tree of an earlier load: its messages index nothing here
            self.generation = *generation;
            self.rev = 0;
            Vec::new()
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
pub(crate) mod tests {
    use super::*;

    // Explicit manual measurement, not a wall-clock performance assertion.
    // Use a fresh XDG cache directory for cold-cache evidence.
    #[test]
    #[ignore = "requires DUCKTAPE_BENCH_VIEW pointing to a matching governance guest"]
    fn measure_governance_view_code_and_first_tree() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
        let path =
            std::env::var_os("DUCKTAPE_BENCH_VIEW").expect("DUCKTAPE_BENCH_VIEW is required");
        let bytes = std::fs::read(path).expect("read matching governance wasm");
        let mut config = Config::new();
        config.cranelift_opt_level(OptLevel::Speed);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let uncached = Engine::new(&config).unwrap();
        let before = Instant::now();
        let baseline = Component::new(&uncached, &bytes).expect("uncached compilation");
        let baseline_time = before.elapsed();
        drop(baseline);
        let before = Instant::now();
        let cold = Guest::compile(&bytes, "benchmark").unwrap();
        let cold_time = before.elapsed();
        let before = Instant::now();
        let warm = Guest::compile(&bytes, "benchmark").unwrap();
        let warm_time = before.elapsed();
        assert!(
            Arc::ptr_eq(&cold, &warm),
            "actual guest code must be reused"
        );
        let before = Instant::now();
        let mut first = Guest::instantiate("governance", &warm, "benchmark").unwrap();
        first.init("benchmark").unwrap();
        let initialized = before.elapsed();
        let before = Instant::now();
        first.redraw(&register());
        first.redraw(&register());
        assert!(first.fault.is_none(), "{:?}", first.fault);
        assert!(
            texts(&first).iter().any(|text| text == "node-7"),
            "real register tree: {:?}",
            texts(&first)
        );
        let tree_time = before.elapsed();
        let second = Guest::from_bytes("governance", &bytes, "benchmark").unwrap();
        assert!(
            !Arc::ptr_eq(&first.alive, &second.alive),
            "code reuse must not reuse mutable instances"
        );
        assert!(second.frame.root.is_none());
        tracing::info!(
            baseline_compile_us = baseline_time.as_micros(),
            cold_compile_us = cold_time.as_micros(),
            warm_compile_us = warm_time.as_micros(),
            instantiate_init_us = initialized.as_micros(),
            first_tree_us = tree_time.as_micros(),
            source_bytes = bytes.len(),
            "view_code_benchmark"
        );
    }

    #[test]
    fn view_code_reuses_identical_bytes_but_not_changed_code() {
        let engine = Engine::default();
        let cache = Mutex::new(ViewCodeCache::default());
        let first = compile_view(&engine, &cache, b"(component)").unwrap();
        let repeated = compile_view(&engine, &cache, b"(component)").unwrap();
        assert!(
            Arc::ptr_eq(&first, &repeated),
            "warm loads must reuse compiled code"
        );
        let changed = compile_view(&engine, &cache, b"(component (type (func)))").unwrap();
        assert!(!Arc::ptr_eq(&first, &changed));
        assert!(compile_view(&engine, &cache, b"invalid wasm").is_err());
        assert_eq!(cache.lock().unwrap().entries.len(), 2);
    }

    #[test]
    fn view_code_evicts_old_entries_and_bounds_retained_source_size() {
        let engine = Engine::default();
        let cache = Mutex::new(ViewCodeCache::default());
        let oldest = compile_view(&engine, &cache, b"(component)").unwrap();
        for i in 0..COMPILED_VIEW_LIMIT {
            let source = format!("(component) ;; entry {i}");
            compile_view(&engine, &cache, source.as_bytes()).unwrap();
        }
        assert_eq!(cache.lock().unwrap().entries.len(), COMPILED_VIEW_LIMIT);
        let revisited = compile_view(&engine, &cache, b"(component)").unwrap();
        assert!(!Arc::ptr_eq(&oldest, &revisited));
        let mut retained = cache.lock().unwrap();
        retained.insert(CompiledView {
            hash: [99; 32],
            source_bytes: COMPILED_VIEW_SOURCE_BYTES,
            component: revisited,
        });
        assert_eq!(retained.entries.len(), 1);
        assert_eq!(retained.entries[0].hash, [99; 32]);
    }

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
        assert_eq!(intents_of("agents"), ["status", "save", "register"]);
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

    /// The key and input-handler index of the input whose placeholder is
    /// `hint`.
    fn input_named(guest: &Guest, hint: &str) -> (String, u32) {
        let mut root = guest.frame.root.clone().expect("a tree");
        let mut found = None;
        root.for_each_mut(&mut |node| {
            if let wire::Node::Input {
                key,
                placeholder,
                on_input,
                ..
            } = node
                && placeholder == hint
            {
                found = Some((key.clone(), *on_input));
            }
        });
        found.expect("an input with that placeholder")
    }

    /// The message index the button labelled `name` — by its `label=`, or
    /// by the text it shows — would send.
    fn button_message(guest: &Guest, name: &str) -> u32 {
        let mut root = guest.frame.root.clone().expect("a tree");
        let mut message = None;
        root.for_each_mut(&mut |node| {
            if let wire::Node::Button {
                label,
                content,
                on_press,
                ..
            } = node
                && (label.as_deref() == Some(name)
                    || matches!(content, wire::ButtonContent::Label(text) if text == name))
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
        assert_eq!(
            surface_names(&guest),
            ["artifact_svg"],
            "the header leaves the deployment's seal to the host"
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
    /// register; nothing leaves it until a reader edits a record.
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
                    "controller": "7", "live": false,
                    "allowed_actions": ["chat.post"],
                    "caps": {
                        "forge_read": ["ducktape"], "forge_push": [], "duckfs_read": [],
                        "duckfs_write": [], "tools": [], "secrets": [], "pages_write": ["*"],
                        "subagent_budget": 0
                    },
                    "skills": [
                        {"name": "review", "source_prefix": "/shared/skills/review", "source_snapshot": "", "always": true},
                        {"name": "style", "source_prefix": "/shared/skills/style", "source_snapshot": "", "always": false},
                        {"name": "tests", "source_prefix": "/shared/skills/tests", "source_snapshot": "", "always": false}
                    ]
                }],
                "capabilities": ["claude", "review"], "actions": ["chat.post", "tasks.create"],
                "account": "", "committed": 0,
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

    /// The bundled Pages view through the host: the sidebar and the header,
    /// a pick that leaves as an intent carrying the rail's draft, and the
    /// document slot the host paints — what the reader does in it comes back
    /// as the `edited` intent rather than going to the guest.
    #[test]
    fn the_staged_pages_view_boots_takes_the_facts_and_leaves_the_document_to_the_host() {
        let Some(staged) = staged("pages") else {
            return;
        };
        let mut guest = Guest::load_from("pages", &staged).expect("the view loads");
        assert!(guest.surfaces.contains_key("page_document"));
        guest.redraw(&None);
        assert!(
            texts(&guest).iter().any(|text| text == "Not connected"),
            "{:?}",
            texts(&guest)
        );
        let props = pages_facts();
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in ["Pages", "Alpha", "Beta", "✓ synced"] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} in {shown:?}"
            );
        }
        assert_eq!(surface_names(&guest), ["page_document"]);

        guest.deliver(Output::Activate(button_message(&guest, "Beta")));
        guest.redraw(&props);
        assert_eq!(
            std::mem::take(&mut guest.intents),
            [ModuleViewEvent {
                kind: "choose".into(),
                detail: r#"{"id":"beta","comment_draft":""}"#.into(),
            }]
        );

        // what the reader does in the host's document never reaches the guest
        guest.deliver(Output::Surface {
            handler: None,
            value: wire::SurfaceValue::Unit,
        });
        assert!(guest.pending.is_empty());
        assert_eq!(
            guest.intents,
            [ModuleViewEvent {
                kind: "edited".into(),
                detail: String::new(),
            }]
        );
        assert!(guest.fault.is_none());
    }

    /// The bundled Chat view through the host: the rooms and the stream,
    /// a room pressed that leaves as `choose_channel`, the composer slot
    /// the host paints per room, and its submit crossing as the `composer`
    /// intent rather than a guest request.
    #[test]
    fn the_staged_chat_view_boots_takes_the_facts_and_leaves_the_composer_to_the_host() {
        let Some(staged) = staged("chat") else {
            return;
        };
        let mut guest = Guest::load_from("chat", &staged).expect("the view loads");
        assert!(guest.surfaces.contains_key("chat_composer"));
        guest.redraw(&None);
        let props = chat_facts();
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in ["testnet", "general", "ops", "first light"] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} in {shown:?}"
            );
        }
        assert_eq!(surface_names(&guest), ["chat_composer"]);

        guest.deliver(Output::Activate(button_message(&guest, "ops")));
        guest.redraw(&props);
        assert_eq!(
            std::mem::take(&mut guest.intents),
            [ModuleViewEvent {
                kind: "choose_channel".into(),
                detail: r#"{"id":"channel-b"}"#.into(),
            }]
        );

        // a submit in the host's composer is the `composer` intent, and an
        // edit there never reaches the guest
        guest.deliver(Output::Surface {
            handler: None,
            value: wire::SurfaceValue::Record {
                name: "composer".into(),
                fields: vec![
                    (
                        "scope".into(),
                        wire::SurfaceValue::Str("testnet\u{1f}channel-b".into()),
                    ),
                    ("kind".into(), wire::SurfaceValue::Str("message".into())),
                    ("body".into(), wire::SurfaceValue::Str("hello".into())),
                    ("id".into(), wire::SurfaceValue::Str("message-1".into())),
                ],
            },
        });
        guest.deliver(Output::Surface {
            handler: None,
            value: wire::SurfaceValue::Unit,
        });
        assert!(guest.pending.is_empty());
        assert_eq!(guest.intents.len(), 1);
        assert_eq!(guest.intents[0].kind, "composer");
        assert!(guest.intents[0].detail.contains(r#""body":"hello""#));
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

    /// The bundled Files view through the host: the listing, a typed name
    /// that leaves as a `mkdir` intent, the three preview surfaces in the
    /// guest's tree, and a link the Markdown reader activates coming back
    /// through the guest's own route as an `open_link` intent.
    #[test]
    fn the_staged_files_view_boots_takes_the_listing_and_routes_a_link_through_its_surface() {
        let Some(staged) = staged("files") else {
            return;
        };
        let mut guest = Guest::load_from("files", &staged).expect("the view loads");
        guest.redraw(&None);
        let props = files_facts();
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in ["duckfs", "/shared", "1 file · 1 dir", "README.md", "1 KB"] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} in {shown:?}"
            );
        }
        assert_eq!(surface_names(&guest), ["agent_markdown"]);
        assert!(guest.surfaces.contains_key("agent_markdown"));
        assert!(guest.surfaces.contains_key("forge_code"));
        assert!(guest.surfaces.contains_key("picture"));

        // the Markdown reader's link goes back through the guest's route
        let mut route = None;
        if let Some(root) = &guest.frame.root {
            root.clone().for_each_mut(&mut |node| {
                if let wire::Node::Surface { on_event, .. } = node {
                    route = *on_event;
                }
            });
        }
        guest.deliver(Output::Surface {
            handler: route,
            value: wire::SurfaceValue::Str("https://duck.example/x".into()),
        });
        guest.redraw(&props);
        assert_eq!(
            std::mem::take(&mut guest.intents),
            [ModuleViewEvent {
                kind: "open_link".into(),
                detail: r#"{"url":"https://duck.example/x"}"#.into(),
            }]
        );

        // a typed name leaves trimmed, as a mkdir under the crumb
        let (key, handler) = input_named(&guest, "new name…");
        guest.deliver(Output::Edit {
            key,
            handler,
            text: "  reports  ".into(),
        });
        guest.redraw(&props);
        guest.deliver(Output::Activate(button_message(&guest, "+ Folder")));
        guest.redraw(&props);
        assert_eq!(
            std::mem::take(&mut guest.intents),
            [ModuleViewEvent {
                kind: "mkdir".into(),
                detail: r#"{"name":"reports"}"#.into(),
            }]
        );
        assert!(guest.fault.is_none());
    }

    #[test]
    fn the_staged_files_draft_keeps_its_original_file_and_save_snapshot() {
        let staged = staged("files").expect("the actual Files Wasm fixture is required");
        let mut guest = Guest::load_from("files", &staged).expect("Files loads");
        let mut facts: serde_json::Value = serde_json::from_slice(&files_facts().unwrap()).unwrap();
        let original_text = format!("X{}", facts["preview_text"].as_str().unwrap());
        let props = |facts: &serde_json::Value| Some(serde_json::to_vec(facts).unwrap());
        guest.redraw(&None);
        guest.redraw(&props(&facts));
        guest.deliver(Output::Activate(button_message(&guest, "Edit")));
        guest.redraw(&props(&facts));
        let mut editor = None;
        guest.frame.root.clone().unwrap().for_each_mut(&mut |node| {
            if let wire::Node::Editor {
                key,
                reset,
                on_edit: Some(handler),
                ..
            } = node
            {
                editor = Some((key.clone(), *reset, *handler));
            }
        });
        let (key, reset, handler) = editor.expect("editable document");
        guest.deliver(Output::EditorAction {
            key,
            reset,
            handler,
            action: iced::widget::text_editor::Action::Edit(
                iced::widget::text_editor::Edit::Insert('X'),
            ),
        });
        guest.redraw(&props(&facts));
        let old_save = button_message(&guest, "Save");
        facts["network_scope"] = "network-b".into();
        facts["context"] = "connection-b".into();
        facts["preview_path"] = "/shared/other.md".into();
        facts["preview_text"] = "B source".into();
        guest.redraw(&props(&facts));
        guest.deliver(Output::Activate(old_save));
        guest.redraw(&props(&facts));
        assert!(guest.intents.is_empty(), "an old Save cannot target B");
        assert!(
            texts(&guest)
                .iter()
                .any(|text| text == "Unsaved changes to:")
        );
        assert!(texts(&guest).iter().any(|text| text == "/shared/README.md"));

        facts["network_scope"] = "network-a".into();
        facts["context"] = "connection-a-reconnected".into();
        facts["preview_path"] = "/shared/README.md".into();
        facts["preview_base"] = "external-new-snapshot".into();
        facts["preview_text"] = "external replacement".into();
        guest.redraw(&props(&facts));
        let draft_text = |guest: &Guest| {
            let mut value = None;
            guest.frame.root.clone().unwrap().for_each_mut(&mut |node| {
                if let wire::Node::Editor { text, .. } = node {
                    value = Some(text.clone());
                }
            });
            value.expect("the retained editor")
        };
        assert_eq!(draft_text(&guest), original_text);
        guest.deliver(Output::Activate(button_message(&guest, "Save")));
        guest.redraw(&props(&facts));
        let saves = std::mem::take(&mut guest.intents);
        assert_eq!(saves.len(), 1, "one Save");
        let save = &saves[0];
        assert_eq!(save.kind, "save");
        let payload: serde_json::Value = serde_json::from_str(&save.detail).unwrap();
        assert_eq!(payload["path"], "/shared/README.md");
        assert_eq!(payload["text"], original_text);
        assert_eq!(payload["base"], "snapshot-a");
        assert_eq!(payload["context"], "connection-a-reconnected");
        facts["save_reply"] = serde_json::json!({"replies":[{
            "context": payload["context"], "namespace": payload["namespace"], "request": payload["request"],
            "success": false, "message": "The file changed elsewhere. Your edits are kept."
        }], "overflow":""});
        guest.redraw(&props(&facts));
        assert_eq!(draft_text(&guest), original_text);
        assert!(
            texts(&guest)
                .iter()
                .any(|text| text == "The file changed elsewhere. Your edits are kept.")
        );
        assert!(guest.fault.is_none());
    }

    #[test]
    fn fresh_files_wasm_instances_reject_retained_and_late_old_save_successes() {
        let staged = staged("files").expect("actual Files Wasm fixture required");
        let mut old = Guest::load_from("files", &staged).unwrap();
        let old_props = files_facts();
        old.redraw(&None);
        old.redraw(&old_props);
        old.deliver(Output::Activate(button_message(&old, "Edit")));
        old.redraw(&old_props);
        old.deliver(Output::Activate(button_message(&old, "Save")));
        old.redraw(&old_props);
        let old_intents = std::mem::take(&mut old.intents);
        assert_eq!(old_intents.len(), 1);
        let old_save: serde_json::Value = serde_json::from_str(&old_intents[0].detail).unwrap();
        let old_namespace = old_save["namespace"].as_str().unwrap().to_owned();
        assert_eq!(old_save["request"], 1);
        for retained in [true, false] {
            let mut guest = Guest::load_from("files", &staged).unwrap();
            assert_ne!(guest.files_save_namespace.as_ref().unwrap(), &old_namespace);
            let mut facts: serde_json::Value =
                serde_json::from_slice(&files_facts().unwrap()).unwrap();
            let success = serde_json::json!({"replies":[{"context":"connection-a", "namespace":old_namespace, "request":1, "success":true, "message":""}], "overflow":""});
            if retained {
                facts["save_reply"] = success.clone();
            }
            let props = |value: &serde_json::Value| Some(serde_json::to_vec(value).unwrap());
            guest.redraw(&None);
            guest.redraw(&props(&facts));
            guest.deliver(Output::Activate(button_message(&guest, "Edit")));
            guest.redraw(&props(&facts));
            guest.deliver(Output::Activate(button_message(&guest, "Save")));
            guest.redraw(&props(&facts));
            let saves = std::mem::take(&mut guest.intents);
            assert_eq!(saves.len(), 1);
            let save: serde_json::Value = serde_json::from_str(&saves[0].detail).unwrap();
            assert_eq!(save["request"], 1);
            assert_eq!(
                save["namespace"],
                guest.files_save_namespace.as_ref().unwrap().as_str()
            );
            facts["save_reply"] = success;
            // A changed loading prop forces delivery even when the old success was already retained.
            facts["loading"] = true.into();
            guest.redraw(&props(&facts));
            assert!(
                texts(&guest).iter().any(|text| text == "Save"),
                "old instance completion must not close the fresh editor"
            );
            facts["save_reply"]["replies"][0]["namespace"] = save["namespace"].clone();
            facts["loading"] = false.into();
            guest.redraw(&props(&facts));
            assert!(!texts(&guest).iter().any(|text| text == "Save"));
            assert!(guest.fault.is_none());
        }
    }

    /// A module-owned view has one source, the module's deployment on the
    /// connected node: with no node it is not there yet, and the staged file
    /// a desktop view would take is never opened for it.
    #[test]
    fn a_module_owned_view_never_comes_from_the_staged_file() {
        // a staged file for every one of them, where `views_dir` would look
        let staged = tempfile::tempdir().expect("a staging dir");
        for module in crate::backend::view_source::MODULE_OWNED {
            std::fs::write(staged.path().join(format!("{module}_view.wasm")), b"\0asm")
                .expect("staged");
        }
        // SAFETY: the one test that sets this variable; `views_dir` reads it
        // only for a desktop view, which no test loads through `Guest::load`.
        unsafe { std::env::set_var("DUCKTAPE_VIEWS_DIR", staged.path()) };
        for module in crate::backend::view_source::MODULE_OWNED {
            let mounted = Arc::new(Mutex::new(Mounted {
                slot: Slot::Loading,
                props: None,
                generation: 0,
                hash: None,
                in_flight: true,
                wanted: None,
            }));
            assert_eq!(
                Guest::load(module, None, 0, &mounted).err().as_deref(),
                Some("not connected to a node yet"),
                "{module}"
            );
        }
        assert!(!crate::backend::view_source::module_owned("settings"));
    }

    /// A load still in flight when the app moves to another node lands
    /// nowhere: the node it was asked of may answer late, and its view is
    /// not the view of the node the app is on.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_load_the_previous_node_answers_late_is_not_installed() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::node;
        use module_artifact::{ModuleArtifact, ViewArtifact};
        let Some(staged) = staged("governance") else {
            return;
        };
        let component = std::fs::read(staged).expect("the staged view");
        let deployment = |asset: &str| ModuleArtifact {
            component: vec![1, 2, 3],
            index: None,
            view: Some(ViewArtifact {
                component: component.clone(),
                assets: [(asset.to_owned(), b"<svg/>".to_vec())].into(),
            }),
        };
        let status = |artifact: &ModuleArtifact| {
            serde_json::json!({"module_status": {"modules": [
                {"module_id": "forge", "active_code_hash": artifact.hash().to_vec(),
                 "pending": null, "history": []}
            ]}})
        };
        let (a, b) = (deployment("a.svg"), deployment("b.svg"));
        let hold = Arc::new(tokio::sync::Notify::new());
        let node_a = node(status(&a), Some(a), Some(hold.clone())).await;
        let node_b = node(status(&b), Some(b), None).await;

        // mounted with no node: fails fast, then A is asked and holds
        let mounted = mounted("forge");
        let asked_of_a = connected(&node_a);
        // the app moves to B while A is still composing its answer
        let asked_of_b = connected(&node_b);
        for load in asked_of_b {
            load.join().expect("the load on B");
        }
        hold.notify_one();
        for load in asked_of_a {
            load.join().expect("the load on A");
        }
        let locked = mounted.lock().expect("module view lock");
        let Slot::Ready(guest) = &locked.slot else {
            panic!("the view of B is not there");
        };
        assert!(
            guest.assets.contains_key("b.svg"),
            "{:?}",
            guest.assets.keys()
        );
        assert!(
            !guest.assets.contains_key("a.svg"),
            "A's late answer landed"
        );
    }

    /// The bundled Shell view through the host: the facts, the welcome
    /// for a picked credential, the three host slots, a surface switch as
    /// an intent — and a send, which only the host's composer can raise.
    #[test]
    fn the_staged_shell_view_boots_takes_the_facts_and_leaves_the_composer_to_the_host() {
        let Some(staged) = staged("shell") else {
            return;
        };
        let mut guest = Guest::load_from("shell", &staged).expect("the view loads");
        guest.redraw(&None);
        let props = Some(
            serde_json::to_vec(&serde_json::json!({
                "dark": false, "connected": true, "surface": "tasks", "setup_open": false,
                "identity_options": ["team-codex · Codex"], "identity": "team-codex · Codex",
                "provider_initial": "C", "credential": "team-codex",
                "host_node_options": ["This node"], "host_node": "This node",
                "credentials_loading": false, "terminal_running": false,
                "terminal_busy": false, "terminal_title": "", "terminal_error": "",
                "entries": [], "activity": [], "chat_busy": false, "chat_status": "",
                "chat_detail": "", "live": "", "saga_id": "", "detached_saga": "",
                "run_line": "team-codex · Codex · This node", "grant_note": "",
                "terminal_note": "A sandboxed Codex session.", "composer_hint": "Message Codex…",
                "task_blurb": "Each message runs an agent in a sandbox on this node.",
                "register_hint": "Register one with `ducktape user cred add codex`"
            }))
            .expect("props encode"),
        );
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in [
            "Shell",
            "What should the agent do?",
            "team-codex · Codex · This node",
        ] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} in {shown:?}"
            );
        }
        assert_eq!(surface_names(&guest), ["shell_composer"]);
        assert!(guest.surfaces.contains_key("shell_composer"));
        assert!(guest.surfaces.contains_key("agent_terminal_surface"));
        assert!(guest.surfaces.contains_key("agent_markdown"));
        guest.deliver(Output::Activate(button_message(&guest, "Terminal")));
        guest.redraw(&props);
        assert_eq!(
            std::mem::take(&mut guest.intents),
            [ModuleViewEvent {
                kind: "surface".into(),
                detail: r#"{"surface":"terminal"}"#.into(),
            }]
        );
        // the composer's submit is the host's intent, not a guest request
        guest.deliver(Output::Surface {
            handler: None,
            value: wire::SurfaceValue::Str("ship it".into()),
        });
        assert_eq!(
            std::mem::take(&mut guest.intents),
            [ModuleViewEvent {
                kind: "send".into(),
                detail: r#"{"body":"ship it"}"#.into(),
            }]
        );
        assert!(guest.fault.is_none());
    }

    /// The bundled Forge view through the host: the register, then a repo
    /// opened from its card as the intent the handler signs — and with the
    /// item open, the review body leaves as the intent's payload while the
    /// three reader surfaces are the host's to paint.
    #[test]
    fn the_staged_forge_view_boots_takes_the_register_and_opens_a_repo() {
        let Some(staged) = staged("forge") else {
            return;
        };
        let mut guest = Guest::load_from("forge", &staged).expect("the view loads");
        guest.redraw(&None);
        // the whole register as the app encodes it — a literal, since the
        // document is past `json!`'s recursion limit
        let props = forge_facts();
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in ["duckhouse", "core"] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} in {shown:?}"
            );
        }
        guest.deliver(Output::Activate(button_message(&guest, "Open repo")));
        guest.redraw(&props);
        assert_eq!(
            std::mem::take(&mut guest.intents),
            [ModuleViewEvent {
                kind: "open_repo".into(),
                detail: r#"{"name":"core"}"#.into(),
            }]
        );
        for surface in ["picture", "forge_markdown", "forge_code", "forge_composer"] {
            assert!(
                surfaces_of("forge").contains_key(surface),
                "the host paints the {surface} slot the view leaves"
            );
        }
        assert!(guest.fault.is_none());
    }

    /// A deployment of `module` on the fake node: the staged governance
    /// component as its view, told apart by the one asset it ships.
    fn deployment(component: &[u8], asset: &str) -> module_artifact::ModuleArtifact {
        module_artifact::ModuleArtifact {
            component: vec![1, 2, 3],
            index: None,
            view: Some(module_artifact::ViewArtifact {
                component: component.to_vec(),
                assets: [(asset.to_owned(), b"<svg/>".to_vec())].into(),
            }),
        }
    }

    fn slot_assets(mounted: &Arc<Mutex<Mounted>>) -> Vec<String> {
        let locked = mounted.lock().expect("module view lock");
        match &locked.slot {
            Slot::Ready(guest) => guest.assets.keys().cloned().collect(),
            other => panic!("not a view: {}", slot_name(other)),
        }
    }

    fn slot_name(slot: &Slot) -> String {
        match slot {
            Slot::Loading => "loading".into(),
            Slot::Ready(_) => "ready".into(),
            Slot::Empty => "empty".into(),
            Slot::Failed(reason) => format!("failed: {reason}"),
        }
    }

    /// The connection is one per process: the tests that move it take
    /// turns, so one's node is not another's — including the round trip
    /// over a real node in `backend::tests::wire`, whose connect reloads
    /// every seat here from a node that runs none of these modules.
    pub(crate) async fn connection_turn() -> tokio::sync::MutexGuard<'static, ()> {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
        static TURN: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
        TURN.get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await
    }

    /// The seat of `module` as no test has touched it: the registry is one
    /// per process, and every deployment test leaves its module drawn.
    fn fresh(module: &'static str) -> Arc<Mutex<Mounted>> {
        registry().lock().expect("module views").remove(module);
        mounted(module)
    }

    /// Set by a test: the next candidate's first frame traps.
    pub(super) static FIRST_FRAME_TRAPS: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    /// One open proposal, as the host pushes the register.
    fn register() -> Option<Vec<u8>> {
        Some(
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
        )
    }

    /// Holds the node's next blob answer until released.
    fn hold_blob(
        node: &crate::backend::view_source::tests::FakeDeployment,
    ) -> Arc<tokio::sync::Notify> {
        let hold = Arc::new(tokio::sync::Notify::new());
        *node.hold.lock().unwrap() = Some(hold.clone());
        hold
    }

    /// Holds the node's next status answer until released.
    fn hold_status(
        node: &crate::backend::view_source::tests::FakeDeployment,
    ) -> Arc<tokio::sync::Notify> {
        let hold = Arc::new(tokio::sync::Notify::new());
        *node.hold_status.lock().unwrap() = Some(hold.clone());
        hold
    }

    fn join_all(loads: Vec<std::thread::JoinHandle<()>>) {
        for load in loads {
            load.join().expect("the load");
        }
    }

    fn files_facts() -> Option<Vec<u8>> {
        Some(
        serde_json::to_vec(&serde_json::json!({
            "path": "/shared", "listed": true,
            "entries": [
                {"key": 1, "path": "/shared/docs", "name": "docs", "kind": "dir", "size": 2, "object": "aa"},
                {"key": 2, "path": "/shared/README.md", "name": "README.md", "kind": "file", "size": 1024, "object": "bb"}
            ],
            "directories": [
                {"key": 1, "path": "/shared/docs", "name": "docs", "kind": "dir", "size": 2, "object": "aa"}
            ],
            "connected": true, "loading": false,
            "network_scope": "network-a", "context": "connection-a", "preview_base": "snapshot-a",
            "save_reply": {"replies":[], "overflow":""},
            "preview_path": "/shared/README.md",
            "preview_entry": {"key": 2, "path": "/shared/README.md", "name": "README.md", "kind": "file", "size": 1024, "object": "bb"},
            "delete_target": "", "diff_from": "", "diff": [], "history": [],
            "preview_truncated": false, "preview_binary": false, "preview_picture": false,
            "preview_width": 0, "preview_height": 0,
            "preview_text": "# Hello\n\n[a link](https://duck.example/x)\n",
            "preview_display_text": "# Hello\n\n[a link](https://duck.example/x)\n",
            "preview_display_clipped": false, "display_omitted": 0, "display_shortened": false, "display_unavailable": false,
            "dark": false, "write_refusal": "", "writes": 0
        }))
        .expect("props encode"),
    )
    }

    fn pages_facts() -> Option<Vec<u8>> {
        Some(
            serde_json::to_vec(&serde_json::json!({
                "dark": false, "connected": true, "loading": false, "busy": false,
                "page_link": "duck://pages/alpha",
                "pages": [
                    {"id": "alpha", "title": "Alpha", "parent": "", "prefix": "", "child_count": 0},
                    {"id": "beta", "title": "Beta", "parent": "", "prefix": "", "child_count": 0}
                ],
                "page_create_open": false, "active_page": "alpha",
                "active_page_title": "Alpha", "active_page_parent": "",
                "page_searching": false, "page_search_hits": [], "page_search_query": "",
                "page_delete_armed": false, "autosave": "saved", "page_refusal": "",
                "doc_tabs": [{"id": "alpha", "title": "Alpha", "active": true}],
                "subpages": [], "orphaned_comment_drafts": [],
                "block_comments_open": false, "thread_total": 0, "comment_rows": [],
                "threads_loading": false, "threads_has_more": false, "active_thread": "",
                "thread_resolved": false, "active_thread_anchor": "", "comments": [],
                "comments_loading": false, "comments_has_more": false, "compose_hint": "",
                "seed_rev": 0, "page_seed": "", "comment_seed": ""
            }))
            .expect("props encode"),
        )
    }

    fn forge_facts() -> Option<Vec<u8>> {
        Some(
            br#"{
          "dark": false, "connected": true, "org": "duckhouse", "about": "",
          "tier": "validator", "network_chain_id": "mynet#d0cdf950",
          "connected_rpc": "http://127.0.0.1:1",
          "repos": [{"name": "core", "head": "main"}],
          "list_phase": "ready", "open_repo": "", "repo_menu": false,
          "repo_phase": "idle", "branches": [], "tab": "code", "items": [],
          "forge_item_number": 0, "item_phase": "idle", "forge_item_kind": "",
          "forge_item_title": "", "forge_item_state": "", "forge_item_author": "",
          "forge_item_branches": "", "forge_item_body": "", "forge_item_blocks": [],
          "forge_item_files_changed": 0, "forge_item_additions": 0,
          "forge_item_deletions": 0, "diff_rows": [], "forge_item_diff_truncated": false,
          "forge_item_merge_oid": "", "forge_item_source_oid": "",
          "forge_item_approvals": 0, "forge_item_change_requests": 0,
          "forge_item_reviews": [], "merge_conflicts": [], "has_merge_conflicts": false, "merge_busy": false,
          "review_verdict": "comment", "review_busy": false, "staged_comments": [], "has_staged_comments": false,
          "comment_cap_reached": false, "discussion": [], "linked_note": [], "discussion_clipped": false, "display_omitted": 0, "display_shortened": false, "display_unavailable": false,
          "landed_seq": 0, "landed_tick": 0, "tree_path": "", "tree_rev": "",
          "tree_entries": [], "tree_born": false, "tree_truncated": false,
          "tree_phase": "loading", "file_path": "", "file_text": "",
          "file_binary": false, "file_truncated": false, "file_picture": false,
          "file_width": 0, "file_height": 0, "file_note": "", "file_header": "",
          "file_phase": "idle", "drafts_cleared": 0, "drafts_scope": "",
          "note_scope": "", "note_blocked": true
        }"#
            .to_vec(),
        )
    }

    fn chat_facts() -> Option<Vec<u8>> {
        chat_facts_with(&[first_light()], &[])
    }

    fn first_light() -> crate::backend::ChatMessage {
        crate::backend::ChatMessage {
            id: "m1".into(),
            view_key: 1,
            seq: 1,
            author: "mallard".into(),
            meta: "h 84,912".into(),
            body: "first light".into(),
            blocks: crate::backend::paragraph_blocks("first light"),
            show_author: true,
            initial: "M".into(),
            avatar_kind: "human".into(),
            height: 84_912,
            time: 84_912,
            rev: 1,
            ..Default::default()
        }
    }

    /// The chat facts with `messages` as the stream, encoded the way the
    /// host encodes them.
    fn chat_facts_with(
        messages: &[crate::backend::ChatMessage],
        thread: &[crate::backend::ChatMessage],
    ) -> Option<Vec<u8>> {
        let general = crate::backend::ChatChannel {
            id: "channel-a".into(),
            name: "general".into(),
            ..Default::default()
        };
        let ops = crate::backend::ChatChannel {
            id: "channel-b".into(),
            name: "ops".into(),
            ..Default::default()
        };
        let rooms = [
            crate::backend::ChatSidebarRow {
                channel: general,
                unread: false,
            },
            crate::backend::ChatSidebarRow {
                channel: ops,
                unread: true,
            },
        ];
        let props = ChatProps {
            dark: false,
            endpoint: "http://127.0.0.1:1",
            network_name: "testnet",
            network_chain_id: "testnet#abcd",
            status: "Live",
            block_height: 84_912,
            search_phase: "idle",
            search_query: "",
            search_hits: &[],
            rooms: &rooms,
            dm_rows: &[],
            channel_create_open: false,
            connected: true,
            loading: false,
            busy: false,
            active_channel: "channel-a",
            active_dm_peer: "",
            active_dm: &crate::backend::DmPeer::default(),
            active_channel_name: "general",
            active_channel_archived: false,
            active_channel_members_only: false,
            channel_members: &[],
            post_refusal: "",
            huddle_joined: false,
            huddle_channel: "",
            huddle_channel_name: "",
            huddle_joined_at: 0,
            huddle_now: 0,
            call_muted: false,
            messages,
            has_older_history: false,
            history_view: false,
            at_live_tail: true,
            history_loading: false,
            unread_boundary: 0,
            unread_marker_seq: 0,
            selected_message_seq: 0,
            selected_message_rev: 0,
            message_action: "toolbar",
            channel_settings_open: false,
            active_thread_seq: 0,
            thread_target_seq: 0,
            thread_messages: std::borrow::Cow::Borrowed(thread),
            thread_selected_seq: 0,
            thread_selected_rev: 0,
            thread_message_action: "toolbar",
            thread_has_more: false,
            thread_next_reply_seq: 0,
            thread_loading: false,
            copy_anchor_seq: 0,
            copy_head_seq: 0,
            copy_surface: "nowhere",
            sent_serial: 0,
        };
        Some(encode_chat_props(props))
    }

    /// Inspect the guest frame before the host sanitizer, not its already
    /// sanitized held tree. Resync prevents patch application hiding a loss.
    fn assert_display_projection_survives_wire(
        module: &'static str,
        props: &[u8],
        expected: &str,
        actions: &[&str],
    ) {
        let path = staged(module).expect("build the actual budget fixture with make views");
        let mut guest = Guest::load_from(module, &path).expect("actual guest loads");
        guest.redraw(&None);
        let supplied = Some(props.to_vec());
        guest.redraw(&supplied);
        for action in actions {
            guest.deliver(Output::Activate(button_message(&guest, action)));
            guest.redraw(&supplied);
        }
        guest.pending.push(wire::Event::Resync);
        let events = std::mem::take(&mut guest.pending);
        arm(&mut guest.store);
        let (bytes,) = guest
            .tick
            .call(&mut guest.store, (wire::encode(&events),))
            .expect("guest full frame");
        let frame: wire::Frame = wire::decode(&bytes).expect("wire frame");
        assert!(frame.root.is_some(), "resync emits a full tree");
        let mut observed = frame.root.clone().unwrap();
        let supplied: serde_json::Value = serde_json::from_slice(props).unwrap();
        let omitted = supplied["display_omitted"].as_i64().unwrap();
        let omitted_text = omitted.to_string();
        let mut omitted_seen = omitted == 0;
        let mut expected_seen = false;
        observed.for_each_mut(&mut |node| {
            if let wire::Node::Text { key, content, .. } = node {
                expected_seen |= content.starts_with(expected);
                omitted_seen |= key.ends_with("/display-omitted") && *content == omitted_text;
            }
        });
        assert!(
            expected_seen,
            "{module}: expected actual projected content {expected:?}"
        );
        assert!(
            omitted_seen,
            "{module}: omitted row count is not rendered as a number"
        );
        let mut sanitized = frame.clone();
        wire::sanitize(&mut sanitized);
        assert_eq!(
            frame.root, sanitized.root,
            "{module}: sanitizer changed the production projection"
        );
    }

    #[test]
    fn files_display_projection_bounds_preview_rows_and_preserves_the_edit_source() {
        let mut facts: serde_json::Value = serde_json::from_slice(&files_facts().unwrap()).unwrap();
        let source = "한글 preview ".repeat(5_000);
        facts["preview_text"] = source.clone().into();
        let entries: Vec<_> = (0..80).map(|n| serde_json::json!({
            "key": n + 100, "path": format!("/shared/{n}"), "name": format!("entry-{n}-{}", "한".repeat(300)),
            "kind": "dir", "size": 1, "object": format!("object-{n}")
        })).collect();
        facts["entries"] = entries.clone().into();
        facts["directories"] = entries.into();
        facts["history"] = (0..80).map(|n| serde_json::json!({"id": format!("s{n}"), "short_id": format!("s{n}"), "author": "duck", "height": n, "message": "history".repeat(300)})).collect::<Vec<_>>().into();
        facts["diff_from"] = "s0".into();
        facts["diff"] = (0..80)
            .map(|n| serde_json::json!({"path": format!("/shared/{n}"), "kind": "added"}))
            .collect::<Vec<_>>()
            .into();
        let bytes = display_budget::files(facts.clone());
        let projected: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(projected["preview_text"], source);
        assert_eq!(projected["preview_truncated"], false);
        assert_eq!(projected["preview_display_clipped"], true);
        assert!(
            projected["preview_display_text"]
                .as_str()
                .unwrap()
                .starts_with("한글 preview")
        );
        assert!(projected["display_omitted"].as_i64().unwrap() > 0);
        assert_display_projection_survives_wire(
            "files",
            &bytes,
            "Preview shortened for display.",
            &[],
        );
        facts["entries"] = Vec::<serde_json::Value>::new().into();
        facts["directories"] = Vec::<serde_json::Value>::new().into();
        facts["diff_from"] = "".into();
        let history = display_budget::files(facts.clone());
        assert_display_projection_survives_wire("files", &history, "historyhistory", &["History"]);
        facts["history"] = Vec::<serde_json::Value>::new().into();
        facts["diff_from"] = "s0".into();
        facts["diff"] = (0..80).map(|n| serde_json::json!({"path": format!("/shared/{n}-{}", "long".repeat(200)), "kind": "added"})).collect::<Vec<_>>().into();
        let diff = display_budget::files(facts);
        assert_display_projection_survives_wire("files", &diff, "/shared/0-long", &["History"]);
    }

    #[test]
    fn forge_display_projection_bounds_body_diff_and_newest_notes_together() {
        let mut facts: serde_json::Value = serde_json::from_slice(&forge_facts().unwrap()).unwrap();
        facts["open_repo"] = "core".into();
        facts["forge_item_number"] = 7.into();
        facts["item_phase"] = "ready".into();
        facts["forge_item_kind"] = "pr".into();
        facts["forge_item_state"] = "open".into();
        facts["forge_item_source_oid"] = "source-oid".into();
        facts["forge_item_body"] = "body".repeat(16_000).into();
        facts["forge_item_blocks"] =
            serde_json::to_value(crate::backend::paragraph_blocks(&format!(
                "**{}**\n\n{}",
                "rich body ".repeat(3_000),
                "plain body ".repeat(3_000)
            )))
            .unwrap();
        facts["discussion"] = (1..=40)
            .map(|n| {
                let body = format!("latest-{n} {}", "한글".repeat(400));
                serde_json::to_value(crate::backend::ChatMessage {
                    blocks: crate::backend::paragraph_blocks(&body),
                    body,
                    ..first_light_at(n)
                })
                .unwrap()
            })
            .collect::<Vec<_>>()
            .into();
        facts["diff_rows"] = serde_json::to_value(crate::backend::diff_lines(&format!(
            "--- a/main\n+++ b/main\n@@ -1 +1 @@\n-{}\n+{}\n",
            "old".repeat(10_000),
            "new".repeat(10_000)
        )))
        .unwrap();
        facts["staged_comments"] = (0..64).map(|n| serde_json::json!({"anchor": format!("a{n}{}", "x".repeat(12_000)), "path": "main", "line": "1", "side": "new", "body": "comment".repeat(2_000)})).collect::<Vec<_>>().into();
        facts["merge_conflicts"] = vec!["conflict-path".repeat(8_000)].into();
        let bytes = display_budget::forge(facts);
        let projected: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            projected["discussion"].as_array().unwrap().last().unwrap()["seq"],
            40
        );
        assert_eq!(projected["has_staged_comments"], true);
        assert_display_projection_survives_wire("forge", &bytes, "latest-40", &[]);
        assert!(projected["display_omitted"].as_i64().unwrap() > 0);
        assert!(projected["staged_comments"].as_array().unwrap().is_empty());
        assert!(projected["merge_conflicts"].as_array().unwrap().is_empty());
        let path = staged("forge").expect("actual forge wasm");
        let mut guest = Guest::load_from("forge", &path).unwrap();
        guest.redraw(&None);
        let props = Some(bytes);
        guest.redraw(&props);
        assert!(
            texts(&guest)
                .iter()
                .any(|text| text.starts_with("Merge conflicts —"))
        );
        guest.deliver(Output::Activate(button_message(&guest, "Submit review")));
        guest.redraw(&props);
        assert!(
            guest
                .intents
                .iter()
                .any(|event| event.kind == "review_submit")
        );
    }

    /// A busy room's whole hot window through the real wire: the newest
    /// message must still read, and the clipped older ones are offered as
    /// history.
    #[test]
    fn the_newest_message_of_a_busy_room_still_reads_through_the_wire() {
        let Some(staged) = staged("chat") else {
            return;
        };
        // 40 rows of 2 KB: past the wire's 64 KiB frame budget, and few
        // enough rows that the guest lays them out inside one tick
        const ROWS: i64 = 40;
        let messages: Vec<_> = (1..=ROWS)
            .map(|seq| {
                let body = format!("m{seq} {}", "x".repeat(2_000));
                crate::backend::ChatMessage {
                    blocks: crate::backend::paragraph_blocks(&body),
                    body,
                    ..first_light_at(seq)
                }
            })
            .collect();
        let props = chat_facts_with(&messages, &[]);
        let mut guest = Guest::load_from("chat", &staged).expect("the view loads");
        guest.redraw(&None);
        guest.redraw(&props);
        let shown = texts(&guest);
        let newest = format!("m{ROWS} ");
        assert!(
            shown.iter().any(|text| text.starts_with(&newest)),
            "the newest message is blank (fault {:?}): last texts {:?}",
            guest.fault,
            shown
                .iter()
                .rev()
                .take(6)
                .map(|text| &text[..text.len().min(24)])
                .collect::<Vec<_>>()
        );
        let props_text = String::from_utf8(props.unwrap()).unwrap();
        assert!(
            props_text.contains(r#""has_older_history":true"#),
            "the clip is history"
        );
        assert!(guest.fault.is_none());
    }

    /// The forge discussion has the chat stream's shape: 40 notes of 2 KB
    /// are past the frame's text budget, and the newest note — drawn last —
    /// must still read; the landing note above the list stays whole.
    #[test]
    fn the_newest_discussion_note_still_reads_through_the_wire() {
        let Some(staged) = staged("forge") else {
            return;
        };
        const ROWS: i64 = 40;
        let notes: Vec<_> = (1..=ROWS)
            .map(|seq| {
                let body = format!("n{seq} {}", "x".repeat(2_000));
                crate::backend::ChatMessage {
                    blocks: crate::backend::paragraph_blocks(&body),
                    body,
                    ..first_light_at(seq)
                }
            })
            .collect();
        let landing = crate::backend::ChatMessage {
            body: "the note a link landed on".into(),
            blocks: crate::backend::paragraph_blocks("the note a link landed on"),
            ..first_light_at(1_000)
        };
        let mut facts: serde_json::Value = serde_json::from_slice(&forge_facts().unwrap()).unwrap();
        facts["open_repo"] = "core".into();
        facts["forge_item_number"] = 7.into();
        facts["item_phase"] = "ready".into();
        facts["discussion"] = serde_json::to_value(&notes).unwrap();
        facts["linked_note"] = serde_json::to_value([&landing]).unwrap();
        let props = Some(display_budget::forge(facts));
        let mut guest = Guest::load_from("forge", &staged).expect("the view loads");
        guest.redraw(&None);
        guest.redraw(&props);
        let shown = texts(&guest);
        let newest = format!("n{ROWS} ");
        assert!(
            shown.iter().any(|text| text.starts_with(&newest)),
            "the newest note is blank (fault {:?}): last texts {:?}",
            guest.fault,
            shown
                .iter()
                .rev()
                .take(6)
                .map(|text| &text[..text.len().min(24)])
                .collect::<Vec<_>>()
        );
        assert!(shown.iter().any(|text| text == "the note a link landed on"));
        assert!(
            shown
                .iter()
                .any(|text| text == "Older comments are not shown.")
        );
        assert!(guest.fault.is_none());
    }

    /// A 60 KB file preview: the reader either sees all of it or is told
    /// it is cut — never a silently shortened text.
    #[test]
    fn a_long_file_preview_says_where_it_is_cut() {
        let Some(staged) = staged("files") else {
            return;
        };
        let text = format!("{}END-MARK", "y".repeat(60_000));
        let mut facts: serde_json::Value = serde_json::from_slice(&files_facts().unwrap()).unwrap();
        facts["preview_text"] = text.clone().into();
        let props = Some(display_budget::files(facts));
        let projected: serde_json::Value = serde_json::from_slice(props.as_ref().unwrap()).unwrap();
        assert_eq!(projected["preview_text"], text);
        assert_eq!(projected["preview_truncated"], false);
        assert_eq!(projected["preview_display_clipped"], true);
        let mut guest = Guest::load_from("files", &staged).expect("the view loads");
        guest.redraw(&None);
        guest.redraw(&props);
        let shown = texts(&guest);
        let whole = shown.iter().any(|text| text.ends_with("END-MARK"));
        let told = shown
            .iter()
            .any(|text| text == "Preview shortened for display.");
        assert!(
            whole || told,
            "the preview is cut without a word (fault {:?}): {} texts, longest {}",
            guest.fault,
            shown.len(),
            shown.iter().map(String::len).max().unwrap_or(0)
        );
        assert!(guest.fault.is_none());
        // Display clipping does not hide Edit or replace its authoritative seed.
        guest.deliver(Output::Activate(button_message(&guest, "Edit")));
        guest.redraw(&props);
        let mut root = guest.frame.root.clone().expect("editing tree");
        let mut editor_source = None;
        root.for_each_mut(&mut |node| {
            if let wire::Node::Editor { text, .. } = node {
                editor_source = Some(text.clone());
            }
        });
        assert_eq!(editor_source.as_deref(), Some(text.as_str()));
        // An oversized identity only hides rendering: it must not reseed or
        // consume this draft. Returning to the same facts restores the editor.
        let mut unavailable: serde_json::Value =
            serde_json::from_slice(props.as_ref().unwrap()).unwrap();
        unavailable["path"] = "x".repeat(20_000).into();
        guest.redraw(&Some(display_budget::files(unavailable)));
        assert!(
            texts(&guest)
                .iter()
                .any(|line| line.starts_with("Too much display data."))
        );
        guest.redraw(&props);
        guest.deliver(Output::Activate(button_message(&guest, "Save")));
        guest.redraw(&props);
        let saved = guest
            .intents
            .iter()
            .find(|event| event.kind == "save")
            .expect("save intent");
        let payload: serde_json::Value = serde_json::from_str(&saved.detail).unwrap();
        assert_eq!(payload["text"], text);
        assert_eq!(payload["path"], "/shared/README.md");
    }

    /// `head_within` never cuts inside a char; the discussion budget keeps
    /// the landing note whole and the newest notes.
    #[test]
    fn the_text_head_and_the_discussion_split_hold_their_budgets() {
        assert_eq!(head_within("abc", 3), ("abc", false));
        // "한" is 3 bytes: a 4-byte budget cuts before the second char
        assert_eq!(head_within("한글", 4), ("한", true));
        assert_eq!(head_within("한글", 6), ("한글", false));
        let row = |seq: i64, bytes: usize| crate::backend::ChatMessage {
            author: String::new(),
            meta: String::new(),
            body: "x".repeat(bytes),
            ..first_light_at(seq)
        };
        let landing = row(9, 100);
        let notes = [row(1, 100), row(2, 100), row(3, 100)];
        let (kept, clipped) = newest_within(&notes, 300usize.saturating_sub(text_bytes(&landing)));
        assert_eq!(kept.iter().map(|m| m.seq).collect::<Vec<_>>(), [2, 3]);
        assert!(clipped);
    }

    fn first_light_at(seq: i64) -> crate::backend::ChatMessage {
        crate::backend::ChatMessage {
            id: format!("m{seq}"),
            view_key: seq,
            seq,
            ..first_light()
        }
    }

    /// The budget keeps the newest, drops the oldest, says so; the thread
    /// keeps its root.
    #[test]
    fn the_timeline_budget_drops_the_oldest_first_and_keeps_the_thread_root() {
        let row = |seq: i64, bytes: usize| crate::backend::ChatMessage {
            author: String::new(),
            meta: String::new(),
            body: "x".repeat(bytes),
            ..first_light_at(seq)
        };
        let stream = [row(1, 100), row(2, 100), row(3, 100)];
        assert_eq!(newest_within(&stream, 250).0.len(), 2);
        assert_eq!(newest_within(&stream, 250).0[0].seq, 2);
        assert!(newest_within(&stream, 250).1);
        assert_eq!(newest_within(&stream, 300), (&stream[..], false));

        let big = TIMELINE_TEXT_BUDGET / 2;
        let root = crate::backend::ChatMessage {
            thread_seq: 0,
            ..row(10, 10)
        };
        let reply = |seq| crate::backend::ChatMessage {
            thread_seq: 10,
            ..row(seq, big)
        };
        let thread = [root, reply(11), reply(12), reply(13)];
        let facts: serde_json::Value =
            serde_json::from_slice(&chat_facts_with(&[], &thread).unwrap()).unwrap();
        assert_eq!(facts["thread_has_more"], true);
        let kept: Vec<i64> = facts["thread_messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|message| message["seq"].as_i64().unwrap())
            .collect();
        assert_eq!(kept, [10, 13], "the root, then the newest reply that fits");
    }

    /// The facts a module's host pushes, and one word of them the tree
    /// shows — what a swap must carry from A into B's first tree.
    fn facts(module: &str) -> (Option<Vec<u8>>, &'static str) {
        match module {
            "governance" => (register(), "prop-1"),
            "files" => (files_facts(), "README.md"),
            "pages" => (pages_facts(), "Alpha"),
            "chat" => (chat_facts(), "first light"),
            _ => (forge_facts(), "core"),
        }
    }

    /// A new deployment of a module whose view is drawn replaces the
    /// instance in place: restored from the drawn view's snapshot, under a
    /// new generation (so the old tree's messages route nowhere), with the
    /// new deployment's assets, and its first redraw routes the restored
    /// view's requests — the props subscription among them — without
    /// another tick. Every module-owned view, from its own staged wasm:
    /// a view whose props ride a mount task instead of a subscription can
    /// never be snapshotted while that task is live, and never asks again.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_new_deployment_swaps_the_view_in_place() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        for module in ["governance", "files", "pages", "chat", "forge"] {
            let Some(staged) = staged(module) else {
                continue;
            };
            let (props, shown) = facts(module);
            let component = std::fs::read(staged).expect("the staged view");
            let (a, b) = (
                deployment(&component, "a.svg"),
                deployment(&component, "b.svg"),
            );
            let node = FakeDeployment::serving(module, &a);
            let client = fake_node(node.clone()).await;

            let mounted = fresh(module);
            join_all(connected(&client));
            assert_eq!(slot_assets(&mounted), ["a.svg"], "{module}");
            let (generation, frame_rev) = {
                let mut locked = mounted.lock().expect("module view lock");
                let generation = locked.generation;
                let Slot::Ready(guest) = &mut locked.slot else {
                    panic!("{module}: the view of A");
                };
                // the view draws, takes facts that are not its initial
                // state, and settles
                assert!((0..4).any(|_| !guest.redraw(&props)), "{module}");
                assert!(guest.settled(), "{module} fault: {:?}", guest.fault);
                assert!(guest.props_subscription.is_some(), "{module}");
                assert!(
                    texts(guest).iter().any(|text| text == shown),
                    "{module}: {shown:?} not shown in {:?}",
                    texts(guest)
                );
                (generation, guest.frame_rev)
            };

            // a block activates B: the check finds the hash moved
            node.deploy(module, &b);
            join_all(deployments_checked().await);
            assert_eq!(slot_assets(&mounted), ["b.svg"], "{module}: B installed");
            let generation = {
                let mut locked = mounted.lock().expect("module view lock");
                assert_eq!(locked.hash, Some(b.hash()), "{module}");
                assert!(
                    locked.generation > generation,
                    "{module}: the old tree's messages are refused"
                );
                let Slot::Ready(guest) = &mut locked.slot else {
                    panic!("{module}: the view of B");
                };
                assert!(
                    guest.frame_rev > frame_rev,
                    "{module}: the widget rebuilds for the new tree"
                );
                assert!(
                    guest.staged && guest.props_subscription.is_none(),
                    "{module}"
                );
                // B's first tree is A's state, before any facts reach it
                assert!(
                    texts(guest).iter().any(|text| text == shown),
                    "{module}: the facts did not carry over: {:?}",
                    texts(guest)
                );
                let ticks = guest.ticks;
                // the first redraw routes the staged requests without another
                // tick, and the view is quiet after it
                assert!(!guest.redraw(&None), "{module}");
                assert_eq!(guest.ticks, ticks, "{module}");
                assert!(
                    guest.props_subscription.is_some(),
                    "{module}: the restored view asked for its props again"
                );
                assert!(guest.fault.is_none(), "{module}: {:?}", guest.fault);
                locked.generation
            };
            // and the same deployment again is nothing to do
            join_all(deployments_checked().await);
            {
                let locked = mounted.lock().expect("module view lock");
                assert_eq!(
                    (locked.generation, locked.hash),
                    (generation, Some(b.hash())),
                    "{module}"
                );
            }
            // the seat is given back: the next module's node, and the
            // other deployment tests, know nothing of this one
            registry().lock().expect("module views").remove(module);
        }
    }

    /// A deployment that moves while its view is prepared is not installed:
    /// the active code is read again right before the seat, and the view
    /// drawn keeps its place and its hash.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_deployment_that_moves_while_its_view_is_prepared_is_not_installed() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let Some(staged) = staged("governance") else {
            return;
        };
        let component = std::fs::read(staged).expect("the staged view");
        let (a, b, c) = (
            deployment(&component, "a.svg"),
            deployment(&component, "b.svg"),
            deployment(&component, "c.svg"),
        );
        let node = FakeDeployment::serving("forge", &a);
        let client = fake_node(node.clone()).await;
        let mounted = fresh("forge");
        join_all(connected(&client));
        assert_eq!(slot_assets(&mounted), ["a.svg"]);

        // B activates, but its bytes are slow — and C activates meanwhile
        node.deploy("forge", &b);
        let hold = hold_blob(&node);
        let loads = deployments_checked().await;
        node.held.notified().await;
        node.deploy("forge", &c);
        hold.notify_one();
        join_all(loads);
        assert_eq!(slot_assets(&mounted), ["a.svg"], "B is not installed");
        assert_eq!(mounted.lock().unwrap().hash, Some(a.hash()));
        // the next block's check brings C
        join_all(deployments_checked().await);
        assert_eq!(slot_assets(&mounted), ["c.svg"]);
    }

    /// A deployment that removed its view leaves an explicit empty slot —
    /// never the old view.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_removed_view_leaves_an_empty_slot() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let Some(staged) = staged("governance") else {
            return;
        };
        let component = std::fs::read(staged).expect("the staged view");
        let a = deployment(&component, "a.svg");
        let removed = module_artifact::ModuleArtifact::component(vec![9, 9, 9]);
        let node = FakeDeployment::serving("forge", &a);
        let client = fake_node(node.clone()).await;
        let mounted = fresh("forge");
        join_all(connected(&client));
        assert_eq!(slot_assets(&mounted), ["a.svg"]);
        node.deploy("forge", &removed);
        join_all(deployments_checked().await);
        let locked = mounted.lock().unwrap();
        assert!(
            matches!(locked.slot, Slot::Empty),
            "{}",
            slot_name(&locked.slot)
        );
        assert_eq!(locked.hash, Some(removed.hash()));
    }

    /// Blocks keep landing while a deployment's bytes are slow: the load
    /// after it is left to land, not started over as stale every time.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_slow_deployment_is_not_restarted_by_every_block() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let Some(staged) = staged("governance") else {
            return;
        };
        let component = std::fs::read(staged).expect("the staged view");
        let (a, b) = (
            deployment(&component, "a.svg"),
            deployment(&component, "b.svg"),
        );
        let node = FakeDeployment::serving("forge", &a);
        let client = fake_node(node.clone()).await;
        let mounted = fresh("forge");
        join_all(connected(&client));
        assert_eq!(slot_assets(&mounted), ["a.svg"]);

        let before = mounted.lock().unwrap().generation;
        node.deploy("forge", &b);
        let hold = hold_blob(&node);
        let mut loads = deployments_checked().await;
        node.held.notified().await;
        let generation = mounted.lock().unwrap().generation;
        assert_eq!(generation, before + 1);
        // three more blocks while B's bytes are held: the status still
        // answers, and none of them starts this view over (the views of
        // earlier tests, unknown to this node, get their own loads)
        for _ in 0..3 {
            loads.extend(deployments_checked().await);
            let locked = mounted.lock().unwrap();
            assert_eq!((locked.generation, locked.in_flight), (generation, true));
        }
        hold.notify_one();
        join_all(loads);
        assert_eq!(slot_assets(&mounted), ["b.svg"]);
        let locked = mounted.lock().unwrap();
        assert_eq!((locked.generation, locked.in_flight), (generation, false));
    }

    /// A removal answered late — the code moved on to C while the
    /// verified "no view" for B was on its way — does not empty the slot.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_removal_that_moves_while_it_is_verified_is_not_installed() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let Some(staged) = staged("governance") else {
            return;
        };
        let component = std::fs::read(staged).expect("the staged view");
        let (a, c) = (
            deployment(&component, "a.svg"),
            deployment(&component, "c.svg"),
        );
        let removed = module_artifact::ModuleArtifact::component(vec![9, 9, 9]);
        let node = FakeDeployment::serving("governance", &a);
        let client = fake_node(node.clone()).await;
        let mounted = fresh("governance");
        join_all(connected(&client));
        assert_eq!(slot_assets(&mounted), ["a.svg"]);

        node.deploy("governance", &removed);
        let hold = hold_blob(&node);
        let loads = deployments_checked().await;
        node.held.notified().await;
        node.deploy("governance", &c);
        hold.notify_one();
        join_all(loads);
        assert_eq!(slot_assets(&mounted), ["a.svg"], "A is still drawn");
        assert_eq!(mounted.lock().unwrap().hash, Some(a.hash()));
        join_all(deployments_checked().await);
        assert_eq!(slot_assets(&mounted), ["c.svg"]);
    }

    /// Every module-owned load leaves one `view_load` line naming its path
    /// and the time of each of its stages — the node's two answers, the
    /// compile, the instance, the tree a replacement proves, the second
    /// look at the registry — so where a "Loading" wait goes is read off
    /// the log, not guessed.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn every_load_reports_the_time_of_each_of_its_stages() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let staged = staged("governance").expect("build the governance guest before this test");
        let component = std::fs::read(staged).expect("the staged view");
        let (a, b) = (
            deployment(&component, "a.svg"),
            deployment(&component, "b.svg"),
        );
        let node = FakeDeployment::serving("governance", &a);
        let client = fake_node(node.clone()).await;
        let lines = super::canary::tap();
        let _mounted = fresh("governance");
        join_all(connected(&client));
        node.deploy("governance", &b);
        join_all(deployments_checked().await);

        let loads: Vec<String> = lines
            .try_iter()
            .filter(|line| line.starts_with("view_load module=governance "))
            .collect();
        let field = |line: &str, name: &str| -> String {
            line.split_whitespace()
                .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
                .unwrap_or_else(|| panic!("no {name} in {line}"))
                .to_owned()
        };
        let of = |hash: [u8; 32]| -> String {
            let hash = crate::backend::hex_encode(&hash);
            loads
                .iter()
                .find(|line| field(line, "hash") == hash)
                .cloned()
                .unwrap_or_else(|| panic!("no line for {hash} in {loads:?}"))
        };
        let (first, swap) = (of(a.hash()), of(b.hash()));
        assert_eq!(field(&first, "path"), "first");
        assert_eq!(field(&first, "outcome"), "Ready");
        assert_eq!(
            field(&first, "first_frame_ms"),
            "-",
            "a fresh view draws its first tree on the window thread"
        );
        assert_eq!(field(&swap, "path"), "swap");
        assert_eq!(field(&swap, "outcome"), "Swap");
        field(&swap, "first_frame_ms")
            .parse::<u128>()
            .expect("the replacement's first frame is timed");
        for line in [&first, &swap] {
            let ms = |name: &str| {
                field(line, name)
                    .parse::<u128>()
                    .unwrap_or_else(|_| panic!("{name} in {line}"))
            };
            let total = ms("total_ms");
            let stages = ms("status_ms")
                + ms("fetch_ms")
                + ms("compile_ms")
                + ms("init_ms")
                + ms("check_ms");
            assert!(total >= stages, "{line}");
        }
    }

    /// A view mounted but not yet ticked names its instance too: a tick
    /// that reaches it while a fresh candidate is prepared refuses the
    /// candidate, and the next block's replacement carries the tick's
    /// state over.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_view_ticked_while_its_fresh_candidate_is_prepared_keeps_its_state() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let Some(staged) = staged("governance") else {
            return;
        };
        let component = std::fs::read(staged).expect("the staged view");
        let (a, b) = (
            deployment(&component, "a.svg"),
            deployment(&component, "b.svg"),
        );
        let node = FakeDeployment::serving("governance", &a);
        let client = fake_node(node.clone()).await;
        let mounted = fresh("governance");
        join_all(connected(&client));
        assert_eq!(slot_assets(&mounted), ["a.svg"]);

        node.deploy("governance", &b);
        // the candidate reads the instance after its bytes arrive, and is
        // seated after the node confirms the code did not move: hold the
        // bytes to get past the first, the confirmation to sit between
        let blob = hold_blob(&node);
        let loads = deployments_checked().await;
        node.held.notified().await;
        let status = hold_status(&node);
        blob.notify_one();
        node.held.notified().await;
        {
            let mut locked = mounted.lock().unwrap();
            let Slot::Ready(guest) = &mut locked.slot else {
                panic!("the view of A");
            };
            assert_eq!(guest.ticks, 0);
            assert!((0..4).any(|_| !guest.redraw(&None)));
        }
        status.notify_one();
        join_all(loads);
        let ticks = {
            let locked = mounted.lock().unwrap();
            assert_eq!(locked.hash, Some(a.hash()), "the candidate was refused");
            let Slot::Ready(guest) = &locked.slot else {
                panic!("the view of A");
            };
            guest.ticks
        };
        assert!(ticks > 0);
        join_all(deployments_checked().await);
        let locked = mounted.lock().unwrap();
        assert_eq!(locked.hash, Some(b.hash()));
        let Slot::Ready(guest) = &locked.slot else {
            panic!("the view of B");
        };
        assert!(guest.staged, "restored from A, not booted fresh");
    }

    /// A candidate for a view never ticked proves its first tree too: one
    /// whose first frame traps is not seated, and the view stays.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_fresh_candidate_whose_first_frame_fails_is_not_installed() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let Some(staged) = staged("governance") else {
            return;
        };
        let component = std::fs::read(staged).expect("the staged view");
        let (a, b) = (
            deployment(&component, "a.svg"),
            deployment(&component, "b.svg"),
        );
        let node = FakeDeployment::serving("forge", &a);
        let client = fake_node(node.clone()).await;
        let mounted = fresh("forge");
        join_all(connected(&client));
        assert_eq!(slot_assets(&mounted), ["a.svg"]);
        {
            let locked = mounted.lock().unwrap();
            let Slot::Ready(guest) = &locked.slot else {
                panic!("A is seated");
            };
            assert_eq!(guest.ticks, 0);
        }

        node.deploy("forge", &b);
        let hold = hold_blob(&node);
        let loads = deployments_checked().await;
        node.held.notified().await;
        FIRST_FRAME_TRAPS.store(true, std::sync::atomic::Ordering::SeqCst);
        hold.notify_one();
        join_all(loads);
        assert!(!FIRST_FRAME_TRAPS.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(slot_assets(&mounted), ["a.svg"], "A stays");
        {
            let locked = mounted.lock().unwrap();
            assert_eq!((locked.hash, locked.in_flight), (Some(a.hash()), false));
        }
        // the next block's candidate, drawing its first tree, is seated
        join_all(deployments_checked().await);
        assert_eq!(slot_assets(&mounted), ["b.svg"]);
    }

    /// An artifact asset is found by its canonical relative path, exactly.
    #[test]
    fn an_artifact_asset_is_an_exact_path_lookup() {
        let assets: crate::backend::view_source::Assets =
            [("icons/seal.svg".to_owned(), b"<svg/>".to_vec())].into();
        assert_eq!(
            artifact_asset(&assets, "icons/seal.svg"),
            Some(&b"<svg/>"[..])
        );
        for missing in [
            "seal.svg",
            "/icons/seal.svg",
            "icons/../icons/seal.svg",
            "icons/SEAL.svg",
        ] {
            assert_eq!(artifact_asset(&assets, missing), None, "{missing}");
        }
    }

    /// A staged view carries a manifest the host reads; a component without
    /// one is no view. The manifest's preferred size is for placing a new
    /// window, which the embedded tab never does: it keeps the tab's limits.
    #[test]
    fn a_view_carries_a_readable_manifest() {
        if let Some(staged) = staged("governance") {
            let bytes = std::fs::read(staged).expect("the staged view");
            let manifest = ui_lang_wire::manifest::read_manifest(&bytes).expect("a manifest");
            assert_eq!(manifest.name, "Approvals");
            assert!(
                ui_lang_wire::manifest::read_manifest(b"\0asm\x01\0\0\0").is_none(),
                "a bare core module has no manifest"
            );
        }
    }

    /// With an item open the Forge view leaves the note composer to the
    /// host: a note typed in the host's composer and sent leaves as the
    /// `composer` intent with its body, and the typing itself never
    /// reaches the guest.
    #[test]
    fn the_staged_forge_view_hears_a_note_typed_in_the_hosts_composer() {
        use crate::composer_surface::testing::{Interaction, interact};
        use crate::editor::{ComposerEvent, RichAction};
        let Some(staged) = staged("forge") else {
            return;
        };
        let mut guest = Guest::load_from("forge", &staged).expect("the view loads");
        guest.redraw(&None);
        let props = Some(
            br#"{
              "dark": false, "connected": true, "org": "duckhouse", "about": "",
              "tier": "validator", "network_chain_id": "mynet#d0cdf950",
              "connected_rpc": "http://127.0.0.1:1",
              "repos": [{"name": "core", "head": "main"}],
              "list_phase": "ready", "open_repo": "core", "repo_menu": false,
              "repo_phase": "ready", "branches": ["main"], "tab": "issues", "items": [],
              "forge_item_number": 7, "item_phase": "ready", "forge_item_kind": "issue",
              "forge_item_title": "Bound every list", "forge_item_state": "open",
              "forge_item_author": "duck", "forge_item_branches": "", "forge_item_body": "",
              "forge_item_blocks": [], "forge_item_files_changed": 0, "forge_item_additions": 0,
              "forge_item_deletions": 0, "diff_rows": [], "forge_item_diff_truncated": false,
              "forge_item_merge_oid": "", "forge_item_source_oid": "",
              "forge_item_approvals": 0, "forge_item_change_requests": 0,
              "forge_item_reviews": [], "merge_conflicts": [], "has_merge_conflicts": false, "merge_busy": false,
              "review_verdict": "comment", "review_busy": false, "staged_comments": [], "has_staged_comments": false,
              "comment_cap_reached": false, "discussion": [], "linked_note": [], "discussion_clipped": false, "display_omitted": 0, "display_shortened": false, "display_unavailable": false,
              "landed_seq": 0, "landed_tick": 0, "tree_path": "", "tree_rev": "",
              "tree_entries": [], "tree_born": false, "tree_truncated": false,
              "tree_phase": "loading", "file_path": "", "file_text": "",
              "file_binary": false, "file_truncated": false, "file_picture": false,
              "file_width": 0, "file_height": 0, "file_note": "", "file_header": "",
              "file_phase": "idle", "drafts_cleared": 0, "drafts_scope": "",
              "note_scope": "forge:core:7", "note_blocked": false
            }"#
            .to_vec(),
        );
        guest.redraw(&props);
        assert!(
            surface_names(&guest).contains(&"forge_composer".to_owned()),
            "the open item leaves the note composer slot: {:?}",
            surface_names(&guest)
        );

        // typed in the host's composer, on the item's own document
        let scope = "forge:core:7";
        for glyph in ['h', 'i'] {
            let typed = interact(
                scope,
                "note",
                false,
                false,
                Interaction::Editor(ComposerEvent::Apply(RichAction::Edit(
                    iced::widget::text_editor::Action::Edit(
                        iced::widget::text_editor::Edit::Insert(glyph),
                    ),
                ))),
            );
            assert!(typed.is_none(), "an edit stays in the host");
        }
        let sent = interact(
            scope,
            "note",
            false,
            false,
            Interaction::Editor(ComposerEvent::Submit),
        )
        .expect("a send publishes");
        guest.deliver(Output::Surface {
            handler: None,
            value: sent,
        });
        assert!(guest.pending.is_empty(), "the words never reach the guest");
        assert_eq!(guest.intents.len(), 1);
        assert_eq!(guest.intents[0].kind, "composer");
        assert!(guest.intents[0].detail.contains(r#""body":"hi""#));
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
