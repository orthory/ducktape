use super::theme::{Theme, alpha, mix};
use iced::widget::text::IntoFragment;
use iced::widget::{Checkbox, checkbox as iced_checkbox};
use iced::{Background, Border};

/// A styled native checkbox. Chain `on_toggle` to make it interactive.
pub fn checkbox<'a, Message>(
    label: impl IntoFragment<'a>,
    checked: bool,
    theme: &Theme,
) -> Checkbox<'a, Message>
where
    Message: 'a,
{
    let theme = *theme;
    iced_checkbox(checked)
        .label(label)
        .size(16)
        .spacing(theme.spacing.sm)
        .text_size(theme.typography.sm)
        .style(move |_iced_theme, status| style(&theme, status))
}

pub fn style(theme: &Theme, status: iced_checkbox::Status) -> iced_checkbox::Style {
    let palette = theme.palette;
    let (is_checked, hovered, disabled) = match status {
        iced_checkbox::Status::Active { is_checked } => (is_checked, false, false),
        iced_checkbox::Status::Hovered { is_checked } => (is_checked, true, false),
        iced_checkbox::Status::Disabled { is_checked } => (is_checked, false, true),
    };

    let (background, border, icon, text) = if disabled {
        (
            if is_checked {
                alpha(palette.primary, 0.5)
            } else {
                palette.muted
            },
            alpha(
                if is_checked {
                    palette.primary
                } else {
                    palette.input
                },
                0.5,
            ),
            alpha(palette.primary_foreground, 0.5),
            alpha(palette.foreground, 0.5),
        )
    } else if is_checked {
        let fill = if hovered {
            mix(palette.primary, palette.primary_foreground, 0.08)
        } else {
            palette.primary
        };
        (fill, fill, palette.primary_foreground, palette.foreground)
    } else {
        (
            if hovered {
                palette.accent
            } else {
                palette.background
            },
            if hovered {
                palette.foreground
            } else {
                palette.input
            },
            palette.primary_foreground,
            palette.foreground,
        )
    };

    iced_checkbox::Style {
        background: Background::Color(background),
        icon_color: icon,
        border: Border {
            color: border,
            width: 1.0,
            radius: theme.radius.sm.into(),
        },
        text_color: Some(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::LIGHT;

    #[test]
    fn state_styles_preserve_semantic_feedback() {
        let checked = style(&LIGHT, iced_checkbox::Status::Active { is_checked: true });
        assert_eq!(checked.background, Background::Color(LIGHT.palette.primary));
        assert_eq!(checked.icon_color, LIGHT.palette.primary_foreground);

        let active = style(&LIGHT, iced_checkbox::Status::Active { is_checked: false });
        let hovered = style(&LIGHT, iced_checkbox::Status::Hovered { is_checked: false });
        assert_ne!(hovered.background, active.background);
        assert_ne!(hovered.border.color, active.border.color);

        let disabled = style(&LIGHT, iced_checkbox::Status::Disabled { is_checked: true });
        assert!(disabled.icon_color.a < checked.icon_color.a);
        assert!(disabled.text_color.unwrap().a < checked.text_color.unwrap().a);
    }
}
