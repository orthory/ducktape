// The DIRECT plane: the sidebar row for one peer, and the identity that
// replaces the `#` channel title in the chat header when a DM is open.
//
// NO PRESENCE DOT, anywhere. The artifact hangs a 7px status dot on the row and
// a 6px one in the header, coloured by `memberStatusColor`. Nothing in this
// product publishes presence — there is no `last_seen`, no heartbeat, and
// `PeerRow.live` is a NODE fact, not a person's. A dot that is always the same
// colour is a lie with a hex code, so `DmPeer` carries no status and neither
// surface draws one.
//
// One DIRECT row: 18px avatar, the peer's name at 400 weight, and — for a
// machine peer — the small grey AI chip. Deliberately lighter than a channel
// row, which is 500 weight on 7px of vertical padding. `choose_dm` resolves or
// creates the two-party channel, so the peer list itself is the entry point and
// the DIRECT eyebrow needs no `+`, exactly as in the artifact.
component DmButton(peer:DmPeer, selected:bool)
  emits
    choose_dm(str)
  box #root
    with
      w=fill
      pl=8.0
      pr=8.0
    col w=fill
      if selected
        button -> emit(choose_dm, peer.key)
          with
            label=peer.name
            w=fill
            p=0.0
            @icon_action
          box
            with
              w=fill
              pl=10.0
              pr=10.0
              pt=6.0
              pb=6.0
            DmRow peer=peer selected=true
          active bg=selected_row text=fg border=transparent border-w=1.0 r=7.0
          hovered bg=selected_row text=fg
          pressed bg=rail_hover text=fg
      if !selected
        button -> emit(choose_dm, peer.key)
          with
            label=peer.name
            w=fill
            p=0.0
            @icon_action
          box
            with
              w=fill
              pl=10.0
              pr=10.0
              pt=6.0
              pb=6.0
            DmRow peer=peer selected=false
          active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
          hovered bg=rail_hover text=fg
          pressed bg=subtle text=fg

component DmRow(peer:DmPeer, selected:bool)
  row #root
    with
      w=fill
      gap=8.0
      align=center
    PrincipalAvatar
      with
        initials=peer.initials
        is_agent=peer.is_agent
        plate=18.0
        ink=8.0
        ring=""
    if selected
      box w=fill clip=true
        text peer.name
          with
            size=13.0
            wrap=none
            @text-fg
    if !selected
      box w=fill clip=true
        text peer.name
          with
            size=13.0
            wrap=none
            @text-muted
    if peer.is_agent
      // NOT the AGENT badge: that one is ink-on-white at 9px. This is the
      // hairline-grey chip the artifact reserves for the sidebar.
      box
        with
          px=4.0
          py=1.0
          bg=label
          r=3.0
        text "AI"
          with
            size=9.0
            wrap=none
            font=code_semibold
            @text-primary_fg

// The chat header's DM identity — a 24px peer plate and the peer's name, in
// place of the `# channel` title. The AGENT badge marks a machine peer.
//
// MOUNTED in the chat header (view.ice), replacing `text "#"` + `text
// active_channel_name` under `if !empty(active_dm_peer)`, with those two lines
// kept for the `if empty(active_dm_peer)` half.
//
// FED FROM `active_dm`, a whole PEER mirrored into state — not from a
// `active_dm_name`/`active_dm_is_agent` pair, which is the shape that was once
// declared, never written, and deleted for it. It is written by the same
// statement lists that write `active_dm_peer`, from the key they just decided,
// and a lint pins the pair (`every_writer_of_a_mirrored_view_reading_...`).
// This replaced a FILTER — `for peer in dm_peers` / `if peer.key ==
// active_dm_peer`, because Ice cannot index a list by field — which deep-cloned
// every peer and allocated a scope String per row, every frame, so that one of
// them drew.
//
// A peer who is not on the identity roster resolves to the blank row, whose
// empty name and initials draw nothing; the header then falls through to the
// `#` title, which for a DM is the derived two-party name. An empty plate is
// never the render.
component DmHeader(peer:DmPeer)
  row #root gap=9.0 align=center
    PrincipalAvatar
      with
        initials=peer.initials
        is_agent=peer.is_agent
        plate=24.0
        ink=9.0
        ring=""
    text peer.name
      with
        size=14.0
        wrap=none
        font=display
        @text-fg
    if peer.is_agent
      box
        with
          px=5.0
          py=2.0
          bg=primary
          r=4.0
        text "AGENT"
          with
            size=9.0
            wrap=none
            font=code_semibold
            @text-primary_fg
