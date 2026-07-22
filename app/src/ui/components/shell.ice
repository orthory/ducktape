component Brand()
  row width=fill spacing=9.0 align=center
    container width=28.0 height=28.0 align-x=center align-y=center bg=linear(2.3, white/20@0.0, surface/55@1.0) border=white/22 border-w=1.0 r=8.0 shadow=black/12 shadow-y=2.0 shadow-blur=8.0
      text "D" size=13.0 @font-bold text-fg
    col width=fill spacing=0.0
      text "Ducktape" size=13.0 @font-bold text-fg
      text "Workspace" size=10.0 @text-muted

component EmptyState(title:str, detail:str)
  container width=fill height=fill align-x=center align-y=center
    col spacing=6.0 align=center
      container width=34.0 height=34.0 align-x=center align-y=center bg=subtle r=8.0
        text "·" size=22.0 @text-fg
      text title size=15.0 @font-bold text-fg
      text detail size=12.0 @text-muted

component WorkspaceTabs(status:str, loading:bool)
  state
    tab = "chat"
  on select_tab(next)
    tab = next
  container width=fill height=fill clip=true bg=linear(2.35, elevated/76@0.0, bg/98@0.55, surface/90@1.0) border=white/14 border-w=1.0 r=20.0 shadow=black/18 shadow-y=8.0 shadow-blur=28.0 px-snap=true
    row width=fill height=fill
      container width=242.0 height=fill padding=12.0 padding-top=38.0 bg=linear(2.25, surface/78@0.0, sidebar/98@0.48, bg/96@1.0) border=white/12 border-w=1.0 r-tr=18.0 r-br=18.0 shadow=black/10 shadow-x=4.0 shadow-blur=18.0 clip=true
        col width=fill height=fill spacing=8.0
          Brand
          space height=6.0
          container width=fill padding-left=8.0
            text "APPS" size=10.0 @font-bold text-muted
          match tab
            "chat"
              col width=fill spacing=3.0
                button label="Chat" width=fill height=34.0 padding=7.0 -> select_tab("chat")
                  row width=fill spacing=9.0 align=center
                    text "#" width=18.0 size=14.0 align-x=center @font-bold text-fg
                    text "Chat" width=fill size=12.0 @font-bold text-fg
                  active bg=linear(2.3, white/12@0.0, surface/76@1.0) text=fg border=white/18 border-w=1.0 r=10.0 shadow=black/10 shadow-y=2.0 shadow-blur=8.0
                  pressed bg=selection
                button label="Pages" width=fill height=34.0 padding=7.0 -> select_tab("pages")
                  row width=fill spacing=9.0 align=center
                    text "□" width=18.0 size=14.0 align-x=center @text-muted
                    text "Pages" width=fill size=12.0 @text-muted
                  active bg=transparent text=muted r=10.0
                  hovered bg=white/7 text=fg
                  pressed bg=selection text=fg
            _
              col width=fill spacing=3.0
                button label="Chat" width=fill height=34.0 padding=7.0 -> select_tab("chat")
                  row width=fill spacing=9.0 align=center
                    text "#" width=18.0 size=14.0 align-x=center @text-muted
                    text "Chat" width=fill size=12.0 @text-muted
                  active bg=transparent text=muted r=10.0
                  hovered bg=white/7 text=fg
                  pressed bg=selection text=fg
                button label="Pages" width=fill height=34.0 padding=7.0 -> select_tab("pages")
                  row width=fill spacing=9.0 align=center
                    text "□" width=18.0 size=14.0 align-x=center @font-bold text-fg
                    text "Pages" width=fill size=12.0 @font-bold text-fg
                  active bg=linear(2.3, white/12@0.0, surface/76@1.0) text=fg border=white/18 border-w=1.0 r=10.0 shadow=black/10 shadow-y=2.0 shadow-blur=8.0
                  pressed bg=selection
          container width=fill height=1.0 bg=separator
            text ""
          match tab
            "chat"
              slot chat_sidebar
            _
              slot pages_sidebar
          slot connection
          row width=fill spacing=7.0 padding=7.0 align=center
            container width=7.0 height=7.0 bg=fg/55 r=3.5
              text ""
            if loading
              text "Working…" width=fill size=10.0 wrapping=none @text-muted
            if !loading
              text status width=fill size=10.0 wrapping=none @text-muted
      col width=fill height=fill
        container width=fill padding=12.0 padding-left=16.0
          row width=fill height=38.0 spacing=12.0 align=center
            match tab
              "chat"
                col spacing=0.0
                  text "Chat" size=17.0 @font-bold text-fg
                  text "Workspace conversations" size=10.0 @text-muted
              _
                col spacing=0.0
                  text "Pages" size=17.0 @font-bold text-fg
                  text "Shared documents" size=10.0 @text-muted
        slot notice
        col width=fill height=fill padding=12.0 padding-top=4.0
          match tab
            "chat"
              slot chat
            _
              slot pages
