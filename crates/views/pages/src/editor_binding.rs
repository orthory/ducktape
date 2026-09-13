//! Pages' structural keys and commit-confirmed history on the guest editor lane.
//! Rich document painting, gutter actions and anchored menus are independent
//! consumers; this adapter does not replace them with a plain editor.

use crate::editor::{self, Doc, History};
use ducktape_view_guest::wire::{self, EditorDecision, EditorHistoryEffect, EditorKeyClaim};
use ducktape_view_guest::{EditorBinding, EditorKeyRequest, EditorTransactionEvent};
use std::{cell::RefCell, rc::Rc};
use wire::keyboard::{Key, Modifiers, Named};

/// Guest-owned editor history retained in state snapshots.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryState {
    pub snapshot: Vec<u8>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct StoredHistory {
    reset: Option<u64>,
    history: History,
}

impl StoredHistory {
    fn at_reset(&mut self, reset: u64) {
        if self.reset != Some(reset) {
            self.history.reset();
            self.reset = Some(reset);
        }
    }

    fn save(&self) -> HistoryState {
        HistoryState {
            snapshot: wire::encode(self),
        }
    }
}

pub fn initial_history() -> HistoryState {
    HistoryState::default()
}

/// Small menu state is separate from the bounded undo snapshots so painting
/// never needs to decode the document history.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MenuState {
    pub snapshot: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorUpdate {
    pub notice: String,
    pub history: HistoryState,
    pub menu: MenuState,
    /// The accepted canonical document, without copying its text into an intent.
    pub reference: Vec<u8>,
    /// Read-only presentation actions (link and comment targets) are not edits.
    pub interaction: Vec<u8>,
}

pub fn initial_menu() -> MenuState {
    MenuState::default()
}

/// The menu part of Pages presentation; the Markdown pass adds its spans and
/// document affordances to this same declarative value.
pub fn menu_paint(
    state: ducktape_view_guest::EditorStateView<'_>,
    menu: MenuState,
) -> wire::editor_presentation::EditorPresentation {
    use wire::editor_presentation::{
        EditorMenu, EditorMenuAnchor, EditorMenuItem, EditorPresentation,
    };
    let menu: crate::editor_menu::Menu = if menu.snapshot.is_empty() {
        Default::default()
    } else {
        wire::decode(&menu.snapshot).expect("Pages menu snapshot")
    };
    let mut presentation = EditorPresentation::default();
    if !menu.is_open() {
        return presentation;
    }
    presentation.affordances.menu = menu
        .current(&document(state.text, state.cursor))
        .map(|view| EditorMenu {
            anchor: view.line.map_or(EditorMenuAnchor::Caret, |line| {
                EditorMenuAnchor::Line(line as u32)
            }),
            items: view
                .items
                .into_iter()
                .map(|(tag, label)| EditorMenuItem {
                    tag: tag.into(),
                    label: label.into(),
                })
                .collect(),
            selected: view.selected as u32,
        });
    presentation
}

struct BindingState {
    history: StoredHistory,
    menu: crate::editor_menu::Menu,
}
impl BindingState {
    fn update(
        &self,
        id: &wire::EditorTransactionId,
        state: ducktape_view_guest::EditorStateView<'_>,
        interaction: Vec<u8>,
    ) -> EditorUpdate {
        let reference = wire::editor_document::EditorDocumentRef {
            document: id.document.clone(),
            reset: state.reset,
            revision: state.revision,
            text_revision: state.text_revision,
            byte_len: state.text.len() as u32,
            cursor: state.cursor,
        };
        EditorUpdate {
            notice: crate::presentation::format_notice(state.text),
            history: self.history.save(),
            menu: MenuState {
                snapshot: wire::encode(&self.menu),
            },
            reference: wire::encode(&reference),
            interaction,
        }
    }

    fn observe(&mut self, event: EditorTransactionEvent<'_>) -> Option<EditorUpdate> {
        match event {
            event @ EditorTransactionEvent::Commit { .. } => self.committed(event),
            EditorTransactionEvent::Interaction {
                id, state, action, ..
            } => self.interacted(id, state, action),
            EditorTransactionEvent::Fault { .. } => None,
            EditorTransactionEvent::Cancelled { .. } => None,
        }
    }

    fn interacted(
        &mut self,
        id: &wire::EditorTransactionId,
        state: ducktape_view_guest::EditorStateView<'_>,
        action: &wire::editor_presentation::EditorInteraction,
    ) -> Option<EditorUpdate> {
        if self.history.reset != Some(state.reset) {
            self.menu.close();
        }
        self.history.at_reset(state.reset);
        let doc = document(state.text, state.cursor);
        let mut navigation = crate::document_sync::Navigation::default();
        let comment_pick = matches!(action, wire::editor_presentation::EditorInteraction::MenuPick { tag } if tag == "comment");
        if comment_pick {
            navigation.comment_line = self
                .menu
                .current(&doc)
                .filter(|menu| menu.items.iter().any(|item| item.0 == "comment"))
                .and_then(|menu| menu.line)
                .map(|line| line as u32);
        }
        let (_, successor) = interaction(&doc, self.menu.clone(), action);
        self.menu = successor;
        match action {
            wire::editor_presentation::EditorInteraction::Margin { line } => {
                navigation.comment_line = Some(*line);
            }
            wire::editor_presentation::EditorInteraction::LinePress { tag: 2, position } => {
                if let Some(line) = wire::editor_lines(state.text).nth(position.line as usize) {
                    navigation.link =
                        crate::inline::document_link_at(line, position.column as usize)
                            .unwrap_or_default();
                }
            }
            _ => {}
        }
        Some(self.update(id, state, wire::encode(&navigation)))
    }

    fn committed(&mut self, event: EditorTransactionEvent<'_>) -> Option<EditorUpdate> {
        let EditorTransactionEvent::Commit {
            id,
            before,
            after,
            kind,
            history: effect,
            input_time_ms,
            origin,
        } = event
        else {
            unreachable!("observe routes only Commit here")
        };
        let replaced = self.history.reset != Some(before.reset);
        self.history.at_reset(before.reset);
        if replaced {
            self.menu.close();
        }
        let after_doc = document(after.text, after.cursor);
        if let Some(wire::EditorRequestInput::Interaction { action }) = origin {
            let (_, successor) = interaction(
                &document(before.text, before.cursor),
                self.menu.clone(),
                action,
            );
            self.menu = successor;
        } else {
            if matches!(kind, wire::EditorEditKind::Cursor) {
                self.menu.close();
            } else {
                let inserted_slash = matches!(kind, wire::EditorEditKind::Insert)
                    && after.text_revision != before.text_revision
                    && after_doc
                        .line(after.cursor.position.line as usize)
                        .and_then(|line| line.get(..after.cursor.position.column as usize))
                        .is_some_and(|prefix| prefix.ends_with('/'));
                self.menu.after_edit(&after_doc, inserted_slash);
            }
        }
        let changes_history = before.text_revision != after.text_revision
            || matches!(
                effect,
                EditorHistoryEffect::Undo | EditorHistoryEffect::Redo
            );
        if changes_history {
            self.history.history.commit(
                &document(before.text, before.cursor),
                &after_doc,
                history_effect(effect),
                input_time_ms,
            );
        }
        Some(self.update(id, after, Vec::new()))
    }
}

/// Claims only structural keys and the platform command's undo/redo keys.
/// All other input stays native and joins this history only after commit.
pub fn keys(state: HistoryState, menu: MenuState) -> EditorBinding<EditorUpdate> {
    let history = if state.snapshot.is_empty() {
        StoredHistory::default()
    } else {
        wire::decode(&state.snapshot).expect("Pages history snapshot")
    };
    let menu = if menu.snapshot.is_empty() {
        crate::editor_menu::Menu::default()
    } else {
        wire::decode(&menu.snapshot).expect("Pages menu snapshot")
    };
    let state = Rc::new(RefCell::new(BindingState { history, menu }));
    let deciding = state.clone();
    let interacting = state.clone();
    let bare = Modifiers::default();
    let mut claims = [Named::Enter, Named::Tab, Named::Backspace]
        .into_iter()
        .map(|key| EditorKeyClaim {
            key: Key::Named(key),
            modifiers: bare,
            command: false,
        })
        .collect::<Vec<_>>();
    claims.push(EditorKeyClaim {
        key: Key::Named(Named::Tab),
        modifiers: Modifiers {
            shift: true,
            ..bare
        },
        command: false,
    });
    for shift in [false, true] {
        claims.push(EditorKeyClaim {
            key: Key::Character("z".into()),
            modifiers: Modifiers { shift, ..bare },
            command: true,
        });
    }
    EditorBinding::new(
        claims,
        move |request| {
            let state = deciding.borrow();
            if state.history.reset == Some(request.state.reset) {
                decide(request, &state.history.history)
            } else {
                decide(request, &History::default())
            }
        },
        move |event| state.borrow_mut().observe(event),
    )
    .on_interaction(move |request| {
        let state = interacting.borrow();
        let doc = document(request.state.text, request.state.cursor);
        let menu = if state.history.reset == Some(request.state.reset) {
            state.menu.clone()
        } else {
            crate::editor_menu::Menu::default()
        };
        let (decision, _) = interaction(&doc, menu, request.action);
        wire_decision(&doc, decision)
    })
}

fn interaction(
    doc: &Doc,
    mut menu: crate::editor_menu::Menu,
    action: &wire::editor_presentation::EditorInteraction,
) -> (editor::EditorDecision, crate::editor_menu::Menu) {
    use editor::EditorDecision::Noop;
    use wire::editor_presentation::{EditorGutterButton, EditorInteraction};
    match action {
        EditorInteraction::Gutter {
            line,
            button: EditorGutterButton::Plus,
        } => menu.plus(doc, *line as usize),
        EditorInteraction::Gutter {
            line,
            button: EditorGutterButton::Handle,
        } => {
            menu.block(doc, *line as usize);
            (Noop, menu)
        }
        EditorInteraction::GutterDrop { from, boundary } => {
            menu.close();
            (
                crate::editor_menu::drop_move(doc, *from as usize, *boundary as usize),
                menu,
            )
        }
        EditorInteraction::MenuPick { tag } => menu.pick(doc, tag),
        EditorInteraction::MenuSelect { index } => {
            menu.select(doc, *index as usize);
            (Noop, menu)
        }
        EditorInteraction::MenuDismiss => {
            menu.close();
            (Noop, menu)
        }
        EditorInteraction::LinePress { tag: 1, position } => {
            menu.close();
            (
                crate::editor_menu::toggle_todo(doc, position.line as usize),
                menu,
            )
        }
        EditorInteraction::LinePress { .. } | EditorInteraction::Margin { .. } => (Noop, menu),
    }
}

fn document(text: &str, cursor: wire::EditorCursor) -> Doc {
    Doc::new(
        text,
        editor::EditorCursor {
            position: position(cursor.position),
            selection: cursor.selection.map(position),
        },
    )
}
fn position(value: wire::EditorPosition) -> editor::EditorPosition {
    editor::EditorPosition {
        line: value.line,
        column: value.column,
    }
}
fn wire_position(value: editor::EditorPosition) -> wire::EditorPosition {
    wire::EditorPosition {
        line: value.line,
        column: value.column,
    }
}
fn history_effect(value: EditorHistoryEffect) -> editor::EditorHistoryEffect {
    match value {
        EditorHistoryEffect::Native => editor::EditorHistoryEffect::Native,
        EditorHistoryEffect::NewGroup => editor::EditorHistoryEffect::NewGroup,
        EditorHistoryEffect::ExtendPrevious => editor::EditorHistoryEffect::ExtendPrevious,
        EditorHistoryEffect::Undo => editor::EditorHistoryEffect::Undo,
        EditorHistoryEffect::Redo => editor::EditorHistoryEffect::Redo,
    }
}
fn wire_history(value: editor::EditorHistoryEffect) -> EditorHistoryEffect {
    match value {
        editor::EditorHistoryEffect::Native => EditorHistoryEffect::Native,
        editor::EditorHistoryEffect::NewGroup => EditorHistoryEffect::NewGroup,
        editor::EditorHistoryEffect::ExtendPrevious => EditorHistoryEffect::ExtendPrevious,
        editor::EditorHistoryEffect::Undo => EditorHistoryEffect::Undo,
        editor::EditorHistoryEffect::Redo => EditorHistoryEffect::Redo,
    }
}

fn decide(request: EditorKeyRequest<'_>, history: &History) -> EditorDecision {
    let doc = document(request.state.text, request.state.cursor);
    let decision = match &request.key.key {
        Key::Character(key) if key.eq_ignore_ascii_case("z") => if request.key.modifiers.shift {
            history.redo(&doc)
        } else {
            history.undo(&doc)
        }
        .unwrap_or(editor::EditorDecision::Noop),
        Key::Named(key) => {
            let key = match key {
                Named::Enter => editor::Key::Enter,
                Named::Tab if request.key.modifiers.shift => editor::Key::ShiftTab,
                Named::Tab => editor::Key::Tab,
                Named::Backspace => editor::Key::Backspace,
                _ => return EditorDecision::DefaultEditorAction,
            };
            editor::decide(&doc, key, history, request.input_time_ms)
        }
        _ => return EditorDecision::DefaultEditorAction,
    };
    wire_decision(&doc, decision)
}

fn wire_decision(doc: &Doc, decision: editor::EditorDecision) -> EditorDecision {
    match decision {
        editor::EditorDecision::DefaultEditorAction => EditorDecision::DefaultEditorAction,
        editor::EditorDecision::Noop => EditorDecision::Noop,
        editor::EditorDecision::Apply {
            patches,
            cursor,
            history,
        } => {
            let after = editor::apply(doc, &patches, cursor);
            // Native replacement endpoints include entire graphemes and paired
            // newlines. A scalar-only diff of emoji history is not sufficient.
            let patches = wire::editor_document::editor_changed_span(&doc.text, &after.text)
                .unwrap_or_else(|_| {
                    patches
                        .into_iter()
                        .map(|p| wire::EditorPatch {
                            start_byte: p.start_byte,
                            end_byte: p.end_byte,
                            replacement: p.replacement,
                        })
                        .collect()
                }); // The host reports an oversized edit as a fault.
            EditorDecision::Apply {
                patches,
                cursor: wire::EditorCursor {
                    position: wire_position(cursor.position),
                    selection: cursor.selection.map(wire_position),
                },
                history: wire_history(history),
            }
        }
    }
}
