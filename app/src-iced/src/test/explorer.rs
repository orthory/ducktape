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
