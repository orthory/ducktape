//! The facts the host pushes are what the screen shows; every act leaves as
//! an intent carrying what the reader chose or typed, and the two composers
//! are slots the host paints, keyed by the room and the thread.

use chat_view::host::{
    Channel, ChatBlock, ChatChannel, ChatMessage, ChatProps, ChatSidebarRow, DispatchId,
    LiveRunHint, Query, RunId, Selection, Text, run_of_message,
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
    ui_lang_guest::wire::sanitize(&mut frame).expect("the chat frame satisfies wire bounds");
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
fn a_thread_drag_tracks_the_pointer_until_release_without_step_buttons() {
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
        fn has_step_button(node: &Node) -> bool {
            matches!(node, Node::Button { label: Some(label), .. } if label == "Narrow thread" || label == "Widen thread")
                || node.children().iter().any(has_step_button)
        }
        assert!(
            !has_step_button(frame.root.as_ref().unwrap()),
            "resize uses the divider, not step buttons"
        );
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

const DISPATCH: &str = "5b0f6b7b0c3e8a4d9f1e2c3b4a5968778695a4b3c2d1e0f9a8b7c6d5e4f30211";

fn live_run(anchor_seq: i64) -> LiveRunHint {
    LiveRunHint {
        anchor_seq,
        run_id: "chat\u{1f}channel-a\u{1f}2\u{1f}agent-1".into(),
        dispatch_id: DISPATCH.into(),
        agent: "ferris".into(),
        status: "Reading the repo".into(),
        ..LiveRunHint::default()
    }
}

/// THE MEMO MUST SEE THE RUN MOVE. The stream's rows are drawn inside the
/// timeline's `lazy`, and a `by` key list takes the lazied value OUT of the
/// memo's hash — what keys it is the REVISION of the state the value reads. So
/// the runs have to ride a state field the memo reads (`host::Timeline`): fold
/// them into `live_agents` alone and the second reading below is a cache hit
/// with the hint still on its first status, for the whole run.
///
/// Only `live_agents` differs between the two readings here. That is the point.
#[test]
fn a_run_in_flight_repaints_as_it_works() {
    on_a_deep_stack(|| {
        let mut starting = live_run(2);
        starting.status = "Starting".into();
        let props = ChatProps {
            live_agents: vec![starting],
            ..facts()
        };
        let (subscription, frame) = shown(&props);
        assert!(has_text(&frame, "Starting"), "{:?}", texts(&frame));

        let moved_on = ChatProps {
            live_agents: vec![live_run(2)],
            ..facts()
        };
        let frame = tick_native(vec![item(subscription, &encoded(&moved_on))]);
        assert!(
            has_text(&frame, "Reading the repo"),
            "the run's status never reached the frame: {:?}",
            texts(&frame)
        );
        assert!(
            !has_text(&frame, "Starting"),
            "the memo served a stale hint: {:?}",
            texts(&frame)
        );
    });
}

/// THE STREAM SAYS A RUN IS WORKING HERE, AND NO MORE. Its status and a way
/// to the run panel; the progress itself is the panel's, so the hint carries
/// none and "View run" hands the app the run's address.
#[test]
fn a_live_run_hint_shows_under_its_anchor_and_view_run_opens_the_run() {
    on_a_deep_stack(|| {
        let props = ChatProps {
            live_agents: vec![live_run(2)],
            ..facts()
        };
        let (_, frame) = shown(&props);
        for expected in ["ferris", "AGENT", "Reading the repo", "View run", "Stop"] {
            assert!(
                has_text(&frame, expected),
                "missing {expected:?} in {:?}",
                texts(&frame)
            );
        }
        let frame = tick_native(press(&frame, "View run"));
        let intent = one_intent(&frame);
        assert_eq!(intent.kind, "chat.open_run");
        assert_eq!(
            serde_json::from_slice::<DispatchId>(&intent.payload).expect("decodes"),
            DispatchId {
                dispatch_id: DISPATCH.into()
            }
        );
    });
}

#[test]
fn a_live_run_hint_stop_cancels_the_run() {
    on_a_deep_stack(|| {
        let props = ChatProps {
            live_agents: vec![live_run(2)],
            ..facts()
        };
        let (_, frame) = shown(&props);
        let frame = tick_native(press(&frame, "Stop"));
        let [intent] = frame.requests.as_slice() else {
            panic!("one intent, got {:?}", frame.requests);
        };
        assert_eq!(intent.kind, "chat.cancel_run");
        assert_eq!(
            serde_json::from_slice::<RunId>(&intent.payload).expect("decodes"),
            RunId {
                run_id: "chat\u{1f}channel-a\u{1f}2\u{1f}agent-1".into()
            }
        );
    });
}

/// How many hints for this run are on the frame. The status is per-hint, so
/// counting it counts hints.
fn cards(frame: &Frame) -> usize {
    texts(frame)
        .iter()
        .filter(|text| *text == "Reading the repo")
        .count()
}

/// ONE RUN, ONE CARD, whichever surface is in front of the reader.
///
/// The stream draws a card under the anchor and the rail draws one at its foot.
/// With the run's OWN thread open both were true at once — two cards and two
/// Stops for one run. The rail owns it while the rail is on screen; the stream
/// takes it back when it is not.
///
/// The settings case is the one a seq-only suppression gets wrong: the drawer
/// replaces the rail while `active_thread_seq` still stands, so "a thread is
/// open" would have hidden BOTH cards and left the run with no Stop at all.
#[test]
fn one_run_draws_exactly_one_card_whichever_surface_owns_it() {
    on_a_deep_stack(|| {
        let run = live_run(2);
        let closed = ChatProps {
            live_agents: vec![run.clone()],
            ..facts()
        };
        let (subscription, frame) = shown(&closed);
        assert_eq!(cards(&frame), 1, "the stream draws it: {:?}", texts(&frame));
        assert!(has_text(&frame, "ferris"));

        // THE RAIL OPENS ON THE RUN'S OWN THREAD. Its card moves; it does not
        // multiply.
        let mut root = message(2, "second wind");
        root.reply_count = 1;
        let railed = ChatProps {
            active_thread_seq: 2,
            thread_messages: vec![root.clone()],
            live_agents: vec![run.clone()],
            ..facts()
        };
        let frame = tick_native(vec![item(subscription, &encoded(&railed))]);
        assert_eq!(
            cards(&frame),
            1,
            "the rail has it AND the stream still drew one: {:?}",
            texts(&frame)
        );
        // the Stop still routes, and there is exactly one of it
        let frame = tick_native(press(&frame, "Stop"));
        let intent = one_intent(&frame);
        assert_eq!(intent.kind, "chat.cancel_run");
        assert_eq!(
            serde_json::from_slice::<RunId>(&intent.payload).expect("decodes"),
            RunId {
                run_id: run.run_id.clone()
            },
            "the same run the stream's card would have stopped"
        );

        // THE SETTINGS DRAWER REPLACES THE RAIL while the thread stays open.
        // The card belongs to the stream again — suppressing on the seq alone
        // would leave the reader no card and no Stop.
        let drawered = ChatProps {
            active_thread_seq: 2,
            channel_settings_open: true,
            thread_messages: vec![root],
            live_agents: vec![run],
            ..facts()
        };
        let frame = tick_native(vec![item(subscription, &encoded(&drawered))]);
        assert_eq!(
            cards(&frame),
            1,
            "the drawer hid the rail and took the card with it: {:?}",
            texts(&frame)
        );

        // AND BACK TO THE STREAM when the rail closes.
        let frame = tick_native(vec![item(subscription, &encoded(&closed))]);
        assert_eq!(cards(&frame), 1, "{:?}", texts(&frame));
    });
}

/// The anchor decides WHERE, and a run summoned inside a thread belongs to the
/// rail: its anchor is a reply, which the stream never draws, so the stream
/// must stay clean and the rail must claim it through `thread_root`.
#[test]
fn a_run_anchored_in_a_thread_draws_in_the_rail_and_not_in_the_stream() {
    on_a_deep_stack(|| {
        let mut in_thread = live_run(7);
        in_thread.thread_root = 2;
        in_thread.status = "Answering in the thread".into();
        let mut root = message(2, "second wind");
        root.reply_count = 1;
        let mut reply = message(7, "and what about the rail");
        reply.thread_seq = 2;
        let props = ChatProps {
            active_thread_seq: 2,
            thread_messages: vec![root, reply],
            live_agents: vec![in_thread.clone()],
            ..facts()
        };
        let (subscription, frame) = shown(&props);
        assert!(
            has_text(&frame, "Answering in the thread"),
            "the rail did not claim the run: {:?}",
            texts(&frame)
        );

        // THE SAME RUN, NO RAIL OPEN: its anchor is a reply, so no message in
        // the stream carries its seq and nothing of it is drawn.
        let closed = ChatProps {
            live_agents: vec![in_thread],
            ..facts()
        };
        let frame = tick_native(vec![item(subscription, &encoded(&closed))]);
        assert!(
            !has_text(&frame, "Answering in the thread"),
            "a reply's run leaked into the stream: {:?}",
            texts(&frame)
        );
    });
}

#[test]
fn the_committed_reply_replaces_the_live_row() {
    on_a_deep_stack(|| {
        let props = ChatProps {
            live_agents: vec![live_run(2)],
            ..facts()
        };
        let (subscription, frame) = shown(&props);
        assert!(has_text(&frame, "Reading the repo"));
        // the run left the pending set as its reply landed: the row goes, the
        // reply stays
        let mut reply = message(3, "here is the answer");
        reply.id = format!("agent/{DISPATCH}");
        reply.author = "ferris".into();
        reply.avatar_kind = "agent".into();
        let landed = ChatProps {
            messages: vec![message(1, "first"), message(2, "second"), reply],
            live_agents: Vec::new(),
            ..facts()
        };
        let frame = tick_native(vec![item(subscription, &encoded(&landed))]);
        assert!(!has_text(&frame, "Reading the repo"), "{:?}", texts(&frame));
        assert!(!has_text(&frame, "Stop"), "{:?}", texts(&frame));
        assert!(has_text(&frame, "here is the answer"));
        assert!(has_text(&frame, "AGENT"), "the reply wears the agent plate");
        // THE REPLY KEEPS THE WAY BACK TO ITS RUN: the chip is the one the
        // hint offered, and it opens the same run.
        let frame = tick_native(press(&frame, "View run"));
        let intent = one_intent(&frame);
        assert_eq!(intent.kind, "chat.open_run");
        assert_eq!(
            serde_json::from_slice::<DispatchId>(&intent.payload).expect("decodes"),
            DispatchId {
                dispatch_id: DISPATCH.into()
            }
        );
    });
}

/// A message posted by no run offers no run to open: a person's message, a
/// message whose id merely starts like a run's, and a run-shaped id whose
/// dispatch is not a dispatch id all read as "".
#[test]
fn only_a_run_posted_message_names_its_run() {
    assert_eq!(run_of_message(&format!("agent/{DISPATCH}")), DISPATCH);
    assert_eq!(
        run_of_message(&format!("agent/{DISPATCH}/post/3")),
        DISPATCH
    );
    assert_eq!(run_of_message("chat\u{1f}channel-a\u{1f}2"), "");
    assert_eq!(run_of_message("agent/ferris"), "");
    assert_eq!(run_of_message("agent/"), "");
    assert_eq!(run_of_message(""), "");
}

#[test]
fn a_failed_run_shows_its_terminal_state() {
    on_a_deep_stack(|| {
        let mut failed = live_run(2);
        failed.status = "the node event stream closed".into();
        let props = ChatProps {
            live_agents: vec![failed],
            ..facts()
        };
        let (_, frame) = shown(&props);
        assert!(
            has_text(&frame, "the node event stream closed"),
            "{:?}",
            texts(&frame)
        );
    });
}
