// The kit shapes the shell view draws, copied from the app's components
// (kit.ice, patterns.ice) rather than `use`d: those files carry every other
// screen's externs. Same geometry, same tokens.

component Alert.Destructive(title:str, description:str)
  box #root
    with
      w=fill
      p=13.0
      bg=danger_bg
      border=danger_line
      border-w=1.0
      r=11.0
    row w=fill gap=9.0
      box
        with
          w=24.0
          h=24.0
          align-x=center
          align-y=center
          bg=danger_dot
          r=7.0
        text "!"
          with
            size=14.0
            @font-semibold
            @text-danger_fg
      col w=fill gap=3.0
        text title
          with
            size=14.0
            @font-semibold
            @text-fg
        text description size=13.0 @text-muted

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


// The agent plate at the sizes this screen uses (24px beside a turn, 46px on
// the welcome): the AgentSquare of patterns.ice, radius by plate.
component AgentAvatar(initials:str, plate:f64, ink:f64)
  col #root
    if plate >= 40.0
      AgentSquare initials=initials plate=plate ink=ink radius=10.0
    if plate < 40.0
      AgentSquare initials=initials plate=plate ink=ink radius=8.0

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
