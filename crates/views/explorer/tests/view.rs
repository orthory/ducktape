//! The ledger the host pushes is what the screen shows; a search leaves as
//! an intent and its answer comes back as props, with the zero-hit plate
//! speaking only for the query that was sent.

use explorer_view::host::{
    Copy, ExplorerBlock, ExplorerHit, ExplorerOp, ExplorerProps, KindCount, Search,
};
use explorer_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, press, submit, texts, type_into};
use ui_lang_guest::wire::Frame;

fn ledger() -> ExplorerProps {
    ExplorerProps {
        connected: true,
        loading: false,
        dark: false,
        blocks: vec![ExplorerBlock {
            height: 84_912,
            hash: "9f3e".into(),
            commit: "c0ffee".into(),
            op_count: 1,
        }],
        ops: vec![ExplorerOp {
            height: 84_912,
            proposer: "val-1".into(),
            target: "chat".into(),
            disposition: "applied".into(),
            op_hash: "ab12cd34".into(),
            payload: "post".into(),
            trace: "chat · 1 msg".into(),
        }],
        head: 84_912,
        sync_line: "live".into(),
        hits: Vec::new(),
        kinds: Vec::new(),
        partial: String::new(),
        searching: false,
        sent_query: String::new(),
    }
}

fn encoded(props: &ExplorerProps) -> Vec<u8> {
    serde_json::to_vec(props).expect("props encode")
}

/// Boot and push the ledger; returns the subscription id and the frame.
fn shown(props: &ExplorerProps) -> (u64, Frame) {
    boot_native();
    let frame = tick_native(Vec::new());
    assert_eq!(frame.requests[0].kind, "explorer.props");
    let subscription = frame.requests[0].id;
    let frame = tick_native(vec![item(subscription, &encoded(props))]);
    (subscription, frame)
}

#[test]
fn the_ledger_the_host_pushes_is_what_the_screen_shows_and_a_block_opens_its_ops() {
    let (_, frame) = shown(&ledger());
    for expected in ["Explorer", "h 84,912"] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
    let frame = tick_native(press(&frame, "Refresh"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "explorer.refresh");
}

#[test]
fn a_search_leaves_as_an_intent_and_its_answer_comes_back_as_props() {
    let (subscription, frame) = shown(&ledger());
    let frame = tick_native(type_into(
        &frame,
        "Search messages, pages, issues, files, runs…",
        "  needle  ",
    ));
    assert!(frame.requests.is_empty(), "typing runs no handler");
    let frame = tick_native(submit(
        &frame,
        "Search messages, pages, issues, files, runs…",
    ));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "explorer.search");
    assert_eq!(
        serde_json::from_slice::<Search>(&intent.payload).expect("decodes"),
        Search {
            query: "needle".into()
        }
    );

    // a zero-hit answer stands for the query it was sent for …
    let answered = ExplorerProps {
        sent_query: "needle".into(),
        partial: String::new(),
        ..ledger()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&answered))]);
    assert!(
        has_text(&frame, "Nothing matched that query in this workspace."),
        "{:?}",
        texts(&frame)
    );
    // … and a hit lists under its kind, with the copy leaving as an intent
    let hit = ExplorerProps {
        hits: vec![ExplorerHit {
            kind: "chat".into(),
            code: "c".into(),
            title: "needle in #general".into(),
            snippet: "the needle".into(),
            meta: "chat".into(),
            target: "general".into(),
        }],
        kinds: vec![KindCount {
            kind: "chat".into(),
            label: "Chat".into(),
            count: 1,
        }],
        ..answered
    };
    let frame = tick_native(vec![item(subscription, &encoded(&hit))]);
    assert!(
        has_text(&frame, "needle in #general"),
        "{:?}",
        texts(&frame)
    );
    let frame = tick_native(press(&frame, "Clear workspace search"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "explorer.clear");
}

#[test]
fn an_ops_hash_leaves_as_a_copy() {
    let (_, frame) = shown(&ledger());
    let frame = tick_native(press(&frame, "Inspect block"));
    let frame = tick_native(press(&frame, "Copy op hash"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "explorer.copy");
    assert_eq!(
        serde_json::from_slice::<Copy>(&intent.payload).expect("decodes"),
        Copy {
            text: "ab12cd34".into(),
            label: "Op hash copied".into()
        }
    );
}

/// A block's hash is a landmark in the list and a key in the detail: twelve
/// hex chars on the row, the whole value beside a copy once the block is
/// open — and the copy carries every character.
#[test]
fn a_block_hash_reads_short_in_the_list_and_whole_with_a_copy_in_the_detail() {
    let hash = "9f3e".repeat(16);
    let commit = "c0ffee11".repeat(8);
    let props = ExplorerProps {
        blocks: vec![ExplorerBlock {
            height: 84_912,
            hash: hash.clone(),
            commit: commit.clone(),
            op_count: 1,
        }],
        ..ledger()
    };
    let (_, frame) = shown(&props);
    assert!(has_text(&frame, "9f3e9f3e9f3e…"), "{:?}", texts(&frame));
    assert!(!has_text(&frame, &hash), "the list abbreviates");
    let frame = tick_native(press(&frame, "Inspect block"));
    assert!(has_text(&frame, &hash), "{:?}", texts(&frame));
    assert!(has_text(&frame, &commit), "{:?}", texts(&frame));
    let frame = tick_native(press(&frame, "Copy block hash"));
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    assert_eq!(intent.kind, "explorer.copy");
    assert_eq!(
        serde_json::from_slice::<Copy>(&intent.payload).expect("decodes"),
        Copy {
            text: hash,
            label: "Block hash copied".into()
        }
    );
}
