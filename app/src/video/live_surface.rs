// Verbatim copy of ducktape-ui's `crates/ui-lang-runtime/src/live_surface.rs`
// at 37fde876^ (the last revision carrying it). Upstream deleted the module in
// #742 (37fde876) as "unused" — a repo-wide grep there finds no consumer — but
// this app IS its consumer: `call_video_tiles` and `call_video_stage` below
// mount it for the huddle's video strip and stage. Vendored here, unchanged,
// so the pin can move past #742.
//
//! A self-redrawing paint surface: draws through a closure and schedules the
//! next redraw of ITS OWN window — no app message, no view rebuild, no other
//! window woken.
//!
//! THE POINT IS WHAT IT AVOIDS. The state-driven way to animate a surface is
//! a timer subscription republishing a counter into state — and one message
//! rebuilds EVERY window's whole view tree, so a 60 Hz surface taxes windows
//! that show none of its pixels. Redraw requests, by contrast, are
//! per-window all the way down to winit (the same machinery as the editor's
//! cursor blink), so a live surface costs its window a paint pass and costs
//! the rest of the app nothing. Reach for this for any region that must
//! refresh on its own clock — video tiles, level meters, waveforms — and
//! keep ordinary state-driven UI out of it.
//!
//! The surface is DUMB on purpose: no children, no events, no theme — a
//! layout rule and a paint closure over data the closures own. Content that
//! needs ice-composed children is a different feature; build it when a
//! consumer exists.

use std::time::Duration;

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Shell, Widget, mouse, renderer};
use iced::{Event, Length, Rectangle, Size, window};

/// The paint callback: renderer, the surface's bounds, the viewport.
type PaintFn<'a, Renderer> = Box<dyn Fn(&mut Renderer, Rectangle, &Rectangle) + 'a>;

/// A [`LiveSurface`]'s callbacks: how tall it is, when its shape changed,
/// whether it is live, and how to paint it.
pub struct LiveSurface<'a, Renderer> {
    /// The paint cadence while `active` holds.
    interval: Duration,
    /// Available width → the surface's size, read at layout time.
    size: Box<dyn Fn(f32) -> Size + 'a>,
    /// A key that changes exactly when the layout-relevant shape changes;
    /// checked once per beat, and a change invalidates layout.
    layout_key: Box<dyn Fn() -> u64 + 'a>,
    /// Whether the next beat should be scheduled. When this goes false the
    /// clock parks; the next app-driven redraw of the window re-arms it.
    active: Box<dyn Fn() -> bool + 'a>,
    /// Paints the surface — see [`PaintFn`].
    paint: PaintFn<'a, Renderer>,
}

/// Creates a [`LiveSurface`] from its cadence and callbacks.
pub fn live_surface<'a, Renderer>(
    interval: Duration,
    size: impl Fn(f32) -> Size + 'a,
    layout_key: impl Fn() -> u64 + 'a,
    active: impl Fn() -> bool + 'a,
    paint: impl Fn(&mut Renderer, Rectangle, &Rectangle) + 'a,
) -> LiveSurface<'a, Renderer> {
    LiveSurface {
        interval,
        size: Box::new(size),
        layout_key: Box::new(layout_key),
        active: Box::new(active),
        paint: Box::new(paint),
    }
}

#[derive(Default)]
struct LiveState {
    layout_key: u64,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for LiveSurface<'_, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<LiveState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(LiveState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        tree.state.downcast_mut::<LiveState>().layout_key = (self.layout_key)();
        layout::Node::new((self.size)(limits.max().width))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let Event::Window(window::Event::RedrawRequested(now)) = event else {
            return;
        };
        if tree.state.downcast_ref::<LiveState>().layout_key != (self.layout_key)() {
            shell.invalidate_layout();
        }
        if (self.active)() {
            shell.request_redraw_at(*now + self.interval);
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        (self.paint)(renderer, layout.bounds(), viewport);
    }
}

impl<'a, Message, Theme, Renderer> From<LiveSurface<'a, Renderer>>
    for iced::Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(surface: LiveSurface<'a, Renderer>) -> Self {
        Self::new(surface)
    }
}
