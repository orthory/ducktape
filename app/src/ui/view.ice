component StatusBadge(label:str)
  row align=center
    match label
      "active"
        Badge.Success label=label
      "paused"
        Badge.Warning label=label
      "open"
        Badge.Success label=label
      "closed"
        Badge.Destructive label=label
      "merged"
        Badge.Success label=label
      "passed"
        Badge.Success label=label
      "rejected"
        Badge.Destructive label=label
      "applied"
        Badge.Success label=label
      "discarded"
        Badge.Warning label=label
      _
        Badge.Outline label=label

view
  WorkspaceTabs network=network_label(account_name, connected_rpc) status=status height=block_height loading=(loading || mutation_phase != "idle") degraded=connection_degraded(status) tab=shell_tab bell_count=bell_unread approvals=open_proposals(gov_rows) account=account_name #workspace-tabs
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
      row w=fill h=fill
        box w=236.0 h=fill bg=sidebar clip=true
          col w=fill h=fill
            box w=fill pl=14.0 pr=14.0 pt=14.0 pb=11.0
              row w=fill gap=8.0 align=center
                text network_label(account_name, connected_rpc) size=13.5 wrap=none font=display @text-fg
                if connection_degraded(status)
                  box w=7.0 h=7.0 bg=danger_dot r=3.5
                    space w=1.0 h=1.0
                if !connection_degraded(status)
                  box w=7.0 h=7.0 bg=success_dot r=3.5
                    space w=1.0 h=1.0
                space w=fill
                text height_label(block_height) size=10.5 wrap=none font=code_medium @text-label
            box w=fill h=1.0 bg=separator
              space w=1.0 h=1.0
            box w=fill pl=12.0 pr=12.0 pt=11.0 pb=6.0
              button label="Search everything" w=fill h=31.0 p=0.0 @icon_action -> toggle_palette
                box w=fill h=fill pl=10.0 pr=10.0 align-y=center
                  row w=fill gap=7.0 align=center
                    Icon name="search" tone="hint" px=13.0
                    text "Search…" w=fill size=12.0 wrap=none @text-hint
                active bg=surface text=muted border=border border-w=1.0 r=8.0
                hovered bg=muted_bg text=fg border=control_line
                pressed bg=elevated text=fg
            box w=fill pl=16.0 pr=16.0 pt=10.0 pb=5.0
              row w=fill gap=6.0 align=center
                text "CHANNELS" size=10.0 wrap=none font=code_semibold @text-label
                space w=fill
                text len(channels) size=10.5 wrap=none font=code_medium @text-label
                if !channel_create_open
                  button label="New channel" disabled=(loading || mutation_phase != "idle" || !connected) p=0.0 @icon_action -> toggle_channel_create
                    Icon name="plus" tone="label" px=16.0
                    active bg=transparent text=muted border=transparent border-w=1.0 r=5.0
                    hovered bg=separator text=fg
                    pressed bg=subtle text=fg
                if channel_create_open
                  button label="Close new channel" disabled=(loading || mutation_phase != "idle") w=18.0 h=18.0 p=0.0 @icon_action -> toggle_channel_create
                    box w=fill h=fill align-x=center align-y=center
                      text "×" size=13.0 wrap=none @text-muted
                    active bg=separator text=muted border=transparent border-w=1.0 r=5.0
                    hovered bg=subtle text=fg
                    pressed bg=subtle text=fg
            if channel_create_open
              col w=fill pl=12.0 pr=12.0 pb=6.0 gap=4.0
                row w=fill h=28.0 gap=5.0 align=center
                  input "" #new-channel label="New channel name" <-> channel_draft hint="New channel" disabled=(loading || mutation_phase != "idle" || !connected) submit=create_channel_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                    active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                    hovered bg=muted_bg border=control_line
                    disabled bg=muted_bg/54 value=muted
                  button label="Create channel" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(channel_draft))) p=6.0 @primary_action -> create_channel_submit
                    Icon name="plus" tone="paper" px=15.0
                button label="Members-only posting" w=fill h=24.0 p=4.0 @ghost_action -> toggle_channel_create_members_only
                  row w=fill h=fill gap=6.0 align=center
                    if channel_create_members_only
                      text "☑" size=13.0 @text-brand
                    if !channel_create_members_only
                      text "☐" size=13.0 @text-muted
                    text "Members-only posting" w=fill size=12.5 wrap=none @text-caption
                  active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                  hovered bg=row_hover text=fg
                  pressed bg=separator text=fg
            scroll dir=vertical w=fill h=fill bar=hidden
              col w=fill gap=2.0
                for channel in channels
                  ChannelButton channel=channel selected=(channel.id == active_channel) unread=channel_is_unread(channel_reads, channel.id, channel.head_seq)
            box w=fill h=1.0 bg=separator
              space w=1.0 h=1.0
            box w=fill pl=14.0 pr=14.0 pt=11.0 pb=11.0
              row w=fill gap=9.0 align=center
                PersonAvatar initials=initial_of(account_name) plate=26.0 ink=10.0
                col w=fill gap=1.0
                  if !empty(account_name)
                    text account_name size=12.0 wrap=none font=display @text-fg
                  if empty(account_name)
                    text "Not signed in" size=12.0 wrap=none font=display @text-muted
                  text account_id size=9.5 wrap=none font=display @text-hint
        box w=1.0 h=fill bg=separator
          space w=1.0 h=1.0
        box w=fill h=fill bg=bg clip=true px-snap=true
          row w=fill h=fill
            col w=fill h=fill
              if !empty(active_channel)
                col w=fill
                  box w=fill h=50.0 pl=18.0 pr=18.0
                    row w=fill h=fill gap=9.0 align=center
                      text "#" size=14.0 wrap=none font=medium @text-hint
                      text active_channel_name size=14.0 wrap=none font=display @text-fg
                      if active_channel_archived
                        Badge.Outline label="Archived"
                      if active_channel_members_only
                        Badge.Outline label="Members only"
                      if active_channel_huddle_count > 0
                        box px=7.0 py=2.0 bg=success_bg border=success_line border-w=1.0 r=5.0
                          row gap=5.0 align=center
                            box w=6.0 h=6.0 bg=success_dot r=3.0
                              space w=1.0 h=1.0
                            text active_channel_huddle_count size=9.0 wrap=none font=code_semibold @text-fg
                      space w=fill
                      input "" #chat-search label="Search messages" <-> chat_search_draft hint="Search messages" disabled=(!connected || chat_searching) submit=search_chat_submit w=190.0 p=6.2 text-size=13.0 line-h=1.2 @control
                        active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                        hovered bg=muted_bg border=control_line
                        disabled bg=transparent value=muted
                      if !empty(chat_search_hits)
                        button label="Clear message search" w=27.0 h=25.0 p=0.0 @icon_action -> clear_chat_search
                          box w=fill h=fill align-x=center align-y=center
                            text "×" size=13.0 wrap=none @text-muted
                          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                          hovered bg=elevated text=fg
                          pressed bg=subtle text=fg
                      button label="Channel details" w=27.0 h=25.0 p=0.0 @icon_action -> toggle_channel_settings
                        box w=fill h=fill align-x=center align-y=center
                          text "⋯" size=14.0 wrap=none @text-muted
                        active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                        hovered bg=elevated text=fg
                        pressed bg=subtle text=fg
                  box w=fill h=1.0 bg=separator
                    space w=1.0 h=1.0
              col w=fill h=fill gap=9.0 pl=18.0 pr=18.0 pt=9.0 pb=14.0
                if !empty(chat_search_hits)
                  box w=fill h=148.0 p=6.0 bg=elevated border=fg/10 border-w=1.0 r=10.0
                    scroll dir=vertical w=fill h=fill
                      col w=fill gap=1.0
                        for hit in chat_search_hits
                          ChatSearchResult hit=hit
                if !connected
                  EmptyState title="Connect to a node" description="Set the RPC endpoint in the sidebar."
                if connected && empty(messages)
                  EmptyState title="No messages yet" description="Create a channel or start the conversation."
                if connected && !empty(messages) && history_view
                  box w=fill h=32.0 pl=10.0 pr=6.0 bg=warning_bg border=warning_line border-w=1.0 r=9.0
                    row w=fill h=fill gap=8.0 align=center
                      text "Viewing history" w=fill size=12.5 wrap=none @text-warning
                      button "Jump to latest" h=24.0 p=5.0 @ghost_action -> choose_channel(active_channel)
                        active bg=surface text=fg border=warning_line border-w=1.0 r=7.0
                        hovered bg=warning_bg text=fg
                        pressed bg=accent text=fg
                if connected && !empty(messages)
                  stack w=fill h=fill
                    mouse move=chat_pointer_moved
                      sensor show=chat_resized resize=chat_resized
                        scroll dir=vertical w=fill h=fill
                          col w=fill gap=1.0
                            if history_has_older(messages)
                              box w=fill align-x=center pt=4.0 pb=8.0
                                button "Load older messages" disabled=(history_loading || mutation_phase != "idle") h=30.0 p=6.0 @secondary_action -> load_more_history
                                  active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=8.0
                                  hovered bg=fg/10 text=fg border=fg/14
                                  pressed bg=fg/14 text=fg
                            for message in messages
                              col w=fill gap=0.0
                                if unread_boundary > 0 && message.seq == first_unread_seq(messages, unread_boundary)
                                  row w=fill gap=8.0 align=center pt=8.0 pb=2.0
                                    box w=fill h=1.0 bg=brand/40
                                      text ""
                                    text "New messages" size=12.5 wrap=none @text-brand
                                    box w=fill h=1.0 bg=brand/40
                                      text ""
                                if message.show_author
                                  space h=10.0
                                stack #message(message.id) w=fill
                                  MessageCard message=message selected=(message.seq == selected_message_seq) hovered=(message.seq == hovered_message_seq) disabled=loading
                    overlay when=(selected_message_seq > 0 && message_action != "toolbar") dismiss=clear_message_selection backdrop=transparent p=8.0 align-x=end align-y=start
                      content
                        space w=fill h=fill
                      layer
                        float x=0.0 y=message_menu_y
                          col
                            if message_action == "more"
                              stack
                                input "" #message-action-focus label="Message action focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                                  active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                                  focused bg=transparent border=transparent value=transparent border-w=0.0
                                box w=190.0 p=4.0 style=raised_style()
                                  col w=fill gap=1.0
                                    button "React" label="Manage reactions" disabled=active_channel_archived w=fill h=28.0 p=6.0 @ghost_action -> open_message_reactions(selected_message_seq, message_edit_draft, selected_message_rev)
                                      active bg=transparent text=muted r=6.0
                                      hovered bg=fg/10 text=fg
                                      pressed bg=fg/15
                                    button "Open thread" w=fill h=28.0 p=6.0 @ghost_action -> open_thread_for(selected_message_seq)
                                      active bg=transparent text=muted r=6.0
                                      hovered bg=fg/10 text=fg
                                      pressed bg=fg/15
                                    button "Edit" w=fill h=28.0 p=6.0 @ghost_action -> begin_message_edit(selected_message_seq, message_edit_draft, selected_message_rev)
                                      active bg=transparent text=muted r=6.0
                                      hovered bg=fg/10 text=fg
                                      pressed bg=fg/15
                                    button "Delete" w=fill h=28.0 p=6.0 @danger_action -> arm_message_delete(selected_message_seq, message_edit_draft, selected_message_rev)
                                    button "Close" label="Close message actions" w=fill h=28.0 p=6.0 @secondary_action -> clear_message_selection
                                      active bg=transparent text=muted r=6.0
                                      hovered bg=fg/10 text=fg
                                      pressed bg=fg/15
                            if message_action == "reactions"
                              stack
                                input "" #message-reaction-focus label="Message reaction focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                                  active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                                  focused bg=transparent border=transparent value=transparent border-w=0.0
                                box p=3.0 style=raised_style()
                                  row gap=2.0 align=center
                                    button "+ 👍" label="Add thumbs up reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_submit("👍")
                                      active bg=transparent text=fg r=6.0
                                      hovered bg=fg/10
                                      pressed bg=fg/15
                                    button "+ ♥" label="Add heart reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_submit("❤️")
                                      active bg=transparent text=fg r=6.0
                                      hovered bg=fg/10
                                      pressed bg=fg/15
                                    button "+ 😄" label="Add smile reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_submit("😄")
                                      active bg=transparent text=fg r=6.0
                                      hovered bg=fg/10
                                      pressed bg=fg/15
                                    button "+ 🎉" label="Add celebration reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_submit("🎉")
                                      active bg=transparent text=fg r=6.0
                                      hovered bg=fg/10
                                      pressed bg=fg/15
                                    button "+ 👀" label="Add eyes reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_submit("👀")
                                      active bg=transparent text=fg r=6.0
                                      hovered bg=fg/10
                                      pressed bg=fg/15
                                    button "+ 🙌" label="Add raised hands reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_submit("🙌")
                                      active bg=transparent text=fg r=6.0
                                      hovered bg=fg/10
                                      pressed bg=fg/15
                                    for message in messages
                                      if message.seq == selected_message_seq
                                        for reaction in message.reactions
                                          if reaction.reacted_by_me
                                            button label="Remove my reaction" description=reaction.emoji disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> remove_reaction_submit(reaction.emoji)
                                              text reaction.emoji size=13.0 @text-fg
                                              active bg=fg/7 text=fg r=6.0
                                              hovered bg=fg/12
                                              pressed bg=fg/17
                                    button "×" label="Close reactions" disabled=(mutation_phase != "idle") w=26.0 h=26.0 p=4.0 @secondary_action -> clear_message_selection
                                      active bg=transparent text=muted r=6.0
                                      hovered bg=fg/10 text=fg
                                      pressed bg=fg/15
                            if message_action == "editing"
                              box w=fill max-w=520.0 p=3.0 style=raised_style()
                                row w=fill gap=4.0 align=center
                                  input "" #message-edit label="Edit message" <-> message_edit_draft hint="Edit message" disabled=(mutation_phase != "idle") submit=edit_message_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                                    active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                    hovered bg=fg/4 border=fg/8
                                    disabled value=muted
                                  button "Save" label="Save message changes" disabled=(mutation_phase != "idle" || empty(trim(message_edit_draft))) h=28.0 p=6.0 @primary_action -> edit_message_submit
                                  button label="Cancel message edit" disabled=(mutation_phase != "idle") w=28.0 h=28.0 p=0.0 @icon_action -> clear_message_selection
                                    box w=fill h=fill align-x=center align-y=center
                                      text "×" size=14.0
                                    active bg=transparent text=muted r=7.0
                                    hovered bg=fg/10 text=fg
                                    pressed bg=fg/15
                            if message_action == "delete"
                              stack
                                input "" #message-delete-focus label="Message delete focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                                  active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                                  focused bg=transparent border=transparent value=transparent border-w=0.0
                                box p=3.0 style=raised_style()
                                  row gap=5.0 align=center
                                    text "Delete this message?" size=12.5 @text-muted
                                    button "Delete" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> delete_message_submit
                                    button "Cancel" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @secondary_action -> clear_message_selection
                                      active bg=transparent text=muted r=6.0
                                      hovered bg=fg/10 text=fg
                                      pressed bg=fg/15
                if !empty(failed_message_draft)
                  row w=fill gap=6.0 align=center
                    text "An earlier message wasn’t sent" w=fill size=12.5 @text-muted
                    button "Restore" disabled=(!empty(trim(editor_text(message_editor))) || mutation_phase != "idle") h=28.0 p=5.0 @secondary_action -> restore_failed_message
                      active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
                      hovered bg=fg/14
                      pressed bg=fg/18
                    button label="Dismiss unsent message" w=28.0 h=28.0 p=0.0 @icon_action -> dismiss_failed_message
                      box w=fill h=fill align-x=center align-y=center
                        text "×" size=14.0
                      active bg=transparent text=muted r=7.0
                      hovered bg=fg/10 text=fg
                      pressed bg=fg/15
                box w=fill bg=surface border=control_line border-w=1.0 r=12.0 clip=true shadow=shadow_popover shadow-y=1.0 shadow-blur=2.0
                  col w=fill
                    editor #message <-> message_editor hint="Message the channel…" disabled=(loading || !connected || empty(active_channel) || active_channel_archived) min-h=44.0 max-h=150.0 size=13.5 line-h=1.3 p=6.6 wrap=word key-binding=composer_keys() -> send_message_submit
                      active bg=transparent border=transparent value=fg placeholder=hint selection=fg/18 border-w=0.0 r=0.0
                      hovered bg=transparent border=transparent
                      focused bg=transparent border=ring border-w=1.0
                      disabled value=muted
                    box w=fill pl=10.0 pr=8.0 pb=8.0
                      row w=fill gap=10.0 align=center
                        space w=fill
                        text "↵ send · ⇧↵ newline" size=10.5 wrap=none font=code_medium @text-label
                        button "Send" disabled=(loading || !connected || empty(active_channel) || active_channel_archived || empty(trim(editor_text(message_editor)))) h=29.0 p=7.0 @primary_action -> send_message_submit
            if channel_settings_open && !empty(active_channel)
              box w=1.0 h=fill bg=fg/8
                text ""
              box w=300.0 h=fill p=12.0 bg=muted_bg
                col w=fill h=fill gap=8.0
                  row w=fill h=28.0 gap=6.0 align=center
                    text "Channel details" w=fill size=14.0 font=display @text-fg
                    button label="Close channel details" w=28.0 h=28.0 p=0.0 @icon_action -> toggle_channel_settings
                      box w=fill h=fill align-x=center align-y=center
                        text "×" size=14.0
                      active bg=transparent text=muted r=7.0
                      hovered bg=fg/10 text=fg
                      pressed bg=fg/15
                  Separator
                  row w=fill gap=5.0 align=center
                    input "" #channel-name label="Channel name" <-> channel_name_draft hint="Channel name" disabled=(mutation_phase != "idle") submit=rename_channel_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                      active bg=transparent border=fg/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                      hovered bg=fg/4 border=fg/14
                      disabled value=muted
                    button "Rename" disabled=(mutation_phase != "idle" || empty(trim(channel_name_draft))) w=56.0 h=28.0 p=5.0 @secondary_action -> rename_channel_submit
                  row w=fill gap=5.0 align=center
                    if !active_channel_archived
                      button "Archive" disabled=(mutation_phase != "idle") h=28.0 p=5.0 @danger_action -> archive_channel_submit
                      button "Join huddle" disabled=(mutation_phase != "idle") h=28.0 p=5.0 @ghost_action -> join_huddle_submit
                        active bg=transparent text=muted r=7.0
                        hovered bg=fg/10 text=fg
                        pressed bg=fg/15
                    if active_channel_archived
                      button "Unarchive" disabled=(mutation_phase != "idle") h=28.0 p=5.0 @secondary_action -> unarchive_channel_submit
                        active bg=transparent text=muted r=7.0
                        hovered bg=fg/10 text=fg
                        pressed bg=fg/15
                    if active_channel_huddle_count > 0
                      button "Leave huddle" disabled=(mutation_phase != "idle") h=28.0 p=5.0 @secondary_action -> leave_huddle_submit
                        active bg=transparent text=muted r=7.0
                        hovered bg=fg/10 text=fg
                        pressed bg=fg/15
                    space w=fill
                    text len(channel_members) size=12.0 font=code @text-muted
                  row w=fill gap=5.0 align=center
                    input "" #member-key label="Member public key" <-> member_key_draft hint="64-character member key" disabled=(mutation_phase != "idle") submit=add_channel_member_submit w=fill p=7.4 text-size=12.0 line-h=1.2 font=code @control
                      active bg=transparent border=fg/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                      hovered bg=fg/4 border=fg/14
                      disabled value=muted
                    button "Add" disabled=(mutation_phase != "idle" || empty(trim(member_key_draft))) w=40.0 h=28.0 p=5.0 @secondary_action -> add_channel_member_submit
                  if !empty(channel_members)
                    scroll dir=vertical w=fill h=fill
                      col w=fill gap=2.0
                        for member in channel_members
                          ChatMemberRow member=member disabled=(mutation_phase != "idle")
            if active_thread_seq > 0 && !channel_settings_open
              box w=1.0 h=fill bg=fg/8
                text ""
              box w=300.0 h=fill p=12.0 bg=muted_bg
                stack w=fill h=fill
                  mouse move=thread_pointer_moved
                    sensor show=thread_resized resize=thread_resized
                      col w=fill h=fill gap=8.0
                        row w=fill h=28.0 gap=6.0 align=center
                          if thread_target_seq <= 0
                            text "Thread" w=fill size=14.0 font=display @text-fg
                          if thread_target_seq > 0
                            text "Thread result" w=fill size=14.0 font=display @text-fg
                          text len(thread_messages) size=12.0 font=code @text-muted
                          button label="Close thread" disabled=(mutation_phase != "idle") w=28.0 h=28.0 p=0.0 @icon_action -> close_thread
                            box w=fill h=fill align-x=center align-y=center
                              text "×" size=14.0
                            active bg=transparent text=muted r=7.0
                            hovered bg=fg/11 text=fg
                            pressed bg=brand_bg
                        Separator
                        scroll dir=vertical w=fill h=fill
                          col w=fill gap=1.0
                            for thread_message in thread_messages
                              ThreadMessageCard message=thread_message selected=(thread_message.seq == thread_target_seq) hovered=(thread_message.seq == thread_hovered_seq) disabled=loading
                            if thread_has_more && thread_next_reply_offset >= 0
                              button "Load more replies" disabled=(thread_loading || mutation_phase != "idle") w=fill h=28.0 p=5.0 @secondary_action -> load_more_thread
                                active bg=transparent text=muted r=7.0
                                hovered bg=fg/9 text=fg
                                pressed bg=brand_bg
                        if !empty(failed_reply_draft)
                          row w=fill gap=6.0 align=center
                            text "Unsent reply" w=fill size=12.5 @text-muted
                            button "Restore" disabled=(!empty(trim(editor_text(reply_editor)))) h=26.0 p=5.0 @secondary_action -> restore_failed_reply
                              active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
                              hovered bg=fg/14
                              pressed bg=fg/18
                            button "×" label="Dismiss unsent reply" w=26.0 h=26.0 p=4.0 @ghost_action -> dismiss_failed_reply
                              active bg=transparent text=muted r=7.0
                              hovered bg=fg/10 text=fg
                              pressed bg=fg/15
                        box w=fill p=5.0 bg=transparent border=fg/12 border-w=1.0 r=7.0
                          row w=fill gap=5.0 align=end
                            editor #reply <-> reply_editor hint="Reply…" disabled=(thread_loading || active_channel_archived) min-h=44.0 max-h=150.0 size=13.5 line-h=1.3 p=6.6 wrap=word key-binding=composer_keys() -> send_reply_submit
                              active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=9.0
                              hovered bg=fg/4 border=fg/8 border-w=1.0
                              focused bg=fg/6 border=ring border-w=1.0
                              disabled value=muted
                            button "Send" label="Send reply" disabled=(thread_loading || active_channel_archived || empty(trim(editor_text(reply_editor)))) h=28.0 p=6.0 @primary_action -> send_reply_submit
                  overlay when=(thread_selected_seq > 0 && thread_message_action != "toolbar") dismiss=clear_thread_message_selection backdrop=transparent p=8.0 align-x=end align-y=start
                    content
                      space w=fill h=fill
                    layer
                      float x=0.0 y=thread_menu_y
                        col
                          if thread_message_action == "more"
                            stack
                              input "" #thread-action-focus label="Thread action focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                                active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                                focused bg=transparent border=transparent value=transparent border-w=0.0
                              box w=190.0 p=4.0 style=raised_style()
                                col w=fill gap=1.0
                                  button "React" label="Manage reactions" disabled=active_channel_archived w=fill h=28.0 p=6.0 @ghost_action -> open_thread_message_reactions(thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                    active bg=transparent text=muted r=6.0
                                    hovered bg=fg/10 text=fg
                                    pressed bg=fg/15
                                  button "Edit" w=fill h=28.0 p=6.0 @ghost_action -> begin_thread_message_edit(thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                    active bg=transparent text=muted r=6.0
                                    hovered bg=fg/10 text=fg
                                    pressed bg=fg/15
                                  button "Delete" w=fill h=28.0 p=6.0 @danger_action -> arm_thread_message_delete(thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                  button "Close" label="Close message actions" w=fill h=28.0 p=6.0 @secondary_action -> clear_thread_message_selection
                                    active bg=transparent text=muted r=6.0
                                    hovered bg=fg/10 text=fg
                                    pressed bg=fg/15
                          if thread_message_action == "reactions"
                            stack
                              input "" #thread-reaction-focus label="Thread reaction focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                                active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                                focused bg=transparent border=transparent value=transparent border-w=0.0
                              box p=3.0 style=raised_style()
                                row gap=2.0 align=center
                                  button "+ 👍" label="Add thumbs up reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_at(thread_selected_seq, "👍")
                                    active bg=transparent text=fg r=6.0
                                    hovered bg=fg/10
                                    pressed bg=fg/15
                                  button "+ ♥" label="Add heart reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_at(thread_selected_seq, "❤️")
                                    active bg=transparent text=fg r=6.0
                                    hovered bg=fg/10
                                    pressed bg=fg/15
                                  button "+ 😄" label="Add smile reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_at(thread_selected_seq, "😄")
                                    active bg=transparent text=fg r=6.0
                                    hovered bg=fg/10
                                    pressed bg=fg/15
                                  button "+ 🎉" label="Add celebration reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_at(thread_selected_seq, "🎉")
                                    active bg=transparent text=fg r=6.0
                                    hovered bg=fg/10
                                    pressed bg=fg/15
                                  button "+ 👀" label="Add eyes reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_at(thread_selected_seq, "👀")
                                    active bg=transparent text=fg r=6.0
                                    hovered bg=fg/10
                                    pressed bg=fg/15
                                  button "+ 🙌" label="Add raised hands reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_at(thread_selected_seq, "🙌")
                                    active bg=transparent text=fg r=6.0
                                    hovered bg=fg/10
                                    pressed bg=fg/15
                                  for thread_message in thread_messages
                                    if thread_message.seq == thread_selected_seq
                                      for reaction in thread_message.reactions
                                        if reaction.reacted_by_me
                                          button label="Remove my reaction" description=reaction.emoji disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> remove_reaction_at(thread_selected_seq, reaction.emoji)
                                            text reaction.emoji size=13.0 @text-fg
                                            active bg=fg/7 text=fg r=6.0
                                            hovered bg=fg/12
                                            pressed bg=fg/17
                                  button "×" label="Close reactions" disabled=(mutation_phase != "idle") w=26.0 h=26.0 p=4.0 @secondary_action -> clear_thread_message_selection
                                    active bg=transparent text=muted r=6.0
                                    hovered bg=fg/10 text=fg
                                    pressed bg=fg/15
                          if thread_message_action == "editing"
                            box w=fill max-w=520.0 p=3.0 style=raised_style()
                              row w=fill gap=4.0 align=center
                                input "" #thread-edit label="Edit message" <-> thread_edit_draft hint="Edit message" disabled=(mutation_phase != "idle") submit=edit_thread_message_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                  hovered bg=fg/4 border=fg/8
                                  disabled value=muted
                                button "Save" label="Save message changes" disabled=(mutation_phase != "idle" || empty(trim(thread_edit_draft))) h=28.0 p=6.0 @primary_action -> edit_thread_message_submit
                                button label="Cancel message edit" disabled=(mutation_phase != "idle") w=28.0 h=28.0 p=0.0 @icon_action -> clear_thread_message_selection
                                  box w=fill h=fill align-x=center align-y=center
                                    text "×" size=14.0
                                  active bg=transparent text=muted r=7.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                          if thread_message_action == "delete"
                            stack
                              input "" #thread-delete-focus label="Thread delete focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                                active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                                focused bg=transparent border=transparent value=transparent border-w=0.0
                              box p=3.0 style=raised_style()
                                row gap=5.0 align=center
                                  text "Delete this message?" size=12.5 @text-muted
                                  button "Delete" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> delete_thread_message_submit
                                  button "Cancel" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @secondary_action -> clear_thread_message_selection
                                    active bg=transparent text=muted r=6.0
                                    hovered bg=fg/10 text=fg
                                    pressed bg=fg/15
    pages:
      row w=fill h=fill
        box w=230.0 h=fill bg=sidebar clip=true
          col w=fill h=fill gap=0.0
            box w=fill pl=14.0 pr=14.0 pt=14.0 pb=11.0
              row w=fill gap=8.0 align=center
                text "Pages" size=13.5 wrap=none font=display @text-fg
                text len(pages) size=10.5 wrap=none font=code_medium @text-hint
                space w=fill
                if !page_create_open
                  button label="New page" disabled=(loading || mutation_phase != "idle" || !connected) p=0.0 @icon_action -> toggle_page_create
                    Icon name="plus" tone="label" px=16.0
                    active bg=transparent text=muted border=transparent border-w=1.0 r=5.0
                    hovered bg=separator text=fg
                    pressed bg=subtle text=fg
                if page_create_open
                  button label="Close new page" disabled=(loading || mutation_phase != "idle") w=18.0 h=18.0 p=0.0 @icon_action -> toggle_page_create
                    box w=fill h=fill align-x=center align-y=center
                      text "×" size=13.0 wrap=none @text-muted
                    active bg=separator text=muted border=transparent border-w=1.0 r=5.0
                    hovered bg=subtle text=fg
                    pressed bg=subtle text=fg
            box w=fill h=1.0 bg=separator
              space w=1.0 h=1.0
            if page_create_open
              row w=fill h=28.0 gap=5.0 align=center
                input "" #new-page label="New page title" <-> page_draft hint="New page" disabled=(loading || mutation_phase != "idle" || !connected) submit=create_page_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                  active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                  hovered bg=elevated border=fg/21
                  disabled bg=muted_bg/54 value=muted
                button label="Create page" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(page_draft))) w=28.0 h=28.0 p=0.0 @icon_action -> create_page_submit
                  box w=fill h=fill align-x=center align-y=center
                    text "+" size=14.0
            scroll dir=vertical w=fill h=fill
              col w=fill gap=2.0
                for page in pages
                  PageButton page=page selected=(page.id == active_page)
        box w=1.0 h=fill bg=separator
          space w=1.0 h=1.0
        mouse move=pages_pointer_moved
          col w=fill h=fill
            if connected && !empty(doc_tab_rows(doc_tabs, pages, active_page))
              box w=fill h=34.0 pl=8.0 pr=8.0 bg=sidebar border=separator border-w=1.0
                scroll dir=horizontal w=fill h=fill bar=hidden
                  row h=fill gap=2.0 align=center
                    for tab in doc_tab_rows(doc_tabs, pages, active_page)
                      row gap=0.0 align=center
                        button label="Open page tab" h=26.0 p=5.0 @ghost_action -> choose_page(tab.id)
                          row h=fill gap=5.0 align=center
                            if tab.active
                              text tab.title size=13.0 wrap=none font=medium @text-fg
                            if !tab.active
                              text tab.title size=13.0 wrap=none @text-muted
                          active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                          hovered bg=fg/5 text=fg
                          pressed bg=fg/8
                        button "×" label="Close page tab" w=20.0 h=20.0 p=0.0 @icon_action -> close_doc_tab(tab.id)
                          active bg=transparent text=muted r=6.0
                          hovered bg=fg/8 text=fg
                          pressed bg=fg/12
            stack w=fill h=fill clip=true
              sensor show=pages_resized resize=pages_resized
                space w=fill h=fill
              if !connected
                EmptyState title="Connect to a node" description="Set the RPC endpoint in the sidebar."
              if connected && empty(active_page)
                EmptyState title="No page selected" description="Create a page from the sidebar."
              if connected && !empty(active_page)
                scroll dir=vertical w=fill h=fill bar=hidden
                  box w=fill max-w=800.0 mx=auto pl=46.0 pr=46.0 pt=24.0 pb=80.0
                    col w=fill gap=8.0
                      row w=fill h=28.0 gap=5.0 align=center
                        if !empty(active_page_parent)
                          text active_page_parent w=fill size=12.0 wrap=none font=code @text-muted
                        if empty(active_page_parent)
                          space w=fill
                        input "" #page-search label="Search pages" <-> page_search_draft hint="Search pages…" disabled=(!connected || page_searching) submit=search_pages_submit w=190.0 p=6.2 text-size=13.0 line-h=1.2 @control
                          active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                          hovered bg=fg/5 border=fg/8
                          disabled value=muted
                        if !empty(page_search_hits)
                          button label="Clear page search" w=28.0 h=28.0 p=0.0 @icon_action -> clear_page_search
                            box w=fill h=fill align-x=center align-y=center
                              text "×" size=14.0
                            active bg=transparent text=muted r=7.0
                            hovered bg=fg/10 text=fg
                            pressed bg=fg/15
                        if !page_delete_armed
                          button label="Page menu" disabled=(mutation_phase != "idle") w=28.0 h=28.0 p=0.0 @icon_action -> arm_page_delete
                            box w=fill h=fill align-x=center align-y=center
                              text "•••" size=13.0
                            active bg=transparent text=muted r=7.0
                            hovered bg=fg/10 text=fg
                            pressed bg=fg/15
                        if page_delete_armed
                          button "Delete page" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> delete_page_submit
                      box w=fill pl=56.0
                        PageTitleEditor rpc=connected_rpc password=password page_id=active_page title=active_page_title disabled=(loading || !connected || mutation_phase != "idle") #page-title(scope_key(connected_rpc, active_page))
                      if !empty(page_search_hits)
                        box w=fill h=148.0 p=5.0 bg=elevated border=fg/8 border-w=1.0 r=9.0
                          scroll dir=vertical w=fill h=fill
                            col w=fill gap=1.0
                              for hit in page_search_hits
                                PageSearchResult hit=hit
                      if !empty(orphaned_block_drafts) || !empty(orphaned_comment_drafts)
                        box w=fill p=7.0 bg=elevated border=fg/9 border-w=1.0 r=9.0
                          col w=fill gap=5.0
                            text "Recovered drafts" size=13.0 font=medium @text-fg
                            for recovered_block in orphaned_block_drafts
                              row w=fill gap=5.0 align=center
                                text recovered_block w=fill size=13.5 @text-muted
                                button "Use" label="Use as block" disabled=(loading || mutation_phase != "idle" || !empty(block_draft)) h=26.0 p=5.0 @ghost_action -> use_orphaned_block_draft(recovered_block)
                                  active bg=fg/9 text=fg border=fg/12 border-w=1.0 r=7.0
                                  hovered bg=fg/14
                                  pressed bg=fg/18
                                button "Discard" disabled=(loading || mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> discard_orphaned_block_draft(recovered_block)
                            for recovered_comment in orphaned_comment_drafts
                              row w=fill gap=5.0 align=center
                                text recovered_comment w=fill size=13.5 @text-muted
                                button "Use" label="Use as block" disabled=(loading || mutation_phase != "idle" || !empty(block_draft)) h=26.0 p=5.0 @ghost_action -> use_orphaned_comment_draft(recovered_comment)
                                  active bg=fg/9 text=fg border=fg/12 border-w=1.0 r=7.0
                                  hovered bg=fg/14
                                  pressed bg=fg/18
                                button "Discard" disabled=(loading || mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> discard_orphaned_comment_draft(recovered_comment)
                      if empty(blocks) && !block_insert_open
                        box w=fill pl=56.0
                          button "Write something…" label="Start writing" disabled=loading w=fill p=6.0 @ghost_action -> open_root_block_insert
                            active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                            hovered bg=fg/4 text=fg border=fg/7
                            pressed bg=fg/8
                      if block_insert_open && empty(block_insert_after_id)
                        InlineBlockInsert kind=new_block_kind kinds=block_kinds disabled=loading prefix="" #block-insert-row(block_insert_after_id)
                          stack w=fill
                            if new_block_kind != "Divider"
                              col w=fill gap=2.0
                                input "" #block-insert label="New block" <-> block_draft hint="Type, or / for a block kind…" disabled=loading submit=add_block_submit w=fill p=5.0 text-size=13.5 line-h=1.3 @control
                                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=6.0
                                  hovered bg=fg/2 border=fg/5
                                  disabled value=muted
                                if !empty(slash_kind_matches(block_draft, editable_block_kinds))
                                  box w=fill p=3.0 bg=surface border=border border-w=1.0 r=8.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
                                    col w=fill gap=1.0
                                      for kind in slash_kind_matches(block_draft, editable_block_kinds)
                                        button label="Set block kind" w=fill h=24.0 p=4.0 @ghost_action -> pick_slash_kind(kind)
                                          row w=fill h=fill gap=6.0 align=center
                                            text kind w=fill size=13.0 wrap=none @text-fg
                                          active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                          hovered bg=brand/14 text=fg
                                          pressed bg=brand/20
                            if new_block_kind == "Divider"
                              button "Insert divider" disabled=loading w=fill h=28.0 p=5.0 @secondary_action -> add_block_submit
                      keyed block in blocks by=block.key
                        col w=fill gap=1.0
                          DocumentBlock block=block selected=(block.id == selected_block_id) hovered=(block.id == hovered_block_id) disabled=loading #block(block.id)
                            col w=fill
                              if block.pending
                                box w=fill p=5.0 bg=fg/3 r=6.0
                                  BlockContents block=block
                              if !block.pending && block.kind == "Page"
                                button label=block.kind description=block.text w=fill p=5.0 @ghost_action -> choose_page(block.id)
                                  BlockContents block=block
                                  active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                  hovered bg=fg/3 text=fg border=transparent
                                  pressed bg=fg/6 text=fg
                              if !block.pending && block.kind != "Page" && block.id != selected_block_id
                                button label=block.kind description=block.text w=fill p=5.0 @ghost_action -> select_block(block.key, block.id, block.kind, block.text, block.checked, false)
                                  BlockContents block=block
                                  active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                  hovered bg=fg/3 text=fg border=transparent
                                  pressed bg=fg/6 text=fg
                              if !block.pending && block.kind != "Page" && block.id == selected_block_id
                                BlockLine block=block #line
                                  col w=fill
                                    if block.kind == "Divider"
                                      Separator
                                    if block.kind != "Divider"
                                      input "" #block-edit label="Edit block" <-> block_edit_draft change=block_text_changed hint="Type something…" disabled=(mutation_phase != "idle") w=fill p=4.0 text-size=13.5 line-h=1.3 @control
                                        active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=5.0
                                        hovered bg=fg/2 border=fg/5
                                        disabled value=muted
                          if block_insert_open && block.id == block_insert_after_id
                            InlineBlockInsert kind=new_block_kind kinds=block_kinds disabled=loading prefix=block.prefix #block-insert-row(block_insert_after_id)
                              stack w=fill
                                if new_block_kind != "Divider"
                                  col w=fill gap=2.0
                                    input "" #block-insert label="New block" <-> block_draft hint="Type, or / for a block kind…" disabled=loading submit=add_block_submit w=fill p=5.0 text-size=13.5 line-h=1.3 @control
                                      active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=6.0
                                      hovered bg=fg/2 border=fg/5
                                      disabled value=muted
                                    if !empty(slash_kind_matches(block_draft, editable_block_kinds))
                                      box w=fill p=3.0 bg=surface border=border border-w=1.0 r=8.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
                                        col w=fill gap=1.0
                                          for kind in slash_kind_matches(block_draft, editable_block_kinds)
                                            button label="Set block kind" w=fill h=24.0 p=4.0 @ghost_action -> pick_slash_kind(kind)
                                              row w=fill h=fill gap=6.0 align=center
                                                text kind w=fill size=13.0 wrap=none @text-fg
                                              active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                              hovered bg=brand/14 text=fg
                                              pressed bg=brand/20
                                if new_block_kind == "Divider"
                                  button "Insert divider" disabled=loading w=fill h=28.0 p=5.0 @secondary_action -> add_block_submit
              overlay when=(connected && !empty(active_page) && block_comments_open) dismiss=close_block_comments backdrop=transparent p=12.0 align-x=end align-y=start
                content
                  space w=fill h=fill
                layer
                  box w=300.0 h=380.0 p=8.0 bg=surface border=border border-w=1.0 r=11.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
                    col w=fill h=fill gap=6.0
                      row w=fill gap=6.0 align=center
                        text "Comments" w=fill size=14.0 font=display @text-fg
                        if block_comment_thread_total > 0
                          text block_comment_thread_total size=12.0 font=code @text-muted
                        if block_comment_threads_loading || block_thread_comments_loading
                          text "Loading…" size=12.5 @text-muted
                        button "×" label="Close comments" disabled=(mutation_phase != "idle") w=24.0 h=24.0 p=4.0 @secondary_action -> close_block_comments
                          active bg=transparent text=muted r=6.0
                          hovered bg=fg/10 text=fg
                          pressed bg=fg/15
                      if empty(active_block_comment_thread)
                        scroll dir=vertical w=fill h=fill
                          col w=fill gap=1.0
                            if empty(block_comment_threads) && !block_comment_threads_loading
                              text "No comments yet" w=fill size=12.5 align-x=center @text-muted
                            for comment_thread in block_comment_threads
                              PageCommentThreadButton thread=comment_thread
                            if block_comment_threads_has_more
                              button "More" disabled=(block_comment_threads_loading || mutation_phase != "idle") h=24.0 p=4.0 @secondary_action -> load_more_block_threads
                                active bg=transparent text=muted r=6.0
                                hovered bg=fg/9 text=fg
                                pressed bg=fg/14
                      if !empty(active_block_comment_thread)
                        row w=fill gap=5.0 align=center
                          button "← Threads" disabled=(block_thread_comments_loading || mutation_phase != "idle") h=24.0 p=4.0 @secondary_action -> close_block_comment_thread
                            active bg=transparent text=muted r=6.0
                            hovered bg=fg/9 text=fg
                            pressed bg=fg/14
                        scroll dir=vertical w=fill h=fill
                          col w=fill gap=1.0
                            for page_comment in block_thread_comments
                              PageCommentCard comment=page_comment
                            if block_thread_comments_has_more
                              button "More" disabled=(block_thread_comments_loading || mutation_phase != "idle") h=24.0 p=4.0 @secondary_action -> load_more_block_comments
                                active bg=transparent text=muted r=6.0
                                hovered bg=fg/9 text=fg
                                pressed bg=fg/14
                      row w=fill gap=5.0 align=center
                        input "" #block-comment(scope_key(connected_rpc, selected_block_id)) label="New block comment" <-> block_comment_draft hint="Add a comment…" disabled=(mutation_phase != "idle" || block_comment_threads_loading || block_thread_comments_loading) submit=post_block_comment_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                          active bg=transparent border=fg/8 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                          hovered bg=fg/4 border=fg/11
                          disabled value=muted
                        button "Post" disabled=(mutation_phase != "idle" || empty(trim(block_comment_draft)) || block_comment_threads_loading || block_thread_comments_loading) h=28.0 p=5.0 @primary_action -> post_block_comment_submit
              overlay when=(connected && !empty(active_page) && !empty(selected_block_id) && block_actions_open) dismiss=close_block_actions backdrop=transparent p=0.0 align-x=start align-y=start
                content
                  space w=fill h=fill
                layer
                  float x=(block_menu_x + 10.0) y=block_menu_y
                    BlockActionsMenu block_id=selected_block_id kind=selected_block_kind disabled=(loading || mutation_phase != "idle") delete_armed=block_delete_armed editable_kinds=editable_block_kinds
    files:
      col w=fill h=fill
        ScreenHeader title="Files" meta=fs_path
          space w=1.0 h=1.0
        col w=fill h=fill p=18.0 gap=11.0
          row w=fill h=28.0 gap=8.0 align=center
            button "↑" label="Parent directory" disabled=(fs_loading || empty(fs_path)) w=26.0 h=26.0 p=0.0 @icon_action -> fs_open_parent
              active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=7.0
              hovered bg=fg/10 text=fg
              pressed bg=fg/14
            text fs_path w=fill size=12.0 wrap=none font=code @text-fg
            if fs_loading
              text "Loading…" size=12.5 @text-muted
            input "" #fs-new label="New entry name" <-> fs_new_name change=fs_new_name_changed hint="new name…" disabled=fs_loading w=140.0 p=5.0 text-size=13.0 line-h=1.2 @control
              active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
              hovered bg=elevated border=fg/21
              disabled bg=muted_bg/54 value=muted
            button "+ Folder" disabled=(fs_loading || empty(trim(fs_new_name))) h=26.0 p=5.0 @secondary_action -> fs_mkdir_submit
            button "+ File" disabled=(fs_loading || empty(trim(fs_new_name))) h=26.0 p=5.0 @secondary_action -> fs_new_file_submit
            button "History" h=26.0 p=5.0 @secondary_action -> fs_toggle_history
              active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=7.0
              hovered bg=fg/10 text=fg
              pressed bg=fg/14
          row w=fill h=fill gap=10.0
            box w=340.0 h=fill p=6.0 bg=muted_bg border=fg/10 border-w=1.0 r=10.0
              stack w=fill h=fill
                if empty(fs_entries) && !fs_loading
                  EmptyState title="Empty directory" description="Nothing committed under this path."
                if !empty(fs_entries)
                  scroll dir=vertical w=fill h=fill
                    col w=fill gap=1.0
                      for entry in fs_entries
                        col w=fill
                          if entry.kind == "dir"
                            button label="Open directory" w=fill p=6.0 @ghost_action -> fs_open_dir(entry.path)
                              row w=fill h=fill gap=8.0 align=center
                                text "▸" w=14.0 size=12.0 align-x=center @text-muted
                                text entry.name w=fill size=13.0 wrap=none font=medium @text-fg
                              active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
                              hovered bg=row_hover text=fg
                              pressed bg=accent
                          if entry.kind != "dir"
                            row w=fill gap=2.0 align=center
                              button label="Preview file" w=fill p=6.0 @ghost_action -> fs_open_file(entry.path)
                                row w=fill h=fill gap=8.0 align=center
                                  text "·" w=14.0 size=12.0 align-x=center @text-muted
                                  text entry.name w=fill size=13.0 wrap=none @text-fg
                                  text entry.size size=12.0 wrap=none font=code @text-muted
                                active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
                                hovered bg=row_hover text=fg
                                pressed bg=accent
                              if entry.path == fs_delete_target
                                button "rm!" label="Confirm delete" disabled=fs_loading w=34.0 h=22.0 p=0.0 @danger_action -> fs_delete_submit
                              if entry.path != fs_delete_target
                                button "×" label="Delete file" w=22.0 h=22.0 p=0.0 @danger_action -> fs_arm_delete(entry.path)
            box w=fill h=fill p=8.0 bg=muted_bg border=fg/10 border-w=1.0 r=10.0
              stack w=fill h=fill
                if fs_history_open
                  scroll dir=vertical w=fill h=fill
                    col w=fill gap=4.0
                      box w=fill pl=4.0
                        text "SNAPSHOTS" size=10.0 font=code_semibold @text-muted
                      if !empty(fs_diff_from)
                        col w=fill gap=3.0
                          row w=fill gap=8.0 align=center
                            text "Changes vs head" w=fill size=13.0 font=medium @text-muted
                            button "Back" h=22.0 p=4.0 @secondary_action -> fs_close_diff
                              active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=6.0
                              hovered bg=fg/10 text=fg
                              pressed bg=fg/14
                          if empty(fs_diff)
                            text "No differences." size=12.5 @text-muted
                          for entry in fs_diff
                            row w=fill gap=8.0 align=center
                              text entry.kind w=64.0 size=12.0 wrap=none font=code @text-muted
                              text entry.path w=fill size=12.0 wrap=none font=code @text-fg
                      if empty(fs_diff_from)
                        col w=fill gap=4.0
                          for snapshot in fs_history
                            box w=fill p=7.0 bg=surface border=fg/10 border-w=1.0 r=8.0
                              col w=fill gap=2.0
                                row w=fill gap=8.0 align=center
                                  text snapshot.short_id size=12.0 wrap=none font=code @text-fg
                                  text snapshot.height size=12.0 wrap=none font=code @text-muted
                                  space w=fill
                                  text snapshot.author size=12.0 wrap=none font=code @text-muted
                                  button "Diff" h=20.0 p=3.0 @ghost_action -> fs_show_diff(snapshot.id)
                                    active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=6.0
                                    hovered bg=fg/10 text=fg
                                    pressed bg=fg/14
                                if !empty(snapshot.message)
                                  text snapshot.message size=13.5 @text-fg
                if !fs_history_open && empty(fs_preview_path)
                  EmptyState title="Select a file" description="Text files preview here; History shows the commit window."
                if !fs_history_open && !empty(fs_preview_path)
                  col w=fill h=fill gap=6.0
                    row w=fill gap=8.0 align=center
                      text fs_preview_path w=fill size=12.0 wrap=none font=code @text-muted
                      if fs_preview_truncated
                        text "first 64 KiB" size=12.5 wrap=none @text-muted
                      if !fs_preview_binary && !fs_editing && !fs_preview_truncated
                        button "Edit" h=22.0 p=4.0 @ghost_action -> fs_begin_edit
                          active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=6.0
                          hovered bg=fg/10 text=fg
                          pressed bg=fg/14
                      if fs_editing
                        button "Cancel" h=22.0 p=4.0 @secondary_action -> fs_cancel_edit
                          active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=6.0
                          hovered bg=fg/10 text=fg
                          pressed bg=fg/14
                      if fs_editing
                        button "Save" disabled=fs_loading h=22.0 p=4.0 @primary_action -> fs_save_edit
                    stack w=fill h=fill
                      if fs_editing
                        editor #fs-editor <-> fs_editor hint="File contents…" disabled=fs_loading min-h=200.0 size=12.0 line-h=1.3 p=6.6 wrap=word
                          active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                          hovered bg=muted_bg border=fg/21
                          focused bg=muted_bg border=ring border-w=1.0
                      if !fs_editing
                        scroll dir=vertical w=fill h=fill
                          col w=fill gap=6.0
                            if fs_preview_binary
                              text fs_preview_text size=12.0 font=code @text-muted
                            if !fs_preview_binary
                              text fs_preview_text size=12.0 font=code @text-fg
    members:
      col w=fill h=fill
        ScreenHeader title="Members" meta=members_summary(members_validators, members_residents)
          space w=1.0 h=1.0
        if empty(members_rows)
          box w=fill h=fill p=22.0
            EmptyPlate message="No members yet — validators and residents appear as they join."
        if !empty(members_rows)
          scroll dir=vertical w=fill h=fill
            col w=fill pl=12.0 pr=12.0 pt=6.0 pb=6.0
              for member in members_rows
                MemberRowCard member=member
    agents:
      col w=fill h=fill
        ScreenHeader title="Agents" meta=agents_summary(agents_rows)
          space w=1.0 h=1.0
        if empty(agents_rows)
          box w=fill h=fill p=22.0
            EmptyPlate message="No agents registered — a registered agent appears here with its capability and grants."
        if !empty(agents_rows)
          scroll dir=vertical w=fill h=fill
            col w=fill p=18.0 gap=11.0
              for agent in agents_rows
                AgentCard agent=agent
    forge:
      col w=fill h=fill
        ScreenHeader title="Forge" meta=forge_repo
          space w=1.0 h=1.0
        col w=fill h=fill p=18.0 gap=11.0
          if empty(forge_repos)
            EmptyState title="No repos" description="Consensus-backed repos appear here once created."
          if !empty(forge_repos) && empty(forge_repo)
            scroll dir=vertical w=fill h=fill
              col w=fill gap=2.0
                for repo in forge_repos
                  button label="Open repo" w=fill p=8.0 @ghost_action -> forge_open_repo(repo.name)
                    row w=fill h=fill gap=8.0 align=center
                      text repo.name w=fill size=13.0 wrap=none font=medium @text-fg
                      text repo.head size=12.0 wrap=none font=code @text-muted
                    active bg=muted_bg text=fg border=fg/8 border-w=1.0 r=9.0
                    hovered bg=row_hover text=fg
                    pressed bg=accent
          if !empty(forge_repo) && forge_item_number <= 0
            col w=fill h=fill gap=6.0
              if !empty(forge_branches)
                scroll dir=horizontal w=fill h=26.0 bar=hidden
                  row h=fill gap=4.0 align=center
                    for branch in forge_branches
                      box h=20.0 pl=7.0 pr=7.0 align-y=center bg=muted_bg border=fg/10 border-w=1.0 r=10.0
                        text branch size=9.0 wrap=none font=code_semibold @text-muted
              if empty(forge_items)
                EmptyState title="No issues or PRs" description="The tracker is empty for this repo."
              if !empty(forge_items)
                scroll dir=vertical w=fill h=fill
                  col w=fill gap=2.0
                    for item in forge_items
                      button label="Open item" w=fill p=7.0 @ghost_action -> forge_open_item(item.number)
                        row w=fill h=fill gap=8.0 align=center
                          text item.number size=12.0 wrap=none font=code @text-muted
                          Badge.Outline label=item.kind
                          text item.title w=fill size=13.0 wrap=none @text-fg
                          StatusBadge label=item.state
                        active bg=muted_bg text=fg border=fg/8 border-w=1.0 r=8.0
                        hovered bg=row_hover text=fg
                        pressed bg=accent
          if forge_item_number > 0
            col w=fill h=fill gap=6.0
              row w=fill gap=8.0 align=center
                button "‹ Back" h=24.0 p=5.0 @ghost_action -> forge_close_item
                  active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=7.0
                  hovered bg=fg/10 text=fg
                  pressed bg=fg/14
                text forge_item_title w=fill size=14.0 wrap=none font=display @text-fg
                Badge.Outline label=forge_item_kind
                StatusBadge label=forge_item_state
              row w=fill gap=8.0 align=center
                if !empty(forge_item_author)
                  text forge_item_author size=11.0 wrap=none font=code_medium @text-muted
                if !empty(forge_item_branches)
                  text forge_item_branches size=12.0 wrap=none font=code @text-muted
                if forge_item_files_changed > 0
                  text forge_stats(forge_item_files_changed, forge_item_additions, forge_item_deletions) size=12.0 wrap=none font=code @text-muted
                space w=fill
              scroll dir=vertical w=fill h=fill
                col w=fill gap=8.0
                  if !empty(forge_item_body)
                    box w=fill p=11.0 style=card_style()
                      text forge_item_body size=13.5 @text-fg
                  if !empty(forge_item_diff)
                    box w=fill p=11.0 style=card_style()
                      col w=fill gap=5.0
                        if forge_item_diff_truncated
                          text "Patch truncated — the statistics cover the full diff." size=12.5 @text-muted
                        text forge_item_diff size=12.0 font=code @text-fg
                  if forge_item_kind == "pr"
                    box w=fill p=11.0 style=card_style()
                      col w=fill gap=6.0
                        row w=fill gap=6.0 align=center
                          text "Merge" w=fill size=14.0 font=display @text-fg
                          text forge_item_approvals size=12.0 wrap=none font=code @text-success
                          text "approvals" size=12.5 wrap=none @text-muted
                          text "·" size=11.0 wrap=none font=code_medium @text-muted
                          text forge_item_change_requests size=12.0 wrap=none font=code @text-muted
                          text "change requests" size=12.5 wrap=none @text-muted
                        if forge_item_state == "merged"
                          text forge_merge_note(forge_item_merge_oid, forge_item_branches) size=12.0 font=code @text-success
                        if forge_item_state == "closed"
                          text "Closed without merging." size=12.5 @text-muted
                        if forge_item_state == "open"
                          if !empty(forge_merge_conflicts)
                            col w=fill gap=3.0
                              text "Merge conflicts — resolve on the branch and push again:" size=12.5 @text-muted
                              for conflict_path in forge_merge_conflicts
                                text conflict_path size=12.0 font=code @text-fg
                          row w=fill gap=8.0 align=center
                            if !forge_merge_busy
                              button "Merge pull request" disabled=(!connected || empty(forge_item_source_oid)) h=28.0 p=6.0 @primary_action -> forge_merge_submit
                            if forge_merge_busy
                              button "Merging…" disabled=true h=28.0 p=6.0 @primary_action -> forge_merge_submit
                            text "Approvals are advisory — merging is never gated." size=12.5 wrap=none @text-muted
                  if forge_item_kind == "pr"
                    box w=fill p=11.0 style=card_style()
                      col w=fill gap=6.0
                        text "Reviews" size=14.0 font=display @text-fg
                        if empty(forge_item_reviews)
                          text "No reviews yet." size=12.5 @text-muted
                        for review in forge_item_reviews
                          box w=fill p=8.0 bg=elevated border=fg/8 border-w=1.0 r=8.0
                            col w=fill gap=4.0
                              row w=fill gap=7.0 align=center
                                text review.author_name size=14.0 wrap=none font=display @text-fg
                                if review.verdict == "approve"
                                  text verdict_label(review.verdict) size=10.5 wrap=none font=code_medium @text-success
                                if review.verdict != "approve"
                                  text verdict_label(review.verdict) size=10.5 wrap=none font=code_medium @text-danger
                                text review.commit size=12.0 wrap=none font=code @text-muted
                                if review.outdated
                                  text "outdated" size=9.0 wrap=none font=code_semibold @text-muted
                                space w=fill
                              if !empty(review.body)
                                text review.body size=13.5 @text-fg
                              for comment in review.comments
                                box w=fill p=6.0 bg=muted_bg border=fg/8 border-w=1.0 r=7.0
                                  col w=fill gap=2.0
                                    text comment.anchor size=12.0 font=code @text-muted
                                    text comment.body size=13.5 @text-fg
                        row w=fill gap=6.0 align=center
                          button label="Pick comment verdict" h=24.0 p=5.0 @ghost_action -> forge_review_pick("comment")
                            text verdict_pick_label(forge_review_verdict, "comment", "Comment") size=13.0
                            active bg=fg/6 text=fg border=fg/10 border-w=1.0 r=7.0
                            hovered bg=fg/10 text=fg
                            pressed bg=fg/14
                          button label="Pick approve verdict" h=24.0 p=5.0 @ghost_action -> forge_review_pick("approve")
                            text verdict_pick_label(forge_review_verdict, "approve", "Approve") size=13.0
                            active bg=brand/14 text=fg border=brand/26 border-w=1.0 r=7.0
                            hovered bg=brand/22 text=fg
                            pressed bg=brand/30
                          button label="Pick request-changes verdict" h=24.0 p=5.0 @ghost_action -> forge_review_pick("request_changes")
                            text verdict_pick_label(forge_review_verdict, "request_changes", "Request changes") size=13.0
                            active bg=danger/10 text=fg border=danger/26 border-w=1.0 r=7.0
                            hovered bg=danger/18 text=fg
                            pressed bg=danger/24
                          space w=fill
                        row w=fill gap=6.0 align=center
                          input "" #forge-review-body label="Review body" <-> forge_review_draft hint="Leave a review…" disabled=(forge_review_busy || !connected) submit=forge_review_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                            active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                            hovered bg=elevated border=fg/21
                            disabled bg=fg/6 value=muted
                          button "Submit review" disabled=(forge_review_busy || !connected || empty(forge_item_source_oid)) h=28.0 p=6.0 @primary_action -> forge_review_submit
                  box w=fill p=11.0 style=card_style()
                    col w=fill gap=6.0
                      text "Discussion" size=14.0 font=display @text-fg
                      if empty(forge_discussion)
                        text "No discussion yet." size=12.5 @text-muted
                      for message in forge_discussion
                        row w=fill gap=9.0 align=start
                          MessageAvatar initials=message.initial kind=message.avatar_kind
                          col w=fill gap=2.0
                            row w=fill gap=7.0 align=center
                              text message.author size=13.0 wrap=none font=display @text-fg
                              text message.meta size=11.0 wrap=none font=code_medium @text-muted
                              space w=fill
                            MessageBody message=message
                      flex w=fill gap=8.0 items=end
                        editor #forge-note <-> forge_discussion_editor hint="Write a note…" disabled=(loading || !connected || empty(forge_item_channel)) min-h=38.0 max-h=120.0 size=13.5 line-h=1.3 p=6.0 wrap=word key-binding=composer_keys() -> forge_note_submit
                          active bg=transparent border=fg/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                          hovered bg=fg/4 border=fg/12 border-w=1.0
                          focused bg=fg/6 border=ring border-w=1.0
                          disabled value=muted
                        button "Send" disabled=(loading || !connected || empty(forge_item_channel) || !empty(forge_discussion_pending) || empty(trim(editor_text(forge_discussion_editor)))) w=60.0 h=28.0 p=6.0 @primary_action -> forge_note_submit
    governance:
      scroll dir=vertical w=fill h=fill
        col w=fill p=22.0 gap=16.0
          row gap=9.0 align=center
            text "Approvals" size=16.0 wrap=none font=display @text-primary
            if !empty(gov_rows)
              CountChip label=proposals_summary(gov_rows)
          if empty(gov_rows)
            EmptyPlate message="No proposals — membership and share actions appear here as they are opened."
          if !empty(gov_rows)
            col w=fill gap=12.0
              for proposal in gov_rows
                if proposal.open
                  ProposalCard proposal=proposal busy=(!empty(gov_voting))
          if !empty(gov_rows)
            col w=fill gap=10.0
              GroupLabel label="RECENTLY FINALIZED"
              for proposal in gov_rows
                if !proposal.open
                  SettledProposalRow proposal=proposal
    node:
      col w=fill h=fill
        ScreenHeader title="Node" meta=height_label(block_height)
          input "" #log-filter label="Filter logs" <-> node_log_filter change=node_log_filter_changed hint="filter logs…" w=200.0 p=6.2 text-size=13.0 line-h=1.2 @control
            active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
            hovered bg=muted_bg border=control_line
        col w=fill h=fill p=18.0 gap=11.0
          if !empty(node_peers)
            box w=fill p=8.0 bg=muted_bg border=fg/8 border-w=1.0 r=9.0
              col w=fill gap=3.0
                text "PEERS" size=10.0 font=code_semibold @text-muted
                for peer in node_peers
                  row w=fill gap=8.0 align=center
                    if peer.live
                      box w=7.0 h=7.0 bg=success_dot r=3.5
                        text ""
                    if !peer.live
                      box w=7.0 h=7.0 bg=fg/30 r=3.5
                        text ""
                    text peer.key w=fill size=12.0 wrap=none font=code @text-fg
                    text peer.height size=12.0 wrap=none font=code @text-muted
          box w=fill h=fill p=8.0 bg=muted_bg border=fg/8 border-w=1.0 r=9.0
            stack w=fill h=fill
              if empty(node_log_lines)
                EmptyState title="Waiting for logs" description="The node's log ring streams here live."
              if !empty(node_log_lines)
                scroll dir=vertical w=fill h=fill
                  col w=fill gap=1.0
                    for line in filter_log_lines(node_log_lines, node_log_filter)
                      text line.line size=12.0 font=code @text-fg
    settings:
      scroll dir=vertical w=fill h=fill
        col w=fill p=22.0 gap=18.0
          text "Settings" size=16.0 wrap=none font=display @text-primary
          grid min-cell=420.0 gap=22.0
            col w=fill gap=9.0
              GroupLabel label="NETWORK"
              GroupCard
                col w=fill
                  KeyValueRow label="Workspace" value=network_label(account_name, connected_rpc) last=false
                  KeyValueRow label="Endpoint" value=settings_endpoint last=false
                  KeyValueRow label="Node key" value=settings_node_key last=false
                  KeyValueRow label="Block height" value=height_label(settings_height) last=true
            col w=fill gap=9.0
              GroupLabel label="CONNECTION"
              box w=fill p=15.0 bg=surface border=card_line border-w=1.0 r=11.0
                col w=fill gap=9.0
                  input "" #rpc label="RPC endpoint" <-> rpc hint="Node URL" disabled=(loading || (mutation_phase != "idle" && mutation_phase != "recovering")) submit=reconnect w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                    active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
                    hovered bg=elevated border=fg/21
                    disabled bg=muted_bg/54 value=muted
                  input "" #password label="Local key password" secure=true <-> password hint="Key password" disabled=(loading || mutation_phase != "idle") w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                    active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
                    hovered bg=elevated border=fg/21
                    disabled bg=muted_bg/54 value=muted
                  row w=fill gap=9.0 align=center
                    if connection_degraded(status)
                      Badge.Destructive label=status
                    if !connection_degraded(status)
                      Badge.Success label=status
                    space w=fill
                    button "Connect" disabled=(loading || (mutation_phase != "idle" && mutation_phase != "recovering")) h=32.0 p=7.0 @primary_action -> reconnect
            col w=fill gap=9.0
              GroupLabel label="YOUR IDENTITY"
              box w=fill p=15.0 bg=surface border=card_line border-w=1.0 r=11.0
                row w=fill gap=13.0 align=center
                  PersonAvatar initials=initial_of(account_name) plate=40.0 ink=14.0
                  col w=fill gap=3.0
                    row w=fill gap=7.0 align=center
                      if !empty(account_name)
                        text account_name size=13.5 wrap=none font=display @text-fg
                      if empty(account_name)
                        text "(unnamed)" size=13.5 wrap=none @text-muted
                      if account_bound
                        Badge.Secondary label="BOUND"
                      if !account_bound
                        Badge.Outline label="UNBOUND"
                    text account_id size=10.5 wrap=none font=code_medium @text-hint
                  col gap=5.0
                    row w=fill h=28.0 gap=5.0 align=center
                      input "" #account-rename label="New display name" <-> account_name_draft change=account_name_draft_changed hint="rename account…" disabled=account_renaming w=150.0 p=5.0 text-size=13.0 line-h=1.2 @control
                        active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                        hovered bg=elevated border=fg/21
                        disabled bg=muted_bg/54 value=muted
                      button "Rename" disabled=(account_renaming || empty(trim(account_name_draft))) h=28.0 p=5.0 @secondary_action -> account_rename_submit
                    row gap=8.0 align=center
                      text account_members size=12.0 wrap=none font=code @text-meta
                      text "keys" size=12.5 wrap=none @text-meta
                      text account_nodes size=12.0 wrap=none font=code @text-meta
                      text "nodes" size=12.5 wrap=none @text-meta
            col w=fill gap=9.0
              GroupLabel label="IDENTITY KEY"
              GroupCard
                col w=fill
                  KeyValueRow label="Key state" value=settings_key_state last=false
                  KeyValueRow label="Key path" value=settings_key_path last=true
            col w=fill gap=9.0
              GroupLabel label="THIS DEVICE"
              box w=fill bg=surface border=card_line border-w=1.0 r=11.0 clip=true
                col w=fill
                  box w=fill px=15.0 py=13.0
                    row w=fill gap=10.0 align=center
                      col w=fill gap=1.0
                        text "Open page tabs" size=12.5 @text-accent_fg
                        text "Preferences persist per endpoint in app-prefs.json beside the user key." size=12.5 @text-meta
                      text settings_open_tabs size=12.0 wrap=none font=code_medium @text-secondary_fg
                      button "Forget tabs" h=28.0 p=5.0 @secondary_action -> settings_clear_tabs
    explorer:
      col w=fill h=fill
        col w=fill pl=24.0 pr=24.0 pt=22.0 gap=16.0
          ScreenTitle title="Explorer" detail="Every block this node has verified for itself, newest first. Open one for the ops it carried."
          row w=fill gap=10.0 align=center
            if explorer_loading
              text "Loading…" size=12.5 @text-caption
            space w=fill
            button "Refresh" disabled=explorer_loading h=30.0 p=7.0 @outline_action -> refresh_explorer
        col w=fill h=fill p=18.0 gap=11.0
          if empty(explorer_blocks) && !explorer_loading
            EmptyState title="No blocks yet" description="Non-empty blocks appear here as they finalize."
          if !empty(explorer_blocks)
            row w=fill h=fill gap=10.0
              box w=340.0 h=fill p=6.0 bg=muted_bg border=fg/10 border-w=1.0 r=10.0
                scroll dir=vertical w=fill h=fill
                  col w=fill gap=1.0
                    for block in explorer_blocks
                      button label="Inspect block" w=fill p=6.0 @ghost_action -> select_explorer_block(block.height)
                        row w=fill h=fill gap=8.0 align=center
                          text block.height size=12.0 wrap=none font=code @text-fg
                          text block.hash w=fill size=12.0 wrap=none font=code @text-muted
                          text block.op_count size=12.0 wrap=none font=code @text-muted
                        active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
                        hovered bg=row_hover text=fg
                        pressed bg=accent
              box w=fill h=fill p=8.0 bg=muted_bg border=fg/10 border-w=1.0 r=10.0
                stack w=fill h=fill
                  if explorer_selected <= 0
                    EmptyState title="Select a block" description="Its operations and dispatch traces appear here."
                  if explorer_selected > 0
                    scroll dir=vertical w=fill h=fill
                      col w=fill gap=6.0
                        for op in explorer_ops_at(explorer_ops, explorer_selected)
                          box w=fill p=8.0 bg=surface border=fg/10 border-w=1.0 r=9.0
                            col w=fill gap=3.0
                              row w=fill gap=8.0 align=center
                                text op.target size=14.0 wrap=none font=display @text-fg
                                StatusBadge label=op.disposition
                                space w=fill
                                text op.op_hash size=12.0 wrap=none font=code @text-muted
                              row w=fill gap=8.0 align=center
                                text "by" size=11.0 wrap=none font=code_medium @text-muted
                                text op.proposer size=12.0 wrap=none font=code @text-muted
                              if !empty(op.trace)
                                text op.trace size=12.0 font=code @text-muted
                              text op.payload size=13.5 @text-fg
    palette:
      stack w=fill h=fill
        if palette_open
          box w=fill h=fill align-x=center pt=72.0 bg=scrim
            box w=540.0 p=10.0 bg=surface border=border border-w=1.0 r=14.0 shadow=shadow_modal shadow-y=24.0 shadow-blur=60.0
              col w=fill gap=8.0
                input "" #palette-input label="Search everything" <-> palette_draft change=palette_changed hint="Search messages and pages… (Esc closes)" submit=close_palette w=fill p=8.0 text-size=13.0 line-h=1.2 @control
                  active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
                  hovered bg=elevated border=fg/21
                if palette_searching
                  text "Searching…" size=12.5 @text-muted
                if !empty(palette_chat_hits) || !empty(palette_page_hits)
                  scroll dir=vertical w=fill h=380.0
                    col w=fill gap=4.0
                      if !empty(palette_chat_hits)
                        box w=fill pl=4.0
                          text "MESSAGES" size=10.0 font=code_semibold @text-muted
                        col w=fill gap=1.0
                          for hit in palette_chat_hits
                            button label="Open message" w=fill p=6.0 @ghost_action -> open_chat_search_hit(hit.channel_id, hit.root_seq, hit.seq)
                              col w=fill gap=1.0
                                text hit.text size=13.0 wrap=none @text-fg
                                text hit.meta size=11.0 wrap=none font=code_medium @text-muted
                              active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                              hovered bg=row_hover text=fg
                              pressed bg=accent
                      if !empty(palette_page_hits)
                        box w=fill pl=4.0
                          text "PAGES" size=10.0 font=code_semibold @text-muted
                        col w=fill gap=1.0
                          for hit in palette_page_hits
                            button label="Open page" w=fill p=6.0 @ghost_action -> open_page_search_hit(hit.page_id, hit.block_id)
                              col w=fill gap=1.0
                                text hit.text size=13.0 wrap=none @text-fg
                                text hit.kind size=12.0 wrap=none font=code @text-muted
                              active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                              hovered bg=row_hover text=fg
                              pressed bg=accent
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
