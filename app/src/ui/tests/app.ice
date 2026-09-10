preset ui_offline
  state
    rpc = ""
    status = "Offline"
    connected = false
    loading = false
    mutation_phase = MutationPhase.idle
    error = ""
    shell_tab = ShellTab.chat
    channel_draft = ""
    channel_create_members_only = false
    palette_open = false
    palette_draft = ""

preset ui_palette_open
  state
    status = "Offline"
    connected = false
    loading = false
    mutation_phase = MutationPhase.idle
    shell_tab = ShellTab.chat
    palette_open = true
    palette_draft = ""

preset ui_settings
  state
    rpc = ""
    status = "Offline"
    connected = false
    loading = false
    mutation_phase = MutationPhase.idle
    error = ""
    shell_tab = ShellTab.settings

preset ui_component_error
  state
    error = "Connection failed"

test palette_escape_contract
  preset ui_palette_open
  viewport 1120 720
  mount
    WorkspaceTabs wall_now=wall_now #workspace-tabs
      with
        network="testnet"
        status
        height=84912
        sync_line=sync_label(node_phase, node_sync_applied, node_sync_target)
        loading
        degraded=false
        tab=shell_tab
        bell_count=0
        bell_sev="info"
        approvals=0
        account=""
        agent_live=false
        tier="validator"
        answered=true
        root_hash=""
        consensus_view="—"
        quorum="—"
        reachable="—"
        last_finalized=0
      events
        select_shell_tab -> select_shell_tab _
        toggle_bell -> toggle_bell
        switch_network -> switch_network

      notice:
        space w=1.0 h=1.0
      chat:
        space w=1.0 h=1.0
      shell:
        space w=1.0 h=1.0
      pages:
        space w=1.0 h=1.0
      files:
        space w=1.0 h=1.0
      members:
        space w=1.0 h=1.0
      agents:
        space w=1.0 h=1.0
      forge:
        space w=1.0 h=1.0
      governance:
        space w=1.0 h=1.0
      node:
        space w=1.0 h=1.0
      settings:
        space w=1.0 h=1.0
      explorer:
        space w=1.0 h=1.0
      palette:
        stack w=fill h=fill
          if palette_open
            input "" #palette-input <-> palette_draft
              with
                label="Search everything"
                hint="Search messages and pages… (Esc closes)"
                w=540.0
                @control
      bell:
        space w=1.0 h=1.0
  target palette = #workspace-tabs/palette-input
  expect palette_open
  expect exists palette
  click palette
  key escape
  expect !palette_open
  expect missing palette

test channel_draft_contract
  preset ui_offline
  viewport 480 240
  mount
    Field label="Channel name" description="Used when creating a channel." #channel-field
      input "" #draft <-> channel_draft
        with
          label="Channel name"
          hint="general"
          @control
  target draft = #channel-field/root/draft
  expect channel_draft == ""
  click draft
  type "general"
  expect draft.value == "general"
  expect channel_draft == "general"
  key backspace
  expect draft.value == "genera"
  expect channel_draft == "genera"
  dispatch toggle_channel_create_members_only
  expect channel_create_members_only

test shared_components_contract
  preset ui_component_error
  viewport 560 360
  mount
    box #surface w=fill
      Panel #library
        with
          title="Shared components"
          description="The app uses the default component library."
        col w=fill gap=12.0
          Alert.Destructive title="Connection failed" description=error #alert
          row
            with
              w=fill
              gap=8.0
              align=center
            Badge.Success label="Ready" #badge
            Kbd label="Esc" #kbd
          button "Dismiss" #dismiss @primary_action -> dismiss_error
  target library = #surface
  target alert = #surface/library/root/alert/root
  target badge = #surface/library/root/badge/root
  target kbd = #surface/library/root/kbd/root
  target dismiss = #surface/library/root/dismiss
  expect text "Shared components" within library
  expect text "Connection failed" within alert
  expect text "Ready" within badge
  expect text "Esc" within kbd
  expect alert.width ~= library.width - 40.0
  expect alert.border.color == color.rgb8(239, 214, 211)
  expect alert.border.width ~= 1.0
  expect alert.border.radius == radius(11.0)
  expect dismiss.background == background.color(color.rgb8(38, 37, 31))
  expect dismiss.border.radius == radius(9.0)
  click dismiss
  expect error == ""

test minimum_window_layout_contract
  preset ui_offline
  viewport 1280 800
  mount
    WorkspaceTabs wall_now=wall_now #workspace-tabs
      with
        network="testnet"
        status
        height=84912
        sync_line=sync_label(node_phase, node_sync_applied, node_sync_target)
        loading
        degraded=false
        tab=shell_tab
        bell_count=0
        bell_sev="info"
        approvals=0
        account=""
        agent_live=false
        tier="validator"
        answered=true
        root_hash=""
        consensus_view="—"
        quorum="—"
        reachable="—"
        last_finalized=0
      events
        select_shell_tab -> select_shell_tab _
        toggle_bell -> toggle_bell
        switch_network -> switch_network

      notice:
        space w=1.0 h=1.0
      chat:
        space w=1.0 h=1.0
      shell:
        space w=1.0 h=1.0
      pages:
        space w=1.0 h=1.0
      files:
        space w=1.0 h=1.0
      members:
        space w=1.0 h=1.0
      agents:
        space w=1.0 h=1.0
      forge:
        space w=1.0 h=1.0
      governance:
        space w=1.0 h=1.0
      node:
        space w=1.0 h=1.0
      settings:
        space w=1.0 h=1.0
      explorer:
        space w=1.0 h=1.0
      palette:
        space w=1.0 h=1.0
      bell:
        space w=1.0 h=1.0
  target titlebar = #workspace-tabs/titlebar/root
  target rail = #workspace-tabs/rail/root
  target content = #workspace-tabs/content
  expect titlebar.height ~= 40.0
  expect rail.width ~= 74.0
  expect rail.y ~= titlebar.bottom
  expect content.x ~= rail.right + 1.0
  expect content.width > 1180.0
  expect rail.background == background.color(color.rgb8(250, 250, 248))
  expect content.background == background.color(color.rgb8(253, 253, 251))
  window resize 820 540
  expect rail.width ~= 74.0
  expect content.x ~= rail.right + 1.0
  expect content.width > 730.0

preset ui_launch
  state
    mutation_phase = MutationPhase.idle
    onboarding_error = ""
    hub_step = HubStep.networks
    hub_networks = []
    hub_selected = ""
    rpc = "http://127.0.0.1:1"
    password = "hunter2-hunter2"
    // Ice reads extern structs but cannot construct one, so the rows come
    // from the same `wallet_info` constructor the backend hands the list.
    hub_wallets = [wallet_info("alice", "aabbccddeeff00112233", "encrypted", false), wallet_info("demo", "eeff0011", "encrypted", true)]
    hub_wallet_selected = "demo"

// The launch window's two load-bearing renders: the wallet list's selected
// row carries the password field, and an empty network list is the welcome
// plate whose one CTA routes to the join flow — with the remote-endpoint
// field still there, because a device with no local network is exactly the
// one that connects to a remote node.
test launch_wallets_contract
  preset ui_launch
  viewport 480 680
  mount
    HubColumn name_draft<->welcome_name_draft #hub
      with
        network=""
        phase=""
        qr=""
        detail=""
        left=""
        step=HubStep.wallets
        wallets=hub_wallets
        wallet_selected=hub_wallet_selected
        networks=hub_networks
        selected=""
        name=""
        invite=""
        steps=provision_steps
        step_index=0
        height=-1
        tier=""
        error=""
        busy=false
        restore_empty=true
        join_empty=true
      events
        drag_launch_window -> drag_launch_window
        close_launch_window -> close_launch_window
        pick_wallet -> pick_wallet _
        unlock_submit -> unlock_submit _
        login_skip -> login_skip
        password_submit -> password_submit _
        phrase_written_down -> phrase_written_down
        show_phrase_again -> show_phrase_again
        confirm_phrase_submit -> confirm_phrase_submit _
        welcome_create_submit -> welcome_create_submit _
        welcome_login_submit -> welcome_login_submit
        welcome_desktop -> welcome_desktop
        welcome_cancel -> welcome_cancel
        welcome_skip -> welcome_skip
        go_restore -> go_restore
        go_login -> go_login
        restore_submit -> restore_submit _ _
        pick_network -> pick_network _
        open_network_submit -> open_network_submit
        forget_network_submit -> forget_network_submit _
        connect_remote_submit -> connect_remote_submit _
        go_join -> go_join
        go_networks -> go_networks
        join_network_submit -> join_network_submit
        copy_onboarding_invite -> copy_onboarding_invite
        enter_console -> enter_console
  target pw = #hub/root/wallets/root/wallet-row("demo")/root/wallet-password
  expect exists pw
  dispatch go_restore
  expect hub_step == HubStep.restore
  dispatch go_login
  expect hub_step == HubStep.wallets

// THE WALLET LIST IS THE UNLOCK SURFACE. Every wallet on the keystore is a
// row; the selected one — the active wallet on boot — is the only one showing
// a password field, and picking another moves the field with the selection.
test wallet_list_contract
  preset ui_launch
  viewport 520 680
  mount
    WalletsScreen #wallets
      with
        wallets=hub_wallets
        selected=hub_wallet_selected
        network="demo"
        busy=false
        error=""
      events
        pick_wallet -> pick_wallet _
        unlock_submit -> unlock_submit _
        login_skip -> login_skip
        go_restore -> go_restore
        go_networks -> go_networks
  target list = #wallets/root
  target demo_pw = #wallets/root/wallet-row("demo")/root/wallet-password
  target alice_row = #wallets/root/wallet-row("alice")/root/wallet-pick
  target alice_pw = #wallets/root/wallet-row("alice")/root/wallet-password
  // A secure input renders no text to read back, so the draft is asserted
  // where it lives: the row instance's own component state.
  target alice = #wallets/root/wallet-row("alice")
  // the active wallet is preselected: its row is the one carrying the input.
  expect exists demo_pw
  expect missing alice_pw
  expect text "demo" within list
  expect text "alice" within list
  // the row shows the identity it signs as, shortened, never invented.
  expect text "aabbccddeeff0011…" within list
  // selecting the other row moves the input there.
  click alice_row
  expect hub_wallet_selected == "alice"
  expect exists alice_pw
  expect missing demo_pw
  click alice_pw
  type "hunter2-hunter2"
  expect component alice.pw == "hunter2-hunter2"

// A KEYSTORE THAT COULD NOT BE READ LANDS HERE, and read-only is the way out.
// A failed `wallet list` yields an empty list, which is the password step —
// so this screen, not just the wallet list, has to carry `login_skip`, or
// someone who HAS wallets is trapped on a mint screen by an unreadable
// keystore. The mint itself is NOT dispatched: `password_submit` seals a
// real key. Skipping opens the console (a window task — not asserted here).
test password_screen_read_only_escape_contract
  preset ui_launch
  viewport 480 680
  mount
    PasswordScreen #pw network="demo" busy=false error="the keystore listing is unreadable"
      events
        password_submit -> password_submit _
        go_restore -> go_restore
        login_skip -> login_skip
        go_networks -> go_networks
  target screen = #pw/root
  target skip = #pw/root/password-skip
  target field = #pw/root/device-password
  target go = #pw/root/password-submit
  expect exists field
  expect exists go
  expect text "the keystore listing is unreadable" within screen
  expect exists skip

// A NETWORK PICK OPENS THE DOOR ITS KEYSTORE NAMES — the launch window's
// load-bearing branch. Rows land on the wallet list with the active row
// picked; an empty keystore lands on the password step, carrying the
// listing's error. A keystore that could not be NAMED (a remote whose node
// never answered) opens nothing: the step stays where it was and the error
// shows there. There is no door that opens the console read-only on its own.
test a_network_pick_opens_the_door_its_keystore_names
  preset ui_pick_probe
  dispatch wallets_loaded(wallet_list([wallet_info("alice", "aabbccddeeff00112233", "encrypted", false), wallet_info("demo", "eeff0011", "encrypted", true)], "", true))
  expect hub_step == HubStep.wallets
  expect hub_wallet_selected == "demo"
  expect mutation_phase == MutationPhase.idle
  dispatch wallets_loaded(wallet_list([], "the keystore listing is unreadable", true))
  expect hub_step == HubStep.password
  expect hub_wallet_selected == ""
  expect onboarding_error == "the keystore listing is unreadable"
  dispatch pick_network("remote")
  dispatch wallets_loaded(wallet_list([], "this node could not be reached", false))
  expect hub_step == HubStep.password
  expect onboarding_error == "this node could not be reached"
  expect mutation_phase == MutationPhase.idle

// THE PHRASE SCREEN, on a FIXED mnemonic. `phrase_rows_of` is mounted rather
// than `phrase_rows` on purpose: the live one reads the phrase a real mint is
// holding, and a capture of THAT would put a live key's only backup into a
// PNG. All 24 words are drawn, the grid pairs 1↔13 … 12↔24, and the one door
// out of the screen goes to the confirm.
test phrase_screen_shows_all_24_words
  preset ui_launch
  viewport 480 680
  mount
    PhraseScreen #phrase
      with
        rows=phrase_rows_of("abandon amount liar amount expire adjust cage candy arch gather drum bullet absurd math era live bid rhythm alien crouch range attend journey unaware")
        busy=false
      events
        phrase_written_down -> phrase_written_down
  target screen = #phrase/root
  target go = #phrase/root/phrase-continue
  expect exists go
  expect text "abandon" within screen
  expect text "absurd" within screen
  expect text "unaware" within screen
  expect text "after this ceremony, the app never shows it again" within screen
  capture launch_phrase_light
  click go
  expect hub_step == HubStep.confirm

// THE CONFIRM SCREEN. The prompt is the backend's sentence (Ice cannot build
// one), the field starts empty so the Confirm button starts dead, and the way
// past a typo is back to the phrase — which is still held until this passes.
test confirm_screen_asks_three_words_back
  preset ui_launch
  viewport 480 680
  mount
    ConfirmPhraseScreen #confirm
      with
        prompt="Type words 5, 12 and 20 — in that order, separated by spaces."
        busy=false
        error=""
      events
        confirm_phrase_submit -> confirm_phrase_submit _
        show_phrase_again -> show_phrase_again
  target screen = #confirm/root
  target field = #confirm/root/confirm-words
  target back = #confirm/root/confirm-back
  expect exists field
  expect text "Type words 5, 12 and 20 — in that order, separated by spaces." within screen
  capture launch_confirm_light
  click back
  expect hub_step == HubStep.phrase

test launch_networks_empty_contract
  preset ui_launch
  viewport 480 680
  mount
    HubColumn name_draft<->welcome_name_draft #hub
      with
        network=""
        phase=""
        qr=""
        detail=""
        left=""
        step=HubStep.networks
        wallets=hub_wallets
        wallet_selected=hub_wallet_selected
        networks=hub_networks
        selected=""
        name=""
        invite=""
        steps=provision_steps
        step_index=0
        height=-1
        tier=""
        error=""
        busy=false
        restore_empty=true
        join_empty=true
      events
        drag_launch_window -> drag_launch_window
        close_launch_window -> close_launch_window
        pick_wallet -> pick_wallet _
        unlock_submit -> unlock_submit _
        login_skip -> login_skip
        password_submit -> password_submit _
        phrase_written_down -> phrase_written_down
        show_phrase_again -> show_phrase_again
        confirm_phrase_submit -> confirm_phrase_submit _
        welcome_create_submit -> welcome_create_submit _
        welcome_login_submit -> welcome_login_submit
        welcome_desktop -> welcome_desktop
        welcome_cancel -> welcome_cancel
        welcome_skip -> welcome_skip
        go_restore -> go_restore
        go_login -> go_login
        restore_submit -> restore_submit _ _
        pick_network -> pick_network _
        open_network_submit -> open_network_submit
        forget_network_submit -> forget_network_submit _
        connect_remote_submit -> connect_remote_submit _
        go_join -> go_join
        go_networks -> go_networks
        join_network_submit -> join_network_submit
        copy_onboarding_invite -> copy_onboarding_invite
        enter_console -> enter_console
  target cta = #hub/root/networks/root/join-cta
  target remote_field = #hub/root/networks/root/remote-endpoint
  expect exists cta
  expect exists remote_field
  click cta
  expect hub_step == HubStep.join

// The welcome step mid-ceremony: the QR the stream handed back is on screen.
preset ui_welcome_qr
  state
    mutation_phase = MutationPhase.onboarding
    onboarding_error = ""
    hub_step = HubStep.account
    ceremony_phase = "show_qr"
    ceremony_qr = "https://auth.ducktape.industries/#op=get&challenge=AQID"
    ceremony_detail = "Your phone will confirm with the passkey."
    ceremony_left = "4:58"

// A connected, signing console whose device key has no account here.
preset ui_console_no_account
  state
    rpc = "http://127.0.0.1:1"
    connected_rpc = "http://127.0.0.1:1"
    password = "hunter2-hunter2"
    status = "Connected"
    connected = true
    loading = false
    mutation_phase = MutationPhase.idle
    error = ""
    account_exists = false
    account_banner_dismissed = false
    shell_tab = ShellTab.settings

// A signing session that just picked a network — the probe is in flight.
preset ui_pick_probe
  state
    password = "hunter2-hunter2"
    rpc = "http://127.0.0.1:1"
    mutation_phase = MutationPhase.onboarding
    hub_step = HubStep.networks

// THE WELCOME SCREEN is where a device key with no account on the picked
// network lands. Skipping opens the console (a window task — not asserted
// here); a QR in state renders as a real qr node; cancel clears it.
test welcome_screen_contract
  preset ui_welcome_qr
  viewport 480 680
  mount
    WelcomeScreen name_draft<->welcome_name_draft #welcome
      with
        network="demo"
        phase=ceremony_phase
        qr=ceremony_qr
        detail=ceremony_detail
        left=ceremony_left
        busy=false
        error=""
      events
        welcome_create_submit -> welcome_create_submit _
        welcome_login_submit -> welcome_login_submit
        welcome_desktop -> welcome_desktop
        welcome_cancel -> welcome_cancel
        welcome_skip -> welcome_skip
  target screen = #welcome/root
  target qr = #welcome/root/welcome-qr
  target left = #welcome/root/welcome-left
  target cancel = #welcome/root/welcome-cancel
  target create = #welcome/root/welcome-create
  expect exists qr
  expect exists left
  expect missing create
  expect text "demo" within screen
  click cancel
  expect ceremony_qr == ""
  expect ceremony_phase == ""
  expect hub_step == HubStep.account
  // with no ceremony in flight the doors are back, and the QR is gone.
  expect exists create
  expect missing qr

// A network pick probes the account BEFORE the console opens: no account →
// the welcome step; an account → straight through (window task, not
// asserted). The probe answer is dispatched directly — the pick itself
// launches a real `load_account`.
test network_pick_lands_on_the_welcome_step_without_an_account
  preset ui_pick_probe
  dispatch account_probed(account_data_none(7))
  expect hub_step == HubStep.account
  expect mutation_phase == MutationPhase.idle
  expect ceremony_phase == ""

// THE BANNER: a connected console whose device key has no account says so;
// dismiss hides it for the session. Reopening the launch window at the
// welcome is a window task (not asserted).
test console_banner_names_the_missing_account
  preset ui_console_no_account
  viewport 900 300
  mount
    AccountBanner #account-banner
      with
        connected
        account_exists
        dismissed=account_banner_dismissed
        password
      events
        open_account_welcome -> open_account_welcome
        dismiss_account_banner -> dismiss_account_banner
  target banner = #account-banner/root/banner
  target dismiss = #account-banner/root/banner/dismiss
  expect exists banner
  click dismiss
  expect account_banner_dismissed == true
  expect missing banner

// A ceremony's steps drive the welcome: `working` swaps the QR for its line,
// `failed` lands the message on the screen and frees the machine.
test ceremony_steps_drive_the_welcome
  preset ui_welcome_qr
  dispatch ceremony_stepped(ceremony_step("working", "", "Consenting to the new key…"))
  expect ceremony_phase == "working"
  expect ceremony_qr == ""
  expect ceremony_detail == "Consenting to the new key…"
  expect mutation_phase == MutationPhase.onboarding
  dispatch ceremony_stepped(ceremony_step("failed", "", "the phone did not answer in time"))
  expect ceremony_phase == ""
  expect onboarding_error == "the phone did not answer in time"
  expect mutation_phase == MutationPhase.idle

preset ui_palette_overlay
  state
    palette_open = true
    palette_draft = ""
    // Every keystroke here launches a real `palette_search`. Pinned for the
    // reason `ui_chat_stream` states at length: an empty endpoint is a
    // fallback, not a refusal.
    connected_rpc = "http://127.0.0.1:1"

// The palette is an `overlay`, not a tinted box, so the backdrop takes the
// pointer instead of letting clicks through to the console behind it. This
// guards the half that a compile cannot: that the field is still REACHABLE
// inside the layer, so the widget swap cannot silently hide the palette.
test palette_overlay_contract
  preset ui_palette_overlay
  viewport 1120 720
  mount
    OverlayLayer draft<->channel_draft query<->palette_draft #overlays
      with
        create_open=false
        members_only=false
        busy=false
        connected=true
        loading=false
        toast=""
        tone="info"
        open=palette_open
        search_phase=palette_search_phase
        chat_hits=palette_chat_hits
        page_hits=palette_page_hits
      events
        toggle_channel_create -> toggle_channel_create
        toggle_channel_create_members_only -> toggle_channel_create_members_only
        create_channel_submit -> create_channel_submit
        dismiss_toast -> dismiss_toast
        close_palette -> close_palette
        palette_changed -> palette_changed _
        open_chat_search_hit -> open_chat_search_hit _ _ _
        open_page_search_hit -> open_page_search_hit _ _
  target field = #overlays/palette-input
  expect exists field
  click field
  type "duck"
  expect palette_draft == "duck"
  key escape
  expect !palette_open

// AND THE BACKDROP TAKES THE POINTER. #804's other half: the palette used to be
// a `box bg=scrim`, which tints the console and captures nothing — the rail and
// the composer behind it stayed live and clicking the dim did nothing at all.
// `palette_overlay_contract` above proves the field inside the layer is still
// reachable; this proves the console underneath is NOT. The click is aimed at a
// live control beneath the layer, and the two expectations are the whole claim:
// its handler does not fire, and the press lands on the backdrop, which
// dismisses. Both are established as false-then-true, so neither can pass on a
// palette that never opened.
test palette_backdrop_takes_the_pointer
  preset ui_palette_overlay
  viewport 1120 720
  mount
    stack w=fill h=fill
      button "Create a channel" #beneath @primary_action -> toggle_channel_create
      OverlayLayer draft<->channel_draft query<->palette_draft #overlays
        with
          create_open=false
          members_only=false
          busy=false
          connected=true
          loading=false
          toast=""
          tone="info"
          open=palette_open
          search_phase=palette_search_phase
          chat_hits=palette_chat_hits
          page_hits=palette_page_hits
        events
          toggle_channel_create -> toggle_channel_create
          toggle_channel_create_members_only -> toggle_channel_create_members_only
          create_channel_submit -> create_channel_submit
          dismiss_toast -> dismiss_toast
          close_palette -> close_palette
          palette_changed -> palette_changed _
          open_chat_search_hit -> open_chat_search_hit _ _ _
          open_page_search_hit -> open_page_search_hit _ _
  target beneath = #beneath
  target field = #overlays/palette-input
  expect palette_open
  expect !channel_create_open
  expect exists field
  click beneath
  expect !channel_create_open
  expect !palette_open
  expect missing field

// The status item's rows are commands, and a chosen row reaches its handler:
// a row index that drifts in codegen fails here, not silently in the menu bar.
// The two stat rows are not commands — a reader reads them — and with no node
// answering the icon is the grey one.
// Only the launch window is up here, so the console's rows and the huddle's
// are out of the menu — not disabled, absent.
test tray_menu_contract
  preset ui_offline
  expect tray icon "../../assets/tray-offline.rgba"
  expect tray item "No network"
  expect tray item "Offline"
  expect no tray command "Offline"
  expect tray command "Open Ducktape"
  expect tray command "Quit Ducktape"
  expect tray item "Appearance"
  expect no tray item "Notifications"
  expect no tray item "Go to"
  expect no tray item "Huddle"
  expect no tray item "Leave huddle"
  expect no tray item "Reconnect"
  tray choose "Open Ducktape"

preset ui_tray_live
  state
    connected = true
    // A console is up: `window_target(none)` names a fresh id, which is all
    // the console-only rows ask of the slot.
    console_win = some(window_target(none))
    // Refused at once, off the network — see `ui_chat_stream`. The live
    // stream's failure writes `status` (the retry arm), which is why no test
    // below reads the status row; `tray_menu_contract` does, offline.
    connected_rpc = "http://127.0.0.1:1"
    network_name = "demo"
    bell_unread = 3
    huddle_joined = true
    huddle_channel_name = "general"
    call_muted = false
    appearance = Appearance.dark

// The rows READ the state — the bell count rides the icon, the label and its
// row; the huddle's channel names its submenu; the appearance wears its ✓ —
// and a chosen row moves it: Mute becomes Unmute.
test tray_menu_reads_the_state
  preset ui_tray_live
  expect tray icon "../../assets/tray-unread.rgba"
  expect tray label "3"
  expect tray item "demo"
  expect tray item "Notifications · 3 unread"
  expect tray item "Huddle · #general"
  expect tray item "✓ Dark"
  expect no tray item "✓ Light"
  expect tray command "Mute"
  // Not "Reconnect": the status row reads "Reconnecting…" by now (see the
  // preset), and a text two rows carry names neither.
  expect tray command "Copy node key"
  expect tray item "Go to"
  tray choose "Mute"
  expect tray item "Unmute"
  expect call_muted

// A HUDDLE WITH A SCREEN ON THE STAGE, at the panel's narrowest. The controls
// row is where a huddle goes wrong when it grows: this window is 320px at its
// minimum, and the file above records `Leave` being pushed off the end of it
// once already. A fourth source control is exactly that risk again, so the
// contract is that the leave button still lands inside the panel.
preset ui_huddle_sharing
  state
    // Deliberately NOT connected, and no `huddle_channel`: those two are the
    // media leg's subscription gate, and a session opening under a mount test
    // fires its own `connecting` event, which resets the very toggles this
    // preset is setting.
    connected = false
    huddle_joined = true
    huddle_channel_name = "eng"
    call_status = "live"
    call_muted = false
    call_camera = false
    call_sharing = true
    call_video_live = true
    huddle_stage = "you"

test the_huddle_controls_survive_the_narrowest_panel
  preset ui_huddle_sharing
  viewport 320 640
  mount
    HuddlePanel #huddle
      with
        channel=huddle_channel_name
        elapsed="01:20"
        rows=huddle_rows
        status=call_status
        muted=call_muted
        camera=call_camera
        sharing=call_sharing
        stage=huddle_stage
        video_live=call_video_live
      events
        huddle_go_channel -> huddle_go_channel
        leave_huddle_here -> leave_huddle_here
        toggle_call_mute -> toggle_call_mute
        toggle_call_camera -> toggle_call_camera
        toggle_call_screen -> toggle_call_screen
  target panel = #huddle/root
  target share = #huddle/root/controls/root/share-stop
  target leave = #huddle/root/controls/root/leave
  expect call_sharing
  expect leave.x + leave.width <= panel.x + panel.width
  expect share.width ~= 32.0
  capture huddle_sharing_light

// CLOSING A WINDOW IS NOT QUITTING on a Mac. The process used to leave with
// its last tracked window, which made the red button a quit nobody asked for;
// now a close only unregisters the slot and the daemon goes on living in the
// status item. The one thing this cannot assert is an exit or its absence —
// the harness swallows `Action::Exit`, which is also why it runs the same off
// macOS, where the last close does leave — so it asserts the observable
// consequence: after the close, the menu still answers and can put a window
// back.
test closing_the_last_window_only_unregisters_it
  preset ui_offline
  tray choose "Open Ducktape"
  expect onboarding_win != none
  window closed
  expect onboarding_win == none
  expect console_win == none
  tray choose "Open Ducktape"
  expect onboarding_win != none

// "OPEN" HAS TO OPEN. With both slots empty there is nothing to raise, and
// `window_target` on an empty slot names a fresh id whose focus is a no-op —
// so the raise-only row this replaced did nothing at all once every window was
// closed, which is exactly the state the change above made reachable.
test the_status_item_opens_a_window_when_none_is_tracked
  preset ui_offline
  expect onboarding_win == none
  expect console_win == none
  tray choose "Open Ducktape"
  expect onboarding_win != none

// A CONNECTED NETWORK WITH NOTHING TRACKED IS ORDINARY (#1782): merely closing
// the console must re-enter the network through the doors' landing
// (`network_entered`, whose connect opens the CONSOLE on its answer), not send
// its owner back through the launch window to the network picker —
// `onboarding_opened` always re-runs `hub_state()`, which resets `hub_step`.
// `rpc` stays empty on purpose: the connect the landing would otherwise run
// needs a live socket this scenario has none of, so the landing stops at its
// reset, and the claims under test are which way it went (the console's
// connect begun, no launch window) and whether `hub_step` moved.
preset ui_tray_reconnect
  state
    connected = true
    hub_step = HubStep.live

test the_status_item_reopens_the_console_without_resetting_hub_step
  preset ui_tray_reconnect
  expect console_win == none
  expect onboarding_win == none
  tray choose "Open Ducktape"
  // The door's own writes, not the status row: the preset's `connected`
  // arms the live stream, whose publications write `status` whenever they
  // land (see `ui_tray_live`), so the row is nobody's to expect here.
  expect !connected
  expect loading
  expect onboarding_win == none
  expect hub_step == HubStep.live

// ⌘Q IS ARMED BY ⌘, AND BY NOTHING ELSE. The key-press route that carries the
// chord costs a proxied message and a rebuild for every key it sees, so it
// exists only while the command modifier is down: the modifier stream is the
// cheap half and `cmd_held` is the arming. That arming is what this can watch —
// the exit the chord ends in is swallowed by the harness — and it is the half
// that decides whether ordinary typing pays anything.
test the_quit_chord_route_is_armed_only_while_command_is_held
  preset ui_offline
  expect !cmd_held
  key "q"
  expect !cmd_held
  // Both modifiers at once, because the command key is ⌘ on a Mac and Ctrl
  // everywhere else and this scenario runs on both.
  modifiers control logo
  expect cmd_held
  modifiers
  expect !cmd_held

// A RUN'S PRIVATE OUTPUT IS ON SCREEN, UNDER THE KEY THAT MAY READ IT. The row
// carries a status line folded out of that run's stdout, which on a device that
// does not host the node is readable only because THIS key created the run.
preset ui_live_run_seated
  state
    connected = true
    connected_rpc = "http://127.0.0.1:8844"
    network_chain_id = "testnet#abcd"
    connect_generation = 7
    signer_key = "aa11"
    shell_tab = ShellTab.chat
    live_agents = [live_agent_row("channel-a", 2, "chat:2:agent-1", "Chief Duck", "Reading the repo")]

// THE SEAT MOVED, SO THE ROWS GO — NOW, not when the re-keyed lane next speaks.
//
// Re-keying the subscription on `signer_key` only fences what arrives NEXT, and
// the new lane's first notice waits on a `runs` query: a node that is slow,
// unreachable, or refusing the new key leaves the PREVIOUS key's output on
// screen for as long as that takes. Nothing in these two scenarios delivers a
// notice — that is the point. The clearing is the handler's own act.
test an_unlock_in_place_drops_the_previous_keys_live_rows
  preset ui_live_run_seated
  expect !empty(live_agents)
  dispatch settings_unlocked("bb22")
  expect signer_key == "bb22"
  expect empty(live_agents)

test locking_the_seat_takes_the_private_output_with_it
  preset ui_live_run_seated
  expect !empty(live_agents)
  dispatch settings_view_event(view_event("lock", ""))
  expect empty(signer_key)
  expect empty(password)
  expect empty(live_agents)
