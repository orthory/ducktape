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
