//! The facts the host pushes are what the screen shows; every act leaves as
//! an intent carrying what the reader typed, and a draft the app hands back
//! lands in the field only when the seed moved.

use pages_view::host::{
    Choose, Create, Narrow, PageComment, PageCommentThread, PageCommentThreadRow, PageItem,
    PagesProps, Post, Resolve, Search, sidebar_width_after_delta,
};
use pages_view::{boot_native, tick_native};
use ui_lang_guest::testing::{find, has_text, item, measure, press, submit, texts, type_into};
use ui_lang_guest::wire::{Event, Frame, Length, Node};

fn facts() -> PagesProps {
    PagesProps {
        connected: true,
        page_link: "duck://pages/alpha".into(),
        pages: vec![
            PageItem {
                id: "alpha".into(),
                title: "Alpha".into(),
                ..PageItem::default()
            },
            PageItem {
                id: "beta".into(),
                title: "Beta".into(),
                ..PageItem::default()
            },
        ],
        page_create_open: true,
        active_page: "alpha".into(),
        active_page_title: "Alpha".into(),
        autosave: "saved".into(),
        block_comments_open: true,
        scope_label: "This page · 0 threads".into(),
        compose_hint: "Comment on this page".into(),
        ..PagesProps::default()
    }
}

fn comment(ordinal: i64, author: &str, text: &str) -> PageComment {
    PageComment {
        id: format!("comment-{ordinal}-{author}"),
        ordinal,
        author: author.into(),
        meta: format!("#{ordinal}"),
        text: text.into(),
    }
}

fn thread(id: &str, target: &str, resolved: bool, comments: Vec<PageComment>) -> PageCommentThread {
    PageCommentThread {
        id: id.into(),
        target: target.into(),
        author: comments
            .first()
            .map(|first| first.author.clone())
            .unwrap_or_default(),
        meta: format!("{} comments", comments.len()),
        resolved,
        comment_count: comments.len() as i64,
        comments,
    }
}

fn row(thread: PageCommentThread, anchor: &str) -> PageCommentThreadRow {
    PageCommentThreadRow {
        thread,
        anchor: anchor.into(),
    }
}

/// Two open threads on one block — one of them a conversation with five
/// replies — a thread on another block, one on the page, and a settled one.
fn conversation() -> Vec<PageCommentThreadRow> {
    vec![
        row(
            thread(
                "page-thread",
                "alpha",
                false,
                vec![comment(1, "Ines", "Is the whole page ready?")],
            ),
            "this page",
        ),
        row(
            thread(
                "seven-a",
                "block-7",
                false,
                vec![
                    comment(1, "Ada", "This paragraph reads backwards."),
                    comment(2, "Bo", "reply one"),
                    comment(3, "Ada", "reply two"),
                    comment(4, "Bo", "reply three"),
                    comment(5, "Ada", "reply four"),
                    comment(6, "Bo", "reply five"),
                ],
            ),
            "“Paragraph 7”",
        ),
        row(
            thread(
                "seven-b",
                "block-7",
                false,
                vec![comment(1, "Cy", "And the number is wrong.")],
            ),
            "“Paragraph 7”",
        ),
        row(
            thread(
                "seven-done",
                "block-7",
                true,
                vec![comment(1, "Dee", "Settled long ago.")],
            ),
            "“Paragraph 7”",
        ),
        row(
            thread(
                "nine",
                "block-9",
                false,
                vec![comment(1, "Eve", "Elsewhere entirely.")],
            ),
            "“Paragraph 9”",
        ),
    ]
}

fn encoded(props: &PagesProps) -> Vec<u8> {
    serde_json::to_vec(props).expect("props encode")
}

/// Boot and push the facts; returns the subscription id and the frame.
fn shown(props: &PagesProps) -> (u64, Frame) {
    boot_native();
    let frame = tick_native(Vec::new());
    assert_eq!(frame.requests[0].kind, "pages.props");
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

#[test]
fn the_facts_the_host_pushes_are_what_the_screen_shows_and_a_pick_carries_the_card_draft() {
    let (_, frame) = shown(&facts());
    for expected in [
        "Pages",
        "Alpha",
        "Beta",
        "✓ synced",
        "Comments",
        "No comments on this page yet",
    ] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
    let frame = tick_native(type_into(&frame, "Start a thread…", "half a thought"));
    assert!(frame.requests.is_empty(), "typing runs no handler");
    let frame = tick_native(press(&frame, "Beta"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "pages.choose");
    assert_eq!(
        serde_json::from_slice::<Choose>(&intent.payload).expect("decodes"),
        Choose {
            id: "beta".into(),
            comment_draft: "half a thought".into()
        }
    );
}

#[test]
fn a_create_a_search_and_a_post_leave_with_what_was_typed() {
    let (_, frame) = shown(&facts());
    let frame = tick_native(type_into(&frame, "New page", "  Gamma  "));
    let frame = tick_native(type_into(&frame, "Search pages…", "quorum"));
    let frame = tick_native(type_into(&frame, "Start a thread…", "looks right"));
    let frame = tick_native(submit(&frame, "New page"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "pages.create");
    assert_eq!(
        serde_json::from_slice::<Create>(&intent.payload).expect("decodes"),
        Create {
            title: "Gamma".into(),
            comment_draft: "looks right".into()
        }
    );
    let frame = tick_native(submit(&frame, "Search pages…"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "pages.search");
    assert_eq!(
        serde_json::from_slice::<Search>(&intent.payload).expect("decodes"),
        Search {
            query: "quorum".into()
        }
    );
    let frame = tick_native(press(&frame, "Post"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "pages.post");
    assert_eq!(
        serde_json::from_slice::<Post>(&intent.payload).expect("decodes"),
        Post {
            text: "looks right".into(),
            thread_id: String::new()
        }
    );
    // the field cleared with the act: the button is dark now
    assert!(
        matches!(
            ui_lang_guest::testing::find(&frame, "PagesView/root/pages/comments-card/post"),
            Some(ui_lang_guest::wire::Node::Button { on_press: None, .. })
        ),
        "{:?}",
        texts(&frame)
    );
}

#[test]
fn a_draft_the_app_hands_back_lands_only_when_the_seed_moved() {
    let (subscription, frame) = shown(&facts());
    let _ = tick_native(type_into(&frame, "Start a thread…", "mine"));
    // the same seed pushed again changes nothing …
    let frame = tick_native(vec![item(subscription, &encoded(&facts()))]);
    let frame = tick_native(press(&frame, "Post"));
    assert_eq!(
        serde_json::from_slice::<Post>(&one_intent(&frame).payload).expect("decodes"),
        Post {
            text: "mine".into(),
            thread_id: String::new()
        }
    );
    // … a moved seed replaces the field
    let returned = PagesProps {
        seed_rev: 1,
        comment_seed: "the refused one".into(),
        ..facts()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&returned))]);
    let frame = tick_native(press(&frame, "Post"));
    assert_eq!(
        serde_json::from_slice::<Post>(&one_intent(&frame).payload).expect("decodes"),
        Post {
            text: "the refused one".into(),
            thread_id: String::new()
        }
    );
}

/// The events the host sends when the reader drags a resize handle sideways.
fn drag(frame: &Frame, key: &str, dx: f64) -> Vec<Event> {
    let Some(Node::ResizeHandle {
        on_drag: Some(handler),
        ..
    }) = find(frame, key)
    else {
        panic!("no resize handle {key:?}");
    };
    vec![Event::Drag {
        handler: *handler,
        dx,
        dy: 0.0,
    }]
}

fn list_width(frame: &Frame) -> f32 {
    let Some(Node::Container {
        width: Some(Length::Fixed(width)),
        ..
    }) = find(frame, "PagesView/root/pages/page-list")
    else {
        panic!("no page list in {:?}", texts(frame));
    };
    *width
}

#[test]
fn the_page_list_is_the_readers_to_size_and_never_crowds_the_document() {
    assert_eq!(sidebar_width_after_delta(230.0, 60.0, 1280.0), 290.0);
    assert_eq!(sidebar_width_after_delta(230.0, -400.0, 1280.0), 180.0);
    assert_eq!(sidebar_width_after_delta(230.0, 400.0, 1280.0), 420.0);
    // A narrow console keeps half its width for the document …
    assert_eq!(sidebar_width_after_delta(230.0, 400.0, 600.0), 300.0);
    // … and a window narrower than two list minimums still gets a list.
    assert_eq!(sidebar_width_after_delta(230.0, 0.0, 200.0), 180.0);

    let (_, frame) = shown(&facts());
    assert_eq!(list_width(&frame), 230.0);
    let handle = "PagesView/root/pages/sidebar-resize";
    let frame = tick_native(drag(&frame, handle, 60.0));
    assert!(frame.requests.is_empty(), "sizing the list is view-local");
    assert_eq!(list_width(&frame), 290.0);
    let frame = tick_native(drag(&frame, handle, 500.0));
    assert_eq!(list_width(&frame), 420.0);
    let frame = tick_native(drag(&frame, handle, -500.0));
    assert_eq!(list_width(&frame), 180.0);
}

#[test]
fn the_header_menu_names_the_delete_before_it_arms_it() {
    let menu = "PagesView/root/pages/page-menu";
    let (_, frame) = shown(&facts());
    // The `⋯` arms nothing on its own: it opens a menu, view-locally …
    assert!(find(&frame, menu).is_none());
    let frame = tick_native(press(&frame, "Page actions"));
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
    assert!(find(&frame, menu).is_some(), "{:?}", texts(&frame));
    assert!(has_text(&frame, "Delete page…"), "{:?}", texts(&frame));
    // … and the named item is what arms the confirm dialog, closing behind it.
    let frame = tick_native(press(&frame, "Delete page…"));
    assert_eq!(one_intent(&frame).kind, "pages.arm_delete");
    assert!(find(&frame, menu).is_none(), "the menu left with the act");
}

#[test]
fn focus_observations_hide_named_link_syntax_and_ignore_a_late_focus_reply() {
    use ui_lang_guest::{testing, wire};
    use wire::editor_document::{EditorDocumentRef, EditorTransferId, EditorTransferSender};
    let text = "Title\n[문서](https://example.com)";
    let reference = EditorDocumentRef {
        document: "page-alpha".into(),
        reset: 1,
        revision: 0,
        text_revision: 0,
        byte_len: text.len() as u32,
        cursor: wire::EditorCursor {
            position: wire::EditorPosition { line: 1, column: 1 },
            selection: None,
        },
    };
    let mut props = facts();
    props.document_source = wire::encode(&pages_view::document_source::DocumentIdentity {
        document: reference.document.clone(),
        reset: reference.reset,
    });
    let (_, mut frame) = shown(&props);
    let mut root = frame.root.clone().unwrap();
    let source = frame
        .requests
        .iter()
        .find(|request| request.kind == "pages.document")
        .unwrap()
        .id;
    let mut sender = EditorTransferSender::new(
        EditorTransferId {
            instance: 1,
            document: reference.document.clone(),
            reset: 1,
            serial: 1,
            attempt: 0,
        },
        reference.clone(),
    )
    .unwrap();
    while let Some(transfer) = sender.next_frame(&reference, text).unwrap() {
        frame = tick_native(vec![item(source, &wire::encode(&transfer))]);
        if let Some(next) = &frame.root {
            root = next.clone();
        } else {
            wire::apply(&mut root, frame.patches.clone()).unwrap();
        }
    }
    let focus_request = |frame: &Frame| {
        let request = frame
            .requests
            .iter()
            .find(|request| request.kind == "host.widget")
            .unwrap();
        assert_eq!(
            wire::decode::<wire::WidgetCommand>(&request.payload).unwrap(),
            wire::WidgetCommand::Focused {
                target: "PagesView/root/pages/document".into()
            }
        );
        request.id
    };
    let first = focus_request(&frame);
    let mut reply = |id, focused| {
        let frame = tick_native(vec![wire::Event::Response {
            id,
            result: Ok(wire::encode(&focused)),
            done: true,
        }]);
        if let Some(next) = &frame.root {
            root = next.clone();
        } else {
            wire::apply(&mut root, frame.patches.clone()).unwrap();
        }
        let shown = Frame {
            root: Some(root.clone()),
            ..Default::default()
        };
        let Some(wire::Node::Editor {
            options, document, ..
        }) = testing::find(&shown, "PagesView/root/pages/document")
        else {
            panic!("editor");
        };
        assert_eq!(document.byte_len, text.len() as u32);
        assert_eq!(document.cursor, reference.cursor);
        let paint = options.presentation.as_ref().unwrap();
        paint
            .spans
            .iter()
            .filter(|span| span.line == 1)
            .filter(|span| paint.formats[span.format as usize].size.unwrap_or(14.0) > 1.0)
            .map(|span| &text[6 + span.start as usize..6 + span.end as usize])
            .collect::<String>()
    };
    assert_eq!(reply(first, true), "[문서](https://example.com)");
    let release = || wire::Event::Mouse {
        event: wire::mouse::Event::ButtonReleased(wire::mouse::Button::Left),
        captured: true,
    };
    let old = focus_request(&tick_native(vec![release()]));
    let latest = focus_request(&tick_native(vec![release()]));
    assert_eq!(reply(latest, false), "문서");
    assert_eq!(
        reply(old, true),
        "문서",
        "late responses cannot reopen source syntax"
    );
    let next = focus_request(&tick_native(vec![release()]));
    assert_eq!(reply(next, true), "[문서](https://example.com)");
    // Tab is usually captured by the mounted widgets before this observation.
    use wire::keyboard::{Key, KeyState, Location, Modifiers, Named, NativeCode, Physical};
    let tab = wire::Event::Keyboard {
        event: wire::keyboard::Event::Release(KeyState {
            key: Key::Named(Named::Tab),
            modified_key: Key::Named(Named::Tab),
            physical_key: Physical::Unidentified(NativeCode::Unidentified),
            location: Location::Standard,
            modifiers: Modifiers::default(),
        }),
        captured: true,
    };
    let tab_query = focus_request(&tick_native(vec![tab]));
    assert_eq!(reply(tab_query, false), "문서");
}

#[test]
fn initial_loading_empty_and_recovered_pages_have_visible_states() {
    let loading = PagesProps {
        connected: true,
        loading: true,
        ..PagesProps::default()
    };
    let (subscription, frame) = shown(&loading);
    assert!(has_text(&frame, "Loading pages…"));
    assert!(!has_text(&frame, "No page selected"));
    let empty = PagesProps {
        loading: false,
        ..loading
    };
    let frame = tick_native(vec![item(subscription, &encoded(&empty))]);
    assert!(has_text(&frame, "No page selected"));
    assert!(!has_text(&frame, "Loading pages…"));
    let frame = tick_native(vec![item(subscription, &encoded(&facts()))]);
    assert!(has_text(&frame, "Alpha"));
    assert!(!has_text(&frame, "Loading pages…"));
    assert!(!has_text(&frame, "No page selected"));
    let disconnected = PagesProps::default();
    let frame = tick_native(vec![item(subscription, &encoded(&disconnected))]);
    assert!(has_text(&frame, "Not connected"));
    assert!(!has_text(&frame, "Loading pages…"));
}

#[test]
fn invalid_props_are_visible_and_a_valid_update_recovers_the_same_draft() {
    boot_native();
    let boot = tick_native(Vec::new());
    let subscription = boot
        .requests
        .iter()
        .find(|request| request.kind == "pages.props")
        .unwrap()
        .id;
    let frame = tick_native(vec![item(subscription, br#"{"connected":true}"#)]);
    assert!(has_text(&frame, "Pages could not load"));
    assert!(
        !has_text(&frame, "Not connected"),
        "a props error is not a network status"
    );
    let frame = tick_native(vec![item(subscription, &encoded(&facts()))]);
    assert!(!has_text(&frame, "Pages could not load"));
    let _ = tick_native(type_into(&frame, "Start a thread…", "keep this draft"));
    let frame = tick_native(vec![item(subscription, br#"{"connected":true}"#)]);
    assert!(has_text(&frame, "Pages could not load"));
    assert!(
        texts(&frame)
            .iter()
            .any(|text| text.contains("missing field"))
    );
    assert!(
        has_text(&frame, "Alpha"),
        "keep the last readable page visible"
    );
    let frame = tick_native(vec![item(subscription, &encoded(&facts()))]);
    assert!(!has_text(&frame, "Pages could not load"));
    let frame = tick_native(press(&frame, "Post"));
    assert_eq!(
        serde_json::from_slice::<Post>(&one_intent(&frame).payload)
            .unwrap()
            .text,
        "keep this draft"
    );
}

#[test]
fn malformed_target_update_freezes_queued_actions_until_valid_facts_arrive() {
    let (subscription, frame) = shown(&facts());
    let frame = tick_native(type_into(&frame, "Start a thread…", "draft from Alpha"));
    let post = press(&frame, "Post");
    // The delete is a named item in the header menu now, so open the menu
    // while the facts still stand and queue the press from inside it.
    let opened = tick_native(press(&frame, "Page actions"));
    let delete = press(&opened, "Delete page");
    let choose = press(&frame, "Beta");
    // The host has moved to Beta, but an incomplete update cannot replace
    // the Alpha facts that the reader still sees.
    let malformed = br#"{"connected":true,"active_page":"beta"}"#;
    let frame = tick_native(vec![item(subscription, malformed)]);
    assert!(has_text(&frame, "Pages could not load"));
    assert!(has_text(&frame, "draft from Alpha"));
    assert!(matches!(
        ui_lang_guest::testing::find(&frame, "PagesView/root/pages/comments-card/post"),
        Some(ui_lang_guest::wire::Node::Button { on_press: None, .. })
    ));
    for queued in [post, delete, choose] {
        let frame = tick_native(queued);
        assert!(
            frame.requests.is_empty(),
            "stale actions must not reach the host"
        );
    }
    let beta = PagesProps {
        active_page: "beta".into(),
        active_page_title: "Beta".into(),
        ..facts()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&beta))]);
    assert!(!has_text(&frame, "Pages could not load"));
    let frame = tick_native(press(&frame, "Post"));
    let request = one_intent(&frame);
    assert_eq!(request.kind, "pages.post");
    assert_eq!(
        serde_json::from_slice::<Post>(&request.payload)
            .unwrap()
            .text,
        "draft from Alpha"
    );
}

/// Keys of every button on the frame — how a test names the one it wants when
/// several carry the same words.
fn button_keys(frame: &Frame) -> Vec<String> {
    let mut keys = Vec::new();
    let mut walk = |node: &mut Node| {
        if let Node::Button { key, .. } = node {
            keys.push(key.clone());
        }
    };
    frame.root.clone().unwrap().for_each_mut(&mut walk);
    keys
}

#[test]
fn page_scope_groups_the_threads_under_the_block_each_one_marks() {
    let props = PagesProps {
        comment_rows: conversation(),
        thread_total: 4,
        scope_label: "This page · 4 threads".into(),
        ..facts()
    };
    let (_, frame) = shown(&props);
    let shown_texts = texts(&frame);
    // One group header per commented block, plus the page's own threads under
    // a plain "This page" — and every open thread is already expanded.
    for expected in [
        "This page",
        "“Paragraph 7”",
        "“Paragraph 9”",
        "Is the whole page ready?",
        "This paragraph reads backwards.",
        "And the number is wrong.",
        "Elsewhere entirely.",
    ] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {shown_texts:?}"
        );
    }
    // The two threads on Paragraph 7 are independent cards, not one merged
    // conversation, and the group is drawn once for both.
    assert_eq!(
        shown_texts
            .iter()
            .filter(|text| *text == "“Paragraph 7”")
            .count(),
        1,
        "{shown_texts:?}"
    );
    // A thread with five replies shows three and folds the rest.
    assert!(has_text(&frame, "reply three"), "{shown_texts:?}");
    assert!(!has_text(&frame, "reply four"), "{shown_texts:?}");
    assert!(has_text(&frame, "2 more replies"), "{shown_texts:?}");
    let frame = tick_native(press(&frame, "Show every reply"));
    assert!(frame.requests.is_empty(), "folding a thread is view-local");
    assert!(has_text(&frame, "reply five"), "{:?}", texts(&frame));
    assert!(has_text(&frame, "Fewer replies"), "{:?}", texts(&frame));

    // A group header is the way IN to that block's own scope. The page's own
    // group is not one: it is already the scope the card is showing.
    let frame = tick_native(press(&frame, "Comments on this block"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "pages.narrow");
    assert_eq!(
        serde_json::from_slice::<Narrow>(&intent.payload).expect("decodes"),
        Narrow {
            target: "block-7".into()
        }
    );
}

#[test]
fn a_settled_thread_waits_behind_its_own_toggle() {
    let props = PagesProps {
        comment_rows: conversation(),
        scope_label: "This page · 4 threads".into(),
        ..facts()
    };
    let (_, frame) = shown(&props);
    assert!(has_text(&frame, "Resolved · 1"), "{:?}", texts(&frame));
    assert!(
        !has_text(&frame, "Settled long ago."),
        "a resolved thread is not in the list: {:?}",
        texts(&frame)
    );
    let frame = tick_native(press(&frame, "Resolved threads"));
    assert!(frame.requests.is_empty(), "the toggle is view-local");
    assert!(has_text(&frame, "Settled long ago."), "{:?}", texts(&frame));
    // …and it offers Reopen rather than Resolve, with no reply box.
    assert!(has_text(&frame, "Reopen"), "{:?}", texts(&frame));
    let frame = tick_native(press(&frame, "Reopen thread"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "pages.resolve");
    assert_eq!(
        serde_json::from_slice::<Resolve>(&intent.payload).expect("decodes"),
        Resolve {
            id: "seven-done".into(),
            resolved: false
        }
    );
}

#[test]
fn a_badge_opened_block_scope_is_the_whole_card_and_its_replies_name_their_thread() {
    // What the host pushes when a margin badge on Paragraph 7 is pressed: the
    // scope is that block, pinned, and the rows are already narrowed to it.
    let rows: Vec<PageCommentThreadRow> = conversation()
        .into_iter()
        .filter(|row| row.thread.target == "block-7")
        .collect();
    let props = PagesProps {
        comment_rows: rows,
        scope_target: "block-7".into(),
        scope_pinned: true,
        scope_label: "“Paragraph 7”".into(),
        compose_hint: "New thread on “Paragraph 7”".into(),
        thread_total: 4,
        ..facts()
    };
    let (subscription, frame) = shown(&props);
    let shown_texts = texts(&frame);
    // BOTH open threads on the block, each expanded, and nothing from any
    // other block.
    assert!(
        has_text(&frame, "This paragraph reads backwards."),
        "{shown_texts:?}"
    );
    assert!(
        has_text(&frame, "And the number is wrong."),
        "{shown_texts:?}"
    );
    assert!(!has_text(&frame, "Elsewhere entirely."), "{shown_texts:?}");
    assert!(
        !has_text(&frame, "Is the whole page ready?"),
        "{shown_texts:?}"
    );
    // A badge-opened card offers no way out to the page, and needs no group
    // header: the card's own title already names the block.
    assert!(!has_text(&frame, "← This page"), "{shown_texts:?}");
    assert!(
        shown_texts
            .iter()
            .filter(|text| *text == "“Paragraph 7”")
            .count()
            == 1,
        "the title names the scope once: {shown_texts:?}"
    );

    // A REPLY NAMES ITS THREAD. The resting line becomes the box when pressed.
    let reply_keys: Vec<String> = button_keys(&frame)
        .into_iter()
        .filter(|key| key.contains("reply-on"))
        .collect();
    assert_eq!(reply_keys.len(), 2, "one reply affordance per open thread");
    let frame = tick_native(press(&frame, &reply_keys[1]));
    assert!(frame.requests.is_empty(), "picking a thread is view-local");
    let frame = tick_native(type_into(&frame, "Reply…", "  seconded  "));
    let frame = tick_native(press(&frame, "Post reply"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "pages.post");
    assert_eq!(
        serde_json::from_slice::<Post>(&intent.payload).expect("decodes"),
        Post {
            text: "seconded".into(),
            thread_id: "seven-b".into()
        }
    );

    // THE COMPOSER AT THE FOOT OPENS A NEW THREAD on the same scope — an
    // empty thread id — which is how a third thread on this block is started.
    let frame = tick_native(vec![item(subscription, &encoded(&props))]);
    let frame = tick_native(type_into(&frame, "Start a thread…", "a third point"));
    let frame = tick_native(press(&frame, "PagesView/root/pages/comments-card/post"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "pages.post");
    assert_eq!(
        serde_json::from_slice::<Post>(&intent.payload).expect("decodes"),
        Post {
            text: "a third point".into(),
            thread_id: String::new()
        }
    );
}

#[test]
fn a_chip_opened_card_narrowed_to_a_block_can_widen_back() {
    let props = PagesProps {
        comment_rows: conversation()
            .into_iter()
            .filter(|row| row.thread.target == "block-7")
            .collect(),
        scope_target: "block-7".into(),
        scope_pinned: false,
        scope_label: "“Paragraph 7”".into(),
        ..facts()
    };
    let (_, frame) = shown(&props);
    assert!(has_text(&frame, "← This page"), "{:?}", texts(&frame));
    let frame = tick_native(press(&frame, "All comments on this page"));
    assert_eq!(one_intent(&frame).kind, "pages.widen");
}

/// The screen after the pane sensor reports `width`: the card is placed against
/// the document pane, so this is the one measurement all three placements read.
fn at_pane(props: &PagesProps, width: f32) -> Frame {
    let (_, frame) = shown(props);
    let mut root = frame.root.clone().expect("a first root");
    let next = tick_native(measure(
        &frame,
        "PagesView/root/pages/pane-measure",
        width,
        700.0,
    ));
    match &next.root {
        Some(replacement) => root = replacement.clone(),
        None => {
            ui_lang_guest::wire::apply(&mut root, next.patches.clone()).expect("patches");
        }
    }
    Frame {
        root: Some(root),
        ..Default::default()
    }
}

fn card_width(frame: &Frame) -> f32 {
    let Some(Node::Container {
        width: Some(Length::Fixed(width)),
        ..
    }) = find(frame, "PagesView/root/pages/comments-card")
    else {
        panic!("no comment card in {:?}", texts(frame));
    };
    *width
}

/// Every `max-w` in the tree. With no search answer standing the document
/// surface is the only box that carries one.
fn max_widths(node: &Node) -> Vec<f32> {
    let own = match node {
        Node::Container {
            max_width: Some(width),
            ..
        } => vec![*width],
        _ => Vec::new(),
    };
    node.children()
        .iter()
        .flat_map(max_widths)
        .chain(own)
        .collect()
}

fn float_in(node: &Node) -> Option<&Node> {
    if matches!(node, Node::Float { .. }) {
        return Some(node);
    }
    node.children().iter().find_map(float_in)
}

/// Where the card's float puts it in a pane `pane` wide, given the card's own
/// laid-out width: the same arithmetic the host runs on the float's program,
/// from the card's natural place at the pane's top-left corner.
fn card_placed(frame: &Frame, pane: f32, card: f32) -> (f32, f32) {
    let Some(Node::Float { x, y, .. }) = float_in(frame.root.as_ref().expect("a root")) else {
        panic!("no floating comment card in {:?}", texts(frame));
    };
    let geometry = [
        0.0,
        0.0,
        f64::from(card),
        300.0,
        0.0,
        0.0,
        f64::from(pane),
        700.0,
    ];
    (x.evaluate(geometry), y.evaluate(geometry))
}

#[test]
fn the_comment_card_answers_the_pane_it_is_opened_in() {
    let props = facts();
    // WIDE: the document keeps its own width and the card floats in the margin
    // it leaves, a gutter inside the pane's right edge and a gutter below the
    // header — this rail is page-scoped, so it has no line to sit on.
    let beside = at_pane(&props, 1200.0);
    assert_eq!(max_widths(beside.root.as_ref().unwrap()), vec![766.0]);
    assert_eq!(card_width(&beside), 340.0);
    assert_eq!(card_placed(&beside, 1200.0, 340.0), (1200.0 - 356.0, 16.0));

    // TIGHTER: no margin to float in, so the document gives up exactly the card
    // and its two gutters and the text reflows left of it.
    let squeeze = at_pane(&props, 1000.0);
    let document = 1000.0 - 340.0 - 32.0;
    assert_eq!(max_widths(squeeze.root.as_ref().unwrap()), vec![document]);
    assert_eq!(card_width(&squeeze), 340.0);
    let (left, top) = card_placed(&squeeze, 1000.0, 340.0);
    assert_eq!((left, top), (1000.0 - 356.0, 16.0));
    assert!(
        left >= document,
        "the card must not overlap the document it just squeezed"
    );

    // NARROW: nothing is beside anything. The document takes its full width
    // back and the card drops onto its text column at full width, into the gap
    // the reserve opens under the title.
    let inline = at_pane(&props, 900.0);
    assert_eq!(max_widths(inline.root.as_ref().unwrap()), vec![766.0]);
    assert_eq!(card_width(&inline), 766.0 - 62.0);
    assert_eq!(card_placed(&inline, 900.0, 704.0), (22.0, 67.3));
}

#[test]
fn crossing_a_placement_threshold_keeps_the_rail_and_what_is_typed_in_it() {
    let (_, frame) = shown(&facts());
    let frame = tick_native(measure(
        &frame,
        "PagesView/root/pages/pane-measure",
        1200.0,
        700.0,
    ));
    let frame = tick_native(type_into(&frame, "Start a thread…", "half a thought"));
    // The same rail, the same draft, narrower pane — and no intent leaves for it.
    let frame = tick_native(measure(
        &frame,
        "PagesView/root/pages/pane-measure",
        900.0,
        700.0,
    ));
    assert!(frame.requests.is_empty(), "a placement is view-local");
    let frame = tick_native(press(&frame, "Post"));
    assert_eq!(
        serde_json::from_slice::<Post>(&one_intent(&frame).payload).expect("decodes"),
        Post {
            text: "half a thought".into(),
            thread_id: String::new()
        }
    );
}
