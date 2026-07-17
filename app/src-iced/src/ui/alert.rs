use super::theme::{Theme, mix};
use iced::widget::{Container, container};
use iced::{Background, Border, Element, Length};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AlertVariant {
    #[default]
    Default,
    Success,
    Warning,
    Destructive,
}

/// A compositional notice surface. Keep the message visible in `content`.
pub fn alert<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    variant: AlertVariant,
    theme: &Theme,
) -> Container<'a, Message>
where
    Message: 'a,
{
    let theme = *theme;
    container(content)
        .padding(theme.spacing.lg)
        .width(Length::Fill)
        .style(move |_iced_theme| style(&theme, variant))
}

pub fn style(theme: &Theme, variant: AlertVariant) -> iced::widget::container::Style {
    let tone = tone(theme, variant);
    iced::widget::container::Style {
        background: Some(Background::Color(mix(theme.palette.background, tone, 0.09))),
        text_color: Some(theme.palette.foreground),
        border: Border {
            color: mix(theme.palette.background, tone, 0.25),
            width: 1.0,
            radius: theme.radius.lg.into(),
        },
        ..Default::default()
    }
}

fn tone(theme: &Theme, variant: AlertVariant) -> iced::Color {
    match variant {
        AlertVariant::Default => theme.palette.primary,
        AlertVariant::Success => theme.palette.success,
        AlertVariant::Warning => theme.palette.warning,
        AlertVariant::Destructive => theme.palette.destructive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{DARK, LIGHT};

    #[test]
    fn variants_use_opaque_semantic_tints_and_normal_foreground() {
        let variants = [
            AlertVariant::Default,
            AlertVariant::Success,
            AlertVariant::Warning,
            AlertVariant::Destructive,
        ];

        for theme in [LIGHT, DARK] {
            for variant in variants {
                let appearance = style(&theme, variant);
                let Some(Background::Color(background)) = appearance.background else {
                    panic!("alert tint must be a solid color");
                };
                assert_eq!(background.a, 1.0);
                assert_eq!(appearance.text_color, Some(theme.palette.foreground));
                assert_eq!(
                    background,
                    mix(theme.palette.background, tone(&theme, variant), 0.09)
                );
            }
        }
    }
}
