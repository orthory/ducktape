// The window chrome and the module rail. Glass lives here and nowhere else:
// the titlebar, the rail, and each screen's sidebar are the functional layer
// the artifact allows to float; every content surface below stays opaque paper.

component Brand()
  box #root w=30.0 h=30.0 align-x=center align-y=center bg=primary r=9.0
    text "D" size=14.0 wrap=none font=code_semibold @text-toast_fg

// The chain chip left of the network name — a 15px ink plate with the mark.
component NetworkChip(name:str)
  row #root gap=7.0 align=center
    box w=15.0 h=15.0 align-x=center align-y=center bg=primary r=4.0
      text "◆" size=9.0 wrap=none font=code_semibold @text-toast_fg
    text name size=12.0 wrap=none font=display @text-accent_fg

// The height readout. Its dot carries the connection state, so the pill reads
// at a glance without spending a word on it.
component StatusPill(status:str, height:i64, loading:bool, degraded:bool)
  box #root px=8.0 py=3.0 bg=surface border=border border-w=1.0 r=7.0
    row gap=5.0 align=center
      if degraded
        box w=6.0 h=6.0 bg=danger_dot r=3.0
          space w=1.0 h=1.0
      if !degraded && loading
        box w=6.0 h=6.0 bg=warning_dot r=3.0
          space w=1.0 h=1.0
      if !degraded && !loading
        box w=6.0 h=6.0 bg=success_dot r=3.0
          space w=1.0 h=1.0
      text height_label(height) size=10.5 wrap=none font=code_medium @text-input
      text status size=10.5 wrap=none font=code_medium @text-label

component TitleBar(network:str, status:str, height:i64, loading:bool, degraded:bool, bell_badge:i64)
  col #root w=fill
    box w=fill h=39.0 px=13.0 bg=elevated
      row w=fill h=fill gap=13.0 align=center
        NetworkChip name=network
        space w=fill
        StatusPill status=status height=height loading=loading degraded=degraded
        button label="Search everything" h=24.0 @ghost_action px-8px py-0px -> toggle_palette
          row h=fill gap=6.0 align=center
            Icon name="search" tone="hint" px=13.0
            text "⌘K" size=10.5 wrap=none font=code_medium @text-hint
          active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
          hovered bg=surface text=fg border=border
          pressed bg=subtle
        stack w=26.0 h=24.0
          button label="Alerts" w=26.0 h=24.0 p=0.0 @ghost_action -> toggle_bell
            box w=fill h=fill align-x=center align-y=center
              col align=center
                if bell_badge > 0
                  Icon name="bell" tone="strong-ink" px=15.0
                if bell_badge <= 0
                  Icon name="bell" tone="label" px=15.0
            active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
            hovered bg=surface text=fg border=transparent
            pressed bg=subtle
          if bell_badge > 0
            pin x=12.0 y=1.0
              box h=13.0 px=3.0 align-y=center bg=info_dot border=surface border-w=1.0 r=6.5
                text bell_badge size=9.0 wrap=none font=code_semibold @text-brand_fg
    box w=fill h=1.0 bg=border
      space w=1.0 h=1.0

component ConnectionBanner(status:str)
  box #root w=fill h=30.0 pl=14.0 pr=14.0 bg=danger_bg border=danger_line border-w=1.0
    row w=fill h=fill gap=8.0 align=center
      box w=7.0 h=7.0 bg=danger_dot r=3.5
        space w=1.0 h=1.0
      text "Connection degraded" size=13.0 wrap=none font=medium @text-fg
      text status w=fill size=13.0 wrap=none @text-muted

// One rail entry: a 58px capsule holding a 19px glyph over a 9.5px label.
// The selected state is a `subtle` tint capsule, never a second sheet of glass.
component RailButton(item:NavItem)
  stack #root w=58.0
    if item.active
      button label=item.title w=58.0 p=0.0 @ghost_action -> select_shell_tab(item.id)
        col w=fill py=4.0 gap=4.0 align=center
          Icon name=item.icon tone="ink" px=19.0
          text item.title size=9.5 wrap=none font=display @text-strong_ink
        active bg=subtle text=fg border=transparent border-w=1.0 r=10.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg
    if !item.active
      button label=item.title w=58.0 p=0.0 @ghost_action -> select_shell_tab(item.id)
        col w=fill py=4.0 gap=4.0 align=center
          Icon name=item.icon tone="idle" px=19.0
          text item.title size=9.5 wrap=none font=display @text-caption
        active bg=transparent text=muted border=transparent border-w=1.0 r=10.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg
    if item.badge > 0
      pin x=32.0 y=6.0
        box h=15.0 px=4.0 align-y=center bg=brand border=rail border-w=1.0 r=8.0
          text item.badge size=9.0 wrap=none font=code_semibold @text-brand_fg

component NavRail(tab:str, approvals:i64, account:str)
  box #root w=74.0 h=fill pt=13.0 pb=10.0 bg=rail clip=true
      col w=fill h=fill gap=0.0 align=center
        Brand
        space w=1.0 h=9.0
        scroll dir=vertical w=fill h=fill bar=hidden
          col w=fill gap=2.0 align=center
            for item in shell_nav(tab, approvals)
              RailButton item=item
        space w=1.0 h=6.0
        if tab == "settings"
          button label="Settings" w=34.0 h=34.0 p=0.0 @ghost_action -> select_shell_tab("settings")
            box w=fill h=fill align-x=center align-y=center
              Icon name="gear" tone="ink" px=18.0
            active bg=subtle text=fg border=transparent border-w=1.0 r=9.0
            hovered bg=rail_hover text=fg
            pressed bg=subtle text=fg
        if tab != "settings"
          button label="Settings" w=34.0 h=34.0 p=0.0 @ghost_action -> select_shell_tab("settings")
            box w=fill h=fill align-x=center align-y=center
              Icon name="gear" tone="idle" px=18.0
            active bg=transparent text=muted border=transparent border-w=1.0 r=9.0
            hovered bg=rail_hover text=fg
            pressed bg=subtle text=fg
        space w=1.0 h=6.0
        button label="Account" w=28.0 h=28.0 p=0.0 @ghost_action -> select_shell_tab("settings")
          box w=fill h=fill align-x=center align-y=center
            PersonAvatar initials=initial_of(account) plate=28.0 ink=10.0
          active bg=transparent text=muted border=transparent border-w=1.0 r=14.0
          hovered bg=subtle text=fg
          pressed bg=rail_hover text=fg

// The header every screen sidebar wears: a 13.5px title, a machine count, and
// an optional trailing control the caller fills.
component SidebarHeader(title:str, count:str)
  col #root w=fill
    box w=fill pl=14.0 pr=14.0 pt=14.0 pb=11.0
      row w=fill gap=8.0 align=center
        text title size=13.5 wrap=none font=display @text-fg
        text count size=10.5 wrap=none font=code_medium @text-hint
        space w=fill
        slot
    box w=fill h=1.0 bg=separator
      space w=1.0 h=1.0

// A screen header bar: 56px, a 16px title, a machine subtitle, and whatever
// action the screen puts on the right.
component ScreenHeader(title:str, meta:str)
  col #root w=fill
    box w=fill h=56.0 px=22.0
      row w=fill h=fill gap=10.0 align=center
        text title size=16.0 wrap=none font=display @text-primary
        text meta size=12.0 wrap=none font=code @text-hint
        space w=fill
        slot
    box w=fill h=1.0 bg=separator
      space w=1.0 h=1.0

component WorkspaceTabs(network:str, status:str, height:i64, loading:bool, degraded:bool, tab:str, bell_count:i64, approvals:i64, account:str)
  box w=fill h=fill clip=true bg=bg px-snap=true
    stack w=fill h=fill
      col w=fill h=fill
        TitleBar network=network status=status height=height loading=loading degraded=degraded bell_badge=bell_count #titlebar
        if degraded
          ConnectionBanner status=status
        row w=fill h=fill
          NavRail tab=tab approvals=approvals account=account #rail
          box w=1.0 h=fill bg=separator
            space w=1.0 h=1.0
          box #content w=fill h=fill bg=bg clip=true
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
