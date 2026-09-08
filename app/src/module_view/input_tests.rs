//! Actual staged Chat overlays, laid out and clicked through ModuleView.
use super::*;
use iced::advanced::clipboard;
use iced::advanced::renderer::Headless as _;
use iced_test::runtime::{UserInterface, user_interface};

struct TextBounds<'a> {
    text: &'a str,
    found: Option<Rectangle>,
}
impl Operation for TextBounds<'_> {
    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
        visit(self);
    }
    fn text(&mut self, _: Option<&iced::widget::Id>, bounds: Rectangle, text: &str) {
        if text == self.text {
            self.found = Some(bounds);
        }
    }
}

type Ui = UserInterface<'static, ModuleViewEvent, iced::Theme, iced::Renderer>;
fn bounds(ui: &mut Ui, renderer: &iced::Renderer, text: &str) -> Option<Rectangle> {
    let mut op = TextBounds { text, found: None };
    ui.operate(renderer, &mut op);
    op.found
}

fn seated(action: &str) -> Arc<Mutex<Mounted>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/views/chat_view.wasm");
    let mut guest = Guest::load_from("chat", &path).expect("build current chat view first");
    guest.redraw(&None);
    let mut props: serde_json::Value =
        serde_json::from_slice(&tests::chat_facts().unwrap()).unwrap();
    props["selected_message_seq"] = 1.into();
    props["selected_message_rev"] = 1.into();
    props["message_action"] = action.into();
    let props = Some(serde_json::to_vec(&props).unwrap());
    guest.redraw(&props);
    assert!(guest.fault.is_none());
    Arc::new(Mutex::new(Mounted {
        slot: Slot::Ready(Box::new(guest)),
        props,
        generation: 1,
        hash: None,
        in_flight: false,
        wanted: None,
    }))
}

fn view(mounted: &Arc<Mutex<Mounted>>) -> Element<'static, ModuleViewEvent> {
    let mounted_guard = mounted.lock().unwrap();
    let Slot::Ready(guest) = &mounted_guard.slot else {
        panic!("a live guest")
    };
    Element::new(ModuleView {
        mounted: mounted.clone(),
        generation: mounted_guard.generation,
        rev: guest.frame_rev,
        alive: guest.alive.clone(),
        content: guest.render(),
    })
}

#[test]
fn chat_native_overlays_are_visible_and_route_menu_and_emoji_presses() {
    let mut renderer = crate::frame_probe::headless_renderer();
    for (action, label, expected) in [
        ("more", "Add reaction", "message_reactions"),
        ("reactions", "🦆", "reaction_submit"),
    ] {
        let mounted = seated(action);
        // The guest already emitted the menu; exposing its native overlay is
        // the missing host behavior, not a props or wire-tree setup failure.
        let mut content = view(&mounted);
        let mut tree = Tree::new(content.as_widget());
        let node = content.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(1200.0, 800.0)),
        );
        assert!(
            content
                .as_widget_mut()
                .overlay(
                    &mut tree,
                    Layout::new(&node),
                    &renderer,
                    &Rectangle::with_size(Size::new(1200.0, 800.0)),
                    Vector::ZERO
                )
                .is_some(),
            "an open Chat menu must expose its native overlay"
        );
        let mut ui = UserInterface::build(
            view(&mounted),
            Size::new(1200.0, 800.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let point = bounds(&mut ui, &renderer, label)
            .expect("the menu label is laid out")
            .center();
        ui.draw(
            &mut renderer,
            &iced::Theme::Light,
            &renderer::Style {
                text_color: iced::Color::BLACK,
            },
            mouse::Cursor::Unavailable,
        );
        let rgba = renderer.screenshot(Size::new(1200, 800), 1.0, iced::Color::WHITE);
        let directory =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/chat-input-evidence");
        std::fs::create_dir_all(&directory).unwrap();
        image::RgbaImage::from_raw(1200, 800, rgba)
            .unwrap()
            .save(directory.join(format!("{action}.png")))
            .unwrap();
        let mut messages = Vec::new();
        ui.update(
            &[
                Event::Mouse(mouse::Event::CursorMoved { position: point }),
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                Event::Window(window::Event::RedrawRequested(iced::time::Instant::now())),
            ],
            mouse::Cursor::Available(point),
            &mut renderer,
            &mut clipboard::Null,
            &mut messages,
        );
        assert!(
            messages.iter().any(|event| event.kind == expected),
            "{messages:?}"
        );
    }
}

#[test]
fn a_retained_overlay_cannot_send_a_press_to_a_replacement_instance() {
    let renderer = crate::frame_probe::headless_renderer();
    let mounted = seated("more");
    let mut content = view(&mounted);
    let mut tree = Tree::new(content.as_widget());
    let size = Size::new(1200.0, 800.0);
    let node = content.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, size),
    );
    // Float exposes its text operation through the base widget, while its
    // overlay owns drawing and input. Locate the label before retaining it.
    let mut label = TextBounds {
        text: "Add reaction",
        found: None,
    };
    content
        .as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &renderer, &mut label);
    let point = label.found.expect("the retained menu is laid out").center();
    let mut overlay = content
        .as_widget_mut()
        .overlay(
            &mut tree,
            Layout::new(&node),
            &renderer,
            &Rectangle::with_size(size),
            Vector::ZERO,
        )
        .unwrap();
    let overlay_node = overlay.as_overlay_mut().layout(&renderer, size);
    let layout = Layout::new(&overlay_node);
    {
        let mut locked = mounted.lock().unwrap();
        let Slot::Ready(guest) = &mut locked.slot else {
            unreachable!()
        };
        assert!(guest.pending.is_empty());
        guest.alive = Arc::new(());
    }
    let mut events = Vec::new();
    let mut shell = Shell::new(&mut events);
    for event in [
        mouse::Event::ButtonPressed(mouse::Button::Left),
        mouse::Event::ButtonReleased(mouse::Button::Left),
    ] {
        overlay.as_overlay_mut().update(
            &Event::Mouse(event),
            layout,
            mouse::Cursor::Available(point),
            &renderer,
            &mut clipboard::Null,
            &mut shell,
        );
    }
    let locked = mounted.lock().unwrap();
    let Slot::Ready(guest) = &locked.slot else {
        unreachable!()
    };
    assert!(
        guest.pending.is_empty(),
        "an old overlay delivered an event into its replacement"
    );
}

#[test]
fn a_native_pointer_drag_resizes_the_thread_and_release_ends_it() {
    let mut renderer = crate::frame_probe::headless_renderer();
    let mounted = seated("toolbar");
    {
        let mut locked = mounted.lock().unwrap();
        let mut props: serde_json::Value =
            serde_json::from_slice(locked.props.as_ref().unwrap()).unwrap();
        props["active_thread_seq"] = 1.into();
        props["selected_message_seq"] = 0.into();
        props["thread_messages"] = props["messages"].clone();
        let props = Some(serde_json::to_vec(&props).unwrap());
        let Slot::Ready(guest) = &mut locked.slot else {
            unreachable!()
        };
        guest.redraw(&props);
        locked.props = props;
    }
    let width = || {
        fn find(node: &wire::Node) -> Option<f32> {
            if let wire::Node::Container {
                key,
                width: Some(wire::Length::Fixed(width)),
                ..
            } = node
                && key.ends_with("/thread-pane")
            {
                return Some(*width);
            }
            node.children().iter().find_map(|node| find(node))
        }
        let locked = mounted.lock().unwrap();
        let Slot::Ready(guest) = &locked.slot else {
            unreachable!()
        };
        find(guest.frame.root.as_ref().unwrap()).unwrap()
    };
    let mut ui = UserInterface::build(
        view(&mounted),
        Size::new(1200.0, 800.0),
        user_interface::Cache::default(),
        &mut renderer,
    );
    assert_eq!(width(), 330.0);
    let mut messages = Vec::new();
    let mut send = |ui: &mut Ui, position: iced::Point, events: Vec<Event>| {
        ui.update(
            &events,
            mouse::Cursor::Available(position),
            &mut renderer,
            &mut clipboard::Null,
            &mut messages,
        );
        ui.update(
            &[Event::Window(window::Event::RedrawRequested(
                iced::time::Instant::now(),
            ))],
            mouse::Cursor::Available(position),
            &mut renderer,
            &mut clipboard::Null,
            &mut messages,
        );
    };
    // The pane occupies the rightmost 330px; its 6px divider is immediately before it.
    let start = iced::Point::new(867.0, 100.0);
    send(
        &mut ui,
        start,
        vec![Event::Mouse(mouse::Event::CursorMoved { position: start })],
    );
    send(
        &mut ui,
        start,
        vec![Event::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Left,
        ))],
    );
    let end = iced::Point::new(767.0, 100.0);
    send(
        &mut ui,
        end,
        vec![Event::Mouse(mouse::Event::CursorMoved { position: end })],
    );
    assert_eq!(
        width(),
        430.0,
        "native handle press and global move must reach the guest"
    );
    send(
        &mut ui,
        end,
        vec![Event::Mouse(mouse::Event::ButtonReleased(
            mouse::Button::Left,
        ))],
    );
    let later = iced::Point::new(600.0, 100.0);
    send(
        &mut ui,
        later,
        vec![Event::Mouse(mouse::Event::CursorMoved { position: later })],
    );
    assert_eq!(
        width(),
        430.0,
        "released drag must not follow later pointer motion"
    );
}

#[test]
fn opted_in_mouse_moves_are_local_coalesced_and_keep_button_order() {
    let mounted = seated("toolbar");
    let mut locked = mounted.lock().unwrap();
    let Slot::Ready(guest) = &mut locked.slot else {
        unreachable!()
    };
    let origin = iced::Point::new(20.0, 30.0);
    let movement = |x, y| mouse::Event::CursorMoved {
        position: iced::Point::new(x, y),
    };
    assert!(!input::mouse(guest, movement(25.0, 35.0), origin, false));
    assert!(guest.pending.is_empty());
    guest.frame.mouse_interest = true;
    assert!(input::mouse(guest, movement(25.0, 35.0), origin, false));
    assert!(input::mouse(
        guest,
        mouse::Event::ButtonPressed(mouse::Button::Left),
        origin,
        true
    ));
    assert!(input::mouse(guest, movement(60.0, 70.0), origin, false));
    assert!(input::mouse(
        guest,
        mouse::Event::ButtonReleased(mouse::Button::Left),
        origin,
        true
    ));
    assert_eq!(
        guest.pending,
        vec![
            wire::Event::Mouse {
                event: wire::mouse::Event::ButtonPressed(wire::mouse::Button::Left),
                captured: true
            },
            wire::Event::Mouse {
                event: wire::mouse::Event::CursorMoved { x: 40.0, y: 40.0 },
                captured: false
            },
            wire::Event::Mouse {
                event: wire::mouse::Event::ButtonReleased(wire::mouse::Button::Left),
                captured: true
            },
        ]
    );
}
