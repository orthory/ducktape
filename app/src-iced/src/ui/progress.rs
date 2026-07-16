use super::theme::{Theme, mix};
use iced::widget::{ProgressBar, progress_bar};
use iced::{Background, Border};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProgressVariant {
    #[default]
    Default,
    Success,
    Warning,
    Destructive,
}

/// Pair the bar with adjacent visible status text, such as `42%` or `Complete`.
pub fn progress(percent: f32, variant: ProgressVariant, theme: &Theme) -> ProgressBar<'static> {
    let theme = *theme;
    progress_bar(0.0..=100.0, normalized(percent))
        .girth(5)
        .style(move |_iced_theme| style(&theme, variant))
}

pub fn style(theme: &Theme, variant: ProgressVariant) -> iced::widget::progress_bar::Style {
    iced::widget::progress_bar::Style {
        background: Background::Color(mix(
            theme.palette.background,
            theme.palette.foreground,
            0.12,
        )),
        bar: Background::Color(tone(theme, variant)),
        border: Border {
            radius: 999.0.into(),
            ..Default::default()
        },
    }
}

fn tone(theme: &Theme, variant: ProgressVariant) -> iced::Color {
    match variant {
        ProgressVariant::Default => theme.palette.primary,
        ProgressVariant::Success => theme.palette.success,
        ProgressVariant::Warning => theme.palette.warning,
        ProgressVariant::Destructive => theme.palette.destructive,
    }
}

fn normalized(percent: f32) -> f32 {
    if percent.is_nan() {
        0.0
    } else {
        percent.clamp(0.0, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::LIGHT;

    #[test]
    fn values_are_bounded_and_nan_is_empty() {
        assert_eq!(normalized(-1.0), 0.0);
        assert_eq!(normalized(f32::NAN), 0.0);
        assert_eq!(normalized(42.0), 42.0);
        assert_eq!(normalized(f32::INFINITY), 100.0);
        assert_eq!(normalized(101.0), 100.0);
    }

    #[test]
    fn variants_select_their_semantic_bar_color() {
        for (variant, expected) in [
            (ProgressVariant::Default, LIGHT.palette.primary),
            (ProgressVariant::Success, LIGHT.palette.success),
            (ProgressVariant::Warning, LIGHT.palette.warning),
            (ProgressVariant::Destructive, LIGHT.palette.destructive),
        ] {
            assert_eq!(style(&LIGHT, variant).bar, Background::Color(expected));
        }
    }
}
