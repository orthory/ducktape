use super::*;

/// The global-key router for the command palette: platform-Command+K
/// toggles, Escape closes an open palette; anything else is `none`.
pub fn palette_key_action(
    logical: iced::keyboard::Key,
    physical: iced::keyboard::key::Physical,
    modifiers: iced::keyboard::Modifiers,
    open: bool,
) -> String {
    use iced::keyboard::{
        Key,
        key::{Code, Named, Physical},
    };
    let is_toggle = modifiers.command() && physical == Physical::Code(Code::KeyK);
    if is_toggle {
        return match open {
            true => "close".into(),
            false => "open".into(),
        };
    }
    if open && logical == Key::Named(Named::Escape) {
        return "close".into();
    }
    "none".into()
}

/// Is the command modifier down? The cheap half of the quit chord: it is read
/// off the modifier stream to ARM the key-press route, so an ordinary keystroke
/// never publishes one. It asks the same `command()` [`quit_chord`] judges by —
/// Command on a Mac, Control elsewhere — because arming on one modifier and
/// judging on another yields a chord that can never fire.
pub fn command_held(modifiers: iced::keyboard::Modifiers) -> bool {
    modifiers.command()
}

/// Is ⇧ down right now? A press carries no modifiers of its own, so the chat's
/// shift-click reads the arming this fills instead of the app growing a second
/// key route to learn the same fact.
pub fn shift_held(modifiers: iced::keyboard::Modifiers) -> bool {
    modifiers.shift()
}

/// Which command chord this press is (⌘Q / ⌘W, Ctrl off a Mac), or none. The
/// ONE answer, so the chords live in a single place rather than re-spelled at
/// each use site — macOS binds both through an app menu this app does not have,
/// so it reads them itself.
///
/// Both key readings, like the palette's toggle: the physical code is what a
/// Dvorak or AZERTY layout still calls Q or W, and the logical character is
/// what a layout that remaps the code actually types.
pub fn command_chord(
    logical: iced::keyboard::Key,
    physical: iced::keyboard::key::Physical,
    modifiers: iced::keyboard::Modifiers,
) -> crate::CommandChord {
    use iced::keyboard::key::{Code, Physical};
    if !modifiers.command() {
        return crate::CommandChord::Ignored;
    }
    let quit = physical == Physical::Code(Code::KeyQ) || types_letter(&logical, "q");
    if quit {
        return crate::CommandChord::Quit;
    }
    let close_window = physical == Physical::Code(Code::KeyW) || types_letter(&logical, "w");
    if close_window {
        return crate::CommandChord::CloseWindow;
    }
    crate::CommandChord::Ignored
}

/// The letter a press actually types, after the layout has had its say — the
/// other half of the physical code in every chord above.
fn types_letter(logical: &iced::keyboard::Key, letter: &str) -> bool {
    matches!(logical, iced::keyboard::Key::Character(typed) if typed.eq_ignore_ascii_case(letter))
}

/// The transient layer currently over the console's content — the TOPMOST one
/// only — or `""` when the content itself is the frontmost thing on screen.
/// The order IS the z-order.
///
/// ONE enumeration, two readers. Escape asks it which surface to close;
/// the keyboard scroll asks it only whether anything at all sits over the pane
/// it would otherwise move. Every keyboard route that ignores this list routes
/// a key at the screen BEHIND the layer the reader is looking at.
///
/// NO PER-TAB RUNG IS LEFT, so the ladder takes no tab. The chat tab's menus,
/// drawer and rail belong to the chat view (`crates/views/chat`); the pages
/// tab's armed delete, block menu and comments card belong to the pages view
/// (`crates/views/pages`); the forge's switchers are the host's own pick lists.
/// Each guest paints its own scrim and dismisses its own layers — a key the
/// kernel contract carries no door for. What is enumerated here is what rides
/// EVERY tab, which is why `slot palette` and `slot bell` sit OUTSIDE the
/// `match tab` in `components/shell.ice`.
//
// One argument per layer: the Ice extern surface is flat, and the reading must
// see every layer at once to name the topmost.
pub fn topmost_overlay(palette_open: bool, bell_open: bool, channel_create_open: bool) -> String {
    if palette_open {
        return "palette".into();
    }
    if bell_open {
        return "bell".into();
    }
    if channel_create_open {
        return "channel_create".into();
    }
    // THE CHAT, PAGES AND FILES LAYERS ARE THEIR VIEWS' OWN. Each holds the
    // keyboard inside its tab and answers Escape itself — the chat menus and
    // drawer, the pages armed delete, the comments card. The guest paints the
    // scrim, so the guest owns the exit.
    //
    // The forge's repository and branch switchers are the host's own pick
    // lists: the host dismisses their menus itself, so they hold no rung.
    String::new()
}

/// The surface Escape dismisses — the topmost transient layer, minus the one
/// rung Escape does not own: an open palette swallows the key itself. What is
/// left after the views took their own layers is the palette, the bell and the
/// create modal, all three of which ride every tab.
pub fn escape_target(
    logical: iced::keyboard::Key,
    palette_open: bool,
    bell_open: bool,
    channel_create_open: bool,
) -> String {
    use iced::keyboard::{Key, key::Named};
    let not_escape = logical != Key::Named(Named::Escape);
    if not_escape {
        return String::new();
    }
    let topmost = topmost_overlay(palette_open, bell_open, channel_create_open);
    // `palette_key_action` owns the palette's keys — an open palette swallows
    // Escape, so the ladder yields rather than naming a rung.
    let palette_owns_it = topmost == "palette";
    if palette_owns_it {
        return String::new();
    }
    topmost
}

/// True when the live connection is in a state the shell should banner:
/// the stream is down, retrying, or a resync failed and is backing off.
pub fn connection_degraded(status: &str) -> bool {
    status == "Offline"
        || status == "Sync delayed"
        || status == "Reconnecting…"
        || status == "Live · resyncing"
}

pub fn canonical_endpoint(input: String) -> String {
    let configured = input.trim();
    rpc_client(configured)
        .map(|rpc| rpc.origin().to_string())
        .unwrap_or_else(|_| configured.to_string())
}

/// Is this press ⌘C? Its own question, not an arm on [`command_chord`]: the
/// route that asks it exists ONLY while a copy range is open, so ⌘C costs
/// nothing on a screen with no selection, and the quit/close chord — which is
/// armed by ⌘ alone, on every screen — keeps a vocabulary of two.
///
/// It reaches that route only when no widget captured the press (the
/// subscription is `status=ignored`), so a caret in a composer or a field with
/// its own selection keeps its own copy, exactly as it should.
pub fn is_copy_chord(
    logical: iced::keyboard::Key,
    physical: iced::keyboard::key::Physical,
    modifiers: iced::keyboard::Modifiers,
) -> bool {
    use iced::keyboard::key::{Code, Physical};
    modifiers.command() && (physical == Physical::Code(Code::KeyC) || types_letter(&logical, "c"))
}
