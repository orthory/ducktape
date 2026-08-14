// THE WINDOW OVERLAYS — the create-channel modal, the toast host and the
// command palette. They share one slot because `WorkspaceTabs` gives the layer
// a single child; they share nothing else.
//
// `draft` and `query` are `bind` props: the inputs write straight back to the
// app state the view passes in, so the palette needs no local mirror of a
// field the handlers already own.
component OverlayLayer(create_open:bool, members_only:bool, bind draft:str, busy:bool, connected:bool, loading:bool, toast:str, tone:str, open:bool, bind query:str, search_phase:SearchPhase, chat_hits:[ChatSearchHit], page_hits:[PageSearchHit])
  emits
    toggle_channel_create()
    toggle_channel_create_members_only()
    create_channel_submit()
    dismiss_toast()
    close_palette()
    palette_changed(str)
    open_chat_search_hit(str, i64, i64)
    open_page_search_hit(str, str)
  stack w=fill h=fill
    // THE CHANNEL MODAL. The artifact picks VISIBILITY here; the chat
    // module has no read-visibility concept at all — `CreateChannel`
    // carries a `PostPolicy` of Open or MembersOnly and nothing else — so
    // the segment picks the POSTING policy and says so, rather than
    // promising a privacy the wire cannot keep.
    // A SCRIM IS NOT A MODAL. A `box bg=scrim` tints the console and
    // captures nothing: the rail, the channel list and the composer
    // behind it all stayed live, and clicking the dim did nothing. The
    // `overlay` widget is the only thing here that takes the pointer
    // and closes on the backdrop.
    overlay
      with
        when=create_open
        dismiss=emit(toggle_channel_create)
        backdrop=scrim
        p=30.0
        align-x=center
        align-y=center
      content
        space w=fill h=fill
      layer
        ModalShell title="Create a channel" width=418.0 #channel-modal
          close:
            button -> emit(toggle_channel_create)
              with
                label="Close"
                disabled=(busy)
                w=26.0
                h=26.0
                p=0.0
                @icon_action
              box
                with
                  w=fill
                  h=fill
                  align-x=center
                  align-y=center
                text "×"
                  with
                    size=14.0
                    wrap=none
                    @text-muted
              active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
              hovered bg=elevated text=fg
              pressed bg=subtle text=fg
          body:
            col w=fill gap=13.0
              text "The channel is created immediately — there is no proposal and no approval step."
                with
                  w=fill
                  size=12.0
                  line-h=1.5
                  @text-caption
              col w=fill gap=6.0
                Eyebrow label="CHANNEL NAME" note=""
                box
                  with
                    w=fill
                    pl=11.0
                    pr=11.0
                    pt=2.0
                    pb=2.0
                    bg=surface
                    border=primary
                    border-w=1.5
                    r=9.0
                  row
                    with
                      w=fill
                      gap=7.0
                      align=center
                    text "#"
                      with
                        size=14.0
                        wrap=none
                        font=code_medium
                        @text-label
                    input "" #new-channel <-> draft
                      with
                        label="New channel name"
                        hint="design-review"
                        disabled=(loading || busy || !connected)
                        submit=emit(create_channel_submit)
                        w=fill
                        p=6.2
                        text-size=13.0
                        line-h=1.2
                        font=code
                        @control
                      active bg=transparent border=transparent value=fg placeholder=hint selection=fg/18 border-w=0.0 r=0.0
                      hovered bg=transparent border=transparent
                      disabled value=muted
              col w=fill gap=6.0
                Eyebrow label="POSTING" note=""
                // Two-line cards: the single-line "Members only · added
                // members post" was wider than its half of the 418px modal
                // and punched through the button's right edge.
                row
                  with
                    w=fill
                    gap=8.0
                    align=center
                  if !members_only
                    box
                      with
                        w=fill
                        pl=12.0
                        pr=12.0
                        pt=8.0
                        pb=8.0
                        bg=muted_bg
                        border=primary
                        border-w=1.5
                        r=9.0
                      col w=fill gap=2.0
                        text "Open"
                          with
                            size=12.5
                            wrap=none
                            font=medium
                            @text-accent_fg
                        text "any member posts"
                          with
                            size=11.0
                            wrap=none
                            @text-caption
                  if members_only
                    button -> emit(toggle_channel_create_members_only)
                      with
                        label="Open posting"
                        w=fill
                        p=0.0
                        @ghost_action
                      box
                        with
                          w=fill
                          pl=12.0
                          pr=12.0
                          pt=8.0
                          pb=8.0
                        col w=fill gap=2.0
                          text "Open"
                            with
                              size=12.5
                              wrap=none
                              font=medium
                              @text-accent_fg
                          text "any member posts"
                            with
                              size=11.0
                              wrap=none
                              @text-caption
                      active bg=surface text=fg border=border border-w=1.5 r=9.0
                      hovered bg=muted_bg text=fg border=control_line
                      pressed bg=elevated text=fg
                  if members_only
                    box
                      with
                        w=fill
                        pl=12.0
                        pr=12.0
                        pt=8.0
                        pb=8.0
                        bg=muted_bg
                        border=primary
                        border-w=1.5
                        r=9.0
                      col w=fill gap=2.0
                        text "Members only"
                          with
                            size=12.5
                            wrap=none
                            font=medium
                            @text-accent_fg
                        text "added members post"
                          with
                            size=11.0
                            wrap=none
                            @text-caption
                  if !members_only
                    button -> emit(toggle_channel_create_members_only)
                      with
                        label="Members-only posting"
                        w=fill
                        p=0.0
                        @ghost_action
                      box
                        with
                          w=fill
                          pl=12.0
                          pr=12.0
                          pt=8.0
                          pb=8.0
                        col w=fill gap=2.0
                          text "Members only"
                            with
                              size=12.5
                              wrap=none
                              font=medium
                              @text-accent_fg
                          text "added members post"
                            with
                              size=11.0
                              wrap=none
                              @text-caption
                      active bg=surface text=fg border=border border-w=1.5 r=9.0
                      hovered bg=muted_bg text=fg border=control_line
                      pressed bg=elevated text=fg
              row
                with
                  w=fill
                  gap=8.0
                  align=center
                button "Cancel" -> emit(toggle_channel_create)
                  with
                    disabled=(busy)
                    w=fill
                    h=38.0
                    p=9.0
                    @secondary_action
                button "Create →" -> emit(create_channel_submit)
                  with
                    disabled=(loading || busy || !connected || empty(trim(draft)))
                    w=fill
                    h=38.0
                    p=9.0
                    @primary_action
    // THE TOAST HOST, mounted once for the whole app. It rides in this
    // full-window stack because WorkspaceTabs' overlay slots are `palette`
    // and `bell` and a slot takes one child; the palette below is a
    // top-anchored box, so the two never contend for the same pixels.
    if !empty(toast)
      box
        with
          w=fill
          h=fill
          align-x=center
          align-y=end
          pb=26.0
        button -> emit(dismiss_toast)
          with
            label="Dismiss"
            p=0.0
            @icon_action
          Toast message=toast tone=tone
          active bg=transparent text=fg border=transparent border-w=1.0 r=10.0
          hovered bg=transparent text=fg
          pressed bg=transparent text=fg
    // THE COMMAND PALETTE, on the same footing as the modal above. It used to
    // be a `box bg=scrim` wrapping a card, which is exactly what the note on
    // the modal warns against: the tint captured no pointer, so the rail and
    // the composer behind it stayed live and clicking the dim did nothing.
    // `overlay` takes the pointer and closes on the backdrop, and Esc keeps
    // working through `palette_key_action` — the hint in the field is now true
    // of one of two ways out rather than the only one.
    overlay
      with
        when=open
        dismiss=emit(close_palette)
        backdrop=scrim
        p=72.0
        align-x=center
        align-y=start
      content
        space w=fill h=fill
      layer
        box
          with
            w=540.0
            p=10.0
            bg=surface
            border=border
            border-w=1.0
            r=14.0
            shadow=shadow_modal
            shadow-y=24.0
            shadow-blur=60.0
          col w=fill gap=8.0
            input "" #palette-input <-> query
              with
                label="Search everything"
                change=emit(palette_changed, _)
                hint="Search messages and pages… (Esc closes)"
                submit=emit(close_palette)
                w=fill
                p=8.0
                text-size=13.0
                line-h=1.2
                @control
              active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
              hovered bg=elevated border=fg/21
            if search_phase == SearchPhase.searching
              text "Searching…" size=12.5 @text-muted
            // THE PALETTE ANSWERED, WITH NOTHING. `done` is written only where
            // a result lands, so — unlike a bare `!searching`, which a failure
            // also satisfies — this arm cannot claim a search that never ran
            // matched nothing. No draft term needed: `palette_changed` moves
            // the phase on every keystroke, so `done` cannot outlive the query
            // that earned it.
            if search_phase == SearchPhase.done && empty(chat_hits) && empty(page_hits)
              EmptyPlate message="Nothing matched that search."
            // AND THE FAILURE SAYS SO, HERE. `palette_search_failed` returns
            // the phase to idle and clears the hits — so without this arm the
            // panel collapsed to a bare field indistinguishable from one
            // nobody had typed into. An `error` assignment could not rescue
            // it: the error banner lives in the console column and this
            // palette is an `overlay` with `backdrop=scrim`, so the banner
            // sits BEHIND the scrim and its Dismiss cannot be clicked. This
            // arm is the palette's only word about the failure.
            //
            // The pair is reachable only after one: `palette_changed` raises
            // `searching` for every non-empty draft, so idle under a live
            // query has exactly one cause.
            if search_phase == SearchPhase.idle && !empty(trim(query))
              EmptyPlate message="Search failed."
            if !empty(chat_hits) || !empty(page_hits)
              // HUGS ITS RESULTS, up to a ceiling. A flat `h=380.0` meant one
              // hit sat at the top of a 380px panel with the rest of it empty —
              // the palette claimed a third of the window to show a single line.
              // `h=shrink` under a `max-h` box grows with the list and stops
              // before the palette outgrows the window.
              box w=fill max-h=380.0
                scroll
                  with
                    dir=vertical
                    w=fill
                    h=shrink
                  col w=fill gap=4.0
                      if !empty(chat_hits)
                        box w=fill pl=4.0
                          text "MESSAGES"
                            with
                              size=10.0
                              font=code_semibold
                              @text-muted
                        col w=fill gap=1.0
                          for hit in chat_hits
                            button -> emit(open_chat_search_hit, hit.channel_id, hit.root_seq, hit.seq)
                              with
                                label="Open message"
                                w=fill
                                p=6.0
                                @ghost_action
                              col w=fill gap=1.0
                                // `wrap=none` does not ellipsize — it lays the whole message body
                                // out as one run and the panel edge merely clips the draw, mid-glyph
                                // and with no marker, so a term matched late in a long message is
                                // never on screen. `hit.text` is the FULL body (backend/chat.rs
                                // copies it verbatim, no snippet), so the hit row has to wrap it —
                                // the way components/chat.ice renders this same ChatSearchHit.
                                text hit.text
                                  with
                                    size=13.0
                                    wrap=word-or-glyph
                                    @text-fg
                                text hit.meta
                                  with
                                    size=11.0
                                    wrap=none
                                    font=code_medium
                                    @text-muted
                              active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                              hovered bg=row_hover text=fg
                              pressed bg=accent
                      if !empty(page_hits)
                        box w=fill pl=4.0
                          text "PAGES"
                            with
                              size=10.0
                              font=code_semibold
                              @text-muted
                        col w=fill gap=1.0
                          for hit in page_hits
                            button -> emit(open_page_search_hit, hit.page_id, hit.block_id)
                              with
                                label="Open page"
                                w=fill
                                p=6.0
                                @ghost_action
                              col w=fill gap=1.0
                                // Same clip as the message hits above: a block's text is arbitrary
                                // page prose, and the match can sit anywhere in it.
                                text hit.text
                                  with
                                    size=13.0
                                    wrap=word-or-glyph
                                    @text-fg
                                // The metadata line names the PAGE, then the
                                // block kind — it read a bare `Text` before,
                                // which is true of nearly every hit and told
                                // the reader nothing about where the match is.
                                // Same shape as components/pages.ice renders
                                // this same PageSearchHit.
                                row
                                  with
                                    w=fill
                                    gap=7.0
                                    align=center
                                  text hit.page_title
                                    with
                                      w=fill
                                      size=12.0
                                      font=code_medium
                                      @text-muted
                                  text hit.kind
                                    with
                                      size=12.0
                                      wrap=none
                                      font=code
                                      @text-muted
                              active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                              hovered bg=row_hover text=fg
                              pressed bg=accent
