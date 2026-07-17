use super::theme::{Theme, mix};
use iced::widget::text::IntoFragment;
use iced::widget::{Container, Row, Space, Text, container, text};
use iced::{Alignment, Background, Border, Element};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BadgeVariant {
    #[default]
    Default,
    Secondary,
    Destructive,
    Success,
    Warning,
    Outline,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BadgeSize {
    Small,
    #[default]
    Default,
}

/// A compact label that can optionally include a redundant visual status marker.
pub struct Badge<'a> {
    label: Text<'a>,
    variant: BadgeVariant,
    size: BadgeSize,
    dot: bool,
    theme: Theme,
}

pub fn badge<'a>(label: impl IntoFragment<'a>, variant: BadgeVariant, theme: &Theme) -> Badge<'a> {
    Badge {
        label: text(label),
        variant,
        size: BadgeSize::Default,
        dot: false,
        theme: *theme,
    }
}

impl<'a> Badge<'a> {
    #[must_use]
    pub fn size(mut self, size: BadgeSize) -> Self {
        self.size = size;
        self
    }

    /// Adds a decorative tone marker. Keep a visible label so color is not the only signal.
    #[must_use]
    pub fn dot(mut self) -> Self {
        self.dot = true;
        self
    }

    pub fn into_widget<Message>(self) -> Container<'a, Message>
    where
        Message: 'a,
    {
        let metrics = metrics(self.size, &self.theme);
        let mut content = Row::new().spacing(metrics.gap).align_y(Alignment::Center);
        if self.dot {
            let marker = tone(&self.theme, self.variant);
            content = content.push(
                container(Space::new().width(metrics.dot).height(metrics.dot)).style(
                    move |_iced_theme| iced::widget::container::Style {
                        background: Some(Background::Color(marker)),
                        border: Border {
                            radius: 999.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
            );
        }
        content = content.push(self.label.size(metrics.text));

        let theme = self.theme;
        let variant = self.variant;
        let dot = self.dot;
        container(content)
            .padding([metrics.vertical, metrics.horizontal])
            .style(move |_iced_theme| {
                if dot {
                    style_with_dot(&theme, variant, true)
                } else {
                    style(&theme, variant)
                }
            })
    }
}

impl<'a, Message> From<Badge<'a>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(badge: Badge<'a>) -> Self {
        badge.into_widget().into()
    }
}

pub fn style(theme: &Theme, variant: BadgeVariant) -> iced::widget::container::Style {
    style_with_dot(theme, variant, false)
}

fn style_with_dot(
    theme: &Theme,
    variant: BadgeVariant,
    dot: bool,
) -> iced::widget::container::Style {
    let palette = theme.palette;
    let tinted = |tone| {
        (
            Some(mix(palette.background, tone, 0.09)),
            palette.foreground,
            mix(palette.background, tone, 0.25),
        )
    };
    let (background, foreground, border) = match (variant, dot) {
        (BadgeVariant::Destructive, true) => tinted(palette.destructive),
        (BadgeVariant::Default, _) => (
            Some(palette.primary),
            palette.primary_foreground,
            palette.primary,
        ),
        (BadgeVariant::Secondary, _) => (
            Some(palette.secondary),
            palette.secondary_foreground,
            palette.secondary,
        ),
        (BadgeVariant::Destructive, false) => (
            Some(palette.destructive),
            palette.destructive_foreground,
            palette.destructive,
        ),
        (BadgeVariant::Success, _) => tinted(palette.success),
        (BadgeVariant::Warning, _) => tinted(palette.warning),
        (BadgeVariant::Outline, _) => (None, palette.foreground, palette.border),
    };
    iced::widget::container::Style {
        background: background.map(Background::Color),
        text_color: Some(foreground),
        border: Border {
            color: border,
            width: 1.0,
            radius: 999.0.into(),
        },
        ..Default::default()
    }
}

fn tone(theme: &Theme, variant: BadgeVariant) -> iced::Color {
    match variant {
        BadgeVariant::Success => theme.palette.success,
        BadgeVariant::Warning => theme.palette.warning,
        BadgeVariant::Destructive => theme.palette.destructive,
        BadgeVariant::Default => theme.palette.primary_foreground,
        BadgeVariant::Secondary | BadgeVariant::Outline => theme.palette.foreground,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Metrics {
    vertical: f32,
    horizontal: f32,
    text: f32,
    dot: f32,
    gap: f32,
}

fn metrics(size: BadgeSize, theme: &Theme) -> Metrics {
    match size {
        BadgeSize::Small => Metrics {
            vertical: 1.0,
            horizontal: 6.0,
            text: theme.typography.xs,
            dot: 6.0,
            gap: 3.0,
        },
        BadgeSize::Default => Metrics {
            vertical: 2.0,
            horizontal: 8.0,
            text: theme.typography.sm,
            dot: 8.0,
            gap: 4.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{DARK, LIGHT};

    #[test]
    fn every_badge_label_meets_normal_text_contrast() {
        let variants = [
            BadgeVariant::Default,
            BadgeVariant::Secondary,
            BadgeVariant::Destructive,
            BadgeVariant::Success,
            BadgeVariant::Warning,
            BadgeVariant::Outline,
        ];
        for theme in [LIGHT, DARK] {
            for variant in variants {
                for dot in [false, true] {
                    let style = style_with_dot(&theme, variant, dot);
                    let foreground = style.text_color.unwrap();
                    let background = match style.background {
                        Some(Background::Color(color)) => color,
                        _ => theme.palette.background,
                    };
                    assert!(
                        contrast(foreground, background) >= 4.5,
                        "{} {variant:?} dot={dot}",
                        theme.name
                    );
                }
            }
        }
    }

    #[test]
    fn size_controls_label_and_marker_together() {
        assert_eq!(metrics(BadgeSize::Small, &LIGHT).dot, 6.0);
        assert_eq!(metrics(BadgeSize::Small, &LIGHT).text, LIGHT.typography.xs);
        assert_eq!(metrics(BadgeSize::Default, &LIGHT).dot, 8.0);
        assert_eq!(
            metrics(BadgeSize::Default, &LIGHT).text,
            LIGHT.typography.sm
        );
    }

    fn contrast(a: iced::Color, b: iced::Color) -> f32 {
        let (lighter, darker) = if luminance(a) > luminance(b) {
            (luminance(a), luminance(b))
        } else {
            (luminance(b), luminance(a))
        };
        (lighter + 0.05) / (darker + 0.05)
    }

    fn luminance(color: iced::Color) -> f32 {
        fn channel(value: f32) -> f32 {
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
    }
}
