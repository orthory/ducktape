//! Pages' structural keys and commit-confirmed history on the guest editor lane.
//! Rich document painting, gutter actions and anchored menus are independent
//! consumers; this adapter does not replace them with a plain editor.

use crate::editor::{self, Doc, History};
use std::{cell::RefCell, rc::Rc};
use ui_lang_guest::wire::{self, EditorDecision, EditorHistoryEffect, EditorKeyClaim};
use ui_lang_guest::{EditorBinding, EditorKeyRequest, EditorTransactionEvent};
use wire::keyboard::{Key, Modifiers, Named};

/// Ordinary Ice data: retained in the app state and therefore in snapshots.
#[derive(Clone, Debug, Default, PartialEq)]
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

/// Claims only structural keys and the platform command's undo/redo keys.
/// All other input stays native and joins this history only after commit.
pub fn keys(state: HistoryState) -> EditorBinding<HistoryState> {
    let stored = if state.snapshot.is_empty() {
        StoredHistory::default()
    } else {
        wire::decode(&state.snapshot).expect("Pages history snapshot")
    };
    let history = Rc::new(RefCell::new(stored));
    let deciding = history.clone();
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
            if state.reset == Some(request.state.reset) {
                decide(request, &state.history)
            } else {
                decide(request, &History::default())
            }
        },
        move |event| {
            let EditorTransactionEvent::Commit {
                before,
                after,
                history: effect,
                input_time_ms,
                ..
            } = event
            else {
                // A rejected proposal never moves either history stack.
                return None;
            };
            let mut state = history.borrow_mut();
            let replaced = state.reset != Some(before.reset);
            state.at_reset(before.reset);
            if before.text_revision == after.text_revision
                && !matches!(
                    effect,
                    EditorHistoryEffect::Undo | EditorHistoryEffect::Redo
                )
            {
                return replaced.then(|| state.save());
            }
            state.history.commit(
                &document(before.text, before.cursor),
                &document(after.text, after.cursor),
                history_effect(effect),
                input_time_ms,
            );
            Some(state.save())
        },
    )
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
    match decision {
        editor::EditorDecision::DefaultEditorAction => EditorDecision::DefaultEditorAction,
        editor::EditorDecision::Noop => EditorDecision::Noop,
        editor::EditorDecision::Apply {
            patches,
            cursor,
            history,
        } => {
            let after = editor::apply(&doc, &patches, cursor);
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
