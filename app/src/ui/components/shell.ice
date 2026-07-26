component Brand()
  box w=30.0 h=30.0 align-x=center align-y=center bg=primary r=9.0 shadow=shadow_toast shadow-y=6.0 shadow-blur=18.0
    text "D" size=14.0 font=display @text-primary_fg

component TitleBar(status:str, loading:bool, degraded:bool, bell_badge:i64)
  box #root w=fill h=40.0 px=11.0 bg=glass_regular border=glass_rim border-w=1.0
    row w=fill h=fill gap=11.0 align=center
      text "Ducktape" size=13.0 wrap=none font=medium @text-fg
      text "Workspace" size=12.0 wrap=none @text-muted
      space w=fill
      button label="Notifications" h=28.0 p=5.0 @ghost_action -> toggle_bell
        row h=fill gap=5.0 align=center
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><path d='M6.5 15.5v-5a5.5 5.5 0 0 1 11 0v5l1.5 2H5z'/><path d='M10.5 18.5a1.5 1.5 0 0 0 3 0'/></svg>" memory w=19.0 h=19.0 color=fg
          if bell_badge > 0
            box h=16.0 pl=5.0 pr=5.0 align-y=center bg=brand r=8.0
              text bell_badge size=9.0 wrap=none font=code_semibold @text-brand_fg
        active bg=fg/4 text=muted border=fg/10 border-w=1.0 r=7.0
        hovered bg=fg/8 text=fg border=fg/14
        pressed bg=fg/12
      button label="Search everything" h=28.0 p=5.0 @ghost_action -> toggle_palette
        row h=fill gap=6.0 align=center
          text "Search" size=12.0 wrap=none @text-muted
          text "⌘K" size=12.0 wrap=none font=code @text-muted
        active bg=fg/4 text=muted border=fg/10 border-w=1.0 r=7.0
        hovered bg=fg/8 text=fg border=fg/14
        pressed bg=fg/12
      if loading
        text "Working…" size=12.0 wrap=none @text-muted
      if !loading && !degraded
        row gap=6.0 align=center
          box w=7.0 h=7.0 bg=success r=3.5
            text ""
          text status size=12.0 wrap=none @text-muted

component ConnectionBanner(status:str)
  box w=fill h=30.0 pl=14.0 pr=14.0 bg=danger/12 border=danger/30 border-w=1.0
    row w=fill h=fill gap=8.0 align=center
      box w=7.0 h=7.0 bg=danger r=3.5
        text ""
      text "Connection degraded" size=13.0 wrap=none font=medium @text-fg
      text status w=fill size=13.0 wrap=none @text-muted

component NavIcon(id:str, selected:bool)
  stack w=19.0 h=19.0
    if selected
      match id
        "chat"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><path d='M5 7a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-6l-4 3.5V14H7a2 2 0 0 1-2-2z'/></svg>" memory w=19.0 h=19.0 color=fg
        "pages"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.7' stroke-linecap='round' stroke-linejoin='round'><path d='M7 3h7l4 4v14H7z'/><path d='M14 3v4h4'/></svg>" memory w=19.0 h=19.0 color=fg
        "files"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.7' stroke-linecap='round' stroke-linejoin='round'><path d='M7 3h7l4 4v14H7z'/><path d='M14 3v4h4'/></svg>" memory w=19.0 h=19.0 color=fg
        "members"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><circle cx='10' cy='8' r='3'/><path d='M4.5 18c0-3 2.4-4.6 5.5-4.6 1 0 1.8.2 2.6.5'/><path d='M16 6.3a2.8 2.8 0 0 1 .3 5.4'/><path d='M17.6 13.7c1.9.5 2.9 1.9 2.9 3.9'/></svg>" memory w=19.0 h=19.0 color=fg
        "agents"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><rect x='4.5' y='4.5' width='6' height='6' rx='1.4'/><rect x='13.5' y='4.5' width='6' height='6' rx='1.4'/><rect x='4.5' y='13.5' width='6' height='6' rx='1.4'/><rect x='13.5' y='13.5' width='6' height='6' rx='1.4'/></svg>" memory w=19.0 h=19.0 color=fg
        "forge"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><circle cx='6' cy='6' r='2.4'/><circle cx='6' cy='18' r='2.4'/><circle cx='17.5' cy='7.5' r='2.4'/><path d='M6 8.4v7.2'/><path d='M17.5 9.9c0 3.6-2.7 4.4-5.4 5-1.7.4-2.9.9-2.9 2.4'/></svg>" memory w=19.0 h=19.0 color=fg
        "governance"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><path d='M12 3.4l6.6 2.3v5c0 4.2-2.8 7-6.6 8.5-3.8-1.5-6.6-4.3-6.6-8.5v-5z'/><path d='M9.2 11.7l2 2 3.6-3.8'/></svg>" memory w=19.0 h=19.0 color=fg
        "explorer"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><rect x='4.5' y='4.5' width='6' height='6' rx='1.4'/><rect x='13.5' y='4.5' width='6' height='6' rx='1.4'/><rect x='4.5' y='13.5' width='6' height='6' rx='1.4'/><rect x='13.5' y='13.5' width='6' height='6' rx='1.4'/></svg>" memory w=19.0 h=19.0 color=fg
        "node"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><path d='M12 3.4l7.4 4.27v8.66L12 20.6l-7.4-4.27V7.67z'/><circle cx='12' cy='12' r='2.3'/></svg>" memory w=19.0 h=19.0 color=fg
        "settings"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='3'/><path d='M12 4v2M12 18v2M4 12h2M18 12h2M6.3 6.3l1.4 1.4M16.3 16.3l1.4 1.4M17.7 6.3l-1.4 1.4M7.7 16.3l-1.4 1.4'/></svg>" memory w=19.0 h=19.0 color=fg
        _
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><rect x='4.5' y='4.5' width='6' height='6' rx='1.4'/><rect x='13.5' y='4.5' width='6' height='6' rx='1.4'/><rect x='4.5' y='13.5' width='6' height='6' rx='1.4'/><rect x='13.5' y='13.5' width='6' height='6' rx='1.4'/></svg>" memory w=19.0 h=19.0 color=fg
    if !selected
      match id
        "chat"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><path d='M5 7a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-6l-4 3.5V14H7a2 2 0 0 1-2-2z'/></svg>" memory w=19.0 h=19.0 color=input hover=fg
        "pages"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.7' stroke-linecap='round' stroke-linejoin='round'><path d='M7 3h7l4 4v14H7z'/><path d='M14 3v4h4'/></svg>" memory w=19.0 h=19.0 color=input hover=fg
        "files"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.7' stroke-linecap='round' stroke-linejoin='round'><path d='M7 3h7l4 4v14H7z'/><path d='M14 3v4h4'/></svg>" memory w=19.0 h=19.0 color=input hover=fg
        "members"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><circle cx='10' cy='8' r='3'/><path d='M4.5 18c0-3 2.4-4.6 5.5-4.6 1 0 1.8.2 2.6.5'/><path d='M16 6.3a2.8 2.8 0 0 1 .3 5.4'/><path d='M17.6 13.7c1.9.5 2.9 1.9 2.9 3.9'/></svg>" memory w=19.0 h=19.0 color=input hover=fg
        "agents"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><rect x='4.5' y='4.5' width='6' height='6' rx='1.4'/><rect x='13.5' y='4.5' width='6' height='6' rx='1.4'/><rect x='4.5' y='13.5' width='6' height='6' rx='1.4'/><rect x='13.5' y='13.5' width='6' height='6' rx='1.4'/></svg>" memory w=19.0 h=19.0 color=input hover=fg
        "forge"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><circle cx='6' cy='6' r='2.4'/><circle cx='6' cy='18' r='2.4'/><circle cx='17.5' cy='7.5' r='2.4'/><path d='M6 8.4v7.2'/><path d='M17.5 9.9c0 3.6-2.7 4.4-5.4 5-1.7.4-2.9.9-2.9 2.4'/></svg>" memory w=19.0 h=19.0 color=input hover=fg
        "governance"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><path d='M12 3.4l6.6 2.3v5c0 4.2-2.8 7-6.6 8.5-3.8-1.5-6.6-4.3-6.6-8.5v-5z'/><path d='M9.2 11.7l2 2 3.6-3.8'/></svg>" memory w=19.0 h=19.0 color=input hover=fg
        "explorer"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><rect x='4.5' y='4.5' width='6' height='6' rx='1.4'/><rect x='13.5' y='4.5' width='6' height='6' rx='1.4'/><rect x='4.5' y='13.5' width='6' height='6' rx='1.4'/><rect x='13.5' y='13.5' width='6' height='6' rx='1.4'/></svg>" memory w=19.0 h=19.0 color=input hover=fg
        "node"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><path d='M12 3.4l7.4 4.27v8.66L12 20.6l-7.4-4.27V7.67z'/><circle cx='12' cy='12' r='2.3'/></svg>" memory w=19.0 h=19.0 color=input hover=fg
        "settings"
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='3'/><path d='M12 4v2M12 18v2M4 12h2M18 12h2M6.3 6.3l1.4 1.4M16.3 16.3l1.4 1.4M17.7 6.3l-1.4 1.4M7.7 16.3l-1.4 1.4'/></svg>" memory w=19.0 h=19.0 color=input hover=fg
        _
          svg "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'><rect x='4.5' y='4.5' width='6' height='6' rx='1.4'/><rect x='13.5' y='4.5' width='6' height='6' rx='1.4'/><rect x='4.5' y='13.5' width='6' height='6' rx='1.4'/><rect x='13.5' y='13.5' width='6' height='6' rx='1.4'/></svg>" memory w=19.0 h=19.0 color=input hover=fg

component NavButton(item:NavItem)
  col w=fill align=center
    if item.active
      button label=item.title w=58.0 p=8.0 @ghost_action -> select_shell_tab(item.id)
        col w=fill gap=4.0 align=center
          NavIcon id=item.id selected=true
          text item.title size=9.5 wrap=none font=display @text-fg
        active bg=subtle text=fg border=transparent border-w=1.0 r=10.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg
    if !item.active
      button label=item.title w=58.0 p=8.0 @ghost_action -> select_shell_tab(item.id)
        col w=fill gap=4.0 align=center
          NavIcon id=item.id selected=false
          text item.title size=9.5 wrap=none font=display @text-input
        active bg=transparent text=muted border=transparent border-w=1.0 r=10.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg

component WorkspaceTabs(status:str, loading:bool, degraded:bool, tab:str, bell_count:i64)
  state
    connection_open = false
  on toggle_connection
    connection_open = !connection_open
  box w=fill h=fill clip=true bg=transparent border=border border-w=1.0 r=11.0 px-snap=true
    stack w=fill h=fill
      col w=fill h=fill
        TitleBar status=status loading=loading degraded=degraded bell_badge=bell_count #titlebar
        if degraded
          ConnectionBanner status=status
        row w=fill h=fill
          box #rail w=74.0 h=fill py=11.0 bg=glass_thin border=glass_rim border-w=1.0 clip=true
            col w=fill h=fill align=center
              Brand
              space w=1.0 h=7.0
              scroll dir=vertical w=fill h=fill
                col w=fill gap=4.0 align=center
                  for item in shell_nav(tab)
                    NavButton item=item
          box #sidebar w=236.0 h=fill px=8.0 py=9.0 bg=glass_thin border=glass_rim border-w=1.0 clip=true
            col w=fill h=fill gap=8.0
              match tab
                "chat"
                  slot chat_sidebar
                "pages"
                  slot pages_sidebar
                "files"
                  col w=fill h=fill gap=6.0
                    box w=fill pl=8.0
                      text "FILES" size=10.0 font=code_semibold @text-muted
                    box w=fill pl=8.0
                      text "The network's shared filesystem at its committed head." size=12.5 @text-muted
                "members"
                  col w=fill h=fill gap=6.0
                    box w=fill pl=8.0
                      text "MEMBERS" size=10.0 font=code_semibold @text-muted
                    box w=fill pl=8.0
                      text "Validators hold quorum seats; residents hold mesh + sync standing." size=12.5 @text-muted
                "agents"
                  col w=fill h=fill gap=6.0
                    box w=fill pl=8.0
                      text "AGENTS" size=10.0 font=code_semibold @text-muted
                    box w=fill pl=8.0
                      text "The registered agent roster: capability tags, granted actions, standing." size=12.5 @text-muted
                "forge"
                  col w=fill h=fill gap=6.0
                    box w=fill pl=8.0
                      text "FORGE" size=10.0 font=code_semibold @text-muted
                    box w=fill pl=8.0
                      text "Consensus-backed repos: branches, issues, and reviewable pull requests." size=12.5 @text-muted
                "governance"
                  col w=fill h=fill gap=6.0
                    box w=fill pl=8.0
                      text "GOVERNANCE" size=10.0 font=code_semibold @text-muted
                    box w=fill pl=8.0
                      text "Proposals freeze their electorate when opened; anyone may settle past the deadline." size=12.5 @text-muted
                "settings"
                  col w=fill h=fill gap=6.0
                    box w=fill pl=8.0
                      text "SETTINGS" size=10.0 font=code_semibold @text-muted
                    box w=fill pl=8.0
                      text "Connection, identity, and this device's preferences." size=12.5 @text-muted
                "node"
                  col w=fill h=fill gap=6.0
                    box w=fill pl=8.0
                      text "NODE" size=10.0 font=code_semibold @text-muted
                    box w=fill pl=8.0
                      text "Peers standing and the live log ring. Logs stream only while this pane is open." size=12.5 @text-muted
                _
                  col w=fill h=fill gap=6.0
                    box w=fill pl=8.0
                      text "EXPLORER" size=10.0 font=code_semibold @text-muted
                    box w=fill pl=8.0
                      text "Recent non-empty blocks, newest first. Click a block for its ops." size=12.5 @text-muted
              button label="Connection" #connection-toggle w=fill h=30.0 p=7.0 @ghost_action -> toggle_connection
                row w=fill h=fill gap=7.0 align=center
                  box w=7.0 h=7.0 bg=fg/48 border=fg/16 border-w=1.0 r=3.5
                    text ""
                  text "Connection" size=13.0 font=medium @text-muted
                  if loading
                    text "Working…" w=fill size=12.0 wrap=none @text-muted
                  if !loading
                    text status w=fill size=12.0 wrap=none @text-muted
                  if connection_open
                    text "⌄" size=14.0 @text-muted
                  if !connection_open
                    text "›" size=14.0 @text-muted
                active bg=fg/3 text=muted border=fg/6 border-w=1.0 r=8.0
                hovered bg=fg/7 text=fg border=fg/10
                pressed bg=fg/11
              if connection_open
                slot connection
          box #content w=fill h=fill bg=bg
            col w=fill h=fill
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
