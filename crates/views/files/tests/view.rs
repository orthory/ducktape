//! The facts the host pushes are what the browser shows; every navigation
//! and every write leaves as an intent carrying what the reader chose or
//! typed, and a committed write the host reports consumes the name draft.

use files_view::host::{FilesProps, FsEntry, Name, Path, Save};
use files_view::{boot_native, tick_native};
use ui_lang_guest::testing::{find, has_text, item, keys, press, texts, type_into};
use ui_lang_guest::wire::{Frame, Node};

fn entry(key: i64, path: &str, kind: &str, size: i64) -> FsEntry {
    FsEntry {
        key,
        path: path.into(),
        name: path.rsplit('/').next().unwrap_or(path).into(),
        kind: kind.into(),
        size,
        object: format!("object-{key}"),
    }
}

fn facts() -> FilesProps {
    let docs = entry(1, "/shared/docs", "dir", 2);
    let readme = entry(2, "/shared/README.md", "file", 421_888);
    FilesProps {
        path: "/shared".into(),
        listed: true,
        entries: vec![docs.clone(), readme.clone()],
        directories: vec![docs],
        connected: true,
        loading: false,
        preview_path: "/shared/README.md".into(),
        preview_entry: readme,
        delete_target: String::new(),
        diff_from: String::new(),
        diff: Vec::new(),
        history: Vec::new(),
        preview_truncated: false,
        preview_binary: false,
        preview_picture: false,
        preview_width: 0,
        preview_height: 0,
        preview_text: "# Hello\n".into(),
        dark: false,
        write_refusal: String::new(),
        writes: 0,
    }
}

fn encoded(props: &FilesProps) -> Vec<u8> {
    serde_json::to_vec(props).expect("props encode")
}

/// Boot and push the facts; returns the subscription id and the frame.
fn shown(props: &FilesProps) -> (u64, Frame) {
    boot_native();
    let frame = tick_native(Vec::new());
    assert_eq!(frame.requests[0].kind, "files.props");
    let subscription = frame.requests[0].id;
    let frame = tick_native(vec![item(subscription, &encoded(props))]);
    (subscription, frame)
}

/// What the write bar's name field reads now.
fn name_field(frame: &Frame) -> String {
    let key = keys(frame)
        .into_iter()
        .find(|key| key.ends_with("/fs-new"))
        .expect("the name field");
    match find(frame, &key) {
        Some(Node::Input { value, .. }) => value.clone(),
        other => panic!("not an input: {other:?}"),
    }
}

fn one_intent(frame: &Frame) -> &ui_lang_guest::wire::Request {
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    intent
}

#[test]
fn the_facts_the_host_pushes_are_what_the_browser_shows_and_a_row_opens_as_a_path() {
    let (_, frame) = shown(&facts());
    for expected in ["duckfs", "/shared", "1 file · 1 dir", "README.md", "412 KB"] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
    let frame = tick_native(press(&frame, "Open directory"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "files.open_dir");
    assert_eq!(
        serde_json::from_slice::<Path>(&intent.payload).expect("decodes"),
        Path {
            path: "/shared/docs".into()
        }
    );
    let frame = tick_native(press(&frame, "Parent directory"));
    assert_eq!(one_intent(&frame).kind, "files.open_parent");
}

#[test]
fn a_committed_write_consumes_the_name_it_read() {
    let (subscription, frame) = shown(&facts());
    let frame = tick_native(type_into(&frame, "new name…", "  reports  "));
    assert!(frame.requests.is_empty(), "typing runs no handler");
    let frame = tick_native(press(&frame, "+ Folder"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "files.mkdir");
    assert_eq!(
        serde_json::from_slice::<Name>(&intent.payload).expect("decodes"),
        Name {
            name: "reports".into()
        }
    );
    assert_eq!(
        name_field(&frame),
        "  reports  ",
        "the draft stays until a write lands"
    );
    // the write landed: the name goes …
    let written = FilesProps {
        writes: 1,
        ..facts()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&written))]);
    assert_eq!(name_field(&frame), "");
    // … and the same report pushed again consumes nothing more
    let typed = tick_native(type_into(&frame, "new name…", "notes"));
    assert_eq!(name_field(&typed), "notes");
    let frame = tick_native(vec![item(subscription, &encoded(&written))]);
    assert_eq!(name_field(&frame), "notes");
    // a refused directory says so under the bar and keeps the draft
    let refused = FilesProps {
        write_refusal: "roots are not writable".into(),
        ..written
    };
    let frame = tick_native(vec![item(subscription, &encoded(&refused))]);
    assert!(has_text(&frame, "roots are not writable"));
    assert_eq!(name_field(&frame), "notes");
}

#[test]
fn the_edited_body_leaves_on_save_and_the_pane_drops_back_to_the_reader() {
    let (_, frame) = shown(&facts());
    let frame = tick_native(press(&frame, "Edit"));
    assert!(frame.requests.is_empty(), "editing is the view's own");
    assert!(has_text(&frame, "Save"), "{:?}", texts(&frame));
    let frame = tick_native(press(&frame, "Save"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "files.save");
    assert_eq!(
        serde_json::from_slice::<Save>(&intent.payload).expect("decodes"),
        Save {
            path: "/shared/README.md".into(),
            text: "# Hello\n".into()
        }
    );
    assert!(!has_text(&frame, "Save"), "{:?}", texts(&frame));
}
