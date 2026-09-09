//! The actual generated guest, driven through its editor transaction handlers.
//! Native editor layout/painting is owned by the runtime's separate host tests.
use pages_editor_binding_fixture::{boot_native, restore_native, snapshot_native, tick_native};
use ui_lang_guest::{testing, wire};
use wire::keyboard::{Key, KeyState, Location, Modifiers, Named, NativeCode, Physical};
use wire::{
    EditorCursor, EditorDecision, EditorEditKind, EditorHistoryEffect, EditorPosition,
    EditorTransactionEvent, EditorTransactionId, Event, Frame, Node,
};

struct Guest {
    root: Node,
    text: String,
    sequence: u64,
}
impl Guest {
    fn boot() -> Self {
        boot_native();
        let root = tick_native(vec![]).root.expect("first tree");
        Self {
            root,
            text: "- 한글".into(),
            sequence: 0,
        }
    }
    fn tick(&mut self, events: Vec<Event>) -> Frame {
        let frame = tick_native(events);
        if let Some(root) = &frame.root {
            self.root = root.clone();
        } else if !frame.unchanged {
            wire::apply(&mut self.root, frame.patches.clone()).expect("valid tree patch");
        }
        frame
    }
    fn editor(
        &self,
    ) -> (
        wire::editor_document::EditorDocumentRef,
        wire::EditorBinding,
    ) {
        let frame = Frame {
            root: Some(self.root.clone()),
            ..Default::default()
        };
        let Some(Node::Editor {
            document, options, ..
        }) = testing::find(&frame, "PagesEditorFixture/document")
        else {
            panic!("fixture editor missing: {:?}", self.root);
        };
        (
            document.clone(),
            *options.binding.clone().expect("real generated binding"),
        )
    }
    fn id(&mut self) -> EditorTransactionId {
        self.sequence += 1;
        let (d, _) = self.editor();
        EditorTransactionId {
            instance: 1,
            document: d.document,
            reset: d.reset,
            sequence: self.sequence,
            attempt: 0,
            text_revision: d.text_revision,
            revision: d.revision,
        }
    }
    fn request(&mut self, key: Key, shift: bool) -> wire::EditorResponse {
        let (state, binding) = self.editor();
        let command = matches!(key, Key::Character(_));
        let state_key = KeyState {
            key: key.clone(),
            modified_key: key,
            physical_key: Physical::Unidentified(NativeCode::Unidentified),
            location: Location::Standard,
            modifiers: Modifiers {
                shift,
                control: command,
                ..Default::default()
            },
        };
        assert!(
            binding
                .claims
                .iter()
                .any(|claim| claim.matches(&state_key, false)),
            "host would claim this key"
        );
        let id = self.id();
        let frame = self.tick(vec![Event::EditorKeyRequest {
            handler: binding.on_request,
            request: wire::EditorKeyRequest {
                id,
                state,
                key: state_key,
                repeat: false,
                input_time_ms: self.sequence * 1000,
            },
        }]);
        let [response] = frame.editor_decisions.as_slice() else {
            panic!("one decision: {frame:?}")
        };
        response.clone()
    }
    fn commit(
        &mut self,
        id: EditorTransactionId,
        text: String,
        cursor: EditorCursor,
        effect: EditorHistoryEffect,
        kind: EditorEditKind,
    ) {
        let (before, binding) = self.editor();
        let mut after = before.clone();
        after.revision += 1;
        after.text_revision += u64::from(self.text != text);
        after.cursor = cursor;
        after.byte_len = text.len() as u32;
        let patches = wire::editor_document::editor_changed_span(&self.text, &text)
            .expect("native-representable patch");
        self.tick(vec![Event::EditorTransaction {
            handler: binding.on_event,
            event: EditorTransactionEvent::Commit {
                id,
                before,
                after,
                patches,
                kind,
                history: effect,
                input_time_ms: self.sequence * 1000,
            },
        }]);
        self.text = text;
        let frame = Frame {
            root: Some(self.root.clone()),
            ..Default::default()
        };
        assert!(
            testing::has_text(&frame, &self.text),
            "accepted state reaches rendered echo before history route"
        );
    }
    fn apply(&mut self, response: wire::EditorResponse) {
        let EditorDecision::Apply {
            patches,
            cursor,
            history,
        } = response.decision
        else {
            panic!("expected Apply")
        };
        let text = wire::editor_transaction::patched_editor_text(&self.text, &patches, cursor)
            .expect("native accepts patch endpoints");
        self.commit(
            response.id,
            text,
            cursor,
            history,
            EditorEditKind::GuestPatch,
        );
    }
    fn caret(&mut self, cursor: EditorCursor) {
        let id = self.id();
        self.commit(
            id,
            self.text.clone(),
            cursor,
            EditorHistoryEffect::Native,
            EditorEditKind::Cursor,
        );
    }
}
fn at(line: u32, column: u32) -> EditorCursor {
    EditorCursor {
        position: EditorPosition { line, column },
        selection: None,
    }
}

#[test]
fn korean_enter_cancelled_undo_and_snapshot_redo_use_the_real_guest_binding() {
    let mut guest = Guest::boot();
    guest.caret(at(0, 8));
    let enter = guest.request(Key::Named(Named::Enter), false);
    guest.apply(enter);
    assert_eq!(guest.text, "- 한글\n- ");
    assert_eq!(guest.editor().0.cursor, at(1, 2));
    let undo = guest.request(Key::Character("z".into()), false);
    let (state, binding) = guest.editor();
    guest.tick(vec![Event::EditorTransaction {
        handler: binding.on_event,
        event: EditorTransactionEvent::Cancelled { id: undo.id, state },
    }]);
    let undo_again = guest.request(Key::Character("z".into()), false);
    guest.apply(undo_again);
    assert_eq!(guest.text, "- 한글");
    assert_eq!(guest.editor().0.cursor, at(0, 8));
    let snapshot = snapshot_native().expect("settled history snapshots");
    restore_native(&snapshot, false).expect("restore history without init");
    guest.tick(vec![]);
    let redo = guest.request(Key::Character("z".into()), true);
    guest.apply(redo);
    assert_eq!(guest.text, "- 한글\n- ");
}

#[test]
fn native_emoji_edit_and_selected_undo_preserve_byte_columns_and_graphemes() {
    let mut guest = Guest::boot();
    let id = guest.id();
    guest.commit(
        id,
        "- 👍🏽".into(),
        at(0, 10),
        EditorHistoryEffect::Native,
        EditorEditKind::ImeCommit,
    );
    let selected = EditorCursor {
        position: EditorPosition {
            line: 0,
            column: 10,
        },
        selection: Some(EditorPosition { line: 0, column: 2 }),
    };
    guest.caret(selected);
    let id = guest.id();
    guest.commit(
        id,
        "- 🇰🇷".into(),
        at(0, 10),
        EditorHistoryEffect::NewGroup,
        EditorEditKind::Paste,
    );
    let undo = guest.request(Key::Character("z".into()), false);
    guest.apply(undo);
    assert_eq!(guest.text, "- 👍🏽");
    assert_eq!(guest.editor().0.cursor, selected);
    let redo = guest.request(Key::Character("z".into()), true);
    guest.apply(redo);
    assert_eq!(guest.text, "- 🇰🇷");
    assert_eq!(guest.editor().0.cursor.selection, None);
}

#[test]
fn undoing_a_group_that_returned_to_its_original_text_still_consumes_the_step() {
    let mut guest = Guest::boot();
    guest.caret(at(0, 8));
    let enter = guest.request(Key::Named(Named::Enter), false);
    guest.apply(enter);
    let id = guest.id();
    guest.commit(
        id,
        "- 한글".into(),
        at(0, 8),
        EditorHistoryEffect::ExtendPrevious,
        EditorEditKind::Backspace,
    );
    let undo = guest.request(Key::Character("z".into()), false);
    guest.apply(undo);
    let no_second_step = guest.request(Key::Character("z".into()), false);
    assert_eq!(no_second_step.decision, EditorDecision::Noop);
}

#[test]
fn interrupted_bootstrap_keeps_the_old_document_and_restores_from_a_new_begin() {
    use wire::editor_document::{
        EditorDocumentMessage, EditorTransferId, EditorTransferReceiver, EditorTransferSender,
        MAX_EDITOR_DOCUMENT_BYTES,
    };
    let mut guest = Guest::boot();
    guest.caret(at(0, 8));
    let enter = guest.request(Key::Named(Named::Enter), false);
    guest.apply(enter);
    let previous = guest.editor().0;
    let shown = Frame {
        root: Some(guest.root.clone()),
        ..Default::default()
    };
    let frame = guest.tick(testing::press(&shown, "Load document"));
    let request = frame
        .requests
        .iter()
        .find(|request| request.kind == "pages.document")
        .expect("document subscription");
    let source: wire::editor_document::EditorDocumentRef = wire::decode(&request.payload).unwrap();
    let first_request = request.id;
    let text = format!(
        "{}{}",
        "한".repeat(MAX_EDITOR_DOCUMENT_BYTES / 3),
        "x".repeat(MAX_EDITOR_DOCUMENT_BYTES % 3)
    );
    let id = EditorTransferId {
        instance: 1,
        document: source.document.clone(),
        reset: source.reset,
        serial: 1,
        attempt: 0,
    };
    let mut first = EditorTransferSender::new(id, source.clone()).unwrap();
    for _ in 0..3 {
        let transfer = first.next_frame(&source, &text).unwrap().unwrap();
        guest.tick(vec![testing::item(first_request, &wire::encode(&transfer))]);
        assert_eq!(
            guest.editor().0,
            previous,
            "partial bytes never replace the last committed document or caret"
        );
    }
    let snapshot = snapshot_native().expect("partial subscription state is disposable");
    restore_native(&snapshot, false).unwrap();
    let resumed = guest.tick(vec![]);
    assert_eq!(guest.editor().0, previous);
    let request = resumed
        .requests
        .iter()
        .find(|request| request.kind == "pages.document")
        .expect("restored subscription asks from Begin");
    assert_eq!(request.payload, wire::encode(&source));
    let resumed_id = request.id;
    let id = EditorTransferId {
        instance: 2,
        document: source.document.clone(),
        reset: source.reset,
        serial: 2,
        attempt: 0,
    };
    let mut sender = EditorTransferSender::new(id, source.clone()).unwrap();
    while let Some(transfer) = sender.next_frame(&source, &text).unwrap() {
        let bytes = wire::encode(&transfer);
        assert!(
            bytes.len() < 1024 * 1024,
            "each capability item fits the payload cap with metadata"
        );
        guest.tick(vec![testing::item(resumed_id, &bytes)]);
    }
    let (installed, _) = guest.editor();
    assert_eq!(installed.byte_len as usize, MAX_EDITOR_DOCUMENT_BYTES);
    let undo = guest.request(Key::Character("z".into()), false);
    assert_eq!(
        undo.decision,
        EditorDecision::Noop,
        "a replaced document cannot consume the previous document's history"
    );
    let (state, binding) = guest.editor();
    guest.tick(vec![Event::EditorTransaction {
        handler: binding.on_event,
        event: EditorTransactionEvent::Cancelled { id: undo.id, state },
    }]);
    // Read the canonical guest bytes through the runtime's document transfer,
    // rather than trusting only the advertised length or a display echo.
    let frame = Frame {
        root: Some(guest.root.clone()),
        ..Default::default()
    };
    let Some(Node::Editor { on_document, .. }) =
        testing::find(&frame, "PagesEditorFixture/document")
    else {
        panic!("editor")
    };
    let handler = *on_document;
    let id = EditorTransferId {
        instance: 3,
        document: installed.document.clone(),
        reset: installed.reset,
        serial: 3,
        attempt: 0,
    };
    let mut receiver = EditorTransferReceiver::new(id.clone(), installed.clone()).unwrap();
    let mut frame = guest.tick(vec![Event::EditorDocument {
        handler,
        message: EditorDocumentMessage::Request {
            id: id.clone(),
            target: installed,
        },
    }]);
    let mut actual = None;
    for _ in 0..20 {
        for message in frame.editor_documents {
            let EditorDocumentMessage::Transfer(transfer) = message else {
                panic!("expected source transfer")
            };
            if let Some(complete) = receiver.receive(&transfer).unwrap() {
                actual = Some(complete);
            }
        }
        if actual.is_some() {
            break;
        }
        frame = guest.tick(vec![]);
    }
    assert_eq!(
        actual.as_deref(),
        Some(text.as_str()),
        "all UTF8 bytes survive initial ingress and snapshot restart"
    );
    guest.tick(vec![Event::EditorDocument {
        handler,
        message: EditorDocumentMessage::Acknowledged { id },
    }]);
}
