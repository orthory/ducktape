//! The view driven natively through the wire: the register the host pushes
//! is what the screen shows, and a press leaves as an intent the host acts
//! on — nothing is asked of the host but the register itself.

use governance_view::host::{GovernanceProps, Intent, ProposalRow};
use governance_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, press, texts};
use ui_lang_guest::wire::{Frame, Node, Request};

fn boot() -> Frame {
    boot_native();
    tick_native(Vec::new())
}

/// Whether the button labelled `name` is in the tree with no press to send.
fn button_disabled(frame: &Frame, name: &str) -> bool {
    let mut root = frame.root.clone().expect("a tree");
    let mut disabled = None;
    root.for_each_mut(&mut |node| {
        if let Node::Button {
            label, on_press, ..
        } = node
            && label.as_deref() == Some(name)
        {
            disabled = Some(on_press.is_none());
        }
    });
    disabled.expect("the button is in the tree")
}

fn kinds(requests: &[Request]) -> Vec<&str> {
    requests
        .iter()
        .map(|request| request.kind.as_str())
        .collect()
}

fn proposal(id: &str, approvals: i64, required_yes: i64, open: bool) -> ProposalRow {
    ProposalRow {
        id: id.into(),
        action: "add_validator".into(),
        detail: "node-7".into(),
        proposer: "robin".into(),
        status: if open {
            "open".into()
        } else {
            "executed".into()
        },
        deadline: 4_200,
        approvals,
        rejections: 0,
        rule: "threshold".into(),
        required_yes,
        electorate: 4,
        open,
        settled_height: if open { 0 } else { 84_912 },
    }
}

fn register(rows: Vec<ProposalRow>, voting: &str) -> Vec<u8> {
    serde_json::to_vec(&GovernanceProps {
        rows,
        voting: voting.into(),
        admin: true,
        connected: true,
        answered: true,
        dark: false,
    })
    .expect("props encode")
}

/// The one subscription at boot, and the register it answers with is the
/// whole screen: header count, the open card with its tally, the settled row.
#[test]
fn the_register_the_host_pushes_is_what_the_screen_shows() {
    let frame = boot();
    assert_eq!(
        kinds(&frame.requests),
        ["governance.props"],
        "only the register at boot: {:?}",
        frame.requests
    );
    assert!(has_text(&frame, "Not connected"), "{:?}", texts(&frame));

    let subscription = frame.requests[0].id;
    let rows = vec![
        proposal("prop-open", 1, 2, true),
        proposal("prop-done", 2, 2, false),
    ];
    let frame = tick_native(vec![item(subscription, &register(rows, ""))]);
    for expected in [
        "1 open · 1 settled",
        "1 pending",
        "prop-open",
        "1 / 2",
        "1 approval · 1 more for quorum",
        "Approve →",
        "RECENTLY FINALIZED",
        "prop-done",
        "h 84,912",
    ] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(
        frame.requests.is_empty(),
        "a register asks nothing back: {:?}",
        frame.requests
    );
}

/// Approve leaves as `governance.vote` with the proposal and the answer; the
/// host does the write and pushes the next register, on which the card is
/// busy and a second press goes nowhere.
#[test]
fn a_vote_leaves_as_an_intent_and_a_busy_register_disables_the_card() {
    let frame = boot();
    let subscription = frame.requests[0].id;
    let frame = tick_native(vec![item(
        subscription,
        &register(vec![proposal("prop-open", 1, 2, true)], ""),
    )]);

    let frame = tick_native(press(&frame, "Approve"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent after Approve, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "governance.vote");
    assert_eq!(
        serde_json::from_slice::<Intent>(&intent.payload).expect("an intent decodes"),
        Intent {
            proposal_id: "prop-open".into(),
            approve: true,
        }
    );

    let frame = tick_native(vec![item(
        subscription,
        &register(vec![proposal("prop-open", 1, 2, true)], "prop-open"),
    )]);
    assert!(
        button_disabled(&frame, "Reject"),
        "a busy card's buttons are disabled: {:?}",
        texts(&frame)
    );
}

/// Once the rule is met the card offers Settle instead of Approve, and it
/// leaves as `governance.execute`.
#[test]
fn a_met_rule_offers_settle_which_leaves_as_execute() {
    let frame = boot();
    let subscription = frame.requests[0].id;
    let frame = tick_native(vec![item(
        subscription,
        &register(vec![proposal("prop-met", 2, 2, true)], ""),
    )]);
    assert!(has_text(&frame, "quorum met"), "{:?}", texts(&frame));
    assert!(!has_text(&frame, "Approve"), "{:?}", texts(&frame));

    let frame = tick_native(press(&frame, "Settle"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent after Settle, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "governance.execute");
    assert_eq!(
        serde_json::from_slice::<Intent>(&intent.payload)
            .expect("an intent decodes")
            .proposal_id,
        "prop-met"
    );
}
