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
// The pages body's rich runs — chat's RichLine at the document body metrics.
// Word-level spans reflow in a wrapping flex; links and mentions light up
// exactly as the chat renderer paints them. Headings, quotes, callouts and
// code keep their single plain run — their own fonts already carry the voice.
component PageRichLine(spans:[ChatSpan], line_h:f64)
  flex w=fill wrap=wrap gap-x=0.0 gap-y=4.0 items=start
    for span in spans
      if span.highlight
        text span.text size=14.0 line-h=line_h font=medium @text-brand
      if !span.highlight && span.bold && span.italic
        text span.text size=14.0 line-h=line_h font=strongitalic @text-accent_fg
      if !span.highlight && span.bold && !span.italic
        text span.text size=14.0 line-h=line_h font=strong @text-accent_fg
      if !span.highlight && !span.bold && span.italic
        text span.text size=14.0 line-h=line_h font=italic @text-accent_fg
      if !span.highlight && !span.bold && !span.italic
        text span.text size=14.0 line-h=line_h @text-accent_fg