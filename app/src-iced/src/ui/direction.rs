use iced::Element;
use iced::alignment::Horizontal;
use iced::widget::Row;

/// Explicit layout direction for components that cannot inherit DOM-style context.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    LeftToRight,
    RightToLeft,
}

impl Direction {
    pub const fn start(self) -> Horizontal {
        match self {
            Self::LeftToRight => Horizontal::Left,
            Self::RightToLeft => Horizontal::Right,
        }
    }

    pub const fn end(self) -> Horizontal {
        match self {
            Self::LeftToRight => Horizontal::Right,
            Self::RightToLeft => Horizontal::Left,
        }
    }
}

/// Builds a row in reading order while preserving each caller-owned element.
pub fn directed_row<'a, Message>(
    items: impl IntoIterator<Item = Element<'a, Message>>,
    direction: Direction,
) -> Row<'a, Message>
where
    Message: 'a,
{
    let mut items = items.into_iter().collect::<Vec<_>>();
    if direction == Direction::RightToLeft {
        items.reverse();
    }
    items.into_iter().fold(Row::new(), Row::push)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_end_follow_reading_direction() {
        assert_eq!(Direction::LeftToRight.start(), Horizontal::Left);
        assert_eq!(Direction::LeftToRight.end(), Horizontal::Right);
        assert_eq!(Direction::RightToLeft.start(), Horizontal::Right);
        assert_eq!(Direction::RightToLeft.end(), Horizontal::Left);
    }
}
