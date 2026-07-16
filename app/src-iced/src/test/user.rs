//! user.rs shared reducer: the `ActionFinished` arm routes a per-screen result
//! to exactly one surface. A mis-routed error would silently strand the surface
//! that raised it — the class of bug this suite exists to catch — so the Ok
//! reload chains (covered in-module) are complemented here by pinning the error
//! routing: each failure lands in its own slot and chains no reload.

use crate::screens::user::{Message, Screen, ServiceEvent, State, update};

const SCREENS: [Screen; 4] = [Screen::Home, Screen::Chat, Screen::Pages, Screen::Files];

fn slot_error(state: &State, screen: Screen) -> Option<&str> {
    match screen {
        Screen::Home => state.home.error.as_deref(),
        Screen::Chat => state.chat.error.as_deref(),
        Screen::Pages => state.pages.error.as_deref(),
        Screen::Files => state.files.error.as_deref(),
    }
}

#[test]
fn failed_actions_strand_only_their_own_screen() {
    let raised = "op rejected: Module(denied)";
    for screen in SCREENS {
        let mut state = State::default();
        let command = update(
            &mut state,
            Message::Service(ServiceEvent::ActionFinished {
                screen,
                result: Err(raised.into()),
            }),
        );
        assert!(command.is_none(), "{screen:?}: a failed action must not chain a reload");
        assert_eq!(
            slot_error(&state, screen),
            Some(raised),
            "{screen:?}: the error lands in its own slot"
        );
        for other in SCREENS {
            if other != screen {
                assert!(
                    slot_error(&state, other).is_none(),
                    "{screen:?} error leaked into {other:?}"
                );
            }
        }
    }
}

#[test]
fn home_success_clears_error_without_reloading() {
    // Home is the one screen whose successful action does not re-fetch; the
    // other three chain a reload (covered by the in-module refresh test).
    let mut state = State::default();
    state.home.error = Some("stale failure".into());
    let command = update(
        &mut state,
        Message::Service(ServiceEvent::ActionFinished {
            screen: Screen::Home,
            result: Ok(()),
        }),
    );
    assert!(command.is_none(), "a successful home action does not reload");
    assert!(state.home.error.is_none(), "success clears the home error slot");
}

// The create-and-open contract: a pending page create activates only on Ok
// (reload targets the new page); on Err it clears without a phantom.
#[test]
fn pending_page_create_activates_on_ok_and_clears_on_err() {
    use crate::screens::user::Command;

    let mut state = State::default();
    state.pages.pending_create = Some("page-1".into());
    let command = update(
        &mut state,
        Message::Service(ServiceEvent::ActionFinished {
            screen: Screen::Pages,
            result: Ok(()),
        }),
    );
    match command {
        Some(Command::LoadPages { active, open_tabs }) => {
            assert_eq!(active.as_deref(), Some("page-1"), "commit opens the created page");
            assert_eq!(open_tabs, vec!["page-1".to_string()]);
        }
        other => panic!("expected a pages reload, got {other:?}"),
    }
    assert!(state.pages.pending_create.is_none());

    let mut state = State::default();
    state.pages.pending_create = Some("page-2".into());
    let command = update(
        &mut state,
        Message::Service(ServiceEvent::ActionFinished {
            screen: Screen::Pages,
            result: Err("op rejected: Module(denied)".into()),
        }),
    );
    assert!(command.is_none(), "a failed create chains no reload");
    assert!(state.pages.pending_create.is_none(), "no phantom selection survives");
    assert!(state.pages.document().is_none(), "no phantom document");
    assert_eq!(state.pages.error.as_deref(), Some("op rejected: Module(denied)"));
}
