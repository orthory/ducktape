//! Native window lifetime, command routing and authentication cancellation.
use super::*;

#[test]
fn os_duck_urls_are_received_before_launch_and_wait_for_network_identity() {
    let shell = rust_tokens(include_str!("../shell.rs"));
    let registered = shell
        .find("application.on_open_urls(")
        .expect("OS URL handler");
    let launched = shell.find("application.run(").expect("native launch");
    assert!(
        registered < launched,
        "initial OS URL cannot race registration"
    );
    assert!(shell.contains("url_sender.unbounded_send(urls)"));
    assert!(shell.contains("desktop.pending_urls.extend("));
    assert!(shell.contains("url.starts_with(\"duck://\")"));
    assert!(shell.contains("self.state.connected&&self.state.console_win.is_some()&&!self.state.network_chain_id.is_empty()"));
    assert!(shell.contains("std::mem::take(&mutself.pending_urls)"));
    assert!(shell.contains("self.dispatch(Message::OpenMessageLink(url),cx)"));
    let route = handler_body("OpenMessageLink");
    assert!(route.contains("resolve_duck_link("));
    assert!(route.contains("DuckKind::ForeignNetwork"));
}
#[test]
fn the_last_close_leaves_exactly_where_there_is_no_status_item() {
    use crate::backend::last_window_closed_exits;
    let some = Some(crate::shell::WindowKey::unique());
    assert!(
        !last_window_closed_exits(some, None),
        "a close with the console still up must never leave"
    );
    assert!(
        !last_window_closed_exits(None, some),
        "a close with the launch window still up must never leave"
    );
    let no_status_item = !cfg!(target_os = "macos");
    assert_eq!(
        last_window_closed_exits(None, None),
        no_status_item,
        "the last close leaves exactly where no status item can hold the daemon"
    );
}
#[test]
fn leaving_settings_clears_authentication_but_reselecting_keeps_it() {
    let (mut app, _) = Ducktape::__boot();
    app.shell_tab = ShellTab::Settings;
    app.account_busy = true;
    app.account_ceremony_phase = "working".into();
    app.account_ceremony_detail = "Continue in the browser…".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Settings));
    assert!(app.account_busy);
    assert_eq!(app.account_ceremony_phase, "working");
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Chat));
    assert!(!app.account_busy);
    assert!(app.account_ceremony_phase.is_empty());
    assert!(app.account_ceremony_detail.is_empty());
}
#[test]
fn reselecting_settings_without_authentication_still_refreshes() {
    let (mut app, _) = Ducktape::__boot();
    app.shell_tab = ShellTab::Settings;
    app.connected = true;
    app.settings_generation = 10;
    app.error = "old error".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Settings));
    assert_eq!(app.settings_generation, 11);
    assert!(app.error.is_empty());
}
#[test]
fn closing_a_window_cancels_only_its_own_authentication() {
    let (mut app, _) = Ducktape::__boot();
    let launch = crate::shell::WindowKey::unique();
    let console = crate::shell::WindowKey::unique();
    let huddle = crate::shell::WindowKey::unique();
    app.onboarding_win = Some(launch);
    app.console_win = Some(console);
    app.huddle_win = Some(huddle);
    app.hub_step = crate::HubStep::Account;
    app.mutation_phase = crate::MutationPhase::Onboarding;
    app.ceremony_phase = "working".into();
    app.account_busy = true;
    app.account_ceremony_phase = "working".into();

    let _ = app.__update(__DucktapeMessage::WindowWasClosed(huddle));
    assert_eq!(app.ceremony_phase, "working");
    assert!(app.account_busy);
    let _ = app.__update(__DucktapeMessage::WindowWasClosed(launch));
    assert!(app.ceremony_phase.is_empty());
    assert!(matches!(app.mutation_phase, crate::MutationPhase::Idle));
    assert!(
        app.account_busy,
        "the console still owns its authentication"
    );
    let _ = app.__update(__DucktapeMessage::WindowWasClosed(console));
    assert!(!app.account_busy);
    assert!(app.account_ceremony_phase.is_empty());
}
#[test]
fn returning_to_the_picker_cancels_welcome_authentication() {
    let (mut app, _) = Ducktape::__boot();
    app.hub_step = crate::HubStep::Account;
    app.mutation_phase = crate::MutationPhase::Onboarding;
    app.ceremony_phase = "working".into();
    app.ceremony_detail = "Continue in the browser…".into();
    let _ = app.__update(__DucktapeMessage::GoNetworks);
    assert!(matches!(app.hub_step, crate::HubStep::Networks));
    assert!(matches!(app.mutation_phase, crate::MutationPhase::Idle));
    assert!(app.ceremony_phase.is_empty());
    assert!(app.ceremony_detail.is_empty());
}
#[test]
fn closing_a_window_exits_only_where_no_status_item_lives() {
    let mut app = Ducktape::__state();
    let launch = crate::shell::WindowKey::unique();
    let console = crate::shell::WindowKey::unique();
    app.onboarding_win = Some(launch);
    app.console_win = Some(console);
    let _ = app.__update(__DucktapeMessage::WindowWasClosed(launch));
    assert_eq!(app.onboarding_win, None);
    assert_eq!(app.console_win, Some(console));
    let route = handler_body("WindowWasClosed");
    assert!(route.contains("letleaving=crate::backend::last_window_closed_exits("));
    use quote::ToTokens as _;
    use syn::visit::Visit as _;
    struct Exits(usize);
    impl<'ast> syn::visit::Visit<'ast> for Exits {
        fn visit_arm(&mut self, arm: &'ast syn::Arm) {
            let selected = matches!(&arm.pat, syn::Pat::TupleStruct(pattern) if pattern.path.segments.last().is_some_and(|part| part.ident == "WindowWasClosed"));
            let app_message = arm
                .pat
                .to_token_stream()
                .to_string()
                .contains("__DucktapeMessage");
            if selected || !app_message {
                self.visit_expr(&arm.body);
            }
        }
        fn visit_block(&mut self, block: &'ast syn::Block) {
            for (index, statement) in block.stmts.iter().enumerate() {
                let syn::Stmt::Expr(syn::Expr::Return(returned), _) = statement else {
                    continue;
                };
                let Some(syn::Expr::Call(call)) = returned.expr.as_deref() else {
                    continue;
                };
                let syn::Expr::Path(path) = call.func.as_ref() else {
                    continue;
                };
                if !path
                    .path
                    .segments
                    .last()
                    .is_some_and(|part| part.ident == "quit")
                {
                    continue;
                }
                let Some(syn::Stmt::Expr(syn::Expr::If(guard), _)) = index
                    .checked_sub(1)
                    .and_then(|index| block.stmts.get(index))
                else {
                    panic!("every close exit needs its immediate non-leaving guard");
                };
                let condition = guard.cond.to_token_stream().to_string().replace(' ', "");
                assert_eq!(condition.trim_matches(['(', ')']), "!leaving");
                let refusal = guard
                    .then_branch
                    .to_token_stream()
                    .to_string()
                    .replace(' ', "");
                assert_eq!(refusal, "{return::ducktape_view_guest::Task::none();}");
                self.0 += 1;
            }
            syn::visit::visit_block(self, block);
        }
    }
    let exits = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let mut exits = Exits(0);
            exits.visit_file(
                &syn::parse_file(include_str!("../ui/app_update.rs"))
                    .expect("native window close route"),
            );
            exits.0
        })
        .unwrap()
        .join()
        .unwrap();
    assert!(exits > 0);
}
#[test]
fn only_the_tray_row_and_the_quit_chord_leave() {
    let leaving: Vec<_> = handler_bodies()
        .into_iter()
        .filter(|(_, body)| body.contains("shell::quit"))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        leaving,
        ["WindowWasClosed", "TrayQuit", "CommandChordPressed"]
    );
}
#[test]
fn the_tray_open_row_branches_once_on_a_discriminant() {
    use crate::TrayOpen;
    assert_eq!(backend::tray_open_action(false, false), TrayOpen::Launch);
    assert_eq!(backend::tray_open_action(true, false), TrayOpen::Console);
    assert_eq!(backend::tray_open_action(false, true), TrayOpen::Raise);
    assert_eq!(backend::tray_open_action(true, true), TrayOpen::Raise);
    let route = handler_body("TrayOpen");
    for action in ["TrayOpen::Launch", "TrayOpen::Console", "TrayOpen::Raise"] {
        assert!(route.contains(action));
    }
}
#[test]
fn the_quit_route_is_armed_by_the_modifier_stream() {
    let mut app = Ducktape::__state();
    let mods = command_chord("q").modifiers;
    let _ = app.__update(__DucktapeMessage::ModifierStateChanged(mods));
    assert!(app.cmd_held);
    let _ = app.__update(__DucktapeMessage::ModifierStateChanged(Default::default()));
    assert!(!app.cmd_held);
    let route = handler_body("ModifierStateChanged");
    assert!(!route.contains("Task::perform"));
    assert!(!route.contains("Task::run"));
}
#[test]
fn the_command_chords_are_classified_in_one_extern() {
    let mods = command_chord("q").modifiers;
    assert_eq!(
        backend::command_chord("q".into(), mods),
        crate::CommandChord::Quit
    );
    assert_eq!(
        backend::command_chord("w".into(), mods),
        crate::CommandChord::CloseWindow
    );
    assert_eq!(
        backend::command_chord("q".into(), Default::default()),
        crate::CommandChord::Ignored
    );
    let route = handler_body("CommandChordPressed");
    assert_eq!(route.matches("backend::command_chord(").count(), 1);
    assert!(route.contains("window_target(self.focused_win"));
}
#[test]
fn a_dropped_file_starts_a_files_upload_only_on_the_files_tab() {
    let mut app = Ducktape::__state();
    app.connected = true;
    app.shell_tab = ShellTab::Pages;
    let _ = app.__update(__DucktapeMessage::FsFileDropped("/tmp/notes.md".into()));
    assert!(!app.fs_dropping);
    app.shell_tab = ShellTab::Files;
    app.fs_drop_dir = "/shared/reports".into();
    let _ = app.__update(__DucktapeMessage::FsFileDropped("/tmp/notes.md".into()));
    assert!(app.fs_dropping);
    assert!(handler_body("FsFileDropped").contains("self.fs_drop_dir"));
}
#[test]
fn authentication_operations_always_have_replace_lanes() {
    for operation in ["register_passkey(", "login_with_passkey(", "link_wallet("] {
        let routes: Vec<_> = handler_bodies()
            .into_iter()
            .filter(|(_, body)| body.contains(operation))
            .collect();
        assert!(
            !routes.is_empty(),
            "real authentication operation {operation}"
        );
        for (name, body) in routes {
            assert!(
                body.contains(".abortable()") && body.contains(".abort()"),
                "{name} replaces authentication"
            );
        }
    }
}
#[test]
fn authentication_lanes_retire_before_navigation_and_quit() {
    for (message, closes_account) in [
        (__DucktapeMessage::TrayQuit, true),
        (__DucktapeMessage::GoNetworks, false),
    ] {
        let mut app = Ducktape::__state();
        let before = (
            app.__ice_run_lane_10_generation,
            app.__ice_run_lane_11_generation,
            app.__ice_run_lane_24_generation,
            app.__ice_run_lane_25_generation,
        );
        let _ = app.__update(message);
        // GoNetworks belongs to the onboarding window. It must not cancel
        // the independently open console's account authentication lanes.
        assert_eq!(app.__ice_run_lane_10_generation > before.0, closes_account);
        assert_eq!(app.__ice_run_lane_11_generation > before.1, closes_account);
        assert!(app.__ice_run_lane_24_generation > before.2);
        assert!(app.__ice_run_lane_25_generation > before.3);
    }
}
#[test]
fn phone_and_desktop_account_authentication_retire_together() {
    for (name, body) in handler_bodies() {
        if body.contains("self.__ice_run_lane_10_handle.take()") {
            assert!(
                body.contains("self.__ice_run_lane_11_handle.take()"),
                "{name}"
            );
        }
    }
}
#[test]
fn browser_authentication_keeps_a_visible_cancel_action() {
    let source = rust_tokens(include_str!("../../../crates/views/settings/src/lib.rs"));
    assert!(source.contains("plate-cancel-working"));
    assert!(source.contains("crate::host::cancel_ceremony()"));
    let host = rust_tokens(include_str!("../../../crates/views/settings/src/host.rs"));
    assert!(host.contains("notify(\"settings.ceremony_cancel\",&())"));
}
#[test]
fn passkey_login_shows_its_cancellation_plate_without_an_account() {
    let mut app = Ducktape::__state();
    app.shell_tab = ShellTab::Settings;
    app.account_exists = false;
    app.account_ceremony_phase = "working".into();
    let (view, _) = app.native_view();
    let props: serde_json::Value = serde_json::from_slice(&view.props).unwrap();
    assert_eq!(props["account_exists"], false);
    assert_eq!(props["account_ceremony_phase"], "working");
}
