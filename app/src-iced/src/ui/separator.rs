use super::theme::Theme;
use iced::Theme as IcedTheme;
use iced::widget::rule;
use iced::widget::rule::{FillMode, Rule, Style};

pub fn horizontal(theme: &Theme) -> Rule<'static, IcedTheme> {
    styled(rule::horizontal(1), theme)
}

pub fn vertical(theme: &Theme) -> Rule<'static, IcedTheme> {
    styled(rule::vertical(1), theme)
}

fn styled(rule: Rule<'static, IcedTheme>, theme: &Theme) -> Rule<'static, IcedTheme> {
    let color = theme.palette.border;
    rule.style(move |_iced_theme| Style {
        color,
        radius: 0.0.into(),
        fill_mode: FillMode::Full,
        snap: true,
    })
}
