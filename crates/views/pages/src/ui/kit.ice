// KIT SHAPES the pages screen uses, copied from the app's components
// (kit.ice, shell.ice, overlay.ice carry every other screen's externs, so
// they are not `use`d). Same geometry, same tokens.

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

component ConfirmDelete(title:str, subject:str, note:str, action:str, busy:bool)
  emits
    cancel()
    confirm()
  ModalShell title=title width=418.0
    close:
      button -> emit(cancel)
        with
          label="Cancel"
          disabled=busy
          w=26.0
          h=26.0
          p=0.0
          @icon_action
        box
          with
            w=fill
            h=fill
            align-x=center
            align-y=center
          text "×"
            with
              size=14.0
              wrap=none
              @text-muted
        active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
        hovered bg=elevated text=fg
        pressed bg=subtle text=fg
    body:
      col w=fill gap=13.0
        text subject
          with
            w=fill
            size=13.5
            font=medium
            @text-fg
        text note
          with
            w=fill
            size=12.0
            line-h=1.5
            @text-caption
        row
          with
            w=fill
            gap=8.0
            align=end
          button "Cancel" -> emit(cancel)
            with
              disabled=busy
              h=30.0
              p=7.0
              @secondary_action
          button -> emit(confirm)
            with
              label=action
              disabled=busy
              h=30.0
              p=7.0
              @danger_action
            text action size=13.0 wrap=none

component ModalShell(title:str, width:f64)
  box #root
    with
      w=width
      bg=surface
      border=border
      border-w=1.0
      r=14.0
      shadow=shadow_modal
      shadow-y=24.0
      shadow-blur=60.0
    col
      with
        w=fill
        pl=22.0
        pr=22.0
        pt=20.0
        pb=22.0
        gap=13.0
      row
        with
          w=fill
          gap=10.0
          align=center
        text title
          with
            w=fill
            size=16.0
            wrap=none
            font=display
            @text-primary
        slot close
      slot body

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
