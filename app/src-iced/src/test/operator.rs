//! Operator screens: the Ready-arm layout for the unified node-section chrome.
//! The presets boot without a node, so these render the Ready views directly
//! with synthetic snapshots — the only place the pinned header, compact
//! Start/Stop, two-column permissions, and sandbox picker/badge appear.

use super::harness::*;
use crate::screens::operator::{
    self, CheckState, Command, GatewayData, GatewayMessage, GatewayRoute, LogLine, Message,
    MetricsSnapshot, ModuleCategory, ModuleRoot, NodeMessage, NodeRole, NodeSnapshot, NodeTab,
    Resource, RouteTarget, SandboxCheck, SandboxData, SandboxMessage, SandboxMode, Screen, State,
};
use crate::theme::Mode;

fn node_ready(connected: bool, managed: bool) -> State {
    let mut state = State::default();
    state.node.data = Resource::Ready(NodeSnapshot {
        connected,
        managed,
        workspace_name: "QA Local".into(),
        role: NodeRole::GenesisValidator,
        peer: "aa".repeat(16),
        version: "0.1.0".into(),
        height: 128,
        app_hash: "bb".repeat(16),
        modules: Vec::new(),
        validator_count: 3,
        connections: Vec::new(),
        logs: Vec::new(),
        blocks_per_second: Some(1.5),
        apply_p95_ms: Some(4.2),
    });
    state
}

#[test]
fn node_header_renders_and_running_managed_node_offers_stop() {
    let state = node_ready(true, true);
    let mut ui = sim(operator::view(&state, Screen::Node, Mode::Light));
    assert!(ui.find("Node").is_ok(), "the unified section header renders");
    assert!(
        has(&mut ui, Role::Button, "Stop"),
        "a running managed node offers a compact Stop"
    );
    assert!(!has(&mut ui, Role::Button, "Start"));
}

#[test]
fn stopped_managed_node_offers_start() {
    let state = node_ready(false, true);
    let mut ui = sim(operator::view(&state, Screen::Node, Mode::Light));
    assert!(has(&mut ui, Role::Button, "Start"));
}

#[test]
fn remote_node_hides_daemon_controls() {
    let state = node_ready(true, false);
    let mut ui = sim(operator::view(&state, Screen::Node, Mode::Light));
    assert!(!has(&mut ui, Role::Button, "Start"));
    assert!(!has(&mut ui, Role::Button, "Stop"));
}

#[test]
fn start_marks_busy_and_relabels_the_trigger() {
    let mut state = node_ready(false, true);
    let command = operator::update(&mut state, Message::Node(NodeMessage::Start));
    assert_eq!(command, Some(Command::StartNode));
    assert!(state.node.busy, "Start busies the trigger to block a double-submit");
    let mut ui = sim(operator::view(&state, Screen::Node, Mode::Light));
    assert!(
        has(&mut ui, Role::Button, "Starting…"),
        "the busy trigger relabels while the boot is in flight"
    );
    assert!(!has(&mut ui, Role::Button, "Start"));
}

#[test]
fn permissions_tab_shows_both_role_columns() {
    let mut state = node_ready(true, true);
    state.node.active_tab = NodeTab::Permissions;
    let mut ui = sim(operator::view(&state, Screen::Node, Mode::Light));
    assert!(ui.find("VALIDATOR").is_ok());
    assert!(
        ui.find("GUEST / REMOTE").is_ok(),
        "both role columns render, not just the node's own"
    );
}

fn sandbox_ready() -> State {
    let mut state = State::default();
    state.sandbox.data = Resource::Ready(SandboxData {
        can_control: true,
        backend: "podman".into(),
        os: "linux".into(),
        current_mode: SandboxMode::Off,
        available_modes: vec![SandboxMode::Off, SandboxMode::Podman],
        serving: false,
        checks: vec![SandboxCheck {
            id: "runtime".into(),
            label: "Container runtime".into(),
            detail: "podman not found".into(),
            state: CheckState::Failed,
            fixable: true,
        }],
        active_agents: vec![("a1".into(), "Ada".into()), ("a2".into(), "Bo".into())],
        active_channel: Some("general".into()),
    });
    state
}

#[test]
fn sandbox_badges_the_current_mode() {
    let state = sandbox_ready();
    let mut ui = sim(operator::view(&state, Screen::Sandbox, Mode::Light));
    assert!(
        ui.find("CURRENT").is_ok(),
        "the active mode carries a CURRENT badge, not just a disabled button"
    );
}

#[test]
fn sandbox_offers_an_agent_picker_with_multiple_agents() {
    let state = sandbox_ready();
    let mut ui = sim(operator::view(&state, Screen::Sandbox, Mode::Light));
    assert!(has(&mut ui, Role::Button, "Ada"));
    assert!(
        has(&mut ui, Role::Button, "Bo"),
        "the operator picks which active agent runs the setup"
    );
}

#[test]
fn sandbox_applied_flash_survives_the_reload() {
    let mut state = sandbox_ready();
    state.sandbox.applied = true;
    let mut ui = sim(operator::view(&state, Screen::Sandbox, Mode::Light));
    assert!(
        ui.find("Applied. The node restarted with the selected mode.")
            .is_ok(),
        "the success beat is shown after apply, not snapped back to the idle prompt"
    );
}

// --- Node: resolved-state render variants + tab / copy / filter affordances ---

#[test]
fn node_empty_offers_a_reload() {
    let mut state = State::default();
    state.node.data = Resource::Empty;
    let mut ui = sim(operator::view(&state, Screen::Node, Mode::Light));
    assert!(ui.find("No node connected").is_ok());
    ui.click(by::role(Role::Button, "Reload"))
        .expect("the empty (resolved) node state carries a reload CTA");
    assert!(emitted(ui, &Message::Load(Screen::Node)));
}

// Error surfacing + retry: the class of gap that hid the pages regression.
#[test]
fn node_error_surfaces_detail_and_retries() {
    let mut state = State::default();
    state.node.data = Resource::Error("daemon socket refused".into());
    let mut ui = sim(operator::view(&state, Screen::Node, Mode::Light));
    assert!(
        ui.find("daemon socket refused").is_ok(),
        "the node error is shown selectable so it can be copied into a report"
    );
    ui.click(by::role(Role::Button, "Retry"))
        .expect("an errored node offers retry");
    assert!(emitted(ui, &Message::Load(Screen::Node)));
}

#[test]
fn node_tab_selects_on_click() {
    let state = node_ready(true, true);
    let mut ui = sim(operator::view(&state, Screen::Node, Mode::Light));
    ui.click(by::role(Role::Tab, "Logs"))
        .expect("the pinned tab strip switches tabs");
    assert!(emitted(
        ui,
        &Message::Node(NodeMessage::SelectTab(NodeTab::Logs))
    ));
}

#[test]
fn node_copy_button_emits_the_value() {
    let state = node_ready(true, true);
    let mut ui = sim(operator::view(&state, Screen::Node, Mode::Light));
    ui.click(by::role(Role::Button, "APP HASH"))
        .expect("the app-hash row is a copy affordance");
    assert!(emitted(
        ui,
        &Message::Node(NodeMessage::Copy {
            key: "app-hash".into(),
            value: "bb".repeat(16),
        })
    ));
}

#[test]
fn node_log_filter_hides_nonmatching_lines() {
    let mut state = node_ready(true, true);
    if let Resource::Ready(snapshot) = &mut state.node.data {
        snapshot.logs = vec![LogLine {
            timestamp: "12:00".into(),
            level: "info".into(),
            target: "ducktape::join".into(),
            message: "commit applied".into(),
        }];
    }
    state.node.active_tab = NodeTab::Logs;
    state.node.log_filter = "zzz".into();
    let mut ui = sim(operator::view(&state, Screen::Node, Mode::Light));
    assert!(ui.find("No log lines match this filter.").is_ok());
    assert!(
        ui.find("commit applied").is_err(),
        "a non-matching line is filtered out of the render, not merely dimmed"
    );
}

// --- Gateway: published-route rendering + selection emission ---

fn gateway_ready() -> State {
    let mut state = State::default();
    state.gateway.data = Resource::Ready(GatewayData {
        routes: vec![GatewayRoute {
            key: "rk1".into(),
            label: "api".into(),
            address: "api.acct.duck".into(),
            target: RouteTarget::DuckFs,
            revision: 4,
            this_node: true,
        }],
        handle: Some("acct.duck".into()),
        account_bound: true,
        desktop_signer: true,
        managed_workspace: true,
    });
    state
}

#[test]
fn gateway_lists_routes_and_selects_on_click() {
    let state = gateway_ready();
    let mut ui = sim(operator::view(&state, Screen::Gateway, Mode::Light));
    assert!(
        has(&mut ui, Role::Button, "Save route"),
        "the editor offers a Save"
    );
    ui.click(by::role(Role::Button, "api.acct.duck"))
        .expect("a published route is a selectable row");
    assert!(emitted(
        ui,
        &Message::Gateway(GatewayMessage::SelectRoute("rk1".into()))
    ));
}

// --- Modules: category grouping + per-module copy emission ---

fn modules_ready() -> State {
    let mut state = State::default();
    state.modules.data = Resource::Ready(vec![ModuleRoot {
        id: "chat".into(),
        root: "cafe".repeat(8),
        category: ModuleCategory::Workspace,
    }]);
    state
}

#[test]
fn modules_group_and_copy_emits() {
    let state = modules_ready();
    let mut ui = sim(operator::view(&state, Screen::Modules, Mode::Light));
    assert!(
        ui.find("WORKSPACE").is_ok(),
        "modules render under their category heading"
    );
    ui.click(by::role(Role::Button, "Copy root"))
        .expect("each module card copies its committed root");
    assert!(emitted(
        ui,
        &Message::CopyModule {
            id: "chat".into(),
            root: "cafe".repeat(8),
        }
    ));
}

// --- Metrics: pause toggle + idle-plane notice ---

fn metrics_ready() -> State {
    let mut state = State::default();
    state.metrics.data = Resource::Ready(MetricsSnapshot {
        block_height: 128,
        connected_peers: 2,
        blocks_per_second: 1.5,
        apply_p50_ms: 2.0,
        apply_p95_ms: 4.2,
        accepted: 10,
        rejected: 0,
        data_planes: Vec::new(),
        sync_peers: Vec::new(),
        sampled_at: "12:00:00".into(),
    });
    state
}

#[test]
fn metrics_pause_toggles_and_idle_planes_are_noted() {
    let state = metrics_ready();
    let mut ui = sim(operator::view(&state, Screen::Metrics, Mode::Light));
    assert!(
        ui.find("No open data planes — this node is not carrying overlay traffic.")
            .is_ok()
    );
    ui.click(by::role(Role::Button, "Pause"))
        .expect("live metrics can be paused");
    assert!(emitted(ui, &Message::ToggleMetricsPause));
}

// --- Sandbox: re-check, guarded mode apply, and agent-run setup affordances ---

#[test]
fn sandbox_recheck_emits() {
    let state = sandbox_ready();
    let mut ui = sim(operator::view(&state, Screen::Sandbox, Mode::Light));
    ui.click(by::role(Role::Button, "Re-check"))
        .expect("the sandbox host can be re-checked");
    assert!(emitted(ui, &Message::Sandbox(SandboxMessage::Recheck)));
}

#[test]
fn sandbox_choosing_a_mode_emits() {
    let state = sandbox_ready();
    let mut ui = sim(operator::view(&state, Screen::Sandbox, Mode::Light));
    ui.click(by::role(Role::Button, "Podman"))
        .expect("a non-current available mode is selectable");
    assert!(emitted(
        ui,
        &Message::Sandbox(SandboxMessage::Choose(SandboxMode::Podman))
    ));
}

#[test]
fn sandbox_confirm_card_offers_apply_and_cancel() {
    let mut state = sandbox_ready();
    state.sandbox.chosen = Some(SandboxMode::Podman);
    let mut ui = sim(operator::view(&state, Screen::Sandbox, Mode::Light));
    assert!(
        ui.find("Apply Podman?").is_ok(),
        "a chosen mode raises the guarded restart confirmation"
    );
    assert!(has(&mut ui, Role::Button, "Apply and restart"));
    assert!(
        has(&mut ui, Role::Button, "Cancel"),
        "the guarded apply can be abandoned"
    );
}

#[test]
fn sandbox_setup_button_appears_and_emits() {
    let state = sandbox_ready();
    let mut ui = sim(operator::view(&state, Screen::Sandbox, Mode::Light));
    ui.click(by::role(Role::Button, "Set up with an agent"))
        .expect("a fixable check offers agent-run setup");
    assert!(emitted(
        ui,
        &Message::Sandbox(SandboxMessage::SetUpWithAgent {
            check: "runtime".into(),
            agent: "a1".into(),
        })
    ));
}
