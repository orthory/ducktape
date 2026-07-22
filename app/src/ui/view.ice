view
  WorkspaceTabs status=status loading=(loading || mutation_phase != "idle") #workspace-tabs
    connection:
      col width=fill spacing=5.0
        text "CONNECTION" size=10.0 @font-bold text-muted
        input "" #rpc label="RPC endpoint" <-> rpc hint="Node URL" disabled=(loading || mutation_phase != "idle") submit=reconnect width=fill padding=8.0 text-size=12.0 line-height=1.2
          active bg=white/52 border=white/72 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
          hovered bg=white/62 border=white/88
          focused bg=white/72 border=fg/45 border-w=1.0
          disabled bg=white/28 value=muted
        input "" #password label="Local key password" secure=true <-> password hint="Key password" disabled=(loading || mutation_phase != "idle") width=fill padding=8.0 text-size=12.0 line-height=1.2
          active bg=white/52 border=white/72 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
          hovered bg=white/62 border=white/88
          focused bg=white/72 border=fg/45 border-w=1.0
          disabled bg=white/28 value=muted
        button "Connect" disabled=(loading || mutation_phase != "idle") width=fill height=30.0 padding=7.0 -> reconnect
          active bg=linear(2.3, fg/92@0.0, primary/96@1.0) text=white border=white/32 border-w=1.0 r=10.0 shadow=black/18 shadow-y=2.0 shadow-blur=8.0
          hovered bg=fg/82 text=white
          pressed bg=fg text=white
          disabled bg=fg/36 text=white/65
    chat_sidebar:
      col width=fill height=fill spacing=7.0
        row width=fill padding-left=7.0 padding-right=7.0 align=center
          text "CHANNELS" width=fill size=10.0 @font-bold text-muted
          text len(channels) size=10.0 @text-muted
        scroll direction=vertical width=fill height=fill bar=hidden
          col width=fill spacing=2.0
            for channel in channels
              ChannelButton channel=channel selected=(channel.id == active_channel)
        container width=fill padding=5.0 bg=white/34 border=white/55 border-w=1.0 r=12.0 shadow=black/8 shadow-y=1.0 shadow-blur=6.0
          flex width=fill gap=5.0 align-items=center
            input "" label="New channel name" <-> channel_draft hint="New channel" disabled=(loading || !connected) submit=create_channel_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
              active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=8.0
              focused bg=white/45 border=white/72 border-w=1.0
              disabled value=muted
            button "+" label="Create channel" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(channel_draft))) width=28.0 height=28.0 padding=5.0 -> create_channel_submit
              active bg=white/62 text=fg border=white/78 border-w=1.0 r=9.0 shadow=black/8 shadow-y=1.0 shadow-blur=5.0
              hovered bg=white/82
              pressed bg=selection
              disabled bg=white/24 text=muted
    pages_sidebar:
      col width=fill height=fill spacing=7.0
        row width=fill padding-left=7.0 padding-right=7.0 align=center
          text "PAGES" width=fill size=10.0 @font-bold text-muted
          text len(pages) size=10.0 @text-muted
        scroll direction=vertical width=fill height=fill bar=hidden
          col width=fill spacing=2.0
            for page in pages
              PageButton page=page selected=(page.id == active_page)
        container width=fill padding=5.0 bg=white/34 border=white/55 border-w=1.0 r=12.0 shadow=black/8 shadow-y=1.0 shadow-blur=6.0
          flex width=fill gap=5.0 align-items=center
            input "" label="New page title" <-> page_draft hint="New page" disabled=(loading || !connected) submit=create_page_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
              active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=8.0
              focused bg=white/45 border=white/72 border-w=1.0
              disabled value=muted
            button "+" label="Create page" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(page_draft))) width=28.0 height=28.0 padding=5.0 -> create_page_submit
              active bg=white/62 text=fg border=white/78 border-w=1.0 r=9.0 shadow=black/8 shadow-y=1.0 shadow-blur=5.0
              hovered bg=white/82
              pressed bg=selection
              disabled bg=white/24 text=muted
    notice:
      col width=fill
        if error != ""
          container width=fill padding-left=12.0 padding-right=12.0 padding-bottom=8.0
            container width=fill padding=8.0 bg=linear(2.3, white/78@0.0, surface/60@1.0) border=white/82 border-w=1.0 r=12.0 shadow=black/10 shadow-y=2.0 shadow-blur=10.0
              row width=fill spacing=8.0 align=center
                container width=20.0 height=20.0 align-x=center align-y=center bg=fg/82 r=10.0
                  text "!" size=10.0 @font-bold text-white
                text error width=fill size=10.0 @text-fg
                button "Dismiss" padding=5.0 style=text -> dismiss_error
    chat:
      container width=fill height=fill padding=14.0 bg=linear(2.35, white/76@0.0, elevated/64@0.5, surface/54@1.0) border=white/78 border-w=1.0 r=16.0 shadow=black/12 shadow-y=4.0 shadow-blur=18.0 clip=true px-snap=true
        row width=fill height=fill spacing=10.0
          col width=fill height=fill spacing=9.0
            if !empty(active_channel)
              row width=fill height=26.0 spacing=7.0 align=center
                container width=22.0 height=22.0 align-x=center align-y=center bg=white/52 border=white/72 border-w=1.0 r=7.0
                  text "#" size=11.0 @font-bold text-fg
                text active_channel_name width=fill size=12.0 @font-bold text-fg
                text len(messages) size=10.0 @text-muted
            if empty(messages)
              EmptyState title="No messages yet" detail="Create a channel or start the conversation."
            if !empty(messages)
              scroll direction=vertical width=fill height=fill bar=hidden
                col width=fill spacing=1.0
                  for message in messages
                    MessageCard message=message selected=(message.seq == selected_message_seq)
            if selected_message_seq > 0
              container width=fill padding=7.0 bg=linear(2.3, white/58@0.0, surface/38@1.0) border=white/62 border-w=1.0 r=12.0
                col width=fill spacing=6.0
                  row width=fill spacing=5.0 align=center
                    text "Message actions" width=fill size=10.0 @font-bold text-muted
                    button "Thread" disabled=(thread_loading || mutation_phase != "idle") height=26.0 padding=6.0 -> open_thread
                      active bg=white/48 text=fg border=white/62 border-w=1.0 r=8.0
                      hovered bg=white/72
                      pressed bg=selection
                      disabled bg=white/22 text=muted
                    button "👍" label="Add thumbs up reaction" disabled=(mutation_phase != "idle") width=28.0 height=26.0 padding=5.0 -> add_reaction_submit("👍")
                      active bg=white/48 text=fg border=white/62 border-w=1.0 r=8.0
                      hovered bg=white/72
                      pressed bg=selection
                      disabled bg=white/22 text=muted
                    button "♥" label="Add heart reaction" disabled=(mutation_phase != "idle") width=28.0 height=26.0 padding=5.0 -> add_reaction_submit("❤️")
                      active bg=white/48 text=fg border=white/62 border-w=1.0 r=8.0
                      hovered bg=white/72
                      pressed bg=selection
                      disabled bg=white/22 text=muted
                    button "Delete" disabled=(mutation_phase != "idle") height=26.0 padding=6.0 -> delete_message_submit
                      active bg=white/48 text=fg border=white/62 border-w=1.0 r=8.0
                      hovered bg=white/72
                      pressed bg=selection
                      disabled bg=white/22 text=muted
                    button "×" label="Close message actions" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=5.0 -> clear_message_selection
                      active bg=transparent text=muted r=8.0
                      hovered bg=white/56 text=fg
                      pressed bg=selection
                  row width=fill spacing=6.0 align=center
                    input "" #message-edit label="Edit message" <-> message_edit_draft hint="Edit message" disabled=(mutation_phase != "idle") submit=edit_message_submit width=fill padding=6.0 text-size=11.0 line-height=1.2
                      active bg=white/38 border=white/55 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                      focused bg=white/62 border=fg/42
                      disabled value=muted
                    button "Save" disabled=(mutation_phase != "idle" || empty(trim(message_edit_draft))) height=28.0 padding=6.0 -> edit_message_submit
                      active bg=fg/88 text=white border=white/26 border-w=1.0 r=9.0
                      hovered bg=fg/78
                      pressed bg=fg
                      disabled bg=fg/24 text=white/58
            container width=fill padding=6.0 bg=linear(2.3, white/64@0.0, surface/42@1.0) border=white/72 border-w=1.0 r=14.0 shadow=black/10 shadow-y=2.0 shadow-blur=12.0
              flex width=fill gap=6.0 align-items=center
                input "" #message label="Message" <-> message_draft hint="Write a message…" disabled=(loading || !connected || empty(active_channel)) submit=send_message_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=9.0
                  focused bg=white/38 border=white/66 border-w=1.0
                  disabled value=muted
                button "Send" disabled=(loading || mutation_phase != "idle" || !connected || empty(active_channel) || empty(trim(message_draft))) height=30.0 padding=7.0 -> send_message_submit
                  active bg=fg/90 text=white border=white/28 border-w=1.0 r=10.0 shadow=black/14 shadow-y=2.0 shadow-blur=7.0
                  hovered bg=fg/80 text=white
                  pressed bg=fg text=white
                  disabled bg=fg/28 text=white/60
          if active_thread_seq > 0
            container width=286.0 height=fill padding=10.0 bg=linear(2.35, white/62@0.0, surface/44@1.0) border=white/68 border-w=1.0 r=13.0 shadow=black/8 shadow-y=2.0 shadow-blur=12.0
              col width=fill height=fill spacing=8.0
                row width=fill height=26.0 spacing=6.0 align=center
                  text "Thread" width=fill size=12.0 @font-bold text-fg
                  text len(thread_messages) size=10.0 @text-muted
                  button "×" label="Close thread" disabled=(mutation_phase != "idle") width=26.0 height=26.0 padding=5.0 -> close_thread
                    active bg=transparent text=muted r=8.0
                    hovered bg=white/56 text=fg
                    pressed bg=selection
                container width=fill height=1.0 bg=separator
                  text ""
                scroll direction=vertical width=fill height=fill bar=hidden
                  col width=fill spacing=1.0
                    for thread_message in thread_messages
                      ThreadMessageCard message=thread_message
                container width=fill padding=5.0 bg=white/36 border=white/58 border-w=1.0 r=11.0
                  row width=fill spacing=5.0 align=center
                    input "" #reply label="Thread reply" <-> reply_draft hint="Reply…" disabled=(mutation_phase != "idle") submit=send_reply_submit width=fill padding=6.0 text-size=11.0 line-height=1.2
                      active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=8.0
                      focused bg=white/42 border=white/64 border-w=1.0
                      disabled value=muted
                    button "Send" label="Send reply" disabled=(mutation_phase != "idle" || empty(trim(reply_draft))) height=28.0 padding=6.0 -> send_reply_submit
                      active bg=fg/88 text=white border=white/26 border-w=1.0 r=9.0
                      hovered bg=fg/78
                      pressed bg=fg
                      disabled bg=fg/24 text=white/58
    pages:
      container width=fill height=fill padding=16.0 bg=linear(2.35, white/78@0.0, elevated/64@0.48, surface/52@1.0) border=white/80 border-w=1.0 r=16.0 shadow=black/12 shadow-y=4.0 shadow-blur=18.0 clip=true px-snap=true
        col width=fill height=fill
          if empty(active_page)
            EmptyState title="No page selected" detail="Create a page to begin writing."
          if !empty(active_page)
            col width=fill height=fill spacing=9.0
              row width=fill spacing=7.0 align=center
                input "" #page-title label="Page title" <-> active_page_title hint="Untitled" disabled=(loading || !connected) submit=rename_page_submit width=fill padding=7.0 text-size=17.0 line-height=1.2
                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=9.0
                  hovered bg=white/24
                  focused bg=white/42 border=white/68 border-w=1.0
                  disabled value=muted
                button "Save" label="Save title" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(active_page_title))) height=34.0 padding=7.0 -> rename_page_submit
                  active bg=white/58 text=fg border=white/76 border-w=1.0 r=10.0 shadow=black/10 shadow-y=2.0 shadow-blur=7.0
                  hovered bg=white/78
                  pressed bg=selection
                  disabled bg=white/22 text=muted
              container width=fill padding=6.0 bg=white/28 border=white/52 border-w=1.0 r=11.0
                row width=fill spacing=6.0 align=center
                  input "" #subpage label="New subpage title" <-> subpage_draft hint="New subpage" disabled=(loading || !connected) submit=create_child_page_submit width=fill padding=6.0 text-size=11.0 line-height=1.2
                    active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=8.0
                    focused bg=white/42 border=white/64 border-w=1.0
                    disabled value=muted
                  button "Add subpage" disabled=(mutation_phase != "idle" || empty(trim(subpage_draft))) height=28.0 padding=6.0 -> create_child_page_submit
                    active bg=white/48 text=fg border=white/62 border-w=1.0 r=8.0
                    hovered bg=white/70
                    pressed bg=selection
                    disabled bg=white/20 text=muted
                  if !empty(active_page_parent)
                    button "Move top" disabled=(mutation_phase != "idle") height=28.0 padding=6.0 -> move_page_top_submit
                      active bg=white/48 text=fg border=white/62 border-w=1.0 r=8.0
                      hovered bg=white/70
                      pressed bg=selection
                      disabled bg=white/20 text=muted
                  if !page_delete_armed
                    button "Delete" disabled=(mutation_phase != "idle") height=28.0 padding=6.0 -> arm_page_delete
                      active bg=transparent text=muted border=white/48 border-w=1.0 r=8.0
                      hovered bg=white/58 text=fg
                      pressed bg=selection
                  if page_delete_armed
                    button "Confirm delete" disabled=(mutation_phase != "idle") height=28.0 padding=6.0 -> delete_page_submit
                      active bg=fg/86 text=white border=white/24 border-w=1.0 r=8.0
                      hovered bg=fg/76
                      pressed bg=fg
              container width=fill height=1.0 bg=separator
                text ""
              if empty(blocks)
                EmptyState title="An empty page" detail="Add the first block below."
              if !empty(blocks)
                scroll direction=vertical width=fill height=fill bar=hidden
                  col width=fill spacing=1.0
                    for block in blocks
                      BlockCard block=block selected=(block.id == selected_block_id)
              if !empty(selected_block_id)
                container width=fill padding=7.0 bg=linear(2.3, white/58@0.0, surface/38@1.0) border=white/62 border-w=1.0 r=12.0
                  col width=fill spacing=6.0
                    row width=fill spacing=5.0 align=center
                      pick block_kinds some(selected_block_kind) placeholder="Block type" width=124.0 menu-height=210.0 padding=6.0 text-size=11.0 line-height=1.2 -> selected_block_kind_changed _
                        active text=fg placeholder=muted handle=muted bg=white/42 border=white/58 border-w=1.0 r=8.0
                        hovered text=fg placeholder=muted handle=fg bg=white/58 border=white/72 border-w=1.0 r=8.0
                        opened text=fg placeholder=muted handle=fg bg=white/66 border=white/76 border-w=1.0 r=8.0
                        menu text=fg selected-text=fg selected-bg=white/78 bg=surface border=white/72 border-w=1.0 r=8.0 shadow=black/16 shadow-y=3.0 shadow-blur=10.0
                      button "↑" label="Move block up" disabled=(mutation_phase != "idle") width=28.0 height=27.0 padding=5.0 -> move_block_submit("up")
                        active bg=white/44 text=fg border=white/58 border-w=1.0 r=8.0
                        hovered bg=white/68
                        pressed bg=selection
                      button "↓" label="Move block down" disabled=(mutation_phase != "idle") width=28.0 height=27.0 padding=5.0 -> move_block_submit("down")
                        active bg=white/44 text=fg border=white/58 border-w=1.0 r=8.0
                        hovered bg=white/68
                        pressed bg=selection
                      button "→" label="Indent block" disabled=(mutation_phase != "idle") width=28.0 height=27.0 padding=5.0 -> move_block_submit("indent")
                        active bg=white/44 text=fg border=white/58 border-w=1.0 r=8.0
                        hovered bg=white/68
                        pressed bg=selection
                      button "←" label="Outdent block" disabled=(mutation_phase != "idle") width=28.0 height=27.0 padding=5.0 -> move_block_submit("outdent")
                        active bg=white/44 text=fg border=white/58 border-w=1.0 r=8.0
                        hovered bg=white/68
                        pressed bg=selection
                      if selected_block_kind == "Todo"
                        button "Check" disabled=(mutation_phase != "idle") height=27.0 padding=5.0 -> toggle_block_checked
                          active bg=white/44 text=fg border=white/58 border-w=1.0 r=8.0
                          hovered bg=white/68
                          pressed bg=selection
                      space width=fill
                      if !block_delete_armed
                        button "Delete" disabled=(mutation_phase != "idle") height=27.0 padding=5.0 -> arm_block_delete
                          active bg=transparent text=muted border=white/46 border-w=1.0 r=8.0
                          hovered bg=white/58 text=fg
                          pressed bg=selection
                      if block_delete_armed
                        button "Confirm" disabled=(mutation_phase != "idle") height=27.0 padding=5.0 -> remove_block_submit
                          active bg=fg/86 text=white border=white/24 border-w=1.0 r=8.0
                          hovered bg=fg/76
                          pressed bg=fg
                      button "×" label="Close block editor" disabled=(mutation_phase != "idle") width=27.0 height=27.0 padding=5.0 -> clear_block_selection
                        active bg=transparent text=muted r=8.0
                        hovered bg=white/56 text=fg
                        pressed bg=selection
                    row width=fill spacing=6.0 align=center
                      input "" #block-edit label="Edit block" <-> block_edit_draft hint="Block text" disabled=(mutation_phase != "idle" || selected_block_kind == "Divider") submit=save_block_submit width=fill padding=6.0 text-size=11.0 line-height=1.2
                        active bg=white/38 border=white/55 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                        focused bg=white/62 border=fg/42
                        disabled bg=white/20 value=muted
                      button "Save" disabled=(mutation_phase != "idle" || (selected_block_kind != "Divider" && empty(trim(block_edit_draft)))) height=28.0 padding=6.0 -> save_block_submit
                        active bg=fg/88 text=white border=white/26 border-w=1.0 r=9.0
                        hovered bg=fg/78
                        pressed bg=fg
                        disabled bg=fg/24 text=white/58
              container width=fill padding=6.0 bg=linear(2.3, white/64@0.0, surface/42@1.0) border=white/72 border-w=1.0 r=14.0 shadow=black/10 shadow-y=2.0 shadow-blur=12.0
                row width=fill spacing=6.0 align=center
                  pick block_kinds some(new_block_kind) placeholder="Block type" width=124.0 menu-height=210.0 padding=7.0 text-size=11.0 line-height=1.2 -> new_block_kind_changed _
                    active text=fg placeholder=muted handle=muted bg=transparent border=transparent border-w=0.0 r=8.0
                    hovered text=fg placeholder=muted handle=fg bg=white/38 border=white/55 border-w=1.0 r=8.0
                    opened text=fg placeholder=muted handle=fg bg=white/52 border=white/68 border-w=1.0 r=8.0
                    menu text=fg selected-text=fg selected-bg=white/78 bg=surface border=white/72 border-w=1.0 r=8.0 shadow=black/16 shadow-y=3.0 shadow-blur=10.0
                  input "" #block label="New block" <-> block_draft hint="Add a block…" disabled=(loading || !connected || new_block_kind == "Divider") submit=add_block_submit width=fill padding=7.0 text-size=12.0 line-height=1.2
                    active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=9.0
                    focused bg=white/38 border=white/66 border-w=1.0
                    disabled bg=white/16 value=muted
                  button "Add" disabled=(loading || mutation_phase != "idle" || !connected || (new_block_kind != "Divider" && empty(trim(block_draft)))) height=30.0 padding=7.0 -> add_block_submit
                    active bg=fg/90 text=white border=white/28 border-w=1.0 r=10.0 shadow=black/14 shadow-y=2.0 shadow-blur=7.0
                    hovered bg=fg/80 text=white
                    pressed bg=fg text=white
                    disabled bg=fg/28 text=white/60

