// The shared composition surface: the small generic pieces the app actually
// uses plus the repeated shapes of the canonical console. Anything used once
// stays inline at its call site.

component Panel(title:str, description:str)
  box #root r=11.0 @panel
    col @section
      col @field
        text title @section_title
        text description @caption
      slot

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

component Field(label:str, description:str)
  col #root @field
    text label @field_label
    slot
    text description @caption

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

component Badge.Warning(label:str)
  box #root
    with
      px=7.0
      py=3.0
      bg=warning_bg
      border=warning_line
      border-w=1.0
      r=5.0
    row gap=5.0 align=center
      box
        with
          w=6.0
          h=6.0
          bg=warning_dot
          r=3.0
        space w=1.0 h=1.0
      text label @badge_label

component Kbd(label:str)
  box #root
    with
      px=7.0
      py=3.0
      bg=accent
      border=border
      border-w=1.0
      r=5.0
      shadow=black/10
      shadow-y=1.0
      shadow-blur=2.0
    text label @meta text-fg

component Separator()
  rule horizontal #root thickness=1.0 color=border

// Empty readings sit on the page instead of introducing another bordered slab.
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

// An all-caps section eyebrow — 9px mono over the widest tracking in the scale.
component GroupLabel(label:str)
  text label #root
    with
      size=9.0
      wrap=none
      font=code_semibold
      @text-label

// One label/value line inside a bordered card. `last` drops the rule so the
// card's own border finishes the stack.
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

// The two shape-named entry points into `PrincipalAvatar`. The shape rule and
// the radius ladder live there — these only spell the discriminant, so a call
// site that already knows it is drawing a person keeps reading as one.
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

component BellRow(item:BellItem)
  col #root w=fill
    if item.read
      box
        with
          w=fill
          pl=9.0
          pr=9.0
          pt=9.0
          pb=10.0
          r=9.0
          bg=transparent
        BellBody item=item
    if !item.read
      box
        with
          w=fill
          pl=9.0
          pr=9.0
          pt=9.0
          pb=10.0
          r=9.0
          bg=unread_wash
        BellBody item=item

component BellBody(item:BellItem)
  row #root
    with
      w=fill
      gap=9.0
      align=start
    // The dot carries SEVERITY, which is what `bell_severity` was written for
    // and what the titlebar badge already keys on — a row that colours by read
    // state instead says the same blue for a failure and a mention. Read vs
    // unread is the plate and the pulse; the hue is the severity either way.
    if item.read
      StillDot plate=7.0 tone=bell_severity(item.reason)
    if !item.read
      PulseDot plate=7.0 tone=bell_severity(item.reason)
    col w=fill gap=3.0
      row
        with
          w=fill
          gap=7.0
          align=center
        text bell_title(item.reason)
          with
            w=fill
            size=12.0
            wrap=none
            @text-primary
        if item.height > 0
          text height_label_short(item.height)
            with
              size=9.5
              wrap=none
              font=code_medium
              @text-hint
        box
          with
            px=4.0
            py=1.0
            bg=info_bg
            border=info_line
            border-w=1.0
            r=4.0
          text item.source
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-info
      text bell_detail(item)
        with
          w=fill
          size=12.0
          line-h=1.45
          @text-input

component StatusBadge(label:str)
  row align=center
    match label
      "active"
        Badge.Success label=label
      "paused"
        Badge.Warning label=label
      "open"
        Badge.Success label=label
      "closed"
        Badge.Destructive label=label
      "merged"
        Badge.Success label=label
      "passed"
        Badge.Success label=label
      "rejected"
        Badge.Destructive label=label
      "applied"
        Badge.Success label=label
      "discarded"
        Badge.Warning label=label
      _
        Badge.Outline label=label
