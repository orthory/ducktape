preset ui_offline
  state
    rpc = ""
    status = "Offline"
    connected = false
    loading = false
    mutation_phase = "idle"
    error = ""
    shell_tab = "chat"
    channel_draft = ""
    channel_create_members_only = false
    palette_open = false
    palette_draft = ""

preset ui_palette_open
  state
    status = "Offline"
    connected = false
    loading = false
    mutation_phase = "idle"
    shell_tab = "chat"
    palette_open = true
    palette_draft = ""

preset ui_settings
  state
    rpc = ""
    status = "Offline"
    connected = false
    loading = false
    mutation_phase = "idle"
    error = ""
    shell_tab = "settings"

preset ui_component_error
  state
    error = "Connection failed"

test palette_escape_contract
  preset ui_palette_open
  viewport 1120 720
  mount
    WorkspaceTabs #workspace-tabs
      with
        network="testnet"
        status
        height=84912
        loading
        degraded=false
        tab=shell_tab
        bell_count=0
        bell_sev="info"
        approvals=0
        account=""
        agent_live=false
        tier="validator"
        root_hash=""
        consensus_view="—"
        quorum="—"
        reachable="—"
        last_finalized=0
        checkpoint=0
      events
        select_shell_tab -> select_shell_tab _
        toggle_bell -> toggle_bell
        switch_network -> switch_network

      huddle:
        space w=1.0 h=1.0
      notice:
        space w=1.0 h=1.0
      chat:
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
    WorkspaceTabs #workspace-tabs
      with
        network="testnet"
        status
        height=84912
        loading
        degraded=false
        tab=shell_tab
        bell_count=0
        bell_sev="info"
        approvals=0
        account=""
        agent_live=false
        tier="validator"
        root_hash=""
        consensus_view="—"
        quorum="—"
        reachable="—"
        last_finalized=0
        checkpoint=0
      events
        select_shell_tab -> select_shell_tab _
        toggle_bell -> toggle_bell
        switch_network -> switch_network

      huddle:
        space w=1.0 h=1.0
      notice:
        space w=1.0 h=1.0
      chat:
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
  resize 820 540
  expect rail.width ~= 74.0
  expect content.x ~= rail.right + 1.0
  expect content.width > 730.0

preset ui_launch
  state
    mutation_phase = "idle"
    onboarding_error = ""
    hub_step = "networks"
    hub_networks = []
    hub_selected = ""

// The launch window's two load-bearing renders: the unlock ceremony's
// password field is reachable, and an empty network list is the welcome
// plate whose one CTA routes to the join flow.
test launch_unlock_contract
  preset ui_launch
  viewport 480 680
  mount
    HubColumn #hub
      with
        step="unlock"
        key_state="encrypted"
        networks=hub_networks
        selected=""
        hidden=0
        name=""
        invite=""
        reveal=""
        steps=provision_steps
        step_index=0
        height=-1
        tier=""
        error=""
        busy=false
      events
        unlock_submit -> unlock_submit _
        login_skip -> login_skip
        create_submit -> create_submit _
        reveal_confirm -> reveal_confirm
        go_restore -> go_restore
        go_login -> go_login
        restore_submit -> restore_submit _ _
        pick_network -> pick_network _
        open_network_submit -> open_network_submit
        forget_network_submit -> forget_network_submit _ _
        connect_remote_submit -> connect_remote_submit _
        restore_hidden_submit -> restore_hidden_submit
        go_join -> go_join
        go_networks -> go_networks
        join_network_submit -> join_network_submit _
        copy_onboarding_invite -> copy_onboarding_invite
        enter_console -> enter_console
  target pw = #hub/root/unlock/root/unlock-password
  expect exists pw
  dispatch go_restore
  expect hub_step == "restore"
  dispatch go_login
  expect hub_step == "unlock"

test launch_networks_empty_contract
  preset ui_launch
  viewport 480 680
  mount
    HubColumn #hub
      with
        step="networks"
        key_state="encrypted"
        networks=hub_networks
        selected=""
        hidden=0
        name=""
        invite=""
        reveal=""
        steps=provision_steps
        step_index=0
        height=-1
        tier=""
        error=""
        busy=false
      events
        unlock_submit -> unlock_submit _
        login_skip -> login_skip
        create_submit -> create_submit _
        reveal_confirm -> reveal_confirm
        go_restore -> go_restore
        go_login -> go_login
        restore_submit -> restore_submit _ _
        pick_network -> pick_network _
        open_network_submit -> open_network_submit
        forget_network_submit -> forget_network_submit _ _
        connect_remote_submit -> connect_remote_submit _
        restore_hidden_submit -> restore_hidden_submit
        go_join -> go_join
        go_networks -> go_networks
        join_network_submit -> join_network_submit _
        copy_onboarding_invite -> copy_onboarding_invite
        enter_console -> enter_console
  target cta = #hub/root/networks/root/join-cta
  expect exists cta
  click cta
  expect hub_step == "join"

preset ui_palette_overlay
  state
    palette_open = true
    palette_draft = ""

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
        searching=palette_searching
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
