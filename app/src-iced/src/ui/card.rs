use super::surface::{SurfaceVariant, surface};
use super::theme::Theme;
use iced::Element;
use iced::widget::text::IntoFragment;
use iced::widget::{Column, Container, column, text};

pub fn card<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    theme: &Theme,
) -> Container<'a, Message>
where
    Message: 'a,
{
    surface(content, SurfaceVariant::Card, theme).padding(theme.spacing.xl)
}

pub fn card_header<'a, Message>(
    title: impl IntoFragment<'a>,
    description: impl IntoFragment<'a>,
    theme: &Theme,
) -> Column<'a, Message>
where
    Message: 'a,
{
    column![
        text(title)
            .size(theme.typography.lg)
            .color(theme.palette.card_foreground),
        text(description)
            .size(theme.typography.sm)
            .color(theme.palette.muted_foreground),
    ]
    .spacing(theme.spacing.xs)
}
