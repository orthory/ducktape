// The desktop app's kit shapes this screen uses, copied rather than `use`d:
// the kit files carry every other screen's externs with them.

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

component FilterChip(label:str, count:i64, selected:bool)
  col #root
    if selected
      box
        with
          px=11.0
          py=6.0
          bg=primary
          border=primary
          border-w=1.0
          r=8.0
        row gap=6.0 align=center
          text label
            with
              size=12.0
              wrap=none
              font=display
              @text-primary_fg
          text count
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-meta
    if !selected
      box
        with
          px=11.0
          py=6.0
          bg=surface
          border=border
          border-w=1.0
          r=8.0
        row gap=6.0 align=center
          text label
            with
              size=12.0
              wrap=none
              font=display
              @text-secondary_fg
          text count
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-label

component ExplorerCard(hit:ExplorerHit)
  box #root
    with
      w=fill
      px=15.0
      py=13.0
      bg=surface
      border=separator
      border-w=1.0
      r=11.0
      clip=true
    row
      with
        w=fill
        gap=12.0
        align=start
      ExplorerKindPlate kind=hit.kind code=hit.code
      col w=fill gap=3.0
        row
          with
            w=fill
            gap=8.0
            align=center
          text hit.title
            with
              w=fill
              size=13.0
              wrap=none
              font=display
              @text-primary
          ExplorerKindBadge kind=hit.kind
        // the snippet is raw message/page body of any length — it wraps, the
        // same way chat's own search hits do. The title and meta stay single
        // runs and are held inside the card by the clip.
        text hit.snippet
          with
            w=fill
            size=12.0
            wrap=word
            line-h=1.5
            @text-input
        text hit.meta
          with
            w=fill
            size=10.5
            wrap=none
            font=code
            @text-label

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

component ExplorerKindPlate(kind:str, code:str)
  col #root
    match kind
      "page"
        box
          with
            w=28.0
            h=28.0
            align-x=center
            align-y=center
            bg=kind_page_bg
            r=8.0
          text code
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-kind_page
      "code"
        box
          with
            w=28.0
            h=28.0
            align-x=center
            align-y=center
            bg=kind_code_bg
            r=8.0
          text code
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-kind_code
      "file"
        box
          with
            w=28.0
            h=28.0
            align-x=center
            align-y=center
            bg=kind_file_bg
            r=8.0
          text code
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-kind_file
      "run"
        box
          with
            w=28.0
            h=28.0
            align-x=center
            align-y=center
            bg=kind_run_bg
            r=8.0
          text code
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-kind_run
      "task"
        box
          with
            w=28.0
            h=28.0
            align-x=center
            align-y=center
            bg=kind_task_bg
            r=8.0
          text code
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-kind_task
      _
        box
          with
            w=28.0
            h=28.0
            align-x=center
            align-y=center
            bg=info_bg
            r=8.0
          text code
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-info

component ExplorerKindBadge(kind:str)
  col #root
    match kind
      "page"
        box
          with
            px=5.0
            py=2.0
            bg=kind_page_bg
            r=4.0
          text "PAGE"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-kind_page
      "code"
        box
          with
            px=5.0
            py=2.0
            bg=kind_code_bg
            r=4.0
          text "CODE"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-kind_code
      "file"
        box
          with
            px=5.0
            py=2.0
            bg=kind_file_bg
            r=4.0
          text "FILE"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-kind_file
      "run"
        box
          with
            px=5.0
            py=2.0
            bg=kind_run_bg
            r=4.0
          text "RUN"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-kind_run
      "task"
        box
          with
            px=5.0
            py=2.0
            bg=kind_task_bg
            r=4.0
          text "TASK"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-kind_task
      _
        box
          with
            px=5.0
            py=2.0
            bg=info_bg
            r=4.0
          text "MESSAGE"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-info
