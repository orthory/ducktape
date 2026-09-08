// The desktop app's kit shapes this screen uses, copied rather than `use`d:
// the kit files carry every other screen's externs with them.

// THE DESTRUCTIVE CONFIRM — the app's one dialog for every bare delete, so
// the voice is one voice: name the object, say the blast radius, offer the
// exit first.
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

// The modal card — r=14 over the artifact's 24/60 shadow. It paints no scrim
// and binds no dismiss: both come from the `overlay` at the call site.
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

// An all-caps section eyebrow — 9px mono over the widest tracking in the scale.
component GroupLabel(label:str)
  text label #root
    with
      size=9.0
      wrap=none
      font=code_semibold
      @text-label
