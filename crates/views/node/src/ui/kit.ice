// The desktop app's kit shapes this screen uses, copied rather than `use`d:
// the kit files carry every other screen's externs with them.

component GroupLabel(label:str)
  text label #root
    with
      size=9.0
      wrap=none
      font=code_semibold
      @text-label

component GroupCard()
  box #root
    with
      w=fill
      bg=surface
      border=card_line
      border-w=1.0
      r=11.0
      clip=true
    col w=fill
      slot

component KeyValueRow(label:str, value:str, last:bool)
  col #root w=fill
    box
      with
        w=fill
        px=15.0
        py=13.0
      row
        with
          w=fill
          gap=10.0
          align=center
        text label
          with
            size=12.5
            wrap=none
            @text-accent_fg
        space w=fill
        text value
          with
            size=12.0
            wrap=none
            font=code_medium
            @text-secondary_fg
    if !last
      box
        with
          w=fill
          h=1.0
          bg=elevated
        space w=1.0 h=1.0

component Dot(plate:f64)
  box #root
    with
      w=plate
      h=plate
      bg=success_dot
      r=(plate / 2.0)
    space w=1.0 h=1.0

component EmptyPlate(message:str)
  box #root
    with
      w=fill
      p=30.0
      align-x=center
      bg=transparent
      border=border
      border-w=1.0
      r=12.0
    text message size=13.0 @text-meta

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

component TabLabel(label:str, count:i64, active:bool)
  col #root
    row
      with
        gap=7.0
        pt=10.0
        pb=10.0
        align=center
      if active
        text label
          with
            size=13.0
            wrap=none
            font=display
            @text-primary
      if !active
        text label
          with
            size=13.0
            wrap=none
            font=display
            @text-meta
      if count > 0
        box
          with
            px=7.0
            py=1.0
            bg=elevated
            r=9.0
          text count
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-meta
    if active
      box
        with
          w=fill
          h=2.0
          bg=primary
        space w=1.0 h=1.0
    if !active
      box
        with
          w=fill
          h=2.0
          bg=transparent
        space w=1.0 h=1.0

component StatCard(label:str, value:str, note:str)
  box #root
    with
      w=fill
      px=13.0
      py=11.0
      bg=surface
      border=card_line
      border-w=1.0
      r=10.0
    col w=fill gap=3.0
      text label
        with
          size=9.0
          wrap=none
          font=code_semibold
          @text-label
      row gap=4.0 align=center
        text value
          with
            size=14.0
            wrap=none
            font=code_semibold
            @text-primary
        if note != ""
          text note
            with
              size=11.0
              wrap=none
              font=code_medium
              @text-meta

component MemberFactRow(label:str, value:str)
  box #root
    with
      w=fill
      px=12.0
      py=9.0
      bg=surface
      border=card_line
      border-w=1.0
      r=8.0
    row
      with
        w=fill
        gap=10.0
        align=start
      text label
        with
          size=11.0
          wrap=none
          font=code_medium
          @text-meta
      // `word-or-glyph`, because the value is usually a 64-character hex key
      // and word wrapping cannot break an unbroken token: the text's minimum
      // intrinsic width became the whole key, which pushed this card wider
      // than the 312px panel and let the pane clip cut the key mid-digit at
      // the window edge. A key you cannot read in full is not a key.
      text value
        with
          w=fill
          size=11.0
          wrap=word-or-glyph
          font=code_medium
          @text-secondary_fg

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
