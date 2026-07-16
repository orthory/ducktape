//! Agents screen: error retry, run-log copy, reassign gating, runs-on picker.

use super::harness::*;
use crate::screens::agents::{
    self, AgentData, AgentRecord, AgentStatus, CapabilityStatus, Channel, LoadMode, Message,
    Owner, PendingRun, Resource, ResourceCaps, RunLog, RunLogEntry, RunStream, SkillRef, State,
    Tab, WatchPolicyKind,
};
use crate::theme;

fn data() -> AgentData {
    AgentData {
        agents: vec![],
        capabilities: vec!["codex_gpt-5_medium".into()],
        capability_status: CapabilityStatus::Ready,
        channels: vec![Channel {
            id: "chan-1".into(),
            name: "general".into(),
        }],
        watches: vec![],
        pending_runs: vec![],
        recent_runs: vec![],
        recent_runs_error: None,
        usage: None,
        job_worker_pending: false,
    }
}

fn agent() -> AgentRecord {
    AgentRecord {
        id: "triage".into(),
        owner: Owner::System,
        display_name: "Triage".into(),
        capability: "codex_gpt-5_medium".into(),
        allowed_actions: vec!["chat.post".into()],
        status: AgentStatus::Active,
        created_at: "now".into(),
        updated_at: "now".into(),
        caps: ResourceCaps::default(),
        skills: vec![SkillRef {
            name: "persona".into(),
            source_prefix: "/shared/skills/triage".into(),
            snapshot: None,
            load: LoadMode::Always,
        }],
        pending: false,
    }
}

fn pending_run(lease_remaining: Option<u64>) -> PendingRun {
    PendingRun {
        run_id: "run-1".into(),
        dispatch_id: "ab".repeat(32),
        agent_id: "triage".into(),
        channel_id: "chan-1".into(),
        anchor_sequence: 3,
        job_id: None,
        created_at: "just now".into(),
        requested_by_me: true,
        attempt: 2,
        lease_remaining,
        pending: false,
    }
}

fn view(state: &State) -> iced_test::Simulator<'_, Message> {
    sim(agents::view(state, theme::Mode::Light, theme::ACCENTS[0]))
}

#[test]
fn top_level_error_offers_retry() {
    let state = State {
        data: Resource::Error("agents service is offline".into()),
        ..State::default()
    };
    let mut ui = view(&state);
    assert!(
        has(&mut ui, Role::Button, "Retry"),
        "an unrecoverable Agents error must offer a Retry, not dead-end"
    );
    ui.click(by::role(Role::Button, "Retry"))
        .expect("retry is clickable");
    assert!(emitted(ui, &Message::Load));
}

#[test]
fn expanded_run_log_offers_copy() {
    let mut loaded = data();
    loaded.agents.push(agent());
    loaded.pending_runs.push(pending_run(Some(4)));
    let dispatch_id = "ab".repeat(32);
    let mut state = State {
        data: Resource::Ready(loaded),
        tab: Tab::Activity,
        expanded_run_logs: vec![dispatch_id.clone()],
        ..State::default()
    };
    state.run_logs.insert(
        dispatch_id,
        RunLog {
            entries: vec![RunLogEntry::Line {
                stream: RunStream::Stdout,
                text: "cargo test -p app".into(),
            }],
            ..RunLog::default()
        },
    );
    let mut ui = view(&state);
    assert!(
        has(&mut ui, Role::Button, "Copy"),
        "a run log with output must expose a Copy button (text() cannot be selected)"
    );
    ui.click(by::role(Role::Button, "Copy"))
        .expect("copy is clickable");
    assert!(emitted(ui, &Message::CopyRunLog("ab".repeat(32))));
}

#[test]
fn reassign_only_offered_on_an_expired_lease() {
    let mut healthy = data();
    healthy.agents.push(agent());
    healthy.pending_runs.push(pending_run(Some(6)));
    let state = State {
        data: Resource::Ready(healthy),
        tab: Tab::Activity,
        ..State::default()
    };
    let mut ui = view(&state);
    assert!(
        !has(&mut ui, Role::Button, "Force reassign"),
        "a healthy run (lease not expired) must not offer Force reassign"
    );

    let mut stalled = data();
    stalled.agents.push(agent());
    stalled.pending_runs.push(pending_run(Some(0)));
    let state = State {
        data: Resource::Ready(stalled),
        tab: Tab::Activity,
        ..State::default()
    };
    let mut ui = view(&state);
    assert!(
        has(&mut ui, Role::Button, "Force reassign"),
        "a stalled run (expired lease) must offer Force reassign"
    );
    ui.click(by::role(Role::Button, "Force reassign"))
        .expect("force reassign is clickable");
    // The node increments the attempt; the view must pass the CURRENT attempt (2),
    // never attempt + 1 — an off-by-one had the node reject the reassign.
    assert!(emitted(ui, &Message::ReassignRun("run-1".into(), 2)));
}

#[test]
fn runs_on_error_offers_retry_not_free_text() {
    let mut loaded = data();
    loaded.capabilities = vec![];
    loaded.capability_status = CapabilityStatus::Error;
    let state = State {
        data: Resource::Ready(loaded),
        adding: true,
        ..State::default()
    };
    let mut ui = view(&state);
    // The picker's own actionable empty state carries the Retry (RetryCapabilities),
    // instead of a free-text box that would register an unrunnable agent.
    ui.click(by::role(Role::Button, "Retry"))
        .expect("runs-on error state offers Retry");
    assert!(emitted(ui, &Message::RetryCapabilities));
}

#[test]
fn agent_detail_guards_address_and_opens_skills() {
    let mut loaded = data();
    loaded.agents.push(agent());
    let state = State {
        data: Resource::Ready(loaded),
        selected_agent_id: Some("triage".into()),
        explicit_selection: true,
        ..State::default()
    };
    let mut ui = view(&state);
    // The roster row (with its selected accent bar) lays out headlessly.
    assert!(has(&mut ui, Role::ListItem, "Triage"));
    // A valid label id has an address row.
    assert!(ui.find("Address").is_ok(), "a valid id renders its address");
    // A curated skill is reachable via Open.
    assert!(has(&mut ui, Role::Button, "Open"));
    ui.click(by::role(Role::Button, "Open"))
        .expect("open is clickable");
    assert!(emitted(
        ui,
        &Message::OpenSkillFiles("/shared/skills/triage".into())
    ));
}

#[test]
fn agent_detail_omits_address_for_a_legacy_id() {
    let mut loaded = data();
    let mut legacy = agent();
    legacy.id = "Legacy_Agent".into();
    loaded.agents.push(legacy);
    let state = State {
        data: Resource::Ready(loaded),
        selected_agent_id: Some("Legacy_Agent".into()),
        explicit_selection: true,
        ..State::default()
    };
    let mut ui = view(&state);
    assert!(
        ui.find("Address").is_err(),
        "a non-label id must not fabricate an address"
    );
}

#[test]
fn assigned_watch_agent_picker_reports_no_agents() {
    let mut loaded = data();
    loaded.agents = vec![];
    let mut state = State {
        data: Resource::Ready(loaded),
        tab: Tab::AutoReply,
        ..State::default()
    };
    state.watch.policy = WatchPolicyKind::Assigned;
    let mut ui = view(&state);
    // No free-text agent id: with no agents the picker shows an actionable note.
    assert!(
        ui.find("No agents yet — register one first.").is_ok(),
        "the agent picker must say what to do when no agent exists"
    );
}
