//! The register the host pushes is what the screen shows; a record opens as
//! an editor for its controller and as a reading for everyone else, and the
//! only things that leave are the writes the reader asked for.

use agents_view::host::{
    AgentRow, AgentSkill, AgentsProps, Draft, JournalEntry, LiveActivity, LiveRun, OpenLink,
    OpenRun, RunJournal, RunLink, RunRow, Status,
};
use agents_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, pick, press, texts, toggle, type_into};
use ui_lang_guest::wire::{Frame, Node, Request};

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

fn run(run_id: &str, agent: &str, state: &str) -> RunRow {
    RunRow {
        run_id: run_id.into(),
        dispatch_id: format!("dispatch-of-{}", run_id.replace('\x1f', "-")),
        agent_id: agent.to_lowercase(),
        agent_name: agent.into(),
        origin: "#general · msg 12".into(),
        state: state.into(),
        dispatched: "h 84,912".into(),
        settled: String::new(),
        attempt: 0,
        holder: "ab12cd34ef56ab12…".into(),
        actions: 2,
        degraded: false,
        reason: String::new(),
        output_ref: String::new(),
        pr_number: 0,
    }
}

fn register(rows: Vec<AgentRow>, account: &str, committed: i64) -> Vec<u8> {
    register_with_runs(
        rows,
        Vec::new(),
        "",
        0,
        RunJournal::default(),
        account,
        committed,
    )
}

fn register_with_runs(
    rows: Vec<AgentRow>,
    runs: Vec<RunRow>,
    open_run: &str,
    opened: i64,
    journal: RunJournal,
    account: &str,
    committed: i64,
) -> Vec<u8> {
    register_with_live(
        rows,
        runs,
        open_run,
        opened,
        journal,
        LiveRun::default(),
        account,
        committed,
    )
}

#[allow(clippy::too_many_arguments)]
fn register_with_live(
    rows: Vec<AgentRow>,
    runs: Vec<RunRow>,
    open_run: &str,
    opened: i64,
    journal: RunJournal,
    live: LiveRun,
    account: &str,
    committed: i64,
) -> Vec<u8> {
    serde_json::to_vec(&AgentsProps {
        rows,
        runs,
        open_run: open_run.into(),
        opened,
        journal,
        live,
        capabilities: vec!["claude".into(), "codex".into()],
        account: account.into(),
        committed,
        connected: true,
        answered: true,
        dark: false,
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

/// Whether a button with this accessible name is on the frame. A chip's
/// text is on the frame whether or not it is offered as a link, so the
/// difference is a button node, not a text.
fn frame_has_button(frame: &Frame, name: &str) -> bool {
    fn walk(node: &Node, name: &str) -> bool {
        let named = matches!(node, Node::Button { label, .. } if label.as_deref() == Some(name));
        named || node.children().iter().any(|child| walk(child, name))
    }
    frame.root.as_ref().is_some_and(|root| walk(root, name))
}

fn node_ending<'a>(frame: &'a Frame, suffix: &str) -> &'a Node {
    fn find<'a>(node: &'a Node, suffix: &str) -> Option<&'a Node> {
        if node.key().is_some_and(|key| key.ends_with(suffix)) {
            return Some(node);
        }
        node.children().iter().find_map(|child| find(child, suffix))
    }
    find(frame.root.as_ref().unwrap(), suffix).expect("node exists")
}

#[test]
fn journal_drag_and_receipt_disclosure_keep_identifiers_out_of_the_summary() {
    use ui_lang_guest::wire::{Event, Length, mouse};
    let (subscription, _) = booted(vec![], "7");
    let mut running = run("peer", "Claude", "running");
    running.dispatch_id = "32a29e72a8fc5b673f196f93ab63a18b8cef8f47f94ceac9c1bb7c1".into();
    let target = RunLink {
        relation: "target".into(),
        kind: "chat".into(),
        label: "#Engineering · Eddy: Bound and scroll the branch selector".into(),
        url: "duck://channel/engineering?net=a1b2c3d4#12".into(),
    };
    let journal = RunJournal {
        dispatch_id: running.dispatch_id.clone(),
        entries: vec![JournalEntry {
            height: "h 123".into(),
            kind: "action".into(),
            summary: "React 👀".into(),
            status: "Completed".into(),
            targets: vec![target.clone()],
        }],
        links: vec![RunLink {
            relation: "from".into(),
            kind: "chat".into(),
            label: "Bound and scroll the branch selector without overflowing the button".into(),
            url: "duck://channel/general/12".into(),
        }],
    };
    let frame = tick_native(vec![item(
        subscription,
        &register_with_runs(
            vec![],
            vec![running.clone()],
            &running.dispatch_id,
            1,
            journal,
            "7",
            0,
        ),
    )]);
    assert!(has_text(&frame, "React 👀"));
    assert!(has_text(&frame, "Completed"));
    assert!(has_text(&frame, &target.label));
    assert!(!has_text(&frame, &running.dispatch_id));
    fn check_place(node: &Node) -> bool {
        if let Node::Button {
            label: Some(label),
            width,
            height,
            ..
        } = node
            && label.starts_with("Bound and scroll")
        {
            assert_eq!(*width, Some(Length::Fill));
            assert_eq!(*height, None, "a wrapped title grows its button");
            return true;
        }
        node.children().iter().any(check_place)
    }
    assert!(check_place(frame.root.as_ref().unwrap()));
    let frame = tick_native(press(&frame, &target.label));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "agents.open_link");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&intent.payload).unwrap()["url"],
        target.url
    );
    let frame = tick_native(press(&frame, "Run details"));
    assert!(has_text(&frame, &running.dispatch_id));
    let width = |frame: &Frame| match node_ending(frame, "/journal") {
        Node::Container {
            width: Some(Length::Fixed(width)),
            ..
        } => *width,
        node => panic!("fixed journal width: {node:?}"),
    };
    assert_eq!(width(&frame), 400.0);
    let Node::ResizeHandle {
        on_drag: Some(handler),
        cursor,
        ..
    } = node_ending(&frame, "/journal-resize")
    else {
        panic!("resize handle")
    };
    assert_eq!(*cursor, Some(mouse::Cursor::ResizingHorizontally));
    let frame = tick_native(vec![Event::Drag {
        handler: *handler,
        dx: -80.0,
        dy: 0.0,
    }]);
    assert_eq!(width(&frame), 480.0);
    let frame = tick_native(vec![
        Event::Mouse {
            event: mouse::Event::ButtonReleased(mouse::Button::Left),
            captured: true,
        },
        Event::Mouse {
            event: mouse::Event::CursorMoved { x: 500.0, y: 30.0 },
            captured: true,
        },
    ]);
    assert_eq!(width(&frame), 480.0);
    assert_eq!(
        agents_view::host::journal_width_after_delta(480.0, 900.0, 900.0),
        570.0
    );
    assert_eq!(
        agents_view::host::journal_width_after_delta(480.0, -900.0, 900.0),
        280.0
    );
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
        // the count derives from the record: three skills
        "3",
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
    for expected in ["Identity", "Executor", "Skills", "review", "style"] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    for gone in ["Actions", "Grants"] {
        assert!(
            !has_text(&frame, gone),
            "the record has no {gone} to edit: {:?}",
            texts(&frame)
        );
    }
    assert!(!has_text(&frame, "Only this agent's controller"));

    // pick the executor and flip a skill to the persona — every edit is a
    // draft until the save
    let frame = tick_native(pick(&frame, CAPABILITY_PICK, "claude"));
    let frame = tick_native(press(&frame, "Load always"));
    assert!(
        frame.requests.is_empty(),
        "drafts leave nothing: {:?}",
        frame.requests
    );

    let frame = tick_native(press(&frame, "Save agent"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "agents.save");
    let draft: Draft = serde_json::from_slice(&intent.payload).expect("decodes");
    assert_eq!(draft.agent_id, "reviewer-bot");
    assert_eq!(draft.display_name, "Reviewer Bot");
    assert_eq!(draft.capability, "claude");
    assert!(draft.skills.iter().filter(|skill| skill.always).count() == 2);
}

#[test]
fn a_reader_who_is_not_the_controller_gets_the_record_read_only() {
    let (_, frame) = booted(vec![agent("Reviewer Bot", "active", false)], "9");
    let frame = tick_native(press(&frame, "Reviewer Bot"));
    assert!(
        has_text(
            &frame,
            "Only this agent's controller can change its record. You are reading it."
        ),
        "{:?}",
        texts(&frame)
    );
    // the skills still read, the controls do not
    assert!(has_text(&frame, "review"));
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
    let frame = tick_native(press(&frame, "Runs"));
    assert!(has_text(&frame, "New agent"), "{:?}", texts(&frame));
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
    let frame = tick_native(type_into(
        &frame,
        "skill name (its mount directory)…",
        "chiefduck",
    ));
    let frame = tick_native(toggle(&frame, "load always (persona)", true));
    let frame = tick_native(press(&frame, "Add skill"));
    let frame = tick_native(press(&frame, "Register agent"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "agents.register");
    let draft: Draft = serde_json::from_slice(&intent.payload).expect("decodes");
    assert_eq!(draft.agent_id, "chiefduck");
    assert_eq!(draft.display_name, "ChiefDuck");
    assert_eq!(draft.capability, "claude");
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
    let _ = tick_native(press(&frame, "Load always"));

    // the app committed a write and re-read the register: the row now names
    // the agent differently, and the drafts follow the row, not the reader
    let mut renamed = agent("Reviewer Bot", "paused", false);
    renamed.name = "Renamed Bot".into();
    let frame = tick_native(vec![item(subscription, &register(vec![renamed], "7", 1))]);
    let frame = tick_native(press(&frame, "Save agent"));
    let draft: Draft = serde_json::from_slice(&one_intent(&frame).payload).expect("decodes");
    assert_eq!(draft.display_name, "Renamed Bot");
    assert_eq!(
        draft.skills.iter().filter(|skill| skill.always).count(),
        1,
        "the unsaved persona flip was consumed"
    );
}

#[test]
fn the_runs_panel_lists_every_run_and_opens_one_journal_at_a_time() {
    boot_native();
    let frame = tick_native(Vec::new());
    let subscription = frame.requests[0].id;
    let running = run("chat\x1fgeneral\x1f12\x1freviewer", "Reviewer", "running");
    let mut failed = run("chat\x1fgeneral\x1f9\x1freviewer", "Reviewer", "failed");
    failed.reason = "worker exploded".into();
    failed.settled = "h 84,920".into();
    let frame = tick_native(vec![item(
        subscription,
        &register_with_runs(
            vec![agent("Reviewer", "active", true)],
            vec![running.clone(), failed.clone()],
            "",
            0,
            RunJournal::default(),
            "7",
            0,
        ),
    )]);
    // the registry is the first panel; the tracker is one press away
    assert!(has_text(&frame, "New agent"));
    assert!(!has_text(&frame, "Messages"));
    assert!(
        !has_text(&frame, "#general · msg 12"),
        "{:?}",
        texts(&frame)
    );
    let frame = tick_native(press(&frame, "Runs"));
    assert!(has_text(&frame, "New agent"));
    assert!(!has_text(&frame, "Messages"));
    assert!(
        !texts(&frame)
            .iter()
            .any(|text| text.starts_with("A run is a dispatch"))
    );
    for expected in [
        "2 runs · 1 in flight",
        "#general · msg 12",
        "running",
        "failed",
        "h 84,912",
    ] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);

    // opening a run asks the app for its journal, by the run's address
    let frame = tick_native(press(&frame, &failed.run_id));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "agents.open_run");
    assert_eq!(
        serde_json::from_slice::<OpenRun>(&intent.payload).expect("decodes"),
        OpenRun {
            dispatch_id: failed.dispatch_id.clone()
        }
    );
    assert!(
        has_text(&frame, "Reading the journal…"),
        "{:?}",
        texts(&frame)
    );
    assert!(has_text(&frame, "worker exploded"), "{:?}", texts(&frame));

    // the journal lands under the open run's address and reads fact by fact
    let journal = RunJournal {
        dispatch_id: failed.dispatch_id.clone(),
        links: Vec::new(),
        entries: vec![
            JournalEntry {
                height: "h 84,912".into(),
                kind: "dispatched".into(),
                summary: "for reviewer from #general · msg 9".into(),
                ..JournalEntry::default()
            },
            JournalEntry {
                height: "h 84,920".into(),
                kind: "settled".into(),
                summary: "failed: worker exploded".into(),
                ..JournalEntry::default()
            },
        ],
    };
    let frame = tick_native(vec![item(
        subscription,
        &register_with_runs(
            vec![agent("Reviewer", "active", true)],
            vec![running, failed.clone()],
            &failed.dispatch_id,
            1,
            journal,
            "7",
            0,
        ),
    )]);
    assert!(
        !has_text(&frame, "Reading the journal…"),
        "{:?}",
        texts(&frame)
    );
    assert!(
        has_text(&frame, "for reviewer from #general · msg 9"),
        "{:?}",
        texts(&frame)
    );
    assert!(
        has_text(&frame, "failed: worker exploded"),
        "{:?}",
        texts(&frame)
    );

    // closing tells the app to stop reading it
    let frame = tick_native(press(&frame, "Close journal"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "agents.open_run");
    assert_eq!(
        serde_json::from_slice::<OpenRun>(&intent.payload).expect("decodes"),
        OpenRun {
            dispatch_id: String::new()
        }
    );
    assert!(
        !has_text(&frame, "for reviewer from #general · msg 9"),
        "{:?}",
        texts(&frame)
    );
}

/// THE APP OWNS WHICH RUN IS OPEN. A chat hint, a bell or a duck://run link
/// opens a run from another tab, so the register's `open_run` opens the
/// tracker here without a press, whichever panel the reader was on — and the
/// panel it opens is keyed by the run's address, which is also the key its
/// journal and live reading carry. The run stays open while the reader looks
/// at another panel, and a second door onto that same run lands them on the
/// tracker again: the landing follows the door count, not the run's name.
#[test]
fn the_register_opens_the_run_the_app_names() {
    boot_native();
    let frame = tick_native(Vec::new());
    let subscription = frame.requests[0].id;
    let running = run("chat\x1fgeneral\x1f12\x1freviewer", "Reviewer", "running");
    let opened_by_the_app = |opened: i64| {
        item(
            subscription,
            &register_with_runs(
                vec![agent("Reviewer", "active", true)],
                vec![running.clone()],
                &running.dispatch_id,
                opened,
                RunJournal::default(),
                "7",
                0,
            ),
        )
    };
    let frame = tick_native(vec![opened_by_the_app(1)]);
    assert!(
        has_text(&frame, "Reading the journal…"),
        "the tracker is landed on without a press: {:?}",
        texts(&frame)
    );
    assert!(has_text(&frame, "Details"));
    assert!(!has_text(&frame, &running.dispatch_id));
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);

    // the reader looks at the registry; the run stays open behind it
    let frame = tick_native(press(&frame, "Registry"));
    assert!(
        !has_text(&frame, "Reading the journal…"),
        "{:?}",
        texts(&frame)
    );
    let frame = tick_native(vec![opened_by_the_app(1)]);
    assert!(
        !has_text(&frame, "Reading the journal…"),
        "the register alone moves no panel: {:?}",
        texts(&frame)
    );

    // the same run, through another door: the tracker again
    let frame = tick_native(vec![opened_by_the_app(2)]);
    assert!(
        has_text(&frame, "Reading the journal…"),
        "a door onto the open run lands on the tracker: {:?}",
        texts(&frame)
    );
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
}

/// THE RUN AS IT RUNS, AND EVERYTHING IT TOUCHED. The live reading draws in
/// the panel — the chat stream only hints — and the journal's places draw as
/// chips: a chip with an address opens it through the app's open plane, a
/// place the protocol cannot address yet is a label alone.
#[test]
fn the_open_run_draws_its_progress_and_its_places_as_chips() {
    boot_native();
    let frame = tick_native(Vec::new());
    let subscription = frame.requests[0].id;
    let running = run("chat\x1fgeneral\x1f12\x1freviewer", "Reviewer", "running");
    let journal = RunJournal {
        dispatch_id: running.dispatch_id.clone(),
        entries: vec![JournalEntry {
            height: "h 84,912".into(),
            kind: "dispatched".into(),
            summary: "for reviewer from #general · msg 12".into(),
            ..JournalEntry::default()
        }],
        links: vec![
            RunLink {
                relation: "from".into(),
                kind: "chat".into(),
                label: "#general · msg 12".into(),
                url: "duck://channel/general/12?net=duck-1".into(),
            },
            RunLink {
                relation: "touched".into(),
                kind: "page".into(),
                label: "Release notes".into(),
                url: "duck://page/p-9?net=duck-1".into(),
            },
            RunLink {
                relation: "touched".into(),
                kind: "task".into(),
                label: "task t-4".into(),
                url: String::new(),
            },
        ],
    };
    let live = LiveRun {
        present: true,
        status: "Reading the repo".into(),
        activity: vec![
            LiveActivity {
                label: "Command: cargo test".into(),
                done: true,
            },
            LiveActivity {
                label: "Reasoning".into(),
                done: false,
            },
        ],
        answer_preview: "The notes are drafted".into(),
    };
    let frame = tick_native(vec![item(
        subscription,
        &register_with_live(
            vec![agent("Reviewer", "active", true)],
            vec![running.clone()],
            &running.dispatch_id,
            1,
            journal,
            live,
            "7",
            0,
        ),
    )]);
    for expected in [
        "Reading the repo",
        "Command: cargo test",
        "Reasoning",
        "The notes are drafted",
        "Relevant",
        "#general · msg 12",
        "Release notes",
        "task t-4",
    ] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    // a chip with an address is a link the app's open plane follows
    let frame = tick_native(press(&frame, "Release notes"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "agents.open_link");
    assert_eq!(
        serde_json::from_slice::<OpenLink>(&intent.payload).expect("decodes"),
        OpenLink {
            url: "duck://page/p-9?net=duck-1".into()
        }
    );
    // a place without an address is drawn, not offered
    assert!(
        !frame_has_button(&frame, "task t-4"),
        "an unaddressed place was offered as a link: {:?}",
        texts(&frame)
    );
}
