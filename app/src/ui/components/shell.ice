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
  box #root w=30.0 h=30.0 align-x=center align-y=center bg=primary r=9.0
    text "D" size=14.0 wrap=none font=code_semibold @text-toast_fg

// The chain chip left of the network name — a 15px ink plate with the mark.
component NetworkChip(name:str)
  row #root gap=7.0 align=center
    box w=15.0 h=15.0 align-x=center align-y=center bg=primary r=4.0
      text "◆" size=7.5 wrap=none font=code_semibold @text-toast_fg
    text name size=11.5 wrap=none font=display @text-accent_fg

// The one dot that carries connection state, at both the pill's 6px and the
// status card's 7px. A stopped node wears `alert_dot`, never the traffic-light
// red — that hex belongs to the window's close button and nothing else.
component StatusDot(degraded:bool, loading:bool, plate:f64)
  col #root
    if degraded
      box w=plate h=plate bg=alert_dot r=(plate / 2.0)
        space w=1.0 h=1.0
    if !degraded && loading
      box w=plate h=plate bg=warning_dot r=(plate / 2.0)
        space w=1.0 h=1.0
    if !degraded && !loading
      box w=plate h=plate bg=success_dot r=(plate / 2.0)
        space w=1.0 h=1.0

// The pill says ONE state word. The height it used to print twice now appears
// once, inside the card the pill opens.
component StatusPill(degraded:bool, loading:bool)
  box #root px=8.0 py=3.0 bg=surface border=border border-w=1.0 r=7.0
    row gap=5.0 align=center
      StatusDot degraded=degraded loading=loading plate=6.0
      if degraded
        text "Stopped" size=10.5 wrap=none font=code_medium @text-input
      if !degraded && loading
        text "Syncing…" size=10.5 wrap=none font=code_medium @text-input
      if !degraded && !loading
        text "Synced" size=10.5 wrap=none font=code_medium @text-input

// The 284px card behind the pill: what this node knows about the chain it is
// standing on. Every value comes off /v1/status through `load_node_facts`.
//
// OMITTED, not faked: the `gossip` row (NodeFacts carries peers_live/total but
// no state field holds them) and the 26-bar sparkline (it needs the newest 26
// of a 100-block window with a clamped bar height, and neither a slice nor a
// clamp exists as a helper). Both are named in the handoff report.
component StatusCard(degraded:bool, loading:bool, height:i64, tier:str, root_hash:str, consensus_view:i64, quorum:i64, reachable:i64, last_finalized:i64, checkpoint:i64)
  box #root w=284.0 pl=14.0 pr=14.0 pt=13.0 pb=13.0
    col w=fill gap=11.0
      row w=fill gap=7.0 align=center
        StatusDot degraded=degraded loading=loading plate=7.0
        if degraded
          text "Stopped" size=12.0 wrap=none font=display @text-primary
        if !degraded && loading
          text "Syncing…" size=12.0 wrap=none font=display @text-primary
        if !degraded && !loading
          text "Synced" size=12.0 wrap=none font=display @text-primary
        space w=fill
        text tier size=10.5 wrap=none font=code_medium @text-meta
      text height_label(height) size=14.0 wrap=none font=code_semibold @text-primary
      col w=fill gap=6.0
        row w=fill gap=14.0 align=center
          text "app-hash" size=10.5 wrap=none font=code_medium @text-hint
          box w=fill clip=true
            row w=fill
              space w=fill
              text root_hash size=10.5 wrap=none font=code_medium @text-secondary_fg
        row w=fill gap=14.0 align=center
          text "finality" size=10.5 wrap=none font=code_medium @text-hint
          space w=fill
          row gap=4.0 align=center
            text "view" size=10.5 wrap=none font=code_medium @text-secondary_fg
            text consensus_view size=10.5 wrap=none font=code_medium @text-secondary_fg
            text "·" size=10.5 wrap=none font=code_medium @text-hint
            text reachable size=10.5 wrap=none font=code_medium @text-secondary_fg
            text "/" size=10.5 wrap=none font=code_medium @text-hint
            text quorum size=10.5 wrap=none font=code_medium @text-secondary_fg
            text "certs" size=10.5 wrap=none font=code_medium @text-secondary_fg
        row w=fill gap=14.0 align=center
          text "last block" size=10.5 wrap=none font=code_medium @text-hint
          space w=fill
          text relative_time(last_finalized) size=10.5 wrap=none font=code_medium @text-secondary_fg
        row w=fill gap=14.0 align=center
          text "duckfs gc" size=10.5 wrap=none font=code_medium @text-hint
          space w=fill
          text height_label(checkpoint) size=10.5 wrap=none font=code_medium @text-secondary_fg
      col w=fill gap=9.0
        box w=fill h=1.0 bg=separator
          space w=1.0 h=1.0
        text "This node verifies every record itself · it takes no one's word for it" w=fill size=12.5 @text-meta

// The badge over the bell: 13px tall, ringed 1.5px in the bar's own paper so
// the plate reads as a badge and not as ink on the bell. Its fill is the WORST
// UNREAD SEVERITY, never a fixed accent — three ALERTs and three INFOs are not
// the same news, and the count alone cannot say which one you are looking at.
// `plate` is the width the digit run needs; `sev` is the one discriminant.
component BellBadge(count:i64, sev:str, plate:f64)
  col #root
    match sev
      "alert"
        box w=plate h=13.0 align-x=center align-y=center bg=alert_dot border=surface border-w=1.5 r=7.0
          text count size=9.0 wrap=none font=code_semibold @text-brand_fg
      "warn"
        box w=plate h=13.0 align-x=center align-y=center bg=warning_dot border=surface border-w=1.5 r=7.0
          text count size=9.0 wrap=none font=code_semibold @text-brand_fg
      _
        box w=plate h=13.0 align-x=center align-y=center bg=info_dot border=surface border-w=1.5 r=7.0
          text count size=9.0 wrap=none font=code_semibold @text-brand_fg

// 40px: a 39px bar over its 1px rule. During onboarding the bar keeps only the
// window controls — the artifact drops the chip and the whole right cluster
// until a workspace exists, and draws no title in their place.
component TitleBar(phase:str, network:str, height:i64, loading:bool, degraded:bool, bell_badge:i64, bell_sev:str, tier:str, root_hash:str, consensus_view:i64, quorum:i64, reachable:i64, last_finalized:i64, checkpoint:i64)
  emits
    toggle_bell
  col #root w=fill
    box w=fill h=39.0 px=13.0 bg=elevated
      row w=fill h=fill gap=13.0 align=center
        match phase
          "console"
            NetworkChip name=network
          _
            space w=1.0 h=1.0
        space w=fill
        match phase
          "console"
            row gap=6.0 align=center
              tooltip position=bottom gap=6.0 p=0.0 delay=90 bg=surface border=border border-w=1.0 r=13.0 shadow=shadow_modal shadow-y=16.0 shadow-blur=40.0
                StatusPill degraded=degraded loading=loading
                StatusCard degraded=degraded loading=loading height=height tier=tier root_hash=root_hash consensus_view=consensus_view quorum=quorum reachable=reachable last_finalized=last_finalized checkpoint=checkpoint
              stack w=26.0 h=24.0
                button label="Alerts" p=5.0 @icon_action -> emit(toggle_bell)
                  col align=center
                    if bell_badge > 0
                      Icon name="bell" tone="strong-ink" px=15.0
                    if bell_badge <= 0
                      Icon name="bell" tone="label" px=15.0
                  active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                  hovered bg=surface text=fg border=transparent
                  pressed bg=subtle
                // The badge is right-anchored: `pin` takes x/y only, so each
                // width branch names the x that keeps its right edge on the
                // artifact's line.
                if bell_badge > 0 && bell_badge < 10
                  pin x=12.0 y=1.0
                    BellBadge count=bell_badge sev=bell_sev plate=13.0
                if bell_badge > 9
                  pin x=7.0 y=1.0
                    BellBadge count=bell_badge sev=bell_sev plate=18.0
          _
            space w=1.0 h=1.0
    box w=fill h=1.0 bg=border
      space w=1.0 h=1.0

// The artifact has no banner: a degraded node speaks through the status dot and
// an ALERT row in the bell. Neither carries the node's own words yet, so the
// band stays — restyled onto the alert family so it reads as one severity
// vocabulary with the dot above it rather than as a second red language.
component ConnectionBanner(status:str)
  box #root w=fill h=30.0 pl=14.0 pr=14.0 bg=alert_bg border=alert_line border-w=1.0
    row w=fill h=fill gap=8.0 align=center
      box w=7.0 h=7.0 bg=alert_dot r=3.5
        space w=1.0 h=1.0
      text "Connection degraded" size=13.0 wrap=none font=medium @text-alert_fg
      text status w=fill size=13.0 @text-caption

// One rail entry: a 58px capsule holding a 19px glyph over a 9.5px label. The
// selected state is a `subtle` tint capsule, never a second sheet of glass.
// Forge alone wears a live dot while an agent is running.
component RailButton(item:NavItem)
  emits
    select_shell_tab(str)
  stack #root w=58.0
    if item.active
      button label=item.title w=58.0 p=0.0 @icon_action -> emit(select_shell_tab, item.id)
        col w=fill py=4.0 gap=4.0 align=center
          Icon name=item.icon tone="ink" px=19.0
          text item.title size=9.5 wrap=none font=display @text-strong_ink
        active bg=subtle text=fg border=transparent border-w=1.0 r=10.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg
    if !item.active
      button label=item.title w=58.0 p=0.0 @icon_action -> emit(select_shell_tab, item.id)
        col w=fill py=4.0 gap=4.0 align=center
          Icon name=item.icon tone="idle" px=19.0
          text item.title size=9.5 wrap=none font=display @text-caption
        active bg=transparent text=muted border=transparent border-w=1.0 r=10.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg
    if item.live
      pin x=36.0 y=6.0
        box p=1.5 bg=rail r=5.0
          PulseDot plate=7.0 tone="success"
    // The badge is right-anchored: `pin` takes x/y only, so each width branch
    // names the x that puts its right edge on the artifact's 47px line.
    if item.badge > 0 && item.badge < 10
      pin x=32.0 y=6.0
        box w=15.0 h=15.0 align-x=center align-y=center bg=brand border=rail border-w=1.5 r=8.0
          text item.badge size=9.0 wrap=none font=code_semibold @text-brand_fg
    if item.badge > 9
      pin x=25.0 y=6.0
        box w=22.0 h=15.0 align-x=center align-y=center bg=brand border=rail border-w=1.5 r=8.0
          text item.badge size=9.0 wrap=none font=code_semibold @text-brand_fg

component NavRail(tab:str, approvals:i64, account:str, agent_live:bool)
  emits
    select_shell_tab(str)
  box #root w=74.0 h=fill pt=13.0 pb=10.0 bg=rail clip=true
      col w=fill h=fill gap=0.0 align=center
        Brand
        space w=1.0 h=9.0
        scroll dir=vertical w=fill h=fill bar=hidden
          col w=fill gap=2.0 align=center
            for item in shell_nav(tab, approvals, agent_live)
              RailButton item=item
                forward
                  select_shell_tab
        if tab == "settings"
          button label="Settings" p=8.0 @icon_action -> emit(select_shell_tab, "settings")
            Icon name="gear" tone="ink" px=18.0
            active bg=subtle text=fg border=transparent border-w=1.0 r=9.0
            hovered bg=rail_hover text=fg
            pressed bg=subtle text=fg
        if tab != "settings"
          button label="Settings" p=8.0 @icon_action -> emit(select_shell_tab, "settings")
            Icon name="gear" tone="idle" px=18.0
            active bg=transparent text=muted border=transparent border-w=1.0 r=9.0
            hovered bg=rail_hover text=fg
            pressed bg=subtle text=fg
        space w=1.0 h=6.0
        // The avatar is the one thing hung below the footer button: a 1.5px
        // paper ring inside a 1px hairline halo, which no other person plate in
        // the app wears.
        button label="Account" p=0.0 @icon_action -> emit(select_shell_tab, "settings")
          box p=1.0 bg=pending_line r=16.5
            PrincipalAvatar initials=initial_of(account) is_agent=false plate=28.0 ink=10.0 ring="paper"
          active bg=transparent text=muted border=transparent border-w=1.0 r=16.5
          hovered bg=rail_hover text=fg
          pressed bg=subtle text=fg

// The header every screen sidebar wears: a 13.5px title, a machine count, and
// an optional trailing control the caller fills.
//
// NOT MOUNTED YET, and every call site is in view.ice. The Pages sidebar head
// is already this component spelled out by hand — same pl/pr 14, pt 14, pb 11,
// same 8.0 row gap, same title and count steps, same separator rule — with the
// New-page button in what is this component's slot; it collapses to
// `SidebarHeader title="Pages" count=len(pages)` with that button as the slot
// child. The Chat sidebar head is the same geometry but interleaves a
// connection dot BETWEEN the title and the count, which this signature cannot
// express (the slot is past the `space w=fill`), so chat either keeps its
// hand-rolled head or the dot moves to the trailing slot — a design call, not
// a mechanical one. The Files screen has no sidebar at all yet.
component SidebarHeader(title:str, count:i64)
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

component WorkspaceTabs(network:str, status:str, height:i64, loading:bool, degraded:bool, tab:str, bell_count:i64, bell_sev:str, approvals:i64, account:str, agent_live:bool, phase:str, tier:str, root_hash:str, consensus_view:i64, quorum:i64, reachable:i64, last_finalized:i64, checkpoint:i64)
  emits
    select_shell_tab(str)
    toggle_bell
  box w=fill h=fill clip=true bg=bg px-snap=true
    stack w=fill h=fill
      col w=fill h=fill
        TitleBar phase=phase network=network height=height loading=loading degraded=degraded bell_badge=bell_count bell_sev=bell_sev tier=tier root_hash=root_hash consensus_view=consensus_view quorum=quorum reachable=reachable last_finalized=last_finalized checkpoint=checkpoint #titlebar
          forward
            toggle_bell
        if degraded
          ConnectionBanner status=status
        row w=fill h=fill
          NavRail tab=tab approvals=approvals account=account agent_live=agent_live #rail
            forward
              select_shell_tab
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
                _
                  slot explorer

      slot palette
      slot bell
      // The huddle rides every screen, so it is a window-level layer like the
      // palette and the bell — not a prop on TitleBar. A titlebar seat would
      // widen TitleBar's signature, which a source guard in main.rs pins, and
      // it would land the pill on top of the status/bell cluster already there.
      slot huddle
