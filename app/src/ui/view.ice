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
          hidden=hub_hidden
          name=onboarding_name
          invite=invite_link
          steps=provision_steps
          step_index=provision_index
          height=block_height
          tier=member_tier(members_rows)
          error=onboarding_error
          busy=mutation_busy
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
          forget_network_submit -> forget_network_submit _ _
          go_join -> go_join
          go_networks -> go_networks
          go_wallets -> go_wallets
          join_network_submit -> join_network_submit
          copy_onboarding_invite -> copy_onboarding_invite
          connect_remote_submit -> connect_remote_submit _
          restore_hidden_submit -> restore_hidden_submit
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
          approvals=open_proposals(gov_rows)
          account=account_name
          agent_live=any_agent_active(agents_rows)
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
                        h=26.0
                        p=5.0
                        @ghost_action
                      active bg=transparent text=muted r=7.0
                      hovered bg=fg/9 text=fg
                      pressed bg=fg/14
        chat:
          ChatScreen search_draft<->chat_search_draft message_edit_draft<->message_edit_draft channel_name_draft<->channel_name_draft member_key_draft<->member_key_draft thread_edit_draft<->thread_edit_draft #chat
            with
              endpoint=connected_rpc
              network_name
              network_chain_id
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
              messages
              has_older_history
              history_view
              at_live_tail=chat_at_tail
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
              copy_anchor_seq
              copy_head_seq
              copy_surface
            events
              search_chat_submit -> search_chat_submit
              clear_chat_search -> clear_chat_search
              open_chat_search_hit -> open_chat_search_hit _ _ _
              toggle_channel_create -> toggle_channel_create
              choose_channel -> choose_channel _
              choose_dm -> choose_dm _
              toggle_channel_settings -> toggle_channel_settings
              show_huddle -> show_huddle
              leave_huddle_here -> leave_huddle_here
              huddle_go_channel -> huddle_go_channel
              join_huddle_submit -> join_huddle_submit
              load_more_history -> load_more_history
              chat_scrolled -> chat_scrolled _ _ _ _
              open_message_link -> open_message_link _
              copy_to_clipboard -> copy_to_clipboard _ _
              copy_message_link -> copy_message_link _
              add_reaction_at -> add_reaction_at _ _
              remove_reaction_at -> remove_reaction_at _ _
              open_thread_for -> open_thread_for _
              open_message_actions -> open_message_actions _ _ _
              open_message_reactions -> open_message_reactions _ _ _
              begin_message_edit -> begin_message_edit _ _ _
              arm_message_delete -> arm_message_delete _ _ _
              clear_message_selection -> clear_message_selection
              press_message -> press_message _ _
              clear_copy_range -> clear_copy_range
              copy_selected_messages -> copy_selected_messages
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

        shell:
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
              connected
              dark
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

        // Pages is a MODULE-OWNED VIEW: the sidebar, the header, the tab
        // strip and the comments rail go in as props; the document is the
        // app's editor, painted into the view's slot by the host.
        pages:
          extern pages_view(dark, connected, loading, mutation_phase, network_chain_id, pages, page_create_open, page_draft, block_comment_draft, pages_seed_rev, active_page, active_page_title, active_page_parent, page_searching, page_search_hits, page_search_query, page_delete_armed, block_autosave_status, page_refusal, doc_tabs, blocks, commented_block_hits, caret_comment_target, active_thread_anchor, orphaned_comment_drafts, page_editor, block_comments_open, block_comment_thread_total, block_comment_threads, block_comment_rows, block_comment_threads_loading, block_comment_threads_has_more, active_block_comment_thread, block_thread_comments, block_thread_comments_loading, block_thread_comments_has_more) #pages -> pages_view_event _

        // Files is a MODULE-OWNED VIEW: the listing, the preview, the
        // history and the write refusal go in as props; every navigation
        // and every write comes back as an intent the handler signs. Whether
        // the rows on hand describe the path in the crumb (`listed`) is
        // computed here, once.
        files:
          extern files_view(dark, connected, fs_path, fs_listed_path == fs_path, fs_entries, fs_loading, fs_preview_path, fs_preview_entry, fs_delete_target, fs_diff_from, fs_diff, fs_history, fs_preview_truncated, fs_preview_binary, fs_preview_picture, fs_preview_width, fs_preview_height, fs_preview_text, files_write_gate(fs_path, settings_user_key), fs_writes) #files -> files_view_event _
        members:
          extern members_view(dark, connected, members_is_admin(members_rows), members_answered, members_rows) #members -> members_view_event _
        agents:
          // the agents view declares no intents, so nothing ever arrives on
          // this route; the extern needs one and the roster handler is the
          // honest destination
          extern agents_view(dark, connected, agents_answered, agents_rows) #agents -> members_view_event _
        // Forge is a MODULE-OWNED VIEW: the register, the open repo and item,
        // the code browse's listing and file, and the discussion go in as
        // props; every act comes back as an intent the handler signs. The
        // note composer is the app's own, docked under the view while an
        // item is open — the editor it edits cannot cross the wire.
        forge:
          col w=fill h=fill
            extern forge_view(dark, connected, network_name, account_bio, member_tier(members_rows), network_chain_id, connected_rpc, forge_repos, forge_list_phase, forge_repo, forge_repo_menu, forge_repo_phase, forge_branches, forge_tab, forge_items, forge_item_number, forge_item_phase, forge_item_kind, forge_item_title, forge_item_state, forge_item_author, forge_item_branches, forge_item_body, forge_item_blocks, forge_item_files_changed, forge_item_additions, forge_item_deletions, forge_item_diff, forge_item_diff_truncated, forge_item_merge_oid, forge_item_source_oid, forge_item_approvals, forge_item_change_requests, forge_item_reviews, forge_merge_conflicts, forge_merge_busy, forge_review_verdict, forge_review_busy, forge_comment_staged, forge_discussion, forge_linked_note, forge_landed_seq, forge_landed_tick, forge_tree_path, forge_tree_rev, forge_tree_entries, forge_tree_born, forge_tree_truncated, forge_tree_phase, forge_file_path, forge_file_text, forge_file_binary, forge_file_truncated, forge_file_picture, forge_file_width, forge_file_height, forge_file_note, forge_file_header(forge_opened_dir, forge_opened_rev, forge_tree_path, forge_tree_rev, forge_file_path), forge_file_phase, forge_drafts_cleared, forge_drafts_scope) #forge -> forge_view_event _
            if connected && forge_item_number > 0 && forge_item_phase == ForgePhase.ready
              box
                with
                  w=fill
                  pl=18.0
                  pr=18.0
                  pt=8.0
                  pb=12.0
                flex
                  with
                    w=fill
                    gap=8.0
                    items=end
                  box
                    with
                      w=fill
                      bg=surface
                      border=card_line
                      border-w=1.0
                      r=8.0
                      clip=true
                    extern rich_composer(forge_discussion_editor, "Write a note…", (loading || !connected || empty(forge_item_channel)), 38.0, 120.0, 6.0) #forge-note -> forge_composer_event _
                  button "Send" -> forge_composer_event(composer_submit_event())
                    with
                      disabled=(loading || !connected || empty(forge_item_channel) || !empty(forge_discussion_pending) || empty(trim(editor_text(forge_discussion_editor))))
                      w=60.0
                      h=28.0
                      p=6.0
                      @primary_action
        // Approvals is a MODULE-OWNED VIEW: the register the app holds goes
        // in as props, and what the reader does comes back as an intent the
        // handler below signs — the guest sees no key and no endpoint.
        governance:
          extern governance_view(dark, connected, members_is_admin(members_rows), gov_answered, gov_voting, gov_rows) #governance -> governance_view_event _
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
          extern settings_view(dark, connected, loading, status, mutation_phase, appearance, desktop_notifications, password, account_name, network_name, connected_rpc, account_ceremony_phase, account_ceremony_qr, account_ceremony_detail, account_ceremony_left, settings_key_state, settings_key_path, settings_open_tabs, members_rows, members_answered, account_number, account_renaming, account_exists, account_keys, account_key_rows, account_busy, account_ticket, settings_drafts_cleared, settings_drafts_scope) #settings -> settings_view_event _
        // The Explorer is a MODULE-OWNED VIEW: the ledger and the answer to
        // the last search go in as props; a refresh, a search, its clearing
        // and a copy come back as intents the handler acts on.
        explorer:
          extern explorer_view(dark, connected, explorer_loading, explorer_blocks, explorer_ops, block_height, sync_label(node_phase, node_sync_applied, node_sync_target), explorer_hits, explorer_kinds, explorer_partial, explorer_searching, explorer_sent_query) #explorer -> explorer_view_event _
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
                        button "Mark all read" -> mark_bell_read_submit
                          with
                            disabled=(bell_unread <= 0)
                            h=22.0
                            p=4.0
                            @ghost_action
                          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                          hovered bg=elevated text=brand
                          pressed bg=subtle text=brand
                    box
                      with
                        w=fill
                        h=1.0
                        bg=separator
                      space w=1.0 h=1.0
                    if empty(bell_items)
                      box
                        with
                          w=fill
                          p=26.0
                          align-x=center
                        text "Nothing yet — mentions and deliveries land here." size=12.0 @text-meta
                    if !empty(bell_items)
                      scroll
                        with
                          dir=vertical
                          w=fill
                          h=290.0
                          anchor-y=keep
                        keyed item in bell_items by=item.seq
                          with
                            virtual-row=58.0
                            w=fill
                            p=5.0
                            gap=1.0
                          BellRow item=item
