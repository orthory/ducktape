// The huddle surfaces: the channel-header controls, the titlebar pill it docks
// into, and the popped panel. All of them read one projection — the roster the
// chat module already keeps as `HuddleMember { user, node, joined_at }` — plus
// an elapsed string the caller formats with `mmss`.
//
// FOUR THINGS THE ARTIFACT DRAWS THAT THIS FILE DELIBERATELY DOES NOT:
//
// 1. MUTE and 2. SCREEN SHARE. The app has no `/v1/call/ws` client, so joining
//    a huddle appends a name to a consensus roster and opens no media session.
//    Both toggles would flip a bool nothing on any wire reads.
// 3. THE SPEAKING RING and the "X is speaking" line. `CallServerControl::
//    PeerBeacon` carries `muted`, `camera_on`, `sharing` — there is no
//    `speaking` field to derive a ring from.
// 4. THE PAGES-SUMMARY FOOTER. Nothing in the product turns a call into a page;
//    the strip would promise a document that never appears.
//
// The artifact's 5-bar wave glyph is the console's `success_dot` here: its bar
// heights are a function of the running second, and the frozen component props
// carry the elapsed time as a formatted string, not as a count.

// THE CHANNEL-HEADER LIVE PILL — shown only in the channel whose huddle you are
// in. The plate is the pop-out target; the ✕ beside it is its own button, which
// is how iced spells the artifact's `stopPropagation` on a nested control.
// NOTE: the frozen signature carries no roster, so the artifact's overlapping
// 18px face stack inside this pill cannot be drawn here (see the report).
component HuddleLivePill(name:str, elapsed:str)
  box #root bg=toast_bg r=9.0 pl=9.0 pr=10.0 pt=5.0 pb=5.0
    row gap=8.0 align=center
      button label=name @icon_action px-0px py-0px -> pop_huddle
        row gap=8.0 align=center
          PulseDot plate=6.0 tone="success"
          text "LIVE" size=10.5 wrap=none font=code_medium @text-toast_fg
          text elapsed size=10.5 wrap=none font=code_medium @text-toast_fg
          Icon name="popout" tone="caption" px=11.0
        active bg=transparent text=toast_fg border=transparent border-w=1.0 r=6.0
        hovered bg=ink_hover text=toast_fg
        pressed bg=ink_hover text=toast_fg
      box w=1.0 h=14.0 bg=panel_tile
        space w=1.0 h=1.0
      button label="Leave the huddle" w=20.0 h=20.0 @icon_action px-0px py-0px -> leave_huddle_here
        box w=fill h=fill align-x=center align-y=center
          text "✕" size=10.5 wrap=none font=code_medium @text-danger_soft
        active bg=transparent text=danger_soft border=transparent border-w=1.0 r=5.0
        hovered bg=strong_ink text=danger_soft
        pressed bg=strong_ink text=danger_soft

// THE START CONTROL — paper, hairline, the headset glyph. `headphones.svg` has
// shipped in the design crate since the icon adoption and this is its first
// call site. Shown when no huddle is running anywhere.
component HuddleStart()
  button #root label="Start a huddle" @icon_action px-9px py-5px -> join_huddle_submit
    row gap=7.0 align=center
      Icon name="headphones" tone="muted" px=14.0
      text "Huddle" size=12.0 wrap=none font=display @text-accent_fg
    active bg=surface text=accent_fg border=control_line border-w=1.0 r=9.0
    hovered bg=muted_bg text=accent_fg border=control_line_hover
    pressed bg=subtle text=accent_fg

// THE DOCKED PILL — the titlebar's, shown on every screen EXCEPT the huddle's
// own chat view, where the header pill above says the same thing louder.
component HuddleDockedPill(channel:str, elapsed:str)
  box #root r=8.0 shadow=shadow_toast shadow-y=2.0 shadow-blur=8.0 clip=true
    button label="Open the huddle window" @icon_action px-8px py-4px -> pop_huddle
      row gap=7.0 align=center
        PulseDot plate=6.0 tone="success"
        text channel size=10.5 wrap=none font=code_medium @text-toast_fg
        text elapsed size=10.5 wrap=none font=code_medium @text-caption
        Icon name="popout" tone="caption" px=11.0
      active bg=toast_bg text=toast_fg border=transparent border-w=1.0 r=8.0
      hovered bg=ink_hover text=toast_fg
      pressed bg=ink_hover text=toast_fg

// ONE PARTICIPANT, on the dark panel. The human/agent shape rule holds — circle
// vs rounded square — but the plates are the panel's own dark steps, not the
// paper avatar tokens, so `PrincipalAvatar` is deliberately not reused here.
// The width is fixed so exactly two tiles fit the 296px panel's wrapping row:
// 296 less its 1px border either side and its 13px insets leaves 268, and
// 128 + 8 + 128 is 264.
component HuddleTile(person:HuddleParticipant)
  box #root w=128.0 pl=8.0 pr=8.0 pt=12.0 pb=12.0 bg=ink_hover r=11.0
    col w=fill gap=6.0 align=center
      if person.is_agent
        box w=34.0 h=34.0 align-x=center align-y=center bg=accent_fg r=10.0
          text person.initials size=11.0 wrap=none font=code_medium @text-ink_soft
      if !person.is_agent
        box w=34.0 h=34.0 align-x=center align-y=center bg=panel_tile r=17.0
          text person.initials size=12.0 wrap=none font=display @text-ink_soft
      text person.label size=12.0 wrap=none font=medium @text-chevron_idle

// THE POPPED PANEL — 296px, pinned bottom-right by the caller's stack. Three
// bands: the traffic-light header with its dock button, the body (elapsed +
// roster grid + controls), and nothing else. See the file header for the four
// bands the artifact has that this one honestly refuses to draw.
component HuddlePanel(channel:str, elapsed:str, roster:[HuddleParticipant])
  box #root w=296.0 bg=toast_bg border=accent_fg border-w=1.0 r=15.0 clip=true shadow=shadow_modal shadow-y=30.0 shadow-blur=70.0
    col w=fill
      box w=fill pl=11.0 pr=11.0 pt=9.0 pb=9.0
        row w=fill gap=9.0 align=center
          row gap=5.0 align=center
            box w=8.0 h=8.0 bg=danger_dot r=4.0
              space w=1.0 h=1.0
            box w=8.0 h=8.0 bg=warning_dot r=4.0
              space w=1.0 h=1.0
            box w=8.0 h=8.0 bg=success_dot r=4.0
              space w=1.0 h=1.0
          text "Huddle ·" size=10.5 wrap=none font=code_medium @text-ink_soft
          text channel w=fill size=10.5 wrap=none font=code_medium @text-ink_soft
          button label="Dock the huddle window" @icon_action p-4px -> dock_huddle
            Icon name="collapse" tone="caption" px=12.0
            active bg=transparent text=caption border=transparent border-w=1.0 r=5.0
            hovered bg=ink_hover text=toast_fg
            pressed bg=ink_hover text=toast_fg
      box w=fill h=1.0 bg=accent_fg
        space w=1.0 h=1.0
      box w=fill pl=13.0 pr=13.0 pt=13.0 pb=11.0
        col w=fill gap=12.0
          row gap=8.0 align=center
            PulseDot plate=6.0 tone="success"
            text elapsed size=12.0 wrap=none font=code_semibold @text-toast_fg
          row w=fill gap=8.0 wrap wrap-gap=8.0
            for person in roster
              HuddleTile person=person
          row w=fill gap=7.0 align=center
            button label="Open the huddle channel" @icon_action px-11px py-0px -> huddle_go_channel
              box h=32.0 align-y=center
                row gap=3.0 align=center
                  text "open" size=12.0 wrap=none font=medium @text-chevron_idle
                  text "#" size=12.0 wrap=none font=medium @text-ink_soft
                  text channel size=12.0 wrap=none font=medium @text-chevron_idle
              active bg=ink_hover text=chevron_idle border=transparent border-w=1.0 r=9.0
              hovered bg=strong_ink text=toast_fg
              pressed bg=strong_ink text=toast_fg
            space w=fill
            button label="Leave the huddle" @icon_action px-13px py-0px -> leave_huddle_here
              box h=32.0 align-y=center
                text "Leave" size=12.0 wrap=none font=display @text-primary_fg
              active bg=danger_solid text=primary_fg border=transparent border-w=1.0 r=9.0
              hovered bg=danger_solid_hover text=primary_fg
              pressed bg=danger_solid_hover text=primary_fg

// "A CALL IS LIVE, BUT NOT HERE" — the channel-header affordance for the huddle
// running in another channel. Clicking jumps to it; the pulse dot is the same
// live mark the sidebar row wears.
component HuddleElsewhere(name:str)
  button #root label="Open the channel the huddle is in" @icon_action px-11px py-5px -> huddle_go_channel
    row gap=7.0 align=center
      PulseDot plate=6.0 tone="success"
      text "#" size=12.0 wrap=none font=display @text-hint
      text name size=12.0 wrap=none font=display @text-accent_fg
      text "· call in progress" size=12.0 wrap=none font=display @text-caption
    active bg=surface text=accent_fg border=control_line border-w=1.0 r=9.0
    hovered bg=muted_bg text=accent_fg border=control_line_hover
    pressed bg=subtle text=accent_fg
