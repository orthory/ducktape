use super::theme::{Theme, alpha};
use iced::widget::{Container, Grid, Space, container};
use iced::{Background, Border};
use std::time::Duration;

pub const FRAME_COUNT: u8 = 8;
pub const TICK_INTERVAL: Duration = Duration::from_millis(90);

/// Returns the next controlled animation frame.
///
/// Call this from the message produced by `iced::time::every(TICK_INTERVAL)`.
/// When reduced motion is requested, the current frame is intentionally frozen.
pub const fn next_frame(frame: u8, reduced_motion: bool) -> u8 {
    if reduced_motion {
        frame
    } else {
        (frame + 1) % FRAME_COUNT
    }
}

/// A controlled decorative loading indicator.
///
/// Pair it with a visible status label such as `Loading…`; the spinner does not
/// replace that label. The caller owns the timer and passes its current frame.
pub fn spinner<'a, Message>(frame: u8, reduced_motion: bool, theme: &Theme) -> Grid<'a, Message>
where
    Message: 'a,
{
    const RING_POSITION: [u8; 9] = [0, 1, 2, 7, 0, 3, 6, 5, 4];

    let mut spinner = Grid::new().columns(3).spacing(2).width(20).height(20.0);
    for (cell, position) in RING_POSITION.into_iter().enumerate() {
        spinner = if cell == 4 {
            spinner.push(Space::new())
        } else {
            spinner.push(dot(
                alpha(
                    theme.palette.foreground,
                    opacity(position, frame, reduced_motion),
                ),
                theme.radius.sm,
            ))
        };
    }
    spinner
}

fn dot<'a, Message>(color: iced::Color, radius: f32) -> Container<'a, Message>
where
    Message: 'a,
{
    container(Space::new()).style(move |_| iced::widget::container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            radius: radius.into(),
            ..Default::default()
        },
        ..Default::default()
    })
}

fn opacity(position: u8, frame: u8, reduced_motion: bool) -> f32 {
    if reduced_motion {
        return 0.65;
    }

    let distance = (position + FRAME_COUNT - frame % FRAME_COUNT) % FRAME_COUNT;
    1.0 - f32::from(distance) * 0.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_wraps_and_reduced_motion_freezes() {
        assert_eq!(next_frame(FRAME_COUNT - 1, false), 0);
        assert_eq!(next_frame(3, true), 3);
        assert_eq!(opacity(0, 6, true), 0.65);
    }
}
