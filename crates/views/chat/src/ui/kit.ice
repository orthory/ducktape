// The kit shapes the chat screen mounts, copied from the desktop app's
// components (kit, patterns, overlay, huddle): those files carry every other
// screen's externs, so a view copies the shapes it draws rather than `use`
// the files. Same bodies as the app's, verbatim.

component Badge.Outline(label:str)
  box #root
    with
      px=7.0
      py=3.0
      bg=surface
      border=control_line
      border-w=1.0
      r=5.0
    text label @badge_label text-secondary_fg

component Separator()
  rule horizontal #root thickness=1.0 color=border

component EmptyState(title:str, description:str)
  box #root
    with
      w=fill
      h=fill
      p=22.0
      align-x=center
      align-y=center
    col
      with
        w=fill
        align=center
        gap=7.0
      box
        with
          w=42.0
          h=42.0
          align-x=center
          align-y=center
          bg=surface
          border=border
          border-w=1.0
          r=21.0
        text "◇" size=20.0 @text-primary
      text title @section_title text-fg
      text description @caption

component PersonAvatar(initials:str, plate:f64, ink:f64)
  col #root
    PrincipalAvatar
      with
        initials
        is_agent=false
        plate
        ink
        ring=""

component AgentAvatar(initials:str, plate:f64, ink:f64)
  col #root
    PrincipalAvatar
      with
        initials
        is_agent=true
        plate
        ink
        ring=""

component PrincipalAvatar(initials:str, is_agent:bool, plate:f64, ink:f64, ring:str)
  col #root
    match ring
      "paper"
        box
          with
            p=1.5
            bg=surface
            r=(plate / 2.0 + 1.5)
          PrincipalPlate
            with
              initials
              is_agent
              plate
              ink
      "rail"
        box
          with
            p=1.5
            bg=rail
            r=(plate / 2.0 + 1.5)
          PrincipalPlate
            with
              initials
              is_agent
              plate
              ink
      _
        PrincipalPlate
          with
            initials
            is_agent
            plate
            ink

component PrincipalPlate(initials:str, is_agent:bool, plate:f64, ink:f64)
  col #root
    if is_agent
      AgentPlate
        with
          initials
          plate
          ink
    if !is_agent
      HumanPlate
        with
          initials
          plate
          ink

component GateNote(reason:str, next:str)
  box #root
    with
      w=fill
      px=13.0
      py=11.0
      bg=warning_bg_lit
      border=warning_line
      border-w=1.0
      r=9.0
    row
      with
        w=fill
        gap=8.0
        align=start
      col pt=4.0
        box
          with
            w=6.0
            h=6.0
            bg=warning_dot
            r=3.0
          space w=1.0 h=1.0
      col w=fill gap=2.0
        text reason
          with
            w=fill
            size=12.0
            line-h=1.45
            @text-warning
        if next != ""
          text next
            with
              w=fill
              size=12.0
              line-h=1.45
              @text-caption

component PulseDot(plate:f64, tone:str)
  col #root
    match tone
      "warning"
        box
          with
            w=plate
            h=plate
            bg=warning_dot
            r=(plate / 2.0)
          space w=1.0 h=1.0
      "danger"
        box
          with
            w=plate
            h=plate
            bg=danger_dot
            r=(plate / 2.0)
          space w=1.0 h=1.0
      "info"
        box
          with
            w=plate
            h=plate
            bg=info_dot
            r=(plate / 2.0)
          space w=1.0 h=1.0
      _
        box
          with
            w=plate
            h=plate
            bg=success_dot
            r=(plate / 2.0)
          space w=1.0 h=1.0

component Eyebrow(label:str, note:str)
  row #root gap=8.0 align=center
    text label
      with
        size=9.0
        wrap=none
        font=code_semibold
        @text-label
    if note != ""
      text note
        with
          size=9.0
          wrap=none
          font=code_semibold
          @text-label

component HuddleLivePill(elapsed:str, muted:bool)
  emits
    show_huddle
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
      button -> emit(show_huddle)
        with
          label="Show the huddle window"
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

component HumanPlate(initials:str, plate:f64, ink:f64)
  col #root
    if plate <= 18.0
      box
        with
          w=plate
          h=plate
          align-x=center
          align-y=center
          bg=avatar_bg_sm
          r=(plate / 2.0)
        text initials
          with
            size=ink
            wrap=none
            font=display
            @text-avatar_fg_sm
    if plate > 18.0
      box
        with
          w=plate
          h=plate
          align-x=center
          align-y=center
          bg=avatar_bg
          r=(plate / 2.0)
        text initials
          with
            size=ink
            wrap=none
            font=display
            @text-muted

component AgentPlate(initials:str, plate:f64, ink:f64)
  col #root
    if plate >= 40.0
      AgentSquare
        with
          initials
          plate
          ink
          radius=10.0
    if plate >= 33.0 && plate < 40.0
      AgentSquare
        with
          initials
          plate
          ink
          radius=9.0
    if plate >= 25.0 && plate < 33.0
      AgentSquare
        with
          initials
          plate
          ink
          radius=8.0
    if plate >= 22.0 && plate < 25.0
      AgentSquare
        with
          initials
          plate
          ink
          radius=7.0
    if plate < 22.0
      AgentSquare
        with
          initials
          plate
          ink
          radius=6.0

component AgentSquare(initials:str, plate:f64, ink:f64, radius:f64)
  box #root
    with
      w=plate
      h=plate
      align-x=center
      align-y=center
      bg=primary
      r=radius
    text initials
      with
        size=ink
        wrap=none
        font=code_semibold
        @text-toast_fg
