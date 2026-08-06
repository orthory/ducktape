view
  // THE WINDOW GATE — the daemon's one dispatch. The `window` binding names
  // the window being rendered: the launch window mounts the hub column, the
  // console window mounts the shell, the popped huddle mounts the panel. A
  // window no id claims renders the hub's quiet loading arm — it exists only
  // between open and register.
  col w=fill h=fill
    if (console_win != some(window)) && (huddle_win != some(window))
      HubColumn step=hub_step key_state=hub_key_state networks=hub_networks selected=hub_selected hidden=hub_hidden name=onboarding_name invite=invite_link reveal=reveal_words steps=provision_steps step_index=provision_index height=block_height tier=member_tier(members_rows) error=onboarding_error busy=(mutation_phase != "idle")
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
          go_join -> go_join
          go_networks -> go_networks
          join_network_submit -> join_network_submit _
          copy_onboarding_invite -> copy_onboarding_invite
          connect_remote_submit -> connect_remote_submit _
          restore_hidden_submit -> restore_hidden_submit
          enter_console -> enter_console
    // THE HUDDLE WINDOW — the same panel, now the whole content of a real OS
    // window instead of a card wearing drawn traffic lights. Its close button
    // docks (see `window_was_closed`); leaving the huddle closes it.
    if huddle_win == some(window)
      HuddlePanel channel=huddle_channel_name elapsed=mmss(huddle_now - huddle_joined_at) roster=huddle_roster status=call_status muted=call_muted peers=call_peers camera=call_camera video_live=call_video_live frame_generation=call_frame_generation #huddle
        events
          dock_huddle -> dock_huddle
          huddle_go_channel -> huddle_go_channel
          leave_huddle_here -> leave_huddle_here
          toggle_call_mute -> toggle_call_mute
          toggle_call_camera -> toggle_call_camera
    if console_win == some(window)
      WorkspaceTabs network=network_label(account_name, connected_rpc) status=status height=block_height loading=(loading || mutation_phase != "idle") degraded=connection_degraded(status) tab=shell_tab bell_count=bell_unread bell_sev=bell_worst_severity(bell_items) approvals=open_proposals(gov_rows) account=account_name agent_live=any_agent_active(agents_rows) tier=member_tier(members_rows) root_hash=node_root_hash consensus_view=node_view_label quorum=node_quorum_label reachable=node_reachable_label last_finalized=node_last_finalized checkpoint=node_checkpoint #workspace-tabs
        events
          select_shell_tab -> select_shell_tab _
          toggle_bell -> toggle_bell
          switch_network -> switch_network
        notice:
          col w=fill
            if error != ""
              box w=fill pl=12.0 pr=12.0 pb=8.0
                box w=fill p=8.0 bg=danger_bg border=danger_line border-w=1.0 r=12.0
                  row w=fill gap=8.0 align=center
                    box w=20.0 h=20.0 align-x=center align-y=center bg=danger_dot r=10.0
                      text "!" size=14.0 font=medium @text-danger_fg
                    text error w=fill size=13.5 @text-fg
                    button "Dismiss" h=26.0 p=5.0 @ghost_action -> dismiss_error
                      active bg=transparent text=muted r=7.0
                      hovered bg=fg/9 text=fg
                      pressed bg=fg/14
        chat:
          ChatScreen account_name=account_name connected_rpc=connected_rpc status=status block_height=block_height search_draft<->chat_search_draft searching=chat_searching search_hits=chat_search_hits channels=channels dm_peers=dm_peers channel_reads=channel_reads user_key=settings_user_key channel_create_open=channel_create_open connected=connected loading=loading mutation_phase=mutation_phase active_channel=active_channel active_dm_peer=active_dm_peer active_channel_name=active_channel_name active_channel_archived=active_channel_archived active_channel_members_only=active_channel_members_only channel_members=channel_members huddle_joined=huddle_joined huddle_channel=huddle_channel huddle_channel_name=huddle_channel_name huddle_joined_at=huddle_joined_at huddle_now=huddle_now call_muted=call_muted messages=messages history_view=history_view history_loading=history_loading unread_boundary=unread_boundary unread_marker_seq=unread_marker_seq selected_message_seq=selected_message_seq selected_message_rev=selected_message_rev send_flash_id=send_flash_id send_flash_value=animation.interpolate(send_flash, 0.0, 1.0) message_action=message_action message_menu_y=message_menu_y message_action_focus<->message_action_focus message_edit_draft<->message_edit_draft failed_message_draft=failed_message_draft message_editor<->message_editor channel_settings_open=channel_settings_open channel_name_draft<->channel_name_draft member_key_draft<->member_key_draft active_thread_seq=active_thread_seq thread_target_seq=thread_target_seq thread_messages=thread_messages thread_selected_seq=thread_selected_seq thread_selected_rev=thread_selected_rev thread_message_action=thread_message_action thread_menu_y=thread_menu_y thread_edit_draft<->thread_edit_draft thread_has_more=thread_has_more thread_next_reply_offset=thread_next_reply_offset thread_loading=thread_loading failed_reply_draft=failed_reply_draft reply_editor<->reply_editor shift_held=shift_held #chat
            events
              search_chat_submit -> search_chat_submit
              clear_chat_search -> clear_chat_search
              open_chat_search_hit -> open_chat_search_hit _ _ _
              toggle_channel_create -> toggle_channel_create
              choose_channel -> choose_channel _
              choose_dm -> choose_dm _
              toggle_channel_settings -> toggle_channel_settings
              pop_huddle -> pop_huddle
              leave_huddle_here -> leave_huddle_here
              huddle_go_channel -> huddle_go_channel
              join_huddle_submit -> join_huddle_submit
              chat_pointer_pressed -> chat_pointer_pressed _ _
              load_more_history -> load_more_history
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
              chat_resized -> chat_resized _ _
              thread_resized -> thread_resized _ _

        pages:
          PagesScreen pages=pages page_create_open=page_create_open loading=loading mutation_phase=mutation_phase connected=connected connected_rpc=connected_rpc password=password page_draft<->page_draft active_page=active_page active_page_title=active_page_title active_page_parent=active_page_parent page_search_draft<->page_search_draft page_searching=page_searching page_search_hits=page_search_hits page_delete_armed=page_delete_armed block_autosave_status=block_autosave_status doc_tabs=doc_tabs blocks=blocks orphaned_block_drafts=orphaned_block_drafts orphaned_comment_drafts=orphaned_comment_drafts block_draft<->block_draft block_insert_open=block_insert_open block_insert_after_id=block_insert_after_id new_block_kind=new_block_kind block_kinds=block_kinds editable_block_kinds=editable_block_kinds selected_block_id=selected_block_id selected_block_kind=selected_block_kind hovered_block_id=hovered_block_id block_editor<->block_editor block_actions_open=block_actions_open block_menu_x=block_menu_x block_menu_y=block_menu_y block_delete_armed=block_delete_armed block_comments_open=block_comments_open block_comment_thread_total=block_comment_thread_total block_comment_threads=block_comment_threads block_comment_threads_loading=block_comment_threads_loading block_comment_threads_has_more=block_comment_threads_has_more active_block_comment_thread=active_block_comment_thread block_thread_comments=block_thread_comments block_thread_comments_loading=block_thread_comments_loading block_thread_comments_has_more=block_thread_comments_has_more block_comment_draft<->block_comment_draft #pages
            events
              toggle_page_create -> toggle_page_create
              create_page_submit -> create_page_submit
              choose_page -> choose_page _
              pages_pointer_pressed -> pages_pointer_pressed _ _
              search_pages_submit -> search_pages_submit
              clear_page_search -> clear_page_search
              arm_page_delete -> arm_page_delete
              disarm_page_delete -> disarm_page_delete
              delete_page_submit -> delete_page_submit
              close_doc_tab -> close_doc_tab _
              open_page_search_hit -> open_page_search_hit _ _
              use_orphaned_block_draft -> use_orphaned_block_draft _
              discard_orphaned_block_draft -> discard_orphaned_block_draft _
              use_orphaned_comment_draft -> use_orphaned_comment_draft _
              discard_orphaned_comment_draft -> discard_orphaned_comment_draft _
              open_root_block_insert -> open_root_block_insert
              new_block_kind_changed -> new_block_kind_changed _
              close_block_insert -> close_block_insert
              add_block_submit -> add_block_submit
              pick_slash_kind -> pick_slash_kind _
              block_entered -> block_entered _
              block_exited -> block_exited _
              open_block_insert -> open_block_insert _ _
              select_block -> select_block _ _ _ _ _ _
              set_todo_checked -> set_todo_checked _ _
              block_key -> block_key _
              block_draft_changed -> block_draft_changed _
              close_block_actions -> close_block_actions
              selected_block_kind_changed -> selected_block_kind_changed _
              move_block_submit -> move_block_submit _
              open_block_comments -> open_block_comments
              arm_block_delete -> arm_block_delete
              remove_block_submit -> remove_block_submit
              close_block_comments -> close_block_comments
              open_block_comment_thread -> open_block_comment_thread _
              load_more_block_threads -> load_more_block_threads
              close_block_comment_thread -> close_block_comment_thread
              load_more_block_comments -> load_more_block_comments
              post_block_comment_submit -> post_block_comment_submit
              pages_resized -> pages_resized _ _

        files:
          FilesScreen path=fs_path entries=fs_entries loading=fs_loading new_name<->fs_new_name preview_path=fs_preview_path delete_target=fs_delete_target history_open=fs_history_open diff_from=fs_diff_from diff=fs_diff history=fs_history preview_truncated=fs_preview_truncated preview_binary=fs_preview_binary editing=fs_editing draft<->fs_editor preview_text=fs_preview_text
            events
              fs_open_dir -> fs_open_dir _
              fs_open_file -> fs_open_file _
              fs_open_parent -> fs_open_parent
              fs_new_name_changed -> fs_new_name_changed _
              fs_mkdir_submit -> fs_mkdir_submit
              fs_new_file_submit -> fs_new_file_submit
              fs_arm_delete -> fs_arm_delete _
              fs_disarm_delete -> fs_disarm_delete
              fs_delete_submit -> fs_delete_submit
              fs_toggle_history -> fs_toggle_history
              fs_close_diff -> fs_close_diff
              fs_show_diff -> fs_show_diff _
              fs_begin_edit -> fs_begin_edit
              fs_cancel_edit -> fs_cancel_edit
              fs_save_edit -> fs_save_edit
        members:
          MembersScreen rows=members_rows validators=members_validators residents=members_residents filter=members_filter selected=members_selected admin=members_is_admin(members_rows) answered=members_answered
            events
              pick_members_filter -> pick_members_filter _
              open_member -> open_member _
              copy_to_clipboard -> copy_to_clipboard _ _
              agent_set_status -> agent_set_status _ _
              gov_propose -> gov_propose _ _
        agents:
          AgentsScreen rows=agents_rows answered=agents_answered
        forge:
          ForgeScreen org=network_label(account_name, connected_rpc) about=account_bio tier=member_tier(members_rows) repos=forge_repos open_repo=forge_repo repo_menu=forge_repo_menu branches=forge_branches tab=forge_tab items=forge_items tree_repo=forge_tree_repo tree_path=forge_tree_path tree_entries=forge_tree_entries file_path=forge_file_path file_text=forge_file_text file_binary=forge_file_binary file_truncated=forge_file_truncated forge_item_number=forge_item_number forge_item_kind=forge_item_kind forge_item_title=forge_item_title forge_item_state=forge_item_state forge_item_author=forge_item_author forge_item_branches=forge_item_branches forge_item_body=forge_item_body forge_item_files_changed=forge_item_files_changed forge_item_additions=forge_item_additions forge_item_deletions=forge_item_deletions forge_item_diff=forge_item_diff forge_item_diff_truncated=forge_item_diff_truncated forge_item_merge_oid=forge_item_merge_oid forge_item_source_oid=forge_item_source_oid forge_item_channel=forge_item_channel forge_item_approvals=forge_item_approvals forge_item_change_requests=forge_item_change_requests forge_item_reviews=forge_item_reviews merge_conflicts=forge_merge_conflicts merge_busy=forge_merge_busy review_verdict=forge_review_verdict review_draft<->forge_review_draft review_busy=forge_review_busy comment_target=forge_comment_target(forge_comment_path, forge_comment_line, forge_comment_side) comment_draft<->forge_comment_draft staged_comments=forge_comment_staged answered=forge_answered discussion=forge_discussion discussion_editor<->forge_discussion_editor discussion_pending=forge_discussion_pending connected=connected loading=loading shift_held=shift_held
            events
              forge_open_repo -> forge_open_repo _
              forge_close_repo -> forge_close_repo
              forge_toggle_repo_menu -> forge_toggle_repo_menu
              select_forge_tab -> select_forge_tab _
              forge_open_dir -> forge_open_dir _
              forge_open_file -> forge_open_file _
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
        governance:
          GovernanceScreen rows=gov_rows voting=gov_voting admin=members_is_admin(members_rows) answered=gov_answered
            events
              gov_vote -> gov_vote _ _
              gov_execute -> gov_execute _
        settings:
          SettingsScreen account_name=account_name connected_rpc=connected_rpc settings_endpoint=settings_endpoint settings_node_key=settings_node_key settings_height=settings_height settings_data_dir=settings_data_dir settings_key_state=settings_key_state settings_key_path=settings_key_path settings_open_tabs=settings_open_tabs members_rows=members_rows members_validators=members_validators members_residents=members_residents account_id=account_id account_name_draft<->account_name_draft account_renaming=account_renaming account_members=account_members account_nodes=account_nodes appearance=appearance password=password status=status loading=loading connected=connected mutation_phase=mutation_phase node_tab=node_tab module_rows=module_rows block_height=block_height node_checkpoint=node_checkpoint node_last_finalized=node_last_finalized node_reachable_label=node_reachable_label node_quorum_label=node_quorum_label node_version=node_version node_root_hash=node_root_hash node_peers=node_peers node_log_filter<->node_log_filter node_log_lines=node_log_lines
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
        explorer:
          ExplorerScreen query<->explorer_query connected=connected searching=explorer_searching loading=explorer_loading kinds=explorer_kinds kind=explorer_kind hits=explorer_hits blocks=explorer_blocks selected=explorer_selected ops=explorer_ops
            events
              explorer_search_submit -> explorer_search_submit
              clear_explorer_search -> clear_explorer_search
              refresh_explorer -> refresh_explorer
              pick_explorer_kind -> pick_explorer_kind _
              select_explorer_block -> select_explorer_block _
        huddle:
          box w=fill h=fill align-x=end align-y=end pr=16.0 pb=16.0
            col
              // The pill says "you are still in a call elsewhere". It hides while
              // the huddle has its own window, and where the live pill in the
              // channel header already says so — the Chat tab, looking at the
              // huddle's own channel. On every OTHER screen it must show even
              // when that channel is the selected one, which the missing
              // `shell_tab` term used to suppress.
              if huddle_joined && (huddle_win == none) && (shell_tab != "chat" || huddle_channel != active_channel)
                HuddleDockedPill channel=huddle_channel_name elapsed=mmss(huddle_now - huddle_joined_at)
                  events
                    pop_huddle -> pop_huddle
        palette:
          OverlayLayer create_open=channel_create_open members_only=channel_create_members_only draft<->channel_draft busy=(mutation_phase != "idle") connected=connected loading=loading toast=toast tone=toast_tone open=palette_open query<->palette_draft searching=palette_searching chat_hits=palette_chat_hits page_hits=palette_page_hits #overlays
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
              button label="Close notifications" w=fill h=fill p=0.0 @icon_action -> close_bell
                space w=fill h=fill
                active bg=transparent border=transparent
            if bell_open
              box w=fill h=fill align-x=end align-y=start pt=44.0 pr=13.0
                box w=342.0 bg=surface border=border border-w=1.0 r=13.0 clip=true shadow=shadow_modal shadow-y=16.0 shadow-blur=40.0
                  col w=fill
                    box w=fill pl=13.0 pr=13.0 pt=11.0 pb=9.0
                      row w=fill gap=8.0 align=center
                        text "Alerts" size=12.5 wrap=none @text-primary
                        text bell_unread size=10.5 wrap=none font=code_medium @text-meta
                        text "unread" size=12.5 wrap=none @text-meta
                        space w=fill
                        button "Mark all read" disabled=(bell_unread <= 0) h=22.0 p=4.0 @ghost_action -> mark_bell_read_submit
                          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                          hovered bg=elevated text=brand
                          pressed bg=subtle text=brand
                    box w=fill h=1.0 bg=separator
                      space w=1.0 h=1.0
                    if empty(bell_items)
                      box w=fill p=26.0 align-x=center
                        text "Nothing yet — mentions and deliveries land here." size=12.0 @text-meta
                    if !empty(bell_items)
                      scroll dir=vertical w=fill h=290.0
                        col w=fill p=5.0 gap=1.0
                          for item in bell_items
                            BellRow item=item
