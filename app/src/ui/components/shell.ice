component Brand()
  row width=fill spacing=10.0 align=center
    container width=30.0 height=30.0 align-x=center align-y=center bg=primary border=primaryhi/45 border-w=1.0 r=9.0 shadow=black/28 shadow-y=1.0 shadow-blur=6.0
      text "D" size=15.0 font=display @text-fg
    col width=fill spacing=1.0
      text "Ducktape" size=14.0 wrapping=none font=display @text-fg
      text "Workspace" size=11.0 wrapping=none @text-muted

component EmptyState(title:str, detail:str)
  container width=fill height=fill align-x=center align-y=center
    col spacing=6.0 align=center
      text title size=14.0 font=medium @text-fg
      text detail size=13.0 @text-muted

component WorkspaceTabs(status:str, loading:bool)
  state
    tab = "chat"
    connection_open = false
    sidebar_width = 240.0
  on select_tab(next)
    tab = next
  on toggle_connection
    connection_open = !connection_open
  on sidebar_dragged(dx, dy)
    return if dx < 0.0 && sidebar_width + dx < 180.0
    return if dx > 0.0 && sidebar_width + dx > 460.0
    sidebar_width = sidebar_width + dx
  container width=fill height=fill clip=true bg=bg border=white/6 border-w=1.0 px-snap=true
    row width=fill height=fill
      container width=sidebar_width height=fill padding=12.0 padding-top=38.0 bg=sidebar clip=true
        col width=fill height=fill spacing=8.0
          Brand
          space height=6.0
          container width=fill padding-left=8.0
            text "APPS" size=11.0 font=medium @text-muted
          match tab
            "chat"
              col width=fill spacing=3.0
                button label="Chat" width=fill height=34.0 padding=7.0 -> select_tab("chat")
                  row width=fill height=fill spacing=9.0 align=center
                    text "#" width=18.0 size=15.0 align-x=center font=display @text-primary
                    text "Chat" width=fill size=14.0 font=medium @text-fg
                  active bg=primary/16 text=fg border=primary/26 border-w=1.0 r=10.0
                  hovered bg=primary/22 text=fg border=primary/34
                  pressed bg=primary/30 text=fg
                button label="Pages" width=fill height=34.0 padding=7.0 -> select_tab("pages")
                  row width=fill height=fill spacing=9.0 align=center
                    text "▤" width=18.0 size=14.0 align-x=center @text-muted
                    text "Pages" width=fill size=14.0 @text-muted
                  active bg=transparent text=muted border=transparent border-w=1.0 r=10.0
                  hovered bg=white/6 text=fg border=white/8
                  pressed bg=white/10 text=fg
            _
              col width=fill spacing=3.0
                button label="Chat" width=fill height=34.0 padding=7.0 -> select_tab("chat")
                  row width=fill height=fill spacing=9.0 align=center
                    text "#" width=18.0 size=15.0 align-x=center @text-muted
                    text "Chat" width=fill size=14.0 @text-muted
                  active bg=transparent text=muted border=transparent border-w=1.0 r=10.0
                  hovered bg=white/6 text=fg border=white/8
                  pressed bg=white/10 text=fg
                button label="Pages" width=fill height=34.0 padding=7.0 -> select_tab("pages")
                  row width=fill height=fill spacing=9.0 align=center
                    text "▤" width=18.0 size=15.0 align-x=center font=display @text-primary
                    text "Pages" width=fill size=14.0 font=medium @text-fg
                  active bg=primary/16 text=fg border=primary/26 border-w=1.0 r=10.0
                  hovered bg=primary/22 text=fg border=primary/34
                  pressed bg=primary/30 text=fg
          container width=fill height=1.0 bg=white/6
            text ""
          match tab
            "chat"
              slot chat_sidebar
            _
              slot pages_sidebar
          button label="Connection" width=fill height=28.0 padding=7.0 -> toggle_connection
            row width=fill height=fill spacing=7.0 align=center
              container width=7.0 height=7.0 bg=fg/48 border=white/16 border-w=1.0 r=3.5
                text ""
              text "Connection" size=11.0 font=medium @text-muted
              if loading
                text "Working…" width=fill size=11.0 wrapping=none @text-muted
              if !loading
                text status width=fill size=11.0 wrapping=none @text-muted
              if connection_open
                text "⌄" size=14.0 @text-muted
              if !connection_open
                text "›" size=14.0 @text-muted
            active bg=white/3 text=muted border=white/6 border-w=1.0 r=8.0
            hovered bg=white/7 text=fg border=white/10
            pressed bg=white/11
          if connection_open
            slot connection
      resize-handle drag=sidebar_dragged cursor=resize-horizontal
        container width=6.0 height=fill bg=white/8
          text ""
      col width=fill height=fill padding-top=28.0
        slot notice
        match tab
          "chat"
            slot chat
          _
            slot pages
