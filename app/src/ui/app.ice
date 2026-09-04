daemon Ducktape
  title "Ducktape"
  theme "app"
  palette app_palette
  bg app_background
  fg app_text
  id "dev.ducktape.app"
  font "../../../crates/design/assets/fonts/Geist[wght].ttf"
  font "../../../crates/design/assets/fonts/GeistMono[wght].ttf"
  // Not a type role: with no emoji face in the font system, cosmic-text
  // re-scans the whole database for EVERY emoji on EVERY fresh paragraph
  // (~3.7ms each, uncached on miss) — the chat row toolbar carries three,
  // which made every freshly mounted row ~11ms and a channel switch a
  // half-second layout freeze. A resolved fallback costs ~60us.
  font "../../../crates/design/assets/fonts/NotoColorEmoji.ttf"
  text-size 13.5
  antialiasing true
  // The status item. A click raises THIS menu, never a window: the platform
  // only pops a menu that has rows, so with none a press did nothing but
  // activate the process — which read as "the app opened".
  //
  // The icon is a state channel — grey while no node answers, a dot while
  // the bell holds unread — and the label beside it is the bell count. The
  // menu's two top rows are READ, not chosen: the network and the node's own
  // status line. Everything below is a command; a row whose text moves with
  // the state (the bell count, the huddle's channel, Mute/Unmute, the ✓ on
  // the appearance) is still the same command, which is what lets a test
  // choose it by its current words.
  //
  // A row's `when` takes it out of the menu — not disabled, gone — while
  // there is nothing for it to do: the console's rows while only the launch
  // window is up, the huddle's while she is not in one. The handlers keep
  // their own guards; a native menu is its own event source and may deliver
  // a row the frame after its reason went away.
  tray
    icon-rgba "../../assets/tray-offline.rgba" 128 128 when !connected
    icon-rgba "../../assets/tray-unread.rgba" 128 128 when bell_unread > 0
    icon-rgba "../../assets/tray.rgba" 128 128
    label tray_badge(bell_unread)
    tooltip tray_tooltip(network_name, status)
    menu
      keep_str(empty(network_name), "No network", network_name)
      status
      separator
      "Open Ducktape" -> tray_open
      tray_bell_row(bell_unread) -> tray_open_bell when console_win != none
      "Go to" when console_win != none
        "Chat" -> tray_go_chat
        "Pages" -> tray_go_pages
        "Node" -> tray_go_node
        "Settings" -> tray_go_settings
      separator
      tray_huddle_row(huddle_joined, huddle_channel_name) when huddle_joined
        keep_str(call_muted, "Unmute", "Mute") -> toggle_call_mute
        "Leave huddle" -> leave_huddle_here
      "Appearance"
        tray_choice_row("Light", appearance == Appearance.light) -> set_appearance_light
        tray_choice_row("Dark", appearance == Appearance.dark) -> set_appearance_dark
      separator
      "Copy node key" -> tray_copy_node_key when console_win != none
      "Reconnect" -> tray_reconnect when console_win != none
      separator
      "Quit Ducktape" -> tray_quit
  // The launch window: Discord/Steam-shaped — a small fixed column that
  // signs the user in and picks a network. It opens on mount and closes when
  // the console takes over.
  // EVERY WINDOW REPORTS THE APP ID. On Linux `app-id` is the ONLY source of
  // the X11 WM_CLASS / Wayland app_id — the daemon-level `id` above reaches
  // the BSDs and nothing else (iced_winit's `conversion.rs`), so without this
  // block a window reports an empty class and associates with no installed
  // `.desktop` entry: no icon, no pinned-app identity. It is the same string
  // as `app/packaging/dev.ducktape.app.desktop`'s file name and
  // `StartupWMClass`, and all three windows carry it so the app is one
  // identity to the desktop.
  window onboarding
    icon-rgba "../../assets/icon.rgba" 128 128
    size 480 680
    position centered
    resizable false
    platform linux
      app-id "dev.ducktape.app"
  // The console. Same window the single-window app declared, now a named
  // template `open_network_submit` instantiates.
  // `min-size` is NOT a taste number — it is the arithmetic of the fixed
  // chrome this console draws. The widest screen is chat with a rail open:
  // 74 workspace rail + 1 + 236 channel list + 1 + 330 thread rail = 643px
  // that never yields, leaving the message column whatever is left. At the
  // old 820 that was 177px — a composer whose Send button fell off the
  // window and a sentence wrapped over eleven lines. 1040 leaves 397, and it
  // clears every other screen's worst case too (pages 612 + editor, roster
  // 74+312, files 74+306). No `responsive` breakpoint: the only honest
  // alternative is suppressing a rail, and a console that silently drops the
  // pane you just opened is worse than one that will not get that small.
  window console
    icon-rgba "../../assets/icon.rgba" 128 128
    size 1280 800
    min-size 1040 540
    position centered
    platform linux
      app-id "dev.ducktape.app"
    platform macos
      title-hidden true
      titlebar-transparent true
      fullsize-content-view true
  // The huddle, popped out. It keeps REAL chrome — no `platform macos` block
  // here — because the OS close button IS the dock control; the console is
  // the one window that trades its titlebar away to draw its own.
  // The panel scrolls its stage now, so no roster can push the controls out;
  // the minimum only has to hold the two chrome bands plus one row of tiles.
  // 340 = 42 header + 52 controls + 2 rules + ~190 of stage, and the width
  // never goes under the size the window ships at.
  window huddle
    icon-rgba "../../assets/icon.rgba" 128 128
    size 320 460
    min-size 320 340
    platform linux
      app-id "dev.ducktape.app"

use "extern/backend.ice"
use "extern/editor.ice"
use "extern/call.ice"
use "extern/wasm_view.ice"
use "ducktape-ui/recipes.ice"
use "ducktape-ui/log-timeline.ice"
use "theme.ice"
use "state/types.ice"
use "state/core.ice"
use "state/chat.ice"
use "state/shell.ice"
use "state/explorer.ice"
use "state/roster.ice"
use "state/forge.ice"
use "state/node.ice"
use "state/files.ice"
use "state/overlays.ice"
use "state/pages.ice"
use "state/onboarding.ice"
use "state/huddle.ice"
use "state/derived.ice"
use "components/icon.ice"
use "components/kit.ice"
use "components/patterns.ice"
use "components/overlay.ice"
use "components/shell.ice"
use "components/onboarding.ice"
use "components/chat.ice"
use "components/dm.ice"
use "components/huddle.ice"
use "components/pages.ice"
use "components/forge.ice"
use "components/roster.ice"
use "components/files.ice"
use "components/node.ice"
use "screens/roster.ice"
use "screens/governance.ice"
use "screens/overlays.ice"
use "screens/storage.ice"
use "screens/settings.ice"
use "screens/node.ice"
use "screens/forge.ice"
use "screens/pages.ice"
use "screens/chat.ice"
use "screens/shell.ice"
use "handlers/lifecycle.ice"
use "handlers/forge.ice"
use "handlers/files.ice"
use "handlers/roster.ice"
use "handlers/node.ice"
use "handlers/overlays.ice"
use "handlers/chat.ice"
use "handlers/pages.ice"
use "handlers/onboarding.ice"
use "handlers/huddle.ice"
use "handlers/shell.ice"
use "view.ice"
use "tests/app.ice"
