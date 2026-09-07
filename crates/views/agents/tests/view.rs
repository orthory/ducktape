//! The register the host pushes is what the screen shows; nothing leaves.

use agents_view::host::{AgentRow, AgentsProps};
use agents_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, texts};

fn agent(name: &str, status: &str, live: bool) -> AgentRow {
    AgentRow {
        id: name.to_lowercase(),
        name: name.into(),
        initials: "RB".into(),
        capability: "review".into(),
        status: status.into(),
        owner_handle: "eddy".into(),
        live,
        skill_count: 3,
        cap_count: 2,
    }
}

fn register(rows: Vec<AgentRow>, connected: bool) -> Vec<u8> {
    serde_json::to_vec(&AgentsProps {
        rows,
        connected,
        answered: true,
        dark: false,
    })
    .expect("props encode")
}

#[test]
fn the_register_the_host_pushes_is_what_the_screen_shows() {
    boot_native();
    let frame = tick_native(Vec::new());
    assert_eq!(frame.requests.len(), 1, "{:?}", frame.requests);
    assert_eq!(frame.requests[0].kind, "agents.props");
    assert!(has_text(&frame, "Not connected"), "{:?}", texts(&frame));
    assert!(
        texts(&frame)
            .iter()
            .any(|text| text.starts_with("The registry records")),
        "{:?}",
        texts(&frame)
    );

    let subscription = frame.requests[0].id;
    let rows = vec![
        agent("Reviewer", "active", true),
        agent("Scribe", "paused", false),
    ];
    let frame = tick_native(vec![item(subscription, &register(rows, true))]);
    for expected in [
        "2 agents · 1 working",
        "Reviewer",
        "review",
        "ACTIVE",
        "PAUSED",
        "eddy",
    ] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(!has_text(&frame, "Not connected"), "{:?}", texts(&frame));
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
}
