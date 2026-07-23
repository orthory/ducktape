component PageTitleEditor(rpc:str, password:str, page_id:str, title:str, disabled:bool)
  state
    editing = false
    draft = ""
    local_error = ""
  on begin(current)
    editing = true
    draft = current
    local_error = ""
    task widget focus #title-input
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
      button label=title disabled=disabled width=fill padding=4.0 -> begin(title)
        text title width=fill size=34.0 wrapping=none font=display @text-fg
        active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
        hovered bg=white/5 text=fg border=white/7
        pressed bg=white/8 text=fg
    if !editing && empty(title)
      button label="Untitled" disabled=disabled width=fill padding=4.0 -> begin(title)
        text "Untitled" width=fill size=34.0 wrapping=none font=display @text-muted
        active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
        hovered bg=white/5 text=fg border=white/7
        pressed bg=white/8 text=fg
    if editing
      input "" #title-input label="Page title" <-> draft change=changed(_, rpc, password, page_id) hint="Untitled" disabled=disabled width=fill padding=4.0 text-size=34.0 line-height=1.15
        active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
        focused bg=white/5 border=white/9 border-w=1.0
        disabled value=muted
    if !empty(local_error)
      text local_error size=11.0 @text-muted

component PageButton(page:PageItem, selected:bool)
  col width=fill
    if selected
      button label=page.title width=fill height=34.0 padding=7.0 -> choose_page(page.id)
        row width=fill height=fill spacing=9.0 align=center
          text "▤" width=18.0 size=13.0 align-x=center @text-fg
          text page.prefix size=11.0 wrapping=none @text-muted
          text page.title width=fill size=13.0 wrapping=none font=medium @text-fg
          if page.child_count > 0
            text page.child_count size=11.0 @text-muted
        active bg=white/9 text=fg border=white/15 border-w=1.0 r=10.0
        pressed bg=selection
    if !selected
      button label=page.title width=fill height=34.0 padding=7.0 -> choose_page(page.id)
        row width=fill height=fill spacing=9.0 align=center
          text "▤" width=18.0 size=13.0 align-x=center @text-muted
          text page.prefix size=11.0 wrapping=none @text-muted
          text page.title width=fill size=13.0 wrapping=none @text-muted
          if page.child_count > 0
            text page.child_count size=11.0 @text-muted
        active bg=transparent text=muted r=10.0
        hovered bg=white/6 text=fg
        pressed bg=selection text=fg

component PageSearchResult(hit:PageSearchHit)
  button label=hit.text width=fill padding=7.0 -> open_page_search_hit(hit.page_id, hit.block_id)
    col width=fill spacing=2.0
      row width=fill spacing=7.0 align=center
        text hit.kind width=fill size=11.0 font=medium @text-muted
        text hit.block_id size=11.0 wrapping=none @text-muted
      text hit.text width=fill size=13.0 wrapping=word @text-fg
    active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
    hovered bg=white/9 text=fg border=white/10
    pressed bg=selection text=fg

component PageCommentThreadButton(thread:PageCommentThread)
  button label=thread.author description=thread.meta width=fill padding=6.0 -> open_block_comment_thread(thread.id)
    row width=fill spacing=7.0 align=center
      text thread.author width=fill size=13.0 wrapping=none font=medium @text-fg
      text thread.meta size=11.0 wrapping=none @text-muted
      text "›" size=13.0 @text-muted
    active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
    hovered bg=white/8 text=fg border=white/9
    pressed bg=selection text=fg

component PageCommentCard(comment:PageComment)
  col width=fill spacing=2.0 padding=6.0
    row width=fill spacing=7.0 align=center
      text comment.author width=fill size=11.0 wrapping=none font=medium @text-fg
      text comment.meta size=11.0 wrapping=none @text-muted
    text comment.text width=fill size=13.0 wrapping=word @text-fg

component BlockLine(block:PageBlock)
  row width=fill spacing=7.0 align=start
    match block.kind
      "Page"
        text "▤" width=16.0 size=13.0 align-x=center @text-muted
      "Bullet"
        text "•" width=16.0 size=13.0 align-x=center @text-muted
      "Number"
        text "1." width=16.0 size=11.0 align-x=center @text-muted
      "Todo"
        if block.checked
          text "✓" width=16.0 size=11.0 align-x=center font=medium @text-fg
        if !block.checked
          text "○" width=16.0 size=13.0 align-x=center @text-muted
      "Toggle"
        text "›" width=16.0 size=14.0 align-x=center @text-muted
      "Quote"
        text "│" width=16.0 size=14.0 align-x=center @text-muted
      "Code"
        text "{}" width=16.0 size=11.0 align-x=center font=mono @text-muted
      "Callout"
        text "!" width=16.0 size=11.0 align-x=center font=medium @text-muted
      _
        space width=0.0
    slot

component BlockContents(block:PageBlock)
  BlockLine block=block
    col width=fill spacing=2.0
      match block.kind
        "Page"
          row width=fill spacing=6.0 align=center
            if empty(block.text)
              text "Untitled" width=fill size=14.0 wrapping=word font=medium @text-muted
            if !empty(block.text)
              text block.text width=fill size=14.0 wrapping=word font=medium @text-fg
            text "›" size=14.0 @text-muted
        "Heading 1"
          text block.text width=fill size=20.0 wrapping=word font=display @text-fg
        "Heading 2"
          text block.text width=fill size=17.0 wrapping=word font=medium @text-fg
        "Heading 3"
          text block.text width=fill size=15.0 wrapping=word font=medium @text-fg
        "Code"
          container width=fill padding=7.0 bg=fg/7 border=white/9 border-w=1.0 r=7.0
            text block.text width=fill size=11.0 wrapping=word font=mono @text-fg
        "Divider"
          container width=fill height=1.0 bg=separator
            text ""
        _
          text block.text width=fill size=14.0 wrapping=word @text-fg

component DocumentBlock(block:PageBlock, selected:bool, hovered:bool, disabled:bool)
  mouse enter=block_entered(block.id) exit=block_exited(block.id)
    stack width=fill
      container width=fill padding-left=36.0
        row width=fill align=start
          if !empty(block.prefix)
            text block.prefix size=14.0 wrapping=none font=mono
          slot
      if !block.pending && (hovered || selected)
        container width=fill align-x=start align-y=start
          row width=fill align=center
            if !empty(block.prefix)
              text block.prefix size=14.0 wrapping=none font=mono
            row width=36.0 spacing=0.0 align=center
              button "+" label="Insert block below" disabled=disabled width=18.0 height=26.0 padding=2.0 -> open_block_insert(block.key, block.id)
                active bg=transparent text=muted r=5.0
                hovered bg=white/10 text=fg
                pressed bg=white/15
              button "⋮⋮" label="Block actions" disabled=disabled width=18.0 height=26.0 padding=1.0 -> select_block(block.key, block.id, block.kind, block.text, block.checked, true)
                active bg=transparent text=muted r=5.0
                hovered bg=white/10 text=fg
                pressed bg=white/15

component BlockActionsMenu(block_id:str, kind:str, disabled:bool, delete_armed:bool, editable_kinds:[str])
  container width=172.0 padding=4.0 bg=popover border=white/15 border-w=1.0 r=9.0 shadow=black/22 shadow-y=3.0 shadow-blur=12.0
    col width=fill spacing=2.0
      if kind != "Page"
        pick editable_kinds some(kind) placeholder="Block type" width=fill menu-height=210.0 padding=6.0 text-size=11.0 line-height=1.2 -> selected_block_kind_changed _
          active text=fg placeholder=muted handle=muted bg=transparent border=transparent border-w=0.0 r=6.0
          hovered text=fg placeholder=muted handle=fg bg=white/9 border=white/12 border-w=1.0 r=6.0
          opened text=fg placeholder=muted handle=fg bg=white/12 border=white/15 border-w=1.0 r=6.0
          menu text=fg selected-text=fg selected-bg=white/18 bg=popover border=white/16 border-w=1.0 r=8.0 shadow=black/16 shadow-y=3.0 shadow-blur=10.0
      if kind == "Page"
        button "Open page" label="Open subpage" disabled=disabled width=fill height=28.0 padding=6.0 -> choose_page(block_id)
          active bg=transparent text=muted r=6.0
          hovered bg=white/10 text=fg
          pressed bg=white/15
      row width=fill spacing=2.0 align=center
        button "↑" label="Move block up" disabled=disabled width=40.0 height=27.0 padding=4.0 -> move_block_submit("up")
          active bg=transparent text=muted r=6.0
          hovered bg=white/10 text=fg
          pressed bg=white/15
        button "↓" label="Move block down" disabled=disabled width=40.0 height=27.0 padding=4.0 -> move_block_submit("down")
          active bg=transparent text=muted r=6.0
          hovered bg=white/10 text=fg
          pressed bg=white/15
        button "←" label="Outdent block" disabled=disabled width=40.0 height=27.0 padding=4.0 -> move_block_submit("outdent")
          active bg=transparent text=muted r=6.0
          hovered bg=white/10 text=fg
          pressed bg=white/15
        button "→" label="Indent block" disabled=disabled width=40.0 height=27.0 padding=4.0 -> move_block_submit("indent")
          active bg=transparent text=muted r=6.0
          hovered bg=white/10 text=fg
          pressed bg=white/15
      if kind == "Todo"
        button "Toggle done" label="Toggle checked" disabled=disabled width=fill height=28.0 padding=6.0 -> toggle_block_checked
          active bg=transparent text=muted r=6.0
          hovered bg=white/10 text=fg
          pressed bg=white/15
      button "Comments" label="Comments" disabled=disabled width=fill height=28.0 padding=6.0 -> open_block_comments
        active bg=transparent text=muted r=6.0
        hovered bg=white/10 text=fg
        pressed bg=white/15
      if !delete_armed
        button "Delete" label="Delete block" disabled=disabled width=fill height=28.0 padding=6.0 -> arm_block_delete
          active bg=transparent text=muted r=6.0
          hovered bg=white/10 text=fg
          pressed bg=white/15
      if delete_armed
        button "Confirm delete" label="Confirm block deletion" disabled=disabled width=fill height=28.0 padding=6.0 -> remove_block_submit
          active bg=white/12 text=fg r=6.0
          hovered bg=white/17
          pressed bg=white/22
      button "Close" label="Close block actions" disabled=disabled width=fill height=28.0 padding=6.0 -> close_block_actions
        active bg=transparent text=muted r=6.0
        hovered bg=white/10 text=fg
        pressed bg=white/15

component InlineBlockInsert(kind:str, kinds:[str], disabled:bool, prefix:str)
  stack width=fill
    container width=fill padding-left=36.0 padding-right=118.0
      row width=fill
        if !empty(prefix)
          text prefix size=14.0 wrapping=none font=mono
        slot
    container width=fill align-x=end align-y=start padding-right=4.0
      container padding=2.0 bg=popover border=white/14 border-w=1.0 r=8.0 shadow=black/14 shadow-y=2.0 shadow-blur=7.0
        row spacing=1.0 align=center
          pick kinds some(kind) placeholder="Type" width=82.0 menu-height=210.0 padding=4.0 text-size=11.0 line-height=1.2 -> new_block_kind_changed _
            active text=fg placeholder=muted handle=muted bg=transparent border=transparent border-w=0.0 r=6.0
            hovered text=fg placeholder=muted handle=fg bg=white/9 border=white/12 border-w=1.0 r=6.0
            opened text=fg placeholder=muted handle=fg bg=white/12 border=white/15 border-w=1.0 r=6.0
            menu text=fg selected-text=fg selected-bg=white/18 bg=popover border=white/16 border-w=1.0 r=8.0 shadow=black/16 shadow-y=3.0 shadow-blur=10.0
          button "×" label="Cancel block insertion" disabled=disabled width=26.0 height=26.0 padding=4.0 -> close_block_insert
            active bg=transparent text=muted r=6.0
            hovered bg=white/10 text=fg
            pressed bg=white/15
