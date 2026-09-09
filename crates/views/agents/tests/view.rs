//! The register the host pushes is what the screen shows; a record opens as
//! an editor for its controller and as a reading for everyone else, and the
//! only things that leave are the writes the reader asked for.

use agents_view::host::{AgentCaps, AgentRow, AgentSkill, AgentsProps, Draft, Status};
use agents_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, pick, press, texts, toggle, type_into};
use ui_lang_guest::wire::{Frame, Request};

/// Inputs are found by placeholder and pick lists by key.
const AGENT_ID_HINT: &str = "a-dns-label, e.g. chiefduck";
const CAPABILITY_PICK: &str = "AgentsView/root/editor/agent-capability";

fn agent(name: &str, status: &str, live: bool) -> AgentRow {
    AgentRow {
        id: name.to_lowercase().replace(' ', "-"),
        name: name.into(),
        initials: "RB".into(),
        capability: "review".into(),
        status: status.into(),
        owner_handle: "eddy".into(),
        controller: "7".into(),
        live,
        allowed_actions: vec!["chat.post".into()],
        caps: AgentCaps {
            forge_read: vec!["ducktape".into()],
            pages_write: vec!["*".into()],
            ..AgentCaps::default()
        },
        skills: vec![
            AgentSkill {
                name: "review".into(),
                source_prefix: "/shared/skills/review".into(),
                source_snapshot: String::new(),
                always: true,
            },
            AgentSkill {
                name: "style".into(),
                source_prefix: "/shared/skills/style".into(),
                source_snapshot: String::new(),
                always: false,
            },
            AgentSkill {
                name: "tests".into(),
                source_prefix: "/shared/skills/tests".into(),
                source_snapshot: String::new(),
                always: false,
            },
        ],
    }
}

fn register(rows: Vec<AgentRow>, account: &str, committed: i64) -> Vec<u8> {
    serde_json::to_vec(&AgentsProps {
        rows,
        capabilities: vec!["claude".into(), "codex".into()],
        actions: vec!["chat.post".into(), "tasks.create".into()],
        account: account.into(),
        committed,
        connected: true,
        answered: true,
        dark: false,
        // the messages pane's own tests are `tests/messaging.rs`; the register
        // draws with nothing open beside it
        messaging: agents_view::host::MessagingProps::default(),
    })
    .expect("props encode")
}

/// Boot, subscribe, and push the register as `account`.
fn booted(rows: Vec<AgentRow>, account: &str) -> (u64, Frame) {
    boot_native();
    let frame = tick_native(Vec::new());
    assert_eq!(frame.requests.len(), 1, "{:?}", frame.requests);
    assert_eq!(frame.requests[0].kind, "agents.props");
    let subscription = frame.requests[0].id;
    let frame = tick_native(vec![item(subscription, &register(rows, account, 0))]);
    (subscription, frame)
}

fn one_intent(frame: &Frame) -> &Request {
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    intent
}

#[test]
fn the_register_the_host_pushes_is_what_the_screen_shows() {
    boot_native();
    let frame = tick_native(Vec::new());
    assert!(has_text(&frame, "Not connected"), "{:?}", texts(&frame));
    assert!(
        texts(&frame)
            .iter()
            .any(|text| text.starts_with("The registry records")),
        "{:?}",
        texts(&frame)
    );

    let (_, frame) = booted(
        vec![
            agent("Reviewer", "active", true),
            agent("Scribe", "paused", false),
        ],
        "",
    );
    for expected in [
        "2 agents · 1 working",
        "Reviewer",
        "review",
        "ACTIVE",
        "PAUSED",
        "eddy",
        // counts derive from the record: three skills, two grants
        "3",
        "2",
    ] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(!has_text(&frame, "Not connected"), "{:?}", texts(&frame));
    // no account on this device: nothing to register a new agent under
    assert!(!has_text(&frame, "New agent"), "{:?}", texts(&frame));
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
}

#[test]
fn the_controller_edits_the_whole_record_and_saves_it_in_one_write() {
    let (_, frame) = booted(vec![agent("Reviewer Bot", "active", false)], "7");
    let frame = tick_native(press(&frame, "Reviewer Bot"));
    for expected in ["Identity", "Executor", "Actions", "Grants", "Skills", "ducktape", "*"] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(!has_text(&frame, "Only this agent's controller"));

    // tick an action, pick the executor, drop a grant, flip a skill to the
    // persona, and add a grant — every edit is a draft until the save
    let frame = tick_native(toggle(&frame, "tasks.create", true));
    let frame = tick_native(pick(&frame, CAPABILITY_PICK, "claude"));
    let frame = tick_native(press(&frame, "Remove grant"));
    let frame = tick_native(press(&frame, "Load always"));
    let frame = tick_native(type_into(&frame, "repo, prefix, page id…", "playground"));
    let frame = tick_native(press(&frame, "Add grant"));
    let frame = tick_native(type_into(&frame, "0", "4"));
    assert!(frame.requests.is_empty(), "drafts leave nothing: {:?}", frame.requests);

    let frame = tick_native(press(&frame, "Save agent"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "agents.save");
    let draft: Draft = serde_json::from_slice(&intent.payload).expect("decodes");
    assert_eq!(draft.agent_id, "reviewer-bot");
    assert_eq!(draft.display_name, "Reviewer Bot");
    assert_eq!(draft.capability, "claude");
    assert_eq!(draft.allowed_actions, ["chat.post", "tasks.create"]);
    // the first "Remove grant" was the forge read; the added one is a
    // forge read again (the kind picker's default)
    assert_eq!(draft.caps.forge_read, ["playground"]);
    assert_eq!(draft.caps.pages_write, ["*"]);
    assert_eq!(draft.caps.subagent_budget, 4);
    assert!(draft.skills.iter().filter(|skill| skill.always).count() == 2);
}

#[test]
fn a_reader_who_is_not_the_controller_gets_the_record_read_only() {
    let (_, frame) = booted(vec![agent("Reviewer Bot", "active", false)], "9");
    let frame = tick_native(press(&frame, "Reviewer Bot"));
    assert!(
        has_text(&frame, "Only this agent's controller can change its record. You are reading it."),
        "{:?}",
        texts(&frame)
    );
    // the grants and skills still read, the controls do not
    assert!(has_text(&frame, "ducktape"));
    assert!(has_text(&frame, "on demand"));
    assert!(!has_text(&frame, "Save"), "{:?}", texts(&frame));
    assert!(!has_text(&frame, "Pause"), "{:?}", texts(&frame));
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
}

#[test]
fn the_controller_pauses_a_record_by_registry_id() {
    let (_, frame) = booted(vec![agent("Reviewer Bot", "active", false)], "7");
    let frame = tick_native(press(&frame, "Reviewer Bot"));
    let frame = tick_native(press(&frame, "Pause agent"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "agents.status");
    assert_eq!(
        serde_json::from_slice::<Status>(&intent.payload).expect("decodes"),
        Status {
            agent_id: "reviewer-bot".into(),
            paused: true
        }
    );
}

#[test]
fn a_new_agent_registers_from_the_form_once_its_id_is_a_label() {
    let (_, frame) = booted(vec![agent("Reviewer Bot", "active", false)], "7");
    let frame = tick_native(press(&frame, "New agent"));
    assert!(has_text(&frame, AGENT_ID_HINT), "{:?}", texts(&frame));

    let frame = tick_native(type_into(&frame, AGENT_ID_HINT, "Chief Duck"));
    assert!(
        texts(&frame)
            .iter()
            .any(|text| text.starts_with("An agent id is a lowercase DNS label")),
        "{:?}",
        texts(&frame)
    );
    let frame = tick_native(type_into(&frame, AGENT_ID_HINT, "chiefduck"));
    let frame = tick_native(type_into(&frame, "display name…", "ChiefDuck"));
    let frame = tick_native(pick(&frame, CAPABILITY_PICK, "claude"));
    let frame = tick_native(toggle(&frame, "chat.post", true));
    let frame = tick_native(type_into(&frame, "skill name (its mount directory)…", "chiefduck"));
    let frame = tick_native(toggle(&frame, "load always (persona)", true));
    let frame = tick_native(press(&frame, "Add skill"));
    let frame = tick_native(press(&frame, "Register agent"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "agents.register");
    let draft: Draft = serde_json::from_slice(&intent.payload).expect("decodes");
    assert_eq!(draft.agent_id, "chiefduck");
    assert_eq!(draft.display_name, "ChiefDuck");
    assert_eq!(draft.capability, "claude");
    assert_eq!(draft.allowed_actions, ["chat.post"]);
    // a skill named without a prefix lands in the shared library
    assert_eq!(
        draft.skills,
        [AgentSkill {
            name: "chiefduck".into(),
            source_prefix: "/shared/skills/chiefduck".into(),
            source_snapshot: String::new(),
            always: true,
        }]
    );
}

#[test]
fn a_committed_write_reseeds_the_open_record_from_its_fresh_row() {
    let (subscription, frame) = booted(vec![agent("Reviewer Bot", "active", false)], "7");
    let frame = tick_native(press(&frame, "Reviewer Bot"));
    let _ = tick_native(toggle(&frame, "tasks.create", true));

    // the app committed a write and re-read the register: the row now names
    // the agent differently, and the drafts follow the row, not the reader
    let mut renamed = agent("Reviewer Bot", "paused", false);
    renamed.name = "Renamed Bot".into();
    let frame = tick_native(vec![item(subscription, &register(vec![renamed], "7", 1))]);
    let frame = tick_native(press(&frame, "Save agent"));
    let draft: Draft = serde_json::from_slice(&one_intent(&frame).payload).expect("decodes");
    assert_eq!(draft.display_name, "Renamed Bot");
    assert_eq!(draft.allowed_actions, ["chat.post"], "the unsaved tick was consumed");
}
