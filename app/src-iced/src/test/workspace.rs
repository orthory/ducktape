//! Workspace connect modal: member-activation feedback and the
//! incompatible-workspace fresh-start route.

use super::harness::*;
use crate::screens::workspace::{
    self, AccountLink, BootError, BootErrorKind, ConnectMode, Message, Stage, State, Workspace,
};
use crate::theme;

fn incompatible() -> State {
    State {
        stage: Stage::Failed,
        boot_error: Some(BootError {
            kind: BootErrorKind::IncompatibleWorkspace,
            workspace_id: "ws-1".into(),
            reason: "state schema mismatch".into(),
            log_path: None,
            log_tail: String::new(),
        }),
        ..Default::default()
    }
}

#[test]
fn incompatible_workspace_offers_a_fresh_start() {
    let state = incompatible();
    let mut ui = sim(workspace::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Create fresh workspace"))
        .expect("the incompatible-workspace screen renders the fresh-start button");
    assert!(emitted(ui, &Message::CreateFresh));
}

#[test]
fn create_fresh_routes_to_the_create_tab() {
    let mut state = incompatible();
    state.mode = ConnectMode::Join;
    let command = workspace::update(&mut state, Message::CreateFresh);
    assert!(command.is_none(), "routing is a local view transition");
    assert_eq!(state.stage, Stage::Connect);
    assert_eq!(state.mode, ConnectMode::Create);
    assert!(state.boot_error.is_none());
}

#[test]
fn activating_a_member_shows_connecting() {
    let workspace = Workspace {
        id: "ws-1".into(),
        name: "Demo".into(),
        chain_id: "chain-abc".into(),
        pubkey: "key".into(),
        member: true,
    };
    let state = State {
        stage: Stage::Connect,
        busy: true,
        workspace: Some(workspace.clone()),
        workspaces: vec![workspace],
        ..Default::default()
    };
    let mut ui = sim(workspace::view(&state, theme::Mode::Light));
    assert!(
        ui.find("Connecting…").is_ok(),
        "the row being activated shows a busy label instead of nothing"
    );
}

#[test]
fn a_locked_account_warns_before_linking_a_joining_node() {
    // ON-G5: account_link is now populated from the custody probe, so the
    // load-bearing safety hint (a locked account silently links a node to
    // nobody) actually renders on the join-progress screen.
    let workspace = Workspace {
        id: "ws-1".into(),
        name: "Demo".into(),
        chain_id: "chain-abc".into(),
        pubkey: "key".into(),
        member: false,
    };
    let state = State {
        stage: Stage::Joining,
        workspace: Some(workspace.clone()),
        workspaces: vec![workspace],
        account_link: AccountLink::Locked,
        ..Default::default()
    };
    let mut ui = sim(workspace::view(&state, theme::Mode::Light));
    assert!(
        ui.find(
            "Your account is locked — unlock it in the Account view to link this node to you."
        )
        .is_ok(),
        "a locked account surfaces the link-safety hint while joining"
    );
}
