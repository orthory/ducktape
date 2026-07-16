//! Agents screen: error retry, run-log copy, reassign gating, runs-on picker.

use super::harness::*;
use crate::screens::agents::{
    self, AgentData, AgentRecord, AgentStatus, CapabilityStatus, Channel, LoadMode, Message,
    Owner, PendingRun, Resource, ResourceCaps, RunFilter, RunLog, RunLogEntry, RunOutcome,
    RunRecord, RunStream, SkillRef, State, Tab, TurnPolicy, Watch, WatchPolicyKind,
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

fn run_record(dispatch_id: &str) -> RunRecord {
    RunRecord {
        run_id: "run-h".into(),
        dispatch_id: dispatch_id.into(),
        agent_id: "triage".into(),
        channel_id: "chan-1".into(),
        anchor_sequence: 5,
        outcome: RunOutcome::Delivered,
        degraded: false,
        created_at: 10,
        delivered_at: 13,
        executing_node: "unknown".into(),
        output_ref: None,
        pr_number: None,
    }
}

fn watch() -> Watch {
    Watch {
        channel_id: "chan-1".into(),
        policy: TurnPolicy::Mention,
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

// ---- Top-level render variants -------------------------------------------

#[test]
fn loading_and_empty_render_center_states() {
    let loading = State::default(); // data defaults to Loading
    let mut ui = view(&loading);
    assert!(ui.find("Loading agents...").is_ok(), "the loading beat renders");

    let state = State {
        data: Resource::Empty,
        ..State::default()
    };
    let mut ui = view(&state);
    assert!(
        ui.find("No agent data").is_ok(),
        "the empty resource renders its own center state, not a blank pane"
    );
}

// ---- Roster / navigation / registration affordances ----------------------

#[test]
fn roster_row_selects_agent() {
    let mut loaded = data();
    loaded.agents.push(agent());
    let state = State {
        data: Resource::Ready(loaded),
        ..State::default()
    };
    let mut ui = view(&state);
    ui.click(by::role(Role::ListItem, "Triage"))
        .expect("the roster row is clickable");
    assert!(emitted(ui, &Message::SelectAgent("triage".into())));
}

#[test]
fn header_tab_switches_section() {
    let state = State {
        data: Resource::Ready(data()),
        ..State::default()
    };
    let mut ui = view(&state);
    ui.click(by::role(Role::Tab, "Activity"))
        .expect("the Activity tab is clickable");
    assert!(emitted(ui, &Message::SelectTab(Tab::Activity)));
}

#[test]
fn add_agent_starts_registration() {
    let mut loaded = data();
    loaded.agents.push(agent());
    let state = State {
        data: Resource::Ready(loaded),
        ..State::default()
    };
    let mut ui = view(&state);
    ui.click(by::role(Role::Button, "+ Add agent"))
        .expect("the header Add-agent button is clickable when data is loaded");
    assert!(emitted(ui, &Message::StartAdding));
}

#[test]
fn register_form_renders_identity_fields() {
    let state = State {
        data: Resource::Ready(data()),
        adding: true,
        ..State::default()
    };
    let mut ui = view(&state);
    assert!(
        has(&mut ui, Role::TextInput, "AGENT DISPLAY NAME"),
        "the register surface exposes the name field"
    );
    assert!(
        has(&mut ui, Role::Button, "Register agent"),
        "the register surface exposes its submit affordance"
    );
    assert!(has(&mut ui, Role::Button, "Cancel"));
}

#[test]
fn missing_agent_returns_to_roster() {
    let mut loaded = data();
    let mut real = agent();
    real.id = "real".into();
    loaded.agents.push(real);
    let state = State {
        data: Resource::Ready(loaded),
        selected_agent_id: Some("gone".into()),
        explicit_selection: true,
        ..State::default()
    };
    let mut ui = view(&state);
    ui.click(by::role(Role::Button, "Back to the roster"))
        .expect("the not-found card offers a way back");
    assert!(emitted(ui, &Message::ClearExplicitSelection));
}

// ---- Agent identity / run lifecycle affordances --------------------------

#[test]
fn agent_detail_pause_emits_toggle() {
    let mut loaded = data();
    loaded.agents.push(agent());
    let state = State {
        data: Resource::Ready(loaded),
        selected_agent_id: Some("triage".into()),
        explicit_selection: true,
        ..State::default()
    };
    let mut ui = view(&state);
    assert!(!has(&mut ui, Role::Button, "Resume agent"));
    ui.click(by::role(Role::Button, "Pause agent"))
        .expect("an active agent offers Pause");
    assert!(emitted(ui, &Message::ToggleAgentStatus));
}

#[test]
fn agent_detail_edit_opens() {
    let mut loaded = data();
    loaded.agents.push(agent());
    let state = State {
        data: Resource::Ready(loaded),
        selected_agent_id: Some("triage".into()),
        explicit_selection: true,
        ..State::default()
    };
    let mut ui = view(&state);
    ui.click(by::role(Role::Button, "Edit"))
        .expect("Edit opens the inline edit form");
    assert!(emitted(ui, &Message::StartEditing));
}

#[test]
fn paused_agent_offers_resume() {
    let mut loaded = data();
    let mut paused = agent();
    paused.status = AgentStatus::Paused;
    loaded.agents.push(paused);
    let state = State {
        data: Resource::Ready(loaded),
        selected_agent_id: Some("triage".into()),
        explicit_selection: true,
        ..State::default()
    };
    let mut ui = view(&state);
    assert!(has(&mut ui, Role::Button, "Resume agent"));
    assert!(
        !has(&mut ui, Role::Button, "Pause agent"),
        "a paused agent offers Resume, not Pause"
    );
}

#[test]
fn pending_agent_freezes_lifecycle() {
    let mut loaded = data();
    let mut pending = agent();
    pending.pending = true;
    loaded.agents.push(pending);
    let state = State {
        data: Resource::Ready(loaded),
        selected_agent_id: Some("triage".into()),
        explicit_selection: true,
        ..State::default()
    };
    let mut ui = view(&state);
    // The button still renders (disabled), but a pending write must not emit a
    // second mutation on top of the one in flight.
    ui.click(by::role(Role::Button, "Pause agent"))
        .expect("the disabled control still lays out");
    assert!(!emitted(ui, &Message::ToggleAgentStatus));
}

// ---- Auto-reply / watch affordances --------------------------------------

#[test]
fn watch_segment_switches_policy() {
    let state = State {
        data: Resource::Ready(data()),
        tab: Tab::AutoReply,
        ..State::default()
    };
    let mut ui = view(&state);
    ui.click(by::role(Role::Button, "Every message"))
        .expect("the policy segments are clickable");
    assert!(emitted(ui, &Message::WatchPolicyChanged(WatchPolicyKind::All)));
}

#[test]
fn add_watch_emits_when_channel_chosen() {
    let mut state = State {
        data: Resource::Ready(data()),
        tab: Tab::AutoReply,
        ..State::default()
    };
    state.watch.channel_id = "chan-1".into();
    let mut ui = view(&state);
    ui.click(by::role(Role::Button, "Add auto-reply"))
        .expect("a chosen channel enables the add button");
    assert!(emitted(ui, &Message::AddWatch));
}

#[test]
fn watch_row_turns_off() {
    let mut loaded = data();
    loaded.watches.push(watch());
    let state = State {
        data: Resource::Ready(loaded),
        tab: Tab::AutoReply,
        ..State::default()
    };
    let mut ui = view(&state);
    ui.click(by::role(Role::Button, "Turn off"))
        .expect("a watched channel can be turned off");
    assert!(emitted(ui, &Message::RemoveWatch("chan-1".into())));
}

// ---- Activity / run lifecycle affordances --------------------------------

#[test]
fn filter_switches_run_scope() {
    let state = State {
        data: Resource::Ready(data()),
        tab: Tab::Activity,
        ..State::default()
    };
    let mut ui = view(&state);
    ui.click(by::role(Role::Button, "Requested by you"))
        .expect("the run filter is clickable");
    assert!(emitted(ui, &Message::SelectRunFilter(RunFilter::Mine)));
}

#[test]
fn job_worker_toggle_emits() {
    let state = State {
        data: Resource::Ready(data()),
        tab: Tab::Activity,
        ..State::default()
    };
    let mut ui = view(&state);
    ui.click(by::role(Role::Button, "Enable worker"))
        .expect("the jobs worker can be enabled");
    assert!(emitted(ui, &Message::SetJobWorker(true)));
}

#[test]
fn cancel_run_emits() {
    let mut loaded = data();
    loaded.agents.push(agent());
    loaded.pending_runs.push(pending_run(Some(4)));
    let state = State {
        data: Resource::Ready(loaded),
        tab: Tab::Activity,
        ..State::default()
    };
    let mut ui = view(&state);
    ui.click(by::role(Role::Button, "Cancel"))
        .expect("an in-flight run can be cancelled");
    assert!(emitted(ui, &Message::CancelRun("run-1".into())));
}

#[test]
fn live_log_toggles() {
    let mut loaded = data();
    loaded.agents.push(agent());
    loaded.pending_runs.push(pending_run(Some(4)));
    let state = State {
        data: Resource::Ready(loaded),
        tab: Tab::Activity,
        ..State::default()
    };
    let mut ui = view(&state);
    ui.click(by::role(Role::Button, "Live log"))
        .expect("a run can expand its live log");
    assert!(emitted(ui, &Message::ToggleRunLog("ab".repeat(32))));
}

#[test]
fn history_link_opens_anchor() {
    let mut loaded = data();
    loaded.agents.push(agent());
    loaded.recent_runs.push(run_record(&"cd".repeat(32)));
    let state = State {
        data: Resource::Ready(loaded),
        tab: Tab::Activity,
        ..State::default()
    };
    let mut ui = view(&state);
    // The channel resolves to its name and the anchor becomes a chat deep-link.
    ui.click(by::role(Role::Button, "#general · message 5"))
        .expect("a delivered run links back to its chat anchor");
    assert!(emitted(
        ui,
        &Message::OpenRunAnchor {
            channel_id: "chan-1".into(),
            sequence: 5,
        }
    ));
}

// ---- Run-log render variants ---------------------------------------------

fn expanded_run(dispatch_id: &str) -> State {
    let mut loaded = data();
    loaded.agents.push(agent());
    loaded.pending_runs.push(pending_run(Some(4)));
    State {
        data: Resource::Ready(loaded),
        tab: Tab::Activity,
        expanded_run_logs: vec![dispatch_id.into()],
        ..State::default()
    }
}

#[test]
fn run_log_waiting_has_no_live_copy() {
    let dispatch_id = "ab".repeat(32);
    let state = expanded_run(&dispatch_id); // no RunLog stored yet
    let mut ui = view(&state);
    assert!(
        ui.find("Waiting for retained output...").is_ok(),
        "an expanded run with no output yet says so"
    );
    // Copy renders but is inert until there is something to copy.
    ui.click(by::role(Role::Button, "Copy"))
        .expect("the disabled Copy still lays out");
    assert!(!emitted(ui, &Message::CopyRunLog(dispatch_id)));
}

#[test]
fn run_log_unavailable_state() {
    let dispatch_id = "ab".repeat(32);
    let mut state = expanded_run(&dispatch_id);
    state.run_logs.insert(
        dispatch_id,
        RunLog {
            unavailable: true,
            ..RunLog::default()
        },
    );
    let mut ui = view(&state);
    assert!(ui.find("Run output unavailable.").is_ok());
}

#[test]
fn terminal_run_log_notes_eviction() {
    let dispatch_id = "cd".repeat(32);
    let mut loaded = data();
    loaded.agents.push(agent());
    loaded.recent_runs.push(run_record(&dispatch_id));
    let state = State {
        data: Resource::Ready(loaded),
        tab: Tab::Activity,
        expanded_run_logs: vec![dispatch_id],
        ..State::default()
    };
    let mut ui = view(&state);
    assert!(
        ui.find("No retained output received; older output may have been evicted.")
            .is_ok(),
        "a finished run with no retained log explains the eviction, not silence"
    );
}

#[test]
fn run_log_dropped_notice() {
    let dispatch_id = "ab".repeat(32);
    let mut state = expanded_run(&dispatch_id);
    state.run_logs.insert(
        dispatch_id,
        RunLog {
            entries: vec![RunLogEntry::Line {
                stream: RunStream::Stdout,
                text: "still running".into(),
            }],
            dropped: 5,
            ..RunLog::default()
        },
    );
    let mut ui = view(&state);
    assert!(
        ui.find("live log tail: 5 older events omitted").is_ok(),
        "an evicting live tail warns that older events were dropped"
    );
}

// ---- Error surfacing (the class that hid the pages regression) ------------

#[test]
fn write_failure_surfaces_error() {
    let mut loaded = data();
    loaded.agents.push(agent());
    let state = State {
        data: Resource::Ready(loaded),
        selected_agent_id: Some("triage".into()),
        explicit_selection: true,
        error: Some("op rejected: Module(\"agent id already registered\")".into()),
        ..State::default()
    };
    let mut ui = view(&state);
    // A failed mutation must render its (wrapped) error, not swallow it — the
    // selectable line is a read-only text_input, so match its value directly.
    assert!(
        ui.find("op rejected: Module(\"agent id already registered\")")
            .is_ok(),
        "a write failure surfaces the wrapped error to the operator"
    );
}

#[test]
fn recent_runs_error_surfaces() {
    let mut loaded = data();
    loaded.recent_runs_error = Some("history read failed: node offline".into());
    let state = State {
        data: Resource::Ready(loaded),
        tab: Tab::Activity,
        ..State::default()
    };
    let mut ui = view(&state);
    assert!(ui.find("Run history unavailable").is_ok());
    assert!(
        ui.find("history read failed: node offline").is_ok(),
        "the run-history error text surfaces, not just a generic banner"
    );
}
