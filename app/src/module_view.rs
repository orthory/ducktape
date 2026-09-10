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

pub(crate) mod pages_document;

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
/// A never-valid view may hold its replacement off for this long. Once a
/// guest publishes a valid tree, neither pending work nor a later trap
/// authorizes discarding its state.
const REPLACEMENT_WAIT: Duration = Duration::from_secs(30);
/// The gap before the first re-attempt at a candidate whose load failed,
/// and the longest that gap widens to. A load fetches the artifact,
/// instantiates it, restores the snapshot and verifies the first tree —
/// and compiles the component too, whenever `compiled_view` does not
/// already hold it. A deployment that cannot load fails that way every
/// time, so trying it once a block buys nothing and never stops. Widening
/// the gap rather than giving up keeps a fetch that failed on the
/// transport recoverable: nothing is suppressed for good, only spaced out.
const RETRY_FIRST: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(60);
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
/// number) so the view offers the editor to a record's controller; beside
/// it the run tracker, every run off the runs journal, the journal of the
/// one the reader opened (`open_run`, its dispatch id) with the chips of
/// every place it touched, and that run's live progress while it works.
/// Its intents come back as `status` (`agent_id`, `paused`), `save` and
/// `register` (both the whole draft record as JSON, `AgentDraft`),
/// `open_run` (`dispatch_id`, "" to close) and `open_link` (`url`, a chip's
/// duck:// address for the open plane).
/// Every committed write bumps `committed`, which tells the view its drafts
/// were consumed.
#[allow(clippy::too_many_arguments)]
pub fn agents_view(
    dark: bool,
    connected: bool,
    answered: bool,
    account: &str,
    committed: i64,
    rows: &[crate::backend::AgentRow],
    runs: &[crate::backend::RunRow],
    open_run: &str,
    opened: i64,
    journal: &crate::backend::RunJournal,
    live: &crate::backend::LiveRun,
    capabilities: &[String],
    actions: &[String],
) -> Element<'static, ModuleViewEvent> {
    module_view(
        "agents",
        agents_props(
            dark,
            connected,
            answered,
            account,
            committed,
            rows,
            runs,
            open_run,
            opened,
            journal,
            live,
            capabilities,
            actions,
        ),
    )
}

/// The exact bytes [`agents_view`] pushes — named so a test can assert what
/// this app SENDS rather than a shape it wrote out by hand beside it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn agents_props(
    dark: bool,
    connected: bool,
    answered: bool,
    account: &str,
    committed: i64,
    rows: &[crate::backend::AgentRow],
    runs: &[crate::backend::RunRow],
    open_run: &str,
    opened: i64,
    journal: &crate::backend::RunJournal,
    live: &crate::backend::LiveRun,
    capabilities: &[String],
    actions: &[String],
) -> Vec<u8> {
    // THE APP'S OWN BOOKKEEPING STAYS IN THE APP. `rpc` is an endpoint the
    // guest draws nothing with, and `link`/`account`/`op` are the fence the app
    // installs an answer by — a guest cannot check them and has no reason to
    // see which operation number it is looking at. The reading's error is the
    // app's banner, not a field the guest re-renders.
    const APP_ONLY: [&str; 4] = ["rpc", "link", "account", "op"];
    let mut book = serde_json::to_value(journal).expect("the run journal encodes");
    if let Some(book) = book.as_object_mut() {
        for app_only in APP_ONLY.iter().chain(["error"].iter()) {
            book.remove(*app_only);
        }
    }
    let props = serde_json::json!({
        "rows": rows,
        "runs": runs,
        "open_run": open_run,
        "opened": opened,
        "journal": book,
        "live": live,
        "capabilities": capabilities,
        "actions": actions,
        "account": account,
        "committed": committed,
        "connected": connected,
        "answered": answered,
        "dark": dark,
    });
    serde_json::to_vec(&props).expect("props encode")
}

pub fn agents_intent(event: &ModuleViewEvent) -> crate::AgentsIntent {
    match event.kind.as_str() {
        "save" => crate::AgentsIntent::Save,
        "register" => crate::AgentsIntent::Register,
        "open_run" => crate::AgentsIntent::OpenRun,
        "open_link" => crate::AgentsIntent::OpenLink,
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

/// Test seam: Ice reads extern structs but cannot construct one, and a scenario
/// that presses a view's control has no view to press it in. The `kind` is the
/// same string the guest emits, so a scenario names the act and not an enum the
/// intent mapping could drift from.
pub fn view_event(kind: String, detail: String) -> ModuleViewEvent {
    ModuleViewEvent { kind, detail }
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
    repo_phase: &'static str,
    branches: &'a [crate::backend::ForgeBranch],
    tree_branch: &'a str,
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
    repo_phase: crate::ForgePhase,
    branches: &[crate::backend::ForgeBranch],
    tree_branch: &str,
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
        repo_phase: forge_phase_word(repo_phase),
        branches,
        tree_branch,
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
        "branch" => Intent::Branch,
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

/// The seat an open item belongs to, by its kind: a pull request lights the
/// Pull requests tab, an issue the Issues tab. A kind the tracker has no seat
/// for leaves the code browse lit.
pub fn forge_kind_tab(kind: &str) -> crate::ForgeTab {
    match kind {
        "pr" => crate::ForgeTab::Pulls,
        "issue" => crate::ForgeTab::Issues,
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

// ---------- the pages seat ----------

/// Pages supplies metadata and a stable source identity. Document bytes use
/// bounded chunks, and accepted guest edits feed the existing app save buffer.
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
    page_text: &str,
    buffer_page: &str,
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
    let source = if loading || active_page.is_empty() || buffer_page != active_page {
        Ok(Vec::new())
    } else {
        pages_document::source(
            connection().lock().expect("views rpc").rev,
            network_chain_id,
            active_page,
            page_text,
        )
    };
    let (source, source_error) = match source {
        Ok(source) => (source, String::new()),
        Err(error) => (Vec::new(), error.to_owned()),
    };
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
        "document_source": source,
        "document_error": source_error,
        "comment_marks": crate::pages::comment_marks(blocks, commented_block_hits).into_iter().map(|(line, count)| crate::pages::guest_document::CommentMark { line: line as i64, count: count as i64 }).collect::<Vec<_>>(),
        "commented_lines": crate::pages::commented_lines(blocks, commented_block_hits),
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
    /// THIS ROOM'S runs only, as hints: the reading is taken for the whole
    /// node, and [`encode_chat_props`] cuts it to `active_channel` on the way
    /// out.
    live_agents: Vec<crate::backend::LiveRunHint>,
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
    live_agents: &[crate::backend::LiveAgentRow],
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
        live_agents: Vec::new(),
    };
    module_view("chat", encode_chat_props(props, live_agents))
}

/// Text bytes a guest's big list or blob may put on one frame: the wire
/// spends 64 KiB of text per frame and EMPTIES whatever comes after, and the
/// newest messages come last — a busy room's hot window (256 rows) drew its
/// newest messages blank. The rest of the frame (rooms, names, times, the
/// rail) lives in the headroom.
const TIMELINE_TEXT_BUDGET: usize = 48 << 10;

/// Bytes the live agent cards may take out of [`TIMELINE_TEXT_BUDGET`]. They
/// draw INSIDE the stream, under their anchors, so they spend the timeline's
/// budget rather than the chrome's headroom — and this ceiling is what keeps a
/// room with a great many runs in flight from blanking the messages they sit
/// under.
const LIVE_AGENT_TEXT_BUDGET: usize = 6 << 10;

/// The facts encoded for the view, the timelines held to
/// [`TIMELINE_TEXT_BUDGET`]: the newest messages that fit, oldest dropped
/// first, and a clipped stream says so through `has_older_history` (the
/// thread through `thread_has_more`, its root always kept) so the view still
/// offers what was left behind as history.
///
/// The live agent rows are narrowed to `active_channel` here, and THIS IS THE
/// ONLY PLACE a run is matched to a room: the reading covers the whole node, so
/// a row from a room the reader left cannot reach the screen no matter which of
/// the eight handlers that move `active_channel` she got here through.
fn encode_chat_props(
    mut props: ChatProps<'_>,
    live_rows: &[crate::backend::LiveAgentRow],
) -> Vec<u8> {
    let live = live_agents_within(live_rows, props.active_channel, LIVE_AGENT_TEXT_BUDGET);
    // THE LIVE HINTS ARE SERVED FIRST. A run in flight is the most perishable
    // thing on the frame and the one the reader is waiting on, so it takes its
    // bytes before the scrollback it sits in does.
    let timelines =
        TIMELINE_TEXT_BUDGET.saturating_sub(live.iter().map(live_text_bytes).sum::<usize>());
    props.live_agents = live;
    let (stream, stream_clipped) = newest_within(props.messages, timelines);
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
        timelines
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

/// The bytes a live run hint puts on the wire as text.
fn live_text_bytes(hint: &crate::backend::LiveRunHint) -> usize {
    hint.agent.len() + hint.status.len()
}

/// The runs anchored in `channel_id` whose hints fit `budget`, newest anchor
/// kept first — those are the ones at the tail the reader is looking at — and
/// handed back in anchor order so the list does not churn the guest's timeline
/// memo when two runs start in the same poll.
fn live_agents_within(
    rows: &[crate::backend::LiveAgentRow],
    channel_id: &str,
    budget: usize,
) -> Vec<crate::backend::LiveRunHint> {
    let mut here: Vec<crate::backend::LiveRunHint> = rows
        .iter()
        .filter(|row| row.channel_id == channel_id)
        .map(crate::backend::LiveRunHint::from)
        .collect();
    here.sort_by_key(|hint| std::cmp::Reverse(hint.anchor_seq));
    let mut spent = 0;
    let mut kept = Vec::new();
    for hint in here {
        let cost = live_text_bytes(&hint);
        if spent + cost > budget {
            break;
        }
        spent += cost;
        kept.push(hint);
    }
    kept.sort_by_key(|hint| hint.anchor_seq);
    kept
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
        "cancel_run" => Intent::CancelRun,
        "open_run" => Intent::OpenRun,
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
        "edit" => crate::ComposerKind::Edit,
        "thread_edit" => crate::ComposerKind::ThreadEdit,
        _ => crate::ComposerKind::Message,
    }
}

/// A refused or failed body, handed back to the composer it was written in.
pub fn chat_composer_unsent(scope: &str, text: &str, committed: bool) -> bool {
    crate::composer_surface::unsent(scope, text, committed);
    true
}

/// Seed the native edit composer from canonical blocks, never copy text.
pub fn chat_composer_edit(
    scope: &str,
    messages: &[crate::backend::ChatMessage],
    seq: i64,
    rev: i64,
) -> bool {
    let Some(message) = messages.iter().find(|message| message.seq == seq) else {
        return false;
    };
    let editable = !message.deleted && !message.pending && message.rev == rev;
    if !editable {
        return false;
    }
    crate::composer_surface::seed(scope, &message.edit_body);
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
        "agents" => &[
            "status",
            "save",
            "register",
            "open_run",
            "open_link",
        ],
        "node" => &["copy", "tab", "log_filter"],
        "explorer" => &["refresh", "copy", "search", "clear"],
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
            "cancel_run",
            "open_run",
        ],
        "forge" => &[
            "open_repo",
            "close_repo",
            "branch",
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
/// Drawing never starts a load, and never finds one on its way: the view
/// was asked for when its source was — the staged file at boot
/// ([`booted`]), the node's deployment at connect ([`connected`]) — and
/// both hand every view over before anything draws. A block that moves the
/// deployment ([`deployments_checked`]) reloads the view in place: the tab
/// keeps the one it has until the replacement is ready.
fn module_view(module: &'static str, props: Vec<u8>) -> Element<'static, ModuleViewEvent> {
    mounted(module).lock().expect("module view lock").props = Some(props);
    drawn(module)
}

/// The mounted view as it stands — its current frame, or the notice for a
/// slot without one — with no props pushed.
pub(crate) fn drawn(module: &'static str) -> Element<'static, ModuleViewEvent> {
    let mounted = mounted(module);
    let (content, rev, generation, alive) = {
        let mut locked = mounted.lock().expect("module view lock");
        let generation = locked.generation;
        match &mut locked.slot {
            // a seat before its source event has answered: the boot and the
            // connect hand every view over before a tab can draw, so this
            // is a module's tab drawn before any node was ever asked
            Slot::Loading => return notice("Loading the view…"),
            Slot::Empty => {
                return notice(&format!(
                    "This network has no {module} view yet. An admin activates a {module} deployment that ships one."
                ));
            }
            Slot::Failed(reason) => return notice(reason),
            Slot::Ready(guest) => (
                guest.render(),
                guest.frame_rev,
                generation,
                guest.alive.clone(),
            ),
        }
    };
    Element::new(ModuleView {
        mounted,
        generation,
        rev,
        alive,
        content,
    })
}

/// The node the app is connected to, for the views that come from its
/// deployments. Told by `backend::connect`; every module-owned view is
/// asked of this node, drawn or not, under a new generation, so an answer
/// the previous node is still composing lands nowhere. Every seat keeps
/// what it shows until this node's answer lands: one seated is swapped in
/// place, one failed or empty off the previous node shows that until then,
/// and one no tab has drawn yet is seated here. The connect answers only
/// once the loads returned here have [settled](Loads::settled), so the
/// tabs it opens onto find their views there.
pub fn connected(client: &ducktape_rpc::Client) -> Loads {
    // THE REGISTRY LOCK FIRST: a connection change and the restart of the
    // views under it are one step. Two callers — `backend::connect` runs on
    // the executor's threads, and two connects can overlap — otherwise
    // interleave into loads asked of one node under the other's revision,
    // every one of which dies at install, and the views stay "Loading".
    let mut registry = registry().lock().expect("module views");
    // the client and its revision move as one, and their lock is let go
    // before any view is touched: a load installs under it (see
    // `spawn_load`), and takes the view's own lock inside it. Lock order,
    // everywhere: registry, then connection, then a view.
    let snapshot = {
        let mut connection = connection().lock().expect("views rpc");
        connection.rev += 1;
        connection.client = Some(client.clone());
        pages_document::source_changed();
        connection.clone()
    };
    for module in crate::backend::view_source::MODULE_OWNED {
        registry.entry(module).or_insert_with(Mounted::seat);
    }
    let loads = registry
        .iter()
        .filter_map(|(module, mounted)| {
            let mut locked = mounted.lock().expect("module view lock");
            let desktop_view = !crate::backend::view_source::module_owned(module);
            let seated = matches!(locked.slot, Slot::Ready(_));
            // the desktop's own view is the same on every node: one seated,
            // or on its way from the staged file, is left alone
            let left_alone = desktop_view && (seated || locked.in_flight);
            if left_alone {
                return None;
            }
            // a module-owned view is reloaded from this node's deployment;
            // the seat keeps what it shows until that lands
            let generation = locked.start(None);
            Some(spawn_load(module, mounted, generation, snapshot.clone()))
        })
        .collect();
    Loads(loads)
}

/// The desktop's own views, staged beside the binary: every one is asked
/// for at boot, on its own thread, and `main` [joins](Loads::joined) them
/// before the window, so the first draw of any of their tabs finds the
/// view there.
pub fn booted() -> Loads {
    let mut registry = registry().lock().expect("module views");
    let snapshot = connection().lock().expect("views rpc").clone();
    let loads = crate::backend::view_source::DESKTOP_OWNED
        .into_iter()
        .map(|module| {
            let mounted = registry.entry(module).or_insert_with(Mounted::seat);
            let generation = mounted.lock().expect("module view lock").start(None);
            spawn_load(module, mounted, generation, snapshot.clone())
        })
        .collect();
    Loads(loads)
}

/// The loads one source event started, each on its own thread. The boot
/// and the connect hand their views over — the boot joins them before the
/// window, the connect settles them before it answers — so a tab never
/// draws a seat with its answer still on the way. A block's loads swap in
/// place and are waited on by nobody but a test.
pub struct Loads(Vec<std::thread::JoinHandle<()>>);

impl Loads {
    /// Every load in: the seats hold what their source answered.
    pub fn joined(self) {
        for load in self.0 {
            load.join().expect("a view load");
        }
    }

    /// [`joined`](Self::joined), off the executor's threads: a cranelift
    /// compile is a second or more cold, which no async worker sits
    /// through.
    pub async fn settled(self) {
        tokio::task::spawn_blocking(move || self.joined())
            .await
            .expect("the view loads");
    }

    /// How many loads the event started.
    #[cfg(test)]
    pub(crate) fn started(&self) -> usize {
        self.0.len()
    }
}

/// The node's deployments moved (a new block): every module-owned view
/// whose module's active code is not the one it was drawn from is loaded
/// again, under a new generation, and swapped in place when it is ready.
/// One check in flight at a time; a block that lands during one is
/// covered by the next. The loads it starts swap in place, so nobody waits
/// on them but a test; the block stream drops them.
pub async fn deployments_checked() -> Loads {
    static IN_FLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    use std::sync::atomic::Ordering;
    if IN_FLIGHT.swap(true, Ordering::SeqCst) {
        return Loads(Vec::new());
    }
    let started = deployments_check().await;
    IN_FLIGHT.store(false, Ordering::SeqCst);
    started
}

async fn deployments_check() -> Loads {
    let asked_of = connection().lock().expect("views rpc").clone();
    let Some(client) = &asked_of.client else {
        return Loads(Vec::new());
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
            return Loads(Vec::new());
        }
    };
    let registry = registry().lock().expect("module views");
    // the connection may have moved while the registry answered: a load
    // asked of the old one dies at install, and `connected` restarted
    // everything under the new one
    if connection().lock().expect("views rpc").rev != asked_of.rev {
        return Loads(Vec::new());
    }
    let loads = registry
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
            // a candidate that just failed is left alone until its gap is
            // up: the same bytes fail the same way, so one attempt a block
            // is unbounded work for a deployment that never lands
            if waited_for || active == locked.hash || locked.held_off(active) {
                return None;
            }
            let Some(replacement) = locked.replacement_after_wait() else {
                log_source(
                    module,
                    active.as_ref(),
                    "Failed",
                    locked.generation,
                    "replacement_waiting",
                );
                return None;
            };
            let generation = locked.start(active);
            locked.replacement = replacement;
            Some(spawn_load(module, mounted, generation, asked_of.clone()))
        })
        .collect();
    Loads(loads)
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
    /// The deployment the slot answers for — the view drawn, or the empty
    /// slot of a deployment without one — so a block moves it only when the
    /// active code moved. A load that failed never seated anything, so it
    /// leaves this alone and is held off by `retry` instead.
    hash: Option<[u8; 32]>,
    /// A load is on its way for `generation`, and the deployment it is
    /// after when a block named one: a block that names it again waits
    /// for it instead of starting over.
    in_flight: bool,
    wanted: Option<[u8; 32]>,
    /// The seated view has held a replacement off with pending work since
    /// then; a block that names a deployment for it waits under the same
    /// generation instead of opening one per block.
    waiting_since: Option<Instant>,
    replacement: Replacement,
    /// The candidate a load last failed on, and when the next block may try
    /// it again. Cleared by any load that comes back, so only a repeated
    /// failure on the same candidate widens the gap.
    retry: Option<Retry>,
}

/// A failed load's hold-off: the candidate it failed on, when the next
/// attempt at that same candidate is due, and the gap that produced it.
struct Retry {
    hash: Option<[u8; 32]>,
    next: Instant,
    gap: Duration,
}

impl Retry {
    /// The hold-off after a load for `hash` failed: the gap doubles up to
    /// `RETRY_MAX` while the same candidate keeps failing, and starts over
    /// at `RETRY_FIRST` for a different one.
    fn after(previous: Option<&Retry>, hash: Option<[u8; 32]>) -> Retry {
        let gap = match previous {
            Some(previous) if previous.hash == hash => (previous.gap * 2).min(RETRY_MAX),
            _ => RETRY_FIRST,
        };
        Retry {
            hash,
            next: Instant::now() + gap,
            gap,
        }
    }
}

#[derive(Clone, Copy)]
enum Replacement {
    Preserve,
    RecoverNeverValid,
}

impl Mounted {
    /// A seat with nothing asked for yet: `Loading` until its source is
    /// asked, under generation 0, which no load answers for.
    fn seat() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            slot: Slot::Loading,
            props: None,
            generation: 0,
            hash: None,
            in_flight: false,
            wanted: None,
            waiting_since: None,
            replacement: Replacement::Preserve,
            retry: None,
        }))
    }

    fn replacement_after_wait(&mut self) -> Option<Replacement> {
        let Slot::Ready(old) = &self.slot else {
            return Some(Replacement::Preserve);
        };
        let no_pending_state = old.ticks == 0 || (old.ever_valid_tree && old.settled());
        if no_pending_state {
            return Some(Replacement::Preserve);
        }
        let since = *self.waiting_since.get_or_insert_with(Instant::now);
        let never_valid_expired =
            old.can_recover_without_state() && since.elapsed() >= REPLACEMENT_WAIT;
        never_valid_expired.then_some(Replacement::RecoverNeverValid)
    }

    /// Whether a block naming `active` is still inside the hold-off a
    /// failed load for that same candidate left behind.
    fn held_off(&self, active: Option<[u8; 32]>) -> bool {
        self.retry
            .as_ref()
            .is_some_and(|retry| retry.hash == active && Instant::now() < retry.next)
    }

    /// Opens the next generation for a load after `wanted` (None: whatever
    /// the node holds active), and names it.
    fn start(&mut self, wanted: Option<[u8; 32]>) -> u64 {
        self.generation += 1;
        self.in_flight = true;
        self.wanted = wanted;
        self.waiting_since = None;
        self.replacement = Replacement::Preserve;
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

/// The seat of `module`'s view, made on its first ask. Making it asks for
/// nothing: a load starts at the view's source event, never at a draw, so
/// a seat a tab is first to ask for waits for the next [`connected`] or
/// [`deployments_checked`] like every other.
fn mounted(module: &'static str) -> Arc<Mutex<Mounted>> {
    registry()
        .lock()
        .expect("module views")
        .entry(module)
        .or_insert_with(Mounted::seat)
        .clone()
}

/// Loads the view on its own thread — a cold cranelift compile is a second
/// or more; the window thread shows "Loading" instead of freezing for it —
/// and installs it only if `mounted` still waits for this very load AND,
/// for a module's view, the app is still on the node it was asked of, both
/// checked under the connection lock so a move cannot slip between the
/// check and the seat. The desktop's own view is the same on every node:
/// its load lands wherever the app has moved to meanwhile.
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
        let from_the_node = crate::backend::view_source::module_owned(module);
        let node_since_left = connection.rev != asked_of.rev;
        if from_the_node && node_since_left {
            return;
        }
        let Mounted {
            slot,
            hash,
            replacement,
            retry,
            ..
        } = &mut *locked;
        let replacement = *replacement;
        // a load that came back at all clears the hold-off; only the error
        // arm below puts one back, widened against the one taken here
        let held_off = retry.take();
        match loaded {
            Ok(Loaded::Fresh(mut guest)) => {
                guest.installed_generation = Some(generation);
                guest.report_display_truncation();
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
                // A never-valid view may have become usable while bytes
                // were loading. Recheck at installation before dropping it.
                let still_eligible = match replacement {
                    Replacement::Preserve => old.ticks == ticks && old.settled(),
                    Replacement::RecoverNeverValid => old.can_recover_without_state(),
                };
                if !Arc::ptr_eq(&old.alive, &alive) || !still_eligible {
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
                if let Some(root) = &fresh.frame.root
                    && let Err(reason) = fresh.inputs.retain_restored_projections(&old.inputs, root)
                {
                    log_source(module, fresh.hash.as_ref(), "Failed", generation, reason);
                    return;
                }
                pages_document::retain_source(old, &mut fresh);
                fresh.pictures = std::mem::take(&mut old.pictures);
                if let Some(root) = &mut fresh.frame.root {
                    fresh.pictures.adopt(root);
                    root.for_each_mut(&mut |node| match node {
                        wire::Node::Svg { bytes, .. } => *bytes = None,
                        wire::Node::Image { data, .. } | wire::Node::ImageViewer { data, .. } => {
                            *data = None
                        }
                        _ => {}
                    });
                }
                let reason = match replacement {
                    Replacement::RecoverNeverValid => "recovered_never_valid_view",
                    Replacement::Preserve => "",
                };
                fresh.installed_generation = Some(generation);
                fresh.report_display_truncation();
                log_source(module, fresh.hash.as_ref(), "Swapped", generation, reason);
                *hash = fresh.hash;
                *slot = Slot::Ready(fresh);
            }
            Err(Unloaded {
                hash: failed_on,
                reason,
            }) => {
                tracing::warn!(
                    target: "ducktape::app",
                    module,
                    reason = "module_view_unloadable",
                    error = %reason,
                    "module view not loaded"
                );
                // the next block leaves the candidate this failed on alone
                // until the gap is up; a failure before any candidate (a
                // status or transport error) holds nothing off, and any
                // other deployment is unaffected
                *retry = Some(Retry::after(held_off.as_ref(), failed_on));
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

/// A load that came back with no view, and the candidate it failed on —
/// none when it never got as far as one — so the block check's hold-off
/// keys on the bytes that failed, never on what the load was asked after.
struct Unloaded {
    hash: Option<[u8; 32]>,
    reason: String,
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
/// `first` for a load over an empty slot, `swap` over a view drawn.
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
/// `view_source` lines as they are logged, and the hash a slot answers for.
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
    /// Requests wait for their frame's native layout, inside this instance only.
    widget_commands: Vec<(u64, u64, wire::WidgetCommand)>,
    /// The last frame, its `root` kept across `unchanged` ticks and patched
    /// in place by a frame that carries patches instead of a tree.
    frame: wire::Frame,
    frame_reports: display_diagnostics::FrameReports,
    display_diagnostics: display_diagnostics::DisplayDiagnostics,
    installed_generation: Option<u64>,
    /// Bumped when `frame.root` changes: the widget rebuilds when it sees a
    /// number it has not rendered.
    frame_rev: u64,
    ticks: u64,
    /// A later patch gap or trap must not erase evidence of authored state.
    ever_valid_tree: bool,
    /// The live text of every input in the tree — the host's, not the guest's.
    inputs: Inputs,
    /// Every picture the guest has sent, by hash: the bytes cross once.
    pictures: Pictures,
    surfaces: Surfaces,
    /// The guest's `<module>.props` subscription, once it asked, and the
    /// props it was last given on it.
    props_subscription: Option<u64>,
    pages_document: pages_document::Pending,
    pages_source: pages_document::InstalledSource,
    pages_instance: Option<String>,
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
    ) -> Result<Loaded, Unloaded> {
        use crate::backend::view_source::{self, ViewSource};
        let before_any_candidate = |reason: String| Unloaded { hash: None, reason };
        if !view_source::module_owned(module) {
            let path = views_dir()
                .map_err(before_any_candidate)?
                .join(format!("{module}_view.wasm"));
            return Self::load_from(module, &path)
                .map(|guest| Loaded::Fresh(Box::new(guest)))
                .map_err(before_any_candidate);
        }
        let logged = |hash: Option<&[u8; 32]>, state: &str, reason: &str| {
            log_source(module, hash, state, generation, reason);
        };
        let client = match client {
            Some(client) => client,
            None => {
                let reason = "not connected to a node yet";
                logged(None, "Failed", reason);
                return Err(before_any_candidate(reason.to_owned()));
            }
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| before_any_candidate(error.to_string()))?;
        let started = Instant::now();
        let mut timing = LoadTiming {
            path: "first",
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
                return Err(before_any_candidate(reason));
            }
        };
        let (hash, component, assets) = match source {
            ViewSource::NotActivated => {
                logged(None, "NotActivated", "");
                timing.log(module, None, started, "NotActivated");
                return Err(before_any_candidate(format!(
                    "the {module} module is not activated yet"
                )));
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
                        return Err(Unloaded {
                            hash: Some(hash),
                            reason,
                        });
                    }
                };
                if !active {
                    let reason = "the active code moved while the view was prepared";
                    logged(Some(&hash), "Failed", reason);
                    timing.log(module, Some(&hash), started, "Failed");
                    return Err(Unloaded {
                        hash: Some(hash),
                        reason: reason.to_owned(),
                    });
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
            let (against, replacement) = {
                let locked = mounted.lock().expect("module view lock");
                let against = match &locked.slot {
                    Slot::Ready(old) if old.hash == Some(hash) => return Ok(Loaded::Unchanged),
                    Slot::Ready(old) => Some((old.alive.clone(), old.ticks)),
                    _ => None,
                };
                (against, locked.replacement)
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
                    // A once-valid view carries its state over. Only an
                    // explicitly admitted never-valid recovery may initialize.
                    Some((_, ticks))
                        if ticks > 0 && matches!(replacement, Replacement::Preserve) =>
                    {
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
        outcome.map_err(|reason| Unloaded {
            hash: Some(hash),
            reason,
        })
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

    /// A trap or temporary missing tree cannot revoke previously accepted
    /// authored state. Both recovery admission and installation use this fact.
    fn can_recover_without_state(&self) -> bool {
        !self.ever_valid_tree
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
            && self.widget_commands.is_empty()
            && self.inputs.editor_documents_status() == Ok(true)
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
        let mut requests = Vec::new();
        let mut cancels = Vec::new();
        let limit = wire::editor_document::MAX_EDITOR_DOCUMENTS
            * (wire::editor_document::MAX_EDITOR_CHUNKS + 3)
            + 1;
        for _ in 0..limit {
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
            requests.append(&mut self.frame.requests);
            cancels.append(&mut self.frame.cancels);
            if requests.len() > MAX_REQUESTS_PER_TICK || cancels.len() > 2 * MAX_REQUESTS_PER_TICK {
                return Err(format!(
                    "{shown}: replacement requests exceed the first-frame budget"
                ));
            }
            if self
                .inputs
                .editor_documents_status()
                .map_err(str::to_owned)?
                && self.pending.is_empty()
            {
                break;
            }
        }
        if !self
            .inputs
            .editor_documents_status()
            .map_err(str::to_owned)?
            || !self.pending.is_empty()
        {
            return Err(format!(
                "{shown}: replacement document transfer did not complete"
            ));
        }
        requests.retain(|request| !cancels.contains(&request.id));
        self.frame.requests = requests;
        self.frame.cancels = cancels;
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
            .ok_or_else(|| format!("{shown}: the component's manifest cannot be read"))?
            .check_wire_protocol()
            .map_err(|error| format!("{shown}: {error}"))?;
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
            widget_commands: Vec::new(),
            frame: wire::Frame::default(),
            frame_reports: Default::default(),
            display_diagnostics: Default::default(),
            installed_generation: None,
            frame_rev: 0,
            ticks: 0,
            ever_valid_tree: false,
            inputs: Inputs::default(),
            pictures: Pictures::default(),
            surfaces: surfaces_of(module),
            props_subscription: None,
            pages_document: None,
            pages_source: Default::default(),
            pages_instance: (module == "pages")
                .then(|| crate::backend::fresh_operation_id("pages-view".into())),
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
            // the chat composer's submit carries its body; the other
            // unrouted surfaces only say that something happened
            if self.module == "chat" || self.module == "forge" {
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
        pages_document::drive(self);
        if self.staged {
            // a replacement's first tree is already here; only its
            // requests and cancels are still to route
            self.staged = false;
        } else {
            let quiet = self.ticks > 0
                && !self.frame.busy
                && self.pending.is_empty()
                && self.inputs.editor_documents_status() != Ok(false);
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
            self.widget_commands
                .retain(|(request, _, _)| *request != id);
            if self
                .pages_document
                .as_ref()
                .is_some_and(|(pending, _)| *pending == id)
            {
                self.pages_document = None;
            }
            if self.props_subscription == Some(id) {
                self.props_subscription = None;
            }
        }
        self.fault.is_none()
            && (self.frame.busy
                || !self.pending.is_empty()
                || self.pages_document.is_some()
                || self.inputs.editor_documents_status() == Ok(false))
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
            ("host", "widget") => self.widget_request(id, &payload),
            ("pages", "document") if own => pages_document::request(self, id, &payload),
            _ if own && operation == "props" => {
                self.props_subscription = Some(id);
                self.props_sent = None;
                self.sync_props(props);
            }
            ("pages", "edited") if own => pages_document::emit(self, id, &payload),
            ("pages", "installed") if own => pages_document::installed(self, &payload),
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

    fn widget_request(&mut self, id: u64, payload: &[u8]) {
        let admitted = (|| {
            let mut command: wire::WidgetCommand = wire::decode(payload)?;
            let exact_payload = wire::encoded_size(&command) == payload.len() as u64;
            if !exact_payload {
                return Err("widget request has trailing bytes".into());
            }
            command.validate()?;
            let queue_full = self.widget_commands.len() >= MAX_REQUESTS_PER_TICK;
            if queue_full {
                return Err("too many pending widget requests".into());
            }
            // Exhaustive by design: adding a command requires reviewing its scope.
            use wire::WidgetCommand as C;
            let target = match &command {
                C::FocusPrevious | C::FocusNext => None,
                C::Focus { target }
                | C::Focused { target }
                | C::CursorFront { target }
                | C::CursorEnd { target }
                | C::Cursor { target, .. }
                | C::SelectAll { target }
                | C::Select { target, .. }
                | C::Snap { target, .. }
                | C::SnapEnd { target }
                | C::ScrollTo { target, .. }
                | C::ScrollToKey { target, .. }
                | C::ScrollBy { target, .. } => Some(target),
            };
            fn contains(node: &wire::Node, target: &str) -> bool {
                node.key() == Some(target)
                    || node.children().iter().any(|child| contains(child, target))
            }
            if let Some(target) = target {
                let in_scope = self
                    .frame
                    .root
                    .as_ref()
                    .is_some_and(|root| contains(root, target));
                if !in_scope {
                    return Err("widget target is outside this guest tree".into());
                }
            }
            Ok(command)
        })();
        match admitted {
            Ok(command) => self.widget_commands.push((id, self.frame_rev, command)),
            Err(error) => self.refuse(id, error),
        }
    }

    /// Called only on the matching mounted tree after native editor work drains.
    fn execute_widget_commands(&mut self, mut traverse: impl FnMut(&mut dyn Operation)) {
        for (id, revision, command) in std::mem::take(&mut self.widget_commands) {
            let result = if revision == self.frame_rev {
                view_tree::execute_widget_command(command, &mut traverse)
            } else {
                Err("widget request belongs to a replaced frame".into())
            };
            self.reply(id, result);
        }
    }

    fn reply(&mut self, id: u64, result: Result<Vec<u8>, String>) {
        self.pending.push(wire::Event::Response {
            id,
            result,
            done: true,
        });
    }

    fn report_display_truncation(&mut self) {
        let Some(generation) = self.installed_generation else {
            return;
        };
        for origin in self
            .display_diagnostics
            .observe(self.frame_reports)
            .into_iter()
            .flatten()
        {
            tracing::warn!(target: "ducktape::app", module = self.module, generation,
                reason = "display_text_truncated", origin, "module view display text truncated");
        }
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
            Ok((mut frame, mut reports)) => {
                let inherits = frame.root.is_none();
                let mut previous = self.frame.root.take();
                let mut accepted = true;
                let merged = merge(&mut previous, &mut frame).and_then(|changed| {
                    if changed.0
                        && let Some(root) = &frame.root
                    {
                        self.inputs.validate_editor_documents(root)?;
                    }
                    Ok(changed)
                });
                match merged {
                    Ok((false, _)) => {}
                    Ok((true, report)) => {
                        reports.local.merge(report);
                        self.frame_rev += 1;
                        if let Some(root) = &mut frame.root {
                            self.ever_valid_tree = true;
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
                    // Preserve accepted document state while requesting a full tree.
                    Err(refused) => {
                        accepted = false;
                        frame.root = previous;
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
                if accepted {
                    if inherits {
                        reports.inherit(self.frame_reports);
                    }
                    self.frame_reports = reports;
                    self.report_display_truncation();
                    if self.inputs.editor_frame(&frame, &mut self.pending) {
                        self.frame_rev += 1;
                    }
                }
                self.frame = frame;
                pages_document::verify_installed(self);
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
fn merge(
    held: &mut Option<wire::Node>,
    frame: &mut wire::Frame,
) -> Result<(bool, wire::SanitizeReport), &'static str> {
    if frame.unchanged {
        frame.root = held.take();
        return Ok((false, Default::default()));
    }
    if frame.root.is_some() {
        return Ok((true, Default::default()));
    }
    let patches = std::mem::take(&mut frame.patches);
    let mut root = held.as_ref().ok_or("no tree to patch")?.clone();
    let report = wire::apply(&mut root, patches)?;
    frame.root = Some(root);
    Ok((true, report))
}

/// What the host is willing to take from one tick's bytes: nothing in here
/// is trusted — the length, the counts, the tree.
fn shape(bytes: &[u8]) -> Result<(wire::Frame, display_diagnostics::FrameReports), String> {
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
    let upstream = frame.upstream_sanitization;
    let local = wire::sanitize(&mut frame).map_err(str::to_owned)?;
    frame.upstream_sanitization = upstream;
    Ok((frame, display_diagnostics::FrameReports { local, upstream }))
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

#[path = "module_view/display_diagnostics.rs"]
mod display_diagnostics;

#[path = "module_view/input.rs"]
mod input;
#[cfg(test)]
#[path = "module_view/input_tests.rs"]
mod input_tests;

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
    alive: Arc<()>,
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
        let same_instance =
            *generation == self.generation && Arc::ptr_eq(&guest.alive, &self.alive);
        let outputs = if same_instance {
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
        if same_instance
            && let Event::Mouse(event) = event
            && input::mouse(
                guest,
                *event,
                layout.bounds().position(),
                shell.is_event_captured(),
            )
        {
            shell.request_redraw();
        }
        let Event::Window(window::Event::RedrawRequested(_)) = event else {
            return;
        };
        if guest.redraw(props) {
            shell.request_redraw();
        }
        let native_frame_ready = same_instance
            && self.rev == guest.frame_rev
            && guest.fault.is_none()
            && !guest.staged
            && !shell.is_layout_invalid()
            && !shell.are_widgets_invalid()
            && !guest.inputs.editor_transactions_pending()
            && !guest.widget_commands.is_empty();
        if native_frame_ready {
            guest.execute_widget_commands(|operation| {
                self.content
                    .as_widget_mut()
                    .operate(tree, layout, renderer, operation);
                if let Some(mut overlay) = self
                    .content
                    .as_widget_mut()
                    .overlay(tree, layout, renderer, viewport, Vector::ZERO)
                    .map(overlay::Nested::new)
                {
                    let layout = overlay.layout(renderer, viewport.size());
                    overlay.operate(Layout::new(&layout), renderer, operation);
                }
            });
            shell.request_redraw();
        }
        if !guest.widget_commands.is_empty() {
            shell.request_redraw();
        }
        for intent in std::mem::take(&mut guest.intents) {
            shell.publish(intent);
        }
        // A new tree is re-rendered here, in place: the app's own view is
        // rebuilt only by its own messages, and a guest's tick is not one.
        if guest.frame_rev != self.rev {
            self.rev = guest.frame_rev;
            self.alive = guest.alive.clone();
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
        let alive = {
            let mounted = self.mounted.lock().expect("module view lock");
            let Slot::Ready(guest) = &mounted.slot else {
                return None;
            };
            if mounted.generation != self.generation
                || guest.frame_rev != self.rev
                || !Arc::ptr_eq(&guest.alive, &self.alive)
            {
                return None;
            }
            guest.alive.clone()
        };
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
            .map(|content| {
                input::overlay(
                    content,
                    self.mounted.clone(),
                    self.generation,
                    alive,
                    layout.bounds().position() + translation,
                )
            })
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
        assert_eq!(
            intents_of("agents"),
            [
                "status",
                "save",
                "register",
                "open_run",
                "open_link",
            ]
        );
        let chat = intents_of("chat");
        assert_eq!(chat.len(), 45);
        assert!(chat.contains(&"choose_channel"));
        assert!(
            chat.contains(&"cancel_run"),
            "stopping an anchored agent run is an act the screen offers"
        );
        assert!(
            !chat.contains(&"composer"),
            "a submit reaches the app only through the composer surface it was typed in"
        );
    }

    /// Every kind a module's decoder names is admitted at that module's
    /// door, so a view never emits an intent the app knows how to decode
    /// but refuses to hear. The kinds a decoder names off its door are
    /// exactly the ones that cross by another route: a host surface's own
    /// event, or the composer's submit. In the other direction the door
    /// admits nothing the decoder leaves to its wildcard, except the kind
    /// the wildcard's verdict itself names.
    #[test]
    fn every_kind_a_decoder_names_is_admitted_at_its_door() {
        use std::collections::BTreeSet;
        let (source, _tests) = include_str!("module_view.rs")
            .split_once("\npub(crate) mod tests {")
            .expect("the tests module");
        let other_route_only: [(&str, &str, &[&str]); 10] = [
            ("governance", "gov_intent", &[]),
            ("members", "roster_intent", &[]),
            ("agents", "agents_intent", &[]),
            ("node", "node_intent", &["log_timeline"]),
            ("explorer", "explorer_intent", &[]),
            ("settings", "settings_intent", &[]),
            ("forge", "forge_intent", &[]),
            ("pages", "pages_intent", &["edited"]),
            ("chat", "chat_intent", &["composer"]),
            ("files", "files_intent", &[]),
        ];
        let snake = |variant: &str| -> String {
            let mut word = String::new();
            for (index, letter) in variant.chars().enumerate() {
                let starts_a_word = letter.is_ascii_uppercase() && index > 0;
                if starts_a_word {
                    word.push('_');
                }
                word.push(letter.to_ascii_lowercase());
            }
            word
        };
        for (module, decoder, other_routes) in other_route_only {
            let header = format!("pub fn {decoder}(event: &ModuleViewEvent)");
            let body = &source[source.find(&header).expect(decoder)..];
            let body = &body[..body.find("\n}\n").expect("the decoder's end")];
            let mut arms = BTreeSet::new();
            let mut wildcard_verdict = None;
            for line in body.lines().map(str::trim) {
                let named_arm = line
                    .strip_prefix('"')
                    .and_then(|rest| rest.split_once("\" =>"))
                    .map(|(kind, _)| kind);
                if let Some(kind) = named_arm {
                    arms.insert(kind);
                }
                if let Some(verdict) = line.strip_prefix("_ => ") {
                    let variant = verdict.trim_end_matches(',').rsplit("::").next();
                    wildcard_verdict = variant.map(snake);
                }
            }
            let door: BTreeSet<&str> = intents_of(module).iter().copied().collect();
            let off_door: Vec<&str> = arms.difference(&door).copied().collect();
            assert_eq!(
                off_door, other_routes,
                "{module}: {decoder} names a kind the door refuses"
            );
            let left_to_the_wildcard: Vec<&str> = door.difference(&arms).copied().collect();
            let wildcard_verdict = wildcard_verdict.expect("a total decoder ends in a wildcard");
            assert!(
                left_to_the_wildcard
                    .iter()
                    .all(|kind| *kind == wildcard_verdict),
                "{module}: the door admits {left_to_the_wildcard:?}, which {decoder} decodes only by its wildcard ({wildcard_verdict})"
            );
        }
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
        message.unwrap_or_else(|| {
            let mut seen = Vec::new();
            let mut root = guest.frame.root.clone().expect("a tree");
            root.for_each_mut(&mut |node| {
                if let wire::Node::Button {
                    label,
                    content,
                    on_press,
                    ..
                } = node
                {
                    let shown = label.clone().unwrap_or_else(|| match content {
                        wire::ButtonContent::Label(text) => text.clone(),
                        _ => "<no label>".to_owned(),
                    });
                    seen.push(format!(
                        "{shown}{}",
                        if on_press.is_some() {
                            ""
                        } else {
                            " (disabled)"
                        }
                    ));
                }
            });
            panic!(
                "no enabled button named {name:?}; the frame has buttons {seen:?} and texts {:?}",
                texts(guest)
            )
        })
    }

    /// Whether a button showing `name` is on the frame at all.
    ///
    /// A BUTTON'S LABEL IS NOT A TEXT NODE, so `texts()` never contains it and
    /// asserting over that list says nothing about a button either way — an
    /// `any(== "Stop")` fails on a button that is plainly there, and the
    /// `!any(== "Stop")` twin passes whether it is there or not.
    fn button_shown(guest: &Guest, name: &str) -> bool {
        let mut root = guest.frame.root.clone().expect("a tree");
        let mut found = false;
        root.for_each_mut(&mut |node| {
            if let wire::Node::Button { label, content, .. } = node
                && (label.as_deref() == Some(name)
                    || matches!(content, wire::ButtonContent::Label(text) if text == name))
            {
                found = true;
            }
        });
        found
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
    /// Redraw until the view is quiet and every editor document it draws
    /// has crossed the transfer: an editor node carries a document
    /// reference, and its bytes arrive over frames, so a test that reads
    /// the document right after the redraw that mounted the editor reads
    /// nothing. The pump is bounded by the frames a transfer can take.
    fn settle_documents(guest: &mut Guest, props: &Option<Vec<u8>>) {
        for _ in 0..128 {
            let busy = guest.redraw(props);
            assert!(guest.fault.is_none(), "{:?}", guest.fault);
            if !busy && guest.inputs.editor_documents_status() == Ok(true) {
                return;
            }
        }
        panic!(
            "the view did not settle: frame busy={} pending={:?} staged={:?} documents={:?} texts={:?}",
            guest.frame.busy,
            guest.pending,
            guest.staged,
            guest.inputs.editor_documents_status(),
            texts(guest)
        );
    }

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
        // the props the app ENCODES, not a hand-written shape beside the
        // encoder: a field the encoder dropped, or the guest stopped taking,
        // fails here
        let skill = |name: &str, always: bool| crate::backend::AgentSkill {
            name: name.into(),
            source_prefix: format!("/shared/skills/{name}"),
            source_snapshot: String::new(),
            always,
        };
        let reviewer = crate::backend::AgentRow {
            id: "reviewer-bot".into(),
            name: "Reviewer Bot".into(),
            initials: "RB".into(),
            capability: "review".into(),
            status: "paused".into(),
            owner_handle: "eddy".into(),
            controller: "7".into(),
            live: false,
            allowed_actions: vec!["chat.post".into()],
            caps: crate::backend::AgentCaps {
                forge_read: vec!["ducktape".into()],
                pages_write: vec!["*".into()],
                ..Default::default()
            },
            skills: vec![
                skill("review", true),
                skill("style", false),
                skill("tests", false),
            ],
        };
        let props = Some(agents_props(
            false,
            true,
            true,
            "",
            0,
            &[reviewer],
            &[],
            "",
            0,
            &crate::backend::RunJournal::default(),
            &crate::backend::LiveRun::default(),
            &["claude".into(), "review".into()],
            &["chat.post".into(), "tasks.create".into()],
        ));
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

    #[test]
    fn the_staged_agents_view_renders_semantic_actions_and_opens_the_exact_target() {
        let Some(staged) = staged("agents") else {
            return;
        };
        let mut guest = Guest::load_from("agents", &staged).expect("the view loads");
        let run = crate::backend::RunRow {
            run_id: "machine-run-hash".into(),
            dispatch_id: "machine-dispatch-hash".into(),
            agent_id: "reviewer".into(),
            agent_name: "Reviewer".into(),
            origin: "#Engineering · Message 42".into(),
            state: "running".into(),
            dispatched: "h 1".into(),
            settled: String::new(),
            attempt: 1,
            holder: String::new(),
            actions: 1,
            degraded: false,
            reason: String::new(),
            output_ref: String::new(),
            pr_number: 0,
        };
        let target = crate::backend::RunLink {
            relation: "target".into(),
            kind: "chat".into(),
            label: "#Engineering · Eddy: Bound and scroll".into(),
            url: "duck://channel/room?net=a1b2c3d4#42".into(),
        };
        let journal = crate::backend::RunJournal {
            dispatch_id: run.dispatch_id.clone(),
            entries: vec![crate::backend::JournalEntry {
                height: "h 2".into(),
                kind: "action".into(),
                summary: "React 👀".into(),
                status: "Completed".into(),
                targets: vec![target.clone()],
            }],
            ..Default::default()
        };
        let props = Some(agents_props(
            false,
            true,
            true,
            "7",
            0,
            &[],
            std::slice::from_ref(&run),
            &run.dispatch_id,
            1,
            &journal,
            &crate::backend::LiveRun::default(),
            &[],
            &[],
        ));
        guest.redraw(&None);
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in ["React 👀", "Completed", &target.label] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected}"
            );
        }
        assert!(
            !shown
                .iter()
                .any(|text| text.contains("machine-dispatch-hash"))
        );
        assert!(guest.fault.is_none());
        guest.deliver(Output::Activate(button_message(&guest, &target.label)));
        guest.redraw(&props);
        let intent = guest.intents.last().expect("target navigation");
        assert_eq!(intent.kind, "open_link");
        assert!(matches!(agents_intent(intent), crate::AgentsIntent::OpenLink));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&intent.detail).unwrap()["url"],
            target.url
        );
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
    fn the_staged_pages_view_boots_takes_the_facts_and_owns_the_document() {
        let Some(staged) = staged("pages") else {
            return;
        };
        let mut guest = Guest::load_from("pages", &staged).expect("the view loads");
        assert!(!guest.surfaces.contains_key("page_document"));
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
        assert!(surface_names(&guest).is_empty());

        guest.deliver(Output::Activate(button_message(&guest, "Beta")));
        guest.redraw(&props);
        assert_eq!(
            std::mem::take(&mut guest.intents),
            [ModuleViewEvent {
                kind: "choose".into(),
                detail: r#"{"id":"beta","comment_draft":""}"#.into(),
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

    /// EVERY DIGEST THE LEDGER PUBLISHES REACHES THE SCREEN WHOLE, IN HEX.
    /// The rows are the node's own `GET /v1/blocks` shape, the props are what
    /// `explorer_window` makes of them — the production conversion, not a
    /// hand-built prop — and the reader is the bundled wasm guest. Nothing
    /// between the two may cut a digest or leave one in decimal: the block
    /// hash on the list row, the commit hash and the op hash in the detail,
    /// and the `new_oid` inside the payload all read `0x` and every character
    /// — and the copy intent carries the bare key the blob route takes.
    #[test]
    fn the_staged_explorer_view_shows_every_published_digest_whole_and_in_hex() {
        let Some(staged) = staged("explorer") else {
            return;
        };
        let hash = "9f3e".repeat(16);
        let commit = "c0ffee11".repeat(8);
        let op_hash = "dd".repeat(32);
        let oid: Vec<u8> = (1..=20).collect();
        let rows = vec![serde_json::json!({
            "height": 84_912,
            "hash": hash,
            "commit_hash": commit,
            "ops": [{
                "proposer": "cc".repeat(32),
                "target": "forge",
                "disposition": "applied",
                "op_hash": op_hash,
                "payload": serde_json::to_string(&serde_json::json!({
                    "push": { "new_oid": oid }
                }))
                .expect("payload encodes"),
                "operations": []
            }]
        })];
        let ledger = crate::backend::explorer_window(1, &rows);

        let mut guest = Guest::load_from("explorer", &staged).expect("the view loads");
        guest.redraw(&None);
        let props = Some(
            serde_json::to_vec(&serde_json::json!({
                "connected": true, "loading": false, "dark": false,
                "blocks": ledger.blocks, "ops": ledger.ops,
                "head": 84_912, "sync_line": "live",
                "hits": [], "kinds": [], "partial": "", "searching": false, "sent_query": ""
            }))
            .expect("props encode"),
        );
        guest.redraw(&props);

        let whole = format!("0x{hash}");
        let listed = texts(&guest);
        assert!(
            listed.contains(&whole),
            "the list row carries the whole block hash: {listed:?}"
        );
        // and nothing on it is a cut-down version of that hash — the guard
        // that fails the moment a landmark form comes back.
        let abbreviated = listed
            .iter()
            .find(|text| text.starts_with("0x9f3e") && **text != whole);
        assert!(
            abbreviated.is_none(),
            "the list carries the whole hash, not {abbreviated:?}"
        );

        guest.deliver(Output::Activate(button_message(&guest, "Inspect block")));
        guest.redraw(&props);
        let opened = texts(&guest);
        for expected in [format!("0x{commit}"), format!("0x{op_hash}")] {
            assert!(
                opened.iter().any(|text| text == &expected),
                "missing {expected:?} in {opened:?}"
            );
        }
        assert!(
            opened
                .iter()
                .any(|text| text
                    .contains("\"new_oid\": \"0x0102030405060708090a0b0c0d0e0f1011121314\"")),
            "the payload's digest is hex too: {opened:?}"
        );

        // AND THE CLIPBOARD GETS THE KEY, not the reading of it: `0x` is for
        // the eye, and `GET /v1/files/blob/{op_hash}` takes the bare digest.
        guest.deliver(Output::Activate(button_message(&guest, "Copy op hash")));
        guest.redraw(&props);
        assert_eq!(
            std::mem::take(&mut guest.intents),
            [ModuleViewEvent {
                kind: "copy".into(),
                detail: format!(r#"{{"text":"{op_hash}","label":"Op hash copied"}}"#),
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
        settle_documents(&mut guest, &None);
        settle_documents(&mut guest, &props(&facts));
        guest.deliver(Output::Activate(button_message(&guest, "Edit")));
        settle_documents(&mut guest, &props(&facts));
        let mut editor = None;
        guest.frame.root.clone().unwrap().for_each_mut(&mut |node| {
            if let wire::Node::Editor { key, document, .. } = node {
                editor = Some((key.clone(), document.reset));
            }
        });
        let (key, reset) = editor.expect("editable document");
        guest.deliver(Output::EditorAction {
            key: key.clone(),
            reset,
            action: iced::widget::text_editor::Action::Edit(
                iced::widget::text_editor::Edit::Insert('X'),
            ),
        });
        // EditorAction admits native work; the mounted editor executes it.
        use iced::advanced::renderer::Headless;
        use iced_test::runtime::{UserInterface, user_interface};
        let mut renderer = iced::futures::executor::block_on(<iced::Renderer as Headless>::new(
            iced::Font::DEFAULT,
            iced::Pixels(14.0),
            Some("tiny-skia"),
        ))
        .unwrap();
        let mut ui = UserInterface::build(
            guest.render(),
            iced::Size::new(1100.0, 700.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut outputs = Vec::new();
        ui.update(
            &[Event::Window(
                window::Event::RedrawRequested(Instant::now()),
            )],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut outputs,
        );
        drop(ui);
        assert!(
            outputs
                .iter()
                .any(|output| matches!(output, Output::EditorBatch(_)))
        );
        for output in outputs {
            guest.deliver(output);
        }
        settle_documents(&mut guest, &props(&facts));
        assert_eq!(
            guest.inputs.editor_document(&key).unwrap().text(),
            original_text,
            "the mounted editor must apply X before navigation"
        );
        let old_save = button_message(&guest, "Save");
        facts["network_scope"] = "network-b".into();
        facts["context"] = "connection-b".into();
        facts["preview_path"] = "/shared/other.md".into();
        facts["preview_text"] = "B source".into();
        settle_documents(&mut guest, &props(&facts));
        guest.deliver(Output::Activate(old_save));
        settle_documents(&mut guest, &props(&facts));
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
        settle_documents(&mut guest, &props(&facts));
        let draft_text = |guest: &Guest| {
            let mut value = None;
            guest.frame.root.clone().unwrap().for_each_mut(&mut |node| {
                if let wire::Node::Editor { key, .. } = node {
                    value = guest
                        .inputs
                        .editor_document(key)
                        .map(|doc| doc.text().to_owned());
                }
            });
            value.expect("the retained editor")
        };
        assert_eq!(draft_text(&guest), original_text);
        guest.deliver(Output::Activate(button_message(&guest, "Save")));
        settle_documents(&mut guest, &props(&facts));
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
        settle_documents(&mut guest, &props(&facts));
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
        let editor_text = |guest: &Guest| {
            let mut text = None;
            guest.frame.root.clone().unwrap().for_each_mut(&mut |node| {
                if let wire::Node::Editor { key, .. } = node {
                    text = guest
                        .inputs
                        .editor_document(key)
                        .map(|doc| doc.text().to_owned());
                }
            });
            text
        };
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
            settle_documents(&mut guest, &props(&facts));
            assert_eq!(
                editor_text(&guest),
                Some(facts["preview_text"].as_str().unwrap().to_owned()),
                "old instance completion must not close the fresh editor"
            );
            facts["save_reply"]["replies"][0]["namespace"] = save["namespace"].clone();
            facts["loading"] = false.into();
            guest.redraw(&props(&facts));
            assert!(
                editor_text(&guest).is_none(),
                "its own confirmation closes the editor"
            );
            assert!(guest.fault.is_none());
        }
    }

    /// A module-owned view has one source, the module's deployment on the
    /// connected node: with no node it is not there yet, and the staged file
    /// a desktop view would take is never opened for it.
    #[test]
    fn a_module_owned_view_never_comes_from_the_staged_file() {
        // a staged file for every one of them, where `views_dir` would look
        let _turn = blocking_connection_turn();
        let staged = tempfile::tempdir().expect("a staging dir");
        for module in crate::backend::view_source::MODULE_OWNED {
            std::fs::write(staged.path().join(format!("{module}_view.wasm")), b"\0asm")
                .expect("staged");
        }
        // SAFETY: set under the connection turn, the only one a desktop
        // view — the one reader of this variable — is loaded under;
        // `every_desktop_view_is_asked_at_boot` sets it too, under its own.
        unsafe { std::env::set_var("DUCKTAPE_VIEWS_DIR", staged.path()) };
        for module in crate::backend::view_source::MODULE_OWNED {
            let mounted = Mounted::seat();
            assert_eq!(
                Guest::load(module, None, 0, &mounted)
                    .err()
                    .map(|unloaded| unloaded.reason)
                    .as_deref(),
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

        // seated with no node: nothing asked yet; then A is asked and holds
        let mounted = mounted("forge");
        let asked_of_a = connected(&node_a);
        // the app moves to B while A is still composing its answer
        let asked_of_b = connected(&node_b);
        asked_of_b.joined();
        hold.notify_one();
        asked_of_a.joined();
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
    fn connection_turn_lock() -> &'static tokio::sync::Mutex<()> {
        static TURN: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
        TURN.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    fn reset_connection_turn() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
        // Retire intentionally busy/failed seats left by the previous test.
        // Keep revisions monotonic so detached old loads remain stale.
        let mut registry = registry().lock().expect("module views");
        let mut connection = connection().lock().expect("views rpc");
        registry.clear();
        connection.client = None;
        connection.rev += 1;
    }

    /// One test's turn over the seats: taken with them retired, and
    /// retiring them again when it ends, so a test outside a turn never
    /// inherits what a deployment left seated — a pages test reads the
    /// `pages` seat through `current_page_document`, and a seat another
    /// test left behind is a document it never staged.
    pub(crate) struct ConnectionTurn {
        _held: tokio::sync::MutexGuard<'static, ()>,
    }

    impl Drop for ConnectionTurn {
        fn drop(&mut self) {
            reset_connection_turn();
        }
    }

    pub(crate) async fn connection_turn() -> ConnectionTurn {
        let held = connection_turn_lock().lock().await;
        reset_connection_turn();
        ConnectionTurn { _held: held }
    }

    /// Hold this outside allocation measurement until the render thread joins.
    pub(crate) fn blocking_connection_turn() -> ConnectionTurn {
        let held = connection_turn_lock().blocking_lock();
        reset_connection_turn();
        ConnectionTurn { _held: held }
    }

    #[test]
    fn connection_turn_excludes_renderers_and_retires_the_previous_busy_guest() {
        let turn = blocking_connection_turn();
        let staged = staged("chat").expect("build the actual Chat guest");
        let mut guest = Guest::load_from("chat", &staged).expect("actual Chat guest");
        guest.pending.push(wire::Event::Resync);
        let seat = Arc::new(Mutex::new(Mounted {
            slot: Slot::Ready(Box::new(guest)),
            props: Some(b"previous test props".to_vec()),
            generation: 9,
            hash: Some([7; 32]),
            in_flight: false,
            wanted: None,
            waiting_since: None,
            replacement: Replacement::Preserve,
            retry: None,
        }));
        registry().lock().unwrap().insert("chat", seat.clone());
        let revision = connection().lock().unwrap().rev;
        std::thread::spawn(|| {
            assert!(
                connection_turn_lock().try_lock().is_err(),
                "a renderer cannot enter while a deployment owns the seats"
            );
        })
        .join()
        .unwrap();
        drop(turn);
        let _next = blocking_connection_turn();
        assert!(
            registry().lock().unwrap().is_empty(),
            "the next test must not inherit a pending guest or its props/assets"
        );
        assert!(connection().lock().unwrap().rev > revision);
        let locked = seat.lock().unwrap();
        let Slot::Ready(guest) = &locked.slot else {
            panic!("old seat")
        };
        assert_eq!(
            guest.pending.len(),
            1,
            "retirement must not edit the old guest"
        );
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

    /// The connect seats and loads every module-owned view, drawn or not,
    /// and a draw before any node asks for nothing.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn every_module_owned_view_is_asked_at_connect_not_at_its_tabs_first_draw() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::MODULE_OWNED;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let Some(staged) = staged("governance") else {
            return;
        };
        let component = std::fs::read(staged).expect("the staged view");
        let a = deployment(&component, "a.svg");
        let client = fake_node(FakeDeployment::serving("governance", &a)).await;
        // a tab drawn before any node: a seat, with nothing on its way
        drop(drawn("files"));
        {
            let seat = mounted("files");
            let locked = seat.lock().expect("module view lock");
            assert!(matches!(locked.slot, Slot::Loading));
            assert!(!locked.in_flight, "the draw started a load");
            assert_eq!(locked.generation, 0);
        }
        let loads = connected(&client);
        assert_eq!(
            loads.started(),
            MODULE_OWNED.len(),
            "one load per module-owned view, drawn or not"
        );
        for module in MODULE_OWNED {
            let seat = mounted(module);
            let locked = seat.lock().expect("module view lock");
            assert_eq!(
                locked.generation, 1,
                "{module} was not asked of the node at connect"
            );
        }
        loads.joined();
        assert_eq!(slot_assets(&mounted("governance")), ["a.svg"]);
        for module in MODULE_OWNED {
            registry().lock().expect("module views").remove(module);
        }
    }

    /// The desktop's own views are all asked for at boot and joined before
    /// the window, so the first draw of any of their tabs finds the view
    /// there: every one of them, once the boot's loads are in.
    #[test]
    fn every_desktop_view_is_asked_at_boot() {
        let _turn = blocking_connection_turn();
        use crate::backend::view_source::DESKTOP_OWNED;
        let Some(staged) = staged("members") else {
            return;
        };
        // SAFETY: set under the connection turn, the only one a desktop
        // view is loaded under; `a_module_owned_view_never_comes_from_the_
        // staged_file` sets it too, under its own turn.
        unsafe { std::env::set_var("DUCKTAPE_VIEWS_DIR", staged.parent().expect("staging dir")) };
        let loads = booted();
        assert_eq!(
            loads.started(),
            DESKTOP_OWNED.len(),
            "one load per desktop view"
        );
        for module in DESKTOP_OWNED {
            let seat = mounted(module);
            let locked = seat.lock().expect("module view lock");
            assert_eq!(locked.generation, 1, "{module} was not asked for at boot");
        }
        loads.joined();
        for module in DESKTOP_OWNED {
            let seat = mounted(module);
            let locked = seat.lock().expect("module view lock");
            assert!(!locked.in_flight, "{module}");
            match &locked.slot {
                Slot::Ready(_) => {}
                Slot::Failed(reason) => panic!("{module} is not there: {reason}"),
                Slot::Loading | Slot::Empty => panic!("{module}'s load never answered"),
            }
        }
        for module in DESKTOP_OWNED {
            registry().lock().expect("module views").remove(module);
        }
    }

    /// The desktop's own view is the same on every node: its load lands
    /// though the app moved to a node meanwhile, where a module's view
    /// asked of the node since left lands nowhere.
    #[test]
    fn a_desktop_views_load_lands_after_the_app_moves_where_a_modules_does_not() {
        let _turn = blocking_connection_turn();
        let since_left = Connection {
            client: None,
            rev: connection().lock().expect("views rpc").rev - 1,
        };
        for (module, lands) in [("members", true), ("files", false)] {
            let seat = Mounted::seat();
            let generation = seat.lock().expect("module view lock").start(None);
            spawn_load(module, &seat, generation, since_left.clone())
                .join()
                .expect("the load");
            let locked = seat.lock().expect("module view lock");
            assert!(!locked.in_flight, "{module}");
            let landed = !matches!(locked.slot, Slot::Loading);
            assert_eq!(landed, lands, "{module}");
        }
    }

    /// The connect answers only once every module-owned view has settled:
    /// while one node answer is still held, the loads are not settled; the
    /// moment it is released they are, and no seat is on its way.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_connect_settles_only_once_every_module_view_answered() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::MODULE_OWNED;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let Some(staged) = staged("governance") else {
            return;
        };
        let component = std::fs::read(staged).expect("the staged view");
        let a = deployment(&component, "a.svg");
        let node = FakeDeployment::serving("governance", &a);
        let hold = hold_status(&node);
        let client = fake_node(node.clone()).await;
        let settled = tokio::spawn(connected(&client).settled());
        // one status answer is waiting on the hold: that load cannot be
        // in, so neither can the loads
        node.held.notified().await;
        assert!(
            !settled.is_finished(),
            "the connect settled with a node answer still held"
        );
        hold.notify_one();
        settled.await.expect("the loads settle");
        for module in MODULE_OWNED {
            let seat = mounted(module);
            let locked = seat.lock().expect("module view lock");
            assert!(!locked.in_flight, "{module} is still on its way");
            assert!(
                !matches!(locked.slot, Slot::Loading),
                "{module} settled without an answer"
            );
        }
        assert_eq!(slot_assets(&mounted("governance")), ["a.svg"]);
        for module in MODULE_OWNED {
            registry().lock().expect("module views").remove(module);
        }
    }

    /// A seat keeps what it shows through a reconnect: one failed off the
    /// previous node shows that failure, never "Loading", until the new
    /// node's answer lands — and then that.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_reconnect_keeps_what_a_seat_shows_until_the_new_node_answers() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node, node};
        let Some(staged) = staged("governance") else {
            return;
        };
        let component = std::fs::read(staged).expect("the staged view");
        let a = deployment(&component, "a.svg");
        // a node that runs none of the modules: every seat fails
        let runs_nothing = node(
            serde_json::json!({"module_status": {"modules": []}}),
            None,
            None,
        )
        .await;
        connected(&runs_nothing).joined();
        let seat = mounted("governance");
        assert!(
            matches!(seat.lock().expect("module view lock").slot, Slot::Failed(_)),
            "the seat did not fail off the node that runs nothing"
        );
        // the app moves to a node that runs governance, whose answer waits
        let node_b = FakeDeployment::serving("governance", &a);
        let hold = hold_status(&node_b);
        let client = fake_node(node_b.clone()).await;
        let loads = connected(&client);
        node_b.held.notified().await;
        {
            let locked = seat.lock().expect("module view lock");
            assert!(locked.in_flight, "governance was not asked of the new node");
            assert!(
                matches!(locked.slot, Slot::Failed(_)),
                "the seat went back to loading while the new node composed its answer"
            );
        }
        hold.notify_one();
        loads.joined();
        assert_eq!(slot_assets(&seat), ["a.svg"]);
        for module in crate::backend::view_source::MODULE_OWNED {
            registry().lock().expect("module views").remove(module);
        }
    }

    /// A load starts at a view's source event and never at a draw: the only
    /// callers of `spawn_load` are the boot, the connect and the block
    /// check, and the views the shell draws are exactly the ones those ask
    /// for.
    #[test]
    fn a_load_starts_at_a_source_event_never_at_a_draw() {
        use crate::backend::view_source::{DESKTOP_OWNED, MODULE_OWNED};
        use std::collections::BTreeSet;
        let (shell, _tests) = include_str!("module_view.rs")
            .split_once("\npub(crate) mod tests {")
            .expect("the tests module");
        let mut current = "";
        let mut callers = BTreeSet::new();
        for line in shell.lines() {
            let trimmed = line.trim_start();
            let header = [
                "pub fn ",
                "pub(crate) fn ",
                "pub async fn ",
                "async fn ",
                "fn ",
            ]
            .into_iter()
            .find_map(|keyword| trimmed.strip_prefix(keyword));
            if let Some(header) = header {
                current = header.split(['(', '<']).next().unwrap_or(header);
                continue;
            }
            if trimmed.contains("spawn_load(") {
                callers.insert(current);
            }
        }
        assert_eq!(
            callers,
            BTreeSet::from(["booted", "connected", "deployments_check"]),
            "a load started outside the boot, the connect and the block check"
        );
        let drawn: BTreeSet<&str> = shell
            .split("module_view(")
            .skip(1)
            .filter_map(|after| after.trim_start().strip_prefix('"'))
            .filter_map(|name| name.split('"').next())
            .collect();
        let asked: BTreeSet<&str> = MODULE_OWNED.into_iter().chain(DESKTOP_OWNED).collect();
        assert_eq!(
            drawn, asked,
            "the views the shell draws are not the views the boot and the connect ask for"
        );
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

    #[test]
    fn widget_commands_reach_a_mounted_nested_overlay_without_touching_a_sibling() {
        let _turn = blocking_connection_turn();
        let path = staged("pages").expect("actual guest instantiation is required");
        let mounted = fresh("pages");
        let mut guest = Guest::load_from("pages", &path).unwrap();
        let input = |key: &str| wire::Node::Input {
            options: Default::default(),
            key: key.into(),
            placeholder: String::new(),
            value: "abcd".into(),
            on_input: 0,
            on_submit: None,
            width: None,
            secure: false,
            style: Box::default(),
        };
        let popup = |key: &str, base, content| wire::Node::Overlay {
            key: key.into(),
            padding: 0.0,
            backdrop: wire::Rgba([0.0; 4]),
            align_x: wire::AlignX::Left,
            align_y: wire::AlignY::Top,
            on_dismiss: None,
            children: vec![base, content],
        };
        guest.frame.root = Some(popup(
            "outer",
            wire::Node::empty(),
            popup("inner", wire::Node::empty(), input("popup-draft")),
        ));
        guest.inputs.adopt(guest.frame.root.as_ref().unwrap());
        // Freeze the fixture projection, not responses: ModuleView still routes
        // every real host request through the native mounted overlay operation.
        guest.frame.requests.clear();
        guest.frame.busy = false;
        guest.pending.clear();
        guest.staged = false;
        guest.ticks = 1;
        // The base plane is empty. Unlike Float (which also visits its floated
        // child in base operate), this fixture exposes widgets only in overlays.
        struct OverlayOnly(Element<'static, Output>);
        struct OverlayPlane<'a> {
            content: &'a mut Element<'static, Output>,
            tree: &'a mut Tree,
        }
        impl overlay::Overlay<Output, iced::Theme, iced::Renderer> for OverlayPlane<'_> {
            fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> layout::Node {
                self.content.as_widget_mut().layout(
                    self.tree,
                    renderer,
                    &layout::Limits::new(Size::ZERO, bounds),
                )
            }
            fn draw(
                &self,
                renderer: &mut iced::Renderer,
                theme: &iced::Theme,
                style: &renderer::Style,
                layout: Layout<'_>,
                cursor: mouse::Cursor,
            ) {
                self.content.as_widget().draw(
                    self.tree,
                    renderer,
                    theme,
                    style,
                    layout,
                    cursor,
                    &layout.bounds(),
                );
            }
            fn operate(
                &mut self,
                layout: Layout<'_>,
                renderer: &iced::Renderer,
                operation: &mut dyn Operation,
            ) {
                self.content
                    .as_widget_mut()
                    .operate(self.tree, layout, renderer, operation);
            }
            fn update(
                &mut self,
                event: &Event,
                layout: Layout<'_>,
                cursor: mouse::Cursor,
                renderer: &iced::Renderer,
                clipboard: &mut dyn Clipboard,
                shell: &mut Shell<'_, Output>,
            ) {
                self.content.as_widget_mut().update(
                    self.tree,
                    event,
                    layout,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    &layout.bounds(),
                );
            }
            fn overlay<'a>(
                &'a mut self,
                layout: Layout<'a>,
                renderer: &iced::Renderer,
            ) -> Option<overlay::Element<'a, Output, iced::Theme, iced::Renderer>> {
                self.content.as_widget_mut().overlay(
                    self.tree,
                    layout,
                    renderer,
                    &layout.bounds(),
                    Vector::ZERO,
                )
            }
        }
        impl Widget<Output, iced::Theme, iced::Renderer> for OverlayOnly {
            fn tag(&self) -> tree::Tag {
                self.0.as_widget().tag()
            }
            fn state(&self) -> tree::State {
                self.0.as_widget().state()
            }
            fn children(&self) -> Vec<Tree> {
                self.0.as_widget().children()
            }
            fn diff(&self, tree: &mut Tree) {
                self.0.as_widget().diff(tree);
            }
            fn size(&self) -> Size<Length> {
                self.0.as_widget().size()
            }
            fn layout(
                &mut self,
                tree: &mut Tree,
                renderer: &iced::Renderer,
                limits: &layout::Limits,
            ) -> layout::Node {
                self.0.as_widget_mut().layout(tree, renderer, limits)
            }
            fn draw(
                &self,
                _: &Tree,
                _: &mut iced::Renderer,
                _: &iced::Theme,
                _: &renderer::Style,
                _: Layout<'_>,
                _: mouse::Cursor,
                _: &Rectangle,
            ) {
            }
            fn overlay<'a>(
                &'a mut self,
                tree: &'a mut Tree,
                _: Layout<'a>,
                _: &iced::Renderer,
                _: &Rectangle,
                _: Vector,
            ) -> Option<overlay::Element<'a, Output, iced::Theme, iced::Renderer>> {
                Some(overlay::Element::new(Box::new(OverlayPlane {
                    content: &mut self.0,
                    tree,
                })))
            }
        }
        let content = || {
            Element::new(OverlayOnly(Element::new(OverlayOnly(
                widget::text_input("", "abcd")
                    .id("popup-draft")
                    .on_input(|text| Output::Edit {
                        key: "popup-draft".into(),
                        handler: 0,
                        text,
                    })
                    .into(),
            ))))
        };
        let sibling = content();
        let module = ModuleView {
            mounted: mounted.clone(),
            generation: mounted.lock().unwrap().generation,
            rev: guest.frame_rev,
            alive: guest.alive.clone(),
            content: content(),
        };
        mounted.lock().unwrap().slot = Slot::Ready(Box::new(guest));
        use iced::advanced::renderer::Headless;
        use iced_test::runtime::{UserInterface, user_interface};
        let mut renderer = iced::futures::executor::block_on(<iced::Renderer as Headless>::new(
            iced::Font::DEFAULT,
            iced::Pixels(14.0),
            Some("tiny-skia"),
        ))
        .unwrap();
        let size = Size::new(300.0, 220.0);
        let mut ui = UserInterface::build(
            Element::new(module),
            size,
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut sibling = UserInterface::build(
            sibling,
            size,
            user_interface::Cache::default(),
            &mut renderer,
        );
        macro_rules! command {
            ($command:expr) => {{
                {
                    let mut seat = mounted.lock().unwrap();
                    let Slot::Ready(guest) = &mut seat.slot else {
                        panic!("fixture unmounted")
                    };
                    guest.answer(
                        wire::Request {
                            id: 900,
                            kind: "host.widget".into(),
                            payload: wire::encode(&$command),
                        },
                        &None,
                    );
                }
                ui.update(
                    &[Event::Window(
                        window::Event::RedrawRequested(Instant::now()),
                    )],
                    mouse::Cursor::Unavailable,
                    &mut renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut Vec::new(),
                );
                let mut seat = mounted.lock().unwrap();
                let Slot::Ready(guest) = &mut seat.slot else {
                    panic!("fixture unmounted")
                };
                let Some(wire::Event::Response {
                    id: 900,
                    result,
                    done: true,
                }) = guest.pending.pop()
                else {
                    panic!("overlay host response missing")
                };
                result.expect("mounted overlay operation")
            }};
        }
        let focus = || wire::WidgetCommand::Focused {
            target: "popup-draft".into(),
        };
        assert!(!wire::decode::<bool>(&command!(focus())).unwrap());
        command!(wire::WidgetCommand::Focus {
            target: "popup-draft".into()
        });
        assert!(
            wire::decode::<bool>(&command!(focus())).unwrap(),
            "nested popup did not receive native focus"
        );
        let other = view_tree::execute_widget_command(focus(), |operation| {
            sibling.operate(&renderer, operation)
        })
        .unwrap();
        assert!(
            !wire::decode::<bool>(&other).unwrap(),
            "the sibling popup acquired focus"
        );
        command!(wire::WidgetCommand::Select {
            target: "popup-draft".into(),
            start: 1,
            end: 3
        });
        ui.update(
            &[Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Character("X".into()),
                modified_key: iced::keyboard::Key::Character("X".into()),
                physical_key: iced::keyboard::key::Physical::Unidentified(
                    iced::keyboard::key::NativeCode::Unidentified,
                ),
                location: iced::keyboard::Location::Standard,
                modifiers: Default::default(),
                text: Some("X".into()),
                repeat: false,
            })],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut Vec::new(),
        );
        let seat = mounted.lock().unwrap();
        let Slot::Ready(guest) = &seat.slot else {
            panic!("fixture unmounted")
        };
        assert!(
            guest.pending.iter().any(|event| matches!(event,
            wire::Event::Input { text, .. } if text == "aXd")),
            "typing did not replace the selected native popup text: {:?}",
            guest.pending
        );
    }

    #[test]
    fn widget_commands_reach_native_focus_input_selection_and_scroll() {
        use iced::advanced::renderer::Headless;
        use iced_test::runtime::{UserInterface, user_interface};
        use wire::WidgetCommand as C;
        let path = staged("pages").expect("actual guest instantiation is required");
        let mut guest = Guest::load_from("pages", &path).unwrap();
        let input = |key: &str| wire::Node::Input {
            options: Default::default(),
            key: key.into(),
            placeholder: String::new(),
            value: "abcd".into(),
            on_input: 0,
            on_submit: None,
            width: None,
            secure: false,
            style: Box::default(),
        };
        // A capability fixture made from the same wire primitives guests publish.
        guest.frame.root = Some(wire::Node::Linear {
            max_width: None,
            clip: false,
            key: "fixture".into(),
            wrap: None,
            axis: wire::Axis::Column,
            spacing: None,
            padding: None,
            width: None,
            height: None,
            align: None,
            background: None,
            border: None,
            children: vec![
                input("draft"),
                input("second"),
                wire::Node::Scroll {
                    on_scroll: None,
                    virtual_rows: true,
                    key: "list".into(),
                    direction: wire::ScrollDirection::Vertical,
                    width: None,
                    height: Some(wire::Length::Fixed(100.0)),
                    bar_hidden: false,
                    bar_width: None,
                    bar_margin: None,
                    scroller_width: None,
                    bar_spacing: None,
                    anchor_x: wire::ScrollAnchor::Start,
                    anchor_y: wire::ScrollAnchor::Start,
                    auto_scroll: false,
                    background: None,
                    border: None,
                    content: Box::new(wire::Node::KeyedColumn {
                        key: "rows".into(),
                        keys: Some((0..20).map(wire::ListKey::Integer).collect()),
                        background: None,
                        border: None,
                        spacing: None,
                        padding: None,
                        width: None,
                        height: None,
                        max_width: None,
                        align: None,
                        virtual_row: Some(30.0),
                        children: (0..20)
                            .map(|_| wire::Node::Space {
                                width: None,
                                height: Some(wire::Length::Fixed(30.0)),
                            })
                            .collect(),
                    }),
                },
            ],
        });
        guest.inputs.adopt(guest.frame.root.as_ref().unwrap());
        guest.pending.clear();
        let mut renderer = iced::futures::executor::block_on(<iced::Renderer as Headless>::new(
            iced::Font::DEFAULT,
            iced::Pixels(14.0),
            Some("tiny-skia"),
        ))
        .unwrap();
        let size = Size::new(300.0, 220.0);
        let mut ui = UserInterface::build(
            guest.render(),
            size,
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut sibling = UserInterface::build(
            guest.render(),
            size,
            user_interface::Cache::default(),
            &mut renderer,
        );
        macro_rules! run {
            ($command:expr) => {{
                guest.answer(
                    wire::Request {
                        id: 900,
                        kind: "host.widget".into(),
                        payload: wire::encode(&$command),
                    },
                    &None,
                );
                guest.execute_widget_commands(|operation| ui.operate(&renderer, operation));
                let Some(wire::Event::Response {
                    id: 900,
                    result,
                    done: true,
                }) = guest.pending.pop()
                else {
                    panic!("missing host response")
                };
                result.expect("native widget command")
            }};
        }
        macro_rules! focused {
            ($target:expr) => {
                wire::decode::<bool>(&run!(C::Focused {
                    target: $target.into()
                }))
                .unwrap()
            };
        }
        assert!(!focused!("draft"));
        run!(C::Focus {
            target: "draft".into()
        });
        assert!(focused!("draft"));
        run!(C::FocusNext);
        assert!(focused!("second"));
        run!(C::FocusPrevious);
        assert!(focused!("draft"));
        let untouched = view_tree::execute_widget_command(
            C::Focused {
                target: "draft".into(),
            },
            |operation| sibling.operate(&renderer, operation),
        )
        .unwrap();
        assert!(
            !wire::decode::<bool>(&untouched).unwrap(),
            "a same-key sibling guest must not change"
        );
        for (command, expected) in [
            (
                C::CursorFront {
                    target: "draft".into(),
                },
                "Xabcd",
            ),
            (
                C::CursorEnd {
                    target: "draft".into(),
                },
                "abcdX",
            ),
            (
                C::Cursor {
                    target: "draft".into(),
                    position: 2,
                },
                "abXcd",
            ),
            (
                C::SelectAll {
                    target: "draft".into(),
                },
                "X",
            ),
            (
                C::Select {
                    target: "draft".into(),
                    start: 1,
                    end: 3,
                },
                "aXd",
            ),
        ] {
            ui = UserInterface::build(
                guest.render(),
                size,
                user_interface::Cache::default(),
                &mut renderer,
            );
            run!(C::Focus {
                target: "draft".into()
            });
            run!(command);
            let mut output = Vec::new();
            ui.update(
                &[Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Character("X".into()),
                    modified_key: iced::keyboard::Key::Character("X".into()),
                    physical_key: iced::keyboard::key::Physical::Unidentified(
                        iced::keyboard::key::NativeCode::Unidentified,
                    ),
                    location: iced::keyboard::Location::Standard,
                    modifiers: Default::default(),
                    text: Some("X".into()),
                    repeat: false,
                })],
                mouse::Cursor::Unavailable,
                &mut renderer,
                &mut iced::advanced::clipboard::Null,
                &mut output,
            );
            assert!(
                output
                    .iter()
                    .any(|output| matches!(output, Output::Edit { text, .. } if text == expected)),
                "{output:?}"
            );
        }
        struct Translation(Option<f32>);
        impl Operation for Translation {
            fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
                visit(self);
            }
            fn scrollable(
                &mut self,
                id: Option<&iced::widget::Id>,
                _: Rectangle,
                _: Rectangle,
                translation: Vector,
                _: &mut dyn iced::advanced::widget::operation::Scrollable,
            ) {
                if id == Some(&iced::widget::Id::from("list")) {
                    self.0 = Some(translation.y);
                }
            }
        }
        for (command, expected) in [
            (
                C::ScrollTo {
                    target: "list".into(),
                    x: 0.0,
                    y: 100.0,
                },
                100.0,
            ),
            (
                C::ScrollBy {
                    target: "list".into(),
                    x: 0.0,
                    y: -24.0,
                },
                76.0,
            ),
            (
                C::Snap {
                    target: "list".into(),
                    x: 0.0,
                    y: 0.5,
                },
                250.0,
            ),
            (
                C::SnapEnd {
                    target: "list".into(),
                },
                500.0,
            ),
            (
                C::ScrollToKey {
                    target: "list".into(),
                    key: 3,
                },
                90.0,
            ),
        ] {
            run!(command);
            let mut position = Translation(None);
            ui.operate(&renderer, &mut position);
            assert_eq!(position.0, Some(expected));
        }
    }

    #[test]
    fn widget_requests_validate_scope_payload_budget_and_frame() {
        let path = staged("pages").expect("actual Pages Wasm is required");
        let mut guest = Guest::load_from("pages", &path).unwrap();
        guest.redraw(&None);
        let target = guest.frame.root.as_ref().unwrap().key().unwrap().to_owned();
        let command = wire::WidgetCommand::Focus {
            target: target.clone(),
        };
        let request = |id, payload| wire::Request {
            id,
            kind: "host.widget".into(),
            payload,
        };
        guest.pending.clear();
        let mut trailing = wire::encode(&command);
        trailing.push(0);
        for payload in [
            vec![255; 4],
            trailing,
            vec![0; MAX_PAYLOAD_BYTES + 1],
            wire::encode(&wire::WidgetCommand::Focus {
                target: "OtherGuest/draft".into(),
            }),
            wire::encode(&wire::WidgetCommand::Focus {
                target: "x".repeat(wire::MAX_STRING_BYTES + 1),
            }),
            wire::encode(&wire::WidgetCommand::ScrollBy {
                target: target.clone(),
                x: f32::NAN,
                y: 0.0,
            }),
        ] {
            guest.answer(request(99, payload), &None);
            assert!(guest.widget_commands.is_empty());
            assert!(matches!(
                guest.pending.pop(),
                Some(wire::Event::Response { result: Err(_), .. })
            ));
        }
        for id in 0..MAX_REQUESTS_PER_TICK as u64 {
            guest.answer(request(id, wire::encode(&command)), &None);
        }
        assert_eq!(guest.widget_commands.len(), MAX_REQUESTS_PER_TICK);
        assert!(!guest.settled(), "native commands hold snapshot admission");
        guest.answer(request(999, wire::encode(&command)), &None);
        assert!(matches!(
            guest.pending.pop(),
            Some(wire::Event::Response { result: Err(_), .. })
        ));
        // A staged cancellation is routed before any mounted traversal.
        guest.staged = true;
        guest.frame.cancels = vec![0];
        guest.redraw(&None);
        assert!(!guest.widget_commands.iter().any(|(id, _, _)| *id == 0));
        guest.frame_rev += 1;
        guest.execute_widget_commands(|_| panic!("old-frame commands must not touch native state"));
        assert!(guest.widget_commands.is_empty());
        assert!(
            guest
                .pending
                .iter()
                .all(|event| matches!(event, wire::Event::Response { result: Err(_), .. }))
        );
    }

    #[test]
    fn pages_links_follow_actual_mounted_editor_focus() {
        let _turn = blocking_connection_turn();
        let path = staged("pages").expect("actual Pages Wasm is required");
        let mounted = fresh("pages");
        pages_document::source_changed();
        let connection = connection().lock().unwrap().rev;
        let line = "[작업 보기](duck://agents/runs/abc)";
        let original = "Title\n[작업 보기](duck://agents/runs/abc)";
        let source = pages_document::source(connection, "network-a", "alpha", original).unwrap();
        let mut facts: serde_json::Value = serde_json::from_slice(&pages_facts().unwrap()).unwrap();
        facts["document_source"] = serde_json::json!(source);
        {
            let mut seat = mounted.lock().unwrap();
            seat.props = Some(serde_json::to_vec(&facts).unwrap());
            seat.slot = Slot::Ready(Box::new(Guest::load_from("pages", &path).unwrap()));
        }
        use iced::advanced::renderer::Headless;
        use iced_test::runtime::{UserInterface, user_interface};
        let mut renderer = iced::futures::executor::block_on(<iced::Renderer as Headless>::new(
            iced::Font::DEFAULT,
            iced::Pixels(14.0),
            Some("tiny-skia"),
        ))
        .unwrap();
        let mut ui = UserInterface::build(
            drawn("pages"),
            Size::new(1100.0, 700.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut pointer = mouse::Cursor::Unavailable;
        macro_rules! frame {
            ($event:expr) => {{
                let mut output = Vec::new();
                ui.update(
                    &[$event],
                    pointer,
                    &mut renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut output,
                );
                assert!(
                    output.iter().all(|event| event.kind == "edited"),
                    "focus must not navigate: {output:?}"
                );
            }};
        }
        macro_rules! drain {
            () => {{
                loop {
                    frame!(Event::Window(
                        window::Event::RedrawRequested(Instant::now())
                    ));
                    let seat = mounted.lock().unwrap();
                    let Slot::Ready(guest) = &seat.slot else {
                        panic!("Pages unmounted")
                    };
                    assert!(guest.fault.is_none(), "{:?}", guest.fault);
                    if guest.settled()
                        && !guest.frame.busy
                        && !guest.inputs.editor_transactions_pending()
                    {
                        break;
                    }
                }
            }};
        }
        let visible = || {
            let seat = mounted.lock().unwrap();
            let Slot::Ready(guest) = &seat.slot else {
                panic!("Pages unmounted")
            };
            let mut visible = String::new();
            guest.frame.root.clone().unwrap().for_each_mut(&mut |node| {
                if let wire::Node::Editor { key, options, .. } = node {
                    assert_eq!(guest.inputs.editor_document(key).unwrap().text(), original);
                    let paint = options.presentation.as_ref().unwrap();
                    visible = paint
                        .spans
                        .iter()
                        .filter(|span| span.line == 1)
                        .filter(|span| {
                            paint.formats[span.format as usize].size.unwrap_or(14.0) > 1.0
                        })
                        .map(|span| &line[span.start as usize..span.end as usize])
                        .collect();
                }
            });
            visible
        };
        drain!();
        assert_eq!(visible(), "작업 보기");
        struct EditorBounds(Option<Rectangle>);
        impl Operation for EditorBounds {
            fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
                visit(self);
            }
            fn focusable(
                &mut self,
                id: Option<&iced::widget::Id>,
                bounds: Rectangle,
                _: &mut dyn iced::advanced::widget::operation::Focusable,
            ) {
                if id == Some(&iced::widget::Id::from("PagesView/root/pages/document")) {
                    self.0 = Some(bounds);
                }
            }
        }
        let mut bounds = EditorBounds(None);
        ui.operate(&renderer, &mut bounds);
        let bounds = bounds.0.expect("the actual Pages editor is mounted");
        for (point, expected) in [
            (iced::Point::new(bounds.x + 40.0, bounds.y + 44.0), line),
            (
                iced::Point::new(bounds.x + 40.0, bounds.y - 10.0),
                "작업 보기",
            ),
            (iced::Point::new(bounds.x + 40.0, bounds.y + 44.0), line),
        ] {
            let cursor_before = {
                let seat = mounted.lock().unwrap();
                let Slot::Ready(guest) = &seat.slot else {
                    panic!("Pages unmounted")
                };
                guest
                    .inputs
                    .editor_document("PagesView/root/pages/document")
                    .unwrap()
                    .reference()
                    .cursor
            };
            pointer = mouse::Cursor::Available(point);
            frame!(Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left
            )));
            frame!(Event::Mouse(mouse::Event::ButtonReleased(
                mouse::Button::Left
            )));
            drain!();
            assert_eq!(visible(), expected);
            let blurred = expected == "작업 보기";
            if blurred {
                let seat = mounted.lock().unwrap();
                let Slot::Ready(guest) = &seat.slot else {
                    panic!("Pages unmounted")
                };
                assert_eq!(
                    guest
                        .inputs
                        .editor_document("PagesView/root/pages/document")
                        .unwrap()
                        .reference()
                        .cursor,
                    cursor_before,
                    "concealing Markdown must not move the cursor"
                );
            }
        }
        // A newer pending projection must be consumed before an older native
        // command. Otherwise that command can steal focus before being cancelled.
        pointer = mouse::Cursor::Available(iced::Point::new(bounds.x + 40.0, bounds.y - 10.0));
        frame!(Event::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Left
        )));
        frame!(Event::Mouse(mouse::Event::ButtonReleased(
            mouse::Button::Left
        )));
        drain!();
        {
            let mut seat = mounted.lock().unwrap();
            let Slot::Ready(guest) = &mut seat.slot else {
                panic!("Pages unmounted")
            };
            guest.answer(
                wire::Request {
                    id: 9999,
                    kind: "host.widget".into(),
                    payload: wire::encode(&wire::WidgetCommand::Focus {
                        target: "PagesView/root/pages/document".into(),
                    }),
                },
                &None,
            );
            facts["active_page_title"] = "Renamed".into();
            seat.props = Some(serde_json::to_vec(&facts).unwrap());
        }
        frame!(Event::Window(
            window::Event::RedrawRequested(Instant::now())
        ));
        {
            let seat = mounted.lock().unwrap();
            let Slot::Ready(guest) = &seat.slot else {
                panic!("Pages unmounted")
            };
            assert!(
                guest.widget_commands.iter().any(|(id, _, _)| *id == 9999),
                "a command executed before the pending projection changed its frame"
            );
        }
        frame!(Event::Window(
            window::Event::RedrawRequested(Instant::now())
        ));
        {
            let seat = mounted.lock().unwrap();
            let Slot::Ready(guest) = &seat.slot else {
                panic!("Pages unmounted")
            };
            assert!(
                guest.pending.iter().any(|event| matches!(
                    event,
                    wire::Event::Response {
                        id: 9999,
                        result: Err(_),
                        ..
                    }
                )),
                "the old frame's focus request must be refused"
            );
        }
        let focused = view_tree::execute_widget_command(
            wire::WidgetCommand::Focused {
                target: "PagesView/root/pages/document".into(),
            },
            |operation| ui.operate(&renderer, operation),
        )
        .unwrap();
        assert!(
            !wire::decode::<bool>(&focused).unwrap(),
            "the stale request stole native focus"
        );
        drain!();
    }

    #[test]
    fn pages_document_source_edit_restore_rejects_the_previous_instance_intent() {
        let _turn = blocking_connection_turn();
        let path = staged("pages").expect("actual Pages Wasm is required");
        let mounted = fresh("pages");
        pages_document::source_changed();
        let connection = connection().lock().unwrap().rev;
        let paragraph =
            "A substantial paragraph keeps its complete source and ordinary body text. ".repeat(18);
        let block = format!(
            "## Heading\n- [ ] 한글 paragraph with **bold** and _emphasis_.\n  - Nested text and https://example.com/page\n```\nlet value = 42;\n```\n> Quoted paragraph\n{paragraph}\n"
        );
        let original = format!("한글 👍🏽\n{}", block.repeat(200));
        let source = pages_document::source(connection, "network-a", "alpha", &original).unwrap();
        let mut facts: serde_json::Value = serde_json::from_slice(&pages_facts().unwrap()).unwrap();
        facts["document_source"] = serde_json::json!(source);
        let props = Some(serde_json::to_vec(&facts).unwrap());
        let settle = |guest: &mut Guest, props: &Option<Vec<u8>>| {
            for _ in 0..128 {
                let busy = guest.redraw(props);
                assert!(guest.fault.is_none(), "{:?}", guest.fault);
                if !busy && guest.inputs.editor_documents_status() == Ok(true) {
                    return;
                }
            }
            panic!(
                "Pages document did not settle: frame busy={} pending={:?} staged={:?} source_pending={} documents={:?} texts={:?}",
                guest.frame.busy,
                guest.pending,
                guest.staged,
                guest.pages_document.is_some(),
                guest.inputs.editor_documents_status(),
                texts(guest)
            );
        };
        let mut guest = Guest::load_from("pages", &path).unwrap();
        settle(&mut guest, &props);
        let mut keys = Vec::new();
        guest.frame.root.clone().unwrap().for_each_mut(&mut |node| {
            if let wire::Node::Editor { key, .. } = node {
                keys.push(key.clone());
            }
        });
        assert_eq!(keys.len(), 1, "Pages has one canonical document editor");
        let editor_key = keys.pop().unwrap();
        let document = guest.inputs.editor_document(&editor_key).unwrap();
        assert_eq!(
            document.text(),
            original,
            "bounded bootstrap lost source bytes"
        );
        drop(document);
        let mut spans = 0;
        guest.frame.root.clone().unwrap().for_each_mut(&mut |node| {
            if let wire::Node::Editor { options, .. } = node {
                spans = options.presentation.as_ref().unwrap().spans.len();
            }
        });
        assert!(
            spans > 4000,
            "representative Markdown must stay richly formatted"
        );
        assert!(
            !texts(&guest)
                .iter()
                .any(|text| text.starts_with("Formatting is unavailable"))
        );
        let before_theme = guest
            .inputs
            .editor_document(&editor_key)
            .unwrap()
            .reference();
        facts["dark"] = true.into();
        facts["commented_lines"] = serde_json::json!([2]);
        facts["comment_marks"] = serde_json::json!([{"line": 2, "count": 3}]);
        let props = Some(serde_json::to_vec(&facts).unwrap());
        settle(&mut guest, &props);
        assert_eq!(
            guest
                .inputs
                .editor_document(&editor_key)
                .unwrap()
                .reference(),
            before_theme
        );
        let mut comment_badge = false;
        guest.frame.root.clone().unwrap().for_each_mut(&mut |node| {
            if let wire::Node::Editor { options, .. } = node {
                comment_badge = options
                    .presentation
                    .as_ref()
                    .unwrap()
                    .affordances
                    .margins
                    .iter()
                    .any(|mark| mark.line == 2 && mark.count == 3);
            }
        });
        assert!(
            comment_badge,
            "theme/comment props did not rebuild the prepared presentation"
        );
        guest.intents.clear();
        use iced::advanced::renderer::Headless;
        use iced_test::runtime::{UserInterface, user_interface};
        let mut renderer = iced::futures::executor::block_on(<iced::Renderer as Headless>::new(
            iced::Font::DEFAULT,
            iced::Pixels(14.0),
            Some("tiny-skia"),
        ))
        .unwrap();
        let size = iced::Size::new(1100.0, 700.0);
        let mut ui = UserInterface::build(
            guest.render(),
            size,
            user_interface::Cache::default(),
            &mut renderer,
        );
        struct EditorBounds<'a>(&'a str, Option<Rectangle>, bool);
        impl Operation for EditorBounds<'_> {
            fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
                visit(self);
            }
            fn focusable(
                &mut self,
                id: Option<&iced::widget::Id>,
                bounds: Rectangle,
                state: &mut dyn iced::advanced::widget::operation::Focusable,
            ) {
                if id == Some(&iced::widget::Id::from(self.0.to_owned())) {
                    self.1 = Some(bounds);
                    self.2 = state.is_focused();
                }
            }
        }
        let mut bounds = EditorBounds(&editor_key, None, false);
        ui.operate(&renderer, &mut bounds);
        let bounds = bounds.1.expect("Pages document has native editor bounds");
        // Click inside the first Korean/emoji line, beyond its left padding.
        let pointer = mouse::Cursor::Available(iced::Point::new(bounds.x + 88.0, bounds.y + 10.0));
        let mut outputs = Vec::new();
        ui.update(
            &[
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            ],
            pointer,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut outputs,
        );
        let mut clicked_focus = EditorBounds(&editor_key, None, false);
        ui.operate(&renderer, &mut clicked_focus);
        assert!(clicked_focus.2, "pointer did not focus the editor");
        for output in outputs {
            guest.deliver(output);
        }
        settle(&mut guest, &props);
        let cursor = guest
            .inputs
            .editor_document(&editor_key)
            .unwrap()
            .reference()
            .cursor;
        assert_eq!(
            cursor.position.line, 0,
            "pointer missed the first rendered line"
        );
        let insertion = cursor.position.column as usize;
        assert!(
            insertion > 0 && insertion <= original.find('\n').unwrap(),
            "pointer left the caret at the origin or another line"
        );
        assert!(original.is_char_boundary(insertion));
        assert!(cursor.selection.is_none());
        // A second press must replace the first selection anchor before any
        // subsequent drag or edit. Drive the native queue and actual Pages guest.
        macro_rules! pointer_step {
            ($event:expr, $point:expr) => {{
                let point = $point;
                let mut outputs = Vec::new();
                ui.update(
                    &[$event],
                    mouse::Cursor::Available(point),
                    &mut renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut outputs,
                );
                loop {
                    for output in outputs.drain(..) {
                        guest.deliver(output);
                    }
                    settle(&mut guest, &props);
                    ui = UserInterface::build(guest.render(), size, ui.into_cache(), &mut renderer);
                    if !guest.inputs.editor_transactions_pending() {
                        break;
                    }
                    ui.update(
                        &[Event::Window(
                            window::Event::RedrawRequested(Instant::now()),
                        )],
                        mouse::Cursor::Available(point),
                        &mut renderer,
                        &mut iced::advanced::clipboard::Null,
                        &mut outputs,
                    );
                }
                guest
                    .inputs
                    .editor_document(&editor_key)
                    .unwrap()
                    .reference()
                    .cursor
            }};
        }
        let a = iced::Point::new(bounds.x + 66.0, bounds.y + 10.0);
        let a_end = iced::Point::new(bounds.x + 100.0, bounds.y + 10.0);
        let b = iced::Point::new(bounds.x + 140.0, bounds.y + 10.0);
        let b_end = iced::Point::new(bounds.x + 80.0, bounds.y + 10.0);
        let first = pointer_step!(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            a
        );
        let first_drag = pointer_step!(
            Event::Mouse(mouse::Event::CursorMoved { position: a_end }),
            a_end
        );
        assert_eq!(
            first_drag.selection,
            Some(first.position),
            "first drag must establish A"
        );
        let first_released = pointer_step!(
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            a_end
        );
        let first_moved = pointer_step!(Event::Mouse(mouse::Event::CursorMoved { position: b }), b);
        assert_eq!(
            first_moved, first_released,
            "first selection followed the pointer after release"
        );
        let second = pointer_step!(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            b
        );
        assert_ne!(
            second.position, first.position,
            "second press must land away from A"
        );
        assert_eq!(
            second.selection, None,
            "new Pages press B retained first drag anchor A"
        );
        let second_drag = pointer_step!(
            Event::Mouse(mouse::Event::CursorMoved { position: b_end }),
            b_end
        );
        assert_eq!(
            second_drag.selection,
            Some(second.position),
            "second drag must anchor at B, not A"
        );
        let released = pointer_step!(
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            b_end
        );
        let moved = pointer_step!(Event::Mouse(mouse::Event::CursorMoved { position: a }), a);
        assert_eq!(
            moved, released,
            "movement after release must not change selection"
        );
        // Repeat without guest redraws between inputs: the next drag arrives
        // while the previous press/selection/release still await acknowledgment.
        for (event, point) in [
            (mouse::Event::ButtonPressed(mouse::Button::Left), a),
            (mouse::Event::CursorMoved { position: a_end }, a_end),
            (mouse::Event::ButtonReleased(mouse::Button::Left), a_end),
            (mouse::Event::ButtonPressed(mouse::Button::Left), b),
            (mouse::Event::CursorMoved { position: b_end }, b_end),
            (mouse::Event::ButtonReleased(mouse::Button::Left), b_end),
        ] {
            let mut outputs = Vec::new();
            ui.update(
                &[Event::Mouse(event)],
                mouse::Cursor::Available(point),
                &mut renderer,
                &mut iced::advanced::clipboard::Null,
                &mut outputs,
            );
            for output in outputs {
                guest.deliver(output);
            }
        }
        let burst = pointer_step!(Event::Mouse(mouse::Event::CursorMoved { position: a }), a);
        assert_eq!(
            burst, released,
            "queued second drag must use B and stop on release"
        );
        let restored = pointer_step!(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            pointer.position().unwrap()
        );
        assert_eq!(
            restored, cursor,
            "restore the original caret for the edit below"
        );
        pointer_step!(
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            pointer.position().unwrap()
        );
        let mut expected = original.clone();
        expected.insert(insertion, 'X');
        guest.intents.clear();
        // Rebuild after the click: the native caret must survive the guest echo.
        ui = UserInterface::build(guest.render(), size, ui.into_cache(), &mut renderer);
        let mut rebuilt_focus = EditorBounds(&editor_key, None, false);
        ui.operate(&renderer, &mut rebuilt_focus);
        assert!(rebuilt_focus.2, "guest echo lost native editor focus");
        let key = |value: &str, modifiers| {
            Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Character(value.into()),
                modified_key: iced::keyboard::Key::Character(value.into()),
                physical_key: iced::keyboard::key::Physical::Unidentified(
                    iced::keyboard::key::NativeCode::Unidentified,
                ),
                location: iced::keyboard::Location::Standard,
                modifiers,
                text: Some(value.into()),
                repeat: false,
            })
        };
        let mut outputs = Vec::new();
        ui.update(
            &[key("X", iced::keyboard::Modifiers::default())],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut outputs,
        );
        // Native redraws replay input queued behind the pointer transaction.
        // This is the app event loop, not a focus repair or a synthetic edit.
        for _ in 0..32 {
            for output in outputs.drain(..) {
                guest.deliver(output);
            }
            settle(&mut guest, &props);
            ui = UserInterface::build(guest.render(), size, ui.into_cache(), &mut renderer);
            if !guest.inputs.editor_transactions_pending() {
                break;
            }
            ui.update(
                &[Event::Window(
                    window::Event::RedrawRequested(Instant::now()),
                )],
                mouse::Cursor::Unavailable,
                &mut renderer,
                &mut iced::advanced::clipboard::Null,
                &mut outputs,
            );
        }
        assert_eq!(
            guest.inputs.editor_document(&editor_key).unwrap().text(),
            expected,
            "native typing did not retain the clicked caret after redraw replay"
        );
        let delayed = guest
            .intents
            .iter()
            .find(|event| event.kind == "edited")
            .unwrap()
            .clone();
        mounted.lock().unwrap().slot = Slot::Ready(Box::new(guest));
        // Hold the edit intent: the app mirror has not observed these words.
        assert_eq!(
            pages_document::source(connection, "network-a", "alpha", &expected).unwrap(),
            source,
            "autosave echo must not replace the guest document source"
        );
        facts["autosave"] = "Saved just now".into();
        let props = Some(serde_json::to_vec(&facts).unwrap());
        let (snapshot, reference) = {
            let mut locked = mounted.lock().unwrap();
            let Slot::Ready(old) = &mut locked.slot else {
                unreachable!()
            };
            settle(old, &props);
            // This source/restore test owns a real native tree separately from
            // ModuleView; drain its host operations before snapshot admission.
            assert!(!old.inputs.editor_transactions_pending());
            old.execute_widget_commands(|operation| ui.operate(&renderer, operation));
            settle(old, &props);
            assert!(old.settled(), "notification left a pending task");
            let reference = old.inputs.editor_document(&editor_key).unwrap().reference();
            (old.snapshot().unwrap(), reference)
        };
        let bytes = std::fs::read(&path).unwrap();
        let component = Guest::compile(&bytes, "Pages replacement").unwrap();
        let mut successor = Guest::instantiate("pages", &component, "Pages replacement").unwrap();
        successor.restore(&snapshot, "Pages replacement").unwrap();
        successor.first_frame("Pages replacement").unwrap();
        assert_eq!(
            successor
                .inputs
                .editor_document(&editor_key)
                .unwrap()
                .reference(),
            reference
        );
        assert_eq!(
            successor
                .inputs
                .editor_document(&editor_key)
                .unwrap()
                .text(),
            expected
        );
        {
            let mut locked = mounted.lock().unwrap();
            let Slot::Ready(old) = &locked.slot else {
                unreachable!()
            };
            successor
                .inputs
                .retain_restored_projections(&old.inputs, successor.frame.root.as_ref().unwrap())
                .unwrap();
            pages_document::retain_source(old, &mut successor);
            locked.slot = Slot::Ready(Box::new(successor));
        }
        let refused =
            pages_document::accept_page_document(delayed, "network-a".into(), "alpha".into());
        assert!(
            !refused.accepted,
            "same-reference old instance intent was admitted after restore"
        );
        assert!(refused.text.is_empty() && refused.link.is_empty());
        let mut locked = mounted.lock().unwrap();
        let Slot::Ready(restored) = &mut locked.slot else {
            unreachable!()
        };
        settle(restored, &props);
        assert_eq!(
            restored.inputs.editor_document(&editor_key).unwrap().text(),
            expected
        );
        assert!(
            restored.pages_document.is_none(),
            "completed source was retransferred after restore"
        );
        drop(locked);
        let make_app = || {
            let (mut app, _) = crate::Ducktape::__boot();
            app.connected = true;
            app.loading = false;
            app.network_chain_id = "network-a".into();
            app.active_page = "alpha".into();
            app.buffer_page = "alpha".into();
            app.page_text = original.clone();
            app.page_saved_text = original.clone();
            app.page_inflight_text = original.clone();
            app.block_autosave_status = crate::AutosaveStatus::Saving;
            app
        };
        let mut refused_app = make_app();
        let _ = refused_app.__update(crate::__DucktapeMessage::PageDocumentSaved(
            crate::backend::DocumentSaveResult {
                written: false,
                refusal: "The submitted edit was refused".into(),
                document: original.clone(),
                data: crate::backend::PagesData {
                    pages: Vec::new(),
                    blocks: Vec::new(),
                    active_page: "alpha".into(),
                    active_page_title: "한글 👍🏽".into(),
                    active_page_parent: String::new(),
                    comment_thread_total: 0,
                    commented_block_hits: Vec::new(),
                },
            },
        ));
        assert_eq!(
            refused_app.page_text, expected,
            "late refusal rolled back unobserved canonical typing"
        );
        assert_eq!(
            pages_document::source(connection, "network-a", "alpha", &expected).unwrap(),
            source,
            "refusal replaced the source despite newer canonical typing"
        );
        let mut polled_app = make_app();
        assert_eq!(polled_app.page_text, polled_app.page_saved_text);
        let recipe_ids = |app: &crate::Ducktape| {
            use iced_test::runtime::futures::subscription;
            use std::hash::Hasher as _;
            let mut ids: Vec<_> = subscription::into_recipes(app.__subscription())
                .into_iter()
                .map(|recipe| {
                    let mut hash = subscription::Hasher::default();
                    recipe.hash(&mut hash);
                    hash.finish()
                })
                .collect();
            ids.sort_unstable();
            ids
        };
        let clean_recipes = recipe_ids(&polled_app);
        polled_app.page_saved_text.push('!');
        assert_eq!(
            clean_recipes,
            recipe_ids(&polled_app),
            "clean app mirror removed the canonical reconciliation timer"
        );
        polled_app.page_saved_text.pop();
        // Exercise the real subscription: a direct handler call would hide a
        // dirty-mirror gate that never polls this preserved canonical edit.
        let tick = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                use iced_test::runtime::futures::{futures::StreamExt, subscription};
                let streams = subscription::into_recipes(polled_app.__subscription())
                    .into_iter()
                    .map(|recipe| {
                        // Test builds drive `every` from redraw time, not wall time.
                        let event = subscription::Event::Interaction {
                            window: iced::window::Id::unique(),
                            event: iced::Event::Window(iced::window::Event::RedrawRequested(
                                std::time::Instant::now() + std::time::Duration::from_secs(1),
                            )),
                            status: iced::event::Status::Ignored,
                        };
                        recipe.stream(Box::pin(
                            iced_test::runtime::futures::futures::stream::iter([event]),
                        ))
                    });
                let mut messages =
                    iced_test::runtime::futures::futures::stream::select_all(streams);
                tokio::time::timeout(std::time::Duration::from_secs(3), async {
                    while let Some(message) = messages.next().await {
                        if matches!(message, crate::__DucktapeMessage::PageAutosaveTick) {
                            return message;
                        }
                    }
                    panic!("the active page subscription ended before autosave");
                })
                .await
                .expect("clean mirror prevented the real autosave subscription from polling")
            });
        let _ = polled_app.__update(tick);
        assert_eq!(
            polled_app.page_text, expected,
            "restored edit never reached autosave without another key"
        );
        assert_eq!(
            polled_app.page_inflight_text, original,
            "the existing write was replaced"
        );
        let mut fresh = Guest::load_from("pages", &path).unwrap();
        settle(&mut fresh, &props);
        assert_eq!(
            fresh.inputs.editor_document(&editor_key).unwrap().text(),
            expected,
            "fresh guest bootstrap discarded unsaved edits"
        );
        let mut locked = mounted.lock().unwrap();
        let Slot::Ready(restored) = &mut locked.slot else {
            unreachable!()
        };
        let mut ui = UserInterface::build(restored.render(), size, ui.into_cache(), &mut renderer);
        let modifiers = if cfg!(target_os = "macos") {
            iced::keyboard::Modifiers::LOGO
        } else {
            iced::keyboard::Modifiers::CTRL
        };
        let mut outputs = Vec::new();
        ui.update(
            &[key("z", modifiers)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut outputs,
        );
        assert!(
            !outputs.is_empty(),
            "restored editor lost focus or its Undo route"
        );
        for output in outputs {
            restored.deliver(output);
        }
        settle(restored, &props);
        assert_eq!(
            restored.inputs.editor_document(&editor_key).unwrap().text(),
            original,
            "native Undo after no-init restore must preserve guest history"
        );
        drop(locked);
        pages_document::source_changed();
        pages_document::source(connection, "network-a", "alpha", "replacement source").unwrap();
        let deferred = pages_document::current_page_document(
            "network-a".into(),
            "alpha".into(),
            "new mirror".into(),
        );
        assert!(
            !deferred.ready,
            "old canonical editor was treated as a newly installed source"
        );
        assert_eq!(deferred.text, "new mirror");
    }

    fn pages_facts() -> Option<Vec<u8>> {
        Some(
            serde_json::to_vec(&serde_json::json!({
                "document_source": [], "document_error": "", "commented_lines": [], "comment_marks": [],
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
          "list_phase": "ready", "open_repo": "",
          "repo_phase": "idle", "branches": [], "tree_branch": "", "tab": "code", "items": [],
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

    pub(super) fn chat_facts() -> Option<Vec<u8>> {
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
        chat_facts_in("channel-a", messages, thread, &[])
    }

    /// The same, reading `room` and carrying the node's live agent rows — the
    /// two facts the live cards are decided by. Always the WHOLE node's rows:
    /// narrowing them to `room` is what the host is on the hook for.
    fn chat_facts_in(
        room: &'static str,
        messages: &[crate::backend::ChatMessage],
        thread: &[crate::backend::ChatMessage],
        live: &[crate::backend::LiveAgentRow],
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
            active_channel: room,
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
            live_agents: Vec::new(),
        };
        Some(encode_chat_props(props, live))
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
        wire::sanitize(&mut sanitized).expect("valid production editor document");
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
        settle_documents(&mut guest, &props);
        let mut root = guest.frame.root.clone().expect("editing tree");
        let mut editor_source = None;
        root.for_each_mut(&mut |node| {
            if let wire::Node::Editor { key, .. } = node {
                editor_source = guest
                    .inputs
                    .editor_document(key)
                    .map(|doc| doc.text().to_owned());
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

    const CHIEF_RUN: &str = "chat\u{1f}channel-a\u{1f}2\u{1f}chiefduck";

    /// One pending run of `agent`, anchored at seq 2 of `room`.
    fn live_run(room: &str, agent: &str, status: &str) -> crate::backend::LiveAgentRow {
        crate::backend::LiveAgentRow {
            channel_id: room.into(),
            anchor_seq: 2,
            run_id: CHIEF_RUN.into(),
            agent: agent.into(),
            status: status.into(),
            ..Default::default()
        }
    }

    fn anchored_pair() -> [crate::backend::ChatMessage; 2] {
        [first_light_at(1), first_light_at(2)]
    }

    fn chat_run_thread_facts(
        room: &'static str,
        messages: &[crate::backend::ChatMessage],
        thread: &[crate::backend::ChatMessage],
        live: &[crate::backend::LiveAgentRow],
    ) -> Option<Vec<u8>> {
        let bytes = chat_facts_in(room, messages, thread, live)?;
        let mut props: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        props["active_thread_seq"] = 2.into();
        Some(serde_json::to_vec(&props).unwrap())
    }

    /// Run status repaints in its thread through the real host/guest wire.
    /// Full activity and answer previews remain in the run panel.
    #[test]
    fn a_run_in_flight_draws_in_its_thread_and_repaints_as_it_works() {
        let Some(staged) = staged("chat") else {
            return;
        };
        let messages = anchored_pair();
        let starting = live_run("channel-a", "Chief Duck", "Starting");
        let mut guest = Guest::load_from("chat", &staged).expect("the view loads");
        guest.redraw(&None);
        guest.redraw(&chat_facts_in(
            "channel-a", &messages, &[], std::slice::from_ref(&starting),
        ));
        assert!(button_shown(&guest, "Chief Duck · View thread"));
        assert!(!button_shown(&guest, "Stop"));
        assert!(!texts(&guest).iter().any(|text| text == "Starting"));
        guest.redraw(&chat_run_thread_facts(
            "channel-a",
            &messages,
            &[],
            std::slice::from_ref(&starting),
        ));
        let shown = texts(&guest);
        for expected in ["Chief Duck", "AGENT", "Starting"] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} (fault {:?}) in {shown:?}",
                guest.fault
            );
        }

        let working = crate::backend::LiveAgentRow {
            status: "Reading the repo".into(),
            activity: vec![
                crate::backend::LiveActivity {
                    label: "Command: cargo test".into(),
                    done: true,
                },
                crate::backend::LiveActivity {
                    label: "Reasoning".into(),
                    done: false,
                },
            ],
            answer_preview: "the files crate builds clean".into(),
            ..starting
        };
        guest.redraw(&chat_run_thread_facts("channel-a", &messages, &[], &[working]));
        let shown = texts(&guest);
        assert!(
            shown.iter().any(|text| text == "Reading the repo"),
            "the run's status never reached the frame: {shown:?}"
        );
        for progress in [
            "Command: cargo test",
            "Reasoning",
            "the files crate builds clean",
        ] {
            assert!(
                !shown.iter().any(|text| text == progress),
                "the run's progress is the run panel's, not the stream's: {progress:?} in {shown:?}"
            );
        }
        assert!(
            button_shown(&guest, "View run"),
            "no way from the hint to the run panel (fault {:?}): {:?}",
            guest.fault,
            texts(&guest)
        );
        assert!(
            !shown.iter().any(|text| text == "Starting"),
            "the stale status is still drawn: {shown:?}"
        );
        assert!(guest.fault.is_none());
    }

    /// STOP LEAVES AS A CANCEL, AND A SETTLED RUN TAKES ITS CARD WITH IT. The
    /// row lives exactly as long as the run is pending in `runs`, so the block
    /// that posts the reply is the block that prunes it — cancelled and
    /// completed reconcile through that one path, and nothing of the run is
    /// left on the frame beside the committed message.
    #[test]
    fn stopping_a_run_leaves_as_a_cancel_and_a_settled_run_drops_its_card() {
        let Some(staged) = staged("chat") else {
            return;
        };
        let messages = anchored_pair();
        let live = live_run("channel-a", "Chief Duck", "Reading the repo");
        let facts = chat_run_thread_facts("channel-a", &messages, &[], std::slice::from_ref(&live));
        let mut guest = Guest::load_from("chat", &staged).expect("the view loads");
        guest.redraw(&None);
        guest.redraw(&facts);
        assert!(
            button_shown(&guest, "Stop"),
            "no way to stop the run (fault {:?}): {:?}",
            guest.fault,
            texts(&guest)
        );

        guest.deliver(Output::Activate(button_message(&guest, "Stop")));
        guest.redraw(&facts);
        let fired = std::mem::take(&mut guest.intents);
        let [intent] = fired.as_slice() else {
            panic!("one intent, got {fired:?}");
        };
        assert_eq!(intent.kind, "cancel_run", "{intent:?}");
        // the app's own half of the seam: the press becomes the intent the
        // handler signs, carrying the run it names. Read through the DECODER,
        // not compared to a JSON string: a run id's separator is an escape on
        // the wire, so a literal comparison pins the encoder's escaping and
        // calls it a seam.
        assert!(matches!(chat_intent(intent), crate::ChatIntent::CancelRun));
        assert_eq!(event_text(intent, "run_id"), CHIEF_RUN);

        // THE RUN SETTLED: its pending entry pruned in the block that posted
        // the reply, so the reading no longer carries it.
        let mut reply = first_light_at(3);
        reply.body = "the files crate builds clean".into();
        reply.blocks = crate::backend::paragraph_blocks(&reply.body);
        reply.author = "Chief Duck".into();
        reply.avatar_kind = "agent".into();
        let settled = [messages[0].clone(), messages[1].clone(), reply];
        guest.redraw(&chat_run_thread_facts("channel-a", &settled, &[], &[]));
        let shown = texts(&guest);
        assert!(
            !button_shown(&guest, "Stop"),
            "the settled run left its Stop behind: {shown:?}"
        );
        assert!(
            !shown.iter().any(|text| text == "Reading the repo"),
            "the settled run left its status behind: {shown:?}"
        );
        assert!(
            shown
                .iter()
                .any(|text| text == "the files crate builds clean"),
            "the committed reply did not take the card's place: {shown:?}"
        );
        assert!(guest.fault.is_none());
    }

    /// ROOM ISOLATION, DECIDED BY THE HOST. The reading covers the whole node,
    /// so the room on screen is the only thing that picks rows out of it — and
    /// it is picked at encode time, which is why no handler that moves
    /// `active_channel` has to remember this lane exists. Asserted on the
    /// PRODUCTION PROPS as well as the frame: a row the encoder kept would
    /// reach a guest that happened not to draw it today.
    #[test]
    fn the_room_on_screen_decides_which_of_the_nodes_runs_are_drawn() {
        let messages = anchored_pair();
        let reading = [
            live_run("channel-a", "Chief Duck", "Reading the repo"),
            live_run("channel-b", "Ops Duck", "Draining the queue"),
        ];

        let here = String::from_utf8(
            chat_run_thread_facts("channel-a", &messages, &[], &reading).expect("props encode"),
        )
        .unwrap();
        assert!(here.contains("Chief Duck"), "this room's run is missing");
        assert!(
            !here.contains("Ops Duck"),
            "another room's run crossed to the view: {here}"
        );

        let there = String::from_utf8(
            chat_run_thread_facts("channel-b", &messages, &[], &reading).expect("props encode"),
        )
        .unwrap();
        assert!(there.contains("Ops Duck"), "that room's run is missing");
        assert!(
            !there.contains("Chief Duck"),
            "the room she left kept its run on the frame: {there}"
        );

        let Some(staged) = staged("chat") else {
            return;
        };
        let mut guest = Guest::load_from("chat", &staged).expect("the view loads");
        guest.redraw(&None);
        guest.redraw(&chat_run_thread_facts("channel-a", &messages, &[], &reading));
        let shown = texts(&guest);
        assert!(
            shown.iter().any(|text| text == "Reading the repo"),
            "(fault {:?}) {shown:?}",
            guest.fault
        );
        assert!(
            !shown.iter().any(|text| text == "Draining the queue"),
            "{shown:?}"
        );
        // the same reading, the other room on screen
        guest.redraw(&chat_run_thread_facts("channel-b", &messages, &[], &reading));
        let shown = texts(&guest);
        assert!(
            shown.iter().any(|text| text == "Draining the queue"),
            "{shown:?}"
        );
        assert!(
            !shown.iter().any(|text| text == "Reading the repo"),
            "the room she left kept its card: {shown:?}"
        );
        assert!(guest.fault.is_none());
    }

    /// A room full of runs cannot blank the messages they sit under: the cards
    /// spend [`LIVE_AGENT_TEXT_BUDGET`] and the newest anchors are the ones
    /// kept, because those are the ones at the tail she is looking at.
    #[test]
    fn the_live_cards_are_held_to_their_slice_of_the_frame_budget() {
        let crowd: Vec<_> = (1..=60)
            .map(|seq| crate::backend::LiveAgentRow {
                channel_id: "channel-a".into(),
                anchor_seq: seq,
                run_id: format!("run-{seq}"),
                agent: format!("agent-{seq}"),
                status: "x".repeat(400),
                ..Default::default()
            })
            .collect();
        let kept = live_agents_within(&crowd, "channel-a", LIVE_AGENT_TEXT_BUDGET);
        let spent: usize = kept.iter().map(live_text_bytes).sum();
        assert!(
            spent <= LIVE_AGENT_TEXT_BUDGET,
            "{spent} bytes past the {LIVE_AGENT_TEXT_BUDGET} byte ceiling"
        );
        assert!(kept.len() < crowd.len(), "nothing was held back");
        assert_eq!(
            kept.last().map(|row| row.anchor_seq),
            Some(60),
            "the newest anchor is the one that must survive"
        );
        assert!(
            kept.windows(2)
                .all(|pair| pair[0].anchor_seq < pair[1].anchor_seq),
            "the kept rows are handed back in anchor order"
        );
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
            connected(&client).joined();
            assert_eq!(slot_assets(&mounted), ["a.svg"], "{module}");
            let (generation, frame_rev) = {
                let mut locked = mounted.lock().expect("module view lock");
                let generation = locked.generation;
                let Slot::Ready(guest) = &mut locked.slot else {
                    panic!("{module}: the view of A");
                };
                // the view draws, takes facts that are not its initial
                // state, and settles — its editor's document included
                settle_documents(guest, &props);
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
            deployments_checked().await.joined();
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
            deployments_checked().await.joined();
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

    /// A valid view with an admitted event it has not consumed yet.
    fn drawn_then_stuck(mounted: &Arc<Mutex<Mounted>>) -> u64 {
        let mut locked = mounted.lock().expect("module view lock");
        let generation = locked.generation;
        let Slot::Ready(guest) = &mut locked.slot else {
            panic!("the view of A");
        };
        assert!((0..4).any(|_| !guest.redraw(&register())));
        assert!(guest.settled(), "{:?}", guest.fault);
        assert!(guest.ever_valid_tree);
        guest.pending.push(wire::Event::Resync);
        assert!(!guest.settled());
        generation
    }

    /// The host boundary after a tick returned no valid frame. The actual
    /// staged instance has not published a tree; its next redraw can recover.
    fn never_valid_pending(mounted: &Arc<Mutex<Mounted>>) -> u64 {
        let mut locked = mounted.lock().expect("module view lock");
        let generation = locked.generation;
        let Slot::Ready(guest) = &mut locked.slot else {
            panic!("the view of A");
        };
        assert_eq!(guest.ticks, 0);
        assert!(!guest.ever_valid_tree);
        assert!(guest.frame.root.is_none());
        guest.ticks = 1;
        guest.pending.push(wire::Event::Resync);
        generation
    }

    /// A block that finds the seated view busy waits under the generation
    /// it has: one generation per load, not one per block.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_block_that_waits_on_pending_work_keeps_the_generation() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let staged =
            staged("governance").expect("staged governance is required for recovery evidence");
        let component = std::fs::read(staged).expect("the staged view");
        let (a, b) = (
            deployment(&component, "a.svg"),
            deployment(&component, "b.svg"),
        );
        let node = FakeDeployment::serving("governance", &a);
        let client = fake_node(node.clone()).await;
        let mounted = fresh("governance");
        connected(&client).joined();
        let generation = drawn_then_stuck(&mounted);
        node.deploy("governance", &b);
        for _ in 0..3 {
            deployments_checked().await.joined();
            let locked = mounted.lock().expect("module view lock");
            assert_eq!(locked.generation, generation, "no load opened");
            assert_eq!(locked.hash, Some(a.hash()));
            assert!(locked.waiting_since.is_some(), "the wait is counted");
        }
        assert_eq!(slot_assets(&mounted), ["a.svg"]);
        registry()
            .lock()
            .expect("module views")
            .remove("governance");
    }

    /// A view that never drew a usable tree and keeps work pending past
    /// the wait is replaced without its state; the candidate still proves
    /// its first tree.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_view_without_a_tree_is_replaced_once_its_pending_work_outlives_the_wait() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let staged =
            staged("governance").expect("staged governance is required for recovery evidence");
        let component = std::fs::read(staged).expect("the staged view");
        let (a, b) = (
            deployment(&component, "a.svg"),
            deployment(&component, "b.svg"),
        );
        let node = FakeDeployment::serving("governance", &a);
        let client = fake_node(node.clone()).await;
        let mounted = fresh("governance");
        connected(&client).joined();
        let generation = never_valid_pending(&mounted);
        node.deploy("governance", &b);
        // inside the wait: nothing moves
        deployments_checked().await.joined();
        assert_eq!(slot_assets(&mounted), ["a.svg"]);
        // the wait is over
        mounted.lock().expect("module view lock").waiting_since =
            Some(Instant::now() - REPLACEMENT_WAIT);
        deployments_checked().await.joined();
        assert_eq!(
            slot_assets(&mounted),
            ["b.svg"],
            "B replaced the stuck view"
        );
        let mut locked = mounted.lock().expect("module view lock");
        assert_eq!(locked.hash, Some(b.hash()));
        assert_eq!(
            locked.generation,
            generation + 1,
            "one load, one generation"
        );
        assert!(locked.waiting_since.is_none());
        let Slot::Ready(guest) = &mut locked.slot else {
            panic!("the view of B");
        };
        assert!(guest.frame.root.is_some(), "B proved its first tree");
        assert!(guest.settled(), "{:?}", guest.fault);
        drop(locked);
        registry()
            .lock()
            .expect("module views")
            .remove("governance");
    }

    /// A view that has drawn a tree is never replaced under it: its pending
    /// work may be a reader's, and it waits however long that takes.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_view_that_drew_a_tree_keeps_waiting_past_the_wait() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let staged =
            staged("governance").expect("staged governance is required for recovery evidence");
        let component = std::fs::read(staged).expect("the staged view");
        let (a, b) = (
            deployment(&component, "a.svg"),
            deployment(&component, "b.svg"),
        );
        let node = FakeDeployment::serving("governance", &a);
        let client = fake_node(node.clone()).await;
        let mounted = fresh("governance");
        connected(&client).joined();
        let generation = drawn_then_stuck(&mounted);
        node.deploy("governance", &b);
        deployments_checked().await.joined();
        mounted.lock().expect("module view lock").waiting_since =
            Some(Instant::now() - REPLACEMENT_WAIT * 4);
        deployments_checked().await.joined();
        assert_eq!(slot_assets(&mounted), ["a.svg"], "the drawn view stays");
        let locked = mounted.lock().expect("module view lock");
        assert_eq!(
            (locked.generation, locked.hash),
            (generation, Some(a.hash()))
        );
        drop(locked);
        registry()
            .lock()
            .expect("module views")
            .remove("governance");
    }

    /// A valid view can temporarily lose its rendered tree during resync,
    /// or trap after an edit. Neither state authorizes throwing its data away.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_once_valid_view_keeps_its_state_after_a_patch_gap_or_terminal_fault() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let staged =
            staged("governance").expect("staged governance is required for recovery evidence");
        let component = std::fs::read(staged).expect("the staged view");
        let a = deployment(&component, "a.svg");
        let b = deployment(&component, "b.svg");
        let node = FakeDeployment::serving("governance", &a);
        let client = fake_node(node.clone()).await;
        let mounted = fresh("governance");
        connected(&client).joined();
        let generation = drawn_then_stuck(&mounted);
        node.deploy("governance", &b);
        {
            let mut locked = mounted.lock().unwrap();
            let Slot::Ready(guest) = &mut locked.slot else {
                panic!("A")
            };
            guest.frame.root = None;
            locked.waiting_since = Some(Instant::now() - REPLACEMENT_WAIT * 4);
        }
        deployments_checked().await.joined();
        assert_eq!(
            slot_assets(&mounted),
            ["a.svg"],
            "a temporary patch gap must preserve A"
        );
        {
            let mut locked = mounted.lock().unwrap();
            let Slot::Ready(guest) = &mut locked.slot else {
                panic!("A")
            };
            guest.fault = Some("terminal trap after unsaved edits (test)".into());
        }
        deployments_checked().await.joined();
        assert_eq!(
            slot_assets(&mounted),
            ["a.svg"],
            "a later trap must preserve authored state"
        );
        assert_eq!(mounted.lock().unwrap().generation, generation);
        registry().lock().unwrap().remove("governance");
    }

    /// Once A starts serving a real tree, an already prepared destructive
    /// recovery must be rejected at the final seat, even without a new block.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_never_valid_view_that_recovers_during_candidate_loading_is_not_discarded() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        let staged =
            staged("governance").expect("staged governance is required for recovery evidence");
        let component = std::fs::read(staged).expect("the staged view");
        let a = deployment(&component, "a.svg");
        let b = deployment(&component, "b.svg");
        let node = FakeDeployment::serving("governance", &a);
        let client = fake_node(node.clone()).await;
        let mounted = fresh("governance");
        connected(&client).joined();
        let generation = never_valid_pending(&mounted);
        mounted.lock().unwrap().waiting_since = Some(Instant::now() - REPLACEMENT_WAIT);
        node.deploy("governance", &b);
        let hold = hold_blob(&node);
        let loads = deployments_checked().await;
        node.held.notified().await;
        drawn_then_stuck(&mounted);
        hold.notify_one();
        loads.joined();
        assert_eq!(
            slot_assets(&mounted),
            ["a.svg"],
            "healthy A must not be reset by stale recovery"
        );
        let locked = mounted.lock().unwrap();
        assert_eq!(locked.generation, generation + 1);
        assert!(!locked.in_flight);
        let Slot::Ready(guest) = &locked.slot else {
            panic!("A")
        };
        assert!(guest.ever_valid_tree);
        assert!(
            !guest.pending.is_empty(),
            "the admitted work remains owned by A"
        );
        drop(locked);
        registry().lock().unwrap().remove("governance");
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
        connected(&client).joined();
        assert_eq!(slot_assets(&mounted), ["a.svg"]);

        // B activates, but its bytes are slow — and C activates meanwhile
        node.deploy("forge", &b);
        let hold = hold_blob(&node);
        let loads = deployments_checked().await;
        node.held.notified().await;
        node.deploy("forge", &c);
        hold.notify_one();
        loads.joined();
        assert_eq!(slot_assets(&mounted), ["a.svg"], "B is not installed");
        assert_eq!(mounted.lock().unwrap().hash, Some(a.hash()));
        // the next block's check brings C
        deployments_checked().await.joined();
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
        connected(&client).joined();
        assert_eq!(slot_assets(&mounted), ["a.svg"]);
        node.deploy("forge", &removed);
        deployments_checked().await.joined();
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
        connected(&client).joined();
        assert_eq!(slot_assets(&mounted), ["a.svg"]);

        let before = mounted.lock().unwrap().generation;
        node.deploy("forge", &b);
        let hold = hold_blob(&node);
        let loads = deployments_checked().await;
        node.held.notified().await;
        let generation = mounted.lock().unwrap().generation;
        assert_eq!(generation, before + 1);
        // three more blocks while B's bytes are held: the status still
        // answers, and none of them starts this view over (the views of
        // earlier tests, unknown to this node, get their own loads)
        let mut later_blocks = Vec::new();
        for _ in 0..3 {
            later_blocks.push(deployments_checked().await);
            let locked = mounted.lock().unwrap();
            assert_eq!((locked.generation, locked.in_flight), (generation, true));
        }
        hold.notify_one();
        loads.joined();
        for loads in later_blocks {
            loads.joined();
        }
        assert_eq!(slot_assets(&mounted), ["b.svg"]);
        let locked = mounted.lock().unwrap();
        assert_eq!((locked.generation, locked.in_flight), (generation, false));
    }

    /// Blocks keep landing on a deployment whose view cannot load: it is
    /// tried again, then held off with a widening gap, instead of paying a
    /// cranelift compile once a block for a candidate that never lands.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_failed_deployment_is_not_retried_by_every_block() {
        let _turn = connection_turn().await;
        use crate::backend::view_source::tests::{FakeDeployment, fake_node};
        // bytes that are not a component: every load of them fails alike
        let bad = deployment(b"not a component", "a.svg");
        let node = FakeDeployment::serving("forge", &bad);
        let client = fake_node(node.clone()).await;
        let mounted = fresh("forge");
        connected(&client).joined();
        {
            let locked = mounted.lock().unwrap();
            assert!(
                matches!(locked.slot, Slot::Failed(_)),
                "{}",
                slot_name(&locked.slot)
            );
        }

        // the first block names the candidate the reconnect did not: it is
        // tried once under its own hash, and every block after it is held
        deployments_checked().await.joined();
        let generation = mounted.lock().unwrap().generation;
        for _ in 0..5 {
            deployments_checked().await.joined();
        }
        {
            let locked = mounted.lock().unwrap();
            assert_eq!(
                locked.generation, generation,
                "a failed candidate was loaded again on every block"
            );
            assert!(
                matches!(locked.slot, Slot::Failed(_)),
                "the failure is still what the tab shows: {}",
                slot_name(&locked.slot)
            );
        }

        // nothing is suppressed for good: once the gap is up the same
        // candidate is tried again, so a load that failed on the transport
        // still recovers on its own
        tokio::time::sleep(RETRY_FIRST + Duration::from_millis(100)).await;
        deployments_checked().await.joined();
        assert_eq!(
            mounted.lock().unwrap().generation,
            generation + 1,
            "the hold-off never expired"
        );
        registry().lock().unwrap().remove("forge");
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
        connected(&client).joined();
        assert_eq!(slot_assets(&mounted), ["a.svg"]);

        node.deploy("governance", &removed);
        let hold = hold_blob(&node);
        let loads = deployments_checked().await;
        node.held.notified().await;
        node.deploy("governance", &c);
        hold.notify_one();
        loads.joined();
        assert_eq!(slot_assets(&mounted), ["a.svg"], "A is still drawn");
        assert_eq!(mounted.lock().unwrap().hash, Some(a.hash()));
        deployments_checked().await.joined();
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
        connected(&client).joined();
        node.deploy("governance", &b);
        deployments_checked().await.joined();

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
        connected(&client).joined();
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
        loads.joined();
        let ticks = {
            let locked = mounted.lock().unwrap();
            assert_eq!(locked.hash, Some(a.hash()), "the candidate was refused");
            let Slot::Ready(guest) = &locked.slot else {
                panic!("the view of A");
            };
            guest.ticks
        };
        assert!(ticks > 0);
        deployments_checked().await.joined();
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
        connected(&client).joined();
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
        loads.joined();
        assert!(!FIRST_FRAME_TRAPS.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(slot_assets(&mounted), ["a.svg"], "A stays");
        {
            let locked = mounted.lock().unwrap();
            assert_eq!((locked.hash, locked.in_flight), (Some(a.hash()), false));
        }
        // a first frame that trapped is a property of those bytes, so the
        // very next block does not pay for the same candidate again
        deployments_checked().await.joined();
        assert_eq!(
            slot_assets(&mounted),
            ["a.svg"],
            "B is held off, not retried at once"
        );
        // once its gap is up the candidate is tried again, and this time
        // its first tree draws
        {
            let mut locked = mounted.lock().unwrap();
            let retry = locked.retry.as_mut().expect("B left a hold-off");
            retry.next = Instant::now();
        }
        deployments_checked().await.joined();
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
              "list_phase": "ready", "open_repo": "core",
              "repo_phase": "ready", "branches": [{"name": "main", "head": "1111"}], "tree_branch": "main", "tab": "issues", "items": [],
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
        assert_eq!(
            merge(&mut held, &mut unchanged),
            Ok((false, Default::default()))
        );
        assert_eq!(unchanged.root, Some(held_tree));
        let mut patched = wire::Frame::default();
        assert_eq!(merge(&mut None, &mut patched), Err("no tree to patch"));
    }
}
