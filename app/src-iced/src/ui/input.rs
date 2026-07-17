use super::theme::{Theme, alpha};
use iced::widget::{TextInput, text_input};
use iced::{Background, Border};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InputVariant {
    #[default]
    Default,
    Invalid,
}

pub fn input<'a, Message>(placeholder: &str, value: &str, theme: &Theme) -> TextInput<'a, Message>
where
    Message: Clone + 'a,
{
    input_with_variant(placeholder, value, InputVariant::Default, theme)
}

pub fn input_with_variant<'a, Message>(
    placeholder: &str,
    value: &str,
    variant: InputVariant,
    theme: &Theme,
) -> TextInput<'a, Message>
where
    Message: Clone + 'a,
{
    let theme = *theme;
    text_input(placeholder, value)
        .padding([8, 12])
        .size(theme.typography.sm)
        .style(move |_iced_theme, status| style(&theme, variant, status))
}

pub fn style(
    theme: &Theme,
    variant: InputVariant,
    status: text_input::Status,
) -> text_input::Style {
    let palette = theme.palette;
    let invalid = variant == InputVariant::Invalid;
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
        text_input::Status::Hovered if !invalid => border = palette.foreground,
        text_input::Status::Focused { .. } => {
            border = if invalid {
                palette.destructive
            } else {
                palette.ring
            };
            width = 2.0;
        }
        text_input::Status::Disabled => {
            background = palette.muted;
            value = alpha(value, 0.5);
            placeholder = alpha(placeholder, 0.5);
            border = alpha(border, 0.5);
        }
        text_input::Status::Active | text_input::Status::Hovered => {}
    }

    text_input::Style {
        background: Background::Color(background),
        border: Border {
            color: border,
            width,
            radius: theme.radius.md.into(),
        },
        icon: palette.muted_foreground,
        placeholder,
        value,
        selection: alpha(palette.primary, 0.25),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::LIGHT;

    #[test]
    fn invalid_input_uses_destructive_border() {
        let style = style(&LIGHT, InputVariant::Invalid, text_input::Status::Active);
        assert_eq!(style.border.color, LIGHT.palette.destructive);
    }
}
