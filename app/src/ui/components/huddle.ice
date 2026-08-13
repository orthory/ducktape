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
component HuddleLivePill(name:str, elapsed:str, muted:bool, popped:bool)
  emits
    pop_huddle
    focus_huddle
    leave_huddle_here
  box #root
    with
      bg=toast_bg
      r=9.0
      pl=9.0
      pr=10.0
      pt=5.0
      pb=5.0
    row gap=8.0 align=center
      // Two arms, one face: while the huddle window is up, the pill's click
      // RAISES it instead of trying to open a second one (which a branch-free
      // handler could only swallow as a no-op).
      if !popped
        button -> emit(pop_huddle)
          with
            label=name
            @icon_action
            @px-0px
            @py-0px
          row gap=8.0 align=center
            PulseDot plate=6.0 tone="success"
            text "LIVE"
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-toast_fg
            if !empty(elapsed)
              text elapsed
                with
                  size=10.5
                  wrap=none
                  font=code_medium
                  @text-toast_fg
            if muted
              Icon
                with
                  name="mic-off"
                  tone="caption"
                  px=11.0
            Icon
              with
                name="popout"
                tone="caption"
                px=11.0
          active bg=transparent text=toast_fg border=transparent border-w=1.0 r=6.0
          hovered bg=ink_hover text=toast_fg
          pressed bg=ink_hover text=toast_fg
      if popped
        button -> emit(focus_huddle)
          with
            label="Focus the huddle window"
            @icon_action
            @px-0px
            @py-0px
          row gap=8.0 align=center
            PulseDot plate=6.0 tone="success"
            text "LIVE"
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-toast_fg
            if !empty(elapsed)
              text elapsed
                with
                  size=10.5
                  wrap=none
                  font=code_medium
                  @text-toast_fg
            if muted
              Icon
                with
                  name="mic-off"
                  tone="caption"
                  px=11.0
            Icon
              with
                name="popout"
                tone="caption"
                px=11.0
          active bg=transparent text=toast_fg border=transparent border-w=1.0 r=6.0
          hovered bg=ink_hover text=toast_fg
          pressed bg=ink_hover text=toast_fg
      box
        with
          w=1.0
          h=14.0
          bg=panel_tile
        space w=1.0 h=1.0
      button -> emit(leave_huddle_here)
        with
          label="Leave the huddle"
          w=24.0
          h=24.0
          @icon_action
          @px-0px
          @py-0px
        box
          with
            w=fill
            h=fill
            align-x=center
            align-y=center
          text "✕"
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-danger_soft
        active bg=transparent text=danger_soft border=transparent border-w=1.0 r=5.0
        hovered bg=strong_ink text=danger_soft
        pressed bg=strong_ink text=danger_soft

// THE START CONTROL — paper, hairline, the headset glyph. `headphones.svg` has
// shipped in the design crate since the icon adoption and this is its first
// call site. Shown when no huddle is running anywhere.
component HuddleStart()
  emits
    join_huddle_submit
  button #root -> emit(join_huddle_submit)
    with
      label="Start a huddle"
      @icon_action
      @px-9px
      @py-5px
    row gap=7.0 align=center
      Icon
        with
          name="headphones"
          tone="muted"
          px=14.0
      text "Huddle"
        with
          size=12.0
          wrap=none
          font=display
          @text-accent_fg
    active bg=surface text=accent_fg border=control_line border-w=1.0 r=9.0
    hovered bg=muted_bg text=accent_fg border=control_line_hover
    pressed bg=subtle text=accent_fg

// THE DOCKED PILL — the titlebar's, shown on every screen EXCEPT the huddle's
// own chat view, where the header pill above says the same thing louder.
//
// MOUNTED in the window-level `huddle` slot (view.ice), not on the titlebar:
// `TitleBar`/`WorkspaceTabs` carry no huddle props, and the slot already sits
// above the whole console, so it needs no signature change. The slot anchors
// bottom-right; this component draws no offset of its own.
// The visibility rule is `huddle_joined && huddle_win == none &&
// !(shell_tab == ShellTab.chat && huddle_channel == active_channel)` — the pill is
// what the huddle docks BACK into when its window closes.
component HuddleDockedPill(channel:str, elapsed:str)
  emits
    pop_huddle
  box #root
    with
      r=8.0
      shadow=shadow_toast
      shadow-y=2.0
      shadow-blur=8.0
      clip=true
    button -> emit(pop_huddle)
      with
        label="Open the huddle window"
        @icon_action
        @px-8px
        @py-4px
      row gap=7.0 align=center
        PulseDot plate=6.0 tone="success"
        text channel
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-toast_fg
        if !empty(elapsed)
          text elapsed
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-caption
        Icon
          with
            name="popout"
            tone="caption"
            px=11.0
      active bg=toast_bg text=toast_fg border=transparent border-w=1.0 r=8.0
      hovered bg=ink_hover text=toast_fg
      pressed bg=ink_hover text=toast_fg

// ONE PARTICIPANT. The human/agent shape rule — circle vs rounded square — is
// `PrincipalAvatar`'s, and this file reuses it now that the panel is a normal
// themed surface rather than a dark plate: the hand-drawn 34px steps this
// component used to carry were the SAME two shapes in the panel's own inks,
// and a second copy of a shape rule is a second thing to keep in step.
//
// NO FIXED WIDTH. The tile fills its grid cell and the panel's
// `grid min-cell=` owns the column count, so one roster renders 2 tiles at the
// window's 320px minimum and 3-4 on a widened window instead of stranding
// 400px of background beside a 128px card.
component HuddleTile(person:HuddleParticipant, muted:bool)
  // `h=fill` + `align-y=center` because the grid gives every cell the same
  // SQUARE box: without them the avatar/name pair clings to the top edge and
  // each tile carries a third of itself as empty plate.
  box #root
    with
      w=fill
      h=fill
      pl=8.0
      pr=8.0
      pt=12.0
      pb=12.0
      align-y=center
      bg=elevated
      border=card_line
      border-w=1.0
      r=11.0
    col
      with
        w=fill
        gap=7.0
        align=center
      PrincipalAvatar
        with
          initials=person.initials
          is_agent=person.is_agent
          plate=34.0
          ink=13.0
          ring=""
      text person.label
        with
          size=12.0
          wrap=none
          font=medium
          @text-fg
      // `is_you` is resolved against the same user bytes `signed_write` authors
      // with, so the self tile is marked with the 9px caption the artifact uses
      // for `you` in its member rows — the huddle grid otherwise renders four
      // identical tiles and never says which one is her. It rides its OWN line
      // under the name with the mute mark: a 116px cell cannot hold a label,
      // an icon and a badge on one row without pushing one of them out.
      if (person.is_you || muted)
        row gap=4.0 align=center
          if muted
            Icon
              with
                name="mic-off"
                tone="danger"
                px=10.0
          if person.is_you
            text "you"
              with
                size=9.0
                wrap=none
                font=medium
                @text-caption

// THE POPPED PANEL — the whole content of the huddle's own OS window, and the
// one surface here that is a WINDOW rather than a pill. Three bands, and the
// middle one takes the fill:
//
//   header (live mark · clock · #channel · dock)   — shrink, on `sidebar`
//   [status strip, only when the call says something the clock does not]
//   stage (video strip + roster grid), SCROLLING   — h=fill, on `bg`
//   controls (mic · camera · open · Leave)         — shrink, on `sidebar`
//
// It used to be one top-anchored shrink column inside a `clip=true` box: a
// 320x460 window spent its bottom 250px on empty background, a 560x700 one
// spent 500, and a roster too tall for the panel pushed the controls THROUGH
// the clip — a six-person huddle you could not leave. Height is the whole fix:
// exactly one band carries `h=fill`, and it is the one with the people in it.
//
// The panel wears the app's own light/dark surfaces (`bg`/`sidebar`/`elevated`)
// and NOT the `toast_*` pair it used to. Those tokens are deliberately the
// INVERSE of the surface — right for a pill floating over the console, wrong
// for a whole window: in dark mode the huddle came up as a cream panel with
// near-invisible `ink_hover` tiles while every other window went dark.
//
// See the file header for the four bands the artifact has that this one
// honestly refuses to draw.
//
// IT DRAWS NO WINDOW CHROME. It used to be a 296px card pinned in the
// console's corner, wearing three hand-drawn traffic lights as costume; it is
// a real window now (`window huddle` in app.ice), so the frame, the shadow and
// the dots are the OS's and the close button docks. The `collapse` button
// stays because closing this window does NOT leave the huddle, and a control
// that says so is worth one glyph.
//
// It reads `huddle_roster` from app state, which is kept ONLY while
// `active_channel == huddle_channel` — the same guard `huddle_channel` itself
// carries, since a load of any other channel carries THAT channel's roster.
component HuddlePanel(channel:str, elapsed:str, rows:[HuddleTileRow], status:str, muted:bool, camera:bool, video_live:bool)
  emits
    dock_huddle
    huddle_go_channel
    leave_huddle_here
    toggle_call_mute
    toggle_call_camera
  // No `bg=` on the root: the window's own background IS the app background
  // (`bg app_background` in app.ice), so the stage band is the bare window and
  // only the two chrome bands paint a plate over it.
  col #root
    with
      w=fill
      h=fill
    // BAND 1 — THE HEADER, and the ONLY place the clock and the channel are
    // named. It used to be two bands: a mono "Huddle · <channel>" title over a
    // separate clock row whose right end printed the word `live` beside a
    // pulse dot that already said it. One row, read left to right: the live
    // mark, the running clock, whose channel, dock.
    box
      with
        w=fill
        pl=13.0
        pr=9.0
        pt=9.0
        pb=9.0
        bg=sidebar
      row
        with
          w=fill
          gap=8.0
          align=center
        PulseDot plate=7.0 tone="success"
        if !empty(elapsed)
          text elapsed
            with
              size=13.0
              wrap=none
              font=code_semibold
              @text-fg
        if empty(elapsed)
          text "LIVE"
            with
              size=13.0
              wrap=none
              font=code_semibold
              @text-fg
        // THE CHANNEL NAME IS CLIPPED, not merely given `w=fill`. A channel
        // name is user-sized and `wrap=none` text lays out one line at its
        // INTRINSIC width whatever box it was allotted — `w=fill` decides how
        // much room the row hands it, never how much it draws. Only a
        // `clip=true` ancestor cuts the overflow, which is why the channel
        // list's own rows survive the same names (their 236px pane clips).
        // Without this box the name painted straight through the dock button.
        box
          with
            w=fill
            clip=true
          row gap=3.0 align=center
            text "#"
              with
                size=12.0
                wrap=none
                font=display
                @text-hint
            text channel
              with
                size=12.0
                wrap=none
                font=display
                @text-muted
        button -> emit(dock_huddle)
          with
            label="Dock the huddle window"
            @icon_action
            @p-5px
          Icon
            with
              name="collapse"
              tone="muted"
              px=12.0
          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
          hovered bg=subtle text=fg
          pressed bg=subtle text=fg
    box
      with
        w=fill
        h=1.0
        bg=separator
      space w=1.0 h=1.0
    // The call's own word — connecting, a device note, or the hub's refusal
    // prose. A bare `live` is NOT drawn: the pulse dot and the running clock
    // one band up are the same sentence, and this strip exists to say the
    // thing they cannot.
    if !empty(status) && (status != "live")
      box
        with
          w=fill
          pl=13.0
          pr=13.0
          pt=7.0
          pb=7.0
          bg=subtle
        text status
          with
            w=fill
            size=10.5
            wrap=none
            font=code_medium
            @text-caption
    // BAND 2 — THE STAGE, and the band that takes the fill. Everything the
    // panel is FOR lives here, so it grows with the window instead of leaving
    // the bottom 70% of a 560x700 window as empty background, and it SCROLLS:
    // a six-person roster used to push the controls off a clipped panel with
    // no way to reach them.
    scroll
      with
        dir=vertical
        w=fill
        h=fill
      col
        with
          w=fill
          gap=10.0
          pl=12.0
          pr=12.0
          pt=12.0
          pb=12.0
        // The video strip: every live camera's latest frame (peers first,
        // the local preview last), only while any camera is on. The widget
        // repaints itself — mounting it is the whole contract.
        if video_live
          extern call_video_tiles()
        // `max-cell` owns the column count so no tile carries a width: 2
        // columns at the window's 320px minimum, 4 at 560. It is the MAX and
        // not the min because the min form stretches a lone tile across the
        // whole stage — one person in a widened huddle got a 534px-wide,
        // 100px-tall bar with a 34px avatar adrift in it.
        grid max-cell=168.0 gap=8.0
          for tile in rows
            HuddleTile
              with
                person=tile.person
                muted=tile.muted
    box
      with
        w=fill
        h=1.0
        bg=separator
      space w=1.0 h=1.0
    // BAND 3 — THE CONTROLS, pinned to the bottom of the window on their own
    // plate. Two groups, one gap: what this device is doing (mic, camera, jump
    // to the channel) on the left, and the one destructive action on the
    // right. `Leave` is the only SOLID danger control in the window — a muted
    // mic wears the soft danger plate, so the eye can still tell the button
    // that ends the call from the one that mutes it.
    box
      with
        w=fill
        pl=12.0
        pr=12.0
        pt=10.0
        pb=10.0
        bg=sidebar
      row
        with
          w=fill
          gap=7.0
          align=center
        if !muted
          button -> emit(toggle_call_mute)
            with
              label="Mute the microphone"
              checked=muted
              w=32.0
              h=32.0
              @icon_action
              @px-0px
              @py-0px
            box
              with
                w=fill
                h=fill
                align-x=center
                align-y=center
              Icon
                with
                  name="mic"
                  tone="muted"
                  px=14.0
            active bg=elevated text=fg border=control_line border-w=1.0 r=9.0
            hovered bg=subtle text=fg border=control_line_hover
            pressed bg=subtle text=fg
        if muted
          button -> emit(toggle_call_mute)
            with
              label="Unmute the microphone"
              checked=muted
              w=32.0
              h=32.0
              @icon_action
              @px-0px
              @py-0px
            box
              with
                w=fill
                h=fill
                align-x=center
                align-y=center
              Icon
                with
                  name="mic-off"
                  tone="danger"
                  px=14.0
            active bg=danger_bg text=danger border=danger_line border-w=1.0 r=9.0
            hovered bg=danger_bg text=danger border=danger
            pressed bg=danger_bg text=danger
        if !camera
          button -> emit(toggle_call_camera)
            with
              label="Turn the camera on"
              checked=camera
              w=32.0
              h=32.0
              @icon_action
              @px-0px
              @py-0px
            box
              with
                w=fill
                h=fill
                align-x=center
                align-y=center
              Icon
                with
                  name="camera-off"
                  tone="muted"
                  px=14.0
            active bg=elevated text=fg border=control_line border-w=1.0 r=9.0
            hovered bg=subtle text=fg border=control_line_hover
            pressed bg=subtle text=fg
        if camera
          button -> emit(toggle_call_camera)
            with
              label="Turn the camera off"
              checked=camera
              w=32.0
              h=32.0
              @icon_action
              @px-0px
              @py-0px
            box
              with
                w=fill
                h=fill
                align-x=center
                align-y=center
              Icon
                with
                  name="camera"
                  tone="ink"
                  px=14.0
            // `subtle`, not `selected_row`: this is a 32px media TOGGLE that
            // is engaged, one step up from its own off state (`elevated`) —
            // the same class as the mic's `danger_bg` beside it, not a row in
            // a list you navigated to.
            active bg=subtle text=fg border=control_line_hover border-w=1.0 r=9.0
            hovered bg=subtle text=fg border=control_line_hover
            pressed bg=subtle text=fg
        // The channel is NAMED in the header, so this is a glyph and not the
        // window's loudest element: `open # <channel>` spelled out was wider
        // than both media controls together and grew without bound with the
        // channel name, which is what pushed `Leave` off a narrow panel.
        button -> emit(huddle_go_channel)
          with
            label="Open the huddle channel"
            w=32.0
            h=32.0
            @icon_action
            @px-0px
            @py-0px
          box
            with
              w=fill
              h=fill
              align-x=center
              align-y=center
            Icon
              with
                name="nav-chat"
                tone="muted"
                px=14.0
          active bg=elevated text=fg border=control_line border-w=1.0 r=9.0
          hovered bg=subtle text=fg border=control_line_hover
          pressed bg=subtle text=fg
        space w=fill
        button -> emit(leave_huddle_here)
          with
            label="Leave the huddle"
            @icon_action
            @px-13px
            @py-0px
          box h=32.0 align-y=center
            text "Leave"
              with
                size=12.0
                wrap=none
                font=display
                @text-primary_fg
          active bg=danger_solid text=primary_fg border=transparent border-w=1.0 r=9.0
          hovered bg=danger_solid_hover text=primary_fg
          pressed bg=danger_solid_hover text=primary_fg

// "A CALL IS LIVE, BUT NOT HERE" — the channel-header affordance for the huddle
// running in another channel. Clicking jumps to it; the pulse dot is the same
// live mark the sidebar row wears.
component HuddleElsewhere(name:str)
  emits
    huddle_go_channel
  button #root -> emit(huddle_go_channel)
    with
      label="Open the channel the huddle is in"
      @icon_action
      @px-11px
      @py-5px
    row gap=7.0 align=center
      PulseDot plate=6.0 tone="success"
      text "#"
        with
          size=12.0
          wrap=none
          font=display
          @text-hint
      text name
        with
          size=12.0
          wrap=none
          font=display
          @text-accent_fg
      text "· call in progress"
        with
          size=12.0
          wrap=none
          font=display
          @text-caption
    active bg=surface text=accent_fg border=control_line border-w=1.0 r=9.0
    hovered bg=muted_bg text=accent_fg border=control_line_hover
    pressed bg=subtle text=accent_fg
