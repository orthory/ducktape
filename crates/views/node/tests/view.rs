//! The facts the host pushes are what the screen shows; a tab, the log
//! filter and a copy leave as intents, and the Activity tab leaves the
//! log ring's slot for the host to paint.

use node_view::host::{Copy, LogFilter, ModuleRow, NodeProps, PeerRow, Tab};
use node_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, keys, press, texts, type_into};
use ui_lang_guest::wire::{Frame, Node};

fn facts() -> Vec<u8> {
    serde_json::to_vec(&NodeProps {
        node_key: "ab12cd34".into(),
        node_data_dir: "/var/ducktape/demo".into(),
        tier: "validator".into(),
        admin: true,
        status: "Live".into(),
        loading: false,
        module_rows: vec![ModuleRow {
            id: "chat".into(),
            category: "workspace".into(),
            root: "9f3e".into(),
            code_hash: "77aa".into(),
            pending_hash: String::new(),
            activation_height: 0,
            readiness: 0,
            ready: true,
        }],
        node_height: 84_912,
        node_checkpoint: 84_900,
        node_last_finalized: 1_700_000_000,
        node_reachable_label: "3".into(),
        node_quorum_label: "3".into(),
        node_version: "0.4.2".into(),
        node_root_hash: "c0ffee".into(),
        sync_line: "live".into(),
        node_phase_since: 1_700_000_000,
        node_sync_retries: 0,
        node_sync_failures: 0,
        node_sync_last_error: String::new(),
        node_peers: vec![PeerRow {
            key: "peer-1".into(),
            role: "validator".into(),
            live: true,
        }],
        wall_now: 1_700_000_030,
        connected: true,
        dark: false,
    })
    .expect("props encode")
}

/// Boot and push the facts.
fn shown() -> Frame {
    boot_native();
    let frame = tick_native(Vec::new());
    assert_eq!(frame.requests[0].kind, "node.props");
    assert!(has_text(&frame, "Not connected"), "{:?}", texts(&frame));
    let subscription = frame.requests[0].id;
    tick_native(vec![item(subscription, &facts())])
}

/// Every host surface in the tree, by name.
fn surfaces(node: &Node, out: &mut Vec<String>) {
    if let Node::Surface { name, .. } = node {
        out.push(name.clone());
    }
    for child in node.children() {
        surfaces(child, out);
    }
}

#[test]
fn the_facts_the_host_pushes_are_what_the_overview_shows() {
    let frame = shown();
    for expected in [
        "This node",
        "public key",
        "ab12cd34",
        "/var/ducktape/demo",
        "h 84,912",
        "h 84,900",
        "just now",
        "3 / 3",
        "peer-1",
    ] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
}

#[test]
fn a_tab_leaves_as_an_intent_and_the_activity_tab_leaves_the_log_ring_to_the_host() {
    let frame = shown();
    let frame = tick_native(press(&frame, "Node activity"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "node.tab");
    assert_eq!(
        serde_json::from_slice::<Tab>(&intent.payload).expect("decodes"),
        Tab {
            tab: "activity".into()
        }
    );
    assert!(has_text(&frame, "Log ring"), "{:?}", texts(&frame));
    let mut names = Vec::new();
    surfaces(frame.root.as_ref().expect("a tree"), &mut names);
    assert_eq!(names, ["node_log_timeline"], "{:?}", keys(&frame));

    let frame = tick_native(type_into(&frame, "filter logs…", "warn"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "node.log_filter");
    assert_eq!(
        serde_json::from_slice::<LogFilter>(&intent.payload).expect("decodes"),
        LogFilter {
            filter: "warn".into()
        }
    );
}

#[test]
fn the_modules_tab_lists_the_register_and_the_key_leaves_as_a_copy() {
    let frame = shown();
    let frame = tick_native(press(&frame, "Node modules"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "node.tab");
    assert!(has_text(&frame, "chat"), "{:?}", texts(&frame));

    let frame = tick_native(press(&frame, "Node overview"));
    let frame = tick_native(press(&frame, "Copy node key"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "node.copy");
    assert_eq!(
        serde_json::from_slice::<Copy>(&intent.payload).expect("decodes"),
        Copy {
            text: "ab12cd34".into(),
            label: "Node key copied".into()
        }
    );
}
