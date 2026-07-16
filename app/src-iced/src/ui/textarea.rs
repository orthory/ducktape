use super::theme::{Theme, alpha};
use iced::widget::text::IntoFragment;
use iced::widget::{text_editor, text_editor::Content};
use iced::{Background, Border, Element};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextareaVariant {
    #[default]
    Default,
    Invalid,
}

pub fn textarea<'a, Message>(
    content: &'a Content,
    placeholder: impl IntoFragment<'a>,
    on_action: impl Fn(text_editor::Action) -> Message + 'a,
    variant: TextareaVariant,
    theme: &Theme,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let theme = *theme;

    text_editor(content)
        .placeholder(placeholder)
        .on_action(on_action)
        .min_height(96)
        .padding([theme.spacing.sm, theme.spacing.md])
        .size(theme.typography.sm)
        .style(move |_iced_theme, status| style(&theme, variant, status))
        .into()
}

pub fn style(
    theme: &Theme,
    variant: TextareaVariant,
    status: text_editor::Status,
) -> text_editor::Style {
    let palette = theme.palette;
    let invalid = variant == TextareaVariant::Invalid;
    let mut border = if invalid {
        palette.destructive
    } else {
        palette.input
    };
    let mut background = palette.background;
    let mut value = palette.foreground;
    let mut placeholder = palette.muted_foreground;
    let mut width = 1.0;

    match status {
        text_editor::Status::Hovered if !invalid => border = palette.foreground,
        text_editor::Status::Focused { .. } => {
            border = if invalid {
                palette.destructive
            } else {
                palette.ring
            };
            width = 2.0;
        }
        text_editor::Status::Disabled => {
            background = palette.muted;
            value = alpha(value, 0.5);
            placeholder = alpha(placeholder, 0.5);
            border = alpha(border, 0.5);
        }
        text_editor::Status::Active | text_editor::Status::Hovered => {}
    }

    text_editor::Style {
        background: Background::Color(background),
        border: Border {
            color: border,
            width,
            radius: theme.radius.md.into(),
        },
        placeholder,
        value,
        selection: alpha(palette.primary, 0.25),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::LIGHT;

    const FOCUSED: text_editor::Status = text_editor::Status::Focused { is_hovered: false };

    #[test]
    fn focused_textarea_uses_ring() {
        let style = style(&LIGHT, TextareaVariant::Default, FOCUSED);

        assert_eq!(style.border.color, LIGHT.palette.ring);
        assert_eq!(style.border.width, 2.0);
    }

    #[test]
    fn focused_invalid_textarea_keeps_destructive_border() {
        let style = style(&LIGHT, TextareaVariant::Invalid, FOCUSED);

        assert_eq!(style.border.color, LIGHT.palette.destructive);
        assert_eq!(style.border.width, 2.0);
    }
}
