//! Guest input from native overlays and opted-in window mouse observations.
use super::*;

pub(super) fn mouse(
    guest: &mut Guest,
    event: mouse::Event,
    origin: iced::Point,
    captured: bool,
) -> bool {
    if !guest.frame.mouse_interest {
        return false;
    }
    let mut event: wire::mouse::Event = event.into();
    if let wire::mouse::Event::CursorMoved { x, y } = &mut event {
        *x -= origin.x;
        *y -= origin.y;
    }
    let Some(event) = event.sanitize() else {
        return false;
    };
    if matches!(event, wire::mouse::Event::CursorMoved { .. }) {
        // One move per frame, at its latest position among discrete events.
        guest.pending.retain(|event| {
            !matches!(
                event,
                wire::Event::Mouse {
                    event: wire::mouse::Event::CursorMoved { .. },
                    ..
                }
            )
        });
    }
    guest.pending.push(wire::Event::Mouse { event, captured });
    true
}

pub(super) fn overlay<'a>(
    content: overlay::Element<'a, Output, iced::Theme, iced::Renderer>,
    mounted: Arc<Mutex<Mounted>>,
    generation: u64,
    alive: Arc<()>,
    origin: iced::Point,
) -> overlay::Element<'a, ModuleViewEvent, iced::Theme, iced::Renderer> {
    overlay::Element::new(Box::new(InputOverlay {
        content,
        mounted,
        generation,
        alive,
        origin,
    }))
}

struct InputOverlay<'a> {
    content: overlay::Element<'a, Output, iced::Theme, iced::Renderer>,
    mounted: Arc<Mutex<Mounted>>,
    generation: u64,
    alive: Arc<()>,
    origin: iced::Point,
}

impl overlay::Overlay<ModuleViewEvent, iced::Theme, iced::Renderer> for InputOverlay<'_> {
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> layout::Node {
        self.content.as_overlay_mut().layout(renderer, bounds)
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.content
            .as_overlay()
            .draw(renderer, theme, style, layout, cursor);
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_overlay_mut()
            .operate(layout, renderer, operation);
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, ModuleViewEvent>,
    ) {
        let mut mounted = self.mounted.lock().expect("module view lock");
        let generation_matches = mounted.generation == self.generation;
        let Slot::Ready(guest) = &mut mounted.slot else {
            return;
        };
        let current_instance = generation_matches && Arc::ptr_eq(&guest.alive, &self.alive);
        if !current_instance {
            shell.invalidate_widgets();
            shell.request_redraw();
            return;
        }
        let mut outputs = Vec::new();
        let mut local = Shell::new(&mut outputs);
        self.content
            .as_overlay_mut()
            .update(event, layout, cursor, renderer, clipboard, &mut local);
        let captured = local.is_event_captured();
        if captured {
            shell.capture_event();
        }
        if local.is_layout_invalid() {
            shell.invalidate_layout();
        }
        if local.are_widgets_invalid() {
            shell.invalidate_widgets();
        }
        shell.request_redraw_at(local.redraw_request());
        shell.input_method_mut().merge(local.input_method());
        if !outputs.is_empty() {
            for output in outputs {
                guest.deliver(output);
            }
            shell.request_redraw();
        }
        // Keep the base widget's ordering: routed output before observation.
        if captured
            && let Event::Mouse(event) = event
            && mouse(guest, *event, self.origin, true)
        {
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_overlay()
            .mouse_interaction(layout, cursor, renderer)
    }

    fn overlay<'a>(
        &'a mut self,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
    ) -> Option<overlay::Element<'a, ModuleViewEvent, iced::Theme, iced::Renderer>> {
        self.content
            .as_overlay_mut()
            .overlay(layout, renderer)
            .map(|content| {
                overlay(
                    content,
                    self.mounted.clone(),
                    self.generation,
                    self.alive.clone(),
                    self.origin,
                )
            })
    }

    fn index(&self) -> f32 {
        self.content.as_overlay().index()
    }
}
