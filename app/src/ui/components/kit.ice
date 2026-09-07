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

// The bordered card a settings/detail group lives in. Children draw their own
// separators, so the card only owns the outline and the clip.
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

// One label/value line inside a GroupCard. `last` drops the rule so the card's
// own border finishes the stack.
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

// The status dot that precedes a machine reading.
component Dot(plate:f64)
  box #root
    with
      w=plate
      h=plate
      bg=success_dot
      r=(plate / 2.0)
    space w=1.0 h=1.0

// A dashed empty plate — what a screen shows when its list is legitimately
// empty, as opposed to not loaded yet.
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

// The screen-level heading pair used by the padded screens (Approvals,
// Settings, Explorer) that have no 56px header bar.
component ScreenTitle(title:str, detail:str)
  col #root w=fill gap=3.0
    text title
      with
        size=16.0
        wrap=none
        font=display
        @text-primary
    if detail != ""
      box w=fill max-w=620.0
        text detail
          with
            size=12.5
            line-h=1.5
            @text-caption

// A roster row: 32px plate, name over key line, role marker, standing on the
// right. Shape carries authorship — a person is round, an agent is a square.
component MemberRowCard(member:MemberRow)
  col #root w=fill
    box
      with
        w=fill
        pl=14.0
        pr=14.0
        pt=12.0
        pb=12.0
      row
        with
          w=fill
          gap=12.0
          align=center
        if member.is_agent
          PrincipalAvatar
            with
              initials=initials_of(member.label)
              is_agent=true
              plate=32.0
              ink=10.0
              ring=""
        if !member.is_agent
          PrincipalAvatar
            with
              initials=initial_of(member.label)
              is_agent=false
              plate=32.0
              ink=12.0
              ring=""
        col w=fill gap=2.0
          row gap=6.0 align=center
            text member.label
              with
                size=13.5
                wrap=none
                font=display
                @text-fg
            // `this node`, NOT `you`. The flag is `is_this_node` — the row's
            // key equals the key of the node this client is ATTACHED to — and
            // a client can attach to a node it does not own (the picker keeps
            // remote endpoints), which would have marked a stranger's
            // validator as the person reading the screen. The human identity
            // is a different key entirely: the one Settings calls YOUR
            // IDENTITY and the one that signs every message in chat. This
            // roster lists validators, residents and agents, so it never
            // contains that key at all.
            //
            // The artifact writes the chip in SANS medium and the key line in
            // mono regular. The type-scale guard fixes the SIZE only and
            // asserts no size→face pairing (main.rs), so the face here is a
            // free choice; these steps are the nearest ones on the scale.
            if member.is_this_node
              text "this node"
                with
                  size=9.5
                  wrap=none
                  font=display
                  @text-meta
          text member.key
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-hint
        RoleMarker role=member.role
    box
      with
        w=fill
        h=1.0
        bg=muted_bg
      space w=1.0 h=1.0

// The role marker's tone is its meaning: ink for authority, accent for a
// machine principal, hairline for everyone else.
component RoleMarker(role:str)
  row #root gap=6.0 align=center
    match role
      "validator"
        box
          with
            px=7.0
            py=3.0
            bg=primary
            r=5.0
          text "VALIDATOR"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-primary_fg
      "agent"
        box
          with
            px=7.0
            py=3.0
            bg=brand_bg
            border=brand_line
            border-w=1.0
            r=5.0
          text "AGENT"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-brand
      "resident"
        box
          with
            px=7.0
            py=3.0
            bg=surface
            border=control_line
            border-w=1.0
            r=5.0
          text "RESIDENT"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-input
      _
        box
          with
            px=7.0
            py=3.0
            bg=warning_bg
            border=warning_line
            border-w=1.0
            r=5.0
          text role
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-warning

// A Runs model row: who it is, what capability it holds, who owns it, and
// whether it is live. The artifact lists agents — it does not card them.
component AgentCard(agent:AgentRow)
  col #root w=fill
    box
      with
        w=fill
        pl=14.0
        pr=14.0
        pt=13.0
        pb=13.0
      row
        with
          w=fill
          gap=13.0
          align=center
        PrincipalAvatar
          with
            initials=agent.initials
            is_agent=true
            plate=34.0
            ink=11.0
            ring=""
        col w=fill gap=3.0
          row
            with
              w=fill
              gap=8.0
              align=center
            text agent.name
              with
                size=13.5
                wrap=none
                font=display
                @text-fg
            box
              with
                px=7.0
                py=2.0
                bg=elevated
                r=5.0
              text agent.capability
                with
                  size=10.0
                  wrap=none
                  font=code_semibold
                  @text-secondary_fg
          // what it may do, counted — never a comma-joined dump of grant names
          row
            with
              w=fill
              gap=5.0
              align=center
            text agent.skill_count
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-meta
            text "skills ·"
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-meta
            text agent.cap_count
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-meta
            text "grants · owner"
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-meta
            OwnerHandle handle=agent.owner_handle
        AgentStatusChip status=agent.status
    box
      with
        w=fill
        h=1.0
        bg=muted_bg
      space w=1.0 h=1.0

// A member handle always wears its sigil; an unowned record says so rather
// than leaving the slot blank.
component OwnerHandle(handle:str)
  row #root gap=0.0 align=center
    if empty(handle)
      text "unowned"
        with
          size=10.5
          wrap=none
          font=code_medium
          @text-hint
    if !empty(handle)
      text "@"
        with
          size=10.5
          wrap=none
          font=code_medium
          @text-hint
    if !empty(handle)
      text handle
        with
          size=10.5
          wrap=none
          font=code_medium
          @text-hint

// Standing, in the registry's own words, printed the way a status reads:
// upper-case, because the wire hands us `active` / `paused` in snake case.
component AgentStatusChip(status:str)
  col #root
    match status
      "active"
        box
          with
            px=8.0
            py=3.0
            bg=success_bg
            border=success_line
            border-w=1.0
            r=6.0
          row gap=5.0 align=center
            PulseDot plate=5.0 tone="success"
            text "ACTIVE"
              with
                size=9.0
                wrap=none
                font=code_semibold
                @text-success
      "paused"
        box
          with
            px=8.0
            py=3.0
            bg=warning_bg
            border=warning_line
            border-w=1.0
            r=6.0
          row gap=5.0 align=center
            PulseDot plate=5.0 tone="warning"
            text "PAUSED"
              with
                size=9.0
                wrap=none
                font=code_semibold
                @text-warning
      _
        box
          with
            px=8.0
            py=3.0
            bg=warning_bg
            border=warning_line
            border-w=1.0
            r=6.0
          row gap=5.0 align=center
            PulseDot plate=5.0 tone="warning"
            text status
              with
                size=9.0
                wrap=none
                font=code_semibold
                @text-warning

// One alert: a severity dot, the title, the source that raised it, the body,
// and the block it landed in. Unread rows sit on a warmer plate than read ones
// AND pulse; a read row keeps the same severity colour, held still. `height` is
// a BLOCK, so it prints as one — this chain publishes no wall clock.
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
