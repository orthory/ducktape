//! The facts the host pushes are what the screen shows; every act leaves as
//! an intent carrying what the reader chose or typed, and the two composers
//! are slots the host paints, keyed by the room and the thread.

use chat_view::host::{
    Channel, ChatBlock, ChatChannel, ChatMessage, ChatProps, ChatSidebarRow, LiveActivity,
    LiveAgentRow, Query, RunId, Selection, Text,
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

fn live_run(anchor_seq: i64) -> LiveAgentRow {
    LiveAgentRow {
        anchor_seq,
        run_id: "chat\u{1f}channel-a\u{1f}2\u{1f}agent-1".into(),
        agent: "ferris".into(),
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
        ..LiveAgentRow::default()
    }
}

/// THE MEMO MUST SEE THE RUN MOVE. The stream's rows are drawn inside the
/// timeline's `lazy`, and a `by` key list takes the lazied value OUT of the
/// memo's hash — what keys it is the REVISION of the state the value reads. So
/// the runs have to ride a state field the memo reads (`host::Timeline`): fold
/// them into `live_agents` alone and the second reading below is a cache hit
/// with the card still on its first status, for the whole run.
///
/// Only `live_agents` differs between the two readings here. That is the point.
#[test]
fn a_run_in_flight_repaints_as_it_works() {
    on_a_deep_stack(|| {
        let mut starting = live_run(2);
        starting.status = "Starting".into();
        starting.activity.clear();
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
        for expected in ["Reading the repo", "Command: cargo test", "Reasoning"] {
            assert!(
                has_text(&frame, expected),
                "the run's progress never reached the frame: missing {expected:?} in {:?}",
                texts(&frame)
            );
        }
        assert!(
            !has_text(&frame, "Starting"),
            "the memo served a stale card: {:?}",
            texts(&frame)
        );
    });
}

#[test]
fn a_live_agent_row_shows_under_its_anchor_and_stop_cancels_the_run() {
    on_a_deep_stack(|| {
        let props = ChatProps {
            live_agents: vec![live_run(2)],
            ..facts()
        };
        let (_, frame) = shown(&props);
        for expected in ["ferris", "AGENT", "Reading the repo", "Command: cargo test"] {
            assert!(
                has_text(&frame, expected),
                "missing {expected:?} in {:?}",
                texts(&frame)
            );
        }
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
    });
}

#[test]
fn a_failed_run_shows_its_terminal_state() {
    on_a_deep_stack(|| {
        let mut failed = live_run(2);
        failed.status = "the node event stream closed".into();
        failed.activity.clear();
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
