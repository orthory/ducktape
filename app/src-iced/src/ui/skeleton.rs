use super::theme::Theme;
use iced::widget::{Container, Space, container};
use iced::{Background, Border};

/// A static loading placeholder. It has no animation, so reduced-motion users
/// receive the same presentation. Chain native `width` and `height` methods.
pub fn skeleton<'a, Message>(theme: &Theme) -> Container<'a, Message>
where
    Message: 'a,
{
    let styled_theme = *theme;
    container(Space::new()).style(move |_iced_theme| style(&styled_theme))
}

pub fn style(theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(theme.palette.muted)),
        border: Border {
            radius: theme.radius.md.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::DARK;

    #[test]
    fn placeholder_uses_semantic_muted_surface() {
        let style = style(&DARK);
        assert_eq!(
            style.background,
            Some(Background::Color(DARK.palette.muted))
        );
        assert_eq!(style.border.radius, DARK.radius.md.into());
    }
}
