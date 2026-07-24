component Brand()
  row spacing=9.0 align=center
    container width=26.0 height=26.0 align-x=center align-y=center bg=primary r=8.0 shadow=shadow shadow-y=1.0 shadow-blur=4.0
      text "D" size=14.0 font=display @text-popover
    col spacing=0.0
      text "Ducktape" size=13.0 wrapping=none font=display @text-fg
      text "Workspace" size=11.0 wrapping=none @text-muted

component TitleBar(status:str, loading:bool)
  container width=fill height=44.0 padding-left=14.0 padding-right=14.0 bg=sidebar border=separator border-w=1.0
    row width=fill height=fill spacing=10.0 align=center
      Brand
      space width=fill
      button label="Search everything" height=26.0 padding=5.0 -> toggle_palette
        row height=fill spacing=6.0 align=center
          text "Search" size=11.0 wrapping=none @text-muted
          text "⌘K" size=11.0 wrapping=none font=mono @text-muted
        active bg=fg/4 text=muted border=fg/10 border-w=1.0 r=7.0
        hovered bg=fg/8 text=fg border=fg/14
        pressed bg=fg/12
      if loading
        text "Working…" size=11.0 wrapping=none font=mono @text-muted
      if !loading
        row spacing=6.0 align=center
          container width=7.0 height=7.0 bg=success r=3.5
            text ""
          text status size=11.0 wrapping=none font=mono @text-muted

component ConnectionBanner(status:str)
  container width=fill height=30.0 padding-left=14.0 padding-right=14.0 bg=danger/12 border=danger/30 border-w=1.0
    row width=fill height=fill spacing=8.0 align=center
      container width=7.0 height=7.0 bg=danger r=3.5
        text ""
      text "Connection degraded" size=13.0 wrapping=none font=medium @text-fg
      text status width=fill size=13.0 wrapping=none @text-muted

component EmptyState(title:str, detail:str)
  container width=fill height=fill align-x=center align-y=center
    col spacing=6.0 align=center
      text title size=14.0 font=medium @text-fg
      text detail size=13.0 @text-muted

component WorkspaceTabs(status:str, loading:bool, degraded:bool, tab:str)
  state
    connection_open = false
    sidebar_width = 240.0
  on toggle_connection
    connection_open = !connection_open
  on sidebar_dragged(dx, dy)
    return if dx < 0.0 && sidebar_width + dx < 180.0
    return if dx > 0.0 && sidebar_width + dx > 460.0
    sidebar_width = sidebar_width + dx
  container width=fill height=fill clip=true bg=bg border=border border-w=1.0 px-snap=true
    stack width=fill height=fill
      col width=fill height=fill
        TitleBar status=status loading=loading
        if degraded
          ConnectionBanner status=status
        row width=fill height=fill
          container width=sidebar_width height=fill padding=12.0 bg=sidebar clip=true
            col width=fill height=fill spacing=8.0
              container width=fill padding-left=8.0
                text "APPS" size=11.0 font=medium @text-muted
              match tab
                "chat"
                  col width=fill spacing=3.0
                    button label="Chat" width=fill height=32.0 padding=7.0 -> select_shell_tab("chat")
                      row width=fill height=fill spacing=9.0 align=center
                        text "#" width=18.0 size=14.0 align-x=center font=display @text-primary
                        text "Chat" width=fill size=13.0 font=medium @text-fg
                      active bg=primary/14 text=fg border=primary/30 border-w=1.0 r=9.0
                      hovered bg=primary/20 text=fg border=primary/38
                      pressed bg=primary/26 text=fg
                    button label="Pages" width=fill height=32.0 padding=7.0 -> select_shell_tab("pages")
                      row width=fill height=fill spacing=9.0 align=center
                        text "▤" width=18.0 size=13.0 align-x=center @text-muted
                        text "Pages" width=fill size=13.0 @text-muted
                      active bg=transparent text=muted border=transparent border-w=1.0 r=9.0
                      hovered bg=fg/5 text=fg border=fg/8
                      pressed bg=fg/8 text=fg
                _
                  col width=fill spacing=3.0
                    button label="Chat" width=fill height=32.0 padding=7.0 -> select_shell_tab("chat")
                      row width=fill height=fill spacing=9.0 align=center
                        text "#" width=18.0 size=14.0 align-x=center @text-muted
                        text "Chat" width=fill size=13.0 @text-muted
                      active bg=transparent text=muted border=transparent border-w=1.0 r=9.0
                      hovered bg=fg/5 text=fg border=fg/8
                      pressed bg=fg/8 text=fg
                    button label="Pages" width=fill height=32.0 padding=7.0 -> select_shell_tab("pages")
                      row width=fill height=fill spacing=9.0 align=center
                        text "▤" width=18.0 size=14.0 align-x=center font=display @text-primary
                        text "Pages" width=fill size=13.0 font=medium @text-fg
                      active bg=primary/14 text=fg border=primary/30 border-w=1.0 r=9.0
                      hovered bg=primary/20 text=fg border=primary/38
                      pressed bg=primary/26 text=fg
              container width=fill height=1.0 bg=separator
                text ""
              match tab
                "chat"
                  slot chat_sidebar
                _
                  slot pages_sidebar
              button label="Connection" width=fill height=28.0 padding=7.0 -> toggle_connection
                row width=fill height=fill spacing=7.0 align=center
                  container width=7.0 height=7.0 bg=fg/48 border=fg/16 border-w=1.0 r=3.5
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
                active bg=fg/3 text=muted border=fg/6 border-w=1.0 r=8.0
                hovered bg=fg/7 text=fg border=fg/10
                pressed bg=fg/11
              if connection_open
                slot connection
          resize-handle drag=sidebar_dragged cursor=resize-horizontal
            container width=6.0 height=fill bg=fg/8
              text ""
          col width=fill height=fill
            slot notice
            match tab
              "chat"
                slot chat
              _
                slot pages

      slot palette
