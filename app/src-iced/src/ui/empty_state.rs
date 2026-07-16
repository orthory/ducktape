use super::theme::Theme;
use iced::alignment::Horizontal;
use iced::widget::text::IntoFragment;
use iced::widget::{Column, Container, container, text};
use iced::{Alignment, Element, Length};

/// A centered empty state with required visible title and description text.
pub fn empty_state<'a, Message>(
    leading: Option<Element<'a, Message>>,
    title: impl IntoFragment<'a>,
    description: impl IntoFragment<'a>,
    theme: &Theme,
) -> Container<'a, Message>
where
    Message: 'a,
{
    let mut content = Column::new()
        .width(Length::Fill)
        .spacing(theme.spacing.sm)
        .align_x(Alignment::Center);

    if let Some(leading) = leading {
        content = content.push(leading);
    }

    content = content
        .push(
            text(title)
                .width(Length::Fill)
                .align_x(Horizontal::Center)
                .size(theme.typography.lg)
                .color(theme.palette.foreground),
        )
        .push(
            text(description)
                .width(Length::Fill)
                .align_x(Horizontal::Center)
                .size(theme.typography.sm)
                .color(theme.palette.muted_foreground),
        );

    container(content)
        .padding(theme.spacing.xxl)
        .width(Length::Fill)
        .center_x(Length::Fill)
}
