//! Named boot presets for the dev shell.
//!
//! Presets serve two consumers: `iced_test`'s Emulator (headless in-process
//! tests pick a preset by name) and live dev boots via `DUCKTAPE_PRESET=<name>`
//! (the agent e2e drives the chrome without walking onboarding first). Dev-only:
//! the module is gated at the `mod` site like the rest of the agent wiring.

use super::*;

/// Every named preset, in the shape the `iced::daemon` builder consumes.
pub(super) fn all() -> Vec<iced::Preset<Shell, Message>> {
    vec![
        iced::Preset::new("ui-demo", ui_demo),
        iced::Preset::new("ui-operator", ui_operator),
        iced::Preset::new("ui-terminal", ui_terminal),
    ]
}

/// Resolves `DUCKTAPE_PRESET` for a live dev boot.
///
/// Live boots also get the agent bridge (the fixtures themselves stay pure so
/// in-process tests neither bind ports nor write endpoint files).
pub(super) fn from_env() -> Option<(Shell, Task<Message>)> {
    match std::env::var("DUCKTAPE_PRESET").ok()?.as_str() {
        "ui-demo" => {
            let (mut state, task) = ui_demo();
            state.agent = Some(agent_wire::boot());
            Some((state, task))
        }
        "ui-operator" => {
            let (mut state, task) = ui_operator();
            state.agent = Some(agent_wire::boot());
            Some((state, task))
        }
        "ui-terminal" => {
            let (mut state, task) = ui_terminal();
            state.agent = Some(agent_wire::boot());
            Some((state, task))
        }
        other => {
            tracing::warn!(
                target: "ducktape::agent",
                reason = "unknown_preset",
                preset = other,
                "unknown DUCKTAPE_PRESET; using the default boot"
            );
            None
        }
    }
}

/// Backend-less chrome: onboarding marked ready, no node, empty screen data.
/// UI-logic and navigation tests boot here with zero side effects — nothing
/// is loaded, spawned, or written.
pub(super) fn ui_demo() -> (Shell, Task<Message>) {
    let (main, open_main) = window::open(desktop::main_settings());
    let mut state = Shell::default();
    state.desktop.main = Some(main);
    state.onboarding.stage = onboarding::Stage::Ready;
    (state, open_main.map(Message::MainOpened))
}

/// Backend-less chrome with a synthetic local workspace, so operator surfaces
/// render their real rail while QA remains free of node and filesystem effects.
pub(super) fn ui_operator() -> (Shell, Task<Message>) {
    let (mut state, task) = ui_demo();
    let workspace = Workspace {
        id: "qa-local".into(),
        name: "QA Local".into(),
        chain_id: "qa-local".into(),
        pubkey: "qa-local".into(),
        founder: true,
        member: true,
        ports: crate::backend::WorkspacePorts {
            listen: 41_000,
            http: 41_001,
            rpc: 41_002,
            wireguard: None,
            invite: None,
        },
    };
    let projected = workspace_for_screen(workspace.clone());
    state.workspace.workspaces = vec![projected.clone()];
    state.workspace.workspace = Some(projected);
    state.active_workspace = Some(workspace);
    (state, task)
}

/// Side-effect-free terminal surface for visual QA. Normal navigation still
/// fails closed unless a matching local workspace and node client exist.
pub(super) fn ui_terminal() -> (Shell, Task<Message>) {
    let (mut state, task) = ui_operator();
    state.navigate(Screen::Terminal);
    (state, task)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The iced_test lane: a headless Simulator over the ui-demo chrome can
    /// find and click a module nav button, producing a real Navigate message.
    /// Addressed via the agent's own semantic layer (`by::role`), not visible
    /// text — the two QA lanes share one naming authority.
    #[test]
    fn ui_demo_chrome_navigates_via_simulator() {
        use iced_agent_plugin::selector::by;

        let (state, _boot) = ui_demo();
        let id = state.desktop.main.expect("preset opens a main window");

        let mut ui = iced_test::simulator(view::view(&state, id));
        ui.click(by::role(iced_agent_plugin::Role::Button, "Chat"))
            .expect("ui-demo should boot into chrome with a Chat nav button");

        let navigated = ui
            .into_messages()
            .any(|message| matches!(message, Message::Navigate(Screen::Chat)));
        assert!(navigated, "clicking the nav button should navigate to Chat");
    }

    #[test]
    fn operator_route_without_a_workspace_keeps_the_operator_rail() {
        use iced_agent_plugin::selector::by;

        let (mut state, _boot) = ui_demo();
        state.navigate(Screen::Node);
        assert!(state.active_workspace.is_none());
        let id = state.desktop.main.expect("preset opens a main window");
        let mut ui = iced_test::simulator(view::view(&state, id));

        ui.find(by::role(iced_agent_plugin::Role::Button, "Gateway"))
            .expect("operator routes should keep operator navigation visible");
    }

    #[test]
    fn terminal_preset_opens_without_starting_a_session() {
        let (state, _boot) = ui_terminal();
        assert_eq!(state.screen(), Screen::Terminal);
        assert_eq!(state.section, Section::Operator);
        assert!(state.active_workspace.is_some());
        assert_eq!(state.workspace.workspaces.len(), 1);
        assert_eq!(state.workspace.workspace.as_ref().unwrap().id, "qa-local");
        assert!(state.node_client.is_none());
        assert!(state.terminal.is_none());
        assert_eq!(
            state.terminal_screen.status(),
            terminal_screen::Status::Idle
        );
    }
}
