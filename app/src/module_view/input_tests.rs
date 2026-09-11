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

/// A chat view with its room on screen, driven by PRESSES to whatever the
/// host contract under test needs open: the selection, the menus and the
/// thread are the view's own state now, and the only door into them is the
/// one the reader uses.
fn seated(opened: &[&str]) -> Arc<Mutex<Mounted>> {
    tests::can_the_chat_room();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/views/chat_view.wasm");
    let mut guest = Guest::load_from("chat", &path).expect("build current chat view first");
    let props = tests::chat_facts();
    guest.redraw(&None);
    settle(&mut guest, &props);
    for label in opened {
        guest.deliver(Output::Activate(tests::button_message(&guest, label)));
        settle(&mut guest, &props);
    }
    assert!(guest.fault.is_none());
    Arc::new(Mutex::new(Mounted {
        slot: Slot::Ready(Box::new(guest)),
        props,
        generation: 1,
        hash: None,
        in_flight: false,
        wanted: None,
        waiting_since: None,
        replacement: Replacement::Preserve,
        retry: None,
    }))
}

/// Redraws until the view has nothing left in flight: a read answers, the
/// answer opens the next read, and the room is on screen when they stop.
fn settle(guest: &mut Guest, props: &Option<Vec<u8>>) {
    for _ in 0..32 {
        if !guest.redraw(props) {
            return;
        }
    }
    panic!("the view never settled (fault {:?})", guest.fault);
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
    // What a press on the overlay must leave behind: the menu's own entry
    // opens the next menu, and the emoji in THAT one signs the reaction. Both
    // are the view's, so the proof is its next frame — the app is only the
    // road the press takes.
    let picker_open: fn(&Guest) -> bool =
        |guest| tests::texts(guest).iter().any(|text| text == "🦆");
    // the tapped emoji is a chip with a count on the row it was tapped from,
    // drawn before the block that carries it
    let reaction_chipped: fn(&Guest) -> bool =
        |guest| tests::texts(guest).windows(2).any(|pair| pair == ["🦆", "1"]);
    for (action, opened, label, proof) in [
        (
            "more",
            &["More message actions"][..],
            "Add reaction",
            picker_open,
        ),
        ("reactions", &["Manage reactions"][..], "🦆", reaction_chipped),
    ] {
        let mounted = seated(opened);
        {
            let mut locked = mounted.lock().unwrap();
            let Slot::Ready(guest) = &mut locked.slot else {
                unreachable!()
            };
            // Exercise the host's opt-in observation contract on the actual menu.
            guest.frame.mouse_interest = true;
        }
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
            ],
            mouse::Cursor::Available(point),
            &mut renderer,
            &mut clipboard::Null,
            &mut messages,
        );
        {
            let locked = mounted.lock().unwrap();
            let Slot::Ready(guest) = &locked.slot else {
                unreachable!()
            };
            let routed = guest
                .pending
                .iter()
                .position(|event| matches!(event, wire::Event::Message(_)))
                .expect("the clicked popup queues its route");
            let observed = guest
                .pending
                .iter()
                .position(|event| {
                    matches!(
                        event,
                        wire::Event::Mouse {
                            event: wire::mouse::Event::ButtonReleased(wire::mouse::Button::Left),
                            ..
                        }
                    )
                })
                .expect("a captured release is observed when opted in");
            assert!(
                routed < observed,
                "popup route must precede its release observation"
            );
        }
        ui.update(
            &[Event::Window(window::Event::RedrawRequested(
                iced::time::Instant::now(),
            ))],
            mouse::Cursor::Available(point),
            &mut renderer,
            &mut clipboard::Null,
            &mut messages,
        );
        let locked = mounted.lock().unwrap();
        let Slot::Ready(guest) = &locked.slot else {
            unreachable!()
        };
        assert!(
            proof(guest),
            "the {action} overlay's press never landed: {:?}",
            tests::texts(guest)
        );
    }
}

#[test]
fn a_retained_overlay_cannot_send_a_press_to_a_replacement_instance() {
    let renderer = crate::frame_probe::headless_renderer();
    let mounted = seated(&["More message actions"]);
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
    // the rail opens the way the reader opens it, and reads its own thread
    let mounted = seated(&["Open thread"]);
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
            node.children().iter().find_map(find)
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
    struct DividerBounds {
        key: iced::widget::Id,
        bounds: Option<Rectangle>,
    }
    impl Operation for DividerBounds {
        fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
            visit(self);
        }
        fn container(&mut self, id: Option<&iced::widget::Id>, bounds: Rectangle) {
            if id == Some(&self.key) {
                self.bounds = Some(bounds);
            }
        }
    }
    fn divider_key(node: &wire::Node) -> Option<String> {
        if node
            .key()
            .is_some_and(|key| key.ends_with("/thread-divider"))
        {
            return node.key().map(str::to_owned);
        }
        node.children().iter().find_map(divider_key)
    }
    let key = {
        let locked = mounted.lock().unwrap();
        let Slot::Ready(guest) = &locked.slot else {
            unreachable!()
        };
        divider_key(guest.frame.root.as_ref().unwrap()).unwrap()
    };
    let mut divider = DividerBounds {
        key: iced::widget::Id::from(key),
        bounds: None,
    };
    ui.operate(&renderer, &mut divider);
    let divider = divider.bounds.expect("actual thread divider layout");
    assert_eq!(divider.width, 10.0);
    let start = iced::Point::new(divider.center_x(), divider.y + 100.0);
    let end = iced::Point::new(start.x - 100.0, start.y);
    let mut messages = Vec::new();
    {
        let mut dispatch = |ui: &mut Ui, position: iced::Point, event: Event, redraw: bool| {
            ui.update(
                &[event],
                mouse::Cursor::Available(position),
                &mut renderer,
                &mut clipboard::Null,
                &mut messages,
            );
            if redraw {
                ui.update(
                    &[Event::Window(window::Event::RedrawRequested(
                        iced::time::Instant::now(),
                    ))],
                    mouse::Cursor::Available(position),
                    &mut renderer,
                    &mut clipboard::Null,
                    &mut messages,
                );
            }
        };
        // All three native events arrive before one frame. Coalescing must retain
        // the pre-press pointer baseline, not initialize this drag from zero.
        dispatch(
            &mut ui,
            start,
            Event::Mouse(mouse::Event::CursorMoved { position: start }),
            false,
        );
        dispatch(
            &mut ui,
            start,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            false,
        );
        dispatch(
            &mut ui,
            end,
            Event::Mouse(mouse::Event::CursorMoved { position: end }),
            true,
        );
        assert_eq!(
            width(),
            430.0,
            "same-frame native border drag keeps its press baseline"
        );
        let across = iced::Point::new(start.x + 30.0, start.y);
        dispatch(
            &mut ui,
            across,
            Event::Mouse(mouse::Event::CursorMoved { position: across }),
            true,
        );
        assert_eq!(
            width(),
            300.0,
            "drag crosses back over the original divider"
        );
        let left = iced::Point::new(0.0, start.y);
        dispatch(
            &mut ui,
            left,
            Event::Mouse(mouse::Event::CursorMoved { position: left }),
            true,
        );
        assert_eq!(
            width(),
            624.0,
            "channel/sidebar and both 10px dividers retain their minimum widths"
        );
        let right = iced::Point::new(1190.0, start.y);
        dispatch(
            &mut ui,
            right,
            Event::Mouse(mouse::Event::CursorMoved { position: right }),
            true,
        );
        assert_eq!(width(), 280.0, "thread retains its minimum width");
        dispatch(
            &mut ui,
            right,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            true,
        );
        dispatch(
            &mut ui,
            left,
            Event::Mouse(mouse::Event::CursorMoved { position: left }),
            true,
        );
        assert_eq!(width(), 280.0, "release outside the divider ends the drag");
    }
    let mut ui = UserInterface::build(
        view(&mounted),
        Size::new(1200.0, 800.0),
        ui.into_cache(),
        &mut renderer,
    );
    assert!(bounds(&mut ui, &renderer, "−").is_none());
    assert!(bounds(&mut ui, &renderer, "+").is_none());
    ui.draw(
        &mut renderer,
        &iced::Theme::Light,
        &renderer::Style {
            text_color: iced::Color::BLACK,
        },
        mouse::Cursor::Unavailable,
    );
    let rgba = renderer.screenshot(Size::new(1200, 800), 1.0, iced::Color::WHITE);
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/chat-input-evidence");
    std::fs::create_dir_all(&directory).unwrap();
    image::RgbaImage::from_raw(1200, 800, rgba)
        .unwrap()
        .save(directory.join("thread-border-drag.png"))
        .unwrap();
}

#[test]
fn opted_in_mouse_moves_are_local_coalesced_and_keep_button_order() {
    let mounted = seated(&[]);
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
    assert!(input::mouse(guest, movement(55.0, 65.0), origin, false));
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
                event: wire::mouse::Event::CursorMoved { x: 5.0, y: 5.0 },
                captured: false
            },
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
