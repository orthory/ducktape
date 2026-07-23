view
  WorkspaceTabs status=status loading=(loading || mutation_phase != "idle") #workspace-tabs
    connection:
      container width=fill padding=6.0 bg=transparent border=white/11 border-w=1.0 r=10.0
        col width=fill spacing=5.0
          input "" #rpc label="RPC endpoint" <-> rpc hint="Node URL" disabled=(loading || (mutation_phase != "idle" && mutation_phase != "recovering")) submit=reconnect width=fill padding=6.2 text-size=13.0 line-height=1.2
            active bg=surface border=white/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
            hovered bg=elevated border=white/21
            focused bg=elevated border=fg/45 border-w=1.0
            disabled bg=surface/54 value=muted
          input "" #password label="Local key password" secure=true <-> password hint="Key password" disabled=(loading || mutation_phase != "idle") width=fill padding=6.2 text-size=13.0 line-height=1.2
            active bg=surface border=white/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
            hovered bg=elevated border=white/21
            focused bg=elevated border=fg/45 border-w=1.0
            disabled bg=surface/54 value=muted
          button "Connect" disabled=(loading || (mutation_phase != "idle" && mutation_phase != "recovering")) width=fill height=28.0 padding=7.0 -> reconnect
            active bg=fg/90 text=bg border=white/6 border-w=1.0 r=10.0
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
              hovered bg=white/10 text=fg
              pressed bg=selection
              disabled text=muted
          if channel_create_open
            button label="Close new channel" disabled=(loading || mutation_phase != "idle") width=28.0 height=28.0 padding=0.0 -> toggle_channel_create
              container width=fill height=fill align-x=center align-y=center
                text "×" size=14.0
              active bg=transparent text=muted r=8.0
              hovered bg=white/10 text=fg
              pressed bg=selection
        if channel_create_open
          row width=fill height=28.0 spacing=5.0 align=center
            input "" #new-channel label="New channel name" <-> channel_draft hint="New channel" disabled=(loading || mutation_phase != "idle" || !connected) submit=create_channel_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
              active bg=surface border=white/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
              hovered bg=elevated border=white/21
              focused bg=elevated border=fg/45 border-w=1.0
              disabled bg=surface/54 value=muted
            button label="Create channel" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(channel_draft))) width=28.0 height=28.0 padding=0.0 -> create_channel_submit
              container width=fill height=fill align-x=center align-y=center
                text "+" size=14.0
              active bg=white/13 text=fg border=white/18 border-w=1.0 r=8.0
              hovered bg=white/19
              pressed bg=selection
              disabled bg=white/5 text=muted
        scroll direction=vertical width=fill height=fill
          col width=fill spacing=2.0
            for channel in channels
              ChannelButton channel=channel selected=(channel.id == active_channel)
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
              hovered bg=white/10 text=fg
              pressed bg=selection
              disabled text=muted
          if page_create_open
            button label="Close new page" disabled=(loading || mutation_phase != "idle") width=28.0 height=28.0 padding=0.0 -> toggle_page_create
              container width=fill height=fill align-x=center align-y=center
                text "×" size=14.0
              active bg=transparent text=muted r=8.0
              hovered bg=white/10 text=fg
              pressed bg=selection
        if page_create_open
          row width=fill height=28.0 spacing=5.0 align=center
            input "" #new-page label="New page title" <-> page_draft hint="New page" disabled=(loading || mutation_phase != "idle" || !connected) submit=create_page_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
              active bg=surface border=white/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
              hovered bg=elevated border=white/21
              focused bg=elevated border=fg/45 border-w=1.0
              disabled bg=surface/54 value=muted
            button label="Create page" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(page_draft))) width=28.0 height=28.0 padding=0.0 -> create_page_submit
              container width=fill height=fill align-x=center align-y=center
                text "+" size=14.0
              active bg=white/13 text=fg border=white/18 border-w=1.0 r=8.0
              hovered bg=white/19
              pressed bg=selection
              disabled bg=white/5 text=muted
        scroll direction=vertical width=fill height=fill
          col width=fill spacing=2.0
            for page in pages
              PageButton page=page selected=(page.id == active_page)
    notice:
      col width=fill
        if error != ""
          container width=fill padding-left=12.0 padding-right=12.0 padding-bottom=8.0
            container width=fill padding=8.0 bg=elevated border=white/18 border-w=1.0 r=12.0 shadow=black/12 shadow-y=2.0 shadow-blur=12.0
              row width=fill spacing=8.0 align=center
                container width=20.0 height=20.0 align-x=center align-y=center bg=white/12 border=white/20 border-w=1.0 r=10.0
                  text "!" size=11.0 font=medium @text-fg
                text error width=fill size=13.0 @text-fg
                button "Dismiss" height=26.0 padding=5.0 -> dismiss_error
                  active bg=transparent text=muted r=7.0
                  hovered bg=white/9 text=fg
                  pressed bg=white/14
    chat:
      container width=fill height=fill bg=transparent clip=true px-snap=true
        row width=fill height=fill
          col width=fill height=fill spacing=9.0 padding=14.0
            if !empty(active_channel)
              row width=fill height=28.0 spacing=7.0 align=center
                container width=22.0 height=22.0 align-x=center align-y=center bg=white/10 border=white/16 border-w=1.0 r=7.0
                  text "#" size=13.0 font=medium @text-fg
                text active_channel_name width=fill size=13.0 font=medium @text-fg
                if active_channel_archived
                  text "Archived" size=11.0 @text-muted
                if active_channel_members_only
                  text "Members" size=11.0 @text-muted
                if active_channel_huddle_count > 0
                  text active_channel_huddle_count size=11.0 @text-muted
                input "" #chat-search label="Search messages" <-> chat_search_draft hint="Search messages" disabled=(!connected || chat_searching) submit=search_chat_submit width=180.0 padding=6.2 text-size=13.0 line-height=1.2
                  active bg=transparent border=white/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                  hovered bg=white/4 border=white/14
                  focused bg=white/7 border=fg/40
                  disabled bg=transparent value=muted
                if !empty(chat_search_hits)
                  button label="Clear message search" width=28.0 height=28.0 padding=0.0 -> clear_chat_search
                    container width=fill height=fill align-x=center align-y=center
                      text "×" size=14.0
                    active bg=transparent text=muted r=7.0
                    hovered bg=white/10 text=fg
                    pressed bg=white/15
                button label="Channel details" width=28.0 height=28.0 padding=0.0 -> toggle_channel_settings
                  container width=fill height=fill align-x=center align-y=center
                    text "•••" size=13.0
                  active bg=transparent text=muted r=7.0
                  hovered bg=white/10 text=fg
                  pressed bg=white/15
            if !empty(chat_search_hits)
              container width=fill height=148.0 padding=6.0 bg=elevated border=white/10 border-w=1.0 r=10.0
                scroll direction=vertical width=fill height=fill
                  col width=fill spacing=1.0
                    for hit in chat_search_hits
                      ChatSearchResult hit=hit
            if !connected
              EmptyState title="Connect to a node" detail="Set the RPC endpoint in the sidebar."
            if connected && empty(messages)
              EmptyState title="No messages yet" detail="Create a channel or start the conversation."
            if connected && !empty(messages)
              stack width=fill height=fill
                mouse move=chat_pointer_moved
                  sensor show=chat_resized resize=chat_resized
                    scroll direction=vertical width=fill height=fill
                      col width=fill spacing=1.0
                        for message in messages
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
                            container width=190.0 padding=4.0 bg=popover border=white/16 border-w=1.0 r=8.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
                              col width=fill spacing=1.0
                                button "React" label="Manage reactions" disabled=active_channel_archived width=fill height=28.0 padding=6.0 -> open_message_reactions(selected_message_seq, message_edit_draft, selected_message_rev)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=white/10 text=fg
                                  pressed bg=white/15
                                button "Open thread" width=fill height=28.0 padding=6.0 -> open_thread_for(selected_message_seq)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=white/10 text=fg
                                  pressed bg=white/15
                                button "Edit" width=fill height=28.0 padding=6.0 -> begin_message_edit(selected_message_seq, message_edit_draft, selected_message_rev)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=white/10 text=fg
                                  pressed bg=white/15
                                button "Delete" width=fill height=28.0 padding=6.0 -> arm_message_delete(selected_message_seq, message_edit_draft, selected_message_rev)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=white/10 text=fg
                                  pressed bg=white/15
                                button "Close" label="Close message actions" width=fill height=28.0 padding=6.0 -> clear_message_selection
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=white/10 text=fg
                                  pressed bg=white/15
                        if message_action == "reactions"
                          stack
                            input "" #message-reaction-focus label="Message reaction focus" <-> message_action_focus width=1.0 padding=0.0 text-size=1.0 line-height=1.0
                              active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                              focused bg=transparent border=transparent value=transparent border-w=0.0
                            container padding=3.0 bg=popover border=white/16 border-w=1.0 r=8.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
                              row spacing=2.0 align=center
                                button "+ 👍" label="Add thumbs up reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_submit("👍")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=white/10
                                  pressed bg=white/15
                                button "+ ♥" label="Add heart reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_submit("❤️")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=white/10
                                  pressed bg=white/15
                                button "+ 😄" label="Add smile reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_submit("😄")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=white/10
                                  pressed bg=white/15
                                button "+ 🎉" label="Add celebration reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_submit("🎉")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=white/10
                                  pressed bg=white/15
                                button "+ 👀" label="Add eyes reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_submit("👀")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=white/10
                                  pressed bg=white/15
                                button "+ 🙌" label="Add raised hands reaction" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> add_reaction_submit("🙌")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=white/10
                                  pressed bg=white/15
                                for message in messages
                                  if message.seq == selected_message_seq
                                    for reaction in message.reactions
                                      if reaction.reacted_by_me
                                        button label="Remove my reaction" description=reaction.emoji disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> remove_reaction_submit(reaction.emoji)
                                          text reaction.emoji size=11.0 @text-fg
                                          active bg=white/7 text=fg r=6.0
                                          hovered bg=white/12
                                          pressed bg=white/17
                                button "×" label="Close reactions" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=4.0 -> clear_message_selection
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=white/10 text=fg
                                  pressed bg=white/15
                        if message_action == "editing"
                          container width=fill max-width=520.0 padding=3.0 bg=popover border=white/16 border-w=1.0 r=8.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
                            row width=fill spacing=4.0 align=center
                              input "" #message-edit label="Edit message" <-> message_edit_draft hint="Edit message" disabled=(mutation_phase != "idle") submit=edit_message_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
                                active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                hovered bg=white/4 border=white/8
                                focused bg=white/7 border=white/12
                                disabled value=muted
                              button "Save" label="Save message changes" disabled=(mutation_phase != "idle" || empty(trim(message_edit_draft))) height=28.0 padding=6.0 -> edit_message_submit
                                active bg=white/11 text=fg border=white/13 border-w=1.0 r=7.0
                                hovered bg=white/16
                                pressed bg=white/20
                              button label="Cancel message edit" disabled=(mutation_phase != "idle") width=28.0 height=28.0 padding=0.0 -> clear_message_selection
                                container width=fill height=fill align-x=center align-y=center
                                  text "×" size=14.0
                                active bg=transparent text=muted r=7.0
                                hovered bg=white/10 text=fg
                                pressed bg=white/15
                        if message_action == "delete"
                          stack
                            input "" #message-delete-focus label="Message delete focus" <-> message_action_focus width=1.0 padding=0.0 text-size=1.0 line-height=1.0
                              active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                              focused bg=transparent border=transparent value=transparent border-w=0.0
                            container padding=3.0 bg=popover border=white/16 border-w=1.0 r=8.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
                              row spacing=5.0 align=center
                                text "Delete this message?" size=11.0 @text-muted
                                button "Delete" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> delete_message_submit
                                  active bg=white/12 text=fg r=6.0
                                  hovered bg=white/17
                                  pressed bg=white/22
                                button "Cancel" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> clear_message_selection
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=white/10 text=fg
                                  pressed bg=white/15
            if !empty(failed_message_draft)
              row width=fill spacing=6.0 align=center
                text "An earlier message wasn’t sent" width=fill size=13.0 @text-muted
                button "Restore" disabled=(!empty(message_draft) || mutation_phase != "idle") height=28.0 padding=5.0 -> restore_failed_message
                  active bg=white/9 text=fg border=white/11 border-w=1.0 r=7.0
                  hovered bg=white/14
                  pressed bg=white/18
                button label="Dismiss unsent message" width=28.0 height=28.0 padding=0.0 -> dismiss_failed_message
                  container width=fill height=fill align-x=center align-y=center
                    text "×" size=14.0
                  active bg=transparent text=muted r=7.0
                  hovered bg=white/10 text=fg
                  pressed bg=white/15
            container width=fill padding=6.0 bg=surface border=white/16 border-w=1.0 r=14.0 shadow=black/10 shadow-y=2.0 shadow-blur=12.0
              flex width=fill gap=6.0 align-items=center
                input "" #message label="Message" <-> message_draft hint="Write a message…" disabled=(loading || !connected || empty(active_channel) || active_channel_archived) submit=send_message_submit width=fill padding=6.6 text-size=14.0 line-height=1.2
                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=9.0
                  hovered bg=white/4 border=white/8 border-w=1.0
                  focused bg=white/7 border=white/14 border-w=1.0
                  disabled value=muted
                button "Send" disabled=(loading || !connected || empty(active_channel) || active_channel_archived || empty(trim(message_draft))) width=62.0 height=30.0 padding=7.0 -> send_message_submit
                  active bg=fg/90 text=bg border=white/5 border-w=1.0 r=10.0 shadow=black/14 shadow-y=2.0 shadow-blur=7.0
                  hovered bg=fg/80 text=bg
                  pressed bg=fg text=bg
                  disabled bg=white/8 text=muted
          if channel_settings_open && !empty(active_channel)
            container width=1.0 height=fill bg=white/8
              text ""
            container width=300.0 height=fill padding=12.0 bg=surface
              col width=fill height=fill spacing=8.0
                row width=fill height=28.0 spacing=6.0 align=center
                  text "Channel details" width=fill size=13.0 font=medium @text-fg
                  button label="Close channel details" width=28.0 height=28.0 padding=0.0 -> toggle_channel_settings
                    container width=fill height=fill align-x=center align-y=center
                      text "×" size=14.0
                    active bg=transparent text=muted r=7.0
                    hovered bg=white/10 text=fg
                    pressed bg=white/15
                container width=fill height=1.0 bg=separator
                  text ""
                row width=fill spacing=5.0 align=center
                  input "" #channel-name label="Channel name" <-> channel_name_draft hint="Channel name" disabled=(mutation_phase != "idle") submit=rename_channel_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
                    active bg=transparent border=white/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                    hovered bg=white/4 border=white/14
                    focused bg=white/7 border=white/12
                    disabled value=muted
                  button "Rename" disabled=(mutation_phase != "idle" || empty(trim(channel_name_draft))) width=56.0 height=28.0 padding=5.0 -> rename_channel_submit
                    active bg=white/9 text=fg border=white/11 border-w=1.0 r=7.0
                    hovered bg=white/14
                    pressed bg=white/18
                row width=fill spacing=5.0 align=center
                  if !active_channel_archived
                    button "Archive" disabled=(mutation_phase != "idle") height=28.0 padding=5.0 -> archive_channel_submit
                      active bg=transparent text=muted r=7.0
                      hovered bg=white/10 text=fg
                      pressed bg=white/15
                    button "Join huddle" disabled=(mutation_phase != "idle") height=28.0 padding=5.0 -> join_huddle_submit
                      active bg=transparent text=muted r=7.0
                      hovered bg=white/10 text=fg
                      pressed bg=white/15
                  if active_channel_archived
                    button "Unarchive" disabled=(mutation_phase != "idle") height=28.0 padding=5.0 -> unarchive_channel_submit
                      active bg=transparent text=muted r=7.0
                      hovered bg=white/10 text=fg
                      pressed bg=white/15
                  if active_channel_huddle_count > 0
                    button "Leave huddle" disabled=(mutation_phase != "idle") height=28.0 padding=5.0 -> leave_huddle_submit
                      active bg=transparent text=muted r=7.0
                      hovered bg=white/10 text=fg
                      pressed bg=white/15
                  space width=fill
                  text len(channel_members) size=11.0 @text-muted
                row width=fill spacing=5.0 align=center
                  input "" #member-key label="Member public key" <-> member_key_draft hint="64-character member key" disabled=(mutation_phase != "idle") submit=add_channel_member_submit width=fill padding=7.4 text-size=11.0 line-height=1.2 font=mono
                    active bg=transparent border=white/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                    hovered bg=white/4 border=white/14
                    focused bg=white/7 border=white/12
                    disabled value=muted
                  button "Add" disabled=(mutation_phase != "idle" || empty(trim(member_key_draft))) width=40.0 height=28.0 padding=5.0 -> add_channel_member_submit
                    active bg=white/9 text=fg border=white/11 border-w=1.0 r=7.0
                    hovered bg=white/14
                    pressed bg=white/18
                if !empty(channel_members)
                  scroll direction=vertical width=fill height=fill
                    col width=fill spacing=2.0
                      for member in channel_members
                        ChatMemberRow member=member disabled=(mutation_phase != "idle")
          if active_thread_seq > 0 && !channel_settings_open
            container width=1.0 height=fill bg=white/8
              text ""
            container width=300.0 height=fill padding=12.0 bg=surface
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
                    hovered bg=white/11 text=fg
                    pressed bg=selection
                container width=fill height=1.0 bg=separator
                  text ""
                scroll direction=vertical width=fill height=fill
                  col width=fill spacing=1.0
                    for thread_message in thread_messages
                      ThreadMessageCard message=thread_message selected=(thread_message.seq == thread_target_seq)
                    if thread_has_more && thread_next_reply_offset >= 0
                      button "Load more replies" disabled=(thread_loading || mutation_phase != "idle") width=fill height=28.0 padding=5.0 -> load_more_thread
                        active bg=transparent text=muted r=7.0
                        hovered bg=white/9 text=fg
                        pressed bg=selection
                if !empty(failed_reply_draft)
                  row width=fill spacing=6.0 align=center
                    text "Unsent reply" width=fill size=11.0 @text-muted
                    button "Restore" disabled=(!empty(reply_draft)) height=26.0 padding=5.0 -> restore_failed_reply
                      active bg=white/9 text=fg border=white/11 border-w=1.0 r=7.0
                      hovered bg=white/14
                      pressed bg=white/18
                    button "×" label="Dismiss unsent reply" width=26.0 height=26.0 padding=4.0 -> dismiss_failed_reply
                      active bg=transparent text=muted r=7.0
                      hovered bg=white/10 text=fg
                      pressed bg=white/15
                container width=fill padding=5.0 bg=transparent border=white/12 border-w=1.0 r=7.0
                  row width=fill spacing=5.0 align=center
                    input "" #reply label="Thread reply" <-> reply_draft hint="Reply…" disabled=(thread_loading || active_channel_archived) submit=send_reply_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
                      active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=8.0
                      hovered bg=white/4 border=white/8 border-w=1.0
                      focused bg=white/8 border=white/13 border-w=1.0
                      disabled value=muted
                    button "Send" label="Send reply" disabled=(thread_loading || active_channel_archived || empty(trim(reply_draft))) height=28.0 padding=6.0 -> send_reply_submit
                      active bg=fg/88 text=bg border=white/5 border-w=1.0 r=9.0
                      hovered bg=fg/78
                      pressed bg=fg
                      disabled bg=fg/24 text=bg/12
    pages:
      mouse move=pages_pointer_moved
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
                      hovered bg=white/5 border=white/8
                      focused bg=white/7 border=white/12
                      disabled value=muted
                    if !empty(page_search_hits)
                      button label="Clear page search" width=28.0 height=28.0 padding=0.0 -> clear_page_search
                        container width=fill height=fill align-x=center align-y=center
                          text "×" size=14.0
                        active bg=transparent text=muted r=7.0
                        hovered bg=white/10 text=fg
                        pressed bg=white/15
                    if !page_delete_armed
                      button label="Page menu" disabled=(mutation_phase != "idle") width=28.0 height=28.0 padding=0.0 -> arm_page_delete
                        container width=fill height=fill align-x=center align-y=center
                          text "•••" size=13.0
                        active bg=transparent text=muted r=7.0
                        hovered bg=white/10 text=fg
                        pressed bg=white/15
                    if page_delete_armed
                      button "Delete page" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> delete_page_submit
                        active bg=white/11 text=fg border=white/13 border-w=1.0 r=7.0
                        hovered bg=white/16
                        pressed bg=white/20
                  container width=fill padding-left=56.0
                    PageTitleEditor rpc=connected_rpc password=password page_id=active_page title=active_page_title disabled=(loading || !connected || mutation_phase != "idle") #page-title(scope_key(connected_rpc, active_page))
                  if !empty(page_search_hits)
                    container width=fill height=148.0 padding=5.0 bg=elevated border=white/8 border-w=1.0 r=9.0
                      scroll direction=vertical width=fill height=fill
                        col width=fill spacing=1.0
                          for hit in page_search_hits
                            PageSearchResult hit=hit
                  if !empty(orphaned_block_drafts) || !empty(orphaned_comment_drafts)
                    container width=fill padding=7.0 bg=elevated border=white/9 border-w=1.0 r=9.0
                      col width=fill spacing=5.0
                        text "Recovered drafts" size=11.0 font=medium @text-fg
                        for recovered_block in orphaned_block_drafts
                          row width=fill spacing=5.0 align=center
                            text recovered_block width=fill size=13.0 @text-muted
                            button "Use" label="Use as block" disabled=(loading || mutation_phase != "idle" || !empty(block_draft)) height=26.0 padding=5.0 -> use_orphaned_block_draft(recovered_block)
                              active bg=white/9 text=fg border=white/12 border-w=1.0 r=7.0
                              hovered bg=white/14
                              pressed bg=white/18
                            button "Discard" disabled=(loading || mutation_phase != "idle") height=26.0 padding=5.0 -> discard_orphaned_block_draft(recovered_block)
                              active bg=transparent text=muted r=7.0
                              hovered bg=white/9 text=fg
                              pressed bg=white/14
                        for recovered_comment in orphaned_comment_drafts
                          row width=fill spacing=5.0 align=center
                            text recovered_comment width=fill size=13.0 @text-muted
                            button "Use" label="Use as block" disabled=(loading || mutation_phase != "idle" || !empty(block_draft)) height=26.0 padding=5.0 -> use_orphaned_comment_draft(recovered_comment)
                              active bg=white/9 text=fg border=white/12 border-w=1.0 r=7.0
                              hovered bg=white/14
                              pressed bg=white/18
                            button "Discard" disabled=(loading || mutation_phase != "idle") height=26.0 padding=5.0 -> discard_orphaned_comment_draft(recovered_comment)
                              active bg=transparent text=muted r=7.0
                              hovered bg=white/9 text=fg
                              pressed bg=white/14
                  if empty(blocks) && !block_insert_open
                    container width=fill padding-left=56.0
                      button "Write something…" label="Start writing" disabled=loading width=fill padding=6.0 -> open_root_block_insert
                        active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                        hovered bg=white/4 text=fg border=white/7
                        pressed bg=white/8
                  if block_insert_open && empty(block_insert_after_id)
                    InlineBlockInsert kind=new_block_kind kinds=block_kinds disabled=loading prefix="" #block-insert-row(block_insert_after_id)
                      stack width=fill
                        if new_block_kind != "Divider"
                          input "" #block-insert label="New block" <-> block_draft hint="Type and press Enter…" disabled=loading submit=add_block_submit width=fill padding=5.0 text-size=14.0 line-height=1.3
                            active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=6.0
                            hovered bg=white/2 border=white/5
                            focused bg=white/4 border=white/8
                            disabled value=muted
                        if new_block_kind == "Divider"
                          button "Insert divider" disabled=loading width=fill height=28.0 padding=5.0 -> add_block_submit
                            active bg=transparent text=muted r=6.0
                            hovered bg=white/8 text=fg
                            pressed bg=white/12
                  keyed block in blocks by=block.key
                    col width=fill spacing=1.0
                      DocumentBlock block=block selected=(block.id == selected_block_id) hovered=(block.id == hovered_block_id) disabled=loading #block(block.id)
                        col width=fill
                          if block.pending
                            container width=fill padding=5.0 bg=white/3 r=6.0
                              BlockContents block=block
                          if !block.pending && block.kind == "Page"
                            button label=block.kind description=block.text width=fill padding=5.0 -> choose_page(block.id)
                              BlockContents block=block
                              active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                              hovered bg=white/3 text=fg border=transparent
                              pressed bg=white/6 text=fg
                          if !block.pending && block.kind != "Page" && block.id != selected_block_id
                            button label=block.kind description=block.text width=fill padding=5.0 -> select_block(block.key, block.id, block.kind, block.text, block.checked, false)
                              BlockContents block=block
                              active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                              hovered bg=white/3 text=fg border=transparent
                              pressed bg=white/6 text=fg
                          if !block.pending && block.kind != "Page" && block.id == selected_block_id
                            BlockLine block=block
                              col width=fill
                                if block.kind == "Divider"
                                  container width=fill height=1.0 bg=separator
                                    text ""
                                if block.kind != "Divider"
                                  input "" #block-edit label="Edit block" <-> block_edit_draft change=block_text_changed hint="Type something…" disabled=(mutation_phase != "idle") width=fill padding=4.0 text-size=14.0 line-height=1.3
                                    active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=5.0
                                    hovered bg=white/2 border=white/5
                                    focused bg=white/3 border=white/7
                                    disabled value=muted
                      if block_insert_open && block.id == block_insert_after_id
                        InlineBlockInsert kind=new_block_kind kinds=block_kinds disabled=loading prefix=block.prefix #block-insert-row(block_insert_after_id)
                          stack width=fill
                            if new_block_kind != "Divider"
                              input "" #block-insert label="New block" <-> block_draft hint="Type and press Enter…" disabled=loading submit=add_block_submit width=fill padding=5.0 text-size=14.0 line-height=1.3
                                active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=6.0
                                hovered bg=white/2 border=white/5
                                focused bg=white/4 border=white/8
                                disabled value=muted
                            if new_block_kind == "Divider"
                              button "Insert divider" disabled=loading width=fill height=28.0 padding=5.0 -> add_block_submit
                                active bg=transparent text=muted r=6.0
                                hovered bg=white/8 text=fg
                                pressed bg=white/12
          overlay when=(connected && !empty(active_page) && block_comments_open) dismiss=close_block_comments backdrop=transparent padding=12.0 align-x=end align-y=start
            content
              space width=fill height=fill
            layer
              container width=300.0 height=380.0 padding=8.0 bg=popover border=white/15 border-w=1.0 r=11.0 shadow=black/24 shadow-y=4.0 shadow-blur=16.0
                col width=fill height=fill spacing=6.0
                  row width=fill spacing=6.0 align=center
                    text "Comments" width=fill size=13.0 font=medium @text-fg
                    if block_comment_thread_total > 0
                      text block_comment_thread_total size=11.0 @text-muted
                    if block_comment_threads_loading || block_thread_comments_loading
                      text "Loading…" size=11.0 @text-muted
                    button "×" label="Close comments" disabled=(mutation_phase != "idle") width=24.0 height=24.0 padding=4.0 -> close_block_comments
                      active bg=transparent text=muted r=6.0
                      hovered bg=white/10 text=fg
                      pressed bg=white/15
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
                            hovered bg=white/9 text=fg
                            pressed bg=white/14
                  if !empty(active_block_comment_thread)
                    row width=fill spacing=5.0 align=center
                      button "← Threads" disabled=(block_thread_comments_loading || mutation_phase != "idle") height=24.0 padding=4.0 -> close_block_comment_thread
                        active bg=transparent text=muted r=6.0
                        hovered bg=white/9 text=fg
                        pressed bg=white/14
                    scroll direction=vertical width=fill height=fill
                      col width=fill spacing=1.0
                        for page_comment in block_thread_comments
                          PageCommentCard comment=page_comment
                        if block_thread_comments_has_more
                          button "More" disabled=(block_thread_comments_loading || mutation_phase != "idle") height=24.0 padding=4.0 -> load_more_block_comments
                            active bg=transparent text=muted r=6.0
                            hovered bg=white/9 text=fg
                            pressed bg=white/14
                  row width=fill spacing=5.0 align=center
                    input "" #block-comment(scope_key(connected_rpc, selected_block_id)) label="New block comment" <-> block_comment_draft hint="Add a comment…" disabled=(mutation_phase != "idle" || block_comment_threads_loading || block_thread_comments_loading) submit=post_block_comment_submit width=fill padding=6.2 text-size=13.0 line-height=1.2
                      active bg=transparent border=white/8 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                      hovered bg=white/4 border=white/11
                      focused bg=white/6 border=white/13
                      disabled value=muted
                    button "Post" disabled=(mutation_phase != "idle" || empty(trim(block_comment_draft)) || block_comment_threads_loading || block_thread_comments_loading) height=28.0 padding=5.0 -> post_block_comment_submit
                      active bg=fg/88 text=bg border=white/5 border-w=1.0 r=8.0
                      hovered bg=fg/78 text=bg
                      pressed bg=fg text=bg
                      disabled bg=fg/25 text=bg/12
          overlay when=(connected && !empty(active_page) && !empty(selected_block_id) && block_actions_open) dismiss=close_block_actions backdrop=transparent padding=0.0 align-x=start align-y=start
            content
              space width=fill height=fill
            layer
              float x=(block_menu_x + 10.0) y=block_menu_y
                BlockActionsMenu block_id=selected_block_id kind=selected_block_kind disabled=(loading || mutation_phase != "idle") delete_armed=block_delete_armed editable_kinds=editable_block_kinds
