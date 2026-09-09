//! The facts the host pushes are what the browser shows; every navigation
//! and every write leaves as an intent carrying what the reader chose or
//! typed, and a committed write the host reports consumes the name draft.

use files_view::host::{FilesProps, FsEntry, Name, Path, Save, SaveHistory, SaveReply};
use files_view::{boot_native, tick_native};
use ui_lang_guest::testing::{edit, find, has_text, item, keys, press, texts, type_into};
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
        save_namespace: "guest-a".into(),
        network_scope: "network-a".into(),
        context: "connection-a".into(),
        preview_base: "snapshot-a".into(),
        save_reply: SaveHistory::default(),
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
        preview_display_text: "# Hello\n".into(),
        preview_display_clipped: false,
        dark: false,
        write_refusal: String::new(),
        writes: 0,
        ..FilesProps::default()
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
fn the_edited_body_stays_until_its_exact_save_is_committed() {
    let (subscription, frame) = shown(&facts());
    let frame = tick_native(press(&frame, "Edit"));
    assert!(frame.requests.is_empty(), "editing is the view's own");
    assert!(has_text(&frame, "Save"), "{:?}", texts(&frame));
    let frame = tick_native(press(&frame, "Save"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "files.save");
    assert_eq!(
        serde_json::from_slice::<Save>(&intent.payload).expect("decodes"),
        Save {
            namespace: "guest-a".into(),
            context: "connection-a".into(),
            base: "snapshot-a".into(),
            request: 1,
            path: "/shared/README.md".into(),
            text: "# Hello\n".into()
        }
    );
    assert!(
        has_text(&frame, "Save"),
        "unacknowledged edits stay in the editor"
    );
    let committed = FilesProps {
        // Restore keeps the pending request even when the new host instance supplies a new namespace.
        save_namespace: "guest-replacement".into(),
        save_reply: SaveReply {
            namespace: "guest-a".into(),
            context: "connection-a".into(),
            request: 1,
            success: true,
            message: String::new(),
        }
        .into(),
        ..facts()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&committed))]);
    assert!(!has_text(&frame, "Save"));
}

#[test]
fn a_save_queued_before_navigation_never_targets_the_new_file() {
    let original = facts();
    let (subscription, frame) = shown(&original);
    let editing = tick_native(press(&frame, "Edit"));
    let queued_save = press(&editing, "Save");
    let next = FilesProps {
        preview_path: "/shared/other.md".into(),
        preview_entry: entry(3, "/shared/other.md", "file", 4),
        preview_text: "other file".into(),
        ..facts()
    };
    tick_native(vec![item(subscription, &encoded(&next))]);
    let after = tick_native(queued_save);
    for request in &after.requests {
        if request.kind == "files.save" {
            let save: Save = serde_json::from_slice(&request.payload).unwrap();
            assert_eq!(
                save.path, original.preview_path,
                "a queued Save must never retarget the old draft to a new file"
            );
        }
    }
}

fn read_draft(frame: &Frame) -> (Frame, String) {
    use ui_lang_guest::wire::editor_document::{
        EditorDocumentMessage as Message, EditorTransferId, EditorTransferReceiver,
    };
    let key = keys(frame)
        .into_iter()
        .find(|key| key.ends_with("/fs-editor"))
        .expect("editor present");
    let Some(Node::Editor {
        document,
        on_document,
        ..
    }) = find(frame, &key)
    else {
        panic!("no editor in {:?}", keys(frame));
    };
    let handler = *on_document;
    let id = EditorTransferId {
        instance: 1,
        document: document.document.clone(),
        reset: document.reset,
        serial: document.revision,
        attempt: 0,
    };
    let mut receiver = EditorTransferReceiver::new(id.clone(), document.clone()).unwrap();
    let mut events = vec![ui_lang_guest::wire::Event::EditorDocument {
        handler,
        message: Message::Request {
            id: id.clone(),
            target: document.clone(),
        },
    }];
    for _ in 0..4 {
        let frame = tick_native(std::mem::take(&mut events));
        for message in &frame.editor_documents {
            let Message::Transfer(transfer) = message else {
                panic!("document transfer: {message:?}");
            };
            if let Some(text) = receiver.receive(transfer).unwrap() {
                let settled = tick_native(vec![ui_lang_guest::wire::Event::EditorDocument {
                    handler,
                    message: Message::Acknowledged { id },
                }]);
                return (settled, text);
            }
        }
    }
    panic!("the small Files document must finish its bounded transfer");
}

#[test]
fn parked_draft_returns_with_its_original_bytes_and_snapshot_after_reconnect() {
    let original = facts();
    let (subscription, frame) = shown(&original);
    let editing = tick_native(press(&frame, "Edit"));
    let editor_key = keys(&editing)
        .into_iter()
        .find(|key| key.ends_with("/fs-editor"))
        .unwrap();
    let (editing, before) = read_draft(&editing);
    let editing = tick_native(edit(&editing, &editor_key, &before, "unsaved A — 한글"));
    let stale_save = press(&editing, "Save");
    let other = FilesProps {
        network_scope: "network-b".into(),
        context: "connection-b".into(),
        preview_text: "B source".into(),
        ..facts()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&other))]);
    assert!(has_text(&frame, "Unsaved changes to:"));
    assert!(!has_text(&frame, "Save"));
    let frame = tick_native(stale_save);
    assert!(frame.requests.is_empty());
    let returned = FilesProps {
        context: "connection-a-reconnected".into(),
        preview_base: "snapshot-new".into(),
        preview_text: "external edit".into(),
        ..facts()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&returned))]);
    let (frame, text) = read_draft(&frame);
    assert_eq!(text, "unsaved A — 한글");
    let frame = tick_native(press(&frame, "Save"));
    let saved: Save = serde_json::from_slice(&one_intent(&frame).payload).unwrap();
    assert_eq!(saved.context, returned.context);
    assert_eq!(saved.base, original.preview_base);
    assert_eq!(saved.text, "unsaved A — 한글");
    let refused = FilesProps {
        save_reply: SaveReply {
            namespace: "guest-a".into(),
            context: returned.context.clone(),
            request: saved.request,
            success: false,
            message: "The file changed elsewhere. Your edits are kept.".into(),
        }
        .into(),
        ..returned
    };
    let frame = tick_native(vec![item(subscription, &encoded(&refused))]);
    let (frame, text) = read_draft(&frame);
    assert_eq!(text, "unsaved A — 한글");
    assert!(has_text(
        &frame,
        "The file changed elsewhere. Your edits are kept."
    ));
}

#[test]
fn an_old_save_acknowledgement_cannot_consume_a_new_draft() {
    let (subscription, frame) = shown(&facts());
    let frame = tick_native(press(&frame, "Edit"));
    let frame = tick_native(press(&frame, "Save"));
    let a: Save = serde_json::from_slice(&one_intent(&frame).payload).unwrap();
    let other = FilesProps {
        network_scope: "network-b".into(),
        context: "connection-b".into(),
        preview_text: "B source".into(),
        ..facts()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&other))]);
    let frame = tick_native(press(&frame, "Discard unsaved changes"));
    let frame = tick_native(press(&frame, "Edit"));
    let frame = tick_native(press(&frame, "Save"));
    let b: Save = serde_json::from_slice(&one_intent(&frame).payload).unwrap();
    assert_ne!(a.request, b.request);
    let late = FilesProps {
        save_reply: SaveReply {
            namespace: "guest-a".into(),
            context: a.context,
            request: a.request,
            success: true,
            message: String::new(),
        }
        .into(),
        ..other
    };
    let frame = tick_native(vec![item(subscription, &encoded(&late))]);
    assert!(
        has_text(&frame, "Save"),
        "an old acknowledgement cannot close B's editor"
    );
    let mut pending = false;
    frame.root.clone().unwrap().for_each_mut(&mut |node| {
        if let Node::Button {
            content: ui_lang_guest::wire::ButtonContent::Label(label),
            on_press,
            ..
        } = node
            && label == "Save"
        {
            pending = on_press.is_none();
        }
    });
    assert!(pending, "B remains pending");
    let (_, text) = read_draft(&frame);
    assert_eq!(text, "B source");
}

#[test]
fn a_fresh_guest_never_consumes_the_previous_instances_save_reply() {
    let old_success = SaveReply {
        namespace: "guest-old".into(),
        context: "connection-a".into(),
        request: 1,
        success: true,
        message: String::new(),
    };
    // A retained reply and a reply still in flight when the old guest died.
    for initial_reply in [old_success.clone(), SaveReply::default()] {
        let initial = FilesProps {
            save_namespace: "guest-new".into(),
            save_reply: initial_reply.into(),
            ..facts()
        };
        let (subscription, frame) = shown(&initial);
        let frame = tick_native(press(&frame, "Edit"));
        let key = keys(&frame)
            .into_iter()
            .find(|key| key.ends_with("/fs-editor"))
            .unwrap();
        let (frame, before) = read_draft(&frame);
        let frame = tick_native(edit(&frame, &key, &before, "new unsaved text"));
        let frame = tick_native(press(&frame, "Save"));
        let saved: Save = serde_json::from_slice(&one_intent(&frame).payload).unwrap();
        assert_eq!(saved.namespace, "guest-new");
        assert_eq!(saved.request, 1, "fresh guest restarts its local counter");
        let late = FilesProps {
            save_reply: old_success.clone().into(),
            ..initial
        };
        let frame = tick_native(vec![item(subscription, &encoded(&late))]);
        assert!(
            has_text(&frame, "Save"),
            "a previous instance's success cannot consume the fresh draft"
        );
        let (_, text) = read_draft(&frame);
        assert_eq!(text, "new unsaved text");
        let confirmed = FilesProps {
            save_reply: SaveReply {
                namespace: saved.namespace,
                ..old_success.clone()
            }
            .into(),
            ..late
        };
        let frame = tick_native(vec![item(subscription, &encoded(&confirmed))]);
        assert!(
            !has_text(&frame, "Save"),
            "the new save's own reply consumes it"
        );
    }
}

#[test]
fn lost_confirmation_never_discards_the_draft_or_waits_forever() {
    let (subscription, frame) = shown(&facts());
    let frame = tick_native(press(&frame, "Edit"));
    let key = keys(&frame)
        .into_iter()
        .find(|key| key.ends_with("/fs-editor"))
        .unwrap();
    let (frame, before) = read_draft(&frame);
    let frame = tick_native(edit(
        &frame,
        &key,
        &before,
        "unsaved bytes after history overflow",
    ));
    let frame = tick_native(press(&frame, "Save"));
    assert_eq!(one_intent(&frame).kind, "files.save");
    let overflowed = FilesProps {
        save_reply: SaveHistory {
            replies: Vec::new(),
            overflow: "new-overflow".into(),
        },
        ..facts()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&overflowed))]);
    let (frame, text) = read_draft(&frame);
    assert_eq!(text, "unsaved bytes after history overflow");
    assert!(has_text(
        &frame,
        "Save confirmation is no longer available. Your edits are still here; check the file before saving again."
    ));
    assert!(
        !press(&frame, "Save").is_empty(),
        "the editor is not stranded waiting for an evicted reply"
    );
}

#[test]
fn omitted_rows_are_a_number_not_literal_template_text() {
    let (_, frame) = shown(&FilesProps {
        display_omitted: 12_345,
        ..facts()
    });
    let key = keys(&frame)
        .into_iter()
        .find(|key| key.ends_with("/display-omitted"))
        .expect("the omission count has its own identity");
    assert!(matches!(find(&frame, &key), Some(Node::Text { content, .. }) if content == "12345"));
    assert!(has_text(&frame, "rows are not shown."));
    assert!(has_text(&frame, "Edit"));
}
