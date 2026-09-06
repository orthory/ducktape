//! SETTINGS IS A TAB STRIP OVER ONE MATCH, and this file is why it stays one.
//! The screen was a single `grid min-cell=420.0` of eight cards: the width
//! decided which column a group landed in, so identity, the keys that speak
//! for it and the seat that signs with them could be three columns apart, and
//! `Forget this network` was the last card of a scroll whose only way in was
//! the wheel. Every group belongs to exactly one named pane now, the strip is
//! the only way to pick one, and a new pane fails the build until it is both
//! reachable and routed.

const SETTINGS: &str = include_str!("../ui/screens/settings.ice");
const TYPES: &str = include_str!("../ui/state/types.ice");
const OVERLAYS: &str = include_str!("../ui/handlers/overlays.ice");

/// The panes, in the order the strip offers them. General first because it is
/// the one nothing has to be true for; danger last because it is the one act
/// this screen cannot take back.
const PANES: [&str; 5] = ["general", "network", "account", "security", "danger"];

/// Every authored group, and the pane that owns it. The pairing is the whole
/// of the redesign: a group in two panes is a group that drifted, and a group
/// in none is a group nobody can reach.
const GROUPS: [(&str, &str); 8] = [
    ("APPEARANCE", "general"),
    ("NOTIFICATIONS", "general"),
    ("THIS DEVICE", "general"),
    ("NETWORK", "network"),
    ("YOUR IDENTITY", "account"),
    ("ACCOUNT KEYS", "account"),
    ("IDENTITY KEY", "security"),
    ("DANGER ZONE", "danger"),
];

/// The pane each line carrying `needle` is authored under. An arm header is
/// the only line that BEGINS `SettingsPane.` — the strip spells the same
/// variant inside `pick_pane(…)` and `checked=(…)`, never at the margin — so
/// walking the file top to bottom and remembering the last one seen says which
/// arm any later line belongs to.
fn panes_holding(needle: &str) -> Vec<String> {
    let mut arm = String::new();
    let mut holding = Vec::new();
    for line in SETTINGS.lines() {
        if let Some(name) = line.trim().strip_prefix("SettingsPane.") {
            arm = name.to_owned();
        }
        if line.contains(needle) {
            holding.push(arm.clone());
        }
    }
    holding
}

/// ONE GROUP, ONE PANE. A `GroupLabel` mounted under two arms is the same
/// heading in two places; one mounted before the dispatch is a card that
/// paints on every pane, which is how the grid read in the first place.
#[test]
fn every_group_is_authored_under_exactly_one_pane() {
    for (group, pane) in GROUPS {
        // DANGER ZONE wears the one warmed eyebrow in the console, so it is a
        // bare `text`, not a `GroupLabel` — both spell the group's name.
        let holding = panes_holding(&format!("\"{group}\""));
        assert_eq!(
            holding,
            vec![pane.to_owned()],
            "the {group} group is no longer the {pane} pane's alone"
        );
    }
}

/// EVERY PANE IS REACHABLE AND ROUTED. A variant with an arm and no tab is a
/// pane no reader can open; a variant with a tab and no arm fails the DSL's
/// own exhaustiveness check, so this half only has to pin the strip.
#[test]
fn every_pane_has_a_tab_and_an_arm() {
    for pane in PANES {
        assert!(
            SETTINGS.contains(&format!(
                "button #settings-{pane}-tab -> pick_pane(SettingsPane.{pane})"
            )),
            "the {pane} pane lost its tab, so nothing opens it"
        );
        assert!(
            SETTINGS
                .lines()
                .any(|line| line.trim() == format!("SettingsPane.{pane}")),
            "the {pane} pane lost its arm"
        );
    }
}

/// THE ENUM IS THE STRIP. A sixth variant must fail here — and go on failing
/// until someone gives it both a tab and an arm — rather than quietly render
/// nothing behind a tab that is not there.
#[test]
fn the_enum_names_the_same_panes_in_the_same_order() {
    let declared: Vec<String> = TYPES
        .lines()
        .skip_while(|line| line.trim() != "enum SettingsPane")
        .skip(1)
        .take_while(|line| line.starts_with("  ") && !line.trim().is_empty())
        .map(|line| line.trim().to_owned())
        .collect();
    assert_eq!(
        declared,
        PANES.map(str::to_owned).to_vec(),
        "SettingsPane and the tab strip disagree about what Settings holds"
    );
}

/// ONE DISPATCH, NOTHING OUTSIDE IT. A second `match` — or a card hoisted
/// above the one there is — is a group that shows on every pane, which is the
/// flat list this replaced.
#[test]
fn the_screen_branches_once_and_holds_nothing_above_the_branch() {
    let dispatches = SETTINGS
        .lines()
        .filter(|line| line.trim_start().starts_with("match settings_pane"))
        .count();
    assert_eq!(dispatches, 1, "Settings branches on its pane more than once");
    let branch = SETTINGS
        .lines()
        .position(|line| line.trim_start().starts_with("match settings_pane"))
        .expect("the dispatch was just counted");
    let strays: Vec<String> = SETTINGS
        .lines()
        .take(branch)
        .enumerate()
        .filter(|(_, line)| line.contains("GroupCard") || line.contains("GroupLabel"))
        .map(|(number, line)| format!("{}: {}", number + 1, line.trim()))
        .collect();
    assert!(
        strays.is_empty(),
        "a settings group is authored above the dispatch, so it paints on \
         every pane: {strays:?}"
    );
}

/// THE PANE IS PICKED IN ONE PLACE. `pick_pane` is the screen's own handler,
/// not an emit: the pane is chrome, so it never crosses into app state and no
/// second route can set it.
#[test]
fn the_pane_moves_only_through_the_strip() {
    let writes: Vec<String> = SETTINGS
        .lines()
        .filter(|line| line.trim().starts_with("settings_pane ="))
        .map(|line| line.trim().to_owned())
        .collect();
    assert_eq!(
        writes,
        vec!["settings_pane = picked".to_owned()],
        "something other than the strip's handler writes the pane"
    );
    assert!(
        !SETTINGS.contains("emit(pick_pane"),
        "the pane left the screen as an app event; it is chrome and stays instance state"
    );
}

/// THE SCROLL PANE IS STILL THE ROOT. `overlays.ice` scrolls Settings by the
/// literal id path, so a container wrapped around the scrollable renames the
/// target and Page Down goes dead with nothing failing to say so.
#[test]
fn the_scrollable_is_the_screens_root() {
    assert!(
        SETTINGS.contains("\n  scroll #settings-body\n"),
        "the scrollable is no longer a top-level node of the screen"
    );
    assert!(
        OVERLAYS.contains("#workspace-tabs/content/settings/settings-body"),
        "the keyboard-scroll handler stopped naming the settings pane"
    );
}
