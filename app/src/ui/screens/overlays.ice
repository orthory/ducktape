// THE WINDOW OVERLAYS — the create-channel modal, the toast host and the
// command palette. They share one slot because `WorkspaceTabs` gives the layer
// a single child; they share nothing else.
//
// `draft` and `query` are `bind` props: the inputs write straight back to the
// app state the view passes in, so the palette needs no local mirror of a
// field the handlers already own.
component OverlayLayer(create_open:bool, members_only:bool, bind draft:str, busy:bool, connected:bool, loading:bool, toast:str, tone:str, open:bool, bind query:str, searching:bool, chat_hits:[ChatSearchHit], page_hits:[PageSearchHit])
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
    overlay when=create_open dismiss=emit(toggle_channel_create) backdrop=scrim p=30.0 align-x=center align-y=center
      content
        space w=fill h=fill
      layer
        ModalShell title="Create a channel" width=418.0 #channel-modal
          close:
            button label="Close" disabled=(busy) w=26.0 h=26.0 p=0.0 @icon_action -> emit(toggle_channel_create)
              box w=fill h=fill align-x=center align-y=center
                text "×" size=14.0 wrap=none @text-muted
              active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
              hovered bg=elevated text=fg
              pressed bg=subtle text=fg
          body:
            col w=fill gap=13.0
              text "The channel is created immediately — there is no proposal and no approval step." w=fill size=12.0 line-h=1.5 @text-caption
              col w=fill gap=6.0
                Eyebrow label="CHANNEL NAME" note=""
                box w=fill pl=11.0 pr=11.0 pt=2.0 pb=2.0 bg=surface border=primary border-w=1.5 r=9.0
                  row w=fill gap=7.0 align=center
                    text "#" size=14.0 wrap=none font=code_medium @text-label
                    input "" #new-channel label="New channel name" <-> draft hint="design-review" disabled=(loading || busy || !connected) submit=emit(create_channel_submit) w=fill p=6.2 text-size=13.0 line-h=1.2 font=code @control
                      active bg=transparent border=transparent value=fg placeholder=hint selection=fg/18 border-w=0.0 r=0.0
                      hovered bg=transparent border=transparent
                      disabled value=muted
              col w=fill gap=6.0
                Eyebrow label="POSTING" note=""
                row w=fill gap=8.0 align=center
                  if !members_only
                    box w=fill pl=12.0 pr=12.0 pt=10.0 pb=10.0 bg=muted_bg border=primary border-w=1.5 r=9.0
                      text "Open · any member posts" size=12.0 wrap=none @text-accent_fg
                  if members_only
                    button label="Open posting" w=fill p=0.0 @ghost_action -> emit(toggle_channel_create_members_only)
                      box w=fill pl=12.0 pr=12.0 pt=10.0 pb=10.0
                        text "Open · any member posts" size=12.0 wrap=none @text-accent_fg
                      active bg=surface text=fg border=border border-w=1.5 r=9.0
                      hovered bg=muted_bg text=fg border=control_line
                      pressed bg=elevated text=fg
                  if members_only
                    box w=fill pl=12.0 pr=12.0 pt=10.0 pb=10.0 bg=muted_bg border=primary border-w=1.5 r=9.0
                      text "Members only · added members post" size=12.0 wrap=none @text-accent_fg
                  if !members_only
                    button label="Members-only posting" w=fill p=0.0 @ghost_action -> emit(toggle_channel_create_members_only)
                      box w=fill pl=12.0 pr=12.0 pt=10.0 pb=10.0
                        text "Members only · added members post" size=12.0 wrap=none @text-accent_fg
                      active bg=surface text=fg border=border border-w=1.5 r=9.0
                      hovered bg=muted_bg text=fg border=control_line
                      pressed bg=elevated text=fg
              row w=fill gap=8.0 align=center
                button "Cancel" disabled=(busy) w=fill h=38.0 p=9.0 @secondary_action -> emit(toggle_channel_create)
                button "Create →" disabled=(loading || busy || !connected || empty(trim(draft))) w=fill h=38.0 p=9.0 @primary_action -> emit(create_channel_submit)
    // THE TOAST HOST, mounted once for the whole app. It rides in this
    // full-window stack because WorkspaceTabs' overlay slots are `palette`
    // and `bell` and a slot takes one child; the palette below is a
    // top-anchored box, so the two never contend for the same pixels.
    if !empty(toast)
      box w=fill h=fill align-x=center align-y=end pb=26.0
        button label="Dismiss" p=0.0 @icon_action -> emit(dismiss_toast)
          Toast.Confirm message=toast tone=tone
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
    overlay when=open dismiss=emit(close_palette) backdrop=scrim p=72.0 align-x=center align-y=start
      content
        space w=fill h=fill
      layer
        box w=540.0 p=10.0 bg=surface border=border border-w=1.0 r=14.0 shadow=shadow_modal shadow-y=24.0 shadow-blur=60.0
          col w=fill gap=8.0
            input "" #palette-input label="Search everything" <-> query change=emit(palette_changed, _) hint="Search messages and pages… (Esc closes)" submit=emit(close_palette) w=fill p=8.0 text-size=13.0 line-h=1.2 @control
              active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
              hovered bg=elevated border=fg/21
            if searching
              text "Searching…" size=12.5 @text-muted
            if !empty(chat_hits) || !empty(page_hits)
              scroll dir=vertical w=fill h=380.0
                col w=fill gap=4.0
                  if !empty(chat_hits)
                    box w=fill pl=4.0
                      text "MESSAGES" size=10.0 font=code_semibold @text-muted
                    col w=fill gap=1.0
                      for hit in chat_hits
                        button label="Open message" w=fill p=6.0 @ghost_action -> emit(open_chat_search_hit, hit.channel_id, hit.root_seq, hit.seq)
                          col w=fill gap=1.0
                            text hit.text size=13.0 wrap=none @text-fg
                            text hit.meta size=11.0 wrap=none font=code_medium @text-muted
                          active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                          hovered bg=row_hover text=fg
                          pressed bg=accent
                  if !empty(page_hits)
                    box w=fill pl=4.0
                      text "PAGES" size=10.0 font=code_semibold @text-muted
                    col w=fill gap=1.0
                      for hit in page_hits
                        button label="Open page" w=fill p=6.0 @ghost_action -> emit(open_page_search_hit, hit.page_id, hit.block_id)
                          col w=fill gap=1.0
                            text hit.text size=13.0 wrap=none @text-fg
                            text hit.kind size=12.0 wrap=none font=code @text-muted
                          active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                          hovered bg=row_hover text=fg
                          pressed bg=accent
