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
//
// AND ONE THING THE ARTIFACT ALWAYS DRAWS THAT THESE SOMETIMES DO NOT: THE
// CLOCK. `elapsed` is measured from the local 1 Hz tick between the instant
// THIS process watched the join land and now — never from the roster row's
// `joined_at`, which is a block HEIGHT on a validator network. A process that
// finds itself already on the roster (a restart, or another device joining for
// the same key) has no start instant, and the honest render of an unknown
// duration is no duration: every `elapsed` here is empty-tolerant and the
// surface falls back to the bare LIVE mark rather than a plausible 00:00.

// THE CHANNEL-HEADER LIVE PILL — shown only in the channel whose huddle you are
// in. The plate is the pop-out target; the ✕ beside it is its own button, which
// is how iced spells the artifact's `stopPropagation` on a nested control.
// NOTE: the frozen signature carries no roster, so the artifact's overlapping
// 18px face stack inside this pill cannot be drawn here (see the report).
component HuddleLivePill(name:str, elapsed:str)
  emits
    pop_huddle
    leave_huddle_here
  box #root bg=toast_bg r=9.0 pl=9.0 pr=10.0 pt=5.0 pb=5.0
    row gap=8.0 align=center
      button label=name @icon_action px-0px py-0px -> emit(pop_huddle)
        row gap=8.0 align=center
          PulseDot plate=6.0 tone="success"
          text "LIVE" size=10.5 wrap=none font=code_medium @text-toast_fg
          if !empty(elapsed)
            text elapsed size=10.5 wrap=none font=code_medium @text-toast_fg
          Icon name="popout" tone="caption" px=11.0
        active bg=transparent text=toast_fg border=transparent border-w=1.0 r=6.0
        hovered bg=ink_hover text=toast_fg
        pressed bg=ink_hover text=toast_fg
      box w=1.0 h=14.0 bg=panel_tile
        space w=1.0 h=1.0
      button label="Leave the huddle" w=20.0 h=20.0 @icon_action px-0px py-0px -> emit(leave_huddle_here)
        box w=fill h=fill align-x=center align-y=center
          text "✕" size=10.5 wrap=none font=code_medium @text-danger_soft
        active bg=transparent text=danger_soft border=transparent border-w=1.0 r=5.0
        hovered bg=strong_ink text=danger_soft
        pressed bg=strong_ink text=danger_soft

// THE START CONTROL — paper, hairline, the headset glyph. `headphones.svg` has
// shipped in the design crate since the icon adoption and this is its first
// call site. Shown when no huddle is running anywhere.
component HuddleStart()
  emits
    join_huddle_submit
  button #root label="Start a huddle" @icon_action px-9px py-5px -> emit(join_huddle_submit)
    row gap=7.0 align=center
      Icon name="headphones" tone="muted" px=14.0
      text "Huddle" size=12.0 wrap=none font=display @text-accent_fg
    active bg=surface text=accent_fg border=control_line border-w=1.0 r=9.0
    hovered bg=muted_bg text=accent_fg border=control_line_hover
    pressed bg=subtle text=accent_fg

// THE DOCKED PILL — the titlebar's, shown on every screen EXCEPT the huddle's
// own chat view, where the header pill above says the same thing louder.
//
// NOT MOUNTED YET, and not blocked on a fact: `huddle_joined`, `huddle_popped`,
// `huddle_channel`, `huddle_channel_name` and `huddle_now - huddle_joined_at`
// are all in state.ice:263-268, and `pop_huddle` is a live handler
// (handlers/huddle.ice:21). What is missing is the titlebar SLOT — shell.ice's
// `TitleBar`/`WorkspaceTabs` carry no huddle props — so mounting is two props
// and one event on those two signatures plus the pass-through at view.ice:212.
// The artifact's visibility rule is `huddle_joined && !huddle_popped &&
// !(shell_tab == "chat" && huddle_channel == active_channel)`.
component HuddleDockedPill(channel:str, elapsed:str)
  emits
    pop_huddle
  box #root r=8.0 shadow=shadow_toast shadow-y=2.0 shadow-blur=8.0 clip=true
    button label="Open the huddle window" @icon_action px-8px py-4px -> emit(pop_huddle)
      row gap=7.0 align=center
        PulseDot plate=6.0 tone="success"
        text channel size=10.5 wrap=none font=code_medium @text-toast_fg
        if !empty(elapsed)
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
      // `is_you` is resolved against the same user bytes `signed_write` authors
      // with, so the self tile is marked with the 9px caption the artifact uses
      // for `you` in its member rows — the huddle grid otherwise renders four
      // identical tiles and never says which one is her.
      row gap=4.0 align=center
        text person.label size=12.0 wrap=none font=medium @text-chevron_idle
        if person.is_you
          text "you" size=9.0 wrap=none font=medium @text-caption

// THE POPPED PANEL — 296px, pinned bottom-right by the caller's stack. Three
// bands: the traffic-light header with its dock button, the body (elapsed +
// roster grid + controls), and nothing else. See the file header for the four
// bands the artifact has that this one honestly refuses to draw.
//
// NOT MOUNTED YET, and it is the ONLY consumer of `huddle_popped` — the LIVE
// pill above is already on screen at view.ice:328 and its `pop_huddle` click
// currently changes nothing anyone can see. Mounting needs one
// `huddle_roster:[HuddleParticipant]` held in app state: `next.huddle_roster`
// already reaches handlers/chat.ice:268 and is dropped the instant
// `huddle_self` has read it. It must be kept ONLY while `active_channel ==
// huddle_channel`, the same guard `huddle_channel` itself carries — a load of
// any other channel carries THAT channel's roster, and this panel follows you
// off the huddle's channel. Then a `pin`ned mount in view.ice's full-window
// stack under `if huddle_joined && huddle_popped`.
component HuddlePanel(channel:str, elapsed:str, roster:[HuddleParticipant])
  emits
    dock_huddle
    huddle_go_channel
    leave_huddle_here
  box #root w=296.0 bg=toast_bg border=accent_fg border-w=1.0 r=15.0 clip=true shadow=shadow_modal shadow-y=30.0 shadow-blur=70.0
    col w=fill
      box w=fill pl=11.0 pr=11.0 pt=9.0 pb=9.0
        row w=fill gap=9.0 align=center
          // The artifact draws three static traffic lights. In a real window a
          // red dot that eats the click is a trap, and this panel has exactly
          // one way to close — docking it — so the red dot IS that control and
          // the two beside it stay the chrome they are drawn as.
          row gap=5.0 align=center
            button label="Dock the huddle window" w=8.0 h=8.0 @icon_action px-0px py-0px -> emit(dock_huddle)
              space w=8.0 h=8.0
              active bg=danger_dot text=danger_dot border=transparent border-w=1.0 r=4.0
              hovered bg=danger_solid text=danger_solid
              pressed bg=danger_solid_hover text=danger_solid_hover
            box w=8.0 h=8.0 bg=warning_dot r=4.0
              space w=1.0 h=1.0
            box w=8.0 h=8.0 bg=success_dot r=4.0
              space w=1.0 h=1.0
          text "Huddle ·" size=10.5 wrap=none font=code_medium @text-ink_soft
          text channel w=fill size=10.5 wrap=none font=code_medium @text-ink_soft
          button label="Dock the huddle window" @icon_action p-4px -> emit(dock_huddle)
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
            if !empty(elapsed)
              text elapsed size=12.0 wrap=none font=code_semibold @text-toast_fg
            if empty(elapsed)
              text "LIVE" size=12.0 wrap=none font=code_semibold @text-toast_fg
          row w=fill gap=8.0 wrap wrap-gap=8.0
            for person in roster
              HuddleTile person=person
          row w=fill gap=7.0 align=center
            button label="Open the huddle channel" @icon_action px-11px py-0px -> emit(huddle_go_channel)
              box h=32.0 align-y=center
                row gap=3.0 align=center
                  text "open" size=12.0 wrap=none font=medium @text-chevron_idle
                  text "#" size=12.0 wrap=none font=medium @text-ink_soft
                  text channel size=12.0 wrap=none font=medium @text-chevron_idle
              active bg=ink_hover text=chevron_idle border=transparent border-w=1.0 r=9.0
              hovered bg=strong_ink text=toast_fg
              pressed bg=strong_ink text=toast_fg
            space w=fill
            button label="Leave the huddle" @icon_action px-13px py-0px -> emit(leave_huddle_here)
              box h=32.0 align-y=center
                text "Leave" size=12.0 wrap=none font=display @text-primary_fg
              active bg=danger_solid text=primary_fg border=transparent border-w=1.0 r=9.0
              hovered bg=danger_solid_hover text=primary_fg
              pressed bg=danger_solid_hover text=primary_fg

// "A CALL IS LIVE, BUT NOT HERE" — the channel-header affordance for the huddle
// running in another channel. Clicking jumps to it; the pulse dot is the same
// live mark the sidebar row wears.
component HuddleElsewhere(name:str)
  emits
    huddle_go_channel
  button #root label="Open the channel the huddle is in" @icon_action px-11px py-5px -> emit(huddle_go_channel)
    row gap=7.0 align=center
      PulseDot plate=6.0 tone="success"
      text "#" size=12.0 wrap=none font=display @text-hint
      text name size=12.0 wrap=none font=display @text-accent_fg
      text "· call in progress" size=12.0 wrap=none font=display @text-caption
    active bg=surface text=accent_fg border=control_line border-w=1.0 r=9.0
    hovered bg=muted_bg text=accent_fg border=control_line_hover
    pressed bg=subtle text=accent_fg
