// A sidebar channel row: 7px radius, 8px side inset, the hash in `label` ink
// and the name in `list` weight. Selected is the same tint capsule the rail uses.
component ChannelButton(channel:ChatChannel, selected:bool, unread:bool, disabled:bool)
  emits
    choose_channel(str)
  box
    with
      w=fill
      pl=8.0
      pr=8.0
    col w=fill
      if selected
        button -> emit(choose_channel, channel.id)
          with
            label=channel.name
            disabled=disabled
            checked=selected
            w=fill
            p=0.0
            @icon_action
          // THE PRESSED PLATE ON AN UNSELECTED ROW SCORES 0.00 FROM
          // `selected_row` BY THE REPO'S OWN METRIC — no grey the palette
          // owns is both darker than the hover step and off `selected_row`
          // by a visible margin, so a grey mark alone cannot survive a
          // press. A 2.5px brand bar is the mark instead: `brand` stands
          // 68.00/82.33 from every row plate, which no press state can
          // imitate. The content box loses 2.5px of its own `pl` so the
          // TEXT lands at the same 16px the unselected row's does.
          row gap=0.0
            box
              with
                w=2.5
                h=fill
                bg=brand
                r=1.25
              space w=1.0 h=1.0
            box
              with
                w=fill
                pl=5.5
                pr=8.0
                pt=7.0
                pb=7.0
              row
                with
                  w=fill
                  gap=7.0
                  align=center
                if channel.members_only
                  text "◆"
                    with
                      size=12.0
                      wrap=none
                      @text-label
                if !channel.members_only
                  text "#"
                    with
                      size=13.0
                      wrap=none
                      @text-label
                box w=fill clip=true
                  text channel.name
                    with
                      size=13.0
                      wrap=none
                      font=medium
                      @text-fg
                if channel.archived
                  text "archived"
                    with
                      size=9.0
                      wrap=none
                      font=code_semibold
                      @text-meta
                if channel.huddle_count > 0
                  Icon
                    with
                      name="headphones"
                      tone="success"
                      px=12.0
          active bg=selected_row text=fg border=transparent border-w=1.0 r=7.0
          hovered bg=selected_row text=fg
          pressed bg=rail_hover text=fg
      if !selected
        button -> emit(choose_channel, channel.id)
          with
            label=channel.name
            disabled=disabled
            checked=selected
            w=fill
            p=0.0
            @icon_action
          box
            with
              w=fill
              pl=8.0
              pr=8.0
              pt=7.0
              pb=7.0
            row
              with
                w=fill
                gap=7.0
                align=center
              if channel.members_only
                text "◆"
                  with
                    size=12.0
                    wrap=none
                    @text-label
              if !channel.members_only
                text "#"
                  with
                    size=13.0
                    wrap=none
                    @text-label
              if unread
                box w=fill clip=true
                  text channel.name
                    with
                      size=13.0
                      wrap=none
                      font=display
                      @text-fg
              if !unread
                box w=fill clip=true
                  text channel.name
                    with
                      size=13.0
                      wrap=none
                      font=medium
                      @text-muted
              if channel.archived
                text "archived"
                  with
                    size=9.0
                    wrap=none
                    font=code_semibold
                    @text-meta
              if channel.huddle_count > 0
                Icon
                  with
                    name="headphones"
                    tone="success"
                    px=12.0
              if unread
                box
                  with
                    w=7.0
                    h=7.0
                    bg=brand
                    r=3.5
                  space w=1.0 h=1.0
          active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
          hovered bg=rail_hover text=fg
          pressed bg=subtle text=fg

// One roster row of the details drawer: the key label in machine ink and a
// quiet ×; the danger reading arrives on hover, not as a red button per row.
component ChatMemberRow(member:ChatMember, disabled:bool)
  emits
    remove_channel_member_submit(str)
  row
    with
      w=fill
      gap=6.0
      align=center
    text member.label
      with
        w=fill
        size=11.5
        wrap=none
        font=code
        @text-accent_fg
    button -> emit(remove_channel_member_submit, member.key)
      with
        label="Remove member"
        description=member.label
        disabled=disabled
        w=24.0
        h=24.0
        p=0.0
        @icon_action
      box
        with
          w=fill
          h=fill
          align-x=center
          align-y=center
        // NO `@text-*` ON THE GLYPH. A class here emits an explicit color, and
        // an explicit color ignores the button's own `text=` for every status —
        // so the plate went red on hover while the × stayed muted, and the
        // disabled ramp never reached it either. Uncolored, the glyph inherits
        // the four lines below.
        text "×"
          with
            size=13.0
            wrap=none
      active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
      hovered bg=danger_bg text=fg
      pressed bg=danger_line text=fg

// ONE PARAGRAPH, NOT A FLEX OF TOKENS (ducktape-ui#639 collected by #1096).
// The whole span list lowers into a single native rich-text widget: real word
// wrapping (`word-or-glyph` still, so an unbroken hash or invite one run wide
// breaks instead of clipping), native selection across the line, and `link=`
// handled by the widget's own route instead of a per-token button.
//
// THE ARM CHOICE IS DATA. A rich-text `for` expands a fixed span template per
// item with no conditionals, so the module files every run under exactly one
// `ChatSpan` text field (`span_arm` in chat's client) and this template emits
// every arm for every run — an empty span draws no glyphs, so the five silent
// arms of each run cost nothing.
//
// A MENTION IS A TOKEN, NOT JUST A TINT: the `brand_bg` plate (68.00+/255
// from every row plate it can sit on) makes it an entity you can find at a
// glance, and ONLY a mention wears it — a posted URL is a destination, not a
// person. A LINK wears the underline instead, the one convention every reader
// already knows, and ONLY the link arm draws that rule: it marks a
// destination, not an emphasis. Both keep brand ink; the plate versus the
// rule is what tells a human from a destination.
component RichLine(block:ChatBlock)
  emits
    open_message_link(str)
  rich-text w=fill size=13.5 line-h=1.55 wrap=word-or-glyph color=accent_fg -> emit(open_message_link, _)
    for span in block.spans
      span span.mention bg=brand_bg px=4.0 r=4.0 font=medium color=brand
      span span.link_text link=span.link underline font=medium color=brand
      span span.bold_italic font=strongitalic
      span span.bold font=strong
      span span.italic font=italic
      span span.plain

// max-w=760.0 STOPS THE LINE, not the pane. Nothing here bounded a line's
// measure at all — the default window ran ~130 characters a line and a
// maximized one ~320, both well past the 45–75 characters a reader's eye can
// find its way back across reliably. 760px at 13.5px Geist is ~110 chars:
// generous, and it leaves the narrow rail/min-window case (≤300px) untouched.
// Everything that hangs off this column — the reaction row, the reply pill —
// inherits the cap through `w=fill`, which is correct: neither belongs 2000px
// right of the text it is about.
component MessageBody(message:ChatMessage)
  emits
    open_message_link(str)
  col w=fill max-w=760.0 gap=5.0
    for block in message.blocks
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
              RichLine block=block
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
                  size=13.5
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
          RichLine block=block
            forward
              open_message_link
        if !block.rich
          text block.text
            with
              w=fill
              size=13.5
              line-h=1.55
              wrap=word-or-glyph
              @text-accent_fg

// People are round, agents are 8px-rounded squares. The artifact never mixes
// the two — the shape IS the authorship signal.
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

// ONE reaction chip: FILLED when I am in this reaction, a quiet outlined slab
// when I am not. The count rides at the emoji's size, not one step above it.
//
// A chip is nested INSIDE a message row, so no WASH can be its mark: a wash it
// rests on is a wash some row can also wear, and this palette holds none that
// stands off both the stream's `bg` and a row's `selected_row`. Nearer of the
// two, mean per channel, light/dark: `brand_wash` 4.33/9.33, `brand_bg`
// 7.67/9.00, `muted_bg` 8.00/10.33, `warning_plate` 11.33/12.00 — and that
// last is the warning plate. So the mark is INK on the arm that has one to
// make, and an EDGE on the arm that does not. The tinted pair this chip wore
// before proved the point: mine sat on `brand_bg`, which was the selected
// message card's own plate at the time, 0.00 apart in both themes.
component ReactionChip(reaction:ChatReaction, seq:i64)
  emits
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
  col #root
    if reaction.reacted_by_me
      button -> emit(remove_reaction_at, seq, reaction.emoji)
        with
          label="Remove reaction"
          description=reaction.emoji
          checked=reaction.reacted_by_me
          p=0.0
          @icon_action
        box
          with
            pl=6.0
            pr=8.0
            pt=1.0
            pb=1.0
          row gap=4.0 align=center
            text reaction.emoji
              with
                size=11.0
                wrap=none
                font=code_medium
                @text-brand_fg
            text reaction.count
              with
                size=11.0
                wrap=none
                font=code_medium
                @text-brand_fg
        // A chip you are IN INVERTS TO INK, exactly as the filter chip inverts
        // to `primary`. Ink cannot collide with a row plate, whichever plate a
        // row is later retuned to: `brand` stands 68.00/82.33 from the nearest
        // of them, where the `brand_bg` it replaces stood 0.00 from the
        // selected card's own plate. Hover halos rather than deflating,
        // because deflating to a wash is what put this chip inside its row's
        // plate to begin with.
        //
        // NO state moves the plate, on either arm: a chip nested in a row
        // cannot flash a wash without landing on some row's own plate, so it
        // flashes its RING instead — `brand` at rest, the `brand_line` halo
        // under the cursor (109.00/88.33 from the fill), the full `brand_fg`
        // ring under a finger (151.67/121.00). The glyphs are `brand_fg`
        // whatever happens, which a plate that moved could not promise.
        active bg=brand text=brand_fg border=brand border-w=1.0 r=11.0
        hovered bg=brand text=brand_fg border=brand_line
        pressed bg=brand text=brand_fg border=brand_fg
    if !reaction.reacted_by_me
      button -> emit(add_reaction_at, seq, reaction.emoji)
        with
          label="Add reaction"
          description=reaction.emoji
          checked=reaction.reacted_by_me
          p=0.0
          @icon_action
        box
          with
            pl=6.0
            pr=8.0
            pt=1.0
            pb=1.0
          row gap=4.0 align=center
            text reaction.emoji
              with
                size=11.0
                wrap=none
                font=code_medium
                @text-fg
            text reaction.count
              with
                size=11.0
                wrap=none
                font=code_medium
                @text-muted
        // A chip you are NOT in is a quiet SLAB, and what draws it is its
        // EDGE, not its plate: `muted_bg` sits 8.00/10.33 off the stream's
        // `bg` and 10.67/13.00 off `selected_row`, faint on every row and no
        // fainter on a selected one. The edge is where it actually broke —
        // `card_line` measured 21.00/20.33 against `bg` but 2.33/3.00 against
        // `selected_row`, so on the row you were reading the outline was gone.
        // `control_line`, the token NAMED for a control's resting outline,
        // beats it on all four: 31.67/31.00 and 13.00/7.67.
        //
        // It is chosen for the name, not for the number, and it is not the
        // palette's strongest line — over both themes and every plate a row
        // can wear its worst case is 7.67, where `control_line_hover` scores
        // 17.33, `window_line` 12.33, `pending_line` 11.33 and `brand_line`
        // 9.33. Those are the hover step, the window chrome line, the
        // not-yet-settled dash and the brand family; taking one of them for a
        // resting neutral outline would buy contrast by spending a token's
        // meaning, which is the defect the whole `selected_row` line of work
        // exists to undo.
        //
        // Same rule as the arm above: the plate never moves, the RING does.
        // Hover and press used to deflate to `elevated` and `subtle`, 7.67/8.33
        // and 3.33/2.33 from `selected_row` — so pressing this chip on a
        // selected message painted it the row's own colour, which is the defect
        // #1008 existed for, one level down. The ring warms instead:
        // `control_line` at rest, `brand_line` under the cursor, full `brand`
        // under a finger, each a visible step (13.00/8.33, then further) and
        // none of them a wash the row can also wear.
        active bg=muted_bg text=muted border=control_line border-w=1.0 r=11.0
        hovered bg=muted_bg text=fg border=brand_line
        pressed bg=muted_bg text=fg border=brand

component MessageContents(message:ChatMessage)
  emits
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
    open_thread_for(i64)
    open_message_link(str)
  col w=fill
    row
      with
        w=fill
        gap=11.0
        align=start
      if message.show_author
        MessageAvatar initials=message.initial kind=message.avatar_kind
      if !message.show_author
        space w=30.0
      col w=fill gap=2.0
        if message.show_author
          row
            with
              w=fill
              gap=7.0
              align=center
            text message.author
              with
                size=13.5
                wrap=none
                font=display
                @text-fg
            if message.avatar_kind == "agent"
              box
                with
                  px=5.0
                  py=2.0
                  bg=primary
                  r=4.0
                text "AGENT"
                  with
                    size=9.0
                    wrap=none
                    font=code_semibold
                    @text-primary_fg
            // The artifact stamps a wall clock here. This chain has none: the
            // validator writes `consensus_time = height`, so the honest stamp is
            // the block the message landed in. An unsettled row has no height
            // yet and simply carries none.
            // MUTED, NOT `hint`. This is the only timestamp the product has
            // (there is no wall clock — a message stamps the BLOCK it
            // settled in) and `· edited` changes the meaning of the message
            // it annotates; both are meant to be read, and `hint` measures
            // 2.10:1 on `bg` — below the 3:1 floor. `muted` clears AA at
            // 5.37:1; the size and mono face still carry the recede.
            if message.height > 0
              text height_label_short(message.height)
                with
                  size=11.0
                  wrap=none
                  font=code_medium
                  @text-muted
            if message.edited
              text "· edited"
                with
                  size=11.0
                  wrap=none
                  font=code_medium
                  @text-muted
            space w=fill
        MessageBody message=message
          forward
            open_message_link
        // `· edited` ANNOTATES THE MESSAGE, NOT THE RUN. It lived inside the
        // `show_author` header, so in a run of five messages only the first
        // could ever say it had been edited — and runs are most of a busy
        // channel. A message's text changing under its readers with no mark
        // anywhere on the row is the one integrity signal this product has,
        // silently suppressed on the majority of rows. The header keeps it
        // where a header exists; a continuation row trails it under the body
        // it belongs to, in the same 11px mono muted the header stamps it in.
        if message.edited && !message.show_author
          text "· edited"
            with
              size=11.0
              wrap=none
              font=code_medium
              @text-muted
        // Reactions and the replies button STACK — the artifact gives each its
        // own line under the body, never one shared row.
        if !empty(message.reactions)
          flex
            with
              w=fill
              wrap=wrap
              gap-x=5.0
              gap-y=5.0
              items=start
              pt=6.0
            for reaction in message.reactions
              ReactionChip reaction=reaction seq=message.seq
                forward
                  add_reaction_at
                  remove_reaction_at
        if message.reply_count > 0
          row
            with
              w=fill
              gap=6.0
              align=center
              pt=6.0
            button -> emit(open_thread_for, message.seq)
              with
                label="Open thread"
                p=0.0
                @icon_action
              box
                with
                  pl=7.0
                  pr=9.0
                  pt=3.0
                  pb=3.0
                row gap=6.0 align=center
                  Icon
                    with
                      name="nav-chat"
                      tone="accent"
                      px=12.0
                  text plural(message.reply_count, "reply", "replies")
                    with
                      size=11.0
                      wrap=none
                      font=code_medium
                      @text-brand
              // The third slab in this stream drawn by its EDGE rather than its
              // plate — with the un-reacted reaction chip and the code fence —
              // and it takes the same one-token fix. It is the mirror image of
              // those two: `surface` is 21.33/16.33 off `selected_row` but only
              // 2.67/7.00 off `bg`, so on an ordinary row it is drawn entirely by
              // its outline — and that outline was `border`, 5.33/2.00 from
              // `selected_row`, which left it with neither on a selected one.
              //
              // And the same rule on its states as both chip arms: the plate
              // never moves, the ring does. This one still deflated —
              // `muted_bg` under the cursor and `elevated` under a finger, and
              // `elevated` is 7.67/8.33 from `selected_row`, so pressing it on
              // the message you had a menu open on painted it the row's own
              // colour. That is #1008's defect one level down, and fixing it
              // on the chips while leaving it here is how the same class keeps
              // coming back one sibling out.
              active bg=surface text=brand border=control_line border-w=1.0 r=8.0
              hovered bg=surface text=brand border=brand_line
              pressed bg=surface text=brand border=brand
      // The quiet send-state lane at the row's right edge: an in-flight row
      // carries the mint's own "Sending…" stamp and a small dot. Canonical
      // confirmation removes both immediately; no timer, animation, or second
      // collection shadows the row state. The WORD is load-bearing, not the
      // dot: a send folds into the author run above it (no header, no spacer),
      // so a dot alone left an in-flight row pixel-identical to a settled one.
      //
      // The pr=7 inset is load-bearing: it tucks the indicator fully inside
      // the hover toolbar's opaque plate (which ends 8px in from the card
      // edge, rounded r=9), so hovering OCCLUDES the dot instead of leaving a
      // sliver poking past the plate. The indicator is transient decoration;
      // the toolbar wins the corner while the cursor is on the row.
      // FULL OPACITY ON `muted`, not a recede step dimmed further: `hint` at
      // opacity=0.55 computed to ≈1.47:1 on a 7px mark — invisible, not
      // quiet. `muted` (5.37:1) at full strength reads as a quiet dot; a
      // recede step should never also carry an alpha fade.
      if message.pending
        box pr=7.0
          row gap=5.0 align=center
            text message.meta
              with
                size=11.0
                wrap=none
                font=code_medium
                @text-muted
            svg icon("dot") memory
              with
                w=6.0
                h=6.0
                style=icon_tint("muted")
                opacity=1.0
component MessageCard(message:ChatMessage, selected:bool, menu_open:bool)
  emits
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
    open_thread_for(i64)
    open_message_reactions(i64, str, i64)
    open_message_actions(i64, str, i64)
    open_message_link(str)
  col w=fill
    if message.show_author
      space w=1.0 h=14.0
    // DRAW-TIME HOVER: the tint and the toolbar reveal ride the `hover`
    // widget's cursor check — no enter/exit routes, no hovered state, no view
    // rebuild per row crossing. A cached lazy row keeps its toolbar at native
    // latency; that is the whole point (the state-driven version left the
    // highlight trailing the cursor by the queued rebuilds).
    //
    // `open=` is the one thing the cursor does NOT decide. The ♡ and ⋯ buttons
    // open a card anchored on this row; `menu_open` is that card's own
    // openness, handed back so the toolbar dies WITH it. Without it the next
    // mouse move erased the toolbar and left the emoji picker pointing at a
    // button that was no longer there.
    hover tint=row_hover r=9.0 open=menu_open
      stack w=fill
        if message.deleted
          box
            with
              w=fill
              pl=7.0
              pr=7.0
              pt=4.0
              pb=4.0
              bg=transparent
              border=transparent
              border-w=1.0
              r=9.0
            MessageContents message=message
              forward
                add_reaction_at
                remove_reaction_at
                open_thread_for
                open_message_link
        // Selection is a tint, not a ring — see the QA note in the stream. The
        // tint is `selected_row`, the one plate that means "the row you are on":
        // a menu-open or deep-linked message is that row, exactly as a chosen
        // channel or an open file is. It read `brand_bg` until the reaction chip
        // moved onto the same token and vanished on it.
        if !message.deleted && selected
          box
            with
              w=fill
              pl=7.0
              pr=7.0
              pt=4.0
              pb=4.0
              bg=selected_row
              border=transparent
              border-w=1.0
              r=9.0
            MessageContents message=message
              forward
                add_reaction_at
                remove_reaction_at
                open_thread_for
                open_message_link
        if !message.deleted && !selected
          box
            with
              w=fill
              pl=7.0
              pr=7.0
              pt=4.0
              pb=4.0
              bg=transparent
              border=transparent
              border-w=1.0
              r=9.0
            MessageContents message=message
              forward
                add_reaction_at
                remove_reaction_at
                open_thread_for
                open_message_link
      col w=fill
        if !message.deleted && !message.pending
          box
            with
              w=fill
              align-x=end
              align-y=start
              pr=8.0
            // The designer's own opaque answer for this bar: white card, 1px
            // border, r9 over the 3/12 popover shadow.
            box
              with
                p=2.0
                bg=surface
                border=border
                border-w=1.0
                r=9.0
                shadow=shadow_popover
                shadow-y=3.0
                shadow-blur=12.0
              // EVERY GLYPH LABEL IN THIS BAR IS ITS OWN REGULAR-WEIGHT
              // `text`, NOT A BUTTON STRING LABEL. A string label is
              // generated at semibold, and cosmic-text's fallback fast path
              // only considers faces whose weight matches the request
              // exactly. Every face this app bundles registers at 400, so a
              // non-ASCII glyph asked for at 600 misses all of them and walks
              // the whole font database instead — ~2.4ms per label, per row,
              // per fresh layout, while the regular face costs ~25us. Five
              // such labels on a row that a channel switch mounts 46 times is
              // the half-second freeze: rig-measured 570ms -> 35ms.
              row gap=1.0 align=center
                button -> emit(add_reaction_at, message.seq, "👍")
                  with
                    label="React with 👍"
                    w=27.0
                    h=25.0
                    p=4.0
                    @ghost_action
                  text "👍"
                    with
                      size=12.5
                      font=ui
                  active bg=transparent text=muted r=6.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle text=fg
                button -> emit(add_reaction_at, message.seq, "✅")
                  with
                    label="React with ✅"
                    w=27.0
                    h=25.0
                    p=4.0
                    @ghost_action
                  text "✅"
                    with
                      size=12.5
                      font=ui
                  active bg=transparent text=muted r=6.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle text=fg
                button -> emit(add_reaction_at, message.seq, "👀")
                  with
                    label="React with 👀"
                    w=27.0
                    h=25.0
                    p=4.0
                    @ghost_action
                  text "👀"
                    with
                      size=12.5
                      font=ui
                  active bg=transparent text=muted r=6.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle text=fg
                box
                  with
                    w=1.0
                    h=16.0
                    bg=subtle
                  space w=1.0 h=1.0
                button -> emit(open_message_reactions, message.seq, message.body, message.rev)
                  with
                    label="Manage reactions"
                    w=27.0
                    h=25.0
                    p=4.0
                    @ghost_action
                  text "♡"
                    with
                      size=12.5
                      font=ui
                  active bg=transparent text=muted r=6.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle text=fg
                button -> emit(open_thread_for, message.seq)
                  with
                    label="Open thread"
                    p=5.0
                    @icon_action
                  // `color=inherit` (ducktape-ui#606): the glyph reads the
                  // same status-resolved ink the ♡ and ⋯ string labels beside
                  // it inherit, so all three brighten and dim together on
                  // the button's bounds.
                  svg icon("nav-chat") memory color=inherit
                    with
                      w=15.0
                      h=15.0
                  active bg=transparent text=muted r=6.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle text=fg
                button -> emit(open_message_actions, message.seq, message.body, message.rev)
                  with
                    label="More message actions"
                    w=27.0
                    h=25.0
                    p=4.0
                    @ghost_action
                  text "⋯"
                    with
                      size=12.5
                      font=ui
                  active bg=transparent text=muted r=6.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle text=fg
        if message.deleted || message.pending
          space w=1.0 h=1.0

// A REPLY IS THE SAME MESSAGE BLOCK AS A TIMELINE ROW. It mounts
// `MessageContents` — the run rhythm, the quiet code slab and quote bar, the
// reaction chips and the right-edge send-state lane — instead of a second
// spelling of them; the rail's own body component drifted a whole redesign
// behind and is deleted. The rail's narrowness is its 330px plate, not a
// smaller type scale.
//
// What genuinely diverges is the CARD CHROME: the toolbar is thread-scoped
// (`open_thread_message_*`) and carries no open-thread button, because you are
// already reading the thread. The card padding matches MessageCard's so the
// pr=7 indicator inset still tucks under the pr=8 toolbar plate (#926).
//
// `open_thread_for` is forwarded only because `MessageContents` declares it:
// it fires from the reply pill, and a reply carries no replies
// (`reply_count` only ever climbs on a root), so the pill never renders here.
component ThreadMessageCard(message:ChatMessage, selected:bool, menu_open:bool)
  emits
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
    open_thread_for(i64)
    open_thread_message_actions(i64, str, i64)
    open_thread_message_reactions(i64, str, i64)
    open_message_link(str)
  col w=fill
    if message.show_author
      space w=1.0 h=14.0
    // Same draw-time hover as MessageCard — see the note there. `menu_open` is
    // a prop of its own and not `selected` because in the rail `selected` means
    // the deep-link TARGET reply, which is not the row whose menu is up.
    hover tint=row_hover r=9.0 open=menu_open
      stack w=fill
        if message.deleted
          box
            with
              w=fill
              pl=7.0
              pr=7.0
              pt=4.0
              pb=4.0
              bg=transparent
              border=transparent
              border-w=1.0
              r=9.0
            MessageContents message=message
              forward
                add_reaction_at
                remove_reaction_at
                open_thread_for
                open_message_link
        // Same `selected_row` tint as the stream — see the note in MessageCard.
        if !message.deleted && selected
          box
            with
              w=fill
              pl=7.0
              pr=7.0
              pt=4.0
              pb=4.0
              bg=selected_row
              border=transparent
              border-w=1.0
              r=9.0
            MessageContents message=message
              forward
                add_reaction_at
                remove_reaction_at
                open_thread_for
                open_message_link
        if !message.deleted && !selected
          box
            with
              w=fill
              pl=7.0
              pr=7.0
              pt=4.0
              pb=4.0
              bg=transparent
              border=transparent
              border-w=1.0
              r=9.0
            MessageContents message=message
              forward
                add_reaction_at
                remove_reaction_at
                open_thread_for
                open_message_link
      col w=fill
        if !message.deleted && !message.pending
          box
            with
              w=fill
              align-x=end
              align-y=start
              pr=8.0
            // The stream's bar, minus its open-thread seat AND its one-tap
            // reactions: the rail is a fixed 330px plate, and the full 145px
            // bar would still cover half the author header. Two seats (61px)
            // keep it clear; the one-tap set lives one click away behind ♡.
            box
              with
                p=2.0
                bg=surface
                border=border
                border-w=1.0
                r=9.0
                shadow=shadow_popover
                shadow-y=3.0
                shadow-blur=12.0
              row gap=1.0 align=center
                button -> emit(open_thread_message_reactions, message.seq, message.body, message.rev)
                  with
                    label="Manage reactions"
                    w=27.0
                    h=25.0
                    p=4.0
                    @ghost_action
                  text "♡"
                    with
                      size=12.5
                      font=ui
                  active bg=transparent text=muted r=6.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle text=fg
                button -> emit(open_thread_message_actions, message.seq, message.body, message.rev)
                  with
                    label="More message actions"
                    w=27.0
                    h=25.0
                    p=4.0
                    @ghost_action
                  text "⋯"
                    with
                      size=12.5
                      font=ui
                  active bg=transparent text=muted r=6.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle text=fg
        if message.deleted || message.pending
          space w=1.0 h=1.0

component ChatSearchResult(hit:ChatSearchHit)
  emits
    open_chat_search_hit(str, i64, i64)
  button -> emit(open_chat_search_hit, hit.channel_id, hit.root_seq, hit.seq)
    with
      label=hit.text
      w=fill
      p=8.0
      @ghost_action
    col w=fill gap=3.0
      row
        with
          w=fill
          gap=7.0
          align=center
        text hit.author
          with
            w=fill
            size=13.0
            font=medium
            @text-fg
        text hit.meta
          with
            size=11.0
            font=code_medium
            @text-muted
      text hit.text
        with
          w=fill
          size=13.5
          wrap=word-or-glyph
          @text-fg
    active bg=transparent text=fg border=transparent border-w=1.0 r=9.0
    hovered bg=fg/6 text=fg border=fg/9
    pressed bg=fg/10 text=fg border=fg/13

// THE FORMAT SEAT — one row, mounted by BOTH chat composers. The marks are the
// renderer's own grammar, so a button inserts exactly what typing the fence
// would; the same table the Cmd/Ctrl chords carry.
//
// It is a component because the thread rail had NO toolbar: a reply could be
// formatted only by someone who already knew the chord, and a second authored
// copy of the row is a second thing to drift.
//
// The event is `mark`, not `composer_mark`: the two mounts route it to
// DIFFERENT editors through an `events` block, which a same-name `forward`
// cannot express.
//
// The root is a bare `row` at INTRINSIC width (~115px) — no `w=fill`. The
// caller owns the seat's `space w=fill` and its Send, so the 330px rail's
// `clip=true` has nothing to slice.
component ComposerMarks(disabled:bool)
  emits
    mark(str)
  row #root
    with
      gap=2.0
      align=center
    button -> emit(mark, "bold")
      with
        label="Bold"
        disabled=disabled
        w=26.0
        h=24.0
        p=0.0
        @ghost_action
      box
        with
          w=fill
          h=fill
          align-x=center
          align-y=center
        text "B"
          with
            size=12.5
            wrap=none
            font=strong
      active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
      hovered bg=fg/8 text=fg
      pressed bg=fg/12 text=fg
    button -> emit(mark, "italic")
      with
        label="Italic"
        disabled=disabled
        w=26.0
        h=24.0
        p=0.0
        @ghost_action
      box
        with
          w=fill
          h=fill
          align-x=center
          align-y=center
        text "I"
          with
            size=12.5
            wrap=none
            font=italic
      active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
      hovered bg=fg/8 text=fg
      pressed bg=fg/12 text=fg
    box
      with
        w=1.0
        h=14.0
        bg=separator
      space w=1.0 h=1.0
    button #code -> emit(mark, "code")
      with
        label="Code block"
        disabled=disabled
        w=26.0
        h=24.0
        p=0.0
        @ghost_action
      box
        with
          w=fill
          h=fill
          align-x=center
          align-y=center
        svg icon("code-brackets") #glyph memory color=inherit
          with
            w=13.0
            h=13.0
      active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
      hovered bg=fg/8 text=fg
      pressed bg=fg/12 text=fg
    button -> emit(mark, "quote")
      with
        label="Quote"
        disabled=disabled
        w=26.0
        h=24.0
        p=0.0
        @ghost_action
      box
        with
          w=fill
          h=fill
          align-x=center
          align-y=center
        svg icon("quote") memory color=inherit
          with
            w=13.0
            h=13.0
      active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
      hovered bg=fg/8 text=fg
      pressed bg=fg/12 text=fg

// THE REFUSED COMPOSER. `post_gate` names the refusal; this turns the token
// into the sentence, and every sentence names the move that clears it. An
// empty reason renders nothing at all — the composer itself is the else arm.
//
// MOUNTED above the composer plate (view.ice), as the VIEW half of frozen
// contract item `post_gate(archived, members_only, members, me) -> String`. The
// composer used to grey the editor on `active_channel_archived` alone and say
// NOTHING at all when a members-only channel refused the viewer's key.
//
// `post_gate` is CALLED at the mount, not mirrored into a state field: it is
// pure over facts the view already holds, and `channel_members` lands in seven
// handlers — a mirrored field would be seven assignments and six chances to
// drift. Its reason is now the whole gate, so `active_channel_archived` no
// longer appears in the editor's or Send's `disabled=`: the archived case IS
// the `channel_archived` arm, and one discriminant beats two.
//
// `me` is `settings_user_key` — full-hex, from `SettingsFacts`, which is what a
// `ChatMember.key` carries. There was no such fact when this component landed:
// `account_id` is a short_label of the identity module's ACCOUNT id and cannot
// be compared with a membership row, so the key was added to SettingsFacts
// rather than guessed at from the account card.
component ComposerGate(reason:str)
  col #root w=fill
    match reason
      "channel_archived"
        GateNote
          with
            reason="This channel is archived — new messages are refused."
            next="Unarchive it from Channel details to post here again."
      "members_only"
        GateNote
          with
            reason="This channel is members-only and your key is not on its roster."
            next="Ask a member to add your key from Channel details."
      _
        space w=1.0 h=1.0

// ============================================================================
// THE COMPOSER IS THE ROOM'S COMPOSER (ducktape-ui#697). One retained
// instance per key — the stream mounts `#composer(active_channel)`, the rail
// `#reply_composer(...)` — so a draft never rides a room switch: there is no
// app-level editor left to park, and the park/restore handler class (with
// the ordering lint that policed it) is deleted, not moved.
//
// WHAT RIDES UP: `submitted(text, operation_id)`, fired from the local
// handler AFTER the box is cleared, so the app's optimistic insert and the
// send lane run against a composer that has finished writing. WHAT RIDES
// DOWN: props. The widget's gate rides the ROUTE (`blocked`), so the handler
// refuses with the same verdict the frame drew — and the app re-reads its
// own gate at delivery, where a frame-stale admit stashes the text for the
// banner below instead of losing it.
//
// THE FAILED-SEND BANNER LIVES HERE because its Restore writes THIS
// instance's content — the one thing the app can no longer reach. Formatting
// chords land here too: the widget's `on_chord` claims Cmd/Ctrl+B and
// friends at the caret (see `composer_chord`), so the app's keyboard
// subscription stopped needing to know which composer is focused.
// ============================================================================
component ChatComposer(kind:ComposerKind, compact:bool, hint:str, blocked:bool, restore_blocked:bool, failed_note:str)
  emits
    submitted(ComposerKind, str, str)
  state
    body:editor = ""
    // THE UNSENT WORDS BELONG TO THE ROOM THEY WERE WRITTEN IN. This was one
    // app field, so a send that failed in #private-ops raised its plate over
    // whatever room the reader had moved to and offered to restore it THERE.
    // It is the instance's own state now, delivered by a slice keyed to the
    // room the failure names (ducktape-ui#698).
    failed = ""
  on composer_event(event, gate, which)
    body = apply_composer_event(body, event)
    return if !composer_submits(event)
    return if gate
    let text = trim(editor_text(body))
    return if empty(text)
    body = editor("")
    emit(submitted, which, text, fresh_operation_id(composer_op_prefix(which)))
  on mark(glyph, gate)
    return if gate
    body = composer_toggle_mark(body, glyph)
  // The app's share-out: what this room's send let go of, handed back to the
  // box it came from. A committed body is not unsent, so it never stashes.
  on unsent(text, committed)
    failed = remember_failed_draft(failed, "stash", text, committed)
  on restore(gate)
    return if gate || empty(failed) || !empty(trim(editor_text(body)))
    body = editor(failed)
    failed = ""
  on dismiss
    failed = ""
  col #root w=fill gap=10.0
    // DATA LOSS READS AT LEAST AS LOUD AS AN EXPECTED REFUSAL — the same
    // reversible-danger plate the archive button wears: the draft is
    // recoverable, so this is not `@danger_action` loud.
    if !empty(failed)
      box
        with
          w=fill
          px=13.0
          py=11.0
          bg=danger_zone_bg
          border=danger_zone_line
          border-w=1.0
          r=9.0
        row
          with
            w=fill
            gap=8.0
            align=center
          box
            with
              w=6.0
              h=6.0
              bg=danger_dot
              r=3.0
            space w=1.0 h=1.0
          text failed_note
            with
              w=fill
              size=12.5
              line-h=1.45
              @text-danger
          button "Restore" -> restore(restore_blocked)
            with
              disabled=(restore_blocked || !empty(trim(editor_text(body))))
              h=28.0
              p=5.0
              @secondary_action
            active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
            hovered bg=fg/14
            pressed bg=fg/18
          button -> dismiss
            with
              label="Dismiss unsent draft"
              w=28.0
              h=28.0
              p=0.0
              @icon_action
            box
              with
                w=fill
                h=fill
                align-x=center
                align-y=center
              text "×" size=14.0
            active bg=transparent text=muted r=7.0
            hovered bg=fg/10 text=fg
            pressed bg=fg/15
    box
      with
        w=fill
        bg=surface
        border=control_line
        border-w=1.0
        r=12.0
        clip=true
        shadow=shadow_popover
        shadow-y=1.0
        shadow-blur=2.0
      col w=fill
        extern rich_composer(body, hint, blocked, 44.0, 150.0, 10.0) #field -> composer_event(_, blocked, kind)
        box
          with
            w=fill
            pl=8.0
            pr=8.0
            pb=8.0
          row
            with
              w=fill
              gap=2.0
              align=center
            ComposerMarks
              with
                disabled=blocked
              events
                mark -> mark(_, blocked)
            space w=fill
            if !compact
              text "↵ send · ⇧↵ newline"
                with
                  size=10.5
                  wrap=none
                  font=code_medium
                  @text-muted
              space w=8.0
              button "Send" -> composer_event(composer_submit_event(), blocked, kind)
                with
                  disabled=(blocked || empty(trim(editor_text(body))))
                  h=29.0
                  @primary_action
                  @px-12px
                  @py-7px
            if compact
              button "Send" -> composer_event(composer_submit_event(), blocked, kind)
                with
                  label="Send reply"
                  disabled=(blocked || empty(trim(editor_text(body))))
                  h=28.0
                  @primary_action
                  @px-11px
                  @py-6px

// ============================================================================
// THREAD RAIL — the artifact opens the rail body with the root message in its
// own divided block, at a type scale one notch under a reply.
//
// The rail's 50px HEADER BAR is not a component: it is drawn inline by
// screens/chat.ice — a pane header is a screen-only shape, never a second
// header inside a component file. The artifact's header carries no reply
// count and no "Thread result" title; the screen keeps "Thread result"
// deliberately, because it is the only signpost a chat-search hit gets, and
// trades the count for the honest replies rule below.
// ============================================================================

// THE TRADE, TAKEN DELIBERATELY: the artifact's parent block is READ-ONLY, so
// the root loses the hover bar, reactions and edit/delete that ThreadMessageCard
// gave it in the rail. It does NOT lose them from the product — the rail is a
// pane BESIDE the stream, never over it, so the same message is on screen in
// the stream at the same moment with its full MessageCard toolbar. The cost
// is one extra glance, not a capability.
//
// THE `N replies` RULE is honest now, which is why it lives here and not in
// the header: the count is the ROOT's own `reply_count` — the field the
// stream's reply pill already trusts — not `len(thread_messages)` (root
// included) and not the loaded page (short whenever `thread_has_more` holds).
component ThreadParentBlock(message:ChatMessage)
  emits
    open_message_link(str)
  col #root w=fill
    row
      with
        w=fill
        gap=11.0
        align=start
        pb=14.0
      // GUTTER MATCHED TO THE REPLIES BELOW IT: `plate=30.0 gap=11.0` is the
      // same 41px text gutter `MessageContents` gives every reply, so the
      // root's body lines up with the column it heads instead of sitting 3px
      // left of it — the one thing off-grid at the exact spot the eye enters
      // the rail.
      PrincipalAvatar
        with
          initials=message.initial
          is_agent=(message.avatar_kind == "agent")
          plate=30.0
          ink=11.0
          ring=""
      col w=fill gap=2.0
        row
          with
            w=fill
            gap=6.0
            align=center
          text message.author
            with
              size=13.5
              wrap=none
              font=display
              @text-fg
          // Same fact, same stamp as the stream's — see the note there.
          if message.height > 0
            text height_label_short(message.height)
              with
                size=11.0
                wrap=none
                font=code_medium
                @text-muted
          // The root of a thread is edited like any other message, and this
          // block never said so at all.
          if message.edited
            text "· edited"
              with
                size=11.0
                wrap=none
                font=code_medium
                @text-muted
          space w=fill
        MessageBody message=message
          forward
            open_message_link
    box
      with
        w=fill
        h=1.0
        bg=separator
      space w=1.0 h=1.0
    if message.reply_count > 0
      row
        with
          w=fill
          gap=4.0
          align=center
          pt=13.0
          pb=4.0
        text plural(message.reply_count, "reply", "replies")
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-label

// The EVENT INSPECTOR was built here and mounted nowhere, so it is deleted
// rather than left as a panel no user can reach. What it needed was a lookup
// from `inspector_seq` back to a `ChatMessage`, which no backend fn provides —
// inventing one during an integration pass is how a surface ends up drawing a
// record nobody fetched. `OwnMessageRow` went with it: its finality chip was
// the panel's only remaining route and the row itself was never mounted either.
// The stale `MY OWN POSTS` banner that survived that deletion is gone too — it
// described a right-aligned own-message bubble no arm of MessageCard draws.

// `MessageMenuItem` (a 14px icon + its label) was built here for the message ⋯
// menu and is deleted. The menu it was shaped for is the ARTIFACT's — Inspect
// event / Reply in thread / Copy link / Copy proof / Pin to channel — and this
// app can honestly serve exactly one of those five: there is no inspector rail
// (see the empty shield seat in MessageCard), no `ducktape://` link builder, no
// op hash to copy as a proof, and no pin. A shared row component earns its keep
// by repetition, and the repetition here was four surfaces that do not exist.
// The menu the app actually ships is React / Edit / Delete / Close, inline in
// view.ice:659, and its real gap is that those four rows carry no icon. That is
// one `Icon name=… tone="muted" px=14.0` added to four buttons in view.ice, in
// the file that owns them. The artifact's row metrics for that edit: `gap=9.0`,
// label 12.5 `@text-accent_fg`, `p=8.0`/`pl=9.0`, `r=7.0`, on a 184px plate.

// ONE GHOST ROW, on the real message row's own geometry: a 30px avatar plate,
// an 11px gap, a name bar and a body bar. Mounted three-up while a room loads
// and once inside the search float while a query is out, so the layout does
// not jump when the real rows land on top of it. `subtle` is the palette's own
// track grey, quiet in both themes. No spinner: `spin` is declared and never
// driven (`overlay.ice`), so a ring here would paint a frozen arc.
component SkeletonRow()
  row w=fill gap=11.0 align=start
    box w=30.0 h=30.0 bg=subtle r=15.0
      space w=1.0 h=1.0
    col w=fill gap=6.0 pt=4.0
      box w=96.0 h=9.0 bg=subtle r=4.0
        space w=1.0 h=1.0
      box w=fill max-w=420.0 h=9.0 bg=subtle r=4.0
        space w=1.0 h=1.0
