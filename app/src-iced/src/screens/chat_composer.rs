//! Multiline, IME-aware Chat composer presentation state.

use iced::keyboard;
use iced::widget::{button, column, container, row, text, text_editor};
use iced::{Alignment, Background, Border, Element};

use crate::theme::{self, BODY, LABEL, Palette, RADIUS_SM, SANS};

#[derive(Debug, Clone)]
pub struct State {
    content: text_editor::Content,
}

impl State {
    pub fn new() -> Self {
        Self {
            content: text_editor::Content::new(),
        }
    }

    pub fn text(&self) -> String {
        self.content.text()
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn is_blank(&self) -> bool {
        self.is_empty() || self.text().trim().is_empty()
    }

    pub fn clear(&mut self) {
        self.content = text_editor::Content::new();
    }

    pub fn append_reference(&mut self, reference: &str) {
        if reference.is_empty() {
            return;
        }
        let mut text = self.text();
        if !text.is_empty() && !text.chars().last().is_some_and(char::is_whitespace) {
            text.push(' ');
        }
        text.push_str(reference);
        self.set_text(&text);
    }

    fn set_text(&mut self, text: &str) {
        self.content = text_editor::Content::with_text(text);
        self.content
            .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&str> for State {
    fn from(text: &str) -> Self {
        let mut state = Self::new();
        state.set_text(text);
        state
    }
}

impl From<String> for State {
    fn from(text: String) -> Self {
        Self::from(text.as_str())
    }
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        self.text() == other.text()
    }
}

impl Eq for State {}

impl PartialEq<&str> for State {
    fn eq(&self, other: &&str) -> bool {
        self.text() == *other
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Edit(text_editor::Action),
    Submit,
    ChooseAttachment,
    TogglePagePicker,
    InsertPageRef { id: String, title: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Submit(String),
    ChooseAttachment,
    TogglePagePicker,
    ReferenceInserted,
}

pub fn update(state: &mut State, message: Message) -> Option<Output> {
    match message {
        Message::Edit(action) => {
            state.content.perform(action);
            None
        }
        Message::Submit => {
            let body = state.text().trim().to_owned();
            if body.is_empty() {
                return None;
            }
            state.clear();
            Some(Output::Submit(body))
        }
        Message::ChooseAttachment => Some(Output::ChooseAttachment),
        Message::TogglePagePicker => Some(Output::TogglePagePicker),
        Message::InsertPageRef { id, title } => {
            let label = title.replace([']', '\n', '*'], " ");
            state.append_reference(&format!("[{}](duck://page/{id})", label.trim()));
            Some(Output::ReferenceInserted)
        }
    }
}

pub fn view<'a>(
    state: &'a State,
    placeholder: String,
    submit_label: &'static str,
    attachment_busy: bool,
    page_picker_open: bool,
    pages: impl IntoIterator<Item = (&'a str, &'a str)>,
    p: Palette,
) -> Element<'a, Message> {
    let pages = pages.into_iter().take(24).collect::<Vec<_>>();
    let editor = text_editor(&state.content)
        .placeholder(placeholder)
        .on_action(Message::Edit)
        .key_binding(|press| {
            if press.modifiers.command()
                && matches!(
                    press.key.as_ref(),
                    keyboard::Key::Named(keyboard::key::Named::Enter)
                )
            {
                Some(text_editor::Binding::Custom(Message::Submit))
            } else {
                text_editor::Binding::from_key_press(press)
            }
        })
        .padding([8, 10])
        .size(BODY)
        .font(SANS)
        .min_height(38)
        .max_height(112)
        .style(move |_, status| text_editor::Style {
            background: Background::Color(p.sunken),
            border: Border {
                color: if matches!(status, text_editor::Status::Focused { .. }) {
                    theme::ACCENTS[0]
                } else {
                    p.border_strong
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            placeholder: p.muted_2,
            value: p.ink,
            selection: theme::ACCENTS[0],
        });
    #[cfg(all(feature = "agent", debug_assertions))]
    let editor = iced_agent_plugin::Sem::new(
        iced_agent_plugin::Role::TextInput,
        "Message composer",
        editor,
    )
    .value(state.text());
    let mut content = column![
        row![
            outline_enabled(
                if attachment_busy {
                    "Adding…"
                } else {
                    "+ File"
                },
                Message::ChooseAttachment,
                !attachment_busy,
                p,
            ),
            outline_enabled("¶ Page", Message::TogglePagePicker, !pages.is_empty(), p,),
            editor,
            outline_enabled(submit_label, Message::Submit, !state.is_blank(), p,)
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    ]
    .spacing(7);
    if page_picker_open {
        let mut picker = row![].spacing(5);
        for (id, title) in pages {
            picker = picker.push(outline_enabled(
                if title.trim().is_empty() {
                    "Untitled".to_string()
                } else {
                    title.to_string()
                },
                Message::InsertPageRef {
                    id: id.to_string(),
                    title: title.to_string(),
                },
                true,
                p,
            ));
        }
        content = content.push(picker.wrap());
    }
    container(content)
        .padding([12, 18])
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.paper)),
            border: Border {
                color: p.border_soft,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn outline_enabled<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let label = label.to_string();
    let button = button(text(label.clone()).font(SANS).size(LABEL))
        .padding([7, 10])
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(
                if enabled && matches!(status, iced::widget::button::Status::Hovered) {
                    p.hover
                } else {
                    p.paper
                },
            )),
            text_color: if enabled { p.ink_soft } else { p.muted_2 },
            border: Border {
                color: if enabled {
                    p.border_strong
                } else {
                    p.border_soft
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        });
    let button = if enabled {
        button.on_press(message)
    } else {
        button
    };
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn ime_commit_and_multiline_text_are_kept() {
        let mut state = State::new();
        update(
            &mut state,
            Message::Edit(text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new("안녕\nducks".into()),
            ))),
        );
        assert_eq!(state.text(), "안녕\nducks");
    }

    #[test]
    fn submit_trims_and_clears() {
        let mut state = State::from("  first\nsecond  ");
        assert_eq!(
            update(&mut state, Message::Submit),
            Some(Output::Submit("first\nsecond".into()))
        );
        assert!(state.is_empty());
    }

    #[test]
    fn references_append_without_collapsing_multiline_text() {
        let mut state = State::from("first\nsecond");
        update(
            &mut state,
            Message::InsertPageRef {
                id: "page-1".into(),
                title: "Plan".into(),
            },
        );
        assert_eq!(state.text(), "first\nsecond [Plan](duck://page/page-1)");
    }
}
