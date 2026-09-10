// The kit shapes the forge screen draws, copied from the desktop app's
// components/kit.ice, overlay.ice, patterns.ice and chat.ice: those files
// carry every other screen's externs, so a view that `use`d them would
// drag the whole app's extern surface across the wire. Same shapes, same
// tokens, at the sizes this screen mounts them.

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

component GroupLabel(label:str)
  text label #root
    with
      size=9.0
      wrap=none
      font=code_semibold
      @text-label

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

component FinalityChip(height:i64)
  col #root
    box
      with
        px=7.0
        py=2.0
        bg=final_bg
        border=final_line
        border-w=1.0
        r=5.0
      row gap=4.0 align=center
        text "✓ finalized"
          with
            size=9.0
            wrap=none
            font=code_semibold
            @text-success_tick
        if height > 0
          text "·"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-success_tick
        if height > 0
          text "h"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-success_tick
        if height > 0
          text height
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-success_tick

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

component Popover(width:f64)
  box #root
    with
      w=width
      p=5.0
      bg=surface
      border=border
      border-w=1.0
      r=11.0
      shadow=shadow_popover
      shadow-y=3.0
      shadow-blur=12.0
    col w=fill
      slot

// Shapes lifted from components/chat.ice for the non-chat probes (the
// composer there refuses to compile unmounted).
component RichBody(blocks:[ChatBlock], size:f64)
  emits
    open_message_link(str)
  col w=fill gap=5.0
    for block in blocks
      if block.kind == "divider"
        Separator
      // A code fence is a QUIET slab: the near-surface tint + hairline reads
      // as "preformatted" in both themes (the old black/26 was a dark slab in
      // light mode and vanished in dark). The lang tag is an eyebrow label,
      // not a code line.
      //
      // Same shape as the un-reacted reaction chip, and the same fix: this is
      // nested INSIDE a message row, `muted_bg` was never what drew it, and
      // its `border` edge measured 5.33/2.00 against `selected_row` — inside a
      // message you had a menu open on, the fence had no outline at all.
      // `control_line` is 13.00/7.67 there and 31.67/31.00 against `bg`.
      if block.kind == "code"
        box
          with
            w=fill
            p=11.0
            bg=muted_bg
            border=control_line
            border-w=1.0
            r=9.0
          col w=fill gap=6.0
            if !empty(block.lang)
              text block.lang
                with
                  size=10.0
                  wrap=none
                  font=code_semibold
                  @text-label
            text block.text
              with
                w=fill
                size=12.0
                line-h=1.5
                font=code
                wrap=word-or-glyph
                @text-fg
      // A quote is a LEFT BAR, not a box — boxed it was indistinguishable
      // from a code slab at a glance. The bar wears the warm accent hairline
      // and fills the CONTENT's height (zstack resolves fill layers against
      // the content union — ducktape-ui 1231692; the earlier row form
      // degenerated in the infinite-height scroll and ate the whole row).
      if block.kind == "quote"
        stack w=fill
          col
            with
              w=fill
              pl=13.0
              pt=2.0
              pb=2.0
            if block.rich
              RichLine block=block size=size
                forward
                  open_message_link
            // QUIETER THAN THE PROSE QUOTING IT, at the SAME leading the rich
            // arm renders it at — a quote is cited material, and it should
            // recede from the author's own words, not out-darken them. This
            // was the darkest ink in the message (`fg`, 13.4:1); `muted`
            // (5.37:1) is still comfortably readable and visibly quieter.
            if !block.rich
              text block.text
                with
                  w=fill
                  size=size
                  line-h=1.55
                  wrap=word-or-glyph
                  @text-muted
          box
            with
              w=3.0
              h=fill
              bg=brand_line
              r=1.5
            space w=1.0 h=1.0
      if block.kind == "paragraph"
        if block.rich
          RichLine block=block size=size
            forward
              open_message_link
        if !block.rich
          text block.text
            with
              w=fill
              size=size
              line-h=1.55
              wrap=word-or-glyph
              @text-accent_fg

component MessageAvatar(initials:str, kind:str)
  stack #root w=30.0 h=30.0
    match kind
      "human"
        PersonAvatar
          with
            initials
            plate=30.0
            ink=11.0
      "agent"
        AgentAvatar
          with
            initials
            plate=30.0
            ink=11.0
      _
        AgentAvatar
          with
            initials
            plate=30.0
            ink=11.0

component MessageBody(message:ChatMessage)
  emits
    open_message_link(str)
  col w=fill max-w=760.0
    RichBody blocks=message.blocks size=13.5
      forward
        open_message_link

component RichLine(block:ChatBlock, size:f64)
  emits
    open_message_link(str)
  rich-text -> emit(open_message_link, _)
    with
      w=fill
      size=size
      line-h=1.55
      wrap=word-or-glyph
      color=accent_fg
    for span in block.spans
      // Span padding expands only the paint, not the text layout. Keep it
      // below a prose space so ordinary surrounding spaces stay visible.
      span span.mention bg=brand_bg px=1.0 r=4.0 font=medium color=brand
      span span.link_text link=span.link underline font=medium color=brand
      span span.bold_italic font=strongitalic
      span span.bold font=strong
      span span.italic font=italic
      span span.plain
