//! THE DAEMON'S LIFE, AND THE TWO WAYS OUT OF IT. Closing a window is not
//! quitting: the process leaves only when someone says so, through the tray's
//! Quit or ⌘Q. Every shape below is one a later edit can undo without failing a
//! build, and the scenarios in `ui/tests/app.ice` cannot cover them — the test
//! harness swallows `runtime::Action::Exit` (`ui-lang-runtime/src/testing.rs`),
//! so "it exited" and "it did NOT exit" are both unassertable there. What a
//! scenario CAN see (the close unregisters, the menu reopens, the chord's
//! arming) it does see; this file pins the rest.

use super::ice_handlers;

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

/// A CLOSED WINDOW ENDS NOTHING. This is the whole of the user-visible rule:
/// the handler unregisters the slot the window held and stops. An `exit` back
/// in here turns the red button into a quit again — the exact regression this
/// guards, and the one a scenario cannot catch because the harness ignores the
/// exit it would have to observe.
#[test]
fn closing_a_window_never_exits() {
    let body = handler("window_was_closed");
    for slot in ["onboarding_win", "console_win", "huddle_win"] {
        assert!(
            body.contains(&format!("{slot} = without_window({slot}, id)")),
            "window_was_closed stopped unregistering {slot}"
        );
    }
    assert!(
        !body.contains("exit"),
        "closing a window exits the process again: {body}"
    );
}

/// QUIT IS SAID OUT LOUD, IN EXACTLY TWO PLACES. The tray's row and the ⌘Q
/// chord are the only handlers that may leave; a third one is a way out nobody
/// asked for, and it would most likely be a window path.
#[test]
fn only_the_tray_row_and_the_quit_chord_leave() {
    let leaving: Vec<String> = ice_handlers(LIFECYCLE)
        .into_iter()
        .filter(|(_, body)| body.contains("exit"))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        leaving,
        vec!["tray_quit".to_owned(), "command_chord_pressed".to_owned()],
        "the set of handlers that exit changed"
    );
}

/// "OPEN" OPENS WHEN THERE IS NOTHING TO RAISE, and it decides that ONCE, on a
/// discriminant. `window_target` on an untracked slot names a fresh id whose
/// focus is a no-op, so a raise-only row is a dead row the moment every window
/// is closed — which the change above made an ordinary state to be in.
#[test]
fn the_tray_open_row_branches_once_on_a_discriminant() {
    let body = handler("tray_open");
    assert!(
        body.contains("tray_open_action(console_win, onboarding_win)"),
        "tray_open stopped asking the decide-fn: {body}"
    );
    let branches = body
        .lines()
        .filter(|line| line.trim_start().starts_with("match "))
        .count();
    assert_eq!(branches, 1, "one branch, not a ladder: {body}");
    assert!(body.contains("TrayOpen.open"), "no open arm: {body}");
    assert!(body.contains("TrayOpen.raise"), "no raise arm: {body}");
    assert!(
        body.contains("task window open onboarding"),
        "the open arm no longer opens a window: {body}"
    );
    assert!(
        EXTERNS.contains("pure tray_open_action(console:window-id?, onboarding:window-id?) -> TrayOpen"),
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
        LIFECYCLE.contains("keyboard press status=ignored when cmd_held -> command_chord_pressed _"),
        "the quit key route lost its `when cmd_held` arming, and now taxes every keystroke"
    );
    assert!(
        CORE_STATE.contains("cmd_held = false"),
        "cmd_held is no longer plain state"
    );
    let armed = handler("modifier_state_changed");
    assert_eq!(
        armed.trim(),
        "cmd_held = command_held(mods)",
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
