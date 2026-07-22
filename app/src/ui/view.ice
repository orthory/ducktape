view
  WorkspaceTabs status=status loading=(loading || mutation_phase != "idle") #workspace-tabs
    connection:
      col width=fill spacing=5.0
        text "CONNECTION" size=10.0 @font-bold text-muted
        input "" #rpc label="RPC endpoint" <-> rpc hint="Node URL" disabled=(loading || mutation_phase != "idle") submit=reconnect width=fill padding=8.0 text-size=12.0 line-height=1.2
          active bg=surface border=white/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
          hovered bg=elevated border=white/21
          focused bg=elevated border=fg/45 border-w=1.0
          disabled bg=surface/54 value=muted
        input "" #password label="Local key password" secure=true <-> password hint="Key password" disabled=(loading || mutation_phase != "idle") width=fill padding=8.0 text-size=12.0 line-height=1.2
          active bg=surface border=white/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
          hovered bg=elevated border=white/21
          focused bg=elevated border=fg/45 border-w=1.0
          disabled bg=surface/54 value=muted
        button "Connect" disabled=(loading || mutation_phase != "idle") width=fill height=30.0 padding=7.0 -> reconnect
          active bg=linear(2.3, fg/92@0.0, primary/96@1.0) text=bg border=white/6 border-w=1.0 r=10.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
          hovered bg=fg/82 text=bg
          pressed bg=fg text=bg
          disabled bg=fg/36 text=bg/14
    chat_sidebar:
      col width=fill height=fill spacing=7.0
        row width=fill padding-left=7.0 padding-right=7.0 align=center
          text "CHANNELS" width=fill size=10.0 @font-bold text-muted
          text len(channels) size=10.0 @text-muted
        scroll direction=vertical width=fill height=fill bar=hidden
          col width=fill spacing=2.0
            for channel in channels
              ChannelButton channel=channel selected=(channel.id == active_channel)
        container width=fill padding=5.0 bg=surface/90 border=white/11 border-w=1.0 r=12.0 shadow=black/8 shadow-y=1.0 shadow-blur=6.0
          flex width=fill gap=5.0 align-items=center
            input "" label="New channel name" <-> channel_draft hint="New channel" disabled=(loading || !connected) submit=create_channel_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
              active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=8.0
              focused bg=white/9 border=white/16 border-w=1.0
              disabled value=muted
            button "+" label="Create channel" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(channel_draft))) width=28.0 height=28.0 padding=5.0 -> create_channel_submit
              active bg=white/13 text=fg border=white/18 border-w=1.0 r=9.0 shadow=black/8 shadow-y=1.0 shadow-blur=5.0
              hovered bg=white/19
              pressed bg=selection
              disabled bg=white/5 text=muted
    pages_sidebar:
      col width=fill height=fill spacing=7.0
        row width=fill padding-left=7.0 padding-right=7.0 align=center
          text "PAGES" width=fill size=10.0 @font-bold text-muted
          text len(pages) size=10.0 @text-muted
        scroll direction=vertical width=fill height=fill bar=hidden
          col width=fill spacing=2.0
            for page in pages
              PageButton page=page selected=(page.id == active_page)
        container width=fill padding=5.0 bg=surface/90 border=white/11 border-w=1.0 r=12.0 shadow=black/8 shadow-y=1.0 shadow-blur=6.0
          flex width=fill gap=5.0 align-items=center
            input "" label="New page title" <-> page_draft hint="New page" disabled=(loading || !connected) submit=create_page_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
              active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=8.0
              focused bg=white/9 border=white/16 border-w=1.0
              disabled value=muted
            button "+" label="Create page" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(page_draft))) width=28.0 height=28.0 padding=5.0 -> create_page_submit
              active bg=white/13 text=fg border=white/18 border-w=1.0 r=9.0 shadow=black/8 shadow-y=1.0 shadow-blur=5.0
              hovered bg=white/19
              pressed bg=selection
              disabled bg=white/5 text=muted
    notice:
      col width=fill
        if error != ""
          container width=fill padding-left=12.0 padding-right=12.0 padding-bottom=8.0
            container width=fill padding=8.0 bg=linear(2.3, white/18@0.0, surface/60@1.0) border=white/19 border-w=1.0 r=12.0 shadow=black/10 shadow-y=2.0 shadow-blur=10.0
              row width=fill spacing=8.0 align=center
                container width=20.0 height=20.0 align-x=center align-y=center bg=fg/82 r=10.0
                  text "!" size=10.0 @font-bold text-bg
                text error width=fill size=10.0 @text-fg
                button "Dismiss" padding=5.0 style=text -> dismiss_error
    chat:
      container width=fill height=fill padding=14.0 bg=linear(2.35, elevated/82@0.0, surface/94@0.5, bg/90@1.0) border=white/18 border-w=1.0 r=16.0 shadow=black/12 shadow-y=4.0 shadow-blur=18.0 clip=true px-snap=true
        row width=fill height=fill spacing=10.0
          col width=fill height=fill spacing=9.0
            if !empty(active_channel)
              row width=fill height=26.0 spacing=7.0 align=center
                container width=22.0 height=22.0 align-x=center align-y=center bg=white/10 border=white/16 border-w=1.0 r=7.0
                  text "#" size=11.0 @font-bold text-fg
                text active_channel_name width=fill size=12.0 @font-bold text-fg
                if active_channel_archived
                  text "Archived" size=9.0 @text-muted
                if active_channel_members_only
                  text "Members" size=9.0 @text-muted
                if active_channel_huddle_count > 0
                  text active_channel_huddle_count size=10.0 @text-muted
                text len(messages) size=10.0 @text-muted
                button "•••" label="Channel details" width=28.0 height=26.0 padding=4.0 -> toggle_channel_settings
                  active bg=transparent text=muted r=7.0
                  hovered bg=white/10 text=fg
                  pressed bg=white/15
            if channel_settings_open && !empty(active_channel)
              container width=fill padding=7.0 bg=white/6 border=white/10 border-w=1.0 r=10.0
                col width=fill spacing=6.0
                  row width=fill spacing=5.0 align=center
                    input "" #channel-name label="Channel name" <-> channel_name_draft hint="Channel name" disabled=(mutation_phase != "idle") submit=rename_channel_submit width=fill padding=6.0 text-size=11.0 line-height=1.2
                      active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                      focused bg=white/7 border=white/12
                      disabled value=muted
                    button "Rename" disabled=(mutation_phase != "idle" || empty(trim(channel_name_draft))) height=27.0 padding=5.0 -> rename_channel_submit
                      active bg=white/9 text=fg border=white/11 border-w=1.0 r=7.0
                      hovered bg=white/14
                      pressed bg=white/18
                  row width=fill spacing=5.0 align=center
                    if !active_channel_archived
                      button "Archive" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> archive_channel_submit
                        active bg=transparent text=muted r=7.0
                        hovered bg=white/10 text=fg
                        pressed bg=white/15
                      button "Join huddle" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> join_huddle_submit
                        active bg=transparent text=muted r=7.0
                        hovered bg=white/10 text=fg
                        pressed bg=white/15
                    if active_channel_archived
                      button "Unarchive" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> unarchive_channel_submit
                        active bg=transparent text=muted r=7.0
                        hovered bg=white/10 text=fg
                        pressed bg=white/15
                    if active_channel_huddle_count > 0
                      button "Leave huddle" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> leave_huddle_submit
                        active bg=transparent text=muted r=7.0
                        hovered bg=white/10 text=fg
                        pressed bg=white/15
                    space width=fill
                    text len(channel_members) size=9.0 @text-muted
                  row width=fill spacing=5.0 align=center
                    input "" #member-key label="Member public key" <-> member_key_draft hint="64-character member key" disabled=(mutation_phase != "idle") submit=add_channel_member_submit width=fill padding=6.0 text-size=10.0 line-height=1.2 font=mono
                      active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                      focused bg=white/7 border=white/12
                      disabled value=muted
                    button "Add" disabled=(mutation_phase != "idle" || empty(trim(member_key_draft))) height=27.0 padding=5.0 -> add_channel_member_submit
                      active bg=white/9 text=fg border=white/11 border-w=1.0 r=7.0
                      hovered bg=white/14
                      pressed bg=white/18
                  if !empty(channel_members)
                    col width=fill spacing=2.0
                      for member in channel_members
                        ChatMemberRow member=member disabled=(mutation_phase != "idle")
            row width=fill spacing=6.0 align=center
              input "" #chat-search label="Search messages" <-> chat_search_draft hint="Search workspace messages" disabled=(!connected || chat_searching) submit=search_chat_submit width=fill padding=6.0 text-size=11.0 line-height=1.2
                active bg=surface border=white/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                focused bg=elevated border=fg/40
                disabled bg=surface/54 value=muted
              button "Search" disabled=(!connected || chat_searching || empty(trim(chat_search_draft))) height=28.0 padding=6.0 -> search_chat_submit
                active bg=white/9 text=fg border=white/13 border-w=1.0 r=8.0
                hovered bg=white/15
                pressed bg=selection
                disabled bg=white/4 text=muted
              if !empty(chat_search_hits)
                button "×" label="Clear message search" width=28.0 height=28.0 padding=5.0 -> clear_chat_search
                  active bg=transparent text=muted r=8.0
                  hovered bg=white/10 text=fg
                  pressed bg=selection
            if !empty(chat_search_hits)
              container width=fill height=148.0 padding=6.0 bg=white/6 border=white/10 border-w=1.0 r=10.0
                scroll direction=vertical width=fill height=fill bar=hidden
                  col width=fill spacing=1.0
                    for hit in chat_search_hits
                      ChatSearchResult hit=hit
            if empty(messages)
              EmptyState title="No messages yet" detail="Create a channel or start the conversation."
            if !empty(messages)
              scroll direction=vertical width=fill height=fill bar=hidden
                col width=fill spacing=1.0
                  for message in messages
                    col width=fill spacing=2.0
                      MessageCard message=message selected=(message.seq == selected_message_seq)
                      if message.seq == selected_message_seq
                        container width=fill padding=6.0 bg=white/6 border=white/10 border-w=1.0 r=10.0
                          col width=fill spacing=5.0
                            row width=fill spacing=4.0 align=center
                              button "Thread" disabled=(thread_loading || mutation_phase != "idle") height=26.0 padding=5.0 -> open_thread
                                active bg=transparent text=muted r=7.0
                                hovered bg=white/11 text=fg
                                pressed bg=white/16
                              button "👍" label="Add thumbs up reaction" disabled=(mutation_phase != "idle" || active_channel_archived) width=26.0 height=26.0 padding=4.0 -> add_reaction_submit("👍")
                                active bg=transparent text=muted r=7.0
                                hovered bg=white/11 text=fg
                                pressed bg=white/16
                              button "♥" label="Add heart reaction" disabled=(mutation_phase != "idle" || active_channel_archived) width=26.0 height=26.0 padding=4.0 -> add_reaction_submit("❤️")
                                active bg=transparent text=muted r=7.0
                                hovered bg=white/11 text=fg
                                pressed bg=white/16
                              space width=fill
                              button "Delete" disabled=(mutation_phase != "idle" || active_channel_archived) height=26.0 padding=5.0 -> delete_message_submit
                                active bg=transparent text=muted r=7.0
                                hovered bg=white/11 text=fg
                                pressed bg=white/16
                              button "×" label="Close message actions" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=4.0 -> clear_message_selection
                                active bg=transparent text=muted r=7.0
                                hovered bg=white/11 text=fg
                                pressed bg=white/16
                            row width=fill spacing=5.0 align=center
                              input "" #message-edit label="Edit message" <-> message_edit_draft hint="Edit message" disabled=(mutation_phase != "idle" || active_channel_archived) submit=edit_message_submit width=fill padding=6.0 text-size=12.0 line-height=1.2
                                active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                focused bg=white/7 border=white/12
                                disabled value=muted
                              button "Save changes" disabled=(mutation_phase != "idle" || active_channel_archived || empty(trim(message_edit_draft))) height=28.0 padding=6.0 -> edit_message_submit
                                active bg=white/11 text=fg border=white/13 border-w=1.0 r=8.0
                                hovered bg=white/16
                                pressed bg=white/20
                                disabled bg=white/4 text=muted
            container width=fill padding=6.0 bg=linear(2.3, elevated/82@0.0, surface/90@1.0) border=white/16 border-w=1.0 r=14.0 shadow=black/10 shadow-y=2.0 shadow-blur=12.0
              flex width=fill gap=6.0 align-items=center
                input "" #message label="Message" <-> message_draft hint="Write a message…" disabled=(loading || !connected || empty(active_channel) || active_channel_archived) submit=send_message_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=9.0
                  focused bg=white/7 border=white/14 border-w=1.0
                  disabled value=muted
                button "Send" disabled=(loading || mutation_phase != "idle" || !connected || empty(active_channel) || active_channel_archived || empty(trim(message_draft))) height=30.0 padding=7.0 -> send_message_submit
                  active bg=fg/90 text=bg border=white/5 border-w=1.0 r=10.0 shadow=black/14 shadow-y=2.0 shadow-blur=7.0
                  hovered bg=fg/80 text=bg
                  pressed bg=fg text=bg
                  disabled bg=fg/28 text=bg/12
          if active_thread_seq > 0
            container width=286.0 height=fill padding=10.0 bg=linear(2.35, elevated/84@0.0, surface/92@1.0) border=white/15 border-w=1.0 r=13.0 shadow=black/8 shadow-y=2.0 shadow-blur=12.0
              col width=fill height=fill spacing=8.0
                row width=fill height=26.0 spacing=6.0 align=center
                  text "Thread" width=fill size=12.0 @font-bold text-fg
                  text len(thread_messages) size=10.0 @text-muted
                  button "×" label="Close thread" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=5.0 -> close_thread
                    active bg=transparent text=muted r=8.0
                    hovered bg=white/11 text=fg
                    pressed bg=selection
                container width=fill height=1.0 bg=separator
                  text ""
                scroll direction=vertical width=fill height=fill bar=hidden
                  col width=fill spacing=1.0
                    for thread_message in thread_messages
                      ThreadMessageCard message=thread_message
                container width=fill padding=5.0 bg=white/7 border=white/12 border-w=1.0 r=11.0
                  row width=fill spacing=5.0 align=center
                    input "" #reply label="Thread reply" <-> reply_draft hint="Reply…" disabled=(mutation_phase != "idle" || active_channel_archived) submit=send_reply_submit width=fill padding=6.0 text-size=11.0 line-height=1.2
                      active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=8.0
                      focused bg=white/8 border=white/13 border-w=1.0
                      disabled value=muted
                    button "Send" label="Send reply" disabled=(mutation_phase != "idle" || active_channel_archived || empty(trim(reply_draft))) height=28.0 padding=6.0 -> send_reply_submit
                      active bg=fg/88 text=bg border=white/5 border-w=1.0 r=9.0
                      hovered bg=fg/78
                      pressed bg=fg
                      disabled bg=fg/24 text=bg/12
    pages:
      container width=fill height=fill padding=16.0 bg=linear(2.35, elevated/82@0.0, surface/94@0.48, bg/90@1.0) border=white/18 border-w=1.0 r=16.0 shadow=black/12 shadow-y=4.0 shadow-blur=18.0 clip=true px-snap=true
        col width=fill height=fill
          if empty(active_page)
            EmptyState title="No page selected" detail="Create a page to begin writing."
          if !empty(active_page)
            col width=fill height=fill spacing=9.0
              row width=fill spacing=7.0 align=center
                PageTitleEditor rpc=connected_rpc password=password page_id=active_page title=active_page_title disabled=(loading || !connected) #page-title
                if !page_delete_armed
                  button "•••" label="Page menu" disabled=(mutation_phase != "idle") width=30.0 height=28.0 padding=5.0 -> arm_page_delete
                    active bg=transparent text=muted r=8.0
                    hovered bg=white/10 text=fg
                    pressed bg=white/14 text=fg
                if page_delete_armed
                  button "Delete page" disabled=(mutation_phase != "idle") height=28.0 padding=6.0 -> delete_page_submit
                    active bg=white/11 text=fg border=white/13 border-w=1.0 r=8.0
                    hovered bg=white/16
                    pressed bg=white/20
              row width=fill spacing=6.0 align=center
                input "" #page-search label="Search pages" <-> page_search_draft hint="Search workspace pages" disabled=(!connected || page_searching) submit=search_pages_submit width=fill padding=6.0 text-size=11.0 line-height=1.2
                  active bg=surface border=white/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                  focused bg=elevated border=fg/40
                  disabled bg=surface/54 value=muted
                button "Search" disabled=(!connected || page_searching || empty(trim(page_search_draft))) height=28.0 padding=6.0 -> search_pages_submit
                  active bg=white/9 text=fg border=white/13 border-w=1.0 r=8.0
                  hovered bg=white/15
                  pressed bg=selection
                  disabled bg=white/4 text=muted
                if !empty(page_search_hits)
                  button "×" label="Clear page search" width=28.0 height=28.0 padding=5.0 -> clear_page_search
                    active bg=transparent text=muted r=8.0
                    hovered bg=white/10 text=fg
                    pressed bg=selection
              if !empty(page_search_hits)
                container width=fill height=148.0 padding=6.0 bg=white/6 border=white/10 border-w=1.0 r=10.0
                  scroll direction=vertical width=fill height=fill bar=hidden
                    col width=fill spacing=1.0
                      for hit in page_search_hits
                        PageSearchResult hit=hit
              container width=fill height=1.0 bg=separator
                text ""
              if empty(blocks)
                EmptyState title="An empty page" detail="Add the first block below."
              if !empty(blocks)
                scroll direction=vertical width=fill height=fill bar=hidden
                  col width=fill spacing=1.0
                    for block in blocks
                      col width=fill spacing=2.0
                        BlockCard block=block selected=(block.id == selected_block_id)
                        if block.id == selected_block_id
                          container width=fill padding=6.0 bg=white/6 border=white/10 border-w=1.0 r=10.0
                            col width=fill spacing=5.0
                              row width=fill spacing=4.0 align=center
                                if selected_block_kind != "Page"
                                  pick block_kinds some(selected_block_kind) placeholder="Block type" width=116.0 menu-height=210.0 padding=5.0 text-size=10.0 line-height=1.2 -> selected_block_kind_changed _
                                    active text=fg placeholder=muted handle=muted bg=white/7 border=white/10 border-w=1.0 r=7.0
                                    hovered text=fg placeholder=muted handle=fg bg=white/11 border=white/14 border-w=1.0 r=7.0
                                    opened text=fg placeholder=muted handle=fg bg=white/13 border=white/16 border-w=1.0 r=7.0
                                    menu text=fg selected-text=fg selected-bg=white/18 bg=surface border=white/16 border-w=1.0 r=8.0 shadow=black/16 shadow-y=3.0 shadow-blur=10.0
                                if selected_block_kind == "Page"
                                  button "Open" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> choose_page(selected_block_id)
                                    active bg=white/8 text=fg border=white/11 border-w=1.0 r=7.0
                                    hovered bg=white/14
                                    pressed bg=white/18
                                button "↑" label="Move block up" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=4.0 -> move_block_submit("up")
                                  active bg=transparent text=muted r=7.0
                                  hovered bg=white/11 text=fg
                                  pressed bg=white/16
                                button "↓" label="Move block down" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=4.0 -> move_block_submit("down")
                                  active bg=transparent text=muted r=7.0
                                  hovered bg=white/11 text=fg
                                  pressed bg=white/16
                                button "→" label="Indent block" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=4.0 -> move_block_submit("indent")
                                  active bg=transparent text=muted r=7.0
                                  hovered bg=white/11 text=fg
                                  pressed bg=white/16
                                button "←" label="Outdent block" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=4.0 -> move_block_submit("outdent")
                                  active bg=transparent text=muted r=7.0
                                  hovered bg=white/11 text=fg
                                  pressed bg=white/16
                                if selected_block_kind == "Todo"
                                  button "Check" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> toggle_block_checked
                                    active bg=transparent text=muted r=7.0
                                    hovered bg=white/11 text=fg
                                    pressed bg=white/16
                                space width=fill
                                if !block_delete_armed
                                  button "Delete" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> arm_block_delete
                                    active bg=transparent text=muted r=7.0
                                    hovered bg=white/11 text=fg
                                    pressed bg=white/16
                                if block_delete_armed
                                  button "Confirm" disabled=(mutation_phase != "idle") height=26.0 padding=5.0 -> remove_block_submit
                                    active bg=white/13 text=fg border=white/14 border-w=1.0 r=7.0
                                    hovered bg=white/18
                                    pressed bg=white/22
                                button "×" label="Close block editor" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=4.0 -> clear_block_selection
                                  active bg=transparent text=muted r=7.0
                                  hovered bg=white/11 text=fg
                                  pressed bg=white/16
                              if selected_block_kind != "Divider"
                                row width=fill spacing=6.0 align=center
                                  input "" #block-edit label="Edit block" <-> block_edit_draft change=block_text_changed hint="Block text" disabled=(mutation_phase != "idle") width=fill padding=6.0 text-size=12.0 line-height=1.2
                                    active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                    focused bg=white/7 border=white/12
                                    disabled value=muted
                                  if block_autosave_status == "saving"
                                    text "Saving…" size=9.0 @text-muted
                                  if block_autosave_status == "saved"
                                    text "Saved" size=9.0 @text-muted
              container width=fill padding=6.0 bg=linear(2.3, elevated/82@0.0, surface/90@1.0) border=white/16 border-w=1.0 r=14.0 shadow=black/10 shadow-y=2.0 shadow-blur=12.0
                row width=fill spacing=6.0 align=center
                  pick block_kinds some(new_block_kind) placeholder="Block type" width=124.0 menu-height=210.0 padding=7.0 text-size=11.0 line-height=1.2 -> new_block_kind_changed _
                    active text=fg placeholder=muted handle=muted bg=transparent border=transparent border-w=0.0 r=8.0
                    hovered text=fg placeholder=muted handle=fg bg=white/7 border=white/11 border-w=1.0 r=8.0
                    opened text=fg placeholder=muted handle=fg bg=white/10 border=white/15 border-w=1.0 r=8.0
                    menu text=fg selected-text=fg selected-bg=white/18 bg=surface border=white/16 border-w=1.0 r=8.0 shadow=black/16 shadow-y=3.0 shadow-blur=10.0
                  input "" #block label="New block" <-> block_draft hint="Add a block…" disabled=(loading || !connected || new_block_kind == "Divider") submit=add_block_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
                    active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=9.0
                    focused bg=white/7 border=white/14 border-w=1.0
                    disabled bg=white/3 value=muted
                  button "Add" disabled=(loading || mutation_phase != "idle" || !connected || (new_block_kind != "Divider" && empty(trim(block_draft)))) height=30.0 padding=7.0 -> add_block_submit
                    active bg=fg/90 text=bg border=white/5 border-w=1.0 r=10.0 shadow=black/14 shadow-y=2.0 shadow-blur=7.0
                    hovered bg=fg/80 text=bg
                    pressed bg=fg text=bg
                    disabled bg=fg/28 text=bg/12
