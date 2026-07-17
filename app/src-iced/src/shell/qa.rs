//! In-process recipe interpreter — the fast QA lane.
//!
//! Runs `qa/recipes/*.json` through `iced_test::Simulator` plus the shell's
//! own `update` loop: no display, no node, no side effects. The fleet lane
//! (`ops/iced-fleet run`) executes the same files against live binaries.
//!
//! Lane doctrine (the `lane` field): `both` recipes assert what `update()` +
//! `view()` can prove — navigation, state transitions, widget presence.
//! `fleet` recipes need a living process — subscription-driven global
//! shortcuts, multi-window lifecycle, real node/CEF effects, screenshots,
//! a11y — and are skipped here.
//!
//! In-process limits (documented, enforced by failing loudly): `update` Tasks
//! do not run (no runtime), `wait` degrades to a single `expect` evaluation
//! (nothing is asynchronous in this lane), and `press` reaches only
//! focused-widget handlers, never subscription shortcuts.

use iced_agent_plugin::selector::by;
use iced_agent_plugin::{Cond, Lane, Recipe, Step};

use super::*;

pub(super) fn run_recipe(recipe: &Recipe) -> Result<(), String> {
    if recipe.lane == Lane::Fleet {
        return Err("fleet-lane recipe cannot run in-process".into());
    }
    let (mut state, _boot) = match recipe.preset.as_deref() {
        None | Some("ui-demo") => preset::ui_demo(),
        Some("ui-operator") => preset::ui_operator(),
        Some("ui-terminal") => preset::ui_terminal(),
        Some(other) => return Err(format!("unknown preset '{other}'")),
    };
    let id = state.desktop.main.expect("preset opens a main window");

    for (index, step) in recipe.steps.iter().enumerate() {
        run_step(&mut state, id, step)
            .map_err(|error| format!("step {} failed: {error}", index + 1))?;
    }
    Ok(())
}

fn run_step(state: &mut Shell, id: window::Id, step: &Step) -> Result<(), String> {
    let mut ui = iced_test::simulator(view::view(state, id));

    match step {
        Step::Click { role, name } => {
            ui.click(by::role(*role, name.clone()))
                .map_err(|error| format!("click {role:?} \"{name}\": {error:?}"))?;
        }
        Step::Type(text) => {
            let _ = ui.typewrite(text);
        }
        Step::Press { key, mods } => {
            let _ = ui.simulate(press_events(key, mods)?);
        }
        Step::Intent(intent) => {
            let message = agent_wire::intent_message(intent.clone())
                .ok_or_else(|| format!("unsupported intent {intent:?}"))?;
            drop(ui);
            let _ = update(state, message);
            return Ok(());
        }
        Step::Expect(cond) => {
            return check(state, &mut ui, cond);
        }
        Step::Wait { cond, .. } => {
            // Nothing is asynchronous in-process: a single evaluation is the
            // whole wait.
            return check(state, &mut ui, cond);
        }
    }

    // Feed everything the interaction produced back through the real update
    // loop so the next step sees the resulting state.
    for message in ui.into_messages() {
        let _ = update(state, message);
    }
    Ok(())
}

fn check(
    state: &Shell,
    ui: &mut iced_test::Simulator<'_, Message>,
    cond: &Cond,
) -> Result<(), String> {
    match cond {
        Cond::Node { role, name, exists } => {
            let found = match (role, name) {
                (Some(role), Some(name)) => ui.find(by::role(*role, name.clone())).is_ok(),
                (Some(role), None) => ui.find(by::any(*role)).is_ok(),
                (None, Some(name)) => ui.find(name.as_str()).is_ok(),
                (None, None) => return Err("node cond needs a role or a name".into()),
            };
            if found == *exists {
                Ok(())
            } else {
                Err(format!(
                    "expected {role:?} {name:?} exists={exists}, found={found}"
                ))
            }
        }
        Cond::StatePath { path, equals } => {
            let projection = agent_wire::project_state(state);
            let actual = path
                .split('.')
                .try_fold(&projection, |value, key| value.get(key))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if actual == *equals {
                Ok(())
            } else {
                Err(format!("state.{path} = {actual}, expected {equals}"))
            }
        }
    }
}

fn press_events(
    key: &str,
    mods: &[String],
) -> Result<impl Iterator<Item = iced::Event> + use<>, String> {
    use iced::keyboard;

    let k = match key.to_lowercase().as_str() {
        "enter" => keyboard::Key::Named(keyboard::key::Named::Enter),
        "escape" => keyboard::Key::Named(keyboard::key::Named::Escape),
        "tab" => keyboard::Key::Named(keyboard::key::Named::Tab),
        "backspace" => keyboard::Key::Named(keyboard::key::Named::Backspace),
        single if single.chars().count() == 1 => keyboard::Key::Character(single.into()),
        other => return Err(format!("unsupported key '{other}'")),
    };
    let mut modifiers = keyboard::Modifiers::empty();
    for m in mods {
        match m.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= keyboard::Modifiers::CTRL,
            "shift" => modifiers |= keyboard::Modifiers::SHIFT,
            "alt" => modifiers |= keyboard::Modifiers::ALT,
            "cmd" | "logo" | "super" => modifiers |= keyboard::Modifiers::LOGO,
            other => return Err(format!("unsupported modifier '{other}'")),
        }
    }
    let pressed = iced::Event::Keyboard(keyboard::Event::KeyPressed {
        key: k.clone(),
        modified_key: k.clone(),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    });
    let released = iced::Event::Keyboard(keyboard::Event::KeyReleased {
        key: k.clone(),
        modified_key: k,
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers,
    });
    Ok([pressed, released].into_iter())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every committed recipe runs green in-process (fleet-lane recipes are
    /// parse-checked and skipped). Adding a recipe file = adding a CI test.
    #[test]
    fn qa_recipes() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../qa/recipes");
        let mut ran = 0;
        let mut skipped = 0;
        for entry in std::fs::read_dir(dir).expect("qa/recipes exists") {
            let path = entry.expect("read entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read recipe");
            let recipe = Recipe::parse(&text)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            if recipe.lane == Lane::Fleet {
                skipped += 1;
                continue;
            }
            run_recipe(&recipe)
                .unwrap_or_else(|error| panic!("recipe '{}': {error}", recipe.name));
            ran += 1;
        }
        assert!(ran >= 4, "expected the committed recipes to run, ran {ran}");
        assert!(skipped >= 1, "expected fleet-lane recipes to be skipped");
    }
}
