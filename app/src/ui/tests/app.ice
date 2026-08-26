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

// A dense, settled turn for the inspector: it exercises the real Shell
// composition, answer markdown, long transcript spacing, and the composer's
// disabled-reason line without spending a provider credential.
preset ui_shell_showcase
  state
    shell_tab = ShellTab.shell
    connected = false
    loading = false
    mutation_phase = MutationPhase.idle
    error = ""
    shell_surface = ShellSurface.tasks
    shell_identity = "team-codex · Codex"
    shell_provider = "codex"
    shell_credential = "team-codex"
    shell_chat_entries = agent_chat_answer(agent_chat_push_user([], "Explain the execution path and call out the failure boundaries.", "codex"), "## Execution path\n\nThe request becomes a durable saga, streams provider activity into this view, and commits the final answer before the turn settles.\n\n- **Scheduling** pins work to the selected compute provider.\n- **Live output** stays observational.\n- **Saga state** is the canonical result.", "codex", "done", "", [])

test shell_task_surface_contract
  preset ui_shell_showcase
  viewport 1120 720
  mount
    ShellScreen draft<->shell_chat_draft #shell
      with
        surface=shell_surface
        setup_open=shell_setup_open
        identity_options=shell_identity_options
        identity=shell_identity
        provider=shell_provider
        credential=shell_credential
        host_node_options=shell_host_node_options
        host_node=shell_host_node
        credentials_loading=shell_credentials_loading
        terminal=shell_terminal
        terminal_running=shell_terminal_running
        terminal_busy=shell_terminal_busy
        terminal_title=shell_terminal_title
        terminal_error=shell_terminal_error
        entries=shell_chat_entries
        activity=shell_chat_activity
        chat_busy=shell_chat_busy
        chat_status=shell_chat_status
        chat_detail=shell_chat_detail
        live=shell_chat_live
        saga_id=shell_chat_saga
        steps_open=shell_steps_open
        detached_saga=shell_detached_saga
        connected=true
        dark=false
      events
        shell_surface_changed -> shell_surface_changed _
        shell_setup_toggled -> shell_setup_toggled
        shell_identity_changed -> shell_identity_changed _
        shell_host_node_changed -> shell_host_node_changed _
        shell_credentials_refresh -> shell_credentials_refresh
        shell_terminal_start -> shell_terminal_start
        shell_terminal_stop -> shell_terminal_stop
        shell_composer_event -> shell_composer_event _
        shell_chat_reset -> shell_chat_reset
        shell_chat_detach -> shell_chat_detach
        shell_chat_reopen -> shell_chat_reopen
        shell_chat_discard -> shell_chat_discard
        shell_chat_steps_toggled -> shell_chat_steps_toggled _
        shell_open_link -> open_message_link _
  target transcript = #shell/root/transcript
  target composer = #shell/root/draft
  target setup = #shell/root/setup
  expect exists transcript
  expect exists composer
  expect text "Execution path" within transcript
  expect transcript.width > 1000.0
  // The setup is folded away once an identity is picked: the header's one
  // summary line replaced the permanent four-control band.
  expect missing setup
  capture shell_tasks_light
  dispatch shell_setup_toggled
  expect exists setup
  capture shell_setup_light
  dispatch shell_setup_toggled
  // A SWITCH IS NEVER REFUSED. This used to be gated on nothing running; the
  // gate is gone, so the same dispatch lands whatever the tab is doing.
  dispatch shell_surface_changed(ShellSurface.terminal)
  expect shell_surface == ShellSurface.terminal
  expect missing transcript
  capture shell_terminal_light
  window resize 966 500
  capture shell_terminal_min_light

// The state the old screen could not represent at all: a run this app stopped
// watching, which the node is still executing. The plate is the address back to
// it, and the composer is held until the operator says which way that turn ends.
preset ui_shell_detached
  state
    shell_tab = ShellTab.shell
    connected = false
    loading = false
    mutation_phase = MutationPhase.idle
    error = ""
    shell_surface = ShellSurface.tasks
    shell_identity = "team-codex · Codex"
    shell_provider = "codex"
    shell_credential = "team-codex"
    shell_detached_saga = "sched-4f1c8a2b9d0e"
    shell_chat_entries = agent_chat_detach(agent_chat_push_user([], "Rebuild the drain loop benchmark and report the regression.", "codex"), "codex", "sched-4f1c8a2b9d0e", [])

test shell_detached_run_contract
  preset ui_shell_detached
  viewport 1120 720
  mount
    ShellScreen draft<->shell_chat_draft #shell
      with
        surface=shell_surface
        setup_open=shell_setup_open
        identity_options=shell_identity_options
        identity=shell_identity
        provider=shell_provider
        credential=shell_credential
        host_node_options=shell_host_node_options
        host_node=shell_host_node
        credentials_loading=shell_credentials_loading
        terminal=shell_terminal
        terminal_running=shell_terminal_running
        terminal_busy=shell_terminal_busy
        terminal_title=shell_terminal_title
        terminal_error=shell_terminal_error
        entries=shell_chat_entries
        activity=shell_chat_activity
        chat_busy=shell_chat_busy
        chat_status=shell_chat_status
        chat_detail=shell_chat_detail
        live=shell_chat_live
        saga_id=shell_chat_saga
        steps_open=shell_steps_open
        detached_saga=shell_detached_saga
        connected=true
        dark=false
      events
        shell_surface_changed -> shell_surface_changed _
        shell_setup_toggled -> shell_setup_toggled
        shell_identity_changed -> shell_identity_changed _
        shell_host_node_changed -> shell_host_node_changed _
        shell_credentials_refresh -> shell_credentials_refresh
        shell_terminal_start -> shell_terminal_start
        shell_terminal_stop -> shell_terminal_stop
        shell_composer_event -> shell_composer_event _
        shell_chat_reset -> shell_chat_reset
        shell_chat_detach -> shell_chat_detach
        shell_chat_reopen -> shell_chat_reopen
        shell_chat_discard -> shell_chat_discard
        shell_chat_steps_toggled -> shell_chat_steps_toggled _
        shell_open_link -> open_message_link _
  target transcript = #shell/root/transcript
  expect exists transcript
  expect text "Still running on the network" within transcript
  capture shell_detached_light
  // Discarding is what releases the composer, and it is the operator's call —
  // never a side effect of typing.
  dispatch shell_chat_discard
  expect shell_detached_saga == ""

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

      huddle:
        space w=1.0 h=1.0
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

// The composer toolbar's code glyph wears `color=inherit` (ducktape-ui#606):
// its ink IS the button's status-resolved text color. The probe point sits on
// the button's plate strictly LEFT of the glyph's own bounds — the exact spot
// the deleted IconAction ramp (hover on the svg's own bounds) left grey — and
// must brighten the glyph to the button's `hovered text=fg`; off the plate it
// rests back on `active text=muted`.
test composer_mark_glyph_wears_button_ink
  preset ui_offline
  viewport 360 160
  mount
    box #surface w=fill p=24.0
      ComposerMarks #marks disabled=false
        events
          mark -> open_message_link _
  target code = #surface/marks/root/code
  target glyph = #surface/marks/root/code/glyph
  expect glyph.x > code.x + 4.0
  expect glyph.image_color == color.rgb8(107, 105, 98)
  move (code.x + 2.0) code.center_y
  expect glyph.image_color == color.rgb8(44, 43, 39)
  move (code.x - 8.0) code.center_y
  expect glyph.image_color == color.rgb8(107, 105, 98)

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

      huddle:
        space w=1.0 h=1.0
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

// The launch window's two load-bearing renders: the unlock ceremony's
// password field is reachable, and an empty network list is the welcome
// plate whose one CTA routes to the join flow.
test launch_unlock_contract
  preset ui_launch
  viewport 480 680
  mount
    HubColumn #hub
      with
        step=HubStep.unlock
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
        restore_empty=true
        join_empty=true
      events
        unlock_submit -> unlock_submit _
        login_skip -> login_skip
        create_submit -> create_submit _
        reveal_confirm -> reveal_confirm
        go_restore -> go_restore
        go_login -> go_login
        restore_submit -> restore_submit _
        pick_network -> pick_network _
        open_network_submit -> open_network_submit
        forget_network_submit -> forget_network_submit _ _
        connect_remote_submit -> connect_remote_submit _
        restore_hidden_submit -> restore_hidden_submit
        go_join -> go_join
        go_networks -> go_networks
        join_network_submit -> join_network_submit
        copy_onboarding_invite -> copy_onboarding_invite
        enter_console -> enter_console
  target pw = #hub/root/unlock/root/unlock-password
  expect exists pw
  dispatch go_restore
  expect hub_step == HubStep.restore
  dispatch go_login
  expect hub_step == HubStep.unlock

test launch_networks_empty_contract
  preset ui_launch
  viewport 480 680
  mount
    HubColumn #hub
      with
        step=HubStep.networks
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
        restore_empty=true
        join_empty=true
      events
        unlock_submit -> unlock_submit _
        login_skip -> login_skip
        create_submit -> create_submit _
        reveal_confirm -> reveal_confirm
        go_restore -> go_restore
        go_login -> go_login
        restore_submit -> restore_submit _
        pick_network -> pick_network _
        open_network_submit -> open_network_submit
        forget_network_submit -> forget_network_submit _ _
        connect_remote_submit -> connect_remote_submit _
        restore_hidden_submit -> restore_hidden_submit
        go_join -> go_join
        go_networks -> go_networks
        join_network_submit -> join_network_submit
        copy_onboarding_invite -> copy_onboarding_invite
        enter_console -> enter_console
  target cta = #hub/root/networks/root/join-cta
  expect exists cta
  click cta
  expect hub_step == HubStep.join

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

preset ui_settings_scroll
  state
    shell_tab = ShellTab.settings
    status = "Offline"
    connected = false
    loading = false
    mutation_phase = MutationPhase.idle
    error = ""

// KEYBOARD SCROLL. iced's scrollable answers the wheel and the drag rail only
// — it has no focus and no key handling — so Page Down over Settings moved
// nothing, and neither did Home/End or the arrows on any screen. The whole
// chain is under test: the `status=ignored` key subscription, the
// `content_scroll_step` verdict, and the `scroll-by` operation landing on the
// pane. The mount is the REAL id path the handler names
// (`#workspace-tabs/content/settings/settings-body`) — a scaffold that merely
// imitated that path would stay green while the shipping app stayed dead.
test settings_keyboard_scroll_contract
  preset ui_settings_scroll
  viewport 1120 460
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

      huddle:
        space w=1.0 h=1.0
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
        SettingsScreen account_name_draft<->account_name_draft #settings
          with
            account_name
            network_name
            connected_rpc
            settings_key_state
            settings_key_path
            settings_open_tabs
            members_rows
            members_answered
            account_id
            account_renaming
            account_bound
            account_members
            account_nodes
            appearance
            password
            status
            loading
            connected
            mutation_phase
          events
            select_shell_tab -> select_shell_tab _
            reconnect -> reconnect
            account_name_draft_changed -> account_name_draft_changed _
            account_rename_submit -> account_rename_submit
            copy_to_clipboard -> copy_to_clipboard _ _
            settings_clear_tabs -> settings_clear_tabs
            switch_network -> switch_network
            settings_unlock_submit -> settings_unlock_submit _
            lock_session -> lock_session
            forget_workspace_submit -> forget_workspace_submit
            set_appearance_light -> set_appearance_light
            set_appearance_dark -> set_appearance_dark
      explorer:
        space w=1.0 h=1.0
      palette:
        space w=1.0 h=1.0
      bell:
        space w=1.0 h=1.0
  target body = #workspace-tabs/content/settings/settings-body
  // The scroll handlers qualify their targets with the console window
  // (`window=window_target(console_win)`), so the test first tells the app
  // the harness window IS the console — the same fact `task window open`
  // delivers in the real flow.
  dispatch console_opened(window)
  expect body.content_height > body.visible_height
  expect body.scroll_y ~= 0.0
  key escape
  expect body.scroll_y ~= 0.0
  chord shift page-down
  expect body.scroll_y ~= 0.0
  key page-down
  expect body.scroll_y > 0.0
  key page-up
  expect body.scroll_y ~= 0.0
  key end
  expect body.scroll_y > 0.0
  key home
  expect body.scroll_y ~= 0.0
  // A FOCUSED WIDGET'S KEY NEVER ARRIVES — the `status=ignored` half of the
  // arbitration, and the half that does NOT cover the arrows. Put the caret in
  // Settings' rename field and Home belongs to the field: iced's `text_input`
  // captures it (iced_widget-0.14.2/src/text_input.rs:1119), the subscription
  // never fires, and the scrolled pane holds where it is.
  key end
  expect body.scroll_y > 0.0
  focus #workspace-tabs/content/settings/settings-body/account-rename
  key home
  expect body.scroll_y > 0.0
  // AN ARROW IS NOT THE PANE'S KEY. Same caret — but `text_input` falls Up/Down
  // through to `_ => {}` (text_input.rs:1245) WITHOUT capturing, so these DO
  // arrive and the router itself has to refuse them. Asserted off the top of
  // the pane so a stolen step in either direction moves a visible pixel: this
  // is the press that scrolled the page out from under a live caret.
  scroll-to body 0.0 30.0
  expect body.scroll_y ~= 30.0
  key arrow-down
  expect body.scroll_y ~= 30.0
  key arrow-up
  expect body.scroll_y ~= 30.0
  // …and with nothing focused either. The pane never claims an arrow, because
  // nothing here can tell one meant for a caret from one meant for the page.
  blur
  key arrow-down
  expect body.scroll_y ~= 30.0
  // NOTHING MOVES UNDER A TRANSIENT LAYER. The bell panel is over the content;
  // a Page Down with it open used to scroll the screen BEHIND it.
  dispatch toggle_bell
  key page-down
  expect body.scroll_y ~= 30.0
  key end
  expect body.scroll_y ~= 30.0
  dispatch toggle_bell
  key page-down
  expect body.scroll_y > 30.0

preset ui_explorer
  state
    shell_tab = ShellTab.explorer
    connected = true
    connected_rpc = "http://127.0.0.1:1"
    loading = false
    mutation_phase = MutationPhase.idle
    error = ""

// A PARTIAL ANSWER SAYS SO. Port 1 on loopback cannot hold an unprivileged
// listener, so all six search legs refuse immediately and deterministically.
// Drive the component through its own input and handler instead of seeding its
// now-private state through an app preset.
test explorer_partial_banner_contract
  preset ui_explorer
  viewport 1120 720
  mount
    ExplorerScreen #explorer
      with
        connected_rpc
        connected
        loading
        blocks=explorer_blocks
        ops=explorer_ops
        head=block_height
        sync_line=sync_label(node_phase, node_sync_applied, node_sync_target)
      events
        refresh_explorer -> refresh_explorer
        copy_to_clipboard -> copy_to_clipboard _ _
  target query = #explorer/explorer-search
  target clear = #explorer/explorer-clear
  target banner = #explorer/explorer-partial
  target plate = #explorer/explorer-nothing-matched/root
  click query
  type "needle"
  key enter
  expect exists banner
  expect missing plate
  click clear
  expect query.value == ""
  expect missing banner
  expect missing plate

preset ui_chat_stream
  state
    connected = true
    loading = false
    mutation_phase = MutationPhase.idle
    error = ""
    // THE ENDPOINT IS PINNED BECAUSE AN EMPTY ONE IS NOT INERT. `choose_channel`
    // launches a real `load_channel_window`, and `rpc_client("")` does not
    // refuse — it falls back to `$DUCKTAPE_NODE`, then to the dev box's
    // `~/.ducktape` workspace registry, then to `DEFAULT_RPC`. So this test
    // issued an HTTP request to whatever the machine running it happened to
    // have, and the driver's 10s quiescence budget was the only thing between a
    // slow answer and a red test (`DUCKTAPE_NODE` pointed at a blackhole fails
    // it 100%). Port 1 on loopback can hold no listener — binding below 1024
    // needs root — so the connect is refused immediately and the task settles
    // on the dispatch, deterministically and off the network.
    connected_rpc = "http://127.0.0.1:1"
    shell_tab = ShellTab.chat
    active_channel = "channel-a"
    active_channel_name = "general"
    messages = optimistic_message(messages, "The room she is looking at.", "pending-1")

// THE GATE IS THE STREAM RESET. Every room switch paints an empty loading state,
// so the old scrollable and its offset must disappear before the selected room's
// root window arrives. This asserts that `#chat/message-stream` exists with rows
// and is GONE without them. (Virtualization note:
// offscreen rows leave the a11y tree, so a test that wants a message ROW has to
// scroll it in first. This one only wants the scrollable, which is always
// mounted when it exists at all.)
test message_stream_reset_contract
  preset ui_chat_stream
  viewport 1120 720
  mount
    ChatScreen search_draft<->chat_search_draft message_edit_draft<->message_edit_draft channel_name_draft<->channel_name_draft member_key_draft<->member_key_draft thread_edit_draft<->thread_edit_draft #chat
      with
        endpoint=connected_rpc
        network_name
        status
        block_height
        search_phase=chat_search_phase
        search_query=chat_search_query
        search_hits=chat_search_hits
        rooms
        dm_rows
        channel_create_open
        connected
        loading
        mutation_phase
        active_channel
        active_dm_peer
        active_dm
        active_channel_name
        active_channel_archived
        active_channel_members_only
        channel_members
        post_refusal
        huddle_joined
        huddle_channel
        huddle_channel_name
        huddle_joined_at
        huddle_now
        call_muted
        huddle_popped=false
        messages
        has_older_history
        history_view
        history_loading
        unread_boundary
        unread_marker_seq
        selected_message_seq
        selected_message_rev
        message_action
        channel_settings_open
        active_thread_seq
        thread_target_seq
        thread_messages
        thread_selected_seq
        thread_selected_rev
        thread_message_action
        thread_has_more
        thread_next_reply_seq
        thread_loading
      events
        search_chat_submit -> search_chat_submit
        clear_chat_search -> clear_chat_search
        open_chat_search_hit -> open_chat_search_hit _ _ _
        toggle_channel_create -> toggle_channel_create
        choose_channel -> choose_channel _
        choose_dm -> choose_dm _
        toggle_channel_settings -> toggle_channel_settings
        pop_huddle -> pop_huddle
        focus_huddle -> focus_huddle
        leave_huddle_here -> leave_huddle_here
        huddle_go_channel -> huddle_go_channel
        join_huddle_submit -> join_huddle_submit
        load_more_history -> load_more_history
        chat_scrolled -> chat_scrolled _ _ _ _
        open_message_link -> open_message_link _
        add_reaction_at -> add_reaction_at _ _
        remove_reaction_at -> remove_reaction_at _ _
        open_thread_for -> open_thread_for _
        open_message_actions -> open_message_actions _ _ _
        open_message_reactions -> open_message_reactions _ _ _
        begin_message_edit -> begin_message_edit _ _ _
        arm_message_delete -> arm_message_delete _ _ _
        clear_message_selection -> clear_message_selection
        add_reaction_submit -> add_reaction_submit _
        edit_message_submit -> edit_message_submit
        delete_message_submit -> delete_message_submit
        composer_submitted -> composer_submitted _ _ _
        rename_channel_submit -> rename_channel_submit
        archive_channel_submit -> archive_channel_submit
        unarchive_channel_submit -> unarchive_channel_submit
        add_channel_member_submit -> add_channel_member_submit
        remove_channel_member_submit -> remove_channel_member_submit _
        close_thread -> close_thread
        open_thread_message_actions -> open_thread_message_actions _ _ _
        open_thread_message_reactions -> open_thread_message_reactions _ _ _
        begin_thread_message_edit -> begin_thread_message_edit _ _ _
        arm_thread_message_delete -> arm_thread_message_delete _ _ _
        clear_thread_message_selection -> clear_thread_message_selection
        edit_thread_message_submit -> edit_thread_message_submit
        delete_thread_message_submit -> delete_thread_message_submit
        load_more_thread -> load_more_thread
  target stream = #chat/message-stream
  expect exists stream
  dispatch choose_channel("channel-b")
  expect empty(messages)
  expect missing stream

preset ui_rich_paragraph
  state
    connected = true
    loading = false
    mutation_phase = MutationPhase.idle
    error = ""
    // Port 1 for the same reason as `ui_chat_stream` above: `choose_channel`
    // launches a real `load_channel_window`, and loopback port 1 refuses the
    // connect immediately, so the task settles on the dispatch and the failed
    // load leaves the reducer-set rows alone (`chat_load_failed` touches no
    // `messages`).
    connected_rpc = "http://127.0.0.1:1"
    shell_tab = ShellTab.chat
    active_channel = "channel-a"
    active_channel_name = "general"
    // The optimistic path parses the SAME grammar the send commits, so these
    // rows carry real rich spans — bold, italic, and bare-url link runs —
    // into the paragraph's `for`.
    messages = mark_author_runs(optimistic_message(optimistic_message(messages, "ship the **fix** at https://duck.example/x", "pending-rich"), "and the _second_ line lands", "pending-second"))

// A MESSAGE BODY IS ONE PARAGRAPH (ducktape-ui#639, collected by #1096). The
// span list — plain runs, bold runs, italic runs, links — feeds ONE rich-text
// widget whose `for` expands a span template per run, so the whole line is a
// single drawn run: the exact-match text oracle below only holds while every
// span lands in the same paragraph buffer, marks stripped, spacing intact.
// The second half asserts the expansion FOLLOWS THE DATA, not a first-render
// snapshot: `choose_channel` swaps the span lists out and the paragraphs go
// with them. (The grow direction of a `for` re-expansion is pinned upstream —
// ducktape-ui `rich_text_for.ice` — and is not reachable offline here: every
// committed-row mutation handler guards `seq <= 0`, so a pending fixture row
// cannot be edited, and a failed send removes its own row before the next
// test statement.)
test message_body_renders_as_one_rich_paragraph
  preset ui_rich_paragraph
  viewport 1120 720
  mount
    ChatScreen search_draft<->chat_search_draft message_edit_draft<->message_edit_draft channel_name_draft<->channel_name_draft member_key_draft<->member_key_draft thread_edit_draft<->thread_edit_draft #chat
      with
        endpoint=connected_rpc
        network_name
        status
        block_height
        search_phase=chat_search_phase
        search_query=chat_search_query
        search_hits=chat_search_hits
        rooms
        dm_rows
        channel_create_open
        connected
        loading
        mutation_phase
        active_channel
        active_dm_peer
        active_dm
        active_channel_name
        active_channel_archived
        active_channel_members_only
        channel_members
        post_refusal
        huddle_joined
        huddle_channel
        huddle_channel_name
        huddle_joined_at
        huddle_now
        call_muted
        huddle_popped=false
        messages
        has_older_history
        history_view
        history_loading
        unread_boundary
        unread_marker_seq
        selected_message_seq
        selected_message_rev
        message_action
        channel_settings_open
        active_thread_seq
        thread_target_seq
        thread_messages
        thread_selected_seq
        thread_selected_rev
        thread_message_action
        thread_has_more
        thread_next_reply_seq
        thread_loading
      events
        search_chat_submit -> search_chat_submit
        clear_chat_search -> clear_chat_search
        open_chat_search_hit -> open_chat_search_hit _ _ _
        toggle_channel_create -> toggle_channel_create
        choose_channel -> choose_channel _
        choose_dm -> choose_dm _
        toggle_channel_settings -> toggle_channel_settings
        pop_huddle -> pop_huddle
        focus_huddle -> focus_huddle
        leave_huddle_here -> leave_huddle_here
        huddle_go_channel -> huddle_go_channel
        join_huddle_submit -> join_huddle_submit
        load_more_history -> load_more_history
        chat_scrolled -> chat_scrolled _ _ _ _
        open_message_link -> open_message_link _
        add_reaction_at -> add_reaction_at _ _
        remove_reaction_at -> remove_reaction_at _ _
        open_thread_for -> open_thread_for _
        open_message_actions -> open_message_actions _ _ _
        open_message_reactions -> open_message_reactions _ _ _
        begin_message_edit -> begin_message_edit _ _ _
        arm_message_delete -> arm_message_delete _ _ _
        clear_message_selection -> clear_message_selection
        add_reaction_submit -> add_reaction_submit _
        edit_message_submit -> edit_message_submit
        delete_message_submit -> delete_message_submit
        composer_submitted -> composer_submitted _ _ _
        rename_channel_submit -> rename_channel_submit
        archive_channel_submit -> archive_channel_submit
        unarchive_channel_submit -> unarchive_channel_submit
        add_channel_member_submit -> add_channel_member_submit
        remove_channel_member_submit -> remove_channel_member_submit _
        close_thread -> close_thread
        open_thread_message_actions -> open_thread_message_actions _ _ _
        open_thread_message_reactions -> open_thread_message_reactions _ _ _
        begin_thread_message_edit -> begin_thread_message_edit _ _ _
        arm_thread_message_delete -> arm_thread_message_delete _ _ _
        clear_thread_message_selection -> clear_thread_message_selection
        edit_thread_message_submit -> edit_thread_message_submit
        delete_thread_message_submit -> delete_thread_message_submit
        load_more_thread -> load_more_thread
  target stream = #chat/message-stream
  expect exists stream
  expect text "ship the fix at https://duck.example/x" within stream
  expect text "and the second line lands" within stream
  dispatch choose_channel("channel-b")
  expect no text "ship the fix at https://duck.example/x"
  expect no text "and the second line lands"
  expect missing stream

preset ui_files
  state
    shell_tab = ShellTab.files
    connected = true
    // Pinned for the reason `ui_chat_stream` states at length: an empty
    // endpoint is a fallback, not a refusal, and port 1 on loopback can hold
    // no listener, so anything this screen launches refuses off the network.
    connected_rpc = "http://127.0.0.1:1"
    loading = false
    mutation_phase = MutationPhase.idle
    error = ""

// THE CRUMB AND THE RENAME FIELD ARE TWO ROWS, MEASURED. #804's first live run
// found `/shared` drawn straight over the "new name…" input; the write controls
// are a separate bar under `CrumbBar` now, and this is what keeps them there —
// the field's top edge sits at or below the crumb bar's bottom, with both boxes
// proven to have really drawn first so the ordering cannot pass on a collapsed
// one. 965 wide is the narrowest content pane this console can produce: the
// window declares `min-size 1040`, less the 74px rail and the 1px rule.
test files_write_bar_clears_the_crumb_bar
  preset ui_files
  viewport 965 720
  mount
    FilesScreen new_name<->fs_new_name draft<->fs_editor #files
      with
        path=fs_path
        listed=(fs_listed_path == fs_path)
        entries=fs_entries
        directories=fs_directories(fs_entries)
        connected
        loading=fs_loading
        preview_path=fs_preview_path
        preview_entry=fs_preview_entry
        delete_target=fs_delete_target
        diff_from=fs_diff_from
        diff=fs_diff
        history=fs_history
        preview_truncated=fs_preview_truncated
        preview_binary=fs_preview_binary
        editing=fs_editing
        preview_text=fs_preview_text
        dark=false
        preview_picture=fs_preview_picture
        preview_width=fs_preview_width
        preview_height=fs_preview_height
      events
        open_message_link -> open_message_link _
        fs_open_dir -> fs_open_dir _
        fs_open_file -> fs_open_file _
        fs_open_parent -> fs_open_parent
        fs_new_name_changed -> fs_new_name_changed _
        fs_mkdir_submit -> fs_mkdir_submit
        fs_new_file_submit -> fs_new_file_submit
        fs_arm_delete -> fs_arm_delete _
        fs_disarm_delete -> fs_disarm_delete
        fs_delete_submit -> fs_delete_submit
        fs_close_diff -> fs_close_diff
        fs_show_diff -> fs_show_diff _
        fs_begin_edit -> fs_begin_edit
        fs_cancel_edit -> fs_cancel_edit
        fs_save_edit -> fs_save_edit
  target crumb = #files/crumb/root
  target field = #files/fs-new
  expect text "/shared" within crumb
  expect crumb.height > 40.0
  expect field.width ~= 160.0
  expect field.y >= crumb.bottom

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
        dock_huddle -> dock_huddle
        huddle_go_channel -> huddle_go_channel
        leave_huddle_here -> leave_huddle_here
        toggle_call_mute -> toggle_call_mute
        toggle_call_camera -> toggle_call_camera
        toggle_call_screen -> toggle_call_screen
  target panel = #huddle/root
  target share = #huddle/root/share-stop
  target leave = #huddle/root/leave
  expect call_sharing
  expect leave.x + leave.width <= panel.x + panel.width
  expect share.width ~= 32.0
  capture huddle_sharing_light
