daemon Ducktape
  title "Ducktape"
  theme app_theme
  palette app_palette
  bg app_background
  fg app_text
  id "dev.ducktape.app"
  font "../../../crates/design/assets/fonts/Geist[wght].ttf"
  font "../../../crates/design/assets/fonts/GeistMono[wght].ttf"
  text-size 13.5
  antialiasing true
  tray
    icon-rgba "../../assets/tray.rgba" 128 128
    tooltip "Ducktape"
  // The launch window: Discord/Steam-shaped — a small fixed column that
  // signs the user in and picks a network. It opens on mount and closes when
  // the console takes over.
  window onboarding
    icon-rgba "../../assets/icon.rgba" 128 128
    size 480 680
    position centered
    resizable false
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

use "extern/backend.ice"
use "extern/editor.ice"
use "extern/call.ice"
use "ducktape-ui/default.ice"
use "theme.ice"
use "state.ice"
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
