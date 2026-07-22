component PageTitleEditor(rpc:str, password:str, page_id:str, title:str, disabled:bool)
  state
    editing = false
    draft = ""
    local_error = ""
  on begin(current)
    editing = true
    draft = current
    local_error = ""
  on changed(next, next_rpc, next_password, next_page)
    draft = next
    local_error = ""
    run latest autosave_page_title(next_rpc, next_password, next_page, draft) -> saved _ | save_failed _
  on saved(written)
    return if !written
    local_error = ""
  on save_failed(cause)
    local_error = cause.message
  col width=fill spacing=2.0
    if !editing && !empty(title)
      button label=title width=fill padding=4.0 -> begin(title)
        text title width=fill size=22.0 wrapping=none @font-bold text-fg
        active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
        hovered bg=white/5 text=fg border=white/7
        pressed bg=white/8 text=fg
    if !editing && empty(title)
      button label="Untitled" width=fill padding=4.0 -> begin(title)
        text "Untitled" width=fill size=22.0 wrapping=none @font-bold text-muted
        active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
        hovered bg=white/5 text=fg border=white/7
        pressed bg=white/8 text=fg
    if editing
      input "" #title-input label="Page title" <-> draft change=changed(_, rpc, password, page_id) hint="Untitled" disabled=disabled width=fill padding=4.0 text-size=22.0 line-height=1.15
        active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
        focused bg=white/5 border=white/9 border-w=1.0
        disabled value=muted
    if !empty(local_error)
      text local_error size=10.0 @text-muted

component PageButton(page:PageItem, selected:bool)
  col width=fill
    if selected
      button label=page.title width=fill height=34.0 padding=7.0 -> choose_page(page.id)
        row width=fill spacing=9.0 align=center
          text "□" width=18.0 size=13.0 align-x=center @text-fg
          text page.prefix size=11.0 wrapping=none @text-muted
          text page.title width=fill size=12.0 wrapping=none @text-fg font-bold
          if page.child_count > 0
            text page.child_count size=10.0 @text-muted
        active bg=linear(2.3, white/10@0.0, surface/72@1.0) text=fg border=white/16 border-w=1.0 r=10.0 shadow=black/8 shadow-y=1.0 shadow-blur=6.0
        pressed bg=selection
    if !selected
      button label=page.title width=fill height=34.0 padding=7.0 -> choose_page(page.id)
        row width=fill spacing=9.0 align=center
          text "□" width=18.0 size=13.0 align-x=center @text-muted
          text page.prefix size=11.0 wrapping=none @text-muted
          text page.title width=fill size=12.0 wrapping=none @text-muted
          if page.child_count > 0
            text page.child_count size=10.0 @text-muted
        active bg=transparent text=muted r=10.0
        hovered bg=white/6 text=fg
        pressed bg=selection text=fg

component PageSearchResult(hit:PageSearchHit)
  button label=hit.text width=fill padding=7.0 -> open_page_search_hit(hit.page_id)
    col width=fill spacing=2.0
      row width=fill spacing=7.0 align=center
        text hit.kind width=fill size=10.0 @font-bold text-muted
        text hit.block_id size=10.0 wrapping=none @text-muted
      text hit.text width=fill size=11.0 wrapping=word @text-fg
    active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
    hovered bg=white/9 text=fg border=white/10
    pressed bg=selection text=fg

component BlockContents(block:PageBlock)
  row width=fill spacing=7.0 align=start
    text block.prefix size=11.0 wrapping=none @text-muted
    match block.kind
      "Page"
        text "□" width=16.0 size=12.0 align-x=center @text-muted
      "Bullet"
        text "•" width=16.0 size=13.0 align-x=center @text-muted
      "Number"
        text "1." width=16.0 size=11.0 align-x=center @text-muted
      "Todo"
        if block.checked
          text "✓" width=16.0 size=11.0 align-x=center @font-bold text-fg
        if !block.checked
          text "○" width=16.0 size=12.0 align-x=center @text-muted
      "Toggle"
        text "›" width=16.0 size=15.0 align-x=center @text-muted
      "Quote"
        text "│" width=16.0 size=15.0 align-x=center @text-muted
      "Code"
        text "{}" width=16.0 size=10.0 align-x=center font=mono @text-muted
      "Callout"
        text "!" width=16.0 size=10.0 align-x=center @font-bold text-muted
      _
        space width=0.0
    col width=fill spacing=2.0
      match block.kind
        "Page"
          row width=fill spacing=6.0 align=center
            if empty(block.text)
              text "Untitled" width=fill size=13.0 wrapping=word @font-bold text-muted
            if !empty(block.text)
              text block.text width=fill size=13.0 wrapping=word @font-bold text-fg
            text "›" size=14.0 @text-muted
        "Heading 1"
          text block.text width=fill size=20.0 wrapping=word @font-bold text-fg
        "Heading 2"
          text block.text width=fill size=17.0 wrapping=word @font-bold text-fg
        "Heading 3"
          text block.text width=fill size=15.0 wrapping=word @font-bold text-fg
        "Code"
          container width=fill padding=7.0 bg=fg/7 border=white/9 border-w=1.0 r=7.0
            text block.text width=fill size=11.0 wrapping=word font=mono @text-fg
        "Divider"
          container width=fill height=1.0 bg=separator
            text ""
        _
          text block.text width=fill size=13.0 wrapping=word @text-fg
      if block.child_count > 0 || block.mark_count > 0
        row width=fill spacing=7.0 align=center
          if block.child_count > 0
            text block.child_count size=10.0 @text-muted
          if block.mark_count > 0
            text "Formatted" size=10.0 @text-muted

component BlockCard(block:PageBlock, selected:bool)
  col width=fill
    if block.pending
      container width=fill padding=8.0 bg=white/5 border=transparent border-w=1.0 r=9.0
        BlockContents block=block
    if !block.pending && selected
      button label=block.kind width=fill padding=8.0 -> select_block(block.id, block.kind, block.text, block.checked)
        BlockContents block=block
        active bg=linear(2.3, white/15@0.0, surface/48@1.0) text=fg border=white/15 border-w=1.0 r=9.0
        hovered bg=white/16 text=fg
        pressed bg=selection text=fg
    if !block.pending && !selected
      button label=block.kind width=fill padding=8.0 -> select_block(block.id, block.kind, block.text, block.checked)
        BlockContents block=block
        active bg=transparent text=fg border=transparent border-w=1.0 r=9.0
        hovered bg=white/6 text=fg border=white/7
        pressed bg=selection text=fg
