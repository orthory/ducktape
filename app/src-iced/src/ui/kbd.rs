use super::theme::Theme;
use iced::font::Weight;
use iced::widget::text::IntoFragment;
use iced::widget::{Container, container, text};
use iced::{Background, Border, Font};

/// A visual key cap. Use visible surrounding text to explain the shortcut.
pub fn kbd<'a, Message>(key: impl IntoFragment<'a>, theme: &Theme) -> Container<'a, Message>
where
    Message: 'a,
{
    let font = Font {
        weight: Weight::Medium,
        ..Font::MONOSPACE
    };
    let styled_theme = *theme;

    container(
        text(key)
            .size(theme.typography.xs)
            .font(font)
            .color(theme.palette.muted_foreground),
    )
    .padding([theme.spacing.xs / 2.0, theme.spacing.xs])
    .style(move |_iced_theme| style(&styled_theme))
}

pub fn style(theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(theme.palette.muted)),
        text_color: Some(theme.palette.muted_foreground),
        border: Border {
            color: theme.palette.border,
            width: 1.0,
            radius: theme.radius.sm.into(),
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::LIGHT;

    #[test]
    fn key_cap_uses_semantic_surface_tokens() {
        let style = style(&LIGHT);
        assert_eq!(
            style.background,
            Some(Background::Color(LIGHT.palette.muted))
        );
        assert_eq!(style.text_color, Some(LIGHT.palette.muted_foreground));
        assert_eq!(style.border.color, LIGHT.palette.border);
        assert_eq!(style.border.radius, LIGHT.radius.sm.into());
    }
}
