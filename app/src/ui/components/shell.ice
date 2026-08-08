// The window chrome and the module rail — the artifact's own non-glass ladder,
// desk / rail / sidebar / content. iced has no backdrop blur, so every surface
// here is opaque paper; nothing in this file tints, gradients or floats.
//
// THE RAIL DECISION. The console has EXACTLY EIGHT seats — Chat, Pages, Forge,
// Agents, Files, Explorer, Members, Approvals — and no Node capsule. Node facts
// live in the titlebar's status card (below) and in Settings, reached from the
// rail's footer button. `shell_nav` returns those eight and nothing else, so
// WorkspaceTabs routes eight screens plus settings; there is no `node` slot and
// no Modules seat (its catalog has no data source at all).
//
// TYPE SCALE. The artifact's own values where the scale carries them — the
// network name is 11.5 and the chip mark 7 (nearest step 7.5). The scale was
// widened to hold them rather than the view snapped to fit the guard; the
// guard's own panic says so. Two remain snapped, and deliberately: the card's
// height readout 17 -> 14.0 (16.0 and 20.0 are sans-only by the guard, and a
// block height is a machine value) and the card footnote 10 -> 12.5 (10.0 is
// pinned to mono semibold).

component Brand()
  box #root
    with
      w=30.0
      h=30.0
      align-x=center
      align-y=center
      bg=primary
      r=9.0
    text "D"
      with
        size=14.0
        wrap=none
        font=code_semibold
        @text-toast_fg

// The chain chip left of the network name — a 15px ink plate with the mark.
// The chip is the way BACK: clicking the network's name returns to the
// launch window's picker without forgetting anything — the non-destructive
// sibling of Danger Zone's forget.
component NetworkChip(name:str)
  emits
    switch_network
  button #root -> emit(switch_network)
    with
      label="Switch network"
      p=4.0
      @ghost_action
    row gap=7.0 align=center
      box
        with
          w=15.0
          h=15.0
          align-x=center
          align-y=center
          bg=primary
          r=4.0
        text "◆"
          with
            size=7.5
            wrap=none
            font=code_semibold
            @text-toast_fg
      text name
        with
          size=11.5
          wrap=none
          font=display
          @text-accent_fg
    active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
    hovered bg=rail_hover text=fg
    pressed bg=subtle text=fg

// The one dot that carries connection state, at both the pill's 6px and the
// status card's 7px. A stopped node wears `alert_dot`, never the traffic-light
// red — that hex belongs to the window's close button and nothing else.
component StatusDot(degraded:bool, loading:bool, plate:f64)
  col #root
    if degraded
      box
        with
          w=plate
          h=plate
          bg=alert_dot
          r=(plate / 2.0)
        space w=1.0 h=1.0
    if !degraded && loading
      box
        with
          w=plate
          h=plate
          bg=warning_dot
          r=(plate / 2.0)
        space w=1.0 h=1.0
    if !degraded && !loading
      box
        with
          w=plate
          h=plate
          bg=success_dot
          r=(plate / 2.0)
        space w=1.0 h=1.0

// The pill says ONE state word. The height it used to print twice now appears
// once, inside the card the pill opens.
component StatusPill(degraded:bool, loading:bool)
  box #root
    with
      px=8.0
      py=3.0
      bg=surface
      border=border
      border-w=1.0
      r=7.0
    row gap=5.0 align=center
      StatusDot
        with
          degraded
          loading
          plate=6.0
      if degraded
        text "Stopped"
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-input
      if !degraded && loading
        text "Syncing…"
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-input
      if !degraded && !loading
        text "Synced"
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-input

// The 284px card behind the pill: what this node knows about the chain it is
// standing on. Every value comes off /v1/status through `load_node_facts`.
//
// The card owns its own paper — same surface the bell card wears, r13 over the
// 16/40 modal shadow — because the tooltip frame around it must stay
// transparent to carry the edge gutter (see the mount below).
//
// OMITTED, not faked: the `gossip` row (NodeFacts carries peers_live/total but
// no state field holds them) and the 26-bar sparkline (it needs the newest 26
// of a 100-block window with a clamped bar height, and neither a slice nor a
// clamp exists as a helper). Both are named in the handoff report.
component StatusCard(degraded:bool, loading:bool, answered:bool, height:i64, tier:str, root_hash:str, consensus_view:str, quorum:str, reachable:str, last_finalized:i64, checkpoint:i64)
  box #root
    with
      w=284.0
      pl=14.0
      pr=14.0
      pt=13.0
      pb=13.0
      bg=surface
      border=border
      border-w=1.0
      r=13.0
      shadow=shadow_modal
      shadow-y=16.0
      shadow-blur=40.0
    col w=fill gap=11.0
      row
        with
          w=fill
          gap=7.0
          align=center
        StatusDot
          with
            degraded
            loading
            plate=7.0
        if degraded
          text "Stopped"
            with
              size=12.0
              wrap=none
              font=display
              @text-primary
        if !degraded && loading
          text "Syncing…"
            with
              size=12.0
              wrap=none
              font=display
              @text-primary
        if !degraded && !loading
          text "Synced"
            with
              size=12.0
              wrap=none
              font=display
              @text-primary
        space w=fill
        // Same empty answer Settings guards on: an empty tier is not a
        // standing, it is the roster not having answered — say so instead of
        // leaving the slot blank. But `answered` is what separates the two
        // readings an empty tier collapses: the roster load is kicked off at
        // the END of hydration, AFTER `loading` goes false, so on every cold
        // start there is a window where nothing has answered yet. Alarming
        // through it made "standing unknown" the normal boot state and cost
        // the words their meaning.
        if !empty(tier)
          text tier
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-meta
        if empty(tier) && answered
          text "standing unknown"
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-meta
      text height_label(height)
        with
          size=14.0
          wrap=none
          font=code_semibold
          @text-primary
      col w=fill gap=6.0
        row
          with
            w=fill
            gap=14.0
            align=center
          text "app-hash"
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-hint
          box w=fill clip=true
            row w=fill
              space w=fill
              text root_hash
                with
                  size=10.5
                  wrap=none
                  font=code_medium
                  @text-secondary_fg
        row
          with
            w=fill
            gap=14.0
            align=center
          text "finality"
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-hint
          space w=fill
          row gap=4.0 align=center
            text "view"
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-secondary_fg
            text consensus_view
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-secondary_fg
            text "·"
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-hint
            text reachable
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-secondary_fg
            text "/"
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-hint
            text quorum
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-secondary_fg
            text "certs"
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-secondary_fg
        row
          with
            w=fill
            gap=14.0
            align=center
          text "last block"
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-hint
          space w=fill
          text relative_time(last_finalized)
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-secondary_fg
        row
          with
            w=fill
            gap=14.0
            align=center
          text "checkpoint"
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-hint
          space w=fill
          text height_label(checkpoint)
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-secondary_fg
      col w=fill gap=9.0
        box
          with
            w=fill
            h=1.0
            bg=separator
          space w=1.0 h=1.0
        text "This node verifies every record itself · it takes no one's word for it"
          with
            w=fill
            size=12.5
            @text-meta

// The badge over the bell: 13px tall, ringed 1.5px in the bar's own paper so
// the plate reads as a badge and not as ink on the bell. Its fill is the WORST
// UNREAD SEVERITY, never a fixed accent — three ALERTs and three INFOs are not
// the same news, and the count alone cannot say which one you are looking at.
// `plate` is the width the digit run needs; `sev` is the one discriminant.
component BellBadge(count:i64, sev:str, plate:f64)
  col #root
    match sev
      "danger"
        box
          with
            w=plate
            h=13.0
            align-x=center
            align-y=center
            bg=alert_dot
            border=surface
            border-w=1.5
            r=7.0
          text count
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-brand_fg
      "warning"
        box
          with
            w=plate
            h=13.0
            align-x=center
            align-y=center
            bg=warning_dot
            border=surface
            border-w=1.5
            r=7.0
          text count
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-brand_fg
      _
        box
          with
            w=plate
            h=13.0
            align-x=center
            align-y=center
            bg=info_dot
            border=surface
            border-w=1.5
            r=7.0
          text count
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-brand_fg

// 40px: a 39px bar over its 1px rule. This bar exists only in the console
// window — the launch window wears the OS's own chrome — so the chip and the
// right cluster are unconditional now.
component TitleBar(network:str, height:i64, loading:bool, degraded:bool, bell_badge:i64, bell_sev:str, tier:str, answered:bool, root_hash:str, consensus_view:str, quorum:str, reachable:str, last_finalized:i64, checkpoint:i64)
  emits
    toggle_bell
    switch_network
  col #root w=fill
    // The left padding steps over macOS's traffic lights: with a hidden-title
    // transparent titlebar the three window buttons overlay the content view's
    // top-left ~70px, and the chain chip sat under them. Zero elsewhere.
    box
      with
        w=fill
        h=39.0
        pl=(13.0 + titlebar_inset())
        pr=13.0
        bg=elevated
      row
        with
          w=fill
          h=fill
          gap=13.0
          align=center
        NetworkChip name=network
          forward
            switch_network
        space w=fill
        row gap=6.0 align=center
          // The pill sits ~78px from the window's right wall and the card is
          // 284 wide, so the tip ALWAYS overflows — and an overflowing tooltip
          // is snapped hard to the viewport edge, with no inset of its own.
          // The frame is therefore transparent and carries a 13px right gutter,
          // which lands the card's right edge on the same line the bell card
          // holds (view.ice `pr=13.0`) instead of glued to the wall.
          // gap=13.5 lands the card's TOP on the bell card's line too
          // (view.ice pt=44.0): one dropdown line, both titlebar cards.
          tooltip
            with
              position=bottom
              gap=13.5
              p=0.0
              delay=90
              style=transparent
            StatusPill degraded=degraded loading=loading
            box pr=13.0
              StatusCard
                with
                  degraded
                  loading
                  answered
                  height
                  tier
                  root_hash
                  consensus_view
                  quorum
                  reachable
                  last_finalized
                  checkpoint
          stack w=26.0 h=24.0
            button -> emit(toggle_bell)
              with
                label="Alerts"
                p=5.0
                @icon_action
              col align=center
                if bell_badge > 0
                  Icon
                    with
                      name="bell"
                      tone="strong-ink"
                      px=15.0
                if bell_badge <= 0
                  Icon
                    with
                      name="bell"
                      tone="label"
                      px=15.0
              active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
              hovered bg=surface text=fg border=transparent
              pressed bg=subtle
            // The badge is right-anchored: `pin` takes x/y only, so each
            // width branch names the x that keeps its right edge on the
            // artifact's line.
            if bell_badge > 0 && bell_badge < 10
              pin x=12.0 y=1.0
                BellBadge
                  with
                    count=bell_badge
                    sev=bell_sev
                    plate=13.0
            if bell_badge > 9
              pin x=7.0 y=1.0
                BellBadge
                  with
                    count=bell_badge
                    sev=bell_sev
                    plate=18.0
    box
      with
        w=fill
        h=1.0
        bg=border
      space w=1.0 h=1.0

// The artifact has no banner: a degraded node speaks through the status dot and
// an ALERT row in the bell. Neither carries the node's own words yet, so the
// band stays — restyled onto the alert family so it reads as one severity
// vocabulary with the dot above it rather than as a second red language.
// It is mounted INSIDE the content column, beside the error notice: above the
// panes it would displace the nav rail every time the connection wobbled, and a
// rail that moves under the pointer sends a remembered click to the wrong screen.
component ConnectionBanner(status:str)
  box #root
    with
      w=fill
      h=30.0
      pl=14.0
      pr=14.0
      bg=alert_bg
      border=alert_line
      border-w=1.0
    row
      with
        w=fill
        h=fill
        gap=8.0
        align=center
      box
        with
          w=7.0
          h=7.0
          bg=alert_dot
          r=3.5
        space w=1.0 h=1.0
      text "Connection degraded"
        with
          size=13.0
          wrap=none
          font=medium
          @text-alert_fg
      text status
        with
          w=fill
          size=13.0
          @text-caption

// One rail entry: a 58px capsule holding a 19px glyph over a 9.5px label. The
// selected state is a `subtle` tint capsule, never a second sheet of glass.
// Forge alone wears a live dot while an agent is running.
component RailButton(item:NavItem)
  emits
    select_shell_tab(str)
  stack #root w=58.0
    if item.active
      button -> emit(select_shell_tab, item.id)
        with
          label=item.title
          w=58.0
          p=0.0
          @icon_action
        col
          with
            w=fill
            py=4.0
            gap=4.0
            align=center
          Icon
            with
              name=item.icon
              tone="ink"
              px=19.0
          text item.title
            with
              size=9.5
              wrap=none
              font=display
              @text-strong_ink
        active bg=subtle text=fg border=transparent border-w=1.0 r=10.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg
    if !item.active
      button -> emit(select_shell_tab, item.id)
        with
          label=item.title
          w=58.0
          p=0.0
          @icon_action
        col
          with
            w=fill
            py=4.0
            gap=4.0
            align=center
          Icon
            with
              name=item.icon
              tone="idle"
              px=19.0
          text item.title
            with
              size=9.5
              wrap=none
              font=display
              @text-caption
        active bg=transparent text=muted border=transparent border-w=1.0 r=10.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg
    if item.live
      pin x=36.0 y=6.0
        box
          with
            p=1.5
            bg=rail
            r=5.0
          PulseDot plate=7.0 tone="success"
    // The badge is right-anchored: `pin` takes x/y only, so each width branch
    // names the x that puts its right edge on the artifact's 47px line.
    if item.badge > 0 && item.badge < 10
      pin x=32.0 y=6.0
        box
          with
            w=15.0
            h=15.0
            align-x=center
            align-y=center
            bg=brand
            border=rail
            border-w=1.5
            r=8.0
          text item.badge
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-brand_fg
    if item.badge > 9
      pin x=25.0 y=6.0
        box
          with
            w=22.0
            h=15.0
            align-x=center
            align-y=center
            bg=brand
            border=rail
            border-w=1.5
            r=8.0
          text item.badge
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-brand_fg

component NavRail(tab:str, approvals:i64, account:str, agent_live:bool)
  emits
    select_shell_tab(str)
  box #root
    with
      w=74.0
      h=fill
      pt=13.0
      pb=10.0
      bg=rail
      clip=true
    col
      with
        w=fill
        h=fill
        gap=0.0
        align=center
      Brand
      space w=1.0 h=9.0
      scroll
        with
          dir=vertical
          w=fill
          h=fill
          bar=hidden
        col
          with
            w=fill
            gap=2.0
            align=center
          for item in shell_nav(tab, approvals, agent_live)
            RailButton item=item
              forward
                select_shell_tab
      if tab == "settings"
        button -> emit(select_shell_tab, "settings")
          with
            label="Settings"
            p=8.0
            @icon_action
          Icon
            with
              name="gear"
              tone="ink"
              px=18.0
          active bg=subtle text=fg border=transparent border-w=1.0 r=9.0
          hovered bg=rail_hover text=fg
          pressed bg=subtle text=fg
      if tab != "settings"
        button -> emit(select_shell_tab, "settings")
          with
            label="Settings"
            p=8.0
            @icon_action
          Icon
            with
              name="gear"
              tone="idle"
              px=18.0
          active bg=transparent text=muted border=transparent border-w=1.0 r=9.0
          hovered bg=rail_hover text=fg
          pressed bg=subtle text=fg
      space w=1.0 h=6.0
      // The avatar is the one thing hung below the footer button: a 1.5px
      // paper ring inside a 1px hairline halo, which no other person plate in
      // the app wears.
      button -> emit(select_shell_tab, "settings")
        with
          label="Account"
          p=0.0
          @icon_action
        box
          with
            p=1.0
            bg=pending_line
            r=16.5
          PrincipalAvatar
            with
              initials=initial_of(account)
              is_agent=false
              plate=28.0
              ink=10.0
              ring="paper"
        active bg=transparent text=muted border=transparent border-w=1.0 r=16.5
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg

// The header every screen sidebar wears: a 13.5px title, a machine count, and
// an optional trailing control the caller fills.
//
// MOUNTED by the Pages sidebar (screens/pages.ice), with the New-page button in
// the slot. The Chat sidebar head is the same geometry but interleaves a
// connection dot BETWEEN the title and the count, which this signature cannot
// express (the slot is past the `space w=fill`), so chat either keeps its
// hand-rolled head or the dot moves to the trailing slot — a design call, not
// a mechanical one. The Files screen has no sidebar at all yet.
//
// **50px, not padding-sized.** It used to be pt 14 / pb 11 around the title, so
// the rule under it landed at 44 while the pane header it meets across the
// separator lands at 51 (50 + its own rule) — a 7px step at the seam, on every
// screen that pairs the two. The height is the contract; the padding follows.
component SidebarHeader(title:str, count:i64)
  col #root w=fill
    box
      with
        w=fill
        h=50.0
        pl=14.0
        pr=14.0
      row
        with
          w=fill
          h=fill
          gap=8.0
          align=center
        text title
          with
            size=13.5
            wrap=none
            font=display
            @text-fg
        text count
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-hint
        space w=fill
        slot
    box
      with
        w=fill
        h=1.0
        bg=separator
      space w=1.0 h=1.0

// A screen header bar: 56px, a 16px title, a machine subtitle, and whatever
// action the screen puts on the right.
component ScreenHeader(title:str, meta:str)
  col #root w=fill
    box
      with
        w=fill
        h=56.0
        px=22.0
      row
        with
          w=fill
          h=fill
          gap=10.0
          align=center
        text title
          with
            size=16.0
            wrap=none
            font=display
            @text-primary
        text meta
          with
            size=12.0
            wrap=none
            font=code
            @text-hint
        space w=fill
        slot
    box
      with
        w=fill
        h=1.0
        bg=separator
      space w=1.0 h=1.0

component WorkspaceTabs(network:str, status:str, height:i64, loading:bool, degraded:bool, tab:str, bell_count:i64, bell_sev:str, approvals:i64, account:str, agent_live:bool, tier:str, answered:bool, root_hash:str, consensus_view:str, quorum:str, reachable:str, last_finalized:i64, checkpoint:i64)
  emits
    select_shell_tab(str)
    toggle_bell
    switch_network
  box
    with
      w=fill
      h=fill
      clip=true
      bg=bg
      px-snap=true
    stack w=fill h=fill
      col w=fill h=fill
        TitleBar #titlebar
          with
            network
            height
            loading
            degraded
            bell_badge=bell_count
            bell_sev
            tier
            answered
            root_hash
            consensus_view
            quorum
            reachable
            last_finalized
            checkpoint
          forward
            toggle_bell
            switch_network
        row w=fill h=fill
          NavRail #rail
            with
              tab
              approvals
              account
              agent_live
            forward
              select_shell_tab
          box
            with
              w=1.0
              h=fill
              bg=separator
            space w=1.0 h=1.0
          box #content
            with
              w=fill
              h=fill
              bg=bg
              clip=true
            col w=fill h=fill
              if degraded
                ConnectionBanner status=status
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
                _
                  slot explorer

      slot palette
      slot bell
      // The huddle rides every screen, so it is a window-level layer like the
      // palette and the bell — not a prop on TitleBar. A titlebar seat would
      // widen TitleBar's signature, which a source guard in main.rs pins, and
      // it would land the pill on top of the status/bell cluster already there.
      slot huddle
