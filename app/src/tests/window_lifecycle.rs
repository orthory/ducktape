//! THE DAEMON'S LIFE, AND THE WAYS OUT OF IT. Closing a window is not
//! quitting where a status item can hold the daemon (macOS): there the process
//! leaves only when someone says so, through the tray's Quit or ⌘Q. Off macOS
//! there is no status item, and the last close leaves. Every shape below is one a later edit can undo without failing a
//! build, and the scenarios in `ui/tests/app.ice` cannot cover them — the test
//! harness swallows `runtime::Action::Exit` (`ui-lang-runtime/src/testing.rs`),
//! so "it exited" and "it did NOT exit" are both unassertable there. What a
//! scenario CAN see (the close unregisters, the menu reopens, the chord's
//! arming) it does see; this file pins the rest.

use super::{__DucktapeMessage, Ducktape, ShellTab, ice_handlers};

const LIFECYCLE: &str = include_str!("../ui/handlers/lifecycle.ice");
const CORE_STATE: &str = include_str!("../ui/state/core.ice");
const EXTERNS: &str = include_str!("../ui/extern/backend.ice");

fn handler(name: &str) -> String {
    ice_handlers(LIFECYCLE)
        .into_iter()
        .find(|(handler, _)| handler == name)
        .map(|(_, body)| body)
        .unwrap_or_else(|| panic!("lifecycle.ice routes `{name}`"))
}

/// A CLOSED WINDOW ENDS NOTHING BY ITSELF. The handler unregisters the slot
/// the window held and then asks ONE decide-fn whether this close leaves —
/// true only off macOS, where no status item exists, once no window remains.
/// A bare `exit` back in here, or a survivor guard spelled inline, turns the
/// red button into a quit on a Mac again — the exact regression this guards,
/// and the one a scenario cannot catch because the harness ignores the exit
/// it would have to observe.
#[test]
fn closing_a_window_exits_only_where_no_status_item_lives() {
    let body = handler("window_was_closed");
    for slot in ["onboarding_win", "console_win", "huddle_win"] {
        assert!(
            body.contains(&format!("{slot} = without_window({slot}, id)")),
            "window_was_closed stopped unregistering {slot}"
        );
    }
    assert!(
        body.contains("let leaving = last_window_closed_exits(console_win, onboarding_win)"),
        "window_was_closed stopped asking the decide-fn: {body}"
    );
    assert!(
        body.contains("return if !leaving"),
        "the exit is no longer gated on the decide-fn: {body}"
    );
    assert!(
        !body.contains("!= none"),
        "the survivor guard is spelled inline again: {body}"
    );
    assert!(
        EXTERNS.contains(
            "pure last_window_closed_exits(console:window-id?, onboarding:window-id?) -> bool"
        ),
        "backend.ice lost the close-exits discriminant"
    );
}

/// OFF MACOS THE LAST CLOSE LEAVES, ON A MAC IT NEVER DOES. The decide-fn is
/// the platform rule in one place; this pins both halves on the platform the
/// test runs on.
#[test]
fn the_last_close_leaves_exactly_where_there_is_no_status_item() {
    use crate::backend::last_window_closed_exits;
    let some = Some(iced::window::Id::unique());
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

/// QUIT IS SAID OUT LOUD, IN EXACTLY THREE PLACES. The tray's row, the ⌘Q
/// chord and the last close off macOS are the only handlers that may leave; a
/// fourth one is a way out nobody asked for.
#[test]
fn only_the_tray_row_and_the_quit_chord_leave() {
    let leaving: Vec<String> = ice_handlers(LIFECYCLE)
        .into_iter()
        .filter(|(_, body)| body.contains("exit"))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        leaving,
        vec![
            "window_was_closed".to_owned(),
            "tray_quit".to_owned(),
            "command_chord_pressed".to_owned(),
        ],
        "the set of handlers that exit changed"
    );
}

/// "OPEN" OPENS WHEN THERE IS NOTHING TO RAISE, and it decides that ONCE, on a
/// discriminant — three-valued (#1782), since a connected network with
/// nothing tracked must reopen the CONSOLE, never the launch window (which
/// resets `hub_step` to the picker). `window_target` on an untracked slot
/// names a fresh id whose focus is a no-op, so a raise-only row is a dead row
/// the moment every window is closed — which the change above made an
/// ordinary state to be in.
#[test]
fn the_tray_open_row_branches_once_on_a_discriminant() {
    let body = handler("tray_open");
    assert!(
        body.contains("tray_open_action(connected, window_tracked)"),
        "tray_open stopped asking the decide-fn: {body}"
    );
    let branches = body
        .lines()
        .filter(|line| line.trim_start().starts_with("match "))
        .count();
    assert_eq!(branches, 1, "one branch, not a ladder: {body}");
    assert!(body.contains("TrayOpen.launch"), "no launch arm: {body}");
    assert!(body.contains("TrayOpen.console"), "no console arm: {body}");
    assert!(body.contains("TrayOpen.raise"), "no raise arm: {body}");
    assert!(
        body.contains("task window open onboarding"),
        "the launch arm no longer opens the onboarding window: {body}"
    );
    assert!(
        body.contains("done -> network_entered()"),
        "the console arm no longer re-enters the network through the doors' landing: {body}"
    );
    assert!(
        EXTERNS
            .contains("pure tray_open_action(network_open:bool, window_tracked:bool) -> TrayOpen"),
        "backend.ice lost the tray-open discriminant"
    );
}

/// ORDINARY TYPING PAYS NOTHING FOR ⌘Q. A key-press subscription publishes a
/// proxied message and an unconditional rebuild for every key it sees, so the
/// route that carries the chord is gated on the modifier actually being down;
/// the modifier stream, which fires only when a modifier moves, is what arms
/// it. Dropping the gate is invisible in every test except this one — the app
/// still quits on ⌘Q, it just taxes every keystroke to do it.
#[test]
fn the_quit_route_is_armed_by_the_modifier_stream() {
    assert!(
        LIFECYCLE.contains("keyboard modifiers -> modifier_state_changed _"),
        "the cheap half is gone: nothing sets cmd_held"
    );
    assert!(
        LIFECYCLE
            .contains("keyboard press status=ignored when cmd_held -> command_chord_pressed _"),
        "the quit key route lost its `when cmd_held` arming, and now taxes every keystroke"
    );
    assert!(
        CORE_STATE.contains("cmd_held = false"),
        "cmd_held is no longer plain state"
    );
    // ARMING, AND NOTHING ELSE. Both lines are the same cheap fact off the
    // same stream — ⌘ arms the quit/close chord, ⇧ is read by the chat's
    // shift-click, and a press carries no modifiers of its own. A THIRD kind
    // of statement here is the thing this guards: the stream fires on every
    // modifier transition, so work done in it is work done constantly.
    let armed = handler("modifier_state_changed");
    assert_eq!(
        armed.trim(),
        "cmd_held = command_held(mods)\n  shift_held = shift_held(mods)",
        "the arming handler does more than arm: {armed}"
    );
}

/// THE CHORD IS ONE ANSWER, IN ONE PLACE. The handler asks the extern and
/// leaves; spelling the modifier and the key here instead would put the chord
/// in two places, and the arming gate reads `command()` — so a chord judged by
/// anything else is a chord that never fires off a Mac.
#[test]
fn the_command_chords_are_classified_in_one_extern() {
    let body = handler("command_chord_pressed");
    assert!(
        body.contains("let chord = command_chord(event.key, event.physical_key, event.modifiers)"),
        "the chord handler stopped asking the extern: {body}"
    );
    assert!(body.contains("exit"), "the chord handler stopped quitting");
    assert!(
        body.contains("task window close target=window_target(focused_win)"),
        "⌘W stopped closing the FOCUSED window: {body}"
    );
    // Statement lines only: prose in this handler's own comments says "match"
    // too, and a lint a comment can satisfy is a lint that proves nothing.
    let dispatches = body
        .lines()
        .filter(|line| line.trim_start().starts_with("match "))
        .count();
    assert_eq!(dispatches, 1, "one dispatch for both chords: {body}");
    for spelled in ["logo", "command()", "Named::", "physical =="] {
        assert!(
            !body.contains(spelled),
            "the chord is spelled inside the handler as well as the extern: {spelled}"
        );
    }
    for declared in [
        "pure command_held(modifiers:key-modifiers) -> bool",
        "pure command_chord(logical:key, physical:physical-key, modifiers:key-modifiers) -> CommandChord",
    ] {
        assert!(EXTERNS.contains(declared), "backend.ice lost `{declared}`");
    }
}

/// A FILE DROPPED ON THE WINDOW IS A FILES-TAB UPLOAD. The Files screen is a
/// module-owned view that reads and writes duckfs itself, but the OS drop
/// stays a WINDOW event the daemon hears (`window file-dropped ->
/// fs_file_dropped _`): on the Files tab, connected, the handler uploads into
/// the directory the view last said it stands in; on any other tab the drop is
/// nobody's and changes nothing.
#[test]
fn a_dropped_file_starts_a_files_upload_only_on_the_files_tab() {
    assert!(
        LIFECYCLE.contains("window file-dropped -> fs_file_dropped _"),
        "lifecycle.ice stopped routing the OS drop to the files handler"
    );
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.shell_tab = ShellTab::Pages;
    let _ = app.__update(__DucktapeMessage::FsFileDropped("/tmp/notes.md".into()));
    assert!(!app.fs_dropping, "a drop off the Files tab is nobody's");

    app.shell_tab = ShellTab::Files;
    app.fs_drop_dir = "/shared/reports".into();
    let _ = app.__update(__DucktapeMessage::FsFileDropped("/tmp/notes.md".into()));
    assert!(app.error.is_empty(), "{}", app.error);
    assert!(app.fs_dropping, "the drop starts its upload");
    assert!(
        include_str!("../ui/handlers/files.ice")
            .contains("files_upload(connected_rpc, password, fs_drop_dir, path)"),
        "the drop stopped landing in the directory the view named"
    );
}

// Cancellation is an effect contract: the scenario harness cannot observe a
// dropped backend future, so pin lane ownership alongside its state scenarios.
#[test]
fn authentication_operations_always_have_replace_lanes() {
    for source in [
        include_str!("../ui/handlers/node.ice"),
        include_str!("../ui/handlers/onboarding.ice"),
    ] {
        for line in source.lines() {
            let authenticates = ["register_passkey(", "login_with_passkey(", "link_wallet("]
                .iter()
                .any(|operation| line.contains(operation));
            if authenticates {
                assert!(
                    line.trim_start().starts_with("run replace lane="),
                    "uncancellable authentication: {line}"
                );
            }
        }
    }
}

#[test]
fn authentication_lanes_retire_before_navigation_and_quit() {
    let onboarding = include_str!("../ui/handlers/onboarding.ice");
    for name in ["switch_network", "open_account_welcome", "network_entered"] {
        let body = super::ice_handler_body(onboarding, name);
        assert!(
            body.contains("invalidate lane=account_ceremony"),
            "{name} leaves account authentication alive"
        );
    }
    for name in ["tray_quit", "command_chord_pressed", "window_was_closed"] {
        let body = handler(name);
        for lane in [
            "ceremony",
            "desktop_ceremony",
            "account_ceremony",
            "account_desktop_ceremony",
        ] {
            assert!(
                body.contains(&format!("invalidate lane={lane}")),
                "{name} leaves {lane} alive"
            );
        }
    }
    let body = handler("select_shell_tab");
    let cancel = body
        .find("invalidate lane=account_ceremony")
        .expect("leaving Settings aborts authentication");
    assert!(cancel < body.find("return if !connected").unwrap());
}

#[test]
fn browser_authentication_keeps_a_visible_cancel_action() {
    let source = include_str!("../../../crates/views/settings/src/ui/kit.ice");
    let plate = source.split("component CeremonyPlate(").nth(1).unwrap();
    let working = plate.split("if phase == \"working\"").nth(1).unwrap();
    assert!(working.contains("#plate-cancel-working -> emit(account_ceremony_cancel)"));
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
    let launch = iced::window::Id::unique();
    let console = iced::window::Id::unique();
    let huddle = iced::window::Id::unique();
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
fn phone_and_desktop_account_authentication_retire_together() {
    for source in [
        include_str!("../ui/handlers/node.ice"),
        include_str!("../ui/handlers/onboarding.ice"),
        include_str!("../ui/handlers/lifecycle.ice"),
        include_str!("../ui/handlers/chat.ice"),
        include_str!("../ui/handlers/pages.ice"),
        include_str!("../ui/handlers/huddle.ice"),
    ] {
        let lines: Vec<_> = source.lines().map(str::trim).collect();
        for (index, line) in lines.iter().enumerate() {
            if *line == "invalidate lane=account_ceremony" {
                assert_eq!(
                    lines.get(index + 1),
                    Some(&"invalidate lane=account_desktop_ceremony")
                );
            }
        }
    }
}

#[test]
fn passkey_login_shows_its_cancellation_plate_without_an_account() {
    let source = include_str!("../../../crates/views/settings/src/ui/settings.ice");
    let missing_account = source.split("if !account_exists").nth(1).unwrap();
    let missing_account = missing_account.split("if account_exists").next().unwrap();
    assert!(missing_account.contains("CeremonyPlate #account-login-ceremony"));
    assert!(missing_account.contains("account_ceremony_cancel"));
}
