//! The roster the host pushes is what the screen shows; a row opens its
//! record, and the record's writes leave as intents.

use members_view::host::{AgentStatus, Copy, MemberRow, MembersProps, Propose};
use members_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, press, texts};
use ui_lang_guest::wire::Frame;

fn member(key: &str, label: &str, role: &str, is_this_node: bool) -> MemberRow {
    MemberRow {
        key: key.into(),
        label: label.into(),
        role: role.into(),
        is_this_node,
        is_agent: role == "agent",
        model: if role == "agent" {
            "review".into()
        } else {
            String::new()
        },
        live: true,
    }
}

fn roster(rows: Vec<MemberRow>, admin: bool) -> Vec<u8> {
    serde_json::to_vec(&MembersProps {
        rows,
        admin,
        connected: true,
        answered: true,
        dark: false,
    })
    .expect("props encode")
}

fn three() -> Vec<MemberRow> {
    vec![
        member("val-1", "Ada", "validator", true),
        member("res-1", "Bo", "resident", false),
        member("reviewer-bot", "Reviewer Bot", "agent", false),
    ]
}

/// Boot, push `rows`, and open the record for `label`.
fn opened(rows: Vec<MemberRow>, admin: bool, label: &str) -> Frame {
    boot_native();
    let frame = tick_native(Vec::new());
    assert_eq!(frame.requests[0].kind, "members.props");
    assert!(has_text(&frame, "Not connected"), "{:?}", texts(&frame));
    let subscription = frame.requests[0].id;
    let frame = tick_native(vec![item(subscription, &roster(rows, admin))]);
    assert!(has_text(&frame, label), "{:?}", texts(&frame));
    tick_native(press(&frame, label))
}

#[test]
fn the_roster_the_host_pushes_is_what_the_screen_shows_and_the_strip_filters_it() {
    boot_native();
    let frame = tick_native(Vec::new());
    let subscription = frame.requests[0].id;
    let frame = tick_native(vec![item(subscription, &roster(three(), true))]);
    for expected in [
        "2 humans · 1 agent",
        "Ada",
        "this node",
        "VALIDATOR",
        "RESIDENT",
        "AGENT",
    ] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    let frame = tick_native(press(&frame, "Show agents only"));
    assert!(has_text(&frame, "Reviewer Bot"), "{:?}", texts(&frame));
    assert!(!has_text(&frame, "Ada"), "{:?}", texts(&frame));
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
}

#[test]
fn this_nodes_record_offers_its_key_which_leaves_as_a_copy() {
    let frame = opened(three(), true, "Ada");
    assert!(has_text(&frame, "public key"), "{:?}", texts(&frame));
    assert!(
        !has_text(&frame, "Remove from the validator set"),
        "{:?}",
        texts(&frame)
    );
    let frame = tick_native(press(&frame, "Copy this node's key"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "members.copy");
    assert_eq!(
        serde_json::from_slice::<Copy>(&intent.payload).expect("decodes"),
        Copy {
            text: "val-1".into(),
            label: "Node key copied".into()
        }
    );
}

#[test]
fn an_agents_record_pauses_it_by_registry_id() {
    let frame = opened(three(), true, "Reviewer Bot");
    assert!(has_text(&frame, "agent id"), "{:?}", texts(&frame));
    assert!(has_text(&frame, "active"), "{:?}", texts(&frame));
    let frame = tick_native(press(&frame, "Pause agent"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "members.agent_status");
    assert_eq!(
        serde_json::from_slice::<AgentStatus>(&intent.payload).expect("decodes"),
        AgentStatus {
            agent_id: "reviewer-bot".into(),
            paused: true
        }
    );
}

#[test]
fn an_admin_opens_a_ballot_over_a_resident_and_a_non_admin_reads_the_rule() {
    let frame = opened(three(), true, "Bo");
    let frame = tick_native(press(&frame, "Promote to validator"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "members.propose");
    assert_eq!(
        serde_json::from_slice::<Propose>(&intent.payload).expect("decodes"),
        Propose {
            action: "add_validator".into(),
            key: "res-1".into()
        }
    );

    let frame = opened(three(), false, "Bo");
    assert!(
        !has_text(&frame, "Promote to validator"),
        "{:?}",
        texts(&frame)
    );
    assert!(
        has_text(
            &frame,
            "Only a validator node may open a membership proposal."
        ),
        "{:?}",
        texts(&frame)
    );
}
