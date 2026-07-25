component Brand()
  row spacing=9.0 align=center
    container width=30.0 height=30.0 align-x=center align-y=center bg=primary r=9.0 shadow=shadow shadow-y=1.0 shadow-blur=4.0
      text "D" size=15.0 font=display @text-popover
    col spacing=0.0
      text "Ducktape" size=17.0 wrapping=none font=display @text-fg
      text "Workspace" size=12.0 wrapping=none @text-muted

component TitleBar(status:str, loading:bool, bell_badge:i64)
  container width=fill height=52.0 padding-left=14.0 padding-right=14.0 bg=sidebar border=separator border-w=1.0
    row width=fill height=fill spacing=10.0 align=center
      Brand
      space width=fill
      button label="Notifications" height=28.0 padding=5.0 -> toggle_bell
        row height=fill spacing=5.0 align=center
          text "🔔" size=14.0 wrapping=none
          if bell_badge > 0
            container height=16.0 padding-left=5.0 padding-right=5.0 align-y=center bg=primary r=8.0
              text bell_badge size=12.0 wrapping=none font=medium @text-popover
        active bg=fg/4 text=muted border=fg/10 border-w=1.0 r=7.0
        hovered bg=fg/8 text=fg border=fg/14
        pressed bg=fg/12
      button label="Search everything" height=28.0 padding=5.0 -> toggle_palette
        row height=fill spacing=6.0 align=center
          text "Search" size=12.0 wrapping=none @text-muted
          text "⌘K" size=12.0 wrapping=none font=mono @text-muted
        active bg=fg/4 text=muted border=fg/10 border-w=1.0 r=7.0
        hovered bg=fg/8 text=fg border=fg/14
        pressed bg=fg/12
      if loading
        text "Working…" size=12.0 wrapping=none @text-muted
      if !loading
        row spacing=6.0 align=center
          container width=7.0 height=7.0 bg=success r=3.5
            text ""
          text status size=12.0 wrapping=none @text-muted

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
      text title size=15.0 font=medium @text-fg
      text detail size=14.0 @text-muted

component NavButton(item:NavItem)
  col width=fill
    if item.active
      button label=item.title width=fill height=34.0 padding=7.0 -> select_shell_tab(item.id)
        row width=fill height=fill spacing=9.0 align=center
          text item.icon width=19.0 size=15.0 align-x=center font=display @text-primary
          text item.title width=fill size=14.0 font=medium @text-fg
        active bg=primary/14 text=fg border=primary/30 border-w=1.0 r=9.0
        hovered bg=primary/20 text=fg border=primary/38
        pressed bg=primary/26 text=fg
    if !item.active
      button label=item.title width=fill height=34.0 padding=7.0 -> select_shell_tab(item.id)
        row width=fill height=fill spacing=9.0 align=center
          text item.icon width=19.0 size=15.0 align-x=center @text-muted
          text item.title width=fill size=14.0 @text-muted
        active bg=transparent text=muted border=transparent border-w=1.0 r=9.0
        hovered bg=fg/5 text=fg border=fg/8
        pressed bg=fg/8 text=fg

component WorkspaceTabs(status:str, loading:bool, degraded:bool, tab:str, bell_count:i64)
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
        TitleBar status=status loading=loading bell_badge=bell_count
        if degraded
          ConnectionBanner status=status
        row width=fill height=fill
          container width=sidebar_width height=fill padding=12.0 bg=sidebar clip=true
            col width=fill height=fill spacing=8.0
              container width=fill padding-left=8.0
                text "APPS" size=12.0 font=medium @text-muted
              col width=fill spacing=3.0
                for item in shell_nav(tab)
                  NavButton item=item
              container width=fill height=1.0 bg=separator
                text ""
              match tab
                "chat"
                  slot chat_sidebar
                "pages"
                  slot pages_sidebar
                "files"
                  col width=fill height=fill spacing=6.0
                    container width=fill padding-left=8.0
                      text "FILES" size=12.0 font=medium @text-muted
                    container width=fill padding-left=8.0
                      text "The network's shared filesystem at its committed head." size=12.0 @text-muted
                "members"
                  col width=fill height=fill spacing=6.0
                    container width=fill padding-left=8.0
                      text "MEMBERS" size=12.0 font=medium @text-muted
                    container width=fill padding-left=8.0
                      text "Validators hold quorum seats; residents hold mesh + sync standing." size=12.0 @text-muted
                "agents"
                  col width=fill height=fill spacing=6.0
                    container width=fill padding-left=8.0
                      text "AGENTS" size=12.0 font=medium @text-muted
                    container width=fill padding-left=8.0
                      text "The registered agent roster: capability tags, granted actions, standing." size=12.0 @text-muted
                "forge"
                  col width=fill height=fill spacing=6.0
                    container width=fill padding-left=8.0
                      text "FORGE" size=12.0 font=medium @text-muted
                    container width=fill padding-left=8.0
                      text "Consensus-backed repos: branches, issues, and reviewable pull requests." size=12.0 @text-muted
                "governance"
                  col width=fill height=fill spacing=6.0
                    container width=fill padding-left=8.0
                      text "GOVERNANCE" size=12.0 font=medium @text-muted
                    container width=fill padding-left=8.0
                      text "Proposals freeze their electorate when opened; anyone may settle past the deadline." size=12.0 @text-muted
                "settings"
                  col width=fill height=fill spacing=6.0
                    container width=fill padding-left=8.0
                      text "SETTINGS" size=12.0 font=medium @text-muted
                    container width=fill padding-left=8.0
                      text "Connection, identity, and this device's preferences." size=12.0 @text-muted
                "node"
                  col width=fill height=fill spacing=6.0
                    container width=fill padding-left=8.0
                      text "NODE" size=12.0 font=medium @text-muted
                    container width=fill padding-left=8.0
                      text "Peers standing and the live log ring. Logs stream only while this pane is open." size=12.0 @text-muted
                _
                  col width=fill height=fill spacing=6.0
                    container width=fill padding-left=8.0
                      text "EXPLORER" size=12.0 font=medium @text-muted
                    container width=fill padding-left=8.0
                      text "Recent non-empty blocks, newest first. Click a block for its ops." size=12.0 @text-muted
              button label="Connection" width=fill height=30.0 padding=7.0 -> toggle_connection
                row width=fill height=fill spacing=7.0 align=center
                  container width=7.0 height=7.0 bg=fg/48 border=fg/16 border-w=1.0 r=3.5
                    text ""
                  text "Connection" size=12.0 font=medium @text-muted
                  if loading
                    text "Working…" width=fill size=12.0 wrapping=none @text-muted
                  if !loading
                    text status width=fill size=12.0 wrapping=none @text-muted
                  if connection_open
                    text "⌄" size=15.0 @text-muted
                  if !connection_open
                    text "›" size=15.0 @text-muted
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
              "pages"
                slot pages
              "files"
                slot files
              "members"
                slot members
              "agents"
                slot agents
              "forge"
                slot forge
              "governance"
                slot governance
              "settings"
                slot settings
              "node"
                slot node
              _
                slot explorer

      slot palette
      slot bell
