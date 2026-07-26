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
  col w=fill gap=2.0
    if !editing && !empty(title)
      button label=title disabled=disabled w=fill p=4.0 @ghost_action -> begin(title)
        text title w=fill size=22.0 wrap=none font=display @text-fg
        active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
        hovered bg=fg/4 text=fg border=fg/6
        pressed bg=fg/7 text=fg border=fg/9
    if !editing && empty(title)
      button label="Untitled" disabled=disabled w=fill p=4.0 @ghost_action -> begin(title)
        text "Untitled" w=fill size=22.0 wrap=none font=display @text-muted
        active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
        hovered bg=fg/4 text=fg border=fg/6
        pressed bg=fg/7 text=fg border=fg/9
    if editing
      input "" #title-input label="Page title" <-> draft change=changed(_, rpc, password, page_id) hint="Untitled" disabled=disabled w=fill p=4.0 text-size=22.0 line-h=1.15 @control
        active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
        disabled value=muted
    if !empty(local_error)
      text local_error size=12.5 @text-muted

component PageButton(page:PageItem, selected:bool)
  col w=fill
    if selected
      button label=page.title w=fill h=34.0 p=7.0 @ghost_action -> choose_page(page.id)
        row w=fill h=fill gap=9.0 align=center
          text "▤" w=18.0 size=13.0 align-x=center @text-fg
          text page.prefix size=12.0 wrap=none font=code @text-muted
          text page.title w=fill size=13.0 wrap=none font=medium @text-fg
          if page.child_count > 0
            text page.child_count size=12.0 font=code @text-muted
        active bg=subtle text=fg border=transparent border-w=1.0 r=8.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg
    if !selected
      button label=page.title w=fill h=34.0 p=7.0 @ghost_action -> choose_page(page.id)
        row w=fill h=fill gap=9.0 align=center
          text "▤" w=18.0 size=13.0 align-x=center @text-muted
          text page.prefix size=12.0 wrap=none font=code @text-muted
          text page.title w=fill size=13.0 wrap=none @text-muted
          if page.child_count > 0
            text page.child_count size=12.0 font=code @text-muted
        active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
        hovered bg=fg/6 text=fg border=fg/8
        pressed bg=fg/10 text=fg border=fg/12

component PageSearchResult(hit:PageSearchHit)
  button label=hit.text w=fill p=7.0 @ghost_action -> open_page_search_hit(hit.page_id, hit.block_id)
    col w=fill gap=2.0
      row w=fill gap=7.0 align=center
        text hit.kind w=fill size=10.5 font=code_medium @text-muted
        text hit.block_id size=12.0 wrap=none font=code @text-muted
      text hit.text w=fill size=13.5 wrap=word @text-fg
    active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
    hovered bg=fg/6 text=fg border=fg/8
    pressed bg=fg/10 text=fg border=fg/12

component PageCommentThreadButton(thread:PageCommentThread)
  button label=thread.author description=thread.meta w=fill p=6.0 @ghost_action -> open_block_comment_thread(thread.id)
    row w=fill gap=7.0 align=center
      text thread.author w=fill size=13.0 wrap=none font=medium @text-fg
      text thread.meta size=11.0 wrap=none font=code_medium @text-muted
      text "›" size=13.0 @text-muted
    active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
    hovered bg=fg/6 text=fg border=fg/8
    pressed bg=fg/10 text=fg border=fg/12

component PageCommentCard(comment:PageComment)
  box w=fill p=7.0 bg=transparent border=transparent border-w=1.0 r=7.0
    col w=fill gap=3.0
      row w=fill gap=7.0 align=center
        text comment.author w=fill size=13.0 wrap=none font=medium @text-fg
        text comment.meta size=11.0 wrap=none font=code_medium @text-muted
      text comment.text w=fill size=13.5 wrap=word @text-fg

component BlockLine(block:PageBlock)
  row w=fill gap=7.0 align=start
    match block.kind
      "Page"
        text "▤" w=16.0 size=13.0 align-x=center @text-muted
      "Bullet"
        text "•" w=16.0 size=13.0 align-x=center @text-muted
      "Number"
        text "1." w=16.0 size=12.0 align-x=center @text-muted
      "Todo"
        if block.checked
          text "✓" w=16.0 size=12.0 align-x=center font=medium @text-fg
        if !block.checked
          text "○" w=16.0 size=13.0 align-x=center @text-muted
      "Toggle"
        text "›" w=16.0 size=14.0 align-x=center @text-muted
      "Quote"
        text "│" w=16.0 size=14.0 align-x=center @text-muted
      "Code"
        text "{}" w=16.0 size=12.0 align-x=center font=code @text-muted
      "Callout"
        text "!" w=16.0 size=12.0 align-x=center font=medium @text-muted
      _
        space w=0.0
    slot

component BlockContents(block:PageBlock)
  BlockLine block=block
    col w=fill gap=2.0
      match block.kind
        "Page"
          row w=fill gap=6.0 align=center
            if empty(block.text)
              text "Untitled" w=fill size=13.0 wrap=word font=medium @text-muted
            if !empty(block.text)
              text block.text w=fill size=13.0 wrap=word font=medium @text-fg
            text "›" size=13.0 @text-muted
        "Heading 1"
          text block.text w=fill size=20.0 wrap=word font=display @text-fg
        "Heading 2"
          text block.text w=fill size=16.0 wrap=word font=display @text-fg
        "Heading 3"
          text block.text w=fill size=14.0 wrap=word font=display @text-fg
        "Code"
          box w=fill p=7.0 bg=fg/7 border=fg/9 border-w=1.0 r=7.0
            text block.text w=fill size=12.0 wrap=word font=code @text-fg
        "Divider"
          Separator
        _
          text block.text w=fill size=13.5 wrap=word @text-fg

component DocumentBlock(block:PageBlock, selected:bool, hovered:bool, disabled:bool)
  mouse enter=block_entered(block.id) exit=block_exited(block.id)
    stack w=fill
      box w=fill pl=56.0
        row w=fill align=start
          if !empty(block.prefix)
            text block.prefix size=12.0 wrap=none font=code
          slot
      if !block.pending && (hovered || selected)
        box w=fill align-x=start align-y=start
          row w=fill align=center
            if !empty(block.prefix)
              text block.prefix size=12.0 wrap=none font=code
            box w=56.0 h=28.0 bg=surface border=border border-w=1.0 r=7.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
              row w=fill gap=0.0 align=center
                button label="Insert block below" disabled=disabled w=28.0 h=28.0 p=0.0 @ghost_action -> open_block_insert(block.key, block.id)
                  box w=fill h=fill align-x=center align-y=center
                    text "+" size=14.0 font=medium
                  active bg=transparent text=muted r=5.0
                  hovered bg=fg/8 text=fg
                  pressed bg=fg/12 text=fg
                button label="Block actions" disabled=disabled w=28.0 h=28.0 p=0.0 @ghost_action -> select_block(block.key, block.id, block.kind, block.text, block.checked, true)
                  box w=fill h=fill align-x=center align-y=center
                    text "⋮⋮" size=13.0 font=medium
                  active bg=transparent text=muted r=5.0
                  hovered bg=fg/8 text=fg
                  pressed bg=fg/12 text=fg

component BlockActionsMenu(block_id:str, kind:str, disabled:bool, delete_armed:bool, editable_kinds:[str])
  box w=172.0 p=5.0 bg=surface border=border border-w=1.0 r=10.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
    col w=fill gap=3.0
      if kind != "Page"
        pick editable_kinds some(kind) hint="Block type" w=fill menu-h=210.0 p=6.0 text-size=13.0 line-h=1.2 -> selected_block_kind_changed _
          active text=fg placeholder=muted handle=muted bg=transparent border=transparent border-w=0.0 r=6.0
          hovered text=fg placeholder=muted handle=fg bg=fg/8 border=fg/10 border-w=1.0 r=6.0
          opened text=fg placeholder=muted handle=fg bg=fg/11 border=ring border-w=1.0 r=6.0
          menu text=fg selected-text=fg selected-bg=fg/14 bg=surface border=border border-w=1.0 r=8.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
      if kind == "Page"
        button "Open page" label="Open subpage" disabled=disabled w=fill h=28.0 p=6.0 @ghost_action -> choose_page(block_id)
          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
          hovered bg=fg/8 text=fg border=fg/9
          pressed bg=fg/12 text=fg border=fg/13
      row w=fill gap=2.0 align=center
        button "↑" label="Move block up" disabled=disabled w=fill h=27.0 p=4.0 @ghost_action -> move_block_submit("up")
          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
          hovered bg=fg/8 text=fg border=fg/9
          pressed bg=fg/12 text=fg border=fg/13
        button "↓" label="Move block down" disabled=disabled w=fill h=27.0 p=4.0 @ghost_action -> move_block_submit("down")
          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
          hovered bg=fg/8 text=fg border=fg/9
          pressed bg=fg/12 text=fg border=fg/13
        button "←" label="Outdent block" disabled=disabled w=fill h=27.0 p=4.0 @ghost_action -> move_block_submit("outdent")
          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
          hovered bg=fg/8 text=fg border=fg/9
          pressed bg=fg/12 text=fg border=fg/13
        button "→" label="Indent block" disabled=disabled w=fill h=27.0 p=4.0 @ghost_action -> move_block_submit("indent")
          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
          hovered bg=fg/8 text=fg border=fg/9
          pressed bg=fg/12 text=fg border=fg/13
      if kind == "Todo"
        button "Toggle done" label="Toggle checked" disabled=disabled w=fill h=28.0 p=6.0 @ghost_action -> toggle_block_checked
          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
          hovered bg=fg/8 text=fg border=fg/9
          pressed bg=fg/12 text=fg border=fg/13
      button "Comments" label="Comments" disabled=disabled w=fill h=28.0 p=6.0 @ghost_action -> open_block_comments
        active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
        hovered bg=fg/8 text=fg border=fg/9
        pressed bg=fg/12 text=fg border=fg/13
      if !delete_armed
        button "Delete" label="Delete block" disabled=disabled w=fill h=28.0 p=6.0 @danger_action -> arm_block_delete
      if delete_armed
        button "Confirm delete" label="Confirm block deletion" disabled=disabled w=fill h=28.0 p=6.0 @danger_action -> remove_block_submit
      button "Close" label="Close block actions" disabled=disabled w=fill h=28.0 p=6.0 @secondary_action -> close_block_actions
        active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
        hovered bg=fg/8 text=fg border=fg/9
        pressed bg=fg/12 text=fg border=fg/13

component InlineBlockInsert(kind:str, kinds:[str], disabled:bool, prefix:str)
  stack w=fill
    box w=fill pl=56.0 pr=118.0
      row w=fill
        if !empty(prefix)
          text prefix size=12.0 wrap=none font=code
        slot
    box w=fill align-x=end align-y=start pr=4.0
      box p=2.0 bg=surface border=border border-w=1.0 r=8.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
        row gap=1.0 align=center
          pick kinds some(kind) hint="Type" w=82.0 menu-h=210.0 p=4.0 text-size=13.0 line-h=1.2 -> new_block_kind_changed _
            active text=fg placeholder=muted handle=muted bg=transparent border=transparent border-w=0.0 r=6.0
            hovered text=fg placeholder=muted handle=fg bg=fg/8 border=fg/10 border-w=1.0 r=6.0
            opened text=fg placeholder=muted handle=fg bg=fg/11 border=ring border-w=1.0 r=6.0
            menu text=fg selected-text=fg selected-bg=fg/14 bg=surface border=border border-w=1.0 r=8.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
          button "×" label="Cancel block insertion" disabled=disabled w=26.0 h=26.0 p=4.0 @secondary_action -> close_block_insert
            active bg=transparent text=muted r=6.0
            hovered bg=fg/8 text=fg
            pressed bg=fg/12 text=fg
