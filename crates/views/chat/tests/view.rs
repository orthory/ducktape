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

/// A native tick of this screen walks a deep tree; libtest's 2 MiB thread
/// is at the edge of it in a debug build, so every test runs on its own
/// roomier stack (the wasm guest is built for release).
fn on_a_deep_stack(test: fn()) {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(test)
        .expect("the test thread spawns")
        .join()
        .expect("the test thread finishes");
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
    on_a_deep_stack(|| {
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
    });
}

#[test]
fn a_search_leaves_as_the_typed_query_and_the_composer_is_the_rooms_slot() {
    on_a_deep_stack(|| {
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
    });
}

#[test]
fn an_edit_is_seeded_from_the_message_and_leaves_as_the_edited_text() {
    on_a_deep_stack(|| {
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
    });
}

/// What the host will lay out: the frame after the wire's own bounds.
fn through_the_wire(mut frame: Frame) -> Frame {
    ui_lang_guest::wire::sanitize(&mut frame);
    frame
}

#[test]
fn the_more_button_opens_the_menu_and_the_heart_opens_the_grid() {
    on_a_deep_stack(|| {
        let (subscription, frame) = shown(&facts());
        let frame = tick_native(press(&frame, "More message actions"));
        let intent = one_intent(&frame);
        assert_eq!(intent.kind, "chat.message_actions");
        assert_eq!(
            serde_json::from_slice::<Selection>(&intent.payload).expect("decodes"),
            Selection {
                seq: 1,
                body: "first light".into(),
                rev: 1
            }
        );
        let menu = ChatProps {
            selected_message_seq: 1,
            selected_message_rev: 1,
            message_action: "more".into(),
            ..facts()
        };
        let frame = through_the_wire(tick_native(vec![item(subscription, &encoded(&menu))]));
        for expected in ["Add reaction", "Reply in thread", "Edit message"] {
            assert!(
                has_text(&frame, expected),
                "missing {expected:?} in {:?}",
                texts(&frame)
            );
        }
        let frame = tick_native(press(&frame, "Manage reactions"));
        assert_eq!(one_intent(&frame).kind, "chat.message_reactions");
        let grid = ChatProps {
            message_action: "reactions".into(),
            ..menu
        };
        let frame = through_the_wire(tick_native(vec![item(subscription, &encoded(&grid))]));
        assert!(has_text(&frame, "🦆"), "{:?}", texts(&frame));
        fn emoji_button(node: &Node) -> Option<u32> {
            if let Node::Button {
                description,
                on_press,
                ..
            } = node
                && description.as_deref() == Some("🦆")
            {
                return *on_press;
            }
            node.children().iter().find_map(emoji_button)
        }
        let message = emoji_button(frame.root.as_ref().unwrap()).expect("duck reaction button");
        let frame = tick_native(vec![ui_lang_guest::wire::Event::Message(message)]);
        assert_eq!(one_intent(&frame).kind, "chat.reaction_submit");
    });
}

#[test]
fn a_copy_range_stays_above_the_scroller_and_clear_routes_to_the_host() {
    on_a_deep_stack(|| {
        let (subscription, _) = shown(&facts());
        let ranged = ChatProps {
            copy_anchor_seq: 1,
            copy_head_seq: 2,
            copy_surface: "timeline".into(),
            ..facts()
        };
        let frame = tick_native(vec![item(subscription, &encoded(&ranged))]);
        let shown = texts(&frame);
        let at = |needle: &str| {
            shown
                .iter()
                .position(|text| text == needle)
                .unwrap_or_else(|| panic!("missing {needle:?} in {shown:?}"))
        };
        assert!(
            at("2 messages selected") < at("first light"),
            "the bar reads above the messages: {shown:?}"
        );
        assert!(at("Copy") < at("first light") && at("Clear") < at("first light"));
        assert!(!has_text(&frame, "⇧-click another message to extend"));
        node_ending(&frame, "/timeline-selection/root");
        fn assert_bar_outside_scrollers(node: &Node) {
            fn has_bar(node: &Node) -> bool {
                node.key()
                    .is_some_and(|key| key.contains("/timeline-selection/"))
                    || node.children().iter().any(has_bar)
            }
            if let Node::Scroll { content, .. } = node {
                assert!(
                    !has_bar(content),
                    "selection controls must stay outside the scroller"
                );
            }
            for child in node.children() {
                assert_bar_outside_scrollers(child);
            }
        }
        assert_bar_outside_scrollers(frame.root.as_ref().unwrap());
        let frame = tick_native(press(&frame, "Clear"));
        assert_eq!(one_intent(&frame).kind, "chat.clear_range");
    });
}

#[test]
fn a_thread_drag_tracks_the_pointer_until_release_and_buttons_also_resize() {
    on_a_deep_stack(|| {
        use ui_lang_guest::wire::{Event, Length, mouse};
        let props = ChatProps {
            active_thread_seq: 1,
            thread_messages: vec![message(1, "first light")],
            ..facts()
        };
        let (_, frame) = shown(&props);
        let width = |frame: &Frame| {
            let node = node_ending(frame, "/thread-pane");
            let Node::Container {
                width: Some(Length::Fixed(width)),
                ..
            } = node
            else {
                panic!("fixed thread width: {node:?}")
            };
            *width
        };
        assert_eq!(width(&frame), 330.0);
        assert!(frame.mouse_interest);
        let movement = |x| Event::Mouse {
            event: mouse::Event::CursorMoved { x, y: 30.0 },
            captured: true,
        };
        let frame = tick_native(vec![movement(700.0)]);
        let handle = node_ending(&frame, "/thread-resize");
        let Node::MouseArea {
            on_press: Some(handler),
            ..
        } = handle
        else {
            panic!("a routed handle")
        };
        let frame = tick_native(vec![Event::Message(*handler), movement(620.0)]);
        assert_eq!(width(&frame), 410.0);
        let frame = tick_native(vec![
            Event::Mouse {
                event: mouse::Event::ButtonReleased(mouse::Button::Left),
                captured: true,
            },
            movement(500.0),
        ]);
        assert_eq!(
            width(&frame),
            410.0,
            "release ends the drag outside the handle"
        );
        let frame = tick_native(press(&frame, "Narrow thread"));
        assert_eq!(width(&frame), 378.0);
        let frame = tick_native(press(&frame, "Widen thread"));
        assert_eq!(width(&frame), 410.0);
    });
}

fn node_ending<'a>(frame: &'a Frame, suffix: &str) -> &'a Node {
    fn walk<'a>(node: &'a Node, suffix: &str) -> Option<&'a Node> {
        if node.key().is_some_and(|key| key.ends_with(suffix)) {
            return Some(node);
        }
        node.children().iter().find_map(|node| walk(node, suffix))
    }
    walk(frame.root.as_ref().expect("a tree"), suffix).expect("an identified node")
}

#[test]
fn a_thread_selection_does_not_add_a_second_bar_to_the_channel() {
    on_a_deep_stack(|| {
        let props = ChatProps {
            active_thread_seq: 1,
            thread_messages: vec![message(1, "first light"), message(2, "a reply")],
            copy_anchor_seq: 1,
            copy_head_seq: 2,
            copy_surface: "thread".into(),
            ..facts()
        };
        let (_, frame) = shown(&props);
        assert_eq!(
            texts(&frame)
                .iter()
                .filter(|text| *text == "2 messages selected")
                .count(),
            1
        );
        node_ending(&frame, "/thread-selection/root");
    });
}
