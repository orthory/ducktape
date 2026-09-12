//! The actual staged Chat view in native GPUI windows: no Iced renderer.
use super::*;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{self as gpui, AppContext as _, Entity, TestAppContext, VisualTestContext};

fn seated(opened: &[&str]) -> Arc<Mutex<Mounted>> {
    tests::can_the_chat_room();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/views/chat_view.wasm");
    let mut guest = Guest::load_from("chat", &path).expect("build current chat view first");
    let props = tests::chat_facts();
    guest.redraw(&None);
    settle(&mut guest, &props);
    for label in opened {
        guest
            .pending
            .push(wire::Event::Message(tests::button_message(&guest, label)));
        settle(&mut guest, &props);
    }
    assert!(guest.fault.is_none());
    let seat = Arc::new(Mutex::new(Mounted {
        slot: Slot::Ready(Box::new(guest)),
        props,
        generation: 1,
        hash: None,
        in_flight: false,
        wanted: None,
        waiting_since: None,
        replacement: Replacement::Preserve,
        retry: None,
    }));
    registry().lock().unwrap().insert("chat", seat.clone());
    seat
}
fn settle(guest: &mut Guest, props: &Option<Vec<u8>>) {
    for _ in 0..32 {
        if !guest.redraw(props) {
            return;
        }
    }
    panic!("view did not settle: {:?}", guest.fault);
}
fn open(cx: &mut TestAppContext) -> (Entity<NativeModuleView>, VisualTestContext) {
    cx.update(gpui_kit::init);
    let window = cx.open_window(gpui::size(gpui::px(1200.), gpui::px(800.)), |_, _| {
        NativeModuleView::new("chat")
    });
    let view = window.root(cx).unwrap();
    let mut native = VisualTestContext::from_window(window.into(), cx);
    native.update(|window, cx| window.render_frame(cx));
    (view, native)
}
fn button(seat: &Arc<Mutex<Mounted>>, label: &str) -> String {
    let locked = seat.lock().unwrap();
    let Slot::Ready(guest) = &locked.slot else {
        panic!("live guest")
    };
    tests::button_key(guest, label)
}
fn click_before_frame(native: &mut VisualTestContext, key: String) {
    native.update(|window, cx| {
        let position = window.find(key).bounds().center();
        window.dispatch_event(
            gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                position,
                pressed_button: None,
                modifiers: Default::default(),
            }),
            cx,
        );
        window.dispatch_event(
            gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                position,
                button: gpui::MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
                position,
                button: gpui::MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
    });
}
#[gpui_kit::test]
fn chat_native_overlays_are_visible_and_route_menu_and_emoji_presses(cx: &mut TestAppContext) {
    let _turn = tests::blocking_connection_turn();
    for (opened, label, reacted) in [
        (&["More message actions"][..], "Manage reactions", false),
        (&["Manage reactions"][..], "🦆", true),
    ] {
        let seat = seated(opened);
        let (_, mut native) = open(cx);
        let key = button(&seat, label);
        native.update(|window, _| {
            assert!(
                window.find(key.clone()).visible(),
                "native popup is visible"
            )
        });
        {
            let mut locked = seat.lock().unwrap();
            let Slot::Ready(guest) = &mut locked.slot else {
                unreachable!()
            };
            guest.frame.mouse_interest = true;
        }
        click_before_frame(&mut native, key);
        {
            let locked = seat.lock().unwrap();
            let Slot::Ready(guest) = &locked.slot else {
                unreachable!()
            };
            let routed = guest
                .pending
                .iter()
                .position(|event| matches!(event, wire::Event::Message(_)))
                .expect("popup queues route");
            let observed = guest
                .pending
                .iter()
                .position(|event| {
                    matches!(
                        event,
                        wire::Event::Mouse {
                            event: wire::mouse::Event::ButtonReleased(wire::mouse::Button::Left),
                            captured: true
                        }
                    )
                })
                .expect("captured release observed");
            assert!(routed < observed, "widget output precedes its observation");
        }
        native.update(|window, cx| window.render_frame(cx));
        let locked = seat.lock().unwrap();
        let Slot::Ready(guest) = &locked.slot else {
            unreachable!()
        };
        let texts = tests::texts(guest);
        if reacted {
            assert!(texts.windows(2).any(|pair| pair == ["🦆", "1"]));
        } else {
            assert!(texts.iter().any(|text| text == "🦆"));
        }
    }
}
#[gpui_kit::test]
fn a_retained_overlay_cannot_send_a_press_to_a_replacement_instance(cx: &mut TestAppContext) {
    let _turn = tests::blocking_connection_turn();
    let seat = seated(&["More message actions"]);
    let (view, mut native) = open(cx);
    let content = view.read_with(&native, |view, _| view.content.clone().unwrap());
    let message = {
        let mut locked = seat.lock().unwrap();
        let Slot::Ready(guest) = &mut locked.slot else {
            unreachable!()
        };
        let message = tests::button_message(guest, "Manage reactions");
        guest.pending.clear();
        guest.alive = Arc::new(());
        message
    };
    // The old native entity's real subscription remains live until the next frame.
    content.update(&mut native, |_, cx| cx.emit(wire::Event::Message(message)));
    let locked = seat.lock().unwrap();
    let Slot::Ready(guest) = &locked.slot else {
        unreachable!()
    };
    assert!(
        guest.pending.is_empty(),
        "retired overlay routed into its replacement"
    );
}
#[gpui_kit::test]
fn a_retained_control_cannot_address_a_new_frames_handler_table(cx: &mut TestAppContext) {
    let _turn = tests::blocking_connection_turn();
    let seat = seated(&["More message actions"]);
    let (view, mut native) = open(cx);
    let content = view.read_with(&native, |view, _| view.content.clone().unwrap());
    let message = {
        let mut locked = seat.lock().unwrap();
        let Slot::Ready(guest) = &mut locked.slot else {
            unreachable!()
        };
        let message = tests::button_message(guest, "Manage reactions");
        guest.pending.clear();
        guest.frame_rev += 1;
        message
    };
    content.update(&mut native, |_, cx| cx.emit(wire::Event::Message(message)));
    let locked = seat.lock().unwrap();
    let Slot::Ready(guest) = &locked.slot else {
        unreachable!()
    };
    assert!(
        guest.pending.is_empty(),
        "old frame's handler index reached a new table"
    );
}
fn thread_width(seat: &Arc<Mutex<Mounted>>) -> f32 {
    let locked = seat.lock().unwrap();
    let Slot::Ready(guest) = &locked.slot else {
        unreachable!()
    };
    let mut root = guest.frame.root.clone().unwrap();
    let mut width = None;
    root.for_each_mut(&mut |node| {
        if let wire::Node::Container {
            key,
            width: Some(wire::Length::Fixed(value)),
            ..
        } = node
            && key.ends_with("/thread-pane")
        {
            width = Some(*value);
        }
    });
    width.expect("thread pane")
}
#[gpui_kit::test]
fn a_native_pointer_drag_resizes_the_thread_and_release_ends_it(cx: &mut TestAppContext) {
    let _turn = tests::blocking_connection_turn();
    let seat = seated(&["Open thread"]);
    let (view, mut native) = open(cx);
    let key = {
        let locked = seat.lock().unwrap();
        let Slot::Ready(guest) = &locked.slot else {
            unreachable!()
        };
        let mut root = guest.frame.root.clone().unwrap();
        let mut key = None;
        root.for_each_mut(&mut |node| {
            if node
                .key()
                .is_some_and(|key| key.ends_with("/thread-divider"))
            {
                key = node.key().map(str::to_owned);
            }
        });
        key.unwrap()
    };
    let bounds = native.update(|_, cx| {
        view.read(cx)
            .content
            .as_ref()
            .unwrap()
            .read(cx)
            .measured_bounds(&key)
            .expect("native divider bounds")
    });
    assert_eq!(thread_width(&seat), 330.);
    let start = bounds.center();
    let end = start - gpui::point(gpui::px(100.), gpui::px(0.));
    native.update(|window, cx| window.drag(start, end, cx));
    assert_eq!(thread_width(&seat), 430.);
    native.simulate_mouse_move(start, None, Default::default());
    native.update(|window, cx| window.render_frame(cx));
    assert_eq!(thread_width(&seat), 430., "release ends the grab");
}
#[test]
fn opted_in_mouse_moves_are_local_coalesced_and_keep_button_order() {
    let _turn = tests::blocking_connection_turn();
    let seat = seated(&[]);
    let mut locked = seat.lock().unwrap();
    let Slot::Ready(guest) = &mut locked.slot else {
        unreachable!()
    };
    let movement = |x, y| wire::mouse::Event::CursorMoved { x, y };
    assert!(!input::mouse(guest, movement(5., 5.), false));
    guest.frame.mouse_interest = true;
    input::mouse(guest, movement(5., 5.), false);
    input::mouse(
        guest,
        wire::mouse::Event::ButtonPressed(wire::mouse::Button::Left),
        true,
    );
    input::mouse(guest, movement(35., 35.), false);
    input::mouse(guest, movement(40., 40.), false);
    input::mouse(
        guest,
        wire::mouse::Event::ButtonReleased(wire::mouse::Button::Left),
        true,
    );
    assert_eq!(
        guest.pending,
        vec![
            wire::Event::Mouse {
                event: movement(5., 5.),
                captured: false
            },
            wire::Event::Mouse {
                event: wire::mouse::Event::ButtonPressed(wire::mouse::Button::Left),
                captured: true
            },
            wire::Event::Mouse {
                event: movement(40., 40.),
                captured: false
            },
            wire::Event::Mouse {
                event: wire::mouse::Event::ButtonReleased(wire::mouse::Button::Left),
                captured: true
            },
        ]
    );
    assert!(!input::mouse(guest, movement(f32::NAN, 0.), false));
}
#[test]
fn ime_observations_keep_unicode_selection_and_commit_order() {
    use wire::events::{Event as E, InputMethod as I};
    let mut previous = None;
    let events = input::ime_events(&mut previous, "a🦆한", Some(1..4), 8, 5..8);
    assert_eq!(
        events,
        vec![
            wire::Event::Observation {
                event: E::InputMethod(I::Opened),
                captured: true
            },
            wire::Event::Observation {
                event: E::InputMethod(I::Preedit {
                    content: "🦆한".into(),
                    selection: Some((4, 7))
                }),
                captured: true
            },
        ]
    );
    let events = input::ime_events(&mut previous, "a🦆한", None, 8, 8..8);
    assert_eq!(
        events,
        vec![
            wire::Event::Observation {
                event: E::InputMethod(I::Commit("🦆한".into())),
                captured: true
            },
            wire::Event::Observation {
                event: E::InputMethod(I::Closed),
                captured: true
            },
        ]
    );
}
