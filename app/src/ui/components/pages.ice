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
      button -> emit(choose_page, page.id)
        with
          label=page.title
          w=fill
          @ghost_action
          @px-12px
          @py-7px
        row
          with
            w=fill
            gap=8.0
            align=center
          if !empty(page.prefix)
            text page.prefix
              with
                size=12.0
                wrap=none
                font=code
                @text-label
          Icon
            with
              name="doc"
              tone="label"
              px=14.0
          box w=fill clip=true
            text page.title
              with
                size=12.5
                wrap=none
                @text-fg
        active bg=selected_row text=fg border=transparent border-w=1.0 r=7.0
        hovered bg=rail_hover text=fg
        pressed bg=selected_row text=fg
    if !selected
      button -> emit(choose_page, page.id)
        with
          label=page.title
          w=fill
          @ghost_action
          @px-12px
          @py-7px
        row
          with
            w=fill
            gap=8.0
            align=center
          if !empty(page.prefix)
            text page.prefix
              with
                size=12.0
                wrap=none
                font=code
                @text-label
          Icon
            with
              name="doc"
              tone="label"
              px=14.0
          box w=fill clip=true
            text page.title
              with
                size=12.5
                wrap=none
                @text-muted
        active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
        hovered bg=rail_hover text=fg
        pressed bg=subtle text=fg

component PageSearchResult(hit:PageSearchHit)
  emits
    open_page_search_hit(str, str)
  button -> emit(open_page_search_hit, hit.page_id, hit.block_id)
    with
      label=hit.text
      w=fill
      p=7.0
      @ghost_action
    col w=fill gap=2.0
      row
        with
          w=fill
          gap=7.0
          align=center
        text hit.kind
          with
            w=fill
            size=10.5
            font=code_medium
            @text-muted
        text hit.block_id
          with
            size=12.0
            wrap=none
            font=code
            @text-muted
      text hit.text
        with
          w=fill
          size=13.5
          wrap=word
          @text-fg
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
component PageCommentThreadButton(thread:PageCommentThread, anchor:str)
  emits
    open_block_comment_thread(str, str)
  button -> emit(open_block_comment_thread, thread.id, thread.target)
    with
      label=thread.author
      description=thread.meta
      w=fill
      @ghost_action
      @px-12px
      @py-11px
    col w=fill gap=7.0
      row
        with
          w=fill
          gap=8.0
          align=center
        PrincipalAvatar
          with
            initials=initials_of(thread.author)
            is_agent=false
            plate=22.0
            ink=9.0
            ring=""
        text thread.author
          with
            w=fill
            size=12.0
            wrap=none
            font=display
            @text-primary
        text "›"
          with
            size=13.0
            wrap=none
            @text-label
      // WHERE it anchors — the one thing the old rail never told you.
      text anchor
        with
          w=fill
          size=10.5
          wrap=none
          font=code_medium
          @text-hint
      text thread.meta
        with
          w=fill
          size=12.0
          line-h=1.55
          wrap=word
          @text-accent_fg
    active bg=surface text=fg border=card_line border-w=1.0 r=11.0
    hovered bg=card_wash_hover text=fg border=control_line
    pressed bg=card_wash text=fg border=control_line

component PageCommentCard(comment:PageComment)
  box
    with
      w=fill
      pl=12.0
      pr=12.0
      pt=11.0
      pb=11.0
      bg=surface
      border=card_line
      border-w=1.0
      r=11.0
    col w=fill gap=7.0
      row
        with
          w=fill
          gap=8.0
          align=center
        PrincipalAvatar
          with
            initials=initials_of(comment.author)
            is_agent=false
            plate=22.0
            ink=9.0
            ring=""
        text comment.author
          with
            w=fill
            size=12.0
            wrap=none
            font=display
            @text-primary
        text comment.meta
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-label
      text comment.text
        with
          w=fill
          size=12.0
          line-h=1.55
          wrap=word
          @text-accent_fg
