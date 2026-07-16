use super::theme::{Theme, alpha, mix};
use iced::alignment::{Horizontal, Vertical};
use iced::widget::text::IntoFragment;
use iced::widget::{Button as IcedButton, button as iced_button, container, text};
use iced::{Background, Border, Color, Element, Length};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Default,
    Destructive,
    Outline,
    Secondary,
    Ghost,
    Link,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    #[default]
    Default,
    Large,
    Icon,
}

/// A thin builder that becomes a native iced button.
pub struct Button<'a, Message>
where
    Message: Clone + 'a,
{
    content: Element<'a, Message>,
    on_press: Option<Message>,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
    width: Length,
    alignment: Horizontal,
    theme: Theme,
}

pub fn button<'a, Message>(label: impl IntoFragment<'a>, theme: &Theme) -> Button<'a, Message>
where
    Message: Clone + 'a,
{
    Button::new(text(label).size(theme.typography.sm), theme)
}

impl<'a, Message> Button<'a, Message>
where
    Message: Clone + 'a,
{
    pub fn new(content: impl Into<Element<'a, Message>>, theme: &Theme) -> Self {
        Self {
            content: content.into(),
            on_press: None,
            variant: ButtonVariant::Default,
            size: ButtonSize::Default,
            disabled: false,
            width: Length::Shrink,
            alignment: Horizontal::Center,
            theme: *theme,
        }
    }

    #[must_use]
    pub fn on_press(mut self, message: Message) -> Self {
        self.on_press = Some(message);
        self
    }

    #[must_use]
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    #[must_use]
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets horizontal content alignment when the button is wider than its label.
    #[must_use]
    pub fn align_x(mut self, alignment: Horizontal) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn into_widget(self) -> IcedButton<'a, Message> {
        let (vertical, horizontal, height) = match self.size {
            ButtonSize::Small => (6.0, 12.0, 32.0),
            ButtonSize::Default => (8.0, 16.0, 36.0),
            ButtonSize::Large => (10.0, 24.0, 40.0),
            ButtonSize::Icon => (8.0, 8.0, 36.0),
        };
        let width = if self.size == ButtonSize::Icon {
            Length::Fixed(height)
        } else {
            self.width
        };
        let content_width = if width == Length::Shrink {
            Length::Shrink
        } else {
            Length::Fill
        };
        let content = container(self.content)
            .width(content_width)
            .height(Length::Fill)
            .align_x(self.alignment)
            .align_y(Vertical::Center);
        let theme = self.theme;
        let variant = self.variant;
        iced_button(content)
            .padding([vertical, horizontal])
            .width(width)
            .height(height)
            .on_press_maybe((!self.disabled).then_some(self.on_press).flatten())
            .style(move |_iced_theme, status| style(&theme, variant, status))
    }
}

impl<'a, Message> From<Button<'a, Message>> for Element<'a, Message>
where
    Message: Clone + 'a,
{
    fn from(button: Button<'a, Message>) -> Self {
        button.into_widget().into()
    }
}

pub fn style(
    theme: &Theme,
    variant: ButtonVariant,
    status: iced_button::Status,
) -> iced_button::Style {
    let palette = theme.palette;
    let (mut background, mut foreground, border_color, border_width) = match variant {
        ButtonVariant::Default => (
            Some(palette.primary),
            palette.primary_foreground,
            palette.primary,
            0.0,
        ),
        ButtonVariant::Destructive => (
            Some(palette.destructive),
            palette.destructive_foreground,
            palette.destructive,
            0.0,
        ),
        ButtonVariant::Secondary => (
            Some(palette.secondary),
            palette.secondary_foreground,
            palette.secondary,
            0.0,
        ),
        ButtonVariant::Outline => (None, palette.foreground, palette.input, 1.0),
        ButtonVariant::Ghost => (None, palette.foreground, Color::TRANSPARENT, 0.0),
        ButtonVariant::Link => (None, palette.primary, Color::TRANSPARENT, 0.0),
    };

    match status {
        iced_button::Status::Hovered => match variant {
            ButtonVariant::Outline | ButtonVariant::Ghost => {
                background = Some(palette.accent);
                foreground = palette.accent_foreground;
            }
            ButtonVariant::Link => foreground = mix(foreground, palette.foreground, 0.25),
            _ => background = background.map(|color| mix(color, foreground, 0.08)),
        },
        iced_button::Status::Pressed => match variant {
            ButtonVariant::Outline | ButtonVariant::Ghost => {
                background = Some(mix(palette.accent, palette.foreground, 0.08));
                foreground = palette.accent_foreground;
            }
            ButtonVariant::Link => foreground = mix(foreground, palette.foreground, 0.40),
            _ => background = background.map(|color| mix(color, foreground, 0.16)),
        },
        iced_button::Status::Disabled => {
            background = background.map(|color| alpha(color, 0.5));
            foreground = alpha(foreground, 0.5);
        }
        iced_button::Status::Active => {}
    }

    iced_button::Style {
        background: background.map(Background::Color),
        text_color: foreground,
        border: Border {
            color: if status == iced_button::Status::Disabled {
                alpha(border_color, 0.5)
            } else {
                border_color
            },
            width: border_width,
            radius: theme.radius.md.into(),
        },
        ..iced_button::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::LIGHT;

    #[test]
    fn disabled_button_reduces_foreground_opacity() {
        let active = style(&LIGHT, ButtonVariant::Default, iced_button::Status::Active);
        let disabled = style(
            &LIGHT,
            ButtonVariant::Default,
            iced_button::Status::Disabled,
        );
        assert!(disabled.text_color.a < active.text_color.a);
    }

    #[test]
    fn content_alignment_is_explicit_and_configurable() {
        let centered: Button<'_, ()> = button("Centered", &LIGHT);
        assert_eq!(centered.alignment, Horizontal::Center);

        let leading: Button<'_, ()> = button("Leading", &LIGHT).align_x(Horizontal::Left);
        assert_eq!(leading.alignment, Horizontal::Left);
    }
}
