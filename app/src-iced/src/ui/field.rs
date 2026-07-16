use super::theme::Theme;
use iced::Element;
use iced::widget::text::IntoFragment;
use iced::widget::{Column, column, text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldHint<'a> {
    Description(&'a str),
    Error(&'a str),
}

/// Adds a persistent visible label and optional help or error text around a native control.
pub fn field<'a, Message>(
    label: impl IntoFragment<'a>,
    control: impl Into<Element<'a, Message>>,
    hint: Option<FieldHint<'a>>,
    theme: &Theme,
) -> Column<'a, Message>
where
    Message: 'a,
{
    let mut content = column![
        text(label)
            .size(theme.typography.sm)
            .color(theme.palette.foreground),
        control.into(),
    ]
    .spacing(theme.spacing.xs);

    if let Some(hint) = hint {
        let (copy, color) = hint_style(hint, theme);
        content = content.push(text(copy).size(theme.typography.xs).color(color));
    }

    content
}

fn hint_style<'a>(hint: FieldHint<'a>, theme: &Theme) -> (&'a str, iced::Color) {
    match hint {
        FieldHint::Description(copy) => (copy, theme.palette.muted_foreground),
        FieldHint::Error(copy) => (copy, theme.palette.destructive),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::LIGHT;

    #[test]
    fn hint_kind_selects_semantic_text_color() {
        assert_eq!(
            hint_style(FieldHint::Description("help"), &LIGHT),
            ("help", LIGHT.palette.muted_foreground)
        );
        assert_eq!(
            hint_style(FieldHint::Error("error"), &LIGHT),
            ("error", LIGHT.palette.destructive)
        );
    }
}
