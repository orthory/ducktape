//! The facts the host pushes are what the screen shows; every act leaves as
//! an intent, the fold is the view's own, and the three things only the
//! host can draw are left as its slots.

use shell_view::host::{AgentActivity, AgentChatEntry, ShellProps, Surface};
use shell_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, press, texts};
use ui_lang_guest::wire::{Frame, Node};

fn facts() -> ShellProps {
    ShellProps {
        dark: false,
        connected: true,
        surface: "tasks".into(),
        setup_open: false,
        identity_options: vec!["team-codex · Codex".into()],
        identity: "team-codex · Codex".into(),
        provider_initial: "C".into(),
        credential: "team-codex".into(),
        host_node_options: vec!["This node".into()],
        host_node: "This node".into(),
        credentials_loading: false,
        terminal_running: false,
        terminal_busy: false,
        terminal_title: String::new(),
        terminal_error: String::new(),
        entries: Vec::new(),
        activity: Vec::new(),
        chat_busy: false,
        chat_status: String::new(),
        chat_detail: String::new(),
        live: String::new(),
        saga_id: String::new(),
        detached_saga: String::new(),
        run_line: "team-codex · Codex · This node".into(),
        grant_note: String::new(),
        terminal_note: "A sandboxed Codex session.".into(),
        composer_hint: "Message Codex…".into(),
        task_blurb: "Each message runs an agent in a sandbox on this node.".into(),
        register_hint: "Register one with `ducktape user cred add codex`".into(),
    }
}

fn settled_turn() -> Vec<AgentChatEntry> {
    vec![
        AgentChatEntry {
            id: 1,
            role: "user".into(),
            body: "Explain the execution path.".into(),
            ..Default::default()
        },
        AgentChatEntry {
            id: 2,
            role: "assistant".into(),
            body: "## Execution path\n\nThe request becomes a durable saga.".into(),
            provider_label: "Codex".into(),
            provider_initial: "C".into(),
            status: "done".into(),
            steps: vec![AgentActivity {
                id: 7,
                title: "Command".into(),
                detail: "cargo test".into(),
                status: "done".into(),
            }],
            steps_label: "1 step · 1 command".into(),
            ..Default::default()
        },
    ]
}

fn encoded(props: &ShellProps) -> Vec<u8> {
    serde_json::to_vec(props).expect("props encode")
}

/// Boot and push the facts; returns the subscription id and the frame.
fn shown(props: &ShellProps) -> (u64, Frame) {
    boot_native();
    let frame = tick_native(Vec::new());
    assert_eq!(frame.requests[0].kind, "shell.props");
    let subscription = frame.requests[0].id;
    let frame = tick_native(vec![item(subscription, &encoded(props))]);
    (subscription, frame)
}

fn one_intent(frame: &Frame) -> &ui_lang_guest::wire::Request {
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    intent
}

fn surfaces(frame: &Frame) -> Vec<String> {
    fn walk(node: &Node, out: &mut Vec<String>) {
        if let Node::Surface { name, .. } = node {
            out.push(name.clone());
        }
        for child in node.children() {
            walk(child, out);
        }
    }
    let mut names = Vec::new();
    if let Some(root) = &frame.root {
        walk(root, &mut names);
    }
    names
}

#[test]
fn the_facts_the_host_pushes_are_what_the_screen_shows_and_a_switch_leaves_as_an_intent() {
    let (_, frame) = shown(&facts());
    for expected in [
        "Shell",
        "team-codex · Codex · This node",
        "What should the agent do?",
    ] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
    // the composer is the host's slot; the transcript has no other
    assert_eq!(surfaces(&frame), ["shell_composer"]);
    let frame = tick_native(press(&frame, "Terminal"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "shell.surface");
    assert_eq!(
        serde_json::from_slice::<Surface>(&intent.payload).expect("decodes"),
        Surface {
            surface: "terminal".into()
        }
    );
}

#[test]
fn the_terminal_surface_is_the_hosts_and_only_while_a_session_runs() {
    let running = ShellProps {
        surface: "terminal".into(),
        terminal_running: true,
        terminal_title: "codex · team-codex".into(),
        ..facts()
    };
    let (subscription, frame) = shown(&running);
    assert!(
        has_text(&frame, "codex · team-codex"),
        "{:?}",
        texts(&frame)
    );
    assert_eq!(surfaces(&frame), ["agent_terminal_surface"]);
    let stopped = ShellProps {
        terminal_running: false,
        terminal_title: String::new(),
        ..running
    };
    let frame = tick_native(vec![item(subscription, &encoded(&stopped))]);
    assert!(surfaces(&frame).is_empty(), "{:?}", surfaces(&frame));
    assert!(has_text(&frame, "No session open"), "{:?}", texts(&frame));
}

#[test]
fn a_settled_turn_draws_its_answer_in_the_hosts_markdown_and_folds_its_work_in_the_view() {
    let settled = ShellProps {
        entries: settled_turn(),
        ..facts()
    };
    let (_, frame) = shown(&settled);
    assert_eq!(surfaces(&frame), ["agent_markdown", "shell_composer"]);
    assert!(
        has_text(&frame, "1 step · 1 command"),
        "{:?}",
        texts(&frame)
    );
    assert!(!has_text(&frame, "cargo test"));
    // the fold is the view's own: opening it runs no intent
    let frame = tick_native(press(&frame, "Show what the agent did"));
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
    assert!(has_text(&frame, "cargo test"), "{:?}", texts(&frame));
    let frame = tick_native(press(&frame, "Show what the agent did"));
    assert!(!has_text(&frame, "cargo test"));
    let frame = tick_native(press(&frame, "New chat"));
    assert_eq!(one_intent(&frame).kind, "shell.reset");
}
