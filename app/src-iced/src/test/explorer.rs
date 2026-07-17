//! Explorer: block digests are selectable copy targets.

use super::harness::*;
use crate::screens::explorer::{self, BlockRecord, Disposition, Message, Resource, RootOp};
use crate::theme;

fn op() -> RootOp {
    RootOp {
        proposer: "cc".repeat(32),
        proposer_name: Some("Founder Rae".into()),
        disposition: Disposition::Applied,
        target: "chat".into(),
        operations: Vec::new(),
        payload: "{\"Post\":{}}".into(),
        op_hash: "dd".repeat(32),
    }
}

fn block() -> BlockRecord {
    BlockRecord {
        height: 7,
        hash: "aa".repeat(32),
        commit_hash: "bb".repeat(32),
        ops: vec![op()],
    }
}

/// EX-1.4: every full digest (block hash, commit hash, proposer key, op hash,
/// payload) renders as a selectable copy target so the truncated table values
/// stay recoverable. The digests live in the detail view, so open a block first.
#[test]
fn block_digests_are_copy_targets() {
    let state = explorer::State {
        blocks: Resource::Ready(vec![block()]),
        open: Some(block()),
        pending_focus: None,
    };
    let mut ui = sim(explorer::view(&state, theme::Mode::Light));
    for target in ["HASH", "COMMIT", "PROPOSER", "OP HASH", "PAYLOAD"] {
        assert!(
            has(&mut ui, Role::TextInput, target),
            "the {target} digest must be a selectable copy target"
        );
    }
}

/// A block still on the list (not opened) shows no copy targets — the affordance
/// is scoped to the detail — but the row still opens that detail.
#[test]
fn list_rows_open_without_exposing_copy_targets() {
    let state = explorer::State {
        blocks: Resource::Ready(vec![block()]),
        open: None,
        pending_focus: None,
    };
    let mut ui = sim(explorer::view(&state, theme::Mode::Light));
    assert!(
        !has(&mut ui, Role::TextInput, "HASH"),
        "the block list should not expose the full-hash inputs"
    );
    ui.click(by::role(Role::ListItem, "#7"))
        .expect("the block row opens the detail");
    assert!(emitted(ui, &Message::Open(7)));
}

fn explorer_ui(state: &explorer::State) -> iced_test::Simulator<'_, Message> {
    sim(explorer::view(state, theme::Mode::Light))
}

#[test]
fn passive_states_show_their_center_copy() {
    for (blocks, needle) in [
        (Resource::Loading, "Loading blocks…"),
        (Resource::Empty, "No blocks yet"),
    ] {
        let state = explorer::State {
            blocks,
            ..Default::default()
        };
        let mut ui = explorer_ui(&state);
        assert!(ui.find(needle).is_ok(), "expected {needle:?} to render");
    }
}

// The error arm renders the failure as a copyable detail with a live Retry —
// the surfacing-plus-retry path this test layer exists to protect.
#[test]
fn error_state_offers_copyable_detail_and_retry() {
    let state = explorer::State {
        blocks: Resource::Error("ring read failed".into()),
        ..Default::default()
    };
    let mut ui = explorer_ui(&state);
    assert!(ui.find("Block explorer unavailable").is_ok());
    assert!(
        has(&mut ui, Role::TextInput, "error"),
        "the failure detail is a selectable copy target"
    );
    ui.click(by::role(Role::Button, "Retry"))
        .expect("the error arm offers Retry");
    assert!(emitted(ui, &Message::Load));
}

#[test]
fn open_block_detail_backs_out() {
    let state = explorer::State {
        blocks: Resource::Ready(vec![block()]),
        open: Some(block()),
        pending_focus: None,
    };
    let mut ui = explorer_ui(&state);
    ui.click(by::role(Role::Button, "← Blocks"))
        .expect("the detail offers a Back control");
    assert!(emitted(ui, &Message::Back));
}

// An idle (heartbeat) block has no ops: its detail reads as a nop rather than an
// empty ops list.
#[test]
fn idle_block_detail_reads_as_heartbeat() {
    let idle = BlockRecord {
        height: 9,
        hash: "aa".repeat(32),
        commit_hash: "bb".repeat(32),
        ops: Vec::new(),
    };
    let state = explorer::State {
        blocks: Resource::Ready(vec![idle.clone()]),
        open: Some(idle),
        pending_focus: None,
    };
    let mut ui = explorer_ui(&state);
    assert!(
        ui.find("Idle block — no ops committed in this window (a heartbeat nop).")
            .is_ok(),
        "an empty block explains itself as a heartbeat"
    );
}
