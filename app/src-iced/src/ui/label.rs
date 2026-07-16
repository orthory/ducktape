use super::theme::Theme;
use iced::font::Weight;
use iced::widget::text::IntoFragment;
use iced::widget::{Text, text};
use iced::{Color, Font};

/// Visible label text for a nearby native control.
pub fn label<'a>(content: impl IntoFragment<'a>, theme: &Theme) -> Text<'a> {
    let style = label_style(theme);
    text(content)
        .size(style.size)
        .font(style.font)
        .color(style.color)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LabelStyle {
    size: f32,
    color: Color,
    font: Font,
}

fn label_style(theme: &Theme) -> LabelStyle {
    LabelStyle {
        size: theme.typography.sm,
        color: theme.palette.foreground,
        font: Font {
            weight: Weight::Medium,
            ..Font::DEFAULT
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::LIGHT;

    #[test]
    fn label_uses_semantic_text_tokens() {
        let style = label_style(&LIGHT);
        assert_eq!(style.size, LIGHT.typography.sm);
        assert_eq!(style.color, LIGHT.palette.foreground);
        assert_eq!(style.font.weight, Weight::Medium);
    }
}
