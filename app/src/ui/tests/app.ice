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

preset ui_settings_scroll
  state
    shell_tab = "settings"
    status = "Offline"
    connected = false
    loading = false
    mutation_phase = "idle"
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
        SettingsScreen wall_now=wall_now account_name_draft<->account_name_draft node_log_filter<->node_log_filter #settings
          with
            account_name
            network_name
            connected_rpc
            settings_endpoint
            settings_node_key
            node_height
            settings_data_dir
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
            node_tab
            module_rows
            node_checkpoint
            node_last_finalized
            node_reachable_label
            node_quorum_label
            node_version
            node_root_hash
            sync_line=sync_label(node_phase, node_sync_applied, node_sync_target)
            node_phase_since
            node_sync_retries
            node_sync_failures
            node_sync_last_error
            node_peers
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
            select_node_tab -> select_node_tab _
            open_node_modules -> open_node_modules
            node_log_filter_changed -> node_log_filter_changed _
            set_appearance_light -> set_appearance_light
            set_appearance_dark -> set_appearance_dark
          activity_log:
            extern node_log_timeline(node_log_timeline, settings_endpoint) #node-log-timeline -> node_log_timeline_changed _
      explorer:
        space w=1.0 h=1.0
      palette:
        space w=1.0 h=1.0
      bell:
        space w=1.0 h=1.0
  target body = #workspace-tabs/content/settings/settings-body
  target overview_tab = #workspace-tabs/content/settings/settings-body/node-overview-tab
  target activity_tab = #workspace-tabs/content/settings/settings-body/node-activity-tab
  expect body.content_height > body.visible_height
  expect a11y overview_tab checked true
  expect a11y activity_tab checked false
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

preset ui_explorer_partial
  state
    shell_tab = "explorer"
    connected = true
    loading = false
    mutation_phase = "idle"
    error = ""
    explorer_query = "needle"
    explorer_searching = false
    explorer_kind = "all"
    explorer_partial = "Code did not answer — these results are incomplete."

preset ui_explorer_whole
  state
    shell_tab = "explorer"
    connected = true
    loading = false
    mutation_phase = "idle"
    error = ""
    explorer_query = "needle"
    explorer_searching = false
    explorer_kind = "all"
    explorer_partial = ""

// A PARTIAL ANSWER SAYS SO, AND ONLY A WHOLE ONE CLAIMS EMPTY. Each of the six
// sources behind a workspace search fails silently and the node's per-module
// cold start runs tens of seconds against a 30s client ceiling, so a search
// that reached the node and lost three of its sources rendered the survivors as
// the whole truth — and with no survivors it printed "Nothing matched that
// query in this workspace", which is a claim about the WORKSPACE that only the
// sources that answered can support.
test explorer_partial_banner_contract
  preset ui_explorer_partial
  viewport 1120 720
  mount
    ExplorerScreen query<->explorer_query #explorer
      with
        connected
        searching=explorer_searching
        loading
        kinds=explorer_kinds
        kind=explorer_kind
        partial=explorer_partial
        hits=explorer_hits
        blocks=explorer_blocks
        selected=explorer_selected
        ops=explorer_ops
        head=block_height
        sync_line=sync_label(node_phase, node_sync_applied, node_sync_target)
      events
        explorer_search_submit -> explorer_search_submit
        clear_explorer_search -> clear_explorer_search
        refresh_explorer -> refresh_explorer
        pick_explorer_kind -> pick_explorer_kind _
        select_explorer_block -> select_explorer_block _
  target banner = #explorer/explorer-partial
  target plate = #explorer/explorer-nothing-matched/root
  expect exists banner
  expect missing plate

// The other half, which is what stops the banner from being wallpaper: with
// every source answered there is nothing to warn about, and "nothing matched"
// is then the honest line.
test explorer_whole_answer_contract
  preset ui_explorer_whole
  viewport 1120 720
  mount
    ExplorerScreen query<->explorer_query #explorer
      with
        connected
        searching=explorer_searching
        loading
        kinds=explorer_kinds
        kind=explorer_kind
        partial=explorer_partial
        hits=explorer_hits
        blocks=explorer_blocks
        selected=explorer_selected
        ops=explorer_ops
        head=block_height
        sync_line=sync_label(node_phase, node_sync_applied, node_sync_target)
      events
        explorer_search_submit -> explorer_search_submit
        clear_explorer_search -> clear_explorer_search
        refresh_explorer -> refresh_explorer
        pick_explorer_kind -> pick_explorer_kind _
        select_explorer_block -> select_explorer_block _
  target banner = #explorer/explorer-partial
  target plate = #explorer/explorer-nothing-matched/root
  expect missing banner
  expect exists plate

preset ui_chat_stream
  state
    connected = true
    loading = false
    mutation_phase = "idle"
    error = ""
    shell_tab = "chat"
    active_channel = "channel-a"
    active_channel_name = "general"
    messages = optimistic_message(messages, "The room she is looking at.", "pending-1")

// THE SCROLL RESET IS THE `!empty(messages)` GATE. Nothing in the app calls
// `scroll-to` on the timeline and nothing remembers a per-channel offset:
// `choose_channel` empties `messages`, the gate around the stack goes false,
// the scrollable unmounts, its offset dies with it, and the next room mounts a
// fresh one at the tail. That is load-bearing behaviour resting on a widget's
// LIFETIME, which is exactly the kind of thing a later "stop remounting the
// scrollable" cleanup deletes without noticing — it would hand the new room the
// offset the old one was left at, mid-scrollback.
//
// So this asserts the lifetime itself: `#chat/message-stream` exists with rows
// and is GONE without them. (Virtualization note: offscreen rows leave the a11y
// tree, so a test that wants a message ROW has to scroll it in first. This one
// only wants the scrollable, which is always mounted when it exists at all.)
test message_stream_reset_contract
  preset ui_chat_stream
  viewport 1120 720
  mount
    ChatScreen search_draft<->chat_search_draft message_action_focus<->message_action_focus message_edit_draft<->message_edit_draft message_editor<->message_editor channel_name_draft<->channel_name_draft member_key_draft<->member_key_draft thread_edit_draft<->thread_edit_draft reply_editor<->reply_editor #chat
      with
        network_name
        status
        block_height
        search_phase=chat_search_phase
        search_hits=chat_search_hits
        rooms
        unread_channel_ids
        dm_peers
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
        send_flash_id
        send_flash_value=0.0
        message_action
        message_menu_y
        failed_message_draft
        channel_settings_open
        active_thread_seq
        thread_target_seq
        thread_messages
        thread_selected_seq
        thread_selected_rev
        thread_message_action
        thread_menu_y
        thread_send_flash_id
        thread_has_more
        thread_next_reply_offset
        thread_loading
        failed_reply_draft
        shift_held
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
        chat_pointer_pressed -> chat_pointer_pressed _ _
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
        restore_failed_message -> restore_failed_message
        dismiss_failed_message -> dismiss_failed_message
        composer_event -> chat_composer_event _
        composer_mark -> composer_mark _
        rename_channel_submit -> rename_channel_submit
        archive_channel_submit -> archive_channel_submit
        unarchive_channel_submit -> unarchive_channel_submit
        add_channel_member_submit -> add_channel_member_submit
        remove_channel_member_submit -> remove_channel_member_submit _
        thread_pointer_pressed -> thread_pointer_pressed _ _
        close_thread -> close_thread
        open_thread_message_actions -> open_thread_message_actions _ _ _
        open_thread_message_reactions -> open_thread_message_reactions _ _ _
        begin_thread_message_edit -> begin_thread_message_edit _ _ _
        arm_thread_message_delete -> arm_thread_message_delete _ _ _
        clear_thread_message_selection -> clear_thread_message_selection
        edit_thread_message_submit -> edit_thread_message_submit
        delete_thread_message_submit -> delete_thread_message_submit
        load_more_thread -> load_more_thread
        restore_failed_reply -> restore_failed_reply
        dismiss_failed_reply -> dismiss_failed_reply
        reply_composer_event -> reply_composer_event _
        reply_composer_mark -> reply_composer_mark _
        chat_resized -> chat_resized _ _
        thread_resized -> thread_resized _ _
  target stream = #chat/message-stream
  expect exists stream
  dispatch choose_channel("channel-b")
  expect empty(messages)
  expect missing stream
