//! Keyboard-focusable activation for passive content.
//!
//! This wrapper owns pointer, touch, Enter, and Space activation. Its child is
//! expected to be passive content (text, icons, or layout), not another button;
//! interactive children capture their own events and are deliberately not
//! activated a second time.

use super::theme::Theme as UiTheme;
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer, widget};
use iced::keyboard::{self, key};
use iced::{
    Background, Border, Color, Element, Event, Length, Rectangle, Shadow, Size, Vector, touch,
};

/// The interaction state supplied to [`FocusControl::style`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Active,
    Hovered,
    Focused,
    Pressed,
    Disabled,
}

/// Visuals drawn around the wrapped content.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub background: Option<Background>,
    pub text_color: Option<Color>,
    pub border: Border,
    pub shadow: Shadow,
    pub focus_ring: Border,
    pub focus_offset: f32,
}

/// The default transparent shell and semantic focus ring.
pub fn style(theme: &UiTheme, _status: Status) -> Style {
    Style {
        background: None,
        text_color: None,
        border: Border::default(),
        shadow: Shadow::default(),
        focus_ring: Border {
            color: theme.palette.ring,
            width: 2.0,
            radius: (theme.radius.md + 4.0).into(),
        },
        focus_offset: 2.0,
    }
}

type StyleFn<'a, Theme> = dyn Fn(&Theme, Status) -> Style + 'a;
type KeyPressFn<'a, Message> = dyn Fn(keyboard::Key, keyboard::Modifiers) -> Option<Message> + 'a;

/// Persistent focus and press state exposed through iced widget operations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    focused: bool,
    press: Option<Press>,
}

impl State {
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn is_pressed(&self) -> bool {
        self.press.is_some()
    }

    pub fn focus(&mut self) {
        self.focused = true;
    }

    pub fn unfocus(&mut self) {
        self.focused = false;
        self.press = None;
    }
}

impl widget::operation::Focusable for State {
    fn is_focused(&self) -> bool {
        self.is_focused()
    }

    fn focus(&mut self) {
        self.focus();
    }

    fn unfocus(&mut self) {
        self.unfocus();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Press {
    Pointer,
    Enter,
    Space,
}

/// A focusable control shell for tabs, toggles, switches, and disclosures.
///
/// The caller must route Tab and Shift+Tab to iced's
/// `advanced::widget::operate::focus_next` and `focus_previous` tasks. A stable
/// [`widget::Id`] lets callers focus or query this control with the matching
/// iced widget operations.
pub struct FocusControl<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: renderer::Renderer,
{
    id: widget::Id,
    content: Element<'a, Message, Theme, Renderer>,
    on_activate: Message,
    on_key_press: Option<Box<KeyPressFn<'a, Message>>>,
    disabled: bool,
    style: Box<StyleFn<'a, Theme>>,
}

/// Wraps passive content in a keyboard- and pointer-activatable control.
pub fn focus_control<'a, Message>(
    id: widget::Id,
    content: impl Into<Element<'a, Message>>,
    on_activate: Message,
    theme: &UiTheme,
) -> FocusControl<'a, Message>
where
    Message: Clone + 'a,
{
    FocusControl::new(id, content, on_activate, theme)
}

impl<'a, Message, Theme, Renderer> FocusControl<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    pub fn new(
        id: widget::Id,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        on_activate: Message,
        theme: &UiTheme,
    ) -> Self {
        let theme = *theme;

        Self {
            id,
            content: content.into(),
            on_activate,
            on_key_press: None,
            disabled: false,
            style: Box::new(move |_iced_theme, status| style(&theme, status)),
        }
    }

    pub fn id(&self) -> &widget::Id {
        &self.id
    }

    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Publishes a caller-selected message for additional focused key presses.
    ///
    /// Use this for compound-control navigation such as arrow keys, Home, and
    /// End. If the callback returns a message, the key event is captured and
    /// normal Enter/Space activation does not run for that press.
    #[must_use]
    pub fn on_key_press(
        mut self,
        handler: impl Fn(keyboard::Key, keyboard::Modifiers) -> Option<Message> + 'a,
    ) -> Self {
        self.on_key_press = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self {
        self.style = Box::new(style);
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for FocusControl<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));

        if self.disabled {
            tree.state.downcast_mut::<State>().unfocus();
        }
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        {
            let state = tree.state.downcast_mut::<State>();

            if self.disabled {
                state.unfocus();
            } else {
                operation.focusable(Some(&self.id), layout.bounds(), state);
            }
        }

        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            );
        });
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let is_over = cursor.is_over(layout.bounds());
        let state = tree.state.downcast_mut::<State>();

        if shell.is_event_captured() {
            if is_pointer_press(event) && !is_over {
                state.unfocus();
            }

            return;
        }

        if handle_key_press(
            state,
            event,
            !self.disabled,
            self.on_key_press.as_deref(),
            shell,
        ) {
            return;
        }

        handle_event(
            state,
            event,
            is_over,
            !self.disabled,
            &self.on_activate,
            shell,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let interaction = self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        );

        if interaction == mouse::Interaction::None
            && !self.disabled
            && cursor.is_over(layout.bounds())
        {
            mouse::Interaction::Pointer
        } else {
            interaction
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        renderer_style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let status = status(state, self.disabled, cursor.is_over(layout.bounds()));
        let style = (self.style)(theme, status);

        if style.background.is_some() || style.border.width > 0.0 || style.shadow.color.a > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: style.border,
                    shadow: style.shadow,
                    ..renderer::Quad::default()
                },
                style
                    .background
                    .unwrap_or(Background::Color(Color::TRANSPARENT)),
            );
        }

        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            &renderer::Style {
                text_color: style.text_color.unwrap_or(renderer_style.text_color),
            },
            layout,
            cursor,
            viewport,
        );

        if state.is_focused()
            && !self.disabled
            && style.focus_ring.width > 0.0
            && style.focus_ring.color.a > 0.0
        {
            let expansion = style.focus_offset.max(0.0) + style.focus_ring.width;

            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds().expand(expansion),
                    border: style.focus_ring,
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
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<FocusControl<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(control: FocusControl<'a, Message, Theme, Renderer>) -> Self {
        Element::new(control)
    }
}

fn status(state: &State, disabled: bool, hovered: bool) -> Status {
    if disabled {
        Status::Disabled
    } else if state.is_pressed()
        && state
            .press
            .is_some_and(|press| press != Press::Pointer || hovered)
    {
        Status::Pressed
    } else if hovered {
        Status::Hovered
    } else if state.is_focused() {
        Status::Focused
    } else {
        Status::Active
    }
}

fn is_pointer_press(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. })
    )
}

fn activation_key(key: &keyboard::Key) -> Option<Press> {
    match key {
        keyboard::Key::Named(key::Named::Enter) => Some(Press::Enter),
        keyboard::Key::Named(key::Named::Space) => Some(Press::Space),
        _ => None,
    }
}

fn handle_event<Message: Clone>(
    state: &mut State,
    event: &Event,
    is_over: bool,
    enabled: bool,
    on_activate: &Message,
    shell: &mut Shell<'_, Message>,
) {
    if !enabled {
        state.unfocus();
        return;
    }

    match event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        | Event::Touch(touch::Event::FingerPressed { .. }) => {
            if is_over {
                state.focus();
                state.press = Some(Press::Pointer);
                shell.capture_event();
                shell.request_redraw();
            } else {
                state.unfocus();
            }
        }
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
        | Event::Touch(touch::Event::FingerLifted { .. }) => {
            if state.press == Some(Press::Pointer) {
                state.press = None;

                if is_over {
                    shell.publish(on_activate.clone());
                }

                shell.capture_event();
                shell.request_redraw();
            }
        }
        Event::Touch(touch::Event::FingerLost { .. }) => {
            if state.press == Some(Press::Pointer) {
                state.press = None;
                shell.request_redraw();
            }
        }
        Event::Keyboard(keyboard::Event::KeyPressed { key, repeat, .. }) if state.is_focused() => {
            if let Some(press) = activation_key(key) {
                if !repeat && state.press.is_none() {
                    state.press = Some(press);
                    shell.request_redraw();
                }

                shell.capture_event();
            }
        }
        Event::Keyboard(keyboard::Event::KeyReleased { key, .. }) if state.is_focused() => {
            if let Some(press) = activation_key(key)
                && state.press == Some(press)
            {
                state.press = None;
                shell.publish(on_activate.clone());
                shell.capture_event();
                shell.request_redraw();
            }
        }
        _ => {}
    }
}

fn handle_key_press<Message>(
    state: &State,
    event: &Event,
    enabled: bool,
    handler: Option<&KeyPressFn<'_, Message>>,
    shell: &mut Shell<'_, Message>,
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

    if !enabled || !state.is_focused() || *repeat {
        return false;
    }

    let Some(message) = handler.and_then(|handler| handler(key.clone(), *modifiers)) else {
        return false;
    };

    shell.publish(message);
    shell.capture_event();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::event;
    use iced::keyboard::{Location, Modifiers};

    fn key_event(named: key::Named, pressed: bool) -> Event {
        let key = keyboard::Key::Named(named);
        let physical_key = key::Physical::Code(match named {
            key::Named::Enter => key::Code::Enter,
            key::Named::Space => key::Code::Space,
            _ => key::Code::Escape,
        });

        Event::Keyboard(if pressed {
            keyboard::Event::KeyPressed {
                key: key.clone(),
                modified_key: key,
                physical_key,
                location: Location::Standard,
                modifiers: Modifiers::default(),
                text: None,
                repeat: false,
            }
        } else {
            keyboard::Event::KeyReleased {
                key: key.clone(),
                modified_key: key,
                physical_key,
                location: Location::Standard,
                modifiers: Modifiers::default(),
            }
        })
    }

    #[test]
    fn enter_and_space_activate_only_after_focus() {
        for named in [key::Named::Enter, key::Named::Space] {
            let mut state = State::default();
            let mut messages = Vec::new();

            {
                let mut shell = Shell::new(&mut messages);
                handle_event(
                    &mut state,
                    &key_event(named, true),
                    false,
                    true,
                    &7,
                    &mut shell,
                );
                assert_eq!(shell.event_status(), event::Status::Ignored);
            }
            assert!(messages.is_empty());

            state.focus();
            {
                let mut shell = Shell::new(&mut messages);
                handle_event(
                    &mut state,
                    &key_event(named, true),
                    false,
                    true,
                    &7,
                    &mut shell,
                );
                assert!(state.is_pressed());
                assert_eq!(shell.event_status(), event::Status::Captured);
            }
            assert!(messages.is_empty());

            {
                let mut shell = Shell::new(&mut messages);
                handle_event(
                    &mut state,
                    &key_event(named, false),
                    false,
                    true,
                    &7,
                    &mut shell,
                );
                assert_eq!(shell.event_status(), event::Status::Captured);
            }
            assert_eq!(messages, [7]);
            assert!(!state.is_pressed());
        }
    }

    #[test]
    fn pointer_activates_on_release_inside_and_unfocuses_outside() {
        let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        let release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        let mut state = State::default();
        let mut messages = Vec::new();

        {
            let mut shell = Shell::new(&mut messages);
            handle_event(&mut state, &press, true, true, &9, &mut shell);
        }
        assert!(state.is_focused());
        assert!(state.is_pressed());

        {
            let mut shell = Shell::new(&mut messages);
            handle_event(&mut state, &release, true, true, &9, &mut shell);
        }
        assert_eq!(messages, [9]);

        {
            let mut shell = Shell::new(&mut messages);
            handle_event(&mut state, &press, false, true, &9, &mut shell);
        }
        assert!(!state.is_focused());
    }

    #[test]
    fn disabled_controls_are_not_focused_or_activated() {
        let mut state = State::default();
        state.focus();
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);

        handle_event(
            &mut state,
            &key_event(key::Named::Enter, true),
            false,
            false,
            &1,
            &mut shell,
        );

        assert!(!state.is_focused());
        assert!(messages.is_empty());
    }

    #[test]
    fn status_preserves_focus_and_press_feedback() {
        let mut state = State::default();
        assert_eq!(status(&state, false, false), Status::Active);
        assert_eq!(status(&state, false, true), Status::Hovered);

        state.focus();
        assert_eq!(status(&state, false, false), Status::Focused);
        state.press = Some(Press::Space);
        assert_eq!(status(&state, false, false), Status::Pressed);
        assert_eq!(status(&state, true, true), Status::Disabled);
    }

    #[test]
    fn additional_key_binding_requires_focus_and_captures_a_match() {
        let handler = |key: keyboard::Key, _modifiers| {
            (key == keyboard::Key::Named(key::Named::ArrowRight)).then_some(11)
        };
        let mut state = State::default();
        let mut messages = Vec::new();

        {
            let mut shell = Shell::new(&mut messages);
            assert!(!handle_key_press(
                &state,
                &key_event(key::Named::ArrowRight, true),
                true,
                Some(&handler),
                &mut shell,
            ));
        }

        state.focus();
        {
            let mut shell = Shell::new(&mut messages);
            assert!(handle_key_press(
                &state,
                &key_event(key::Named::ArrowRight, true),
                true,
                Some(&handler),
                &mut shell,
            ));
            assert_eq!(shell.event_status(), event::Status::Captured);
        }
        assert_eq!(messages, [11]);
    }
}
