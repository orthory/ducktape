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

// The identity card's 40px plate: the app's PersonAvatar at the one size this
// screen draws it (above the 18px sidebar step, no ring).
component PersonAvatar(initials:str, plate:f64, ink:f64)
  box #root
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

component Badge.Secondary(label:str)
  box #root
    with
      px=7.0
      py=3.0
      bg=primary
      r=5.0
    text label @badge_label text-primary_fg

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

component Badge.Destructive(label:str)
  box #root
    with
      px=7.0
      py=3.0
      bg=danger_bg
      border=danger_line
      border-w=1.0
      r=5.0
    row gap=5.0 align=center
      box
        with
          w=6.0
          h=6.0
          bg=danger_dot
          r=3.0
        space w=1.0 h=1.0
      text label @badge_label

component Badge.Success(label:str)
  box #root
    with
      px=7.0
      py=3.0
      bg=success_bg
      border=success_line
      border-w=1.0
      r=5.0
    row gap=5.0 align=center
      box
        with
          w=6.0
          h=6.0
          bg=success_dot
          r=3.0
        space w=1.0 h=1.0
      text label @badge_label

// A PHONE CEREMONY ON A CARD: the QR while the phone is asked, the line
// while the chain is; an empty phase renders nothing. The Settings card's
// reading of the same stream the welcome shows full-size.
component CeremonyPlate(phase:str, qr:str, detail:str, left:str)
  emits
    account_ceremony_cancel()
  col #root
    with
      w=fill
      gap=8.0
      align=center
    if phase == "show_qr"
      qr qr #plate-qr cell-size=3.0 correction=medium
      text detail
        with
          w=fill
          size=12.0
          align-x=center
          @text-meta
      text left #plate-left
        with
          size=11.0
          wrap=none
          font=code_medium
          @text-hint
      button "Cancel" #plate-cancel -> emit(account_ceremony_cancel)
        with
          p=5.0
          @secondary_action
    if phase == "working"
      text detail
        with
          w=fill
          size=12.0
          @text-meta
      button "Cancel" #plate-cancel-working -> emit(account_ceremony_cancel)
        with
          p=5.0
          @secondary_action
