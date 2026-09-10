//! The facts the host pushes are what the screen shows; every act leaves as
//! an intent carrying what the reader typed, and a draft the app hands back
//! lands in the field only when the seed moved.

use pages_view::host::{Choose, Create, PageItem, PagesProps, Post, Search};
use pages_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, press, submit, texts, type_into};
use ui_lang_guest::wire::Frame;

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
        compose_hint: "Comment on the page".into(),
        ..PagesProps::default()
    }
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
fn the_facts_the_host_pushes_are_what_the_screen_shows_and_a_pick_carries_the_rail_draft() {
    let (_, frame) = shown(&facts());
    for expected in [
        "Pages",
        "Alpha",
        "Beta",
        "✓ synced",
        "Comments",
        "No comments yet",
    ] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
    let frame = tick_native(type_into(&frame, "Add a comment…", "half a thought"));
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
    let frame = tick_native(type_into(&frame, "Add a comment…", "looks right"));
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
            text: "looks right".into()
        }
    );
    // the field cleared with the act: the button is dark now
    assert!(
        matches!(
            ui_lang_guest::testing::find(&frame, "PagesView/root/pages/post"),
            Some(ui_lang_guest::wire::Node::Button { on_press: None, .. })
        ),
        "{:?}",
        texts(&frame)
    );
}

#[test]
fn a_draft_the_app_hands_back_lands_only_when_the_seed_moved() {
    let (subscription, frame) = shown(&facts());
    let _ = tick_native(type_into(&frame, "Add a comment…", "mine"));
    // the same seed pushed again changes nothing …
    let frame = tick_native(vec![item(subscription, &encoded(&facts()))]);
    let frame = tick_native(press(&frame, "Post"));
    assert_eq!(
        serde_json::from_slice::<Post>(&one_intent(&frame).payload).expect("decodes"),
        Post {
            text: "mine".into()
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
            text: "the refused one".into()
        }
    );
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
}
