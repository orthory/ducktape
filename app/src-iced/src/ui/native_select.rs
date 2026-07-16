//! A styled iced pick list with a focusable, keyboard-capable trigger.
//!
//! Pointer and touch opening, option hover, and option selection remain owned by
//! iced's [`PickList`]. The thin wrapper adds stable focus ownership and closed
//! trigger key handling without claiming browser or DOM `<select>` semantics.

use super::direction::Direction;
use super::focus_control::State as FocusState;
use super::theme::{Theme, alpha};
use iced::advanced::text::Renderer as _;
use iced::advanced::{
    Clipboard, Layout, Renderer as _, Shell, Widget, layout, mouse, overlay, renderer, text, widget,
};
use iced::alignment;
use iced::keyboard::{self, key};
use iced::widget::overlay::menu;
use iced::widget::pick_list;
use iced::widget::pick_list::{Handle, PickList};
use iced::{
    Background, Border, Color, Element, Event, Length, Padding, Pixels, Point, Rectangle, Shadow,
    Size, Vector, touch,
};
use std::borrow::Borrow;
use std::rc::Rc;

pub const NATIVE_SELECT_HEIGHT: f32 = 36.0;
pub const NATIVE_SELECT_TEXT_LINE_HEIGHT: f32 = 20.0;

const HORIZONTAL_PADDING: f32 = 12.0;
const CHEVRON_SLOT: f32 = 20.0;

type SelectFn<'a, T, Message> = dyn Fn(T) -> Message + 'a;

/// A themed native iced pick list with controlled options and selection.
///
/// Use [`NativeSelect::id`] with an ID kept in application state when focus
/// operations must address this trigger across view rebuilds. Existing concise
/// calls remain valid; builders cover iced's common pick-list controls plus
/// direction, disabled, and invalid states.
pub fn native_select<'a, T, L, V, Message>(
    options: L,
    selected: Option<V>,
    on_selected: impl Fn(T) -> Message + 'a,
    theme: &Theme,
) -> NativeSelect<'a, T, Message>
where
    T: ToString + PartialEq + Clone + 'a,
    L: Borrow<[T]>,
    V: Borrow<T>,
    Message: Clone + 'a,
{
    NativeSelect::new(widget::Id::unique(), options, selected, on_selected, theme)
}

/// Builds a native select with a caller-owned stable focus ID.
pub fn native_select_with_id<'a, T, L, V, Message>(
    id: widget::Id,
    options: L,
    selected: Option<V>,
    on_selected: impl Fn(T) -> Message + 'a,
    theme: &Theme,
) -> NativeSelect<'a, T, Message>
where
    T: ToString + PartialEq + Clone + 'a,
    L: Borrow<[T]>,
    V: Borrow<T>,
    Message: Clone + 'a,
{
    NativeSelect::new(id, options, selected, on_selected, theme)
}

/// Keyboard selection applied while the closed trigger owns focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSelectCommand {
    Previous,
    Next,
    First,
    Last,
}

/// Returns the option selected by a keyboard command.
///
/// Previous and next wrap, matching iced's other roving selection controls.
pub fn selection_index(
    option_count: usize,
    selected: Option<usize>,
    command: NativeSelectCommand,
) -> Option<usize> {
    if option_count == 0 {
        return None;
    }

    match command {
        NativeSelectCommand::First => Some(0),
        NativeSelectCommand::Last => Some(option_count - 1),
        NativeSelectCommand::Next => Some(
            selected
                .filter(|index| *index < option_count)
                .map_or(0, |index| (index + 1) % option_count),
        ),
        NativeSelectCommand::Previous => Some(
            selected
                .filter(|index| *index < option_count)
                .map_or(option_count - 1, |index| {
                    (index + option_count - 1) % option_count
                }),
        ),
    }
}

pub struct NativeSelect<'a, T, Message>
where
    T: ToString + PartialEq + Clone,
    Message: Clone,
{
    id: widget::Id,
    pick_list: PickList<'a, T, Rc<[T]>, T, Message>,
    options: Rc<[T]>,
    selected_index: Option<usize>,
    selected_label: Option<String>,
    placeholder: Option<String>,
    on_selected: Rc<SelectFn<'a, T, Message>>,
    width: Length,
    direction: Direction,
    disabled: bool,
    invalid: bool,
    theme: Theme,
}

impl<'a, T, Message> NativeSelect<'a, T, Message>
where
    T: ToString + PartialEq + Clone + 'a,
    Message: Clone + 'a,
{
    fn new<L, V>(
        id: widget::Id,
        options: L,
        selected: Option<V>,
        on_selected: impl Fn(T) -> Message + 'a,
        theme: &Theme,
    ) -> Self
    where
        L: Borrow<[T]>,
        V: Borrow<T>,
    {
        let options: Rc<[T]> = Rc::from(options.borrow().to_vec());
        let selected = selected.map(|value| value.borrow().clone());
        let selected_index = selected
            .as_ref()
            .and_then(|selected| options.iter().position(|option| option == selected));
        let selected_label = selected.as_ref().map(ToString::to_string);
        let on_selected: Rc<SelectFn<'a, T, Message>> = Rc::new(on_selected);
        let select = Rc::clone(&on_selected);
        let theme = *theme;

        let pick_list = pick_list(Rc::clone(&options), selected, move |option| select(option))
            .padding(Padding::from([8.0, HORIZONTAL_PADDING]))
            .text_size(theme.typography.sm)
            .text_line_height(text::LineHeight::Absolute(Pixels(
                NATIVE_SELECT_TEXT_LINE_HEIGHT,
            )))
            .handle(Handle::None)
            .menu_style(move |_iced_theme| menu_style(&theme));

        Self {
            id,
            pick_list,
            options,
            selected_index,
            selected_label,
            placeholder: None,
            on_selected,
            width: Length::Shrink,
            direction: Direction::LeftToRight,
            disabled: false,
            invalid: false,
            theme,
        }
        .restyle()
    }

    /// Replaces the focus ID. Keep it stable in application state.
    #[must_use]
    pub fn id(mut self, id: widget::Id) -> Self {
        self.id = id;
        self
    }

    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        let placeholder = placeholder.into();
        self.pick_list = self.pick_list.placeholder(placeholder.clone());
        self.placeholder = Some(placeholder);
        self
    }

    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        let width = width.into();
        self.pick_list = self.pick_list.width(width);
        self.width = width;
        self
    }

    #[must_use]
    pub fn menu_height(mut self, height: impl Into<Length>) -> Self {
        self.pick_list = self.pick_list.menu_height(height);
        self
    }

    #[must_use]
    pub fn on_open(mut self, message: Message) -> Self {
        self.pick_list = self.pick_list.on_open(message);
        self
    }

    #[must_use]
    pub fn on_close(mut self, message: Message) -> Self {
        self.pick_list = self.pick_list.on_close(message);
        self
    }

    #[must_use]
    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self.restyle()
    }

    #[must_use]
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self.restyle()
    }

    fn restyle(mut self) -> Self {
        let theme = self.theme;
        let disabled = self.disabled;
        let invalid = self.invalid;

        self.pick_list = self.pick_list.style(move |_iced_theme, status| {
            let mut style = state_style(&theme, status, disabled, invalid);
            // The wrapper draws trigger content so reading direction is explicit.
            style.text_color = Color::TRANSPARENT;
            style.placeholder_color = Color::TRANSPARENT;
            style.handle_color = Color::TRANSPARENT;
            style
        });
        self
    }
}

impl<'a, T, Message> Widget<Message, iced::Theme, iced::Renderer> for NativeSelect<'a, T, Message>
where
    T: ToString + PartialEq + Clone + 'a,
    Message: Clone + 'a,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<FocusState>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(FocusState::default())
    }

    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.pick_list as &dyn Widget<_, _, _>)]
    }

    fn diff(&self, tree: &mut widget::Tree) {
        tree.children[0].diff(&self.pick_list as &dyn Widget<_, _, _>);

        if self.disabled {
            tree.state.downcast_mut::<FocusState>().unfocus();
            // Reset iced's private open state so disabling an open trigger cannot
            // leave a menu waiting to reappear when it is enabled again.
            tree.children[0] = widget::Tree::new(&self.pick_list as &dyn Widget<_, _, _>);
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Fixed(NATIVE_SELECT_HEIGHT))
    }

    fn size_hint(&self) -> Size<Length> {
        self.size()
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits.height(Length::Fixed(NATIVE_SELECT_HEIGHT));
        let node = self
            .pick_list
            .layout(&mut tree.children[0], renderer, &limits);

        if self.width == Length::Shrink {
            layout::Node::new(Size::new(
                (node.size().width + CHEVRON_SLOT).min(limits.max().width),
                node.size().height,
            ))
        } else {
            node
        }
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if !self.disabled {
            operation.focusable(
                Some(&self.id),
                layout.bounds(),
                tree.state.downcast_mut::<FocusState>(),
            );
        }

        operation.traverse(&mut |operation| {
            self.pick_list
                .operate(&mut tree.children[0], layout, renderer, operation);
        });
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if self.disabled {
            tree.state.downcast_mut::<FocusState>().unfocus();
            return;
        }

        let bounds = layout.bounds();
        let focused = tree.state.downcast_ref::<FocusState>().is_focused();

        if focused
            && !shell.is_event_captured()
            && self.handle_keyboard(
                &mut tree.children[0],
                event,
                layout,
                renderer,
                clipboard,
                shell,
                viewport,
            )
        {
            return;
        }

        self.pick_list.update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if is_pointer_press(event) {
            let state = tree.state.downcast_mut::<FocusState>();
            if press_is_over(event, cursor, bounds) {
                state.focus();
            } else {
                state.unfocus();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if self.disabled {
            mouse::Interaction::None
        } else {
            self.pick_list
                .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        renderer_style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.pick_list.draw(
            &tree.children[0],
            renderer,
            theme,
            renderer_style,
            layout,
            cursor,
            viewport,
        );

        let bounds = layout.bounds();
        let label = self.selected_label.as_ref().or(self.placeholder.as_ref());
        let label_color = if self.disabled {
            alpha(self.theme.palette.foreground, 0.5)
        } else if self.selected_label.is_some() {
            self.theme.palette.foreground
        } else {
            self.theme.palette.muted_foreground
        };
        let handle_color = if self.disabled {
            alpha(self.theme.palette.muted_foreground, 0.5)
        } else {
            self.theme.palette.muted_foreground
        };

        let (label_x, label_alignment, handle_x, handle_alignment, label_clip) =
            content_geometry(bounds, self.direction);

        if let (Some(label), Some(clip)) = (label, label_clip.intersection(viewport)) {
            renderer.fill_text(
                text::Text {
                    content: label.clone(),
                    bounds: Size::new(label_clip.width, NATIVE_SELECT_TEXT_LINE_HEIGHT),
                    size: Pixels(self.theme.typography.sm),
                    line_height: text::LineHeight::Absolute(Pixels(NATIVE_SELECT_TEXT_LINE_HEIGHT)),
                    font: renderer.default_font(),
                    align_x: label_alignment,
                    align_y: alignment::Vertical::Center,
                    shaping: text::Shaping::Advanced,
                    wrapping: text::Wrapping::None,
                },
                Point::new(label_x, bounds.center_y()),
                label_color,
                clip,
            );
        }

        renderer.fill_text(
            text::Text {
                content: iced::Renderer::ARROW_DOWN_ICON.to_string(),
                bounds: Size::new(CHEVRON_SLOT, NATIVE_SELECT_TEXT_LINE_HEIGHT),
                size: Pixels(self.theme.typography.sm),
                line_height: text::LineHeight::Absolute(Pixels(NATIVE_SELECT_TEXT_LINE_HEIGHT)),
                font: iced::Renderer::ICON_FONT,
                align_x: handle_alignment,
                align_y: alignment::Vertical::Center,
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::None,
            },
            Point::new(handle_x, bounds.center_y()),
            handle_color,
            *viewport,
        );

        let state = tree.state.downcast_ref::<FocusState>();
        if state.is_focused() {
            let color = if self.invalid {
                self.theme.palette.destructive
            } else {
                self.theme.palette.ring
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: bounds.expand(4.0),
                    border: Border {
                        color,
                        width: 2.0,
                        radius: (self.theme.radius.md + 4.0).into(),
                    },
                    ..renderer::Quad::default()
                },
                Background::Color(Color::TRANSPARENT),
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        if self.disabled {
            None
        } else {
            self.pick_list.overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            )
        }
    }
}

impl<'a, T, Message> NativeSelect<'a, T, Message>
where
    T: ToString + PartialEq + Clone + 'a,
    Message: Clone + 'a,
{
    #[allow(clippy::too_many_arguments)]
    fn handle_keyboard(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> bool {
        let Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modifiers,
            repeat,
            ..
        }) = event
        else {
            return false;
        };

        if !modifiers.command() && !modifiers.alt() {
            if let Some(command) = key_command(key) {
                let index = selection_index(self.options.len(), self.selected_index, command);
                if index != self.selected_index
                    && let Some(option) = index.and_then(|index| self.options.get(index))
                {
                    shell.publish((self.on_selected)(option.clone()));
                }
                shell.capture_event();
                return true;
            }

            if !repeat
                && matches!(
                    key,
                    keyboard::Key::Named(key::Named::Enter | key::Named::Space)
                )
            {
                // PickList does not expose an open operation. Feed its own pointer
                // path so iced still owns the menu state and overlay behavior.
                let event = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
                self.pick_list.update(
                    tree,
                    &event,
                    layout,
                    mouse::Cursor::Available(layout.bounds().center()),
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );
                shell.capture_event();
                return true;
            }
        }

        false
    }
}

impl<'a, T, Message> From<NativeSelect<'a, T, Message>> for Element<'a, Message>
where
    T: ToString + PartialEq + Clone + 'a,
    Message: Clone + 'a,
{
    fn from(select: NativeSelect<'a, T, Message>) -> Self {
        Element::new(select)
    }
}

fn key_command(key: &keyboard::Key) -> Option<NativeSelectCommand> {
    match key {
        keyboard::Key::Named(key::Named::ArrowUp) => Some(NativeSelectCommand::Previous),
        keyboard::Key::Named(key::Named::ArrowDown) => Some(NativeSelectCommand::Next),
        keyboard::Key::Named(key::Named::Home) => Some(NativeSelectCommand::First),
        keyboard::Key::Named(key::Named::End) => Some(NativeSelectCommand::Last),
        _ => None,
    }
}

fn is_pointer_press(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. })
    )
}

fn press_is_over(event: &Event, cursor: mouse::Cursor, bounds: Rectangle) -> bool {
    match event {
        Event::Touch(touch::Event::FingerPressed { position, .. }) => bounds.contains(*position),
        _ => cursor.is_over(bounds),
    }
}

fn content_geometry(
    bounds: Rectangle,
    direction: Direction,
) -> (f32, text::Alignment, f32, text::Alignment, Rectangle) {
    let label_width = (bounds.width - HORIZONTAL_PADDING * 2.0 - CHEVRON_SLOT).max(0.0);

    match direction {
        Direction::LeftToRight => (
            bounds.x + HORIZONTAL_PADDING,
            text::Alignment::Left,
            bounds.x + bounds.width - HORIZONTAL_PADDING,
            text::Alignment::Right,
            Rectangle::new(
                Point::new(bounds.x + HORIZONTAL_PADDING, bounds.y),
                Size::new(label_width, bounds.height),
            ),
        ),
        Direction::RightToLeft => (
            bounds.x + bounds.width - HORIZONTAL_PADDING,
            text::Alignment::Right,
            bounds.x + HORIZONTAL_PADDING,
            text::Alignment::Left,
            Rectangle::new(
                Point::new(bounds.x + HORIZONTAL_PADDING + CHEVRON_SLOT, bounds.y),
                Size::new(label_width, bounds.height),
            ),
        ),
    }
}

pub fn style(
    theme: &Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    state_style(theme, status, false, false)
}

pub fn state_style(
    theme: &Theme,
    status: iced::widget::pick_list::Status,
    disabled: bool,
    invalid: bool,
) -> iced::widget::pick_list::Style {
    use iced::widget::pick_list::Status;

    let mut border_color = if invalid {
        theme.palette.destructive
    } else {
        theme.palette.input
    };
    let mut border_width = 1.0;
    let mut background = theme.palette.background;
    let mut text_color = theme.palette.foreground;
    let mut placeholder_color = theme.palette.muted_foreground;
    let mut handle_color = theme.palette.muted_foreground;

    if disabled {
        background = theme.palette.muted;
        border_color = alpha(border_color, 0.5);
        text_color = alpha(text_color, 0.5);
        placeholder_color = alpha(placeholder_color, 0.5);
        handle_color = alpha(handle_color, 0.5);
    } else {
        match status {
            Status::Active => {}
            Status::Hovered if !invalid => {
                border_color = theme.palette.foreground;
                handle_color = theme.palette.foreground;
            }
            Status::Opened { .. } => {
                border_color = if invalid {
                    theme.palette.destructive
                } else {
                    theme.palette.ring
                };
                border_width = 2.0;
                handle_color = theme.palette.foreground;
            }
            Status::Hovered => {}
        }
    }

    iced::widget::pick_list::Style {
        text_color,
        placeholder_color,
        handle_color,
        background: Background::Color(background),
        border: Border {
            color: border_color,
            width: border_width,
            radius: theme.radius.md.into(),
        },
    }
}

pub fn menu_style(theme: &Theme) -> menu::Style {
    menu::Style {
        background: Background::Color(theme.palette.popover),
        border: Border {
            color: theme.palette.border,
            width: 1.0,
            radius: theme.radius.md.into(),
        },
        text_color: theme.palette.popover_foreground,
        selected_text_color: theme.palette.accent_foreground,
        selected_background: Background::Color(theme.palette.accent),
        shadow: Shadow {
            color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.14),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::LIGHT;

    #[test]
    fn opened_select_uses_ring_and_accent_roles() {
        let field = style(
            &LIGHT,
            iced::widget::pick_list::Status::Opened { is_hovered: false },
        );
        let menu = menu_style(&LIGHT);

        assert_eq!(field.border.color, LIGHT.palette.ring);
        assert_eq!(field.border.width, 2.0);
        assert_eq!(
            menu.selected_background,
            Background::Color(LIGHT.palette.accent)
        );
        assert_eq!(menu.selected_text_color, LIGHT.palette.accent_foreground);
    }

    #[test]
    fn keyboard_selection_wraps_and_handles_empty_options() {
        assert_eq!(selection_index(0, None, NativeSelectCommand::Next), None);
        assert_eq!(selection_index(3, None, NativeSelectCommand::Next), Some(0));
        assert_eq!(
            selection_index(3, None, NativeSelectCommand::Previous),
            Some(2)
        );
        assert_eq!(
            selection_index(3, Some(2), NativeSelectCommand::Next),
            Some(0)
        );
        assert_eq!(
            selection_index(3, Some(0), NativeSelectCommand::Previous),
            Some(2)
        );
    }

    #[test]
    fn home_and_end_select_boundaries() {
        assert_eq!(
            selection_index(4, Some(2), NativeSelectCommand::First),
            Some(0)
        );
        assert_eq!(
            selection_index(4, Some(1), NativeSelectCommand::Last),
            Some(3)
        );
    }

    #[test]
    fn disabled_and_invalid_styles_keep_semantic_states() {
        let invalid = state_style(
            &LIGHT,
            iced::widget::pick_list::Status::Hovered,
            false,
            true,
        );
        let disabled = state_style(
            &LIGHT,
            iced::widget::pick_list::Status::Hovered,
            true,
            false,
        );

        assert_eq!(invalid.border.color, LIGHT.palette.destructive);
        assert_eq!(disabled.background, Background::Color(LIGHT.palette.muted));
        assert!(disabled.text_color.a < LIGHT.palette.foreground.a);
    }

    #[test]
    fn content_alignment_flips_for_rtl() {
        let bounds = Rectangle::new(Point::new(10.0, 20.0), Size::new(180.0, 36.0));
        let ltr = content_geometry(bounds, Direction::LeftToRight);
        let rtl = content_geometry(bounds, Direction::RightToLeft);

        assert_eq!(ltr.1, text::Alignment::Left);
        assert_eq!(ltr.3, text::Alignment::Right);
        assert_eq!(rtl.1, text::Alignment::Right);
        assert_eq!(rtl.3, text::Alignment::Left);
        assert!(ltr.0 < ltr.2);
        assert!(rtl.0 > rtl.2);
    }

    #[test]
    fn trigger_metrics_match_shadcn_compact_control() {
        assert_eq!(NATIVE_SELECT_HEIGHT, 36.0);
        assert_eq!(NATIVE_SELECT_TEXT_LINE_HEIGHT, 20.0);
        assert_eq!(NATIVE_SELECT_TEXT_LINE_HEIGHT + 16.0, NATIVE_SELECT_HEIGHT);
    }

    #[test]
    fn stable_id_wrapper_owns_one_pick_list_child() {
        let select = native_select_with_id(
            widget::Id::new("native-select-test"),
            ["Light", "Dark"],
            Some("Light"),
            |value| value,
            &LIGHT,
        );
        let tree = widget::Tree::new(&select as &dyn Widget<_, _, _>);

        assert_eq!(tree.children.len(), 1);
        assert!(!tree.state.downcast_ref::<FocusState>().is_focused());
    }
}
