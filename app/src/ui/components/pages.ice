// Pages — the document column and the comments rail.
//
// Painted from "Ducktape Console - Liquid Glass.dc.html":885-965, which is the
// ONLY handoff file that carries a Pages screen (the designer's non-glass
// variant has no `isPages`). Nothing in this surface is glass: every plate the
// artifact draws here is already an opaque hex.
//
// THREE VALUES THE APP CANNOT SPELL EXACTLY, recorded once here instead of at
// each call site:
//   * quote text is 15px and code text is 11.5px in the artifact; neither is on
//     `design::type_scale::ALL`, so they take the neighbouring steps 14.0/12.0.
//   * the code plate is #201f1a; the nearest token is `primary` #26251f. Its
//     ink #c8c6bc IS a token value (`chevron_idle`) under another name.
//   * the callout tile is #ece1d2; `brand_bg` #f9f1ea is the artifact's own
//     accent wash and the tile's ink `brand` #a05a3c is exact.
// A `text` line at 12.5 may carry no `font=` (the design-system guard pins the
// caption step to weight 400), so the sidebar rows are 400 where the artifact
// is 500.

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

// SIDEBAR ROW — `margin:1px 0;padding:7px 12px;border-radius:7px;gap:8px`, no
// fixed height, the doc line icon rather than a ▤ glyph, and the SAME #f0efea
// hover plate in both states (Liquid Glass:893-895).
//
// `page.prefix` is two spaces per depth (backend.rs:5439). The artifact indents
// the row itself by `11 + depth * 15`px, which needs a depth NUMBER — until
// `PageItem` carries one the prefix stays as the only hierarchy signal, moved
// ahead of the icon so it indents the whole row instead of the title alone.
component PageButton(page:PageItem, selected:bool)
  emits
    choose_page(str)
  col w=fill
    if selected
      button label=page.title w=fill @ghost_action px-12px py-7px -> emit(choose_page, page.id)
        row w=fill gap=8.0 align=center
          if !empty(page.prefix)
            text page.prefix size=12.0 wrap=none font=code @text-label
          Icon name="doc" tone="label" px=14.0
          text page.title w=fill size=12.5 wrap=none @text-fg
        active bg=subtle text=fg border=transparent border-w=1.0 r=7.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg
    if !selected
      button label=page.title w=fill @ghost_action px-12px py-7px -> emit(choose_page, page.id)
        row w=fill gap=8.0 align=center
          if !empty(page.prefix)
            text page.prefix size=12.0 wrap=none font=code @text-label
          Icon name="doc" tone="label" px=14.0
          text page.title w=fill size=12.5 wrap=none @text-muted
        active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg

component PageSearchResult(hit:PageSearchHit)
  emits
    open_page_search_hit(str, str)
  button label=hit.text w=fill p=7.0 @ghost_action -> emit(open_page_search_hit, hit.page_id, hit.block_id)
    col w=fill gap=2.0
      row w=fill gap=7.0 align=center
        text hit.kind w=fill size=10.5 font=code_medium @text-muted
        text hit.block_id size=12.0 wrap=none font=code @text-muted
      text hit.text w=fill size=13.5 wrap=word @text-fg
    active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
    hovered bg=fg/6 text=fg border=fg/8
    pressed bg=fg/10 text=fg border=fg/12

// COMMENT CARDS — `border:1px solid #ece9e1;background:#fff;border-radius:11px;
// padding:11px 12px` with a 22px principal plate, the author at 600 12px and
// the body at 12px/1.55 (Liquid Glass:944-953).
//
// TWO PARTS OF THE ARTIFACT CARD ARE NOT DRAWN, because nothing on the wire
// carries them and a plausible one would be a lie:
//   * the AGENT badge and the square plate — `PageComment`/`PageCommentThread`
//     carry a flattened author STRING, and `page_author_name` throws the
//     `AuthorRef::Agent` variant away (backend.rs:5213). Every plate is drawn
//     as a person until the discriminant survives the boundary.
//   * the relative timestamp and the anchor quote. NOT because the wire lacks
//     them — `ThreadRow` carries both `anchor` and `created_at`
//     (crates/modules/apps/pages/src/index.rs) — but because
//     `page_comment_thread` (backend.rs) drops the pair before the boundary.
//     Carrying them through is a projection change, not a view change; until
//     then the right-hand slot shows the ordinal the record does have
//     (`#3`, `#3 · edited`) rather than an invented age.
component PageCommentThreadButton(thread:PageCommentThread)
  emits
    open_block_comment_thread(str)
  button label=thread.author description=thread.meta w=fill @ghost_action px-12px py-11px -> emit(open_block_comment_thread, thread.id)
    col w=fill gap=7.0
      row w=fill gap=8.0 align=center
        PrincipalAvatar initials=initials_of(thread.author) is_agent=false plate=22.0 ink=9.0 ring=""
        text thread.author w=fill size=12.0 wrap=none font=display @text-primary
        text "›" size=13.0 wrap=none @text-label
      text thread.meta w=fill size=12.0 line-h=1.55 wrap=word @text-panel_tile
    active bg=surface text=fg border=card_line border-w=1.0 r=11.0
    hovered bg=card_wash_hover text=fg border=control_line
    pressed bg=card_wash text=fg border=control_line

component PageCommentCard(comment:PageComment)
  box w=fill pl=12.0 pr=12.0 pt=11.0 pb=11.0 bg=surface border=card_line border-w=1.0 r=11.0
    col w=fill gap=7.0
      row w=fill gap=8.0 align=center
        PrincipalAvatar initials=initials_of(comment.author) is_agent=false plate=22.0 ink=9.0 ring=""
        text comment.author w=fill size=12.0 wrap=none font=display @text-primary
        text comment.meta size=10.5 wrap=none font=code_medium @text-label
      text comment.text w=fill size=12.0 line-h=1.55 wrap=word @text-panel_tile

// The rail's docked composer (Liquid Glass:959 — a bordered field with a 27px
// ink ↑ send key) is NOT a component: `input` binds `<->` to state, a component
// only owns local state, and the draft has to be the app's `block_comment_draft`
// for `post_block_comment_submit` to read and clear it. It stays in view.ice.

// THE BLOCK GUTTER, for the row being EDITED. The read view draws its markers
// inline (see BlockContents) exactly as the artifact does — only the editor
// keeps a gutter, so the caret does not jump when a block is selected.
component BlockLine(block:PageBlock)
  emits
    set_todo_checked(str, bool)
  row w=fill gap=0.0 align=start
    match block.kind
      "Page"
        box pr=11.0
          Icon name="doc" tone="label" px=14.0
      "Bullet"
        box pt=9.0 pr=11.0
          BulletDot
      "Number"
        box pr=11.0
          text "1." size=12.0 wrap=none font=code @text-label
      "Todo"
        box pt=3.0 pr=11.0
          TodoCheckbox block=block
            forward
              set_todo_checked
      "Toggle"
        box pr=11.0
          Icon name="chevron-right" tone="label" px=14.0
      _
        space w=0.0
    slot

// THE BLOCK RHYTHM. The artifact gives every kind its own top margin — h2 20,
// paragraph 8, bullet 6, todo 7, callout 14, quote 16, code 14 — so a heading
// opens a section (Liquid Glass:913-926). A flat gap does not.
component BlockContents(block:PageBlock)
  emits
    set_todo_checked(str, bool)
  col w=fill
    match block.kind
      "Page"
        box w=fill pt=8.0
          row w=fill gap=8.0 align=center
            Icon name="doc" tone="label" px=14.0
            if empty(block.text)
              text "Untitled" w=fill size=13.5 wrap=word font=medium @text-muted
            if !empty(block.text)
              text block.text w=fill size=13.5 wrap=word font=medium @text-fg
            text "›" size=13.0 wrap=none @text-label
      "Heading 1"
        box w=fill pt=20.0
          text block.text w=fill size=20.0 line-h=1.25 wrap=word font=display @text-primary
      "Heading 2"
        box w=fill pt=20.0
          text block.text w=fill size=16.0 line-h=1.3 wrap=word font=display @text-primary
      "Heading 3"
        box w=fill pt=16.0
          text block.text w=fill size=14.0 line-h=1.35 wrap=word font=display @text-primary
      "Bullet"
        BulletBlock body=block.text
      "Number"
        box w=fill pt=6.0
          row w=fill gap=11.0 align=start
            text "1." size=12.0 wrap=none font=code @text-label
            text block.text w=fill size=14.0 line-h=1.65 wrap=word @text-accent_fg
      "Todo"
        TodoBlock block=block
          forward
            set_todo_checked
      "Toggle"
        box w=fill pt=8.0
          row w=fill gap=11.0 align=start
            Icon name="chevron-right" tone="label" px=14.0
            text block.text w=fill size=14.0 line-h=1.7 wrap=word @text-accent_fg
      "Quote"
        QuoteBlock body=block.text
      "Callout"
        CalloutBlock body=block.text
      "Code"
        CodeBlock body=block.text
      "Divider"
        Separator
      _
        box w=fill pt=8.0
          text block.text w=fill size=14.0 line-h=1.7 wrap=word @text-accent_fg

// A 5px dot on the gutter ink, not a • glyph in a 16px cell.
component BulletDot()
  box #root w=5.0 h=5.0 bg=gutter_ink r=2.5
    space w=1.0 h=1.0

component BulletBlock(body:str)
  box #root w=fill pt=6.0
    row w=fill gap=0.0 align=start
      box pt=9.0 pr=11.0
        BulletDot
      text body w=fill size=14.0 line-h=1.65 wrap=word @text-accent_fg

// ONE CLICK FINALIZES THE TICK. The DRAWN box stays 17px r5 — ink when done,
// a 1.5px hairline when open — inside a 24px transparent target: a tick is a
// precision task at 17px, and the pointer feedback moved to a wash around the
// box because a button's hover styling cannot reach into its child.
component TodoCheckbox(block:PageBlock)
  emits
    set_todo_checked(str, bool)
  col #root
    if block.checked
      button label="Mark not done" w=24.0 h=24.0 p=0.0 @icon_action -> emit(set_todo_checked, block.id, false)
        box w=17.0 h=17.0 align-x=center align-y=center bg=primary border=primary border-w=1.5 r=5.0
          text "✓" size=9.0 wrap=none font=code_semibold @text-primary_fg
        active bg=transparent text=primary_fg border=transparent border-w=1.0 r=7.0
        hovered bg=fg/8
        pressed bg=fg/12
    if !block.checked
      button label="Mark done" w=24.0 h=24.0 p=0.0 @icon_action -> emit(set_todo_checked, block.id, true)
        box w=17.0 h=17.0 bg=surface border=control_line_hover border-w=1.5 r=5.0
          space w=1.0 h=1.0
        active bg=transparent border=transparent border-w=1.0 r=7.0
        hovered bg=fg/8
        pressed bg=fg/12

// A done todo fades to `meta` and strikes through; an open one keeps body ink.
component TodoBlock(block:PageBlock)
  emits
    set_todo_checked(str, bool)
  box #root w=fill pt=7.0
    row w=fill gap=0.0 align=start
      box pt=3.0 pr=11.0
        TodoCheckbox block=block
          forward
            set_todo_checked
      if block.checked
        rich-text w=fill size=14.0 line-h=1.65 wrap=word color=meta
          span block.text strike
      if !block.checked
        text block.text w=fill size=14.0 line-h=1.65 wrap=word @text-accent_fg

// The rule IS the marker — the artifact draws no gutter glyph on a quote.
component QuoteBlock(body:str)
  box #root w=fill pt=16.0
    row w=fill gap=0.0 align=start
      box w=2.0 h=fill bg=control_line
        space w=1.0 h=1.0
      box w=fill pl=14.0
        text body w=fill size=14.0 line-h=1.6 wrap=word font=italic @text-muted

component CalloutBlock(body:str)
  box #root w=fill pt=14.0
    box w=fill pl=15.0 pr=15.0 pt=13.0 pb=13.0 bg=card_wash border=separator border-w=1.0 r=11.0
      row w=fill gap=11.0 align=start
        box w=20.0 h=20.0 align-x=center align-y=center bg=brand_bg r=6.0
          text "i" size=10.0 wrap=none font=code_semibold @text-brand
        text body w=fill size=13.0 line-h=1.6 wrap=word @text-panel_tile

// Code is a dark card and it never reflows: the artifact keeps `white-space:pre`
// and lets the plate scroll. `wrap=none` + a clip is that rule without nesting a
// scroll area inside a clickable block.
component CodeBlock(body:str)
  box #root w=fill pt=14.0
    box w=fill pl=15.0 pr=15.0 pt=13.0 pb=13.0 bg=primary r=10.0 clip=true
      text body w=fill size=12.0 line-h=1.6 wrap=none font=code @text-chevron_idle

component DocumentBlock(block:PageBlock, selected:bool, hovered:bool, disabled:bool)
  emits
    block_entered(str)
    block_exited(str)
    open_block_insert(i64, str)
    select_block(i64, str, str, str, bool, bool)
  mouse enter=emit(block_entered, block.id) exit=emit(block_exited, block.id)
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
                button label="Insert block below" disabled=disabled w=28.0 h=28.0 p=0.0 @icon_action -> emit(open_block_insert, block.key, block.id)
                  box w=fill h=fill align-x=center align-y=center
                    text "+" size=14.0 font=medium
                  active bg=transparent text=muted r=5.0
                  hovered bg=fg/8 text=fg
                  pressed bg=fg/12 text=fg
                button label="Block actions" disabled=disabled w=28.0 h=28.0 p=0.0 @icon_action -> emit(select_block, block.key, block.id, block.kind, block.text, block.checked, true)
                  box w=fill h=fill align-x=center align-y=center
                    text "⋮⋮" size=13.0 font=medium
                  active bg=transparent text=muted r=5.0
                  hovered bg=fg/8 text=fg
                  pressed bg=fg/12 text=fg

component BlockActionsMenu(block_id:str, kind:str, disabled:bool, delete_armed:bool, editable_kinds:[str])
  emits
    selected_block_kind_changed(str)
    choose_page(str)
    move_block_submit(str)
    open_block_comments
    arm_block_delete
    remove_block_submit
    close_block_actions
  box w=172.0 p=5.0 bg=surface border=border border-w=1.0 r=10.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
    col w=fill gap=3.0
      if kind != "Page"
        pick editable_kinds some(kind) hint="Block type" w=fill menu-h=210.0 p=6.0 text-size=13.0 line-h=1.2 -> emit(selected_block_kind_changed, _)
          active text=fg placeholder=muted handle=muted bg=transparent border=transparent border-w=0.0 r=6.0
          hovered text=fg placeholder=muted handle=fg bg=fg/8 border=fg/10 border-w=1.0 r=6.0
          opened text=fg placeholder=muted handle=fg bg=fg/11 border=ring border-w=1.0 r=6.0
          menu text=fg selected-text=fg selected-bg=fg/14 bg=surface border=border border-w=1.0 r=8.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
      if kind == "Page"
        button "Open page" label="Open subpage" disabled=disabled w=fill h=28.0 p=6.0 @ghost_action -> emit(choose_page, block_id)
          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
          hovered bg=fg/8 text=fg border=fg/9
          pressed bg=fg/12 text=fg border=fg/13
      row w=fill gap=2.0 align=center
        button "↑" label="Move block up" disabled=disabled w=fill h=27.0 p=4.0 @ghost_action -> emit(move_block_submit, "up")
          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
          hovered bg=fg/8 text=fg border=fg/9
          pressed bg=fg/12 text=fg border=fg/13
        button "↓" label="Move block down" disabled=disabled w=fill h=27.0 p=4.0 @ghost_action -> emit(move_block_submit, "down")
          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
          hovered bg=fg/8 text=fg border=fg/9
          pressed bg=fg/12 text=fg border=fg/13
        button "←" label="Outdent block" disabled=disabled w=fill h=27.0 p=4.0 @ghost_action -> emit(move_block_submit, "outdent")
          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
          hovered bg=fg/8 text=fg border=fg/9
          pressed bg=fg/12 text=fg border=fg/13
        button "→" label="Indent block" disabled=disabled w=fill h=27.0 p=4.0 @ghost_action -> emit(move_block_submit, "indent")
          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
          hovered bg=fg/8 text=fg border=fg/9
          pressed bg=fg/12 text=fg border=fg/13
      button "Comments" label="Comments" disabled=disabled w=fill h=28.0 p=6.0 @ghost_action -> emit(open_block_comments)
        active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
        hovered bg=fg/8 text=fg border=fg/9
        pressed bg=fg/12 text=fg border=fg/13
      if !delete_armed
        button "Delete" label="Delete block" disabled=disabled w=fill h=28.0 p=6.0 @danger_action -> emit(arm_block_delete)
      if delete_armed
        button "Confirm delete" label="Confirm block deletion" disabled=disabled w=fill h=28.0 p=6.0 @danger_action -> emit(remove_block_submit)
      button "Close" label="Close block actions" disabled=disabled w=fill h=28.0 p=6.0 @secondary_action -> emit(close_block_actions)
        active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
        hovered bg=fg/8 text=fg border=fg/9
        pressed bg=fg/12 text=fg border=fg/13

component InlineBlockInsert(kind:str, kinds:[str], disabled:bool, prefix:str)
  emits
    new_block_kind_changed(str)
    close_block_insert
  stack w=fill
    box w=fill pl=56.0 pr=118.0
      row w=fill
        if !empty(prefix)
          text prefix size=12.0 wrap=none font=code
        slot
    box w=fill align-x=end align-y=start pr=4.0
      box p=2.0 bg=surface border=border border-w=1.0 r=8.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
        row gap=1.0 align=center
          pick kinds some(kind) hint="Type" w=82.0 menu-h=210.0 p=4.0 text-size=13.0 line-h=1.2 -> emit(new_block_kind_changed, _)
            active text=fg placeholder=muted handle=muted bg=transparent border=transparent border-w=0.0 r=6.0
            hovered text=fg placeholder=muted handle=fg bg=fg/8 border=fg/10 border-w=1.0 r=6.0
            opened text=fg placeholder=muted handle=fg bg=fg/11 border=ring border-w=1.0 r=6.0
            menu text=fg selected-text=fg selected-bg=fg/14 bg=surface border=border border-w=1.0 r=8.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
          button "×" label="Cancel block insertion" disabled=disabled w=26.0 h=26.0 p=4.0 @secondary_action -> emit(close_block_insert)
            active bg=transparent text=muted r=6.0
            hovered bg=fg/8 text=fg
            pressed bg=fg/12 text=fg
