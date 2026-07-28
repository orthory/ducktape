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
  box #root w=fill pl=8.0 pr=8.0
    col w=fill
      if selected
        button label=peer.name w=fill p=0.0 @icon_action -> emit(choose_dm, peer.key)
          box w=fill pl=10.0 pr=10.0 pt=6.0 pb=6.0
            DmRow peer=peer selected=true
          active bg=subtle text=fg border=transparent border-w=1.0 r=7.0
          hovered bg=subtle text=fg
          pressed bg=rail_hover text=fg
      if !selected
        button label=peer.name w=fill p=0.0 @icon_action -> emit(choose_dm, peer.key)
          box w=fill pl=10.0 pr=10.0 pt=6.0 pb=6.0
            DmRow peer=peer selected=false
          active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
          hovered bg=rail_hover text=fg
          pressed bg=subtle text=fg

component DmRow(peer:DmPeer, selected:bool)
  row #root w=fill gap=8.0 align=center
    PrincipalAvatar initials=peer.initials is_agent=peer.is_agent plate=18.0 ink=8.0 ring=""
    if selected
      text peer.name w=fill size=13.0 wrap=none @text-fg
    if !selected
      text peer.name w=fill size=13.0 wrap=none @text-muted
    if peer.is_agent
      // NOT the AGENT badge: that one is ink-on-white at 9px. This is the
      // hairline-grey chip the artifact reserves for the sidebar.
      box px=4.0 py=1.0 bg=label r=3.0
        text "AI" size=9.0 wrap=none font=code_semibold @text-primary_fg

// The chat header's DM identity — a 24px peer plate and the peer's name, in
// place of the `# channel` title. The AGENT badge marks a machine peer.
//
// MOUNTED in the chat header (view.ice), replacing `text "#"` + `text
// active_channel_name` under `if !empty(active_dm_peer)`, with those two lines
// kept for the `if empty(active_dm_peer)` half. It needs NO new backend fn and
// no new state: `active_dm_peer` is written by `choose_dm` and `dm_peers`
// already reaches the view, so the peer is a filter — `for peer in dm_peers` /
// `if peer.key == active_dm_peer`.
//
// `active_dm_name`/`active_dm_is_agent` were NOT the route and are DELETED from
// state.ice: no handler anywhere wrote either one, so they were dead
// declarations rather than data, and feeding this plate from them would have
// rendered a blank name over a permanently human avatar.
//
// A peer who is not on the identity roster matches no row and this draws
// nothing; the header then falls through to the `#` title, which for a DM is
// the derived two-party name. An empty plate is never the render.
component DmHeader(peer:DmPeer)
  row #root gap=9.0 align=center
    PrincipalAvatar initials=peer.initials is_agent=peer.is_agent plate=24.0 ink=9.0 ring=""
    text peer.name size=14.0 wrap=none font=display @text-fg
    if peer.is_agent
      box px=5.0 py=2.0 bg=primary r=4.0
        text "AGENT" size=9.0 wrap=none font=code_semibold @text-primary_fg
