app Ducktape
  title "Ducktape"
  theme app_theme
  bg app_background
  fg app_text
  id "dev.ducktape.app"
  font "../../../crates/design/assets/fonts/Geist[wght].ttf"
  font "../../../crates/design/assets/fonts/GeistMono[wght].ttf"
  font "../../../crates/design/assets/fonts/IBMPlexSansKR-Regular.ttf"
  font "../../../crates/design/assets/fonts/IBMPlexSansKR-Medium.ttf"
  font "../../../crates/design/assets/fonts/IBMPlexSansKR-SemiBold.ttf"
  text-size 13.5
  antialiasing true
  window
    size 1280 800
    min-size 820 540
    position centered
    platform macos
      title-hidden true
      titlebar-transparent true
      fullsize-content-view true

use "backend.ice"
use "ducktape-ui/default.ice"
use "theme.ice"
use "state.ice"
use "components/icon.ice"
use "components/kit.ice"
use "components/shell.ice"
use "components/chat.ice"
use "components/pages.ice"
use "handlers/lifecycle.ice"
use "handlers/chat.ice"
use "handlers/pages.ice"
use "view.ice"
use "tests.ice"
