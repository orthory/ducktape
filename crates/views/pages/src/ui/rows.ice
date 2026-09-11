// SIDEBAR ROW — `margin:1px 0;padding:7px 12px;border-radius:7px;gap:8px`, no
// fixed height, the doc line icon rather than a ▤ glyph, and the SAME #f0efea
// hover plate in both states (Liquid Glass:893-895).
//
// `page.prefix` is two spaces per depth (backend.rs:5439). The artifact indents
// the row itself by `11 + depth * 15`px, which needs a depth NUMBER — until
// `PageItem` carries one the prefix stays as the only hierarchy signal, moved
// ahead of the icon so it indents the whole row instead of the title alone.
component PageButton(page:PageItem, selected:bool, frozen:bool)
  emits
    choose_page(str)
  col w=fill
    if selected
      button -> emit(choose_page, page.id)
        with
          disabled=frozen
          label=page.title
          checked=selected
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
          disabled=frozen
          label=page.title
          checked=selected
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

component PageSearchResult(hit:PageSearchHit, frozen:bool)
  emits
    open_page_search_hit(str, str)
  button -> emit(open_page_search_hit, hit.page_id, hit.block_id)
    with
      disabled=frozen
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
        // THE PAGE, THEN THE BLOCK KIND. The left slot named the kind and the
        // right slot printed the raw `block_id` — an opaque `block-1786…` that
        // says nothing to a reader — while the one fact a hit needs, the page
        // it was found in, went unsaid.
        text hit.page_title
          with
            w=fill
            size=10.5
            font=code_medium
            @text-muted
        text hit.kind
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

// ONE THREAD, WHOLE — `border:1px solid #ece9e1;background:#fff;
// border-radius:11px;padding:12px` with a 22px principal plate, the author at
// 600 12px and the body at 12px/1.55 (Liquid Glass:944-953). The opening
// comment sits at the top with the thread's one action; its replies are
// indented under a hairline, the tail folded away past three.
//
// TWO PARTS OF THE ARTIFACT CARD ARE NOT DRAWN, because nothing on the wire
// carries them and a plausible one would be a lie:
//   * the AGENT badge and the square plate — `PageComment`/`PageCommentThread`
//     carry a display name. Account authorship alone does not identify the
//     account's control mode, so this projection uses the person plate.
//   * the relative timestamp. `ThreadRow.created_at` is a block HEIGHT on a
//     validator network and unix millis on a single-writer noded, so it is not
//     a wall clock and "2h ago" off it would be invented. The right-hand slot
//     shows the ordinal the record does have (`#3`, `#3 · edited`).
component PageCommentThreadCard(thread:PageCommentThread, replying:bool, expanded:bool, busy:bool, frozen:bool, bind reply_draft:str)
  emits
    resolve_thread_submit(str, bool)
    select_reply_thread(str)
    toggle_thread_replies(str)
    post_thread_reply(str)
  box
    with
      w=fill
      p=12.0
      bg=surface
      border=card_line
      border-w=1.0
      r=11.0
    col w=fill gap=8.0
      row
        with
          w=fill
          gap=8.0
          align=center
        PersonAvatar
          with
            initials=initials_of(thread.author)
            plate=22.0
            ink=9.0
        text thread.author
          with
            size=12.0
            wrap=none
            font=display
            @text-primary
        text thread.meta
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-label
        space w=fill
        // ONE ACTION PER THREAD, and it is the one that settles it.
        if !thread.resolved
          button "Resolve" -> emit(resolve_thread_submit, thread.id, true)
            with
              label="Resolve thread"
              disabled=(busy || frozen)
              p=4.0
              @secondary_action text-11px leading-snug font-medium
            active bg=transparent text=muted r=6.0
            hovered bg=fg/9 text=fg
            pressed bg=fg/14
        if thread.resolved
          button "Reopen" -> emit(resolve_thread_submit, thread.id, false)
            with
              label="Reopen thread"
              disabled=(busy || frozen)
              p=4.0
              @secondary_action text-11px leading-snug font-medium
            active bg=transparent text=muted r=6.0
            hovered bg=fg/9 text=fg
            pressed bg=fg/14
      text opener_text(thread)
        with
          w=fill
          size=12.0
          line-h=1.55
          wrap=word
          @text-accent_fg
      // THE REPLIES, indented behind a hairline so the thread reads as one
      // conversation rather than a stack of equal cards.
      if !empty(thread_replies(thread, expanded))
        row
          with
            w=fill
            gap=12.0
            pl=17.0
          box
            with
              w=1.0
              h=fill
              bg=separator
            space w=1.0 h=1.0
          col w=fill gap=8.0
            for reply in thread_replies(thread, expanded)
              col w=fill gap=5.0
                row
                  with
                    w=fill
                    gap=8.0
                    align=center
                  PersonAvatar
                    with
                      initials=initials_of(reply.author)
                      plate=22.0
                      ink=9.0
                  text reply.author
                    with
                      w=fill
                      size=12.0
                      wrap=none
                      font=display
                      @text-primary
                  text reply.meta
                    with
                      size=10.5
                      wrap=none
                      font=code_medium
                      @text-label
                text reply.text
                  with
                    w=fill
                    size=12.0
                    line-h=1.55
                    wrap=word
                    @text-accent_fg
      if !empty(reply_toggle_label(thread, expanded))
        button -> emit(toggle_thread_replies, thread.id)
          with
            label="Show every reply"
            expanded=expanded
            disabled=frozen
            p=4.0
            @secondary_action
          text reply_toggle_label(thread, expanded)
            with
              size=11.0
              wrap=none
              font=medium
              @text-muted
          active bg=transparent text=muted r=6.0
          hovered bg=fg/9 text=fg
          pressed bg=fg/14
      // THE COMPOSE TARGET IS ALWAYS VISIBLE: a resting "Reply…" line that
      // becomes the field when pressed. One draft at a time, so only the
      // picked thread carries a live box.
      if !thread.resolved && !replying
        button "Reply…" #reply-on(thread.id) -> emit(select_reply_thread, thread.id)
          with
            label="Reply to this thread"
            disabled=(busy || frozen)
            w=fill
            p=6.2
            @ghost_action text-13px leading-snug
          active bg=transparent text=muted border=fg/8 border-w=1.0 r=7.0
          hovered bg=fg/4 text=fg border=fg/11
          pressed bg=fg/8 text=fg border=fg/11
      if !thread.resolved && replying
        row
          with
            w=fill
            gap=5.0
            align=center
          input "" #thread-reply(thread.id) <-> reply_draft
            with
              label="Reply"
              hint="Reply…"
              disabled=(busy || frozen)
              submit=emit(post_thread_reply, thread.id)
              w=fill
              p=6.2
              text-size=13.0
              line-h=1.2
              @control
            active bg=transparent border=fg/8 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
            hovered bg=fg/4 border=fg/11
            focused bg=fg/4 border=ring
            disabled value=muted
          button "Post" -> emit(post_thread_reply, thread.id)
            with
              label="Post reply"
              disabled=(busy || frozen || empty(trim(reply_draft)))
              p=5.0
              @primary_action
