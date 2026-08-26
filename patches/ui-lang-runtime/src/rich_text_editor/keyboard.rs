use super::movement::{move_cursor, uses_rich_geometry};
use super::{Action, document::DocumentLayout};
use iced::advanced::{Clipboard, Shell};
use iced::keyboard::{self, key};
use iced::widget::text_editor::{self, Binding, Content, Edit, Motion};
use std::sync::Arc;

pub(super) struct BindingContext<'a> {
    document: &'a DocumentLayout,
    preferred_x: &'a mut Option<f32>,
    viewport_height: f32,
}

impl<'a> BindingContext<'a> {
    pub(super) fn new(
        document: &'a DocumentLayout,
        preferred_x: &'a mut Option<f32>,
        viewport_height: f32,
    ) -> Self {
        Self {
            document,
            preferred_x,
            viewport_height,
        }
    }
}

pub(super) fn rich_binding(press: &text_editor::KeyPress) -> Option<Binding<Edit>> {
    match press.modified_key.as_ref() {
        keyboard::Key::Named(key::Named::Tab) if press.modifiers.shift() => {
            Some(Binding::Custom(Edit::Unindent))
        }
        keyboard::Key::Named(key::Named::Tab) => Some(Binding::Custom(Edit::Indent)),
        keyboard::Key::Named(key::Named::Backspace) if press.modifiers.jump() => {
            Some(Binding::Sequence(vec![
                Binding::Select(Motion::WordLeft),
                Binding::Backspace,
            ]))
        }
        keyboard::Key::Named(key::Named::Backspace) if press.modifiers.macos_command() => {
            Some(Binding::Sequence(vec![
                Binding::Select(Motion::Home),
                Binding::Backspace,
            ]))
        }
        keyboard::Key::Named(key::Named::Delete)
            if press.modifiers.jump()
                && (press.text.is_none() || press.text.as_deref() == Some("\u{7f}")) =>
        {
            Some(Binding::Sequence(vec![
                Binding::Select(Motion::WordRight),
                Binding::Delete,
            ]))
        }
        keyboard::Key::Named(key::Named::Delete)
            if press.modifiers.macos_command()
                && (press.text.is_none() || press.text.as_deref() == Some("\u{7f}")) =>
        {
            Some(Binding::Sequence(vec![
                Binding::Select(Motion::End),
                Binding::Delete,
            ]))
        }
        keyboard::Key::Named(named @ (key::Named::ArrowUp | key::Named::ArrowDown)) => {
            document_edge_binding(
                named,
                press.modifiers.macos_command(),
                press.modifiers.shift(),
            )
        }
        _ => None,
    }
}

/// macOS moves the caret to the document edges on Cmd+Up/Down; iced's stock
/// binding only remaps Cmd+Left/Right to Home/End.
pub(super) fn document_edge_binding(
    key: key::Named,
    command: bool,
    shift: bool,
) -> Option<Binding<Edit>> {
    if !command {
        return None;
    }
    let motion = match key {
        key::Named::ArrowUp => Motion::DocumentStart,
        key::Named::ArrowDown => Motion::DocumentEnd,
        _ => return None,
    };
    Some(if shift {
        Binding::Select(motion)
    } else {
        Binding::Move(motion)
    })
}

/// The editor's stock key handling: application command shortcuts bubble,
/// rich-document bindings (indent, word/line deletion) apply, and everything
/// else falls through to [`Binding::from_key_press`]. A
/// [`RichTextEditor::key_binding`](super::RichTextEditor::key_binding)
/// override delegates here for every press it does not decide itself.
pub fn default_key_binding(press: &text_editor::KeyPress) -> Option<Binding<Edit>> {
    if command_shortcut_bubbles(press) {
        return None;
    }

    rich_binding(press).or_else(|| Binding::<Edit>::from_key_press(press.clone()))
}

fn command_shortcut_bubbles(press: &text_editor::KeyPress) -> bool {
    if !press.modifiers.command() {
        return false;
    }

    match press.key.to_latin(press.physical_key) {
        Some('a' | 'c' | 'x') => false,
        Some('v') => press.modifiers.alt(),
        Some(_) => true,
        None => false,
    }
}

#[derive(Debug, Default)]
pub(super) struct PendingImeCommit {
    content: Option<String>,
}

impl PendingImeCommit {
    pub(super) fn clear(&mut self) {
        self.content = None;
    }

    pub(super) fn is_pending(&self) -> bool {
        self.content.is_some()
    }

    pub(super) fn on_preedit(&mut self, content: &str) {
        // The built-in macOS Korean IME emits an additional empty preedit
        // after Commit. It is still part of the same key event, so only a new
        // non-empty composition supersedes the pending boundary.
        if !content.is_empty() {
            self.clear();
        }
    }

    pub(super) fn on_commit(&mut self, content: &str) {
        self.content = Some(content.to_owned());
    }

    pub(super) fn resolve(&mut self, character: Option<char>) -> ImeBoundary {
        let Some(character) = character else {
            return ImeBoundary::Unrelated;
        };
        let Some(committed) = self.content.take() else {
            return ImeBoundary::Unrelated;
        };

        if committed.ends_with(character) {
            ImeBoundary::Duplicate
        } else {
            ImeBoundary::Missing(character)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImeBoundary {
    Missing(char),
    Duplicate,
    Unrelated,
}

pub(super) fn single_printable_ascii(text: &str) -> Option<char> {
    let mut characters = text.chars();
    let character = characters.next()?;
    (characters.next().is_none() && character.is_ascii() && !character.is_ascii_control())
        .then_some(character)
}

fn logical_ascii_character(key: &keyboard::Key) -> Option<char> {
    match key.as_ref() {
        keyboard::Key::Character(text) => single_printable_ascii(text),
        keyboard::Key::Named(key::Named::Space) => Some(' '),
        _ => None,
    }
}

fn physical_ime_boundary_fallback(code: key::Code, shift: bool) -> Option<char> {
    Some(match (code, shift) {
        (key::Code::Comma, false) => ',',
        (key::Code::Comma, true) => '<',
        (key::Code::Period, false) => '.',
        (key::Code::Period, true) => '>',
        (key::Code::Space, _) => ' ',
        _ => return None,
    })
}

pub(super) fn ime_boundary_character(
    key: &keyboard::Key,
    modified_key: &keyboard::Key,
    physical_key: key::Physical,
    modifiers: keyboard::Modifiers,
) -> Option<char> {
    if modifiers.control() || modifiers.alt() || modifiers.logo() {
        return None;
    }

    logical_ascii_character(modified_key)
        .or_else(|| {
            if modifiers.shift() {
                None
            } else {
                logical_ascii_character(key)
            }
        })
        .or_else(|| {
            let key::Physical::Code(code) = physical_key else {
                return None;
            };
            physical_ime_boundary_fallback(code, modifiers.shift())
        })
}

pub(super) fn apply_binding<Message>(
    binding: Binding<Edit>,
    content: &Content,
    context: &mut BindingContext<'_>,
    on_action: &dyn Fn(Action) -> Message,
    clipboard: &mut dyn Clipboard,
    shell: &mut Shell<'_, Message>,
) -> bool {
    let publish = |shell: &mut Shell<'_, Message>, action| {
        shell.publish(on_action(action));
    };

    match binding {
        Binding::Unfocus => {
            return true;
        }
        Binding::Copy => {
            if let Some(selection) = content.selection() {
                clipboard.write(iced::advanced::clipboard::Kind::Standard, selection);
            }
        }
        Binding::Cut => {
            if let Some(selection) = content.selection() {
                clipboard.write(iced::advanced::clipboard::Kind::Standard, selection);
                publish(shell, Action::Edit(text_editor::Action::Edit(Edit::Delete)));
            }
            *context.preferred_x = None;
        }
        Binding::Paste => {
            if let Some(source) = clipboard.read(iced::advanced::clipboard::Kind::Standard) {
                publish(
                    shell,
                    Action::Edit(text_editor::Action::Edit(Edit::Paste(Arc::new(source)))),
                );
            }
            *context.preferred_x = None;
        }
        Binding::Move(motion) => {
            if uses_rich_geometry(motion) {
                let cursor = move_cursor(
                    context.document,
                    context.preferred_x,
                    context.viewport_height,
                    content.cursor(),
                    motion,
                    false,
                );
                publish(shell, Action::MoveTo(cursor));
            } else {
                publish(shell, Action::Edit(text_editor::Action::Move(motion)));
                *context.preferred_x = None;
            }
        }
        Binding::Select(motion) => {
            if uses_rich_geometry(motion) {
                let cursor = move_cursor(
                    context.document,
                    context.preferred_x,
                    context.viewport_height,
                    content.cursor(),
                    motion,
                    true,
                );
                publish(shell, Action::MoveTo(cursor));
            } else {
                publish(shell, Action::Edit(text_editor::Action::Select(motion)));
                *context.preferred_x = None;
            }
        }
        Binding::SelectWord => {
            publish(shell, Action::Edit(text_editor::Action::SelectWord));
            *context.preferred_x = None;
        }
        Binding::SelectLine => {
            publish(shell, Action::Edit(text_editor::Action::SelectLine));
            *context.preferred_x = None;
        }
        Binding::SelectAll => {
            publish(shell, Action::Edit(text_editor::Action::SelectAll));
            *context.preferred_x = None;
        }
        Binding::Insert(character) => {
            publish(
                shell,
                Action::Edit(text_editor::Action::Edit(Edit::Insert(character))),
            );
            *context.preferred_x = None;
        }
        Binding::Enter => {
            publish(shell, Action::Edit(text_editor::Action::Edit(Edit::Enter)));
            *context.preferred_x = None;
        }
        Binding::Backspace => {
            publish(
                shell,
                Action::Edit(text_editor::Action::Edit(Edit::Backspace)),
            );
            *context.preferred_x = None;
        }
        Binding::Delete => {
            publish(shell, Action::Edit(text_editor::Action::Edit(Edit::Delete)));
            *context.preferred_x = None;
        }
        Binding::Sequence(bindings) => {
            let mut unfocus = false;
            for binding in bindings {
                unfocus |= apply_binding(binding, content, context, on_action, clipboard, shell);
            }
            return unfocus;
        }
        Binding::Custom(edit) => {
            publish(shell, Action::Edit(text_editor::Action::Edit(edit)));
            *context.preferred_x = None;
        }
    }

    false
}
