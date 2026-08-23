view
  // THE WINDOW GATE — the daemon's one dispatch. The `window` binding names
  // the window being rendered: the launch window mounts the hub column, the
  // console window mounts the shell, the popped huddle mounts the panel. A
  // window no id claims renders the hub's quiet loading arm — it exists only
  // between open and register.
  col w=fill h=fill
    if (console_win != some(window)) && (huddle_win != some(window))
      HubColumn
        with
          step=hub_step
          key_state=hub_key_state
          networks=hub_networks
          selected=hub_selected
          hidden=hub_hidden
          name=onboarding_name
          invite=invite_link
          reveal=reveal_words
          steps=provision_steps
          step_index=provision_index
          height=block_height
          tier=member_tier(members_rows)
          error=onboarding_error
          busy=mutation_busy
          restore_empty=empty(restore_words)
          join_empty=empty(join_invite)
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
          go_join -> go_join
          go_networks -> go_networks
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
    // docks (see `window_was_closed`); leaving the huddle closes it.
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
          dock_huddle -> dock_huddle
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
              huddle_popped
              messages
              messages_revision
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
              thread_messages_revision
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

        pages:
          PagesScreen page_draft<->page_draft page_search_draft<->page_search_draft page_editor<->page_editor block_comment_draft<->block_comment_draft #pages
            with
              pages
              page_create_open
              loading
              mutation_phase
              connected
              connected_rpc
              password
              dark
              active_page
              active_page_title
              active_page_parent
              page_searching
              page_search_hits
              page_search_query
              page_delete_armed
              block_autosave_status
              page_refusal
              doc_tabs
              blocks
              commented_block_hits
              caret_comment_target
              active_thread_target
              active_thread_anchor
              orphaned_comment_drafts
              block_comments_open
              block_comment_thread_total
              block_comment_threads
              block_comment_rows
              block_comment_threads_loading
              block_comment_threads_has_more
              active_block_comment_thread
              block_thread_comments
              block_thread_comments_loading
              block_thread_comments_has_more
            events
              toggle_page_create -> toggle_page_create
              create_page_submit -> create_page_submit
              choose_page -> choose_page _
              search_pages_submit -> search_pages_submit
              clear_page_search -> clear_page_search
              arm_page_delete -> arm_page_delete
              disarm_page_delete -> disarm_page_delete
              delete_page_submit -> delete_page_submit
              close_doc_tab -> close_doc_tab _
              open_page_search_hit -> open_page_search_hit _ _
              use_orphaned_comment_draft -> use_orphaned_comment_draft _
              discard_orphaned_comment_draft -> discard_orphaned_comment_draft _
              page_edited -> page_edited _
              toggle_block_comments -> toggle_block_comments
              close_block_comments -> close_block_comments
              open_block_comment_thread -> open_block_comment_thread _ _
              load_more_block_threads -> load_more_block_threads
              close_block_comment_thread -> close_block_comment_thread
              load_more_block_comments -> load_more_block_comments
              post_block_comment_submit -> post_block_comment_submit
              resolve_thread_submit -> resolve_thread_submit _

        files:
          FilesScreen new_name<->fs_new_name draft<->fs_editor
            with
              path=fs_path
              // Do the rows on hand describe the path in the crumb? Every
              // reading of `entries` on that screen is gated on this.
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
              preview_picture=fs_preview_picture
              preview_width=fs_preview_width
              preview_height=fs_preview_height
              dark
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
        members:
          MembersScreen #members
            with
              rows=members_rows
              admin=members_is_admin(members_rows)
              connected
              answered=members_answered
            events
              copy_to_clipboard -> copy_to_clipboard _ _
              agent_set_status -> agent_set_status _ _
              gov_propose -> gov_propose _ _
        agents:
          AgentsScreen rows=agents_rows connected answered=agents_answered #agents
        forge:
          ForgeScreen review_draft<->forge_review_draft comment_draft<->forge_comment_draft discussion_editor<->forge_discussion_editor #forge
            with
              org=network_name
              about=account_bio
              connected_rpc
              tier=member_tier(members_rows)
              repos=forge_repos
              list_phase=forge_list_phase
              open_repo=forge_repo
              repo_menu=forge_repo_menu
              repo_phase=forge_repo_phase
              branches=forge_branches
              tab=forge_tab
              items=forge_items
              forge_item_number
              item_phase=forge_item_phase
              forge_item_kind
              forge_item_title
              forge_item_state
              forge_item_author
              forge_item_branches
              forge_item_body
              forge_item_files_changed
              forge_item_additions
              forge_item_deletions
              forge_item_diff
              forge_item_diff_truncated
              forge_item_merge_oid
              forge_item_source_oid
              forge_item_channel
              forge_item_approvals
              forge_item_change_requests
              forge_item_reviews
              merge_conflicts=forge_merge_conflicts
              merge_busy=forge_merge_busy
              review_verdict=forge_review_verdict
              review_busy=forge_review_busy
              comment_target=forge_comment_target(forge_comment_path, forge_comment_line, forge_comment_side)
              staged_comments=forge_comment_staged
              discussion=forge_discussion
              discussion_pending=forge_discussion_pending
              linked_note=forge_linked_note
              connected
              loading
              dark
            events
              forge_open_repo -> forge_open_repo _
              forge_close_repo -> forge_close_repo
              forge_toggle_repo_menu -> forge_toggle_repo_menu
              select_forge_tab -> select_forge_tab _
              forge_open_item -> forge_open_item _
              forge_close_item -> forge_close_item
              forge_merge_submit -> forge_merge_submit
              forge_review_pick -> forge_review_pick _
              forge_review_submit -> forge_review_submit
              forge_comment_open -> forge_comment_open _ _ _
              forge_comment_stage -> forge_comment_stage
              forge_comment_cancel -> forge_comment_cancel
              forge_comment_drop -> forge_comment_drop _
              note_composer_event -> forge_composer_event _
              open_message_link -> open_message_link _
        governance:
          GovernanceScreen #governance
            with
              rows=gov_rows
              voting=gov_voting
              admin=members_is_admin(members_rows)
              connected
              answered=gov_answered
            events
              gov_vote -> gov_vote _ _
              gov_execute -> gov_execute _
        node:
          NodeScreen wall_now=wall_now node_log_filter<->node_log_filter #node
            with
              node_key
              node_data_dir
              members_rows
              status
              loading
              node_tab
              module_rows
              node_height
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
              select_node_tab -> select_node_tab _
              open_node_modules -> open_node_modules
              node_log_filter_changed -> node_log_filter_changed _
              copy_to_clipboard -> copy_to_clipboard _ _
            activity_log:
              extern node_log_timeline(node_log_timeline, connected_rpc) #node-log-timeline -> node_log_timeline_changed _
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
          ExplorerScreen #explorer(connected_rpc)
            with
              connected_rpc
              connected
              loading=explorer_loading
              blocks=explorer_blocks
              ops=explorer_ops
              head=block_height
              sync_line=sync_label(node_phase, node_sync_applied, node_sync_target)
            events
              refresh_explorer -> refresh_explorer
              copy_to_clipboard -> copy_to_clipboard _ _
        huddle:
          box
            with
              w=fill
              h=fill
              align-x=end
              align-y=end
              pr=16.0
              pb=16.0
            col
              // The pill says "you are still in a call elsewhere". It hides while
              // the huddle has its own window, and where the live pill in the
              // channel header already says so — the Chat tab, looking at the
              // huddle's own channel. On every OTHER screen it must show even
              // when that channel is the selected one, which the missing
              // `shell_tab` term used to suppress.
              if huddle_joined && !huddle_popped && (shell_tab != ShellTab.chat || huddle_channel != active_channel)
                HuddleDockedPill
                  with
                    channel=huddle_channel_name
                    elapsed=mmss(huddle_now - huddle_joined_at)
                  events
                    pop_huddle -> pop_huddle
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
                        keyed item in bell_items by=item.seq virtual-row=58.0 w=fill p=5.0 gap=1.0
                          BellRow item=item
