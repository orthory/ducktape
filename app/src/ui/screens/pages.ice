// PAGES — the whole document surface: the 230px page sidebar, the 50px document
// header, the doc-tab strip, the writing surface, and the 306px comments rail.
//
// A screen is a component like any other, which means it cannot reach app state
// — every reading it draws arrives as a prop, and every act it offers leaves as
// a named event that `view.ice` routes back to the handler of the same name.
//
// THE CANVAS IS ONE EDITOR. It used to be a stack of blocks where each line was
// a BUTTON until you clicked it, at which point a per-kind editor was swapped in
// behind it — so reaching a line cost a click that did nothing but change what
// the line was made of, and the page carried a `+`/`⋮⋮` gutter cluster, an
// insert row with a block-type dropdown parked at the right margin, and a `/`
// menu to pick from a list of kind names. All of that is gone. `page_document`
// (extern, `crate::pages`) is a single rich editor over the page's markdown:
// the caret lands where you click, and `# ` IS the block-type menu.
//
// WHAT THE DOCUMENT DOES NOT HOLD. Subpage blocks have no markdown spelling and
// are not prose, so a text diff has no business deciding they were deleted —
// they are listed under the body as the navigation they are.
//
// THREE DRAFTS ARE `bind` PROPS, because the writes are the app's, not this
// screen's: the sidebar's new-page field, the document search field and the
// rail's composer all write back to the state their handlers read and clear.
// `page_document` is bound too — the buffer IS app state, so the save tick can
// read it.
//
// THE TITLE IS LINE 0 OF THAT SAME BUFFER. It is a page property on the wire,
// not a block, but making it a separate control is what left the document with
// the very defect the body just lost: you had to CLICK the title to edit it.
// As line 0 it needs no control at all, and Enter at its end / Backspace at the
// body's start are ordinary text edits that cross the boundary for free.
component PagesScreen(network_chain_id:str, pages:[PageItem], page_create_open:bool, loading:bool, mutation_phase:MutationPhase, connected:bool, connected_rpc:str, password:str, dark:bool, bind page_draft:str, active_page:str, active_page_title:str, active_page_parent:str, bind page_search_draft:str, page_searching:bool, page_search_hits:[PageSearchHit], page_search_query:str, page_delete_armed:bool, block_autosave_status:AutosaveStatus, page_refusal:str, doc_tabs:[str], blocks:[PageBlock], commented_block_hits:[str], caret_comment_target:str, active_thread_target:str, active_thread_anchor:str, orphaned_comment_drafts:[str], bind page_editor:editor, block_comments_open:bool, block_comment_thread_total:i64, block_comment_threads:[PageCommentThread], block_comment_rows:[PageCommentThreadRow], block_comment_threads_loading:bool, block_comment_threads_has_more:bool, active_block_comment_thread:str, block_thread_comments:[PageComment], block_thread_comments_loading:bool, block_thread_comments_has_more:bool, bind block_comment_draft:str)
  emits
    toggle_page_create()
    create_page_submit()
    choose_page(str)
    search_pages_submit()
    clear_page_search()
    arm_page_delete()
    disarm_page_delete()
    delete_page_submit()
    close_doc_tab(str)
    open_page_search_hit(str, str)
    use_orphaned_comment_draft(str)
    discard_orphaned_comment_draft(str)
    page_edited(PageEvent)
    toggle_block_comments()
    close_block_comments()
    open_block_comment_thread(str, str)
    resolve_thread_submit(bool)
    load_more_block_threads()
    close_block_comment_thread()
    load_more_block_comments()
    post_block_comment_submit()
    copy_to_clipboard(str, str)
  row w=fill h=fill
    box
      with
        w=230.0
        h=fill
        bg=sidebar
        clip=true
      col
        with
          w=fill
          h=fill
          gap=0.0
        // THE SIDEBAR HEAD, from the component that owns the shape.
        // Chat's head is deliberately NOT this one: it interleaves a
        // connection dot BETWEEN the title and the count, which is past
        // the `space w=fill` this signature puts the slot behind.
        SidebarHeader title="Pages" count=len(pages)
          col
            if !page_create_open
              button -> emit(toggle_page_create)
                with
                  label="New page"
                  expanded=page_create_open
                  disabled=(loading || mutation_phase != MutationPhase.idle || !connected)
                  p=0.0
                  @icon_action
                Icon
                  with
                    name="plus"
                    tone="label"
                    px=16.0
                active bg=transparent text=muted border=transparent border-w=1.0 r=5.0
                hovered bg=separator text=fg
                pressed bg=subtle text=fg
            if page_create_open
              button -> emit(toggle_page_create)
                with
                  label="Close new page"
                  expanded=page_create_open
                  disabled=(loading || mutation_phase != MutationPhase.idle)
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
                  text "×"
                    with
                      size=13.0
                      wrap=none
                      @text-muted
                active bg=separator text=muted border=transparent border-w=1.0 r=5.0
                hovered bg=subtle text=fg
                pressed bg=subtle text=fg
        if page_create_open
          row
            with
              w=fill
              h=28.0
              gap=5.0
              align=center
            input "" #new-page <-> page_draft
              with
                label="New page title"
                hint="New page"
                disabled=(loading || mutation_phase != MutationPhase.idle || !connected)
                submit=emit(create_page_submit)
                w=fill
                p=6.2
                text-size=13.0
                line-h=1.2
                @control
              active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
              hovered bg=elevated border=fg/21
              disabled bg=muted_bg/54 value=muted
            button -> emit(create_page_submit)
              with
                label="Create page"
                disabled=(loading || mutation_phase != MutationPhase.idle || !connected || empty(trim(page_draft)))
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
                text "+" size=14.0
        scroll
          with
            dir=vertical
            w=fill
            h=fill
          col w=fill gap=2.0
            for page in pages
              PageButton page=page selected=(page.id == active_page)
                forward
                  choose_page
    box
      with
        w=1.0
        h=fill
        bg=separator
      space w=1.0 h=1.0
    row w=fill h=fill
      col w=fill h=fill
        // The 50px document header bar: the page title and the one
        // always-on trust signal the surface carries.
        if connected && !empty(active_page)
          col w=fill
            box
              with
                w=fill
                h=50.0
                pl=22.0
                pr=22.0
              row
                with
                  w=fill
                  h=fill
                  gap=9.0
                  align=center
                // THE TITLE IS BOUNDED, because it is the one thing in this
                // row a USER sizes. `wrap=none` lays the glyphs out at their
                // intrinsic width, and with no clipping ancestor a long title
                // simply keeps drawing — straight over the page search box,
                // the Comments button and the actions after it, which stay
                // clickable under paint they no longer own. The `box w=fill
                // clip=true` is the bound: the box takes the row's slack and
                // cuts the draw at its edge, the same shape the channel name
                // sits in in chat.ice.
                box w=fill clip=true
                  text active_page_title
                    with
                      size=13.5
                      wrap=none
                      font=display
                      @text-fg
                // DOCUMENT ACTIONS BELONG IN THE DOCUMENT HEADER. The
                // artifact's B1 bar is title + meta + the sync chip.
                // The parent crumb takes the artifact's `pgMeta` seat.
                if !empty(active_page_parent)
                  text active_page_parent
                    with
                      size=11.0
                      wrap=none
                      font=code
                      @text-hint
                input "" #page-search <-> page_search_draft
                  with
                    label="Search pages"
                    hint="Search pages…"
                    disabled=(!connected || page_searching)
                    submit=emit(search_pages_submit)
                    w=190.0
                    p=6.2
                    text-size=13.0
                    line-h=1.2
                    @control
                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                  hovered bg=fg/5 border=fg/8
                  focused bg=fg/5 border=ring
                  disabled value=muted
                // KEYED ON THE FIELD OR THE HITS: keyed on the hits alone, a
                // query that matched nothing hid its own ×, leaving the text
                // stuck in the box — and keyed on the field alone, hand-
                // emptying the field stranded the hits float with no dismiss
                // left (submit no-ops on an empty draft). The button clears
                // both, so either gates it.
                if !empty(trim(page_search_draft)) || !empty(page_search_hits)
                  button -> emit(clear_page_search)
                    with
                      label="Clear page search"
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
                // COMMENTS ARE A DOCUMENT ACTION, NOT A HIDDEN ONE. They
                // used to be reachable only by hovering a line, pressing
                // its `⋮⋮` handle and finding "Comments" in the menu that
                // opened — for a rail that is PAGE-scoped and never was
                // about that line (`load_page_threads` asks for the page
                // and all of its blocks at once). It sits in the header
                // wearing its own count, like every other document action.
                button -> emit(toggle_block_comments)
                  with
                    label="Comments"
                    expanded=block_comments_open
                    disabled=(mutation_phase != MutationPhase.idle)
                    h=26.0
                    p=5.0
                    @ghost_action
                  row
                    with
                      h=fill
                      gap=5.0
                      align=center
                    Icon
                      with
                        name="nav-chat"
                        tone="label"
                        px=14.0
                    text "Comments"
                      with
                        size=11.5
                        wrap=none
                        @text-muted
                    if block_comment_thread_total > 0
                      text count_label(block_comment_thread_total)
                        with
                          size=10.5
                          wrap=none
                          font=code_medium
                          @text-label
                  active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                  hovered bg=fg/8 text=fg border=fg/10
                  pressed bg=fg/12 text=fg
                // A page id is a uuid, so this button is the only way a
                // member gets a page's address out of the app at all. The
                // link it copies names the network it belongs to.
                button -> emit(copy_to_clipboard, duck_page_link(active_page, network_chain_id), "Page link copied")
                  with
                    label="Copy page link"
                    disabled=empty(active_page)
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
                    Icon
                      with
                        name="link"
                        tone="label"
                        px=14.0
                  active bg=transparent text=muted r=7.0
                  hovered bg=fg/10 text=fg
                  pressed bg=fg/15
                // The trigger STAYS a trigger: arming opens the named
                // confirm dialog below — it must never swap the red
                // button in under the same cursor.
                button -> emit(arm_page_delete)
                  with
                    label="Delete page"
                    disabled=(mutation_phase != MutationPhase.idle || page_delete_armed)
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
                    text "•••" size=13.0
                  active bg=transparent text=muted r=7.0
                  hovered bg=fg/10 text=fg
                  pressed bg=fg/15
                // THE TICK IS EARNED, NEVER ASSUMED. One discriminant,
                // one match, and `✓ synced` is painted for "saved"
                // alone: a write the node REFUSED says so, and an edit
                // still sitting in the buffer ("idle") carries no mark
                // at all.
                match block_autosave_status
                  AutosaveStatus.saving
                    box
                      with
                        px=9.0
                        py=4.0
                        bg=warning_bg
                        border=warning_line
                        border-w=1.0
                        r=7.0
                      text "saving…"
                        with
                          size=10.5
                          wrap=none
                          font=code_medium
                          @text-warning
                  AutosaveStatus.error
                    box
                      with
                        px=9.0
                        py=4.0
                        bg=danger_bg
                        border=danger_line
                        border-w=1.0
                        r=7.0
                      text "not saved"
                        with
                          size=10.5
                          wrap=none
                          font=code_medium
                          @text-danger
                  AutosaveStatus.saved
                    box
                      with
                        px=9.0
                        py=4.0
                        bg=final_bg
                        border=final_line
                        border-w=1.0
                        r=7.0
                      text "✓ synced"
                        with
                          size=10.5
                          wrap=none
                          font=code_medium
                          @text-success_tick
                  AutosaveStatus.idle
                    space w=1.0 h=1.0
            box
              with
                w=fill
                h=1.0
                bg=separator
              space w=1.0 h=1.0
        if connected && !empty(doc_tab_rows(doc_tabs, pages, active_page))
          box
            with
              w=fill
              h=34.0
              pl=8.0
              pr=8.0
              bg=sidebar
              border=separator
              border-w=1.0
            scroll
              with
                dir=horizontal
                w=fill
                h=fill
                bar=hidden
              row
                with
                  h=fill
                  gap=2.0
                  align=center
                for tab in doc_tab_rows(doc_tabs, pages, active_page)
                  row gap=0.0 align=center
                    button -> emit(choose_page, tab.id)
                      with
                        label="Open page tab"
                        checked=tab.active
                        h=26.0
                        p=5.0
                        @ghost_action
                      row
                        with
                          h=fill
                          gap=5.0
                          align=center
                        if tab.active
                          text tab.title
                            with
                              size=13.0
                              wrap=none
                              font=medium
                              @text-fg
                        if !tab.active
                          text tab.title
                            with
                              size=13.0
                              wrap=none
                              @text-muted
                      active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                      hovered bg=fg/5 text=fg
                      pressed bg=fg/8
                    button -> emit(close_doc_tab, tab.id)
                      with
                        label="Close page tab"
                        w=24.0
                        h=24.0
                        p=0.0
                        @icon_action
                      text "×" size=12.5 font=ui
                      active bg=transparent text=muted r=6.0
                      hovered bg=fg/8 text=fg
                      pressed bg=fg/12
        stack
          with
            w=fill
            h=fill
            clip=true
          if !connected
            EmptyState
              with
                title="Not connected"
                description="Click the network name in the titlebar to pick or reconnect a network."
          if connected && !loading && empty(active_page)
            EmptyState title="No page selected" description="Create a page from the sidebar."
          if connected && !empty(active_page)
            // NO outer scroll: the editor owns a FINITE viewport and scrolls
            // itself, which is what keeps its caret-reveal alive — an outer
            // scrollable would hand it infinite height, and typing below the
            // fold would walk the caret off screen with nothing following it.
            //
            // THE TWO PADDINGS ARE UNEQUAL BECAUSE THE EDITOR'S ARE. It keeps
            // its affordances inside its own padding, and the two sides are
            // not the same width: 46 of hover gutter (two buttons) on the
            // left against 28 of comment margin (one mark) on the right, +2 of
            // breathing room each side. Equal padding here therefore hung the
            // text 18px right of the surface it sits on, and the page read as
            // if it were sliding off. 22 + 48 == 40 + 30.
            //
            // The surface itself is LEFT-ANCHORED in a pane wider than 766 —
            // it carried an `mx=auto` that never ran, because the ice margin
            // family is honoured on FLEX ITEMS ONLY and this is a `stack`
            // child. Dropped rather than left to read as a promise. Centring
            // it needs a lever ice does not have here: `justify=center` sizes
            // the item to content, and a `box align-x=center` cannot centre a
            // `w=fill` child. Only shows above 1040 window width; the pane is
            // narrower than the surface at the console's minimum.
            box
              with
                w=fill
                h=fill
                max-w=766.0
                pl=22.0
                pr=40.0
                pt=26.0
                pb=18.0
              col
                with
                  w=fill
                  h=fill
                  gap=8.0
                // A REFUSED (or fence-held) WRITE SAYS SO, IN THE DOCUMENT.
                // An untouched buffer was rolled back to the canonical text by
                // the time this paints; a buffer the user kept typing into is
                // preserved, and this line explains why it is not saving yet.
                if !empty(page_refusal)
                  box
                    with
                      w=fill
                      px=12.0
                      py=9.0
                      bg=alert_bg
                      border=alert_line
                      border-w=1.0
                      r=9.0
                    text page_refusal
                      with
                        w=fill
                        size=12.5
                        wrap=word
                        @text-alert_fg
                if !empty(orphaned_comment_drafts)
                  box
                    with
                      w=fill
                      p=7.0
                      bg=elevated
                      border=fg/9
                      border-w=1.0
                      r=9.0
                    col w=fill gap=5.0
                      text "Recovered drafts"
                        with
                          size=13.0
                          font=medium
                          @text-fg
                      for recovered_comment in orphaned_comment_drafts
                        row
                          with
                            w=fill
                            gap=5.0
                            align=center
                          text recovered_comment
                            with
                              w=fill
                              size=13.5
                              @text-muted
                          button "Use" -> emit(use_orphaned_comment_draft, recovered_comment)
                            with
                              label="Use as comment"
                              disabled=(loading || mutation_phase != MutationPhase.idle)
                              h=26.0
                              p=5.0
                              @ghost_action
                            active bg=fg/9 text=fg border=fg/12 border-w=1.0 r=7.0
                            hovered bg=fg/14
                            pressed bg=fg/18
                          button "Discard" -> emit(discard_orphaned_comment_draft, recovered_comment)
                            with
                              disabled=(loading || mutation_phase != MutationPhase.idle)
                              h=26.0
                              p=5.0
                              @danger_action
                // THE PAGE. One editor, the whole document — see the file
                // header. It is never disabled while connected: a page you
                // can read is a page you can type in. It FILLS the column
                // and scrolls itself.
                extern page_document(page_editor, dark, (loading || !connected), blocks, commented_block_hits) #document -> emit(page_edited, _)
                // Subpages: navigation, listed rather than typed.
                if !empty(subpage_blocks(blocks))
                  // The 46px inset matches the editor's hover-gutter strip, so
                  // subpages align with the text column, not the gutter.
                  col
                    with
                      w=fill
                      gap=2.0
                      pt=10.0
                      pl=46.0
                    text "Subpages"
                      with
                        size=10.5
                        wrap=none
                        font=code_medium
                        @text-hint
                    for child in subpage_blocks(blocks)
                      button -> emit(choose_page, child.id)
                        with
                          label="Open subpage"
                          description=child.text
                          w=fill
                          p=6.0
                          @ghost_action
                        row
                          with
                            w=fill
                            gap=8.0
                            align=center
                          Icon
                            with
                              name="doc"
                              tone="label"
                              px=14.0
                          if empty(child.text)
                            text "Untitled"
                              with
                                w=fill
                                size=13.5
                                wrap=none
                                font=medium
                                @text-muted
                          if !empty(child.text)
                            text child.text
                              with
                                w=fill
                                size=13.5
                                wrap=none
                                font=medium
                                @text-fg
                          text "›"
                            with
                              size=13.0
                              wrap=none
                              @text-label
                        active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
                        hovered bg=fg/4 text=fg border=fg/7
                        pressed bg=fg/8 text=fg
          // THE HITS FLOAT, A SIBLING STACK LAYER FOR THE SAME REASONS AS THE
          // ZERO-HIT PLATE BELOW — and it needed them just as badly. Nested in
          // the document arm it was a row in the document COLUMN, so
          // `live_resynced` emptying `active_page` under a standing answer took
          // the whole thing down: the input, the ×, and the hits themselves,
          // leaving "No page selected" over a search nobody could see or
          // dismiss. Hoisted, the answer survives that arrival and a hit is the
          // way back into a document.
          //
          // Declared AFTER the document arm for the stack's paint order (see
          // the plate below), and it is the same opaque card it always was —
          // `bg=elevated` over live text. `connected` is carried here now that
          // no ancestor supplies it; the hits alone still gate the rest, since
          // every handler that drops them drops the standing query with them.
          if connected && !empty(page_search_hits)
            box
              with
                w=fill
                h=fill
                pl=22.0
                pr=40.0
                pt=26.0
                align-y=start
              box
                with
                  w=fill
                  max-w=766.0
                  h=148.0
                  p=5.0
                  bg=elevated
                  border=fg/8
                  border-w=1.0
                  r=9.0
                scroll
                  with
                    dir=vertical
                    w=fill
                    h=fill
                  col w=fill gap=1.0
                    for hit in page_search_hits
                      PageSearchResult hit=hit
                        forward
                          open_page_search_hit
          // NOTHING MATCHED — A STACK LAYER, NOT A ROW IN THE DOCUMENT COLUMN,
          // AND DECLARED AFTER THE DOCUMENT ARM: a stack draws its layers in
          // declaration order, first at the BOTTOM, so above the document this
          // card would paint UNDER the title and first lines and the editor
          // would own the pointer straight through it — the exact occlusion
          // the opaque card exists to prevent.
          //
          // NOT NESTED in the document arm, and not because a search could run
          // with no page open (the whole document header, page-search input
          // included, lives inside that same `connected && !empty(active_page)`
          // arm) — the plate's one real arrival with no page is `live_resynced`
          // moving `active_page` to "" under a standing query. Nested, that
          // showed "No page selected" and said nothing about the query still
          // standing; the × is gone with the header there, so picking a page
          // would be the only way out. It carries `connected` itself now that
          // no ancestor supplies it.
          //
          // ON THE QUERY, NOT ON A FLAG: this field is enter-to-submit and
          // two-way bound, so a keystroke runs no handler and only
          // `trim(draft) == query` can retire the plate as the user types.
          // That comparison is `search_answer_stands`, shared with chat and the
          // explorer — it carries the round trip too, and `page_search_failed`
          // drops the query.
          if connected && empty(page_search_hits) && search_answer_stands(page_search_query, page_search_draft, page_searching)
            box
              with
                w=fill
                h=fill
                pl=22.0
                pr=40.0
                pt=26.0
                align-y=start
              // OPAQUE: `EmptyPlate` is `bg=transparent` and this layer sits
              // over the live document (or over "No page selected"), which
              // would read straight through it.
              box
                with
                  w=fill
                  max-w=766.0
                  bg=elevated
                  r=12.0
                  shadow=shadow_popover
                  shadow-y=8.0
                  shadow-blur=24.0
                EmptyPlate message="No pages matched that search."
          overlay
            with
              when=page_delete_armed
              dismiss=emit(disarm_page_delete)
              backdrop=scrim
              p=30.0
              align-x=center
              align-y=center
            content
              space w=fill h=fill
            layer
              // NAME WHAT ACTUALLY DIES. `RemoveBlock` walks the whole subtree
              // and purges the comment threads on every descendant, so the
              // subpages listed right above this overlay go with it. The title
              // is line 0 of the editable doc and may be saved empty, so the
              // fallback sits HERE — `load.rs` feeds that same string to the
              // editor buffer, where "Untitled" would be written back as a
              // real title.
              ConfirmDelete
                with
                  title="Delete this page"
                  subject=keep_str(!empty(active_page_title), active_page_title, "Untitled")
                  note="Everything nested under it goes too — its blocks, any subpages beneath them, and every comment thread on any of it — for every member. This cannot be undone from the app."
                  action="Delete page"
                  busy=(mutation_phase != MutationPhase.idle)
                events
                  cancel -> emit(disarm_page_delete)
                  confirm -> emit(delete_page_submit)
      // The artifact hangs a 306px rail off the document, not a
      // floating card. The Spec tab is omitted: pages carry no kind,
      // no last-editor and no derivation pipeline (see omissions).
      if connected && !empty(active_page) && block_comments_open
        box
          with
            w=1.0
            h=fill
            bg=separator
          space w=1.0 h=1.0
        box
          with
            w=306.0
            h=fill
            bg=sidebar
            clip=true
          col w=fill h=fill
            box
              with
                w=fill
                h=50.0
                pl=16.0
                pr=16.0
              row
                with
                  w=fill
                  h=fill
                  gap=18.0
                  align=center
                TabLabel
                  with
                    label="Comments"
                    count=block_comment_thread_total
                    active=true
                space w=fill
                button -> emit(close_block_comments)
                  with
                    label="Close comments"
                    disabled=(mutation_phase != MutationPhase.idle)
                    w=24.0
                    h=24.0
                    p=4.0
                    @icon_action
                  box
                    with
                      w=fill
                      h=fill
                      align-x=center
                      align-y=center
                    text "×"
                      with
                        size=13.0
                        wrap=none
                        @text-muted
                  active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle text=fg
            box
              with
                w=fill
                h=1.0
                bg=separator
              space w=1.0 h=1.0
            col
              with
                w=fill
                h=fill
                p=12.0
                gap=6.0
              if empty(active_block_comment_thread)
                scroll
                  with
                    dir=vertical
                    w=fill
                    h=fill
                  col w=fill gap=1.0
                    if empty(block_comment_threads) && !block_comment_threads_loading
                      text "No comments yet"
                        with
                          w=fill
                          size=12.5
                          align-x=center
                          @text-muted
                    for comment_row in block_comment_rows
                      PageCommentThreadButton thread=comment_row.thread anchor=comment_row.anchor
                        forward
                          open_block_comment_thread
                    if block_comment_threads_has_more
                      button "More" -> emit(load_more_block_threads)
                        with
                          disabled=(block_comment_threads_loading || mutation_phase != MutationPhase.idle)
                          h=24.0
                          p=4.0
                          @secondary_action
                        active bg=transparent text=muted r=6.0
                        hovered bg=fg/9 text=fg
                        pressed bg=fg/14
              if !empty(active_block_comment_thread)
                row
                  with
                    w=fill
                    gap=5.0
                    align=center
                  button "← Threads" -> emit(close_block_comment_thread)
                    with
                      disabled=(block_thread_comments_loading || mutation_phase != MutationPhase.idle)
                      h=24.0
                      p=4.0
                      @secondary_action
                    active bg=transparent text=muted r=6.0
                    hovered bg=fg/9 text=fg
                    pressed bg=fg/14
                  text active_thread_anchor
                    with
                      w=fill
                      size=10.5
                      wrap=none
                      font=code_medium
                      @text-hint
                  if !thread_is_resolved(block_comment_threads, active_block_comment_thread)
                    button "Resolve" -> emit(resolve_thread_submit, true)
                      with
                        disabled=(mutation_phase != MutationPhase.idle)
                        h=24.0
                        p=4.0
                        @secondary_action
                      active bg=transparent text=muted r=6.0
                      hovered bg=fg/9 text=fg
                      pressed bg=fg/14
                  if thread_is_resolved(block_comment_threads, active_block_comment_thread)
                    button "Reopen" -> emit(resolve_thread_submit, false)
                      with
                        disabled=(mutation_phase != MutationPhase.idle)
                        h=24.0
                        p=4.0
                        @secondary_action
                      active bg=transparent text=muted r=6.0
                      hovered bg=fg/9 text=fg
                      pressed bg=fg/14
                scroll
                  with
                    dir=vertical
                    w=fill
                    h=fill
                  col w=fill gap=1.0
                    for page_comment in block_thread_comments
                      PageCommentCard comment=page_comment
                    if block_thread_comments_has_more
                      button "More" -> emit(load_more_block_comments)
                        with
                          disabled=(block_thread_comments_loading || mutation_phase != MutationPhase.idle)
                          h=24.0
                          p=4.0
                          @secondary_action
                        active bg=transparent text=muted r=6.0
                        hovered bg=fg/9 text=fg
                        pressed bg=fg/14
              if empty(active_block_comment_thread)
                text comment_compose_hint(blocks, caret_comment_target, active_page)
                  with
                    w=fill
                    size=10.5
                    wrap=none
                    font=code_medium
                    @text-hint
              row
                with
                  w=fill
                  gap=5.0
                  align=center
                input "" #page-comment(scope_key(connected_rpc, active_page)) <-> block_comment_draft
                  with
                    label="New page comment"
                    hint="Add a comment…"
                    disabled=(mutation_phase != MutationPhase.idle || block_comment_threads_loading || block_thread_comments_loading)
                    submit=emit(post_block_comment_submit)
                    w=fill
                    p=6.2
                    text-size=13.0
                    line-h=1.2
                    @control
                  active bg=transparent border=fg/8 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                  hovered bg=fg/4 border=fg/11
                  focused bg=fg/4 border=ring
                  disabled value=muted
                button "Post" -> emit(post_block_comment_submit)
                  with
                    disabled=(mutation_phase != MutationPhase.idle || empty(trim(block_comment_draft)) || block_comment_threads_loading || block_thread_comments_loading)
                    h=28.0
                    p=5.0
                    @primary_action
