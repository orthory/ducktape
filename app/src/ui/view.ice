view
  // THE WINDOW GATE — the daemon's one dispatch. The `window` binding names
  // the window being rendered: the launch window mounts the hub column, the
  // console window mounts the shell, the popped huddle mounts the panel. A
  // window no id claims renders the hub's quiet loading arm — it exists only
  // between open and register.
  col w=fill h=fill
    if (console_win != some(window)) && (huddle_win != some(window))
      HubColumn name_draft<->welcome_name_draft
        with
          step=hub_step
          network=network_name
          phase=ceremony_phase
          qr=ceremony_qr
          detail=ceremony_detail
          left=ceremony_left
          wallets=hub_wallets
          wallet_selected=hub_wallet_selected
          networks=hub_networks
          selected=hub_selected
          name=onboarding_name
          invite=invite_link
          steps=provision_steps
          step_index=provision_index
          height=block_height
          tier=member_tier(members_rows)
          error=onboarding_error
          busy=hub_busy
          restore_empty=empty(restore_words)
          join_empty=empty(join_invite)
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
          go_join -> go_join
          go_networks -> go_networks
          join_network_submit -> join_network_submit
          copy_onboarding_invite -> copy_onboarding_invite
          connect_remote_submit -> connect_remote_submit _
          enter_console -> enter_console
        restore_phrase:
          input "" #restore-words <-> restore_words
            with
              label="Recovery phrase"
              hint="24 words, space-separated"
              disabled=mutation_busy
              w=fill
              p=0.0
              text-size=12.0
              line-h=1.2
              font=code
              @control
            active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
            disabled value=hint
        join_invite:
          input "" #join-invite <-> join_invite
            with
              label="Invite"
              hint="🦆AAAA…"
              disabled=mutation_busy
              submit=join_network_submit
              w=fill
              p=0.0
              text-size=12.0
              line-h=1.2
              font=code
              @control
            active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
            disabled value=hint
    // THE HUDDLE WINDOW — the same panel, now the whole content of a real OS
    // window instead of a card wearing drawn traffic lights. Its close button
    // only closes it (see `window_was_closed`); leaving the huddle closes it too.
    if huddle_win == some(window)
      HuddlePanel #huddle
        with
          channel=huddle_channel_name
          elapsed=mmss(huddle_now - huddle_joined_at)
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
    if console_win == some(window)
      WorkspaceTabs wall_now=wall_now #workspace-tabs
        with
          network=network_name
          status
          height=block_height
          sync_line=sync_label(node_phase, node_sync_applied, node_sync_target)
          loading=(loading || mutation_busy)
          degraded=connection_degraded(status)
          tab=shell_tab
          bell_count=bell_unread
          bell_sev=bell_worst_severity(bell_items)
          approvals=gov_open
          account=account_name
          agent_live=agents_live
          tier=member_tier(members_rows)
          answered=members_answered
          root_hash=node_root_hash
          consensus_view=node_view_label
          quorum=node_quorum_label
          reachable=node_reachable_label
          last_finalized=node_last_finalized
        events
          select_shell_tab -> select_shell_tab _
          toggle_bell -> toggle_bell
          switch_network -> switch_network
        notice:
          col w=fill
            AccountBanner #account-banner
              with
                connected
                account_exists
                dismissed=account_banner_dismissed
                password
              events
                open_account_welcome -> open_account_welcome
                dismiss_account_banner -> dismiss_account_banner
            if has_error
              box
                with
                  w=fill
                  pl=12.0
                  pr=12.0
                  pb=8.0
                box
                  with
                    w=fill
                    p=8.0
                    bg=danger_bg
                    border=danger_line
                    border-w=1.0
                    r=12.0
                  row
                    with
                      w=fill
                      gap=8.0
                      align=center
                    box
                      with
                        w=20.0
                        h=20.0
                        align-x=center
                        align-y=center
                        bg=danger_dot
                        r=10.0
                      text "!"
                        with
                          size=14.0
                          font=medium
                          @text-danger_fg
                    text error
                      with
                        w=fill
                        size=13.5
                        @text-fg
                    button "Dismiss" -> dismiss_error
                      with
                        p=5.0
                        @ghost_action
                      active bg=transparent text=muted r=7.0
                      hovered bg=fg/9 text=fg
                      pressed bg=fg/14
        // Chat is a MODULE-OWNED VIEW: the facts go in as props — the mutation
        // lock as a flag, the enums by name — and every act comes back as an
        // intent the handler signs. The drafts are the view's; the composers
        // are host surfaces the view leaves slots for (module_view.rs).
        chat:
          extern chat_view(dark, connected_rpc, network_name, network_chain_id, status, block_height, chat_search_phase, chat_search_query, chat_search_hits, rooms, dm_rows, channel_create_open, connected, loading, mutation_phase, active_channel, active_dm_peer, active_dm, active_channel_name, active_channel_archived, active_channel_members_only, channel_members, post_refusal, huddle_joined, huddle_channel, huddle_channel_name, huddle_joined_at, huddle_now, call_muted, messages, has_older_history, history_view, chat_at_tail, history_loading, unread_boundary, unread_marker_seq, selected_message_seq, selected_message_rev, message_action, channel_settings_open, active_thread_seq, thread_target_seq, thread_messages, thread_selected_seq, thread_selected_rev, thread_message_action, thread_has_more, thread_next_reply_seq, thread_loading, copy_anchor_seq, copy_head_seq, copy_surface, chat_sent_serial, live_agents) #chat -> chat_view_event _

        // Pages is a MODULE-OWNED VIEW: the sidebar, the header, the tab
        // strip and the comments rail go in as props; the document is the
        // app's editor, painted into the view's slot by the host.
        pages:
          extern pages_view(dark, connected, loading, mutation_phase, network_chain_id, pages, page_create_open, page_draft, block_comment_draft, pages_seed_rev, active_page, active_page_title, active_page_parent, page_searching, page_search_hits, page_search_query, page_delete_armed, block_autosave_status, page_refusal, blocks, commented_block_hits, caret_comment_target, active_thread_anchor, orphaned_comment_drafts, page_text, buffer_page, block_comments_open, block_comment_thread_total, block_comment_threads, block_comment_rows, block_comment_threads_loading, block_comment_threads_has_more, active_block_comment_thread, block_thread_comments, block_thread_comments_loading, block_thread_comments_has_more) #pages -> pages_view_event _

        // Files is a MODULE-OWNED VIEW on the KERNEL CONTRACT: session facts
        // go in — the chain among them, because a draft belongs to the
        // network it was read on — and the view lists the directory, reads
        // the preview and the history, and writes through `op.submit` for
        // itself. The pictures, the highlighted reader and the Markdown
        // document are the app's surfaces, painted into the slots it leaves.
        files:
          extern files_view(dark, connected, network_chain_id, fs_route, fs_route_serial) #files -> files_view_event _
        // Members is a MODULE-OWNED VIEW on the KERNEL CONTRACT: session
        // facts go in, the view reads the roster off the node itself and
        // writes through `op.submit` (signed with the seated key). The
        // app's own `members_rows` stays — it is the SESSION fact of who
        // this node is on this network, which the rail, the forge gate and
        // the approvals gate all read.
        members:
          extern members_view(dark, connected, members_is_admin(members_rows)) #members -> members_view_event _
        agents:
          // Agents is a VIEW ON THE KERNEL CONTRACT: session facts go in —
          // the signing account and the run another tab opened for the
          // reader — and the view reads its own register and signs its own
          // writes through `op.submit`. What comes back is its working
          // count, a registration, and two navigations.
          extern agents_view(dark, connected, account_number, agents_open_run, agents_opened) #agents -> agents_view_event _
        // Forge is a MODULE-OWNED VIEW on the KERNEL CONTRACT: session facts
        // go in — the network's name and chain id, the endpoint, and the
        // `duck://forge/...` the open plane routed here — and the view reads
        // its repos, tracker, patches, reviews, discussion and code browse
        // itself, writing through `op.submit`. What comes back is a
        // clipboard write, a link to open, or a note written in the host
        // composer it docked over the item's own channel.
        forge:
          extern forge_view(dark, connected, network_name, account_bio, member_tier(members_rows), network_chain_id, connected_rpc, forge_link, forge_link_tick) #forge -> forge_view_event _
        // Approvals is a MODULE-OWNED VIEW on the KERNEL CONTRACT: session
        // facts go in, the view reads its own register and writes through
        // `op.submit` (signed with the seated key), and the one event back
        // is the tab badge — the guest sees no key and no endpoint.
        governance:
          extern governance_view(dark, connected, members_is_admin(members_rows)) #governance -> governance_view_event _
        // Node is a MODULE-OWNED VIEW too: the facts the app holds go in as
        // props; the tab, the log filter and a clipboard copy come back as
        // intents. The live log ring stays native — the view leaves a slot
        // the app paints from `node_log_timeline` (module_view.rs).
        node:
          extern node_view(dark, connected, members_is_admin(members_rows), member_tier(members_rows), status, loading, module_rows, node_key, node_data_dir, node_height, node_checkpoint, node_last_finalized, node_reachable_label, node_quorum_label, node_version, node_root_hash, sync_label(node_phase, node_sync_applied, node_sync_target), node_phase_since, node_sync_retries, node_sync_failures, node_sync_last_error, node_peers, wall_now, node_log_timeline, connected_rpc) #node -> node_view_event _
        // Settings is a MODULE-OWNED VIEW too: the facts go in as props — the
        // signing seat as a flag, never the password — and every act comes
        // back as an intent the handler signs. The drafts are the view's.
        settings:
          extern settings_view(dark, connected, loading, status, mutation_phase, appearance, desktop_notifications, password, settings_user_key, account_name, network_name, connected_rpc, account_ceremony_phase, account_ceremony_qr, account_ceremony_detail, account_ceremony_left, settings_key_state, settings_key_path, account_number, account_exists, account_busy, account_ticket) #settings -> settings_view_event _
        // The Explorer is a MODULE-OWNED VIEW on the KERNEL CONTRACT: session
        // facts go in — the live head and the sync line among them, because
        // they are the titlebar's own readings and a second source would
        // disagree with it — and the view reads the block window and runs its
        // workspace search through the kernel. A clipboard copy is the one
        // intent that comes back.
        explorer:
          extern explorer_view(dark, connected, block_height, sync_label(node_phase, node_sync_applied, node_sync_target)) #explorer -> explorer_view_event _
        palette:
          OverlayLayer draft<->channel_draft query<->palette_draft #overlays
            with
              create_open=channel_create_open
              members_only=channel_create_members_only
              busy=mutation_busy
              connected
              loading
              toast
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
        bell:
          stack w=fill h=fill
            if bell_open
              button -> close_bell
                with
                  label="Close notifications"
                  w=fill
                  h=fill
                  p=0.0
                  @icon_action
                space w=fill h=fill
                active bg=transparent border=transparent
            if bell_open
              box
                with
                  w=fill
                  h=fill
                  align-x=end
                  align-y=start
                  pt=44.0
                  pr=13.0
                box
                  with
                    w=342.0
                    bg=surface
                    border=border
                    border-w=1.0
                    r=13.0
                    clip=true
                    shadow=shadow_modal
                    shadow-y=16.0
                    shadow-blur=40.0
                  col w=fill
                    box
                      with
                        w=fill
                        pl=13.0
                        pr=13.0
                        pt=11.0
                        pb=9.0
                      row
                        with
                          w=fill
                          gap=8.0
                          align=center
                        text "Alerts"
                          with
                            size=12.5
                            wrap=none
                            @text-primary
                        // NOT `0 unread` OVER "Nothing yet". The panel below
                        // already says the inbox is empty; a zero beside it is
                        // the same nothing said twice, and louder. `Mark all
                        // read` is already gated the same way.
                        if bell_unread > 0
                          text count_label(bell_unread)
                            with
                              size=10.5
                              wrap=none
                              font=code_medium
                              @text-meta
                        if bell_unread > 0
                          text "unread"
                            with
                              size=12.5
                              wrap=none
                              @text-meta
                        space w=fill
                        button "Mark all read" #mark-bell-read -> mark_bell_read_submit
                          with
                            disabled=(bell_unread <= 0 || bell_marking)
                            p=4.0
                            @ghost_action text-11px leading-snug font-medium
                          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                          hovered bg=elevated text=brand
                          pressed bg=subtle text=brand
                    box
                      with
                        w=fill
                        h=1.0
                        bg=separator
                      space w=1.0 h=1.0
                    if !empty(bell_error)
                      col gap=4.0 p=9.0
                        text bell_error size=12.0 @text-danger
                        button "Retry" @ghost_action -> reload_bell
                    if empty(bell_visible_items(bell_items, account_number, settings_user_key))
                      box
                        with
                          w=fill
                          p=26.0
                          align-x=center
                        text "Nothing yet — mentions and deliveries land here." size=12.0 @text-meta
                    if !empty(bell_visible_items(bell_items, account_number, settings_user_key))
                      scroll
                        with
                          dir=vertical
                          w=fill
                          h=290.0
                          anchor-y=keep
                        keyed item in bell_visible_items(bell_items, account_number, settings_user_key) by=item.seq
                          with
                            w=fill
                            p=5.0
                            gap=1.0
                          button #open-notification -> bell_open_item(connect_generation, account_number, bell_presentation(item, bell_presentations))
                            with
                              label=bell_label(item, bell_presentations)
                              w=fill
                              p=0.0
                              disabled=!bell_openable(item, bell_presentations)
                              @ghost_action
                            BellRow item=item context=bell_presentation(item, bell_presentations)
