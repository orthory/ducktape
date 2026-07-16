//! Operator screens: the Ready-arm layout for the unified node-section chrome.
//! The presets boot without a node, so these render the Ready views directly
//! with synthetic snapshots — the only place the pinned header, compact
//! Start/Stop, two-column permissions, and sandbox picker/badge appear.

use super::harness::*;
use crate::screens::operator::{
    self, CheckState, Command, Message, NodeMessage, NodeRole, NodeSnapshot, NodeTab, Resource,
    SandboxCheck, SandboxData, SandboxMode, Screen, State,
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
