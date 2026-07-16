use super::theme::Theme;
use iced::widget::text::IntoFragment;
use iced::widget::{Container, container, text};
use iced::{Background, Border, Element};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AvatarSize {
    Small,
    #[default]
    Default,
    Large,
}

/// A circular frame for caller-owned content, including images when enabled by the app.
pub fn avatar<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    size: AvatarSize,
    theme: &Theme,
) -> Container<'a, Message>
where
    Message: 'a,
{
    let diameter = metrics(size, theme).diameter;
    let theme = *theme;

    container(content)
        .center(diameter)
        .clip(true)
        .style(move |_iced_theme| style(&theme))
}

/// Text fallback for an avatar. Use a short visible name or initials.
pub fn avatar_fallback<'a, Message>(
    label: impl IntoFragment<'a>,
    size: AvatarSize,
    theme: &Theme,
) -> Container<'a, Message>
where
    Message: 'a,
{
    let metrics = metrics(size, theme);
    avatar(
        text(label)
            .size(metrics.text)
            .color(theme.palette.foreground),
        size,
        theme,
    )
}

pub fn style(theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(theme.palette.muted)),
        text_color: Some(theme.palette.foreground),
        border: Border {
            color: theme.palette.border,
            width: 1.0,
            radius: 999.0.into(),
        },
        ..Default::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Metrics {
    diameter: f32,
    text: f32,
}

fn metrics(size: AvatarSize, theme: &Theme) -> Metrics {
    match size {
        AvatarSize::Small => Metrics {
            diameter: 32.0,
            text: theme.typography.xs,
        },
        AvatarSize::Default => Metrics {
            diameter: 40.0,
            text: theme.typography.sm,
        },
        AvatarSize::Large => Metrics {
            diameter: 48.0,
            text: theme.typography.base,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::LIGHT;

    #[test]
    fn avatar_sizes_scale_frame_and_fallback_together() {
        let small = metrics(AvatarSize::Small, &LIGHT);
        let default = metrics(AvatarSize::Default, &LIGHT);
        let large = metrics(AvatarSize::Large, &LIGHT);

        assert!(small.diameter < default.diameter && default.diameter < large.diameter);
        assert!(small.text < default.text && default.text < large.text);
    }
}
