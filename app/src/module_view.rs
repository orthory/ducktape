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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
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
) -> Element<'static, ModuleViewEvent> {
    let props = serde_json::json!({
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
    module_view("files", serde_json::to_vec(&props).expect("props encode"))
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

/// The operations a view may ask of the app, by module. An intent outside
/// the list is refused at the door, never handed to a handler.
fn intents_of(module: &str) -> &'static [&'static str] {
    match module {
        "governance" => &["vote", "execute"],
        "members" => &["copy", "agent_status", "propose"],
        "agents" => &[],
        "node" => &["copy", "tab", "log_filter"],
        "explorer" => &["refresh", "copy", "search", "clear"],
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
            "forget",
            "light",
            "dark",
            "notifications",
        ],
        _ => &[],
    }
}

// ---------- mounting ----------

/// The widget for one module's view, with `props` as the app has them now.
/// The view is loaded on its own thread, and the tab shows what stage it is
/// at until then. A load that failed is tried again once the app points at
/// another node ([`connected`]); a view that loaded stays for the process
/// (the swap-in-place on a new deployment is the next step, behind the
/// `generation` this registry already keeps).
fn module_view(module: &'static str, props: Vec<u8>) -> Element<'static, ModuleViewEvent> {
    let mounted = mounted(module);
    let (content, rev, generation) = {
        let mut locked = mounted.lock().expect("module view lock");
        locked.props = Some(props);
        if matches!(locked.slot, Slot::Failed(_)) && locked.client_rev != client_rev() {
            locked.slot = Slot::Loading;
            locked.generation += 1;
            spawn_load(module, &mounted, &mut locked);
        }
        match &mut locked.slot {
            Slot::Loading => return notice("Loading the view…"),
            Slot::Failed(reason) => return notice(reason),
            Slot::Ready(guest) => (guest.render(), guest.frame_rev, locked.generation),
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
/// deployments. Told by `backend::connect`; a change retries every module
/// view that failed to load off the previous one.
pub fn connected(client: &ducktape_rpc::Client) {
    *rpc_client().lock().expect("views rpc") = Some(client.clone());
    CLIENT_REV.fetch_add(1, Ordering::SeqCst);
}

static CLIENT_REV: AtomicU64 = AtomicU64::new(0);

fn client_rev() -> u64 {
    CLIENT_REV.load(Ordering::SeqCst)
}

fn rpc_client() -> &'static Mutex<Option<ducktape_rpc::Client>> {
    static CLIENT: OnceLock<Mutex<Option<ducktape_rpc::Client>>> = OnceLock::new();
    CLIENT.get_or_init(Mutex::default)
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
    /// The [`client_rev`] the last load was asked under.
    client_rev: u64,
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
                generation: 0,
                client_rev: 0,
            }));
            spawn_load(
                module,
                &mounted,
                &mut mounted.lock().expect("module view lock"),
            );
            mounted
        })
        .clone()
}

/// Loads the view on its own thread — a cold cranelift compile is a second
/// or more; the window thread shows "Loading" instead of freezing for it —
/// and installs it only if `mounted` still waits for this very load.
fn spawn_load(module: &'static str, mounted: &Arc<Mutex<Mounted>>, locked: &mut Mounted) {
    let generation = locked.generation;
    locked.client_rev = client_rev();
    let client = rpc_client().lock().expect("views rpc").clone();
    let loading = mounted.clone();
    std::thread::spawn(move || {
        let slot = match Guest::load(module, client.as_ref(), generation) {
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
        let mut locked = loading.lock().expect("module view lock");
        if locked.generation == generation {
            locked.slot = slot;
        }
    });
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
    /// The assets the deployment shipped beside this view, for the host
    /// surfaces that paint them by canonical relative path; swapped with the
    /// instance as one unit. Empty for a staged desktop view.
    #[allow(dead_code, reason = "the artifact surfaces are the next step")]
    assets: Arc<crate::backend::view_source::Assets>,
}

fn hex_short(hash: &[u8; 32]) -> String {
    hash[..6].iter().map(|byte| format!("{byte:02x}")).collect()
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
    /// A module-owned view comes from the module's active deployment on the
    /// connected node — nothing else, so with no node there is nothing to
    /// load yet, and no staged file is ever opened for it; the desktop's own
    /// views come from the staged file. Every outcome for a module-owned view
    /// is one `view_source` log line with stable fields.
    fn load(
        module: &'static str,
        client: Option<&ducktape_rpc::Client>,
        generation: u64,
    ) -> Result<Self, String> {
        use crate::backend::view_source::{self, ViewSource};
        if !view_source::module_owned(module) {
            let path = views_dir()?.join(format!("{module}_view.wasm"));
            return Self::load_from(module, &path);
        }
        let logged = |hash: Option<&[u8; 32]>, state: &str, reason: &str| {
            tracing::info!(
                target: "ducktape::app",
                module,
                hash = %hash.map_or_else(|| "-".to_owned(), |hash| crate::backend::hex_encode(hash)),
                state,
                gen = generation,
                reason = %if reason.is_empty() { "-" } else { reason },
                "view_source"
            );
        };
        let source = client
            .ok_or_else(|| "not connected to a node yet".to_owned())
            .and_then(|client| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| error.to_string())?;
                runtime
                    .block_on(view_source::resolve(client, module))
                    .map_err(|error| error.to_string())
            });
        let source = match source {
            Ok(source) => source,
            Err(reason) => {
                logged(None, "Failed", &reason);
                return Err(reason);
            }
        };
        match source {
            ViewSource::NotActivated => {
                logged(None, "NotActivated", "");
                Err(format!("the {module} module is not activated yet"))
            }
            ViewSource::Missing { hash } => {
                logged(Some(&hash), "Missing", "");
                Err(format!("the {module} module's deployment ships no view"))
            }
            ViewSource::Ready {
                hash,
                component,
                assets,
            } => {
                let shown = format!("{module} view @ {}", hex_short(&hash));
                match Self::from_bytes(module, &component, &shown) {
                    Ok(mut guest) => {
                        guest.assets = assets;
                        logged(Some(&hash), "Ready", "");
                        Ok(guest)
                    }
                    Err(reason) => {
                        logged(Some(&hash), "Failed", &reason);
                        Err(reason)
                    }
                }
            }
        }
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
        if bytes.len() as u64 > MAX_MODULE_BYTES {
            return Err(format!(
                "{shown}: past the {MAX_MODULE_BYTES} byte module limit"
            ));
        }
        let engine = engine();
        let component =
            Component::new(engine, bytes).map_err(|error| format!("{shown}: {error}"))?;
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
            assets: Arc::default(),
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
        if let Output::Surface { handler: None, .. } = output {
            self.intents.push(ModuleViewEvent {
                kind: "log_timeline".into(),
                detail: String::new(),
            });
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
            Slot::Failed(_) => return,
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
        assert!(intents_of("chat").is_empty());
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
        let props = Some(
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
                "preview_path": "/shared/README.md",
                "preview_entry": {"key": 2, "path": "/shared/README.md", "name": "README.md", "kind": "file", "size": 1024, "object": "bb"},
                "delete_target": "", "diff_from": "", "diff": [], "history": [],
                "preview_truncated": false, "preview_binary": false, "preview_picture": false,
                "preview_width": 0, "preview_height": 0,
                "preview_text": "# Hello\n\n[a link](https://duck.example/x)\n",
                "dark": false, "write_refusal": "", "writes": 0
            }))
            .expect("props encode"),
        );
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
            assert_eq!(
                Guest::load(module, None, 0).err().as_deref(),
                Some("not connected to a node yet"),
                "{module}"
            );
        }
        assert!(!crate::backend::view_source::module_owned("settings"));
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
