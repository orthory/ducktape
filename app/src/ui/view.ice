view
  WorkspaceTabs status=status loading=(loading || mutation_phase != "idle") degraded=connection_degraded(status) tab=shell_tab bell_count=bell_unread #workspace-tabs
    connection:
      container width=fill padding=6.0 bg=transparent border=fg/11 border-w=1.0 r=10.0
        col width=fill spacing=5.0
          input "" #rpc label="RPC endpoint" <-> rpc hint="Node URL" disabled=(loading || (mutation_phase != "idle" && mutation_phase != "recovering")) submit=reconnect width=fill padding=6.2 text-size=13.0 line-height=1.2
            active bg=surface border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
            hovered bg=elevated border=fg/21
            focused bg=elevated border=fg/45 border-w=1.0
            disabled bg=surface/54 value=muted
          input "" #password label="Local key password" secure=true <-> password hint="Key password" disabled=(loading || mutation_phase != "idle") width=fill padding=6.2 text-size=13.0 line-height=1.2
            active bg=surface border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
            hovered bg=elevated border=fg/21
            focused bg=elevated border=fg/45 border-w=1.0
            disabled bg=surface/54 value=muted
          button "Connect" disabled=(loading || (mutation_phase != "idle" && mutation_phase != "recovering")) width=fill height=28.0 padding=7.0 -> reconnect
            active bg=fg/90 text=bg border=fg/6 border-w=1.0 r=10.0
            hovered bg=fg/82 text=bg
            pressed bg=fg text=bg
            disabled bg=fg/36 text=bg/14
    chat_sidebar:
      col width=fill height=fill spacing=7.0
        row width=fill padding-left=7.0 padding-right=7.0 spacing=6.0 align=center
          text "CHANNELS" width=fill size=11.0 font=medium @text-muted
          text len(channels) size=11.0 @text-muted
          if !channel_create_open
            button label="New channel" disabled=(loading || mutation_phase != "idle" || !connected) width=28.0 height=28.0 padding=0.0 -> toggle_channel_create
              container width=fill height=fill align-x=center align-y=center
                text "+" size=14.0
              active bg=transparent text=muted r=8.0
              hovered bg=fg/10 text=fg
              pressed bg=selection
              disabled text=muted
          if channel_create_open
            button label="Close new channel" disabled=(loading || mutation_phase != "idle") width=28.0 height=28.0 padding=0.0 -> toggle_channel_create
              container width=fill height=fill align-x=center align-y=center
                text "×" size=14.0
              active bg=transparent text=muted r=8.0
              hovered bg=fg/10 text=fg
              pressed bg=selection
        if channel_create_open
          col width=fill spacing=4.0
            row width=fill height=28.0 spacing=5.0 align=center
              input "" #new-channel label="New channel name" <-> channel_draft hint="New channel" disabled=(loading || mutation_phase != "idle" || !connected) submit=create_channel_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
                active bg=surface border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                hovered bg=elevated border=fg/21
                focused bg=elevated border=fg/45 border-w=1.0
                disabled bg=surface/54 value=muted
              button label="Create channel" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(channel_draft))) width=28.0 height=28.0 padding=0.0 -> create_channel_submit
                container width=fill height=fill align-x=center align-y=center
                  text "+" size=14.0
                active bg=fg/13 text=fg border=fg/18 border-w=1.0 r=8.0
                hovered bg=fg/19
                pressed bg=selection
                disabled bg=fg/5 text=muted
            button label="Members-only posting" width=fill height=24.0 padding=4.0 -> toggle_channel_create_members_only
              row width=fill height=fill spacing=6.0 align=center
                if channel_create_members_only
                  text "☑" size=13.0 @text-primary
                if !channel_create_members_only
                  text "☐" size=13.0 @text-muted
                text "Members-only posting" width=fill size=11.0 wrapping=none @text-muted
              active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
              hovered bg=fg/5 text=fg
              pressed bg=fg/8
        scroll direction=vertical width=fill height=fill
          col width=fill spacing=2.0
            for channel in channels
              ChannelButton channel=channel selected=(channel.id == active_channel) unread=channel_is_unread(channel_reads, channel.id, channel.head_seq)
    pages_sidebar:
      col width=fill height=fill spacing=7.0
        row width=fill padding-left=7.0 padding-right=7.0 spacing=6.0 align=center
          text "PAGES" width=fill size=11.0 font=medium @text-muted
          text len(pages) size=11.0 @text-muted
          if !page_create_open
            button label="New page" disabled=(loading || mutation_phase != "idle" || !connected) width=28.0 height=28.0 padding=0.0 -> toggle_page_create
              container width=fill height=fill align-x=center align-y=center
                text "+" size=14.0
              active bg=transparent text=muted r=8.0
              hovered bg=fg/10 text=fg
              pressed bg=selection
              disabled text=muted
          if page_create_open
            button label="Close new page" disabled=(loading || mutation_phase != "idle") width=28.0 height=28.0 padding=0.0 -> toggle_page_create
              container width=fill height=fill align-x=center align-y=center
                text "×" size=14.0
              active bg=transparent text=muted r=8.0
              hovered bg=fg/10 text=fg
              pressed bg=selection
        if page_create_open
          row width=fill height=28.0 spacing=5.0 align=center
            input "" #new-page label="New page title" <-> page_draft hint="New page" disabled=(loading || mutation_phase != "idle" || !connected) submit=create_page_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
              active bg=surface border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
              hovered bg=elevated border=fg/21
              focused bg=elevated border=fg/45 border-w=1.0
              disabled bg=surface/54 value=muted
            button label="Create page" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(page_draft))) width=28.0 height=28.0 padding=0.0 -> create_page_submit
              container width=fill height=fill align-x=center align-y=center
                text "+" size=14.0
              active bg=fg/13 text=fg border=fg/18 border-w=1.0 r=8.0
              hovered bg=fg/19
              pressed bg=selection
              disabled bg=fg/5 text=muted
        scroll direction=vertical width=fill height=fill
          col width=fill spacing=2.0
            for page in pages
              PageButton page=page selected=(page.id == active_page)
    notice:
      col width=fill
        if error != ""
          container width=fill padding-left=12.0 padding-right=12.0 padding-bottom=8.0
            container width=fill padding=8.0 bg=elevated border=fg/18 border-w=1.0 r=12.0 shadow=black/12 shadow-y=2.0 shadow-blur=12.0
              row width=fill spacing=8.0 align=center
                container width=20.0 height=20.0 align-x=center align-y=center bg=fg/12 border=fg/20 border-w=1.0 r=10.0
                  text "!" size=11.0 font=medium @text-fg
                text error width=fill size=13.0 @text-fg
                button "Dismiss" height=26.0 padding=5.0 -> dismiss_error
                  active bg=transparent text=muted r=7.0
                  hovered bg=fg/9 text=fg
                  pressed bg=fg/14
    chat:
      container width=fill height=fill bg=transparent clip=true px-snap=true
        row width=fill height=fill
          col width=fill height=fill spacing=9.0 padding=14.0
            if !empty(active_channel)
              col width=fill spacing=12.0
                row width=fill height=32.0 spacing=9.0 align=center
                  text "#" size=18.0 wrapping=none font=display @text-primary
                  text active_channel_name size=16.0 wrapping=none font=display @text-fg
                  if active_channel_archived
                    container padding=2.0 padding-left=7.0 padding-right=7.0 bg=fg/6 border=fg/13 border-w=1.0 r=6.0
                      text "Archived" size=11.0 wrapping=none font=medium @text-muted
                  if active_channel_members_only
                    container padding=2.0 padding-left=7.0 padding-right=7.0 bg=primary/14 border=primary/26 border-w=1.0 r=6.0
                      text "Members only" size=11.0 wrapping=none font=medium @text-primary
                  if active_channel_huddle_count > 0
                    container padding=2.0 padding-left=7.0 padding-right=7.0 bg=success/16 border=success/26 border-w=1.0 r=6.0
                      text active_channel_huddle_count size=11.0 wrapping=none font=medium @text-success
                  space width=fill
                  input "" #chat-search label="Search messages" <-> chat_search_draft hint="Search messages" disabled=(!connected || chat_searching) submit=search_chat_submit width=190.0 padding=6.2 text-size=13.0 line-height=1.2
                    active bg=fg/4 border=fg/11 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                    hovered bg=fg/6 border=fg/15
                    focused bg=fg/8 border=primary/55
                    disabled bg=transparent value=muted
                  if !empty(chat_search_hits)
                    button label="Clear message search" width=28.0 height=28.0 padding=0.0 -> clear_chat_search
                      container width=fill height=fill align-x=center align-y=center
                        text "×" size=14.0
                      active bg=transparent text=muted r=8.0
                      hovered bg=fg/10 text=fg
                      pressed bg=fg/15
                  button label="Channel details" width=28.0 height=28.0 padding=0.0 -> toggle_channel_settings
                    container width=fill height=fill align-x=center align-y=center
                      text "•••" size=13.0
                    active bg=transparent text=muted r=8.0
                    hovered bg=fg/10 text=fg
                    pressed bg=fg/15
                container width=fill height=1.0 bg=separator
                  text ""
            if !empty(chat_search_hits)
              container width=fill height=148.0 padding=6.0 bg=elevated border=fg/10 border-w=1.0 r=10.0
                scroll direction=vertical width=fill height=fill
                  col width=fill spacing=1.0
                    for hit in chat_search_hits
                      ChatSearchResult hit=hit
            if !connected
              EmptyState title="Connect to a node" detail="Set the RPC endpoint in the sidebar."
            if connected && empty(messages)
              EmptyState title="No messages yet" detail="Create a channel or start the conversation."
            if connected && !empty(messages) && history_view
              container width=fill height=32.0 padding-left=10.0 padding-right=6.0 bg=primary/12 border=primary/26 border-w=1.0 r=9.0
                row width=fill height=fill spacing=8.0 align=center
                  text "Viewing history" width=fill size=11.0 wrapping=none font=medium @text-primary
                  button "Jump to latest" height=24.0 padding=5.0 -> choose_channel(active_channel)
                    active bg=primary/16 text=fg border=primary/30 border-w=1.0 r=7.0
                    hovered bg=primary/24 text=fg
                    pressed bg=primary/30 text=fg
            if connected && !empty(messages)
              stack width=fill height=fill
                mouse move=chat_pointer_moved
                  sensor show=chat_resized resize=chat_resized
                    scroll direction=vertical width=fill height=fill
                      col width=fill spacing=1.0
                        if history_has_older(messages)
                          container width=fill align-x=center padding-top=4.0 padding-bottom=8.0
                            button "Load older messages" disabled=(history_loading || mutation_phase != "idle") height=30.0 padding=6.0 -> load_more_history
                              active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=8.0
                              hovered bg=fg/10 text=fg border=fg/14
                              pressed bg=fg/14 text=fg
                        for message in messages
                          col width=fill spacing=0.0
                            if unread_boundary > 0 && message.seq == first_unread_seq(messages, unread_boundary)
                              row width=fill spacing=8.0 align=center padding-top=8.0 padding-bottom=2.0
                                container width=fill height=1.0 bg=primary/40
                                  text ""
                                text "New messages" size=11.0 wrapping=none font=medium @text-primaryhi
                                container width=fill height=1.0 bg=primary/40
                                  text ""
                            if message.show_author
                              space height=10.0
                            stack #message(message.id) width=fill
                              MessageCard message=message selected=(message.seq == selected_message_seq) hovered=(message.seq == hovered_message_seq) disabled=loading
                overlay when=(selected_message_seq > 0 && message_action != "toolbar") dismiss=clear_message_selection backdrop=transparent padding=8.0 align-x=end align-y=start
                  content
                    space width=fill height=fill
                  layer
                    float x=0.0 y=message_menu_y
                      col
                        if message_action == "more"
                          stack
                            input "" #message-action-focus label="Message action focus" <-> message_action_focus width=1.0 padding=0.0 text-size=1.0 line-height=1.0
                              active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                              focused bg=transparent border=transparent value=transparent border-w=0.0
                            container width=190.0 padding=4.0 bg=popover border=fg/16 border-w=1.0 r=8.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
                              col width=fill spacing=1.0
                                button "React" label="Manage reactions" disabled=active_channel_archived width=fill height=28.0 padding=6.0 -> open_message_reactions(selected_message_seq, message_edit_draft, selected_message_rev)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                                button "Open thread" width=fill height=28.0 padding=6.0 -> open_thread_for(selected_message_seq)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                                button "Edit" width=fill height=28.0 padding=6.0 -> begin_message_edit(selected_message_seq, message_edit_draft, selected_message_rev)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                                button "Delete" width=fill height=28.0 padding=6.0 -> arm_message_delete(selected_message_seq, message_edit_draft, selected_message_rev)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                                button "Close" label="Close message actions" width=fill height=28.0 padding=6.0 -> clear_message_selection
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                        if message_action == "reactions"
                          stack
                            input "" #message-reaction-focus label="Message reaction focus" <-> message_action_focus width=1.0 padding=0.0 text-size=1.0 line-height=1.0
                              active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                              focused bg=transparent border=transparent value=transparent border-w=0.0
                            container padding=3.0 bg=popover border=fg/16 border-w=1.0 r=8.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
                              row spacing=2.0 align=center
                                button "+ 👍" label="Add thumbs up reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_submit("👍")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ ♥" label="Add heart reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_submit("❤️")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ 😄" label="Add smile reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_submit("😄")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ 🎉" label="Add celebration reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_submit("🎉")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ 👀" label="Add eyes reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_submit("👀")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ 🙌" label="Add raised hands reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_submit("🙌")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                for message in messages
                                  if message.seq == selected_message_seq
                                    for reaction in message.reactions
                                      if reaction.reacted_by_me
                                        button label="Remove my reaction" description=reaction.emoji disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> remove_reaction_submit(reaction.emoji)
                                          text reaction.emoji size=11.0 @text-fg
                                          active bg=fg/7 text=fg r=6.0
                                          hovered bg=fg/12
                                          pressed bg=fg/17
                                button "×" label="Close reactions" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=4.0 -> clear_message_selection
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                        if message_action == "editing"
                          container width=fill max-width=520.0 padding=3.0 bg=popover border=fg/16 border-w=1.0 r=8.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
                            row width=fill spacing=4.0 align=center
                              input "" #message-edit label="Edit message" <-> message_edit_draft hint="Edit message" disabled=(mutation_phase != "idle") submit=edit_message_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
                                active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                hovered bg=fg/4 border=fg/8
                                focused bg=fg/7 border=fg/12
                                disabled value=muted
                              button "Save" label="Save message changes" disabled=(mutation_phase != "idle" || empty(trim(message_edit_draft))) height=28.0 padding=6.0 -> edit_message_submit
                                active bg=fg/11 text=fg border=fg/13 border-w=1.0 r=7.0
                                hovered bg=fg/16
                                pressed bg=fg/20
                              button label="Cancel message edit" disabled=(mutation_phase != "idle") width=28.0 height=28.0 padding=0.0 -> clear_message_selection
                                container width=fill height=fill align-x=center align-y=center
                                  text "×" size=14.0
                                active bg=transparent text=muted r=7.0
                                hovered bg=fg/10 text=fg
                                pressed bg=fg/15
                        if message_action == "delete"
                          stack
                            input "" #message-delete-focus label="Message delete focus" <-> message_action_focus width=1.0 padding=0.0 text-size=1.0 line-height=1.0
                              active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                              focused bg=transparent border=transparent value=transparent border-w=0.0
                            container padding=3.0 bg=popover border=fg/16 border-w=1.0 r=8.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
                              row spacing=5.0 align=center
                                text "Delete this message?" size=11.0 @text-muted
                                button "Delete" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> delete_message_submit
                                  active bg=fg/12 text=fg r=6.0
                                  hovered bg=fg/17
                                  pressed bg=fg/22
                                button "Cancel" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> clear_message_selection
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
            if !empty(failed_message_draft)
              row width=fill spacing=6.0 align=center
                text "An earlier message wasn’t sent" width=fill size=13.0 @text-muted
                button "Restore" disabled=(!empty(trim(editor_text(message_editor))) || mutation_phase != "idle") height=28.0 padding=5.0 -> restore_failed_message
                  active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
                  hovered bg=fg/14
                  pressed bg=fg/18
                button label="Dismiss unsent message" width=28.0 height=28.0 padding=0.0 -> dismiss_failed_message
                  container width=fill height=fill align-x=center align-y=center
                    text "×" size=14.0
                  active bg=transparent text=muted r=7.0
                  hovered bg=fg/10 text=fg
                  pressed bg=fg/15
            container width=fill padding=8.0 bg=surface border=fg/13 border-w=1.0 r=13.0 shadow=black/24 shadow-y=3.0 shadow-blur=18.0
              flex width=fill gap=8.0 align-items=end
                editor #message <-> message_editor placeholder="Message the channel…" disabled=(loading || !connected || empty(active_channel) || active_channel_archived) min-height=44.0 max-height=150.0 size=14.0 line-height=1.3 padding=6.6 wrapping=word key-binding=composer_keys() -> send_message_submit
                  active bg=transparent border=transparent value=fg placeholder=muted selection=primary/40 border-w=0.0 r=9.0
                  hovered bg=fg/4 border=fg/8 border-w=1.0
                  focused bg=fg/6 border=primary/45 border-w=1.0
                  disabled value=muted
                button "Send" disabled=(loading || !connected || empty(active_channel) || active_channel_archived || empty(trim(editor_text(message_editor)))) width=66.0 height=30.0 padding=7.0 -> send_message_submit
                  active bg=primary text=fg border=primaryhi/50 border-w=1.0 r=10.0 shadow=black/26 shadow-y=2.0 shadow-blur=9.0
                  hovered bg=primaryhi text=fg
                  pressed bg=primary text=fg
                  disabled bg=fg/8 text=muted
          if channel_settings_open && !empty(active_channel)
            container width=1.0 height=fill bg=fg/8
              text ""
            container width=300.0 height=fill padding=12.0 bg=surface
              col width=fill height=fill spacing=8.0
                row width=fill height=28.0 spacing=6.0 align=center
                  text "Channel details" width=fill size=13.0 font=medium @text-fg
                  button label="Close channel details" width=28.0 height=28.0 padding=0.0 -> toggle_channel_settings
                    container width=fill height=fill align-x=center align-y=center
                      text "×" size=14.0
                    active bg=transparent text=muted r=7.0
                    hovered bg=fg/10 text=fg
                    pressed bg=fg/15
                container width=fill height=1.0 bg=separator
                  text ""
                row width=fill spacing=5.0 align=center
                  input "" #channel-name label="Channel name" <-> channel_name_draft hint="Channel name" disabled=(mutation_phase != "idle") submit=rename_channel_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
                    active bg=transparent border=fg/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                    hovered bg=fg/4 border=fg/14
                    focused bg=fg/7 border=fg/12
                    disabled value=muted
                  button "Rename" disabled=(mutation_phase != "idle" || empty(trim(channel_name_draft))) width=56.0 height=28.0 padding=5.0 -> rename_channel_submit
                    active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
                    hovered bg=fg/14
                    pressed bg=fg/18
                row width=fill spacing=5.0 align=center
                  if !active_channel_archived
                    button "Archive" disabled=(mutation_phase != "idle") height=28.0 padding=5.0 -> archive_channel_submit
                      active bg=transparent text=muted r=7.0
                      hovered bg=fg/10 text=fg
                      pressed bg=fg/15
                    button "Join huddle" disabled=(mutation_phase != "idle") height=28.0 padding=5.0 -> join_huddle_submit
                      active bg=transparent text=muted r=7.0
                      hovered bg=fg/10 text=fg
                      pressed bg=fg/15
                  if active_channel_archived
                    button "Unarchive" disabled=(mutation_phase != "idle") height=28.0 padding=5.0 -> unarchive_channel_submit
                      active bg=transparent text=muted r=7.0
                      hovered bg=fg/10 text=fg
                      pressed bg=fg/15
                  if active_channel_huddle_count > 0
                    button "Leave huddle" disabled=(mutation_phase != "idle") height=28.0 padding=5.0 -> leave_huddle_submit
                      active bg=transparent text=muted r=7.0
                      hovered bg=fg/10 text=fg
                      pressed bg=fg/15
                  space width=fill
                  text len(channel_members) size=11.0 @text-muted
                row width=fill spacing=5.0 align=center
                  input "" #member-key label="Member public key" <-> member_key_draft hint="64-character member key" disabled=(mutation_phase != "idle") submit=add_channel_member_submit width=fill padding=7.4 text-size=11.0 line-height=1.2 font=mono
                    active bg=transparent border=fg/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                    hovered bg=fg/4 border=fg/14
                    focused bg=fg/7 border=fg/12
                    disabled value=muted
                  button "Add" disabled=(mutation_phase != "idle" || empty(trim(member_key_draft))) width=40.0 height=28.0 padding=5.0 -> add_channel_member_submit
                    active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
                    hovered bg=fg/14
                    pressed bg=fg/18
                if !empty(channel_members)
                  scroll direction=vertical width=fill height=fill
                    col width=fill spacing=2.0
                      for member in channel_members
                        ChatMemberRow member=member disabled=(mutation_phase != "idle")
          if active_thread_seq > 0 && !channel_settings_open
            container width=1.0 height=fill bg=fg/8
              text ""
            container width=300.0 height=fill padding=12.0 bg=surface
              stack width=fill height=fill
                mouse move=thread_pointer_moved
                  sensor show=thread_resized resize=thread_resized
                    col width=fill height=fill spacing=8.0
                      row width=fill height=28.0 spacing=6.0 align=center
                        if thread_target_seq <= 0
                          text "Thread" width=fill size=13.0 font=medium @text-fg
                        if thread_target_seq > 0
                          text "Thread result" width=fill size=13.0 font=medium @text-fg
                        text len(thread_messages) size=11.0 @text-muted
                        button label="Close thread" disabled=(mutation_phase != "idle") width=28.0 height=28.0 padding=0.0 -> close_thread
                          container width=fill height=fill align-x=center align-y=center
                            text "×" size=14.0
                          active bg=transparent text=muted r=7.0
                          hovered bg=fg/11 text=fg
                          pressed bg=selection
                      container width=fill height=1.0 bg=separator
                        text ""
                      scroll direction=vertical width=fill height=fill
                        col width=fill spacing=1.0
                          for thread_message in thread_messages
                            ThreadMessageCard message=thread_message selected=(thread_message.seq == thread_target_seq) hovered=(thread_message.seq == thread_hovered_seq) disabled=loading
                          if thread_has_more && thread_next_reply_offset >= 0
                            button "Load more replies" disabled=(thread_loading || mutation_phase != "idle") width=fill height=28.0 padding=5.0 -> load_more_thread
                              active bg=transparent text=muted r=7.0
                              hovered bg=fg/9 text=fg
                              pressed bg=selection
                      if !empty(failed_reply_draft)
                        row width=fill spacing=6.0 align=center
                          text "Unsent reply" width=fill size=11.0 @text-muted
                          button "Restore" disabled=(!empty(trim(editor_text(reply_editor)))) height=26.0 padding=5.0 -> restore_failed_reply
                            active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
                            hovered bg=fg/14
                            pressed bg=fg/18
                          button "×" label="Dismiss unsent reply" width=26.0 height=26.0 padding=4.0 -> dismiss_failed_reply
                            active bg=transparent text=muted r=7.0
                            hovered bg=fg/10 text=fg
                            pressed bg=fg/15
                      container width=fill padding=5.0 bg=transparent border=fg/12 border-w=1.0 r=7.0
                        row width=fill spacing=5.0 align=end
                          editor #reply <-> reply_editor placeholder="Reply…" disabled=(thread_loading || active_channel_archived) min-height=44.0 max-height=150.0 size=14.0 line-height=1.3 padding=6.6 wrapping=word key-binding=composer_keys() -> send_reply_submit
                            active bg=transparent border=transparent value=fg placeholder=muted selection=primary/40 border-w=0.0 r=9.0
                            hovered bg=fg/4 border=fg/8 border-w=1.0
                            focused bg=fg/6 border=primary/45 border-w=1.0
                            disabled value=muted
                          button "Send" label="Send reply" disabled=(thread_loading || active_channel_archived || empty(trim(editor_text(reply_editor)))) height=28.0 padding=6.0 -> send_reply_submit
                            active bg=fg/88 text=bg border=fg/5 border-w=1.0 r=9.0
                            hovered bg=fg/78
                            pressed bg=fg
                            disabled bg=fg/24 text=bg/12
                overlay when=(thread_selected_seq > 0 && thread_message_action != "toolbar") dismiss=clear_thread_message_selection backdrop=transparent padding=8.0 align-x=end align-y=start
                  content
                    space width=fill height=fill
                  layer
                    float x=0.0 y=thread_menu_y
                      col
                        if thread_message_action == "more"
                          stack
                            input "" #thread-action-focus label="Thread action focus" <-> message_action_focus width=1.0 padding=0.0 text-size=1.0 line-height=1.0
                              active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                              focused bg=transparent border=transparent value=transparent border-w=0.0
                            container width=190.0 padding=4.0 bg=popover border=fg/16 border-w=1.0 r=8.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
                              col width=fill spacing=1.0
                                button "React" label="Manage reactions" disabled=active_channel_archived width=fill height=28.0 padding=6.0 -> open_thread_message_reactions(thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                                button "Edit" width=fill height=28.0 padding=6.0 -> begin_thread_message_edit(thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                                button "Delete" width=fill height=28.0 padding=6.0 -> arm_thread_message_delete(thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                                button "Close" label="Close message actions" width=fill height=28.0 padding=6.0 -> clear_thread_message_selection
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                        if thread_message_action == "reactions"
                          stack
                            input "" #thread-reaction-focus label="Thread reaction focus" <-> message_action_focus width=1.0 padding=0.0 text-size=1.0 line-height=1.0
                              active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                              focused bg=transparent border=transparent value=transparent border-w=0.0
                            container padding=3.0 bg=popover border=fg/16 border-w=1.0 r=8.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
                              row spacing=2.0 align=center
                                button "+ 👍" label="Add thumbs up reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_at(thread_selected_seq, "👍")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ ♥" label="Add heart reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_at(thread_selected_seq, "❤️")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ 😄" label="Add smile reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_at(thread_selected_seq, "😄")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ 🎉" label="Add celebration reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_at(thread_selected_seq, "🎉")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ 👀" label="Add eyes reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_at(thread_selected_seq, "👀")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ 🙌" label="Add raised hands reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_at(thread_selected_seq, "🙌")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                for thread_message in thread_messages
                                  if thread_message.seq == thread_selected_seq
                                    for reaction in thread_message.reactions
                                      if reaction.reacted_by_me
                                        button label="Remove my reaction" description=reaction.emoji disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> remove_reaction_at(thread_selected_seq, reaction.emoji)
                                          text reaction.emoji size=11.0 @text-fg
                                          active bg=fg/7 text=fg r=6.0
                                          hovered bg=fg/12
                                          pressed bg=fg/17
                                button "×" label="Close reactions" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=4.0 -> clear_thread_message_selection
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                        if thread_message_action == "editing"
                          container width=fill max-width=520.0 padding=3.0 bg=popover border=fg/16 border-w=1.0 r=8.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
                            row width=fill spacing=4.0 align=center
                              input "" #thread-edit label="Edit message" <-> thread_edit_draft hint="Edit message" disabled=(mutation_phase != "idle") submit=edit_thread_message_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
                                active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                hovered bg=fg/4 border=fg/8
                                focused bg=fg/7 border=fg/12
                                disabled value=muted
                              button "Save" label="Save message changes" disabled=(mutation_phase != "idle" || empty(trim(thread_edit_draft))) height=28.0 padding=6.0 -> edit_thread_message_submit
                                active bg=fg/11 text=fg border=fg/13 border-w=1.0 r=7.0
                                hovered bg=fg/16
                                pressed bg=fg/20
                              button label="Cancel message edit" disabled=(mutation_phase != "idle") width=28.0 height=28.0 padding=0.0 -> clear_thread_message_selection
                                container width=fill height=fill align-x=center align-y=center
                                  text "×" size=14.0
                                active bg=transparent text=muted r=7.0
                                hovered bg=fg/10 text=fg
                                pressed bg=fg/15
                        if thread_message_action == "delete"
                          stack
                            input "" #thread-delete-focus label="Thread delete focus" <-> message_action_focus width=1.0 padding=0.0 text-size=1.0 line-height=1.0
                              active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                              focused bg=transparent border=transparent value=transparent border-w=0.0
                            container padding=3.0 bg=popover border=fg/16 border-w=1.0 r=8.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
                              row spacing=5.0 align=center
                                text "Delete this message?" size=11.0 @text-muted
                                button "Delete" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> delete_thread_message_submit
                                  active bg=fg/12 text=fg r=6.0
                                  hovered bg=fg/17
                                  pressed bg=fg/22
                                button "Cancel" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> clear_thread_message_selection
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
    pages:
      mouse move=pages_pointer_moved
        col width=fill height=fill
          if connected && !empty(doc_tab_rows(doc_tabs, pages, active_page))
            container width=fill height=34.0 padding-left=8.0 padding-right=8.0 bg=sidebar border=separator border-w=1.0
              scroll direction=horizontal width=fill height=fill bar=hidden
                row height=fill spacing=2.0 align=center
                  for tab in doc_tab_rows(doc_tabs, pages, active_page)
                    row spacing=0.0 align=center
                      button label="Open page tab" height=26.0 padding=5.0 -> choose_page(tab.id)
                        row height=fill spacing=5.0 align=center
                          if tab.active
                            text tab.title size=13.0 wrapping=none font=medium @text-fg
                          if !tab.active
                            text tab.title size=13.0 wrapping=none @text-muted
                        active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                        hovered bg=fg/5 text=fg
                        pressed bg=fg/8
                      button "×" label="Close page tab" width=20.0 height=20.0 padding=0.0 -> close_doc_tab(tab.id)
                        active bg=transparent text=muted r=6.0
                        hovered bg=fg/8 text=fg
                        pressed bg=fg/12
          stack width=fill height=fill clip=true
            sensor show=pages_resized resize=pages_resized
              space width=fill height=fill
            if !connected
              EmptyState title="Connect to a node" detail="Set the RPC endpoint in the sidebar."
            if connected && empty(active_page)
              EmptyState title="No page selected" detail="Create a page from the sidebar."
            if connected && !empty(active_page)
              scroll direction=vertical width=fill height=fill bar=hidden
                container width=fill max-width=800.0 margin-x=auto padding-left=46.0 padding-right=46.0 padding-top=24.0 padding-bottom=80.0
                  col width=fill spacing=8.0
                    row width=fill height=28.0 spacing=5.0 align=center
                      if !empty(active_page_parent)
                        text active_page_parent width=fill size=11.0 wrapping=none @text-muted
                      if empty(active_page_parent)
                        space width=fill
                      input "" #page-search label="Search pages" <-> page_search_draft hint="Search pages…" disabled=(!connected || page_searching) submit=search_pages_submit width=190.0 padding=6.2 text-size=13.0 line-height=1.2
                        active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                        hovered bg=fg/5 border=fg/8
                        focused bg=fg/7 border=fg/12
                        disabled value=muted
                      if !empty(page_search_hits)
                        button label="Clear page search" width=28.0 height=28.0 padding=0.0 -> clear_page_search
                          container width=fill height=fill align-x=center align-y=center
                            text "×" size=14.0
                          active bg=transparent text=muted r=7.0
                          hovered bg=fg/10 text=fg
                          pressed bg=fg/15
                      if !page_delete_armed
                        button label="Page menu" disabled=(mutation_phase != "idle") width=28.0 height=28.0 padding=0.0 -> arm_page_delete
                          container width=fill height=fill align-x=center align-y=center
                            text "•••" size=13.0
                          active bg=transparent text=muted r=7.0
                          hovered bg=fg/10 text=fg
                          pressed bg=fg/15
                      if page_delete_armed
                        button "Delete page" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> delete_page_submit
                          active bg=fg/11 text=fg border=fg/13 border-w=1.0 r=7.0
                          hovered bg=fg/16
                          pressed bg=fg/20
                    container width=fill padding-left=56.0
                      PageTitleEditor rpc=connected_rpc password=password page_id=active_page title=active_page_title disabled=(loading || !connected || mutation_phase != "idle") #page-title(scope_key(connected_rpc, active_page))
                    if !empty(page_search_hits)
                      container width=fill height=148.0 padding=5.0 bg=elevated border=fg/8 border-w=1.0 r=9.0
                        scroll direction=vertical width=fill height=fill
                          col width=fill spacing=1.0
                            for hit in page_search_hits
                              PageSearchResult hit=hit
                    if !empty(orphaned_block_drafts) || !empty(orphaned_comment_drafts)
                      container width=fill padding=7.0 bg=elevated border=fg/9 border-w=1.0 r=9.0
                        col width=fill spacing=5.0
                          text "Recovered drafts" size=11.0 font=medium @text-fg
                          for recovered_block in orphaned_block_drafts
                            row width=fill spacing=5.0 align=center
                              text recovered_block width=fill size=13.0 @text-muted
                              button "Use" label="Use as block" disabled=(loading || mutation_phase != "idle" || !empty(block_draft)) height=26.0 padding=5.0 -> use_orphaned_block_draft(recovered_block)
                                active bg=fg/9 text=fg border=fg/12 border-w=1.0 r=7.0
                                hovered bg=fg/14
                                pressed bg=fg/18
                              button "Discard" disabled=(loading || mutation_phase != "idle") height=26.0 padding=5.0 -> discard_orphaned_block_draft(recovered_block)
                                active bg=transparent text=muted r=7.0
                                hovered bg=fg/9 text=fg
                                pressed bg=fg/14
                          for recovered_comment in orphaned_comment_drafts
                            row width=fill spacing=5.0 align=center
                              text recovered_comment width=fill size=13.0 @text-muted
                              button "Use" label="Use as block" disabled=(loading || mutation_phase != "idle" || !empty(block_draft)) height=26.0 padding=5.0 -> use_orphaned_comment_draft(recovered_comment)
                                active bg=fg/9 text=fg border=fg/12 border-w=1.0 r=7.0
                                hovered bg=fg/14
                                pressed bg=fg/18
                              button "Discard" disabled=(loading || mutation_phase != "idle") height=26.0 padding=5.0 -> discard_orphaned_comment_draft(recovered_comment)
                                active bg=transparent text=muted r=7.0
                                hovered bg=fg/9 text=fg
                                pressed bg=fg/14
                    if empty(blocks) && !block_insert_open
                      container width=fill padding-left=56.0
                        button "Write something…" label="Start writing" disabled=loading width=fill padding=6.0 -> open_root_block_insert
                          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                          hovered bg=fg/4 text=fg border=fg/7
                          pressed bg=fg/8
                    if block_insert_open && empty(block_insert_after_id)
                      InlineBlockInsert kind=new_block_kind kinds=block_kinds disabled=loading prefix="" #block-insert-row(block_insert_after_id)
                        stack width=fill
                          if new_block_kind != "Divider"
                            col width=fill spacing=2.0
                              input "" #block-insert label="New block" <-> block_draft hint="Type, or / for a block kind…" disabled=loading submit=add_block_submit width=fill padding=5.0 text-size=14.0 line-height=1.3
                                active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=6.0
                                hovered bg=fg/2 border=fg/5
                                focused bg=fg/4 border=fg/8
                                disabled value=muted
                              if !empty(slash_kind_matches(block_draft, editable_block_kinds))
                                container width=fill padding=3.0 bg=popover border=fg/12 border-w=1.0 r=8.0 shadow=shadow shadow-y=2.0 shadow-blur=8.0
                                  col width=fill spacing=1.0
                                    for kind in slash_kind_matches(block_draft, editable_block_kinds)
                                      button label="Set block kind" width=fill height=24.0 padding=4.0 -> pick_slash_kind(kind)
                                        row width=fill height=fill spacing=6.0 align=center
                                          text kind width=fill size=13.0 wrapping=none @text-fg
                                        active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                        hovered bg=primary/14 text=fg
                                        pressed bg=primary/20
                          if new_block_kind == "Divider"
                            button "Insert divider" disabled=loading width=fill height=28.0 padding=5.0 -> add_block_submit
                              active bg=transparent text=muted r=6.0
                              hovered bg=fg/8 text=fg
                              pressed bg=fg/12
                    keyed block in blocks by=block.key
                      col width=fill spacing=1.0
                        DocumentBlock block=block selected=(block.id == selected_block_id) hovered=(block.id == hovered_block_id) disabled=loading #block(block.id)
                          col width=fill
                            if block.pending
                              container width=fill padding=5.0 bg=fg/3 r=6.0
                                BlockContents block=block
                            if !block.pending && block.kind == "Page"
                              button label=block.kind description=block.text width=fill padding=5.0 -> choose_page(block.id)
                                BlockContents block=block
                                active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                hovered bg=fg/3 text=fg border=transparent
                                pressed bg=fg/6 text=fg
                            if !block.pending && block.kind != "Page" && block.id != selected_block_id
                              button label=block.kind description=block.text width=fill padding=5.0 -> select_block(block.key, block.id, block.kind, block.text, block.checked, false)
                                BlockContents block=block
                                active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                hovered bg=fg/3 text=fg border=transparent
                                pressed bg=fg/6 text=fg
                            if !block.pending && block.kind != "Page" && block.id == selected_block_id
                              BlockLine block=block
                                col width=fill
                                  if block.kind == "Divider"
                                    container width=fill height=1.0 bg=separator
                                      text ""
                                  if block.kind != "Divider"
                                    input "" #block-edit label="Edit block" <-> block_edit_draft change=block_text_changed hint="Type something…" disabled=(mutation_phase != "idle") width=fill padding=4.0 text-size=14.0 line-height=1.3
                                      active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=5.0
                                      hovered bg=fg/2 border=fg/5
                                      focused bg=fg/3 border=fg/7
                                      disabled value=muted
                        if block_insert_open && block.id == block_insert_after_id
                          InlineBlockInsert kind=new_block_kind kinds=block_kinds disabled=loading prefix=block.prefix #block-insert-row(block_insert_after_id)
                            stack width=fill
                              if new_block_kind != "Divider"
                                col width=fill spacing=2.0
                                  input "" #block-insert label="New block" <-> block_draft hint="Type, or / for a block kind…" disabled=loading submit=add_block_submit width=fill padding=5.0 text-size=14.0 line-height=1.3
                                    active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=6.0
                                    hovered bg=fg/2 border=fg/5
                                    focused bg=fg/4 border=fg/8
                                    disabled value=muted
                                  if !empty(slash_kind_matches(block_draft, editable_block_kinds))
                                    container width=fill padding=3.0 bg=popover border=fg/12 border-w=1.0 r=8.0 shadow=shadow shadow-y=2.0 shadow-blur=8.0
                                      col width=fill spacing=1.0
                                        for kind in slash_kind_matches(block_draft, editable_block_kinds)
                                          button label="Set block kind" width=fill height=24.0 padding=4.0 -> pick_slash_kind(kind)
                                            row width=fill height=fill spacing=6.0 align=center
                                              text kind width=fill size=13.0 wrapping=none @text-fg
                                            active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                            hovered bg=primary/14 text=fg
                                            pressed bg=primary/20
                              if new_block_kind == "Divider"
                                button "Insert divider" disabled=loading width=fill height=28.0 padding=5.0 -> add_block_submit
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/8 text=fg
                                  pressed bg=fg/12
            overlay when=(connected && !empty(active_page) && block_comments_open) dismiss=close_block_comments backdrop=transparent padding=12.0 align-x=end align-y=start
              content
                space width=fill height=fill
              layer
                container width=300.0 height=380.0 padding=8.0 bg=popover border=fg/15 border-w=1.0 r=11.0 shadow=black/24 shadow-y=4.0 shadow-blur=16.0
                  col width=fill height=fill spacing=6.0
                    row width=fill spacing=6.0 align=center
                      text "Comments" width=fill size=13.0 font=medium @text-fg
                      if block_comment_thread_total > 0
                        text block_comment_thread_total size=11.0 @text-muted
                      if block_comment_threads_loading || block_thread_comments_loading
                        text "Loading…" size=11.0 @text-muted
                      button "×" label="Close comments" disabled=(mutation_phase != "idle") width=24.0 height=24.0 padding=4.0 -> close_block_comments
                        active bg=transparent text=muted r=6.0
                        hovered bg=fg/10 text=fg
                        pressed bg=fg/15
                    if empty(active_block_comment_thread)
                      scroll direction=vertical width=fill height=fill
                        col width=fill spacing=1.0
                          if empty(block_comment_threads) && !block_comment_threads_loading
                            text "No comments yet" width=fill size=11.0 align-x=center @text-muted
                          for comment_thread in block_comment_threads
                            PageCommentThreadButton thread=comment_thread
                          if block_comment_threads_has_more
                            button "More" disabled=(block_comment_threads_loading || mutation_phase != "idle") height=24.0 padding=4.0 -> load_more_block_threads
                              active bg=transparent text=muted r=6.0
                              hovered bg=fg/9 text=fg
                              pressed bg=fg/14
                    if !empty(active_block_comment_thread)
                      row width=fill spacing=5.0 align=center
                        button "← Threads" disabled=(block_thread_comments_loading || mutation_phase != "idle") height=24.0 padding=4.0 -> close_block_comment_thread
                          active bg=transparent text=muted r=6.0
                          hovered bg=fg/9 text=fg
                          pressed bg=fg/14
                      scroll direction=vertical width=fill height=fill
                        col width=fill spacing=1.0
                          for page_comment in block_thread_comments
                            PageCommentCard comment=page_comment
                          if block_thread_comments_has_more
                            button "More" disabled=(block_thread_comments_loading || mutation_phase != "idle") height=24.0 padding=4.0 -> load_more_block_comments
                              active bg=transparent text=muted r=6.0
                              hovered bg=fg/9 text=fg
                              pressed bg=fg/14
                    row width=fill spacing=5.0 align=center
                      input "" #block-comment(scope_key(connected_rpc, selected_block_id)) label="New block comment" <-> block_comment_draft hint="Add a comment…" disabled=(mutation_phase != "idle" || block_comment_threads_loading || block_thread_comments_loading) submit=post_block_comment_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
                        active bg=transparent border=fg/8 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                        hovered bg=fg/4 border=fg/11
                        focused bg=fg/6 border=fg/13
                        disabled value=muted
                      button "Post" disabled=(mutation_phase != "idle" || empty(trim(block_comment_draft)) || block_comment_threads_loading || block_thread_comments_loading) height=28.0 padding=5.0 -> post_block_comment_submit
                        active bg=fg/88 text=bg border=fg/5 border-w=1.0 r=8.0
                        hovered bg=fg/78 text=bg
                        pressed bg=fg text=bg
                        disabled bg=fg/25 text=bg/12
            overlay when=(connected && !empty(active_page) && !empty(selected_block_id) && block_actions_open) dismiss=close_block_actions backdrop=transparent padding=0.0 align-x=start align-y=start
              content
                space width=fill height=fill
              layer
                float x=(block_menu_x + 10.0) y=block_menu_y
                  BlockActionsMenu block_id=selected_block_id kind=selected_block_kind disabled=(loading || mutation_phase != "idle") delete_armed=block_delete_armed editable_kinds=editable_block_kinds
    files:
      col width=fill height=fill padding=14.0 spacing=8.0
        row width=fill height=28.0 spacing=8.0 align=center
          button "↑" label="Parent directory" disabled=(fs_loading || empty(fs_path)) width=26.0 height=26.0 padding=0.0 -> fs_open_parent
            active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=7.0
            hovered bg=fg/10 text=fg
            pressed bg=fg/14
          text fs_path width=fill size=13.0 wrapping=none font=mono @text-fg
          if fs_loading
            text "Loading…" size=11.0 font=mono @text-muted
          input "" #fs-new label="New entry name" <-> fs_new_name change=fs_new_name_changed hint="new name…" disabled=fs_loading width=140.0 padding=5.0 text-size=13.0 line-height=1.2
            active bg=surface border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
            hovered bg=elevated border=fg/21
            focused bg=elevated border=fg/45 border-w=1.0
            disabled bg=surface/54 value=muted
          button "+ Folder" disabled=(fs_loading || empty(trim(fs_new_name))) height=26.0 padding=5.0 -> fs_mkdir_submit
            active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=7.0
            hovered bg=fg/10 text=fg
            pressed bg=fg/14
            disabled bg=fg/3 text=muted
          button "+ File" disabled=(fs_loading || empty(trim(fs_new_name))) height=26.0 padding=5.0 -> fs_new_file_submit
            active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=7.0
            hovered bg=fg/10 text=fg
            pressed bg=fg/14
            disabled bg=fg/3 text=muted
          button "History" height=26.0 padding=5.0 -> fs_toggle_history
            active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=7.0
            hovered bg=fg/10 text=fg
            pressed bg=fg/14
        row width=fill height=fill spacing=10.0
          container width=340.0 height=fill padding=6.0 bg=surface border=fg/10 border-w=1.0 r=10.0
            stack width=fill height=fill
              if empty(fs_entries) && !fs_loading
                EmptyState title="Empty directory" detail="Nothing committed under this path."
              if !empty(fs_entries)
                scroll direction=vertical width=fill height=fill
                  col width=fill spacing=1.0
                    for entry in fs_entries
                      col width=fill
                        if entry.kind == "dir"
                          button label="Open directory" width=fill padding=6.0 -> fs_open_dir(entry.path)
                            row width=fill height=fill spacing=8.0 align=center
                              text "▸" width=14.0 size=11.0 align-x=center @text-muted
                              text entry.name width=fill size=13.0 wrapping=none font=medium @text-fg
                            active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
                            hovered bg=primary/10 text=fg
                            pressed bg=primary/16
                        if entry.kind != "dir"
                          row width=fill spacing=2.0 align=center
                            button label="Preview file" width=fill padding=6.0 -> fs_open_file(entry.path)
                              row width=fill height=fill spacing=8.0 align=center
                                text "·" width=14.0 size=11.0 align-x=center @text-muted
                                text entry.name width=fill size=13.0 wrapping=none @text-fg
                                text entry.size size=11.0 wrapping=none font=mono @text-muted
                              active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
                              hovered bg=primary/10 text=fg
                              pressed bg=primary/16
                            if entry.path == fs_delete_target
                              button "rm!" label="Confirm delete" disabled=fs_loading width=34.0 height=22.0 padding=0.0 -> fs_delete_submit
                                active bg=danger/16 text=fg border=danger/40 border-w=1.0 r=6.0
                                hovered bg=danger/26 text=fg
                                pressed bg=danger/34
                            if entry.path != fs_delete_target
                              button "×" label="Delete file" width=22.0 height=22.0 padding=0.0 -> fs_arm_delete(entry.path)
                                active bg=transparent text=muted r=6.0
                                hovered bg=danger/14 text=fg
                                pressed bg=danger/22
          container width=fill height=fill padding=8.0 bg=surface border=fg/10 border-w=1.0 r=10.0
            stack width=fill height=fill
              if fs_history_open
                scroll direction=vertical width=fill height=fill
                  col width=fill spacing=4.0
                    container width=fill padding-left=4.0
                      text "SNAPSHOTS" size=11.0 font=medium @text-muted
                    if !empty(fs_diff_from)
                      col width=fill spacing=3.0
                        row width=fill spacing=8.0 align=center
                          text "Changes vs head" width=fill size=11.0 font=medium @text-muted
                          button "Back" height=22.0 padding=4.0 -> fs_close_diff
                            active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=6.0
                            hovered bg=fg/10 text=fg
                            pressed bg=fg/14
                        if empty(fs_diff)
                          text "No differences." size=13.0 @text-muted
                        for entry in fs_diff
                          row width=fill spacing=8.0 align=center
                            text entry.kind width=64.0 size=11.0 wrapping=none font=mono @text-primary
                            text entry.path width=fill size=13.0 wrapping=none font=mono @text-fg
                    if empty(fs_diff_from)
                      col width=fill spacing=4.0
                        for snapshot in fs_history
                          container width=fill padding=7.0 bg=popover border=fg/10 border-w=1.0 r=8.0
                            col width=fill spacing=2.0
                              row width=fill spacing=8.0 align=center
                                text snapshot.short_id size=11.0 wrapping=none font=mono @text-primary
                                text snapshot.height size=11.0 wrapping=none font=mono @text-muted
                                space width=fill
                                text snapshot.author size=11.0 wrapping=none font=mono @text-muted
                                button "Diff" height=20.0 padding=3.0 -> fs_show_diff(snapshot.id)
                                  active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/14
                              if !empty(snapshot.message)
                                text snapshot.message size=13.0 @text-fg
              if !fs_history_open && empty(fs_preview_path)
                EmptyState title="Select a file" detail="Text files preview here; History shows the commit window."
              if !fs_history_open && !empty(fs_preview_path)
                col width=fill height=fill spacing=6.0
                  row width=fill spacing=8.0 align=center
                    text fs_preview_path width=fill size=11.0 wrapping=none font=mono @text-muted
                    if fs_preview_truncated
                      text "first 64 KiB" size=11.0 wrapping=none font=mono @text-muted
                    if !fs_preview_binary && !fs_editing && !fs_preview_truncated
                      button "Edit" height=22.0 padding=4.0 -> fs_begin_edit
                        active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=6.0
                        hovered bg=fg/10 text=fg
                        pressed bg=fg/14
                    if fs_editing
                      button "Cancel" height=22.0 padding=4.0 -> fs_cancel_edit
                        active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=6.0
                        hovered bg=fg/10 text=fg
                        pressed bg=fg/14
                    if fs_editing
                      button "Save" disabled=fs_loading height=22.0 padding=4.0 -> fs_save_edit
                        active bg=primary/16 text=fg border=primary/30 border-w=1.0 r=6.0
                        hovered bg=primary/24 text=fg
                        pressed bg=primary/30
                  stack width=fill height=fill
                    if fs_editing
                      editor #fs-editor <-> fs_editor placeholder="File contents…" disabled=fs_loading min-height=200.0 size=13.0 line-height=1.3 padding=6.6 wrapping=word
                        active bg=surface border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                        hovered bg=surface border=fg/21
                        focused bg=surface border=fg/45 border-w=1.0
                    if !fs_editing
                      scroll direction=vertical width=fill height=fill
                        col width=fill spacing=6.0
                          if fs_preview_binary
                            text fs_preview_text size=13.0 font=mono @text-muted
                          if !fs_preview_binary
                            text fs_preview_text size=13.0 font=mono @text-fg
    members:
      col width=fill height=fill padding=14.0 spacing=8.0
        row width=fill height=28.0 spacing=10.0 align=center
          text "Network members" size=14.0 font=display @text-fg
          space width=fill
          text members_validators size=13.0 wrapping=none font=mono @text-primary
          text "validators" size=11.0 wrapping=none @text-muted
          text members_residents size=13.0 wrapping=none font=mono @text-primary
          text "residents" size=11.0 wrapping=none @text-muted
        if empty(members_rows)
          EmptyState title="No members yet" detail="Validators and residents appear as they join."
        if !empty(members_rows)
          scroll direction=vertical width=fill height=fill
            col width=fill spacing=2.0
              for member in members_rows
                container width=fill padding=8.0 bg=surface border=fg/8 border-w=1.0 r=9.0
                  row width=fill spacing=10.0 align=center
                    text member.label size=13.0 wrapping=none font=mono @text-fg
                    text member.role size=11.0 wrapping=none font=mono @text-primary
                    if member.is_this_node
                      container height=18.0 padding-left=6.0 padding-right=6.0 align-y=center bg=primary/14 border=primary/30 border-w=1.0 r=9.0
                        text "this node" size=11.0 wrapping=none @text-primary
                    space width=fill
                    text member.key size=11.0 wrapping=none font=mono @text-muted
    agents:
      col width=fill height=fill padding=14.0 spacing=8.0
        row width=fill height=28.0 spacing=8.0 align=center
          text "Agents" size=14.0 font=display @text-fg
          space width=fill
        if empty(agents_rows)
          EmptyState title="No agents registered" detail="Registered agents appear here with their capability and grants."
        if !empty(agents_rows)
          scroll direction=vertical width=fill height=fill
            col width=fill spacing=4.0
              for agent in agents_rows
                container width=fill padding=9.0 bg=surface border=fg/8 border-w=1.0 r=9.0
                  col width=fill spacing=3.0
                    row width=fill spacing=8.0 align=center
                      text agent.name size=13.0 wrapping=none font=medium @text-fg
                      text agent.id size=11.0 wrapping=none font=mono @text-muted
                      space width=fill
                      text agent.status size=11.0 wrapping=none font=mono @text-primary
                    row width=fill spacing=8.0 align=center
                      text agent.capability size=11.0 wrapping=none font=mono @text-muted
                      text "·" size=11.0 wrapping=none @text-muted
                      text agent.owner size=11.0 wrapping=none font=mono @text-muted
                    if !empty(agent.actions)
                      text agent.actions size=11.0 font=mono @text-muted
    forge:
      col width=fill height=fill padding=14.0 spacing=8.0
        row width=fill height=28.0 spacing=8.0 align=center
          text "Forge" size=14.0 font=display @text-fg
          if !empty(forge_repo)
            text forge_repo size=13.0 wrapping=none font=mono @text-primary
          space width=fill
        if empty(forge_repos)
          EmptyState title="No repos" detail="Consensus-backed repos appear here once created."
        if !empty(forge_repos) && empty(forge_repo)
          scroll direction=vertical width=fill height=fill
            col width=fill spacing=2.0
              for repo in forge_repos
                button label="Open repo" width=fill padding=8.0 -> forge_open_repo(repo.name)
                  row width=fill height=fill spacing=8.0 align=center
                    text repo.name width=fill size=13.0 wrapping=none font=medium @text-fg
                    text repo.head size=11.0 wrapping=none font=mono @text-muted
                  active bg=surface text=fg border=fg/8 border-w=1.0 r=9.0
                  hovered bg=primary/10 text=fg
                  pressed bg=primary/16
        if !empty(forge_repo) && forge_item_number <= 0
          col width=fill height=fill spacing=6.0
            if !empty(forge_branches)
              scroll direction=horizontal width=fill height=26.0 bar=hidden
                row height=fill spacing=4.0 align=center
                  for branch in forge_branches
                    container height=20.0 padding-left=7.0 padding-right=7.0 align-y=center bg=surface border=fg/10 border-w=1.0 r=10.0
                      text branch size=11.0 wrapping=none font=mono @text-muted
            if empty(forge_items)
              EmptyState title="No issues or PRs" detail="The tracker is empty for this repo."
            if !empty(forge_items)
              scroll direction=vertical width=fill height=fill
                col width=fill spacing=2.0
                  for item in forge_items
                    button label="Open item" width=fill padding=7.0 -> forge_open_item(item.number)
                      row width=fill height=fill spacing=8.0 align=center
                        text item.number size=11.0 wrapping=none font=mono @text-muted
                        text item.kind size=11.0 wrapping=none font=mono @text-primary
                        text item.title width=fill size=13.0 wrapping=none @text-fg
                        text item.state size=11.0 wrapping=none font=mono @text-muted
                      active bg=surface text=fg border=fg/8 border-w=1.0 r=8.0
                      hovered bg=primary/10 text=fg
                      pressed bg=primary/16
        if forge_item_number > 0
          col width=fill height=fill spacing=6.0
            row width=fill spacing=8.0 align=center
              button "‹ Back" height=24.0 padding=5.0 -> forge_close_item
                active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=7.0
                hovered bg=fg/10 text=fg
                pressed bg=fg/14
              text forge_item_title width=fill size=13.0 wrapping=none font=medium @text-fg
              text forge_item_kind size=11.0 wrapping=none font=mono @text-primary
              text forge_item_state size=11.0 wrapping=none font=mono @text-muted
            row width=fill spacing=8.0 align=center
              if !empty(forge_item_author)
                text forge_item_author size=11.0 wrapping=none @text-muted
              if !empty(forge_item_branches)
                text forge_item_branches size=11.0 wrapping=none font=mono @text-muted
              if forge_item_files_changed > 0
                text forge_stats(forge_item_files_changed, forge_item_additions, forge_item_deletions) size=11.0 wrapping=none font=mono @text-muted
              space width=fill
            scroll direction=vertical width=fill height=fill
              col width=fill spacing=8.0
                if !empty(forge_item_body)
                  container width=fill padding=9.0 bg=surface border=fg/8 border-w=1.0 r=9.0
                    text forge_item_body size=13.0 @text-fg
                if !empty(forge_item_diff)
                  container width=fill padding=9.0 bg=surface border=fg/8 border-w=1.0 r=9.0
                    col width=fill spacing=5.0
                      if forge_item_diff_truncated
                        text "Patch truncated — the statistics cover the full diff." size=11.0 @text-muted
                      text forge_item_diff size=11.0 font=mono @text-fg
                if forge_item_kind == "pr"
                  container width=fill padding=9.0 bg=surface border=fg/8 border-w=1.0 r=9.0
                    col width=fill spacing=6.0
                      row width=fill spacing=6.0 align=center
                        text "Merge" width=fill size=13.0 font=medium @text-fg
                        text forge_item_approvals size=11.0 wrapping=none font=mono @text-primary
                        text "approvals" size=11.0 wrapping=none @text-muted
                        text "·" size=11.0 wrapping=none @text-muted
                        text forge_item_change_requests size=11.0 wrapping=none font=mono @text-muted
                        text "change requests" size=11.0 wrapping=none @text-muted
                      if forge_item_state == "merged"
                        text forge_merge_note(forge_item_merge_oid, forge_item_branches) size=13.0 font=mono @text-primary
                      if forge_item_state == "closed"
                        text "Closed without merging." size=13.0 @text-muted
                      if forge_item_state == "open"
                        if !empty(forge_merge_conflicts)
                          col width=fill spacing=3.0
                            text "Merge conflicts — resolve on the branch and push again:" size=11.0 @text-muted
                            for conflict_path in forge_merge_conflicts
                              text conflict_path size=11.0 font=mono @text-fg
                        row width=fill spacing=8.0 align=center
                          if !forge_merge_busy
                            button "Merge pull request" disabled=(!connected || empty(forge_item_source_oid)) height=28.0 padding=6.0 -> forge_merge_submit
                              active bg=primary text=fg border=primaryhi/50 border-w=1.0 r=9.0
                              hovered bg=primaryhi text=fg
                              pressed bg=primary text=fg
                              disabled bg=fg/8 text=muted
                          if forge_merge_busy
                            button "Merging…" disabled=true height=28.0 padding=6.0 -> forge_merge_submit
                              active bg=fg/8 text=muted border=fg/10 border-w=1.0 r=9.0
                              disabled bg=fg/8 text=muted
                          text "Approvals are advisory — merging is never gated." size=11.0 wrapping=none @text-muted
                if forge_item_kind == "pr"
                  container width=fill padding=9.0 bg=surface border=fg/8 border-w=1.0 r=9.0
                    col width=fill spacing=6.0
                      text "Reviews" size=13.0 font=medium @text-fg
                      if empty(forge_item_reviews)
                        text "No reviews yet." size=13.0 @text-muted
                      for review in forge_item_reviews
                        container width=fill padding=8.0 bg=elevated border=fg/8 border-w=1.0 r=8.0
                          col width=fill spacing=4.0
                            row width=fill spacing=7.0 align=center
                              text review.author_name size=13.0 wrapping=none font=medium @text-fg
                              if review.verdict == "approve"
                                text verdict_label(review.verdict) size=11.0 wrapping=none font=mono @text-primary
                              if review.verdict != "approve"
                                text verdict_label(review.verdict) size=11.0 wrapping=none font=mono @text-muted
                              text review.commit size=11.0 wrapping=none font=mono @text-muted
                              if review.outdated
                                text "outdated" size=11.0 wrapping=none font=mono @text-muted
                              space width=fill
                            if !empty(review.body)
                              text review.body size=13.0 @text-fg
                            for comment in review.comments
                              container width=fill padding=6.0 bg=surface border=fg/8 border-w=1.0 r=7.0
                                col width=fill spacing=2.0
                                  text comment.anchor size=11.0 font=mono @text-muted
                                  text comment.body size=13.0 @text-fg
                      row width=fill spacing=6.0 align=center
                        button label="Pick comment verdict" height=24.0 padding=5.0 -> forge_review_pick("comment")
                          text verdict_pick_label(forge_review_verdict, "comment", "Comment") size=11.0
                          active bg=fg/6 text=fg border=fg/10 border-w=1.0 r=7.0
                          hovered bg=fg/10 text=fg
                          pressed bg=fg/14
                        button label="Pick approve verdict" height=24.0 padding=5.0 -> forge_review_pick("approve")
                          text verdict_pick_label(forge_review_verdict, "approve", "Approve") size=11.0
                          active bg=primary/14 text=fg border=primary/26 border-w=1.0 r=7.0
                          hovered bg=primary/22 text=fg
                          pressed bg=primary/30
                        button label="Pick request-changes verdict" height=24.0 padding=5.0 -> forge_review_pick("request_changes")
                          text verdict_pick_label(forge_review_verdict, "request_changes", "Request changes") size=11.0
                          active bg=danger/10 text=fg border=danger/26 border-w=1.0 r=7.0
                          hovered bg=danger/18 text=fg
                          pressed bg=danger/24
                        space width=fill
                      row width=fill spacing=6.0 align=center
                        input "" #forge-review-body label="Review body" <-> forge_review_draft hint="Leave a review…" disabled=(forge_review_busy || !connected) submit=forge_review_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
                          active bg=elevated border=fg/16 value=fg placeholder=muted selection=primary/40 border-w=1.0 r=8.0
                          hovered bg=elevated border=fg/21
                          focused bg=elevated border=primary/45
                          disabled bg=fg/6 value=muted
                        button "Submit review" disabled=(forge_review_busy || !connected || empty(forge_item_source_oid)) height=28.0 padding=6.0 -> forge_review_submit
                          active bg=primary/16 text=fg border=primary/30 border-w=1.0 r=8.0
                          hovered bg=primary/24 text=fg
                          pressed bg=primary/30
                          disabled bg=fg/8 text=muted
                container width=fill padding=9.0 bg=surface border=fg/8 border-w=1.0 r=9.0
                  col width=fill spacing=6.0
                    text "Discussion" size=13.0 font=medium @text-fg
                    if empty(forge_discussion)
                      text "No discussion yet." size=13.0 @text-muted
                    for message in forge_discussion
                      row width=fill spacing=9.0 align=start
                        container width=28.0 height=28.0 align-x=center align-y=center style=avatar_style(message.avatar_r, message.avatar_g, message.avatar_b)
                          text message.initial size=13.0 font=display @text-fg
                        col width=fill spacing=2.0
                          row width=fill spacing=7.0 align=center
                            text message.author size=13.0 wrapping=none font=display @text-fg
                            text message.meta size=11.0 wrapping=none @text-muted
                            space width=fill
                          MessageBody message=message
                    flex width=fill gap=8.0 align-items=end
                      editor #forge-note <-> forge_discussion_editor placeholder="Write a note…" disabled=(loading || !connected || empty(forge_item_channel)) min-height=38.0 max-height=120.0 size=13.0 line-height=1.3 padding=6.0 wrapping=word key-binding=composer_keys() -> forge_note_submit
                        active bg=transparent border=fg/10 value=fg placeholder=muted selection=primary/40 border-w=1.0 r=8.0
                        hovered bg=fg/4 border=fg/12 border-w=1.0
                        focused bg=fg/6 border=primary/45 border-w=1.0
                        disabled value=muted
                      button "Send" disabled=(loading || !connected || empty(forge_item_channel) || !empty(forge_discussion_pending) || empty(trim(editor_text(forge_discussion_editor)))) width=60.0 height=28.0 padding=6.0 -> forge_note_submit
                        active bg=primary text=fg border=primaryhi/50 border-w=1.0 r=9.0
                        hovered bg=primaryhi text=fg
                        pressed bg=primary text=fg
                        disabled bg=fg/8 text=muted
    governance:
      col width=fill height=fill padding=14.0 spacing=8.0
        row width=fill height=28.0 spacing=8.0 align=center
          text "Governance" size=14.0 font=display @text-fg
          space width=fill
        if empty(gov_rows)
          EmptyState title="No proposals" detail="Membership and share actions appear here as proposals."
        if !empty(gov_rows)
          scroll direction=vertical width=fill height=fill
            col width=fill spacing=4.0
              for proposal in gov_rows
                container width=fill padding=9.0 bg=surface border=fg/8 border-w=1.0 r=9.0
                  col width=fill spacing=4.0
                    row width=fill spacing=8.0 align=center
                      text proposal.id size=13.0 wrapping=none font=medium @text-fg
                      text proposal.action size=11.0 wrapping=none font=mono @text-primary
                      text proposal.status size=11.0 wrapping=none font=mono @text-muted
                      space width=fill
                      text proposal.proposer size=11.0 wrapping=none font=mono @text-muted
                    row width=fill spacing=8.0 align=center
                      text proposal.approvals size=11.0 wrapping=none font=mono @text-primary
                      text "for" size=11.0 wrapping=none @text-muted
                      text proposal.rejections size=11.0 wrapping=none font=mono @text-muted
                      text "against" size=11.0 wrapping=none @text-muted
                      text "·" size=11.0 wrapping=none @text-muted
                      text proposal.electorate size=11.0 wrapping=none font=mono @text-muted
                      text "electorate" size=11.0 wrapping=none @text-muted
                      space width=fill
                      if proposal.open
                        button "Approve" disabled=(!empty(gov_voting)) height=22.0 padding=4.0 -> gov_vote(proposal.id, true)
                          active bg=primary/16 text=fg border=primary/30 border-w=1.0 r=6.0
                          hovered bg=primary/24 text=fg
                          pressed bg=primary/30
                      if proposal.open
                        button "Reject" disabled=(!empty(gov_voting)) height=22.0 padding=4.0 -> gov_vote(proposal.id, false)
                          active bg=danger/12 text=fg border=danger/30 border-w=1.0 r=6.0
                          hovered bg=danger/20 text=fg
                          pressed bg=danger/28
                      if proposal.open
                        button "Settle" disabled=(!empty(gov_voting)) height=22.0 padding=4.0 -> gov_execute(proposal.id)
                          active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=6.0
                          hovered bg=fg/10 text=fg
                          pressed bg=fg/14
    node:
      col width=fill height=fill padding=14.0 spacing=8.0
        row width=fill height=28.0 spacing=8.0 align=center
          text "Node" size=14.0 font=display @text-fg
          space width=fill
          input "" #log-filter label="Filter logs" <-> node_log_filter change=node_log_filter_changed hint="filter…" width=180.0 padding=5.0 text-size=13.0 line-height=1.2
            active bg=surface border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
            hovered bg=elevated border=fg/21
            focused bg=elevated border=fg/45 border-w=1.0
        if !empty(node_peers)
          container width=fill padding=8.0 bg=surface border=fg/8 border-w=1.0 r=9.0
            col width=fill spacing=3.0
              text "PEERS" size=11.0 font=medium @text-muted
              for peer in node_peers
                row width=fill spacing=8.0 align=center
                  if peer.live
                    container width=7.0 height=7.0 bg=success r=3.5
                      text ""
                  if !peer.live
                    container width=7.0 height=7.0 bg=fg/30 r=3.5
                      text ""
                  text peer.key width=fill size=13.0 wrapping=none font=mono @text-fg
                  text peer.height size=11.0 wrapping=none font=mono @text-muted
        container width=fill height=fill padding=8.0 bg=surface border=fg/8 border-w=1.0 r=9.0
          stack width=fill height=fill
            if empty(node_log_lines)
              EmptyState title="Waiting for logs" detail="The node's log ring streams here live."
            if !empty(node_log_lines)
              scroll direction=vertical width=fill height=fill
                col width=fill spacing=1.0
                  for line in filter_log_lines(node_log_lines, node_log_filter)
                    text line.line size=11.0 font=mono @text-fg
    settings:
      scroll direction=vertical width=fill height=fill
        container width=fill max-width=640.0 margin-x=auto padding=20.0
          col width=fill spacing=12.0
            text "Settings" size=15.0 font=display @text-fg
            container width=fill padding=10.0 bg=surface border=fg/8 border-w=1.0 r=10.0
              col width=fill spacing=6.0
                text "ACCOUNT" size=11.0 font=medium @text-muted
                if !account_bound
                  text "This node is not bound to an account yet." size=13.0 @text-muted
                if account_bound
                  col width=fill spacing=6.0
                    row width=fill spacing=8.0 align=center
                      text "Display name" width=120.0 size=13.0 @text-muted
                      if !empty(account_name)
                        text account_name width=fill size=13.0 wrapping=none font=medium @text-fg
                      if empty(account_name)
                        text "(unnamed)" width=fill size=13.0 wrapping=none @text-muted
                    row width=fill spacing=8.0 align=center
                      input "" #account-rename label="New display name" <-> account_name_draft change=account_name_draft_changed hint="rename account…" disabled=account_renaming width=fill padding=5.0 text-size=13.0 line-height=1.2
                        active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                        hovered bg=elevated border=fg/21
                        focused bg=elevated border=fg/45 border-w=1.0
                        disabled bg=surface/54 value=muted
                      button "Rename" disabled=(account_renaming || empty(trim(account_name_draft))) height=26.0 padding=5.0 -> account_rename_submit
                        active bg=primary/16 text=fg border=primary/30 border-w=1.0 r=7.0
                        hovered bg=primary/24 text=fg
                        pressed bg=primary/30
                        disabled bg=fg/4 text=muted
                    row width=fill spacing=8.0 align=center
                      text "Account" width=120.0 size=13.0 @text-muted
                      text account_id size=13.0 wrapping=none font=mono @text-muted
                      text "·" size=11.0 wrapping=none @text-muted
                      text account_members size=13.0 wrapping=none font=mono @text-muted
                      text "keys" size=11.0 wrapping=none @text-muted
                      text account_nodes size=13.0 wrapping=none font=mono @text-muted
                      text "nodes" size=11.0 wrapping=none @text-muted
            container width=fill padding=10.0 bg=surface border=fg/8 border-w=1.0 r=10.0
              col width=fill spacing=6.0
                text "CONNECTION" size=11.0 font=medium @text-muted
                row width=fill spacing=8.0 align=center
                  text "Endpoint" width=120.0 size=13.0 @text-muted
                  text settings_endpoint width=fill size=13.0 wrapping=none font=mono @text-fg
                row width=fill spacing=8.0 align=center
                  text "Node key" width=120.0 size=13.0 @text-muted
                  text settings_node_key width=fill size=13.0 wrapping=none font=mono @text-fg
                row width=fill spacing=8.0 align=center
                  text "Block height" width=120.0 size=13.0 @text-muted
                  text settings_height width=fill size=13.0 wrapping=none font=mono @text-fg
                text "Change the endpoint from the sidebar's Connection panel." size=11.0 @text-muted
            container width=fill padding=10.0 bg=surface border=fg/8 border-w=1.0 r=10.0
              col width=fill spacing=6.0
                text "IDENTITY" size=11.0 font=medium @text-muted
                row width=fill spacing=8.0 align=center
                  text "User key" width=120.0 size=13.0 @text-muted
                  text settings_key_state width=fill size=13.0 wrapping=none font=mono @text-primary
                row width=fill spacing=8.0 align=center
                  text "Key path" width=120.0 size=13.0 @text-muted
                  text settings_key_path width=fill size=11.0 wrapping=none font=mono @text-muted
            container width=fill padding=10.0 bg=surface border=fg/8 border-w=1.0 r=10.0
              col width=fill spacing=6.0
                text "THIS DEVICE" size=11.0 font=medium @text-muted
                row width=fill spacing=8.0 align=center
                  text "Open page tabs" width=120.0 size=13.0 @text-muted
                  text settings_open_tabs size=13.0 wrapping=none font=mono @text-fg
                  space width=fill
                  button "Forget tabs" height=24.0 padding=5.0 -> settings_clear_tabs
                    active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=7.0
                    hovered bg=fg/10 text=fg
                    pressed bg=fg/14
                text "Preferences persist per endpoint in app-prefs.json beside the user key." size=11.0 @text-muted
    explorer:
      col width=fill height=fill padding=14.0 spacing=8.0
        row width=fill height=28.0 spacing=8.0 align=center
          text "Block explorer" size=14.0 font=display @text-fg
          space width=fill
          if explorer_loading
            text "Loading…" size=11.0 font=mono @text-muted
          button "Refresh" disabled=explorer_loading height=26.0 padding=5.0 -> refresh_explorer
            active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=7.0
            hovered bg=fg/10 text=fg border=fg/14
            pressed bg=fg/14 text=fg
        if empty(explorer_blocks) && !explorer_loading
          EmptyState title="No blocks yet" detail="Non-empty blocks appear here as they finalize."
        if !empty(explorer_blocks)
          row width=fill height=fill spacing=10.0
            container width=340.0 height=fill padding=6.0 bg=surface border=fg/10 border-w=1.0 r=10.0
              scroll direction=vertical width=fill height=fill
                col width=fill spacing=1.0
                  for block in explorer_blocks
                    button label="Inspect block" width=fill padding=6.0 -> select_explorer_block(block.height)
                      row width=fill height=fill spacing=8.0 align=center
                        text block.height size=13.0 wrapping=none font=mono @text-primary
                        text block.hash width=fill size=11.0 wrapping=none font=mono @text-muted
                        text block.op_count size=11.0 wrapping=none font=mono @text-muted
                      active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
                      hovered bg=primary/10 text=fg
                      pressed bg=primary/16
            container width=fill height=fill padding=8.0 bg=surface border=fg/10 border-w=1.0 r=10.0
              stack width=fill height=fill
                if explorer_selected <= 0
                  EmptyState title="Select a block" detail="Its operations and dispatch traces appear here."
                if explorer_selected > 0
                  scroll direction=vertical width=fill height=fill
                    col width=fill spacing=6.0
                      for op in explorer_ops_at(explorer_ops, explorer_selected)
                        container width=fill padding=8.0 bg=popover border=fg/10 border-w=1.0 r=9.0
                          col width=fill spacing=3.0
                            row width=fill spacing=8.0 align=center
                              text op.target size=13.0 wrapping=none font=medium @text-fg
                              text op.disposition size=11.0 wrapping=none font=mono @text-primary
                              space width=fill
                              text op.op_hash size=11.0 wrapping=none font=mono @text-muted
                            row width=fill spacing=8.0 align=center
                              text "by" size=11.0 wrapping=none @text-muted
                              text op.proposer size=11.0 wrapping=none font=mono @text-muted
                            if !empty(op.trace)
                              text op.trace size=11.0 font=mono @text-muted
                            text op.payload size=13.0 @text-fg
    palette:
      stack width=fill height=fill
        if palette_open
          container width=fill height=fill align-x=center padding-top=72.0 bg=shadow/45
            container width=540.0 padding=10.0 bg=popover border=fg/14 border-w=1.0 r=12.0 shadow=shadow shadow-y=8.0 shadow-blur=24.0
              col width=fill spacing=8.0
                input "" #palette-input label="Search everything" <-> palette_draft change=palette_changed hint="Search messages and pages… (Esc closes)" submit=close_palette width=fill padding=8.0 text-size=14.0 line-height=1.2
                  active bg=surface border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
                  hovered bg=elevated border=fg/21
                  focused bg=elevated border=fg/45 border-w=1.0
                if palette_searching
                  text "Searching…" size=11.0 @text-muted
                if !empty(palette_chat_hits) || !empty(palette_page_hits)
                  scroll direction=vertical width=fill height=380.0
                    col width=fill spacing=4.0
                      if !empty(palette_chat_hits)
                        container width=fill padding-left=4.0
                          text "MESSAGES" size=11.0 font=medium @text-muted
                        col width=fill spacing=1.0
                          for hit in palette_chat_hits
                            button label="Open message" width=fill padding=6.0 -> open_chat_search_hit(hit.channel_id, hit.root_seq, hit.seq)
                              col width=fill spacing=1.0
                                text hit.text size=13.0 wrapping=none @text-fg
                                text hit.meta size=11.0 wrapping=none font=mono @text-muted
                              active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                              hovered bg=primary/12 text=fg
                              pressed bg=primary/18
                      if !empty(palette_page_hits)
                        container width=fill padding-left=4.0
                          text "PAGES" size=11.0 font=medium @text-muted
                        col width=fill spacing=1.0
                          for hit in palette_page_hits
                            button label="Open page" width=fill padding=6.0 -> open_page_search_hit(hit.page_id, hit.block_id)
                              col width=fill spacing=1.0
                                text hit.text size=13.0 wrapping=none @text-fg
                                text hit.kind size=11.0 wrapping=none font=mono @text-muted
                              active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                              hovered bg=primary/12 text=fg
                              pressed bg=primary/18
    bell:
      stack width=fill height=fill
        if bell_open
          button label="Close notifications" width=fill height=fill padding=0.0 -> close_bell
            space width=fill height=fill
            active bg=transparent border=transparent
        if bell_open
          container width=fill height=fill align-x=end align-y=start padding-top=48.0 padding-right=120.0
            container width=360.0 padding=8.0 bg=popover border=fg/14 border-w=1.0 r=10.0 shadow=shadow shadow-y=6.0 shadow-blur=18.0
              col width=fill spacing=4.0
                text "Notifications" size=13.0 font=medium @text-fg
                if empty(bell_items)
                  text "Nothing yet — mentions and deliveries land here." size=13.0 @text-muted
                if !empty(bell_items)
                  scroll direction=vertical width=fill height=320.0
                    col width=fill spacing=3.0
                      for item in bell_items
                        container width=fill padding=7.0 bg=surface border=fg/8 border-w=1.0 r=8.0
                          col width=fill spacing=2.0
                            row width=fill spacing=8.0 align=center
                              text item.kind size=11.0 wrapping=none font=mono @text-primary
                              space width=fill
                              text item.source size=11.0 wrapping=none font=mono @text-muted
                            text item.body size=13.0 @text-fg
