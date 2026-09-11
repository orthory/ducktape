use super::*;

fn row(seq: i64) -> backend::BellItem {
    backend::BellItem {
        seq,
        reason: "mention".into(),
        actor: "account:9".into(),
        kind: "added".into(),
        ..Default::default()
    }
}

#[test]
fn bell_acknowledgement_reads_only_the_admitted_watermark() {
    let (mut app, _) = Ducktape::__boot();
    app.connect_generation = 7;
    app.account_number = "4".into();
    app.bell_items = vec![row(3), row(2), row(1)];
    app.bell_unread = 3;
    app.bell_marking = true;
    let _ = app.__update(__DucktapeMessage::BellMarked(
        7,
        "4".into(),
        backend::BellDelta {
            kind: "read".into(),
            up_to_seq: 2,
            ..Default::default()
        },
    ));
    assert!(!app.bell_marking);
    assert_eq!(app.bell_unread, 1);
    assert!(!app.bell_items[0].read);
    assert!(app.bell_items[1].read);
    let _ = app.__update(__DucktapeMessage::BellLoaded(
        7,
        "4".into(),
        backend::BellData {
            unread: 2,
            items: vec![row(2), row(1)],
            presentations: vec![],
        },
    ));
    assert_eq!(app.bell_items[0].seq, 3);
    assert_eq!(app.bell_unread, 1);
}

#[test]
fn bell_failed_read_is_visible_and_stale_accounts_cannot_finish_it() {
    let (mut app, _) = Ducktape::__boot();
    app.connect_generation = 7;
    app.account_number = "4".into();
    app.bell_items = vec![row(1)];
    app.bell_unread = 1;
    app.bell_marking = true;
    let delta = backend::BellDelta {
        kind: "read".into(),
        up_to_seq: 1,
        ..Default::default()
    };
    let _ = app.__update(__DucktapeMessage::BellMarked(6, "4".into(), delta.clone()));
    let _ = app.__update(__DucktapeMessage::BellMarked(7, "5".into(), delta));
    assert!(app.bell_marking);
    assert_eq!(app.bell_unread, 1);
    let _ = app.__update(__DucktapeMessage::BellMarkFailed(
        7,
        "4".into(),
        backend::AppError {
            message: "Cannot reach the node".into(),
            committed: false,
        },
    ));
    assert!(!app.bell_marking);
    assert_eq!(app.bell_error, "Cannot reach the node");
    assert_eq!(app.bell_unread, 1);
    assert!(!app.bell_items[0].read);
}

#[test]
fn bell_page_navigation_cannot_outlive_its_connection_or_account() {
    use iced_test::futures::futures::StreamExt as _;
    for change in ["none", "connection", "account"] {
        let (mut app, _) = Ducktape::__boot();
        app.connect_generation = 7;
        app.account_generation = 1;
        app.account_number = "4".into();
        app.shell_tab = ShellTab::Chat;
        app.loading = false;
        let task = app.__update(__DucktapeMessage::BellOpenItem(
            7,
            "4".into(),
            backend::BellPresentation {
                target: BellTarget::Page,
                object: "page-a".into(),
                anchor: "block-a".into(),
                ..Default::default()
            },
        ));
        let stream =
            iced_test::runtime::task::into_stream(task).expect("page click schedules navigation");
        let queued = iced_test::futures::futures::executor::block_on(
            stream
                .filter_map(|action| async move {
                    match action {
                        iced_test::runtime::Action::Output(message) => Some(message),
                        _ => None,
                    }
                })
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            queued.len(),
            1,
            "hold the real completed echo, not an invented message"
        );
        match change {
            "connection" => {
                let _ = app.__update(__DucktapeMessage::ConnectFailed(backend::HydrationError {
                    generation: 7,
                    message: "disconnected".into(),
                }));
            }
            "account" => {
                let _ = app.__update(__DucktapeMessage::AccountLoaded(backend::AccountData {
                    generation: 1,
                    exists: true,
                    number: "5".into(),
                    name: "Bob".into(),
                    bio: String::new(),
                    keys: 1,
                    key_rows: vec![],
                }));
            }
            _ => {}
        }
        for message in queued {
            let _ = app.__update(message);
        }
        assert_eq!(
            app.shell_tab == ShellTab::Pages,
            change == "none",
            "queued page navigation after {change}"
        );
    }
}

#[test]
fn bell_controls_render_context_and_admit_read_from_the_real_button() {
    use ui_lang_runtime::testing::{Config, Driver, Location, MouseButton};
    let _guard = crate::module_view::tests::blocking_connection_turn();
    fn boot() -> (Ducktape, iced::Task<__DucktapeMessage>) {
        let (mut app, _) = Ducktape::__boot();
        app.console_win = Some(iced::window::Id::unique());
        app.account_number = "4".into();
        app.bell_open = true;
        app.bell_unread = 1;
        app.bell_items = vec![row(17)];
        app.bell_presentations = vec![backend::BellPresentation {
            seq: 17,
            title: "Alice mentioned you".into(),
            detail: "Please review the launch checklist.".into(),
            target: BellTarget::Message,
            object: "general".into(),
            number: 42,
            ..Default::default()
        }];
        (app, iced::Task::none())
    }
    fn update(app: &mut Ducktape, message: __DucktapeMessage) -> iced::Task<__DucktapeMessage> {
        // Exercise the real route and admission; the backend HTTP tests own IO.
        if let __DucktapeMessage::BellOpenItem(_, _, context) = &message {
            assert_eq!(context.target, BellTarget::Message);
            assert_eq!(context.object, "general");
            assert_eq!(
                context.number, 42,
                "the row routes to the source message, not inbox sequence 17"
            );
        }
        let _ = app.__update(message);
        iced::Task::none()
    }
    fn view(app: &Ducktape, _: iced::window::Id) -> iced::Element<'_, __DucktapeMessage> {
        app.__view(app.console_win.expect("the test console"))
    }
    let location = Location::new(file!(), line!() as usize, 1, "notification controls");
    let program = iced::daemon(boot, update, view);
    let mut driver = Driver::new(
        program,
        Config::new("bell_controls").viewport(1120.0, 720.0),
    );
    driver.check_text("Alice mentioned you", None, false, location);
    driver.check_text("Please review the launch checklist.", None, false, location);
    let capture = driver.capture("notification_context", location);
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(capture.metadata_path).unwrap()).unwrap();
    let targets = metadata["targets"]
        .as_array()
        .expect("captured native target tree");
    let mark = targets
        .iter()
        .find_map(|target| {
            target["id"]
                .as_str()
                .filter(|id| id.ends_with("/mark-bell-read"))
        })
        .expect("real mark-read control");
    driver.click_with(mark, MouseButton::Left, 1, location);
    assert!(
        driver.state().bell_marking,
        "the real button admits the read operation"
    );
    driver.dispatch(
        __DucktapeMessage::BellMarked(
            0,
            "4".into(),
            backend::BellDelta {
                kind: "read".into(),
                up_to_seq: 17,
                ..Default::default()
            },
        ),
        location,
    );
    assert_eq!(driver.state().bell_unread, 0);
    assert!(driver.state().bell_items[0].read);
    let row = targets
        .iter()
        .find_map(|target| {
            target["id"]
                .as_str()
                .filter(|id| id.ends_with("/open-notification"))
        })
        .expect("real notification row");
    driver.click_with(row, MouseButton::Left, 1, location);
    assert!(!driver.state().bell_open, "the row click opens its source");
}
