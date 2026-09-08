//! The facts the host pushes are what the screen shows; every act leaves as
//! an intent carrying what the reader chose or typed, and the two composers
//! are slots the host paints, keyed by the room and the thread.

use chat_view::host::{
    Channel, ChatBlock, ChatChannel, ChatMessage, ChatProps, ChatSidebarRow, Query, Selection, Text,
};
use chat_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, press, submit, texts, type_into};
use ui_lang_guest::wire::{Frame, Node, SurfaceValue};

fn message(seq: i64, body: &str) -> ChatMessage {
    ChatMessage {
        id: format!("m{seq}"),
        view_key: seq,
        seq,
        author: "mallard".into(),
        meta: "h 84,912".into(),
        body: body.into(),
        blocks: vec![ChatBlock {
            kind: "paragraph".into(),
            text: body.into(),
            ..ChatBlock::default()
        }],
        show_author: true,
        initial: "M".into(),
        avatar_kind: "human".into(),
        height: 84_912,
        time: 84_912,
        rev: 1,
        ..ChatMessage::default()
    }
}

fn facts() -> ChatProps {
    ChatProps {
        endpoint: "http://127.0.0.1:1".into(),
        network_name: "testnet".into(),
        network_chain_id: "testnet#abcd".into(),
        status: "Live".into(),
        block_height: 84_912,
        search_phase: "idle".into(),
        rooms: vec![
            ChatSidebarRow {
                channel: ChatChannel {
                    id: "channel-a".into(),
                    name: "general".into(),
                    ..ChatChannel::default()
                },
                unread: false,
            },
            ChatSidebarRow {
                channel: ChatChannel {
                    id: "channel-b".into(),
                    name: "ops".into(),
                    ..ChatChannel::default()
                },
                unread: true,
            },
        ],
        connected: true,
        active_channel: "channel-a".into(),
        active_channel_name: "general".into(),
        messages: vec![message(1, "first light"), message(2, "second wind")],
        at_live_tail: true,
        message_action: "toolbar".into(),
        thread_message_action: "toolbar".into(),
        copy_surface: "nowhere".into(),
        ..ChatProps::default()
    }
}

fn encoded(props: &ChatProps) -> Vec<u8> {
    serde_json::to_vec(props).expect("props encode")
}

/// Boot and push the facts; returns the subscription id and the frame.
fn shown(props: &ChatProps) -> (u64, Frame) {
    boot_native();
    let frame = tick_native(Vec::new());
    assert_eq!(frame.requests[0].kind, "chat.props");
    let subscription = frame.requests[0].id;
    let frame = tick_native(vec![item(subscription, &encoded(props))]);
    (subscription, frame)
}

/// The one intent a frame carries — a focus task the view asks of the host
/// beside it is not one.
fn one_intent(frame: &Frame) -> &ui_lang_guest::wire::Request {
    let intents: Vec<_> = frame
        .requests
        .iter()
        .filter(|request| request.kind.starts_with("chat."))
        .collect();
    let [intent] = intents.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    intent
}

/// Every host surface in the tree: its name and its first (key) argument.
fn surfaces(node: &Node, out: &mut Vec<(String, String)>) {
    if let Node::Surface { name, args, .. } = node {
        let scope = match args.first() {
            Some(SurfaceValue::Str(scope)) => scope.clone(),
            other => panic!("a scope string first, got {other:?}"),
        };
        out.push((name.clone(), scope));
    }
    for child in node.children() {
        surfaces(child, out);
    }
}

#[test]
fn the_facts_the_host_pushes_are_what_the_screen_shows_and_a_room_leaves_as_a_choice() {
    let (_, frame) = shown(&facts());
    for expected in [
        "testnet",
        "h 84,912",
        "general",
        "ops",
        "first light",
        "second wind",
    ] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
    let frame = tick_native(press(&frame, "ops"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "chat.choose_channel");
    assert_eq!(
        serde_json::from_slice::<Channel>(&intent.payload).expect("decodes"),
        Channel {
            id: "channel-b".into()
        }
    );
}

#[test]
fn a_search_leaves_as_the_typed_query_and_the_composer_is_the_rooms_slot() {
    let (subscription, frame) = shown(&facts());
    let mut slots = Vec::new();
    surfaces(frame.root.as_ref().expect("a tree"), &mut slots);
    assert_eq!(
        slots,
        [(
            "chat_composer".to_owned(),
            "http://127.0.0.1:1\u{1f}channel-a".to_owned()
        )]
    );
    let frame = tick_native(type_into(&frame, "Search…", "  light  "));
    assert!(frame.requests.is_empty(), "typing runs no handler");
    let frame = tick_native(submit(&frame, "Search…"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "chat.search");
    assert_eq!(
        serde_json::from_slice::<Query>(&intent.payload).expect("decodes"),
        Query {
            query: "light".into()
        }
    );
    // an open thread seats the rail's own composer beside the room's
    let threaded = ChatProps {
        active_thread_seq: 1,
        thread_messages: vec![message(1, "first light")],
        ..facts()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&threaded))]);
    let mut slots = Vec::new();
    surfaces(frame.root.as_ref().expect("a tree"), &mut slots);
    assert_eq!(
        slots
            .iter()
            .map(|(_, scope)| scope.as_str())
            .collect::<Vec<_>>(),
        [
            "http://127.0.0.1:1\u{1f}channel-a",
            "http://127.0.0.1:1\u{1f}channel-a#1"
        ]
    );
}

#[test]
fn an_edit_is_seeded_from_the_message_and_leaves_as_the_edited_text() {
    let (subscription, _) = shown(&facts());
    // the host opened the menu on message 2 (the ⋯ press went through it)
    let menu = ChatProps {
        selected_message_seq: 2,
        selected_message_rev: 1,
        message_action: "more".into(),
        ..facts()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&menu))]);
    let frame = tick_native(press(&frame, "Edit message"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "chat.begin_edit");
    assert_eq!(
        serde_json::from_slice::<Selection>(&intent.payload).expect("decodes"),
        Selection {
            seq: 2,
            body: String::new(),
            rev: 1
        }
    );
    // the host seats the edit; the field carries the body the view seeded
    let editing = ChatProps {
        message_action: "editing".into(),
        ..menu
    };
    let frame = tick_native(vec![item(subscription, &encoded(&editing))]);
    let frame = tick_native(type_into(&frame, "Edit message", "second wind, revised"));
    let frame = tick_native(submit(&frame, "Edit message"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "chat.edit");
    assert_eq!(
        serde_json::from_slice::<Text>(&intent.payload).expect("decodes"),
        Text {
            text: "second wind, revised".into()
        }
    );
}
