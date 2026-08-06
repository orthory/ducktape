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
component PagesScreen(pages:[PageItem], page_create_open:bool, loading:bool, mutation_phase:str, connected:bool, connected_rpc:str, password:str, dark:bool, bind page_draft:str, active_page:str, active_page_title:str, active_page_parent:str, bind page_search_draft:str, page_searching:bool, page_search_hits:[PageSearchHit], page_delete_armed:bool, block_autosave_status:str, page_refusal:str, doc_tabs:[str], blocks:[PageBlock], orphaned_comment_drafts:[str], bind page_editor:editor, block_comments_open:bool, block_comment_thread_total:i64, block_comment_threads:[PageCommentThread], block_comment_threads_loading:bool, block_comment_threads_has_more:bool, active_block_comment_thread:str, block_thread_comments:[PageComment], block_thread_comments_loading:bool, block_thread_comments_has_more:bool, bind block_comment_draft:str)
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
    page_edited(PageAction)
    toggle_block_comments()
    close_block_comments()
    open_block_comment_thread(str)
    load_more_block_threads()
    close_block_comment_thread()
    load_more_block_comments()
    post_block_comment_submit()
  row w=fill h=fill
    box w=230.0 h=fill bg=sidebar clip=true
      col w=fill h=fill gap=0.0
        // THE SIDEBAR HEAD, from the component that owns the shape.
        // Chat's head is deliberately NOT this one: it interleaves a
        // connection dot BETWEEN the title and the count, which is past
        // the `space w=fill` this signature puts the slot behind.
        SidebarHeader title="Pages" count=len(pages)
          col
            if !page_create_open
              button label="New page" disabled=(loading || mutation_phase != "idle" || !connected) p=0.0 @icon_action -> emit(toggle_page_create)
                Icon name="plus" tone="label" px=16.0
                active bg=transparent text=muted border=transparent border-w=1.0 r=5.0
                hovered bg=separator text=fg
                pressed bg=subtle text=fg
            if page_create_open
              button label="Close new page" disabled=(loading || mutation_phase != "idle") w=24.0 h=24.0 p=0.0 @icon_action -> emit(toggle_page_create)
                box w=fill h=fill align-x=center align-y=center
                  text "×" size=13.0 wrap=none @text-muted
                active bg=separator text=muted border=transparent border-w=1.0 r=5.0
                hovered bg=subtle text=fg
                pressed bg=subtle text=fg
        if page_create_open
          row w=fill h=28.0 gap=5.0 align=center
            input "" #new-page label="New page title" <-> page_draft hint="New page" disabled=(loading || mutation_phase != "idle" || !connected) submit=emit(create_page_submit) w=fill p=6.2 text-size=13.0 line-h=1.2 @control
              active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
              hovered bg=elevated border=fg/21
              disabled bg=muted_bg/54 value=muted
            button label="Create page" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(page_draft))) w=28.0 h=28.0 p=0.0 @icon_action -> emit(create_page_submit)
              box w=fill h=fill align-x=center align-y=center
                text "+" size=14.0
        scroll dir=vertical w=fill h=fill
          col w=fill gap=2.0
            for page in pages
              PageButton page=page selected=(page.id == active_page)
                forward
                  choose_page
    box w=1.0 h=fill bg=separator
      space w=1.0 h=1.0
    row w=fill h=fill
      col w=fill h=fill
        // The 50px document header bar: the page title and the one
        // always-on trust signal the surface carries.
        if connected && !empty(active_page)
          col w=fill
            box w=fill h=50.0 pl=22.0 pr=22.0
              row w=fill h=fill gap=9.0 align=center
                text active_page_title w=fill size=13.5 wrap=none font=display @text-fg
                // DOCUMENT ACTIONS BELONG IN THE DOCUMENT HEADER. The
                // artifact's B1 bar is title + meta + the sync chip.
                // The parent crumb takes the artifact's `pgMeta` seat.
                if !empty(active_page_parent)
                  text active_page_parent size=11.0 wrap=none font=code @text-hint
                input "" #page-search label="Search pages" <-> page_search_draft hint="Search pages…" disabled=(!connected || page_searching) submit=emit(search_pages_submit) w=190.0 p=6.2 text-size=13.0 line-h=1.2 @control
                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                  hovered bg=fg/5 border=fg/8
                  disabled value=muted
                if !empty(page_search_hits)
                  button label="Clear page search" w=28.0 h=28.0 p=0.0 @icon_action -> emit(clear_page_search)
                    box w=fill h=fill align-x=center align-y=center
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
                button label="Comments" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @ghost_action -> emit(toggle_block_comments)
                  row h=fill gap=5.0 align=center
                    Icon name="message" tone="label" px=14.0
                    if block_comment_thread_total > 0
                      text count_label(block_comment_thread_total) size=10.5 wrap=none font=code_medium @text-muted
                  active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                  hovered bg=fg/8 text=fg border=fg/10
                  pressed bg=fg/12 text=fg
                // The trigger STAYS a trigger: arming opens the named
                // confirm dialog below — it must never swap the red
                // button in under the same cursor.
                button label="Delete page" disabled=(mutation_phase != "idle" || page_delete_armed) w=28.0 h=28.0 p=0.0 @icon_action -> emit(arm_page_delete)
                  box w=fill h=fill align-x=center align-y=center
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
                  "saving"
                    box px=9.0 py=4.0 bg=warning_bg border=warning_line border-w=1.0 r=7.0
                      text "saving…" size=10.5 wrap=none font=code_medium @text-warning
                  "error"
                    box px=9.0 py=4.0 bg=danger_bg border=danger_line border-w=1.0 r=7.0
                      text "not saved" size=10.5 wrap=none font=code_medium @text-danger
                  "saved"
                    box px=9.0 py=4.0 bg=final_bg border=final_line border-w=1.0 r=7.0
                      text "✓ synced" size=10.5 wrap=none font=code_medium @text-success_tick
                  _
                    space w=1.0 h=1.0
            box w=fill h=1.0 bg=separator
              space w=1.0 h=1.0
        if connected && !empty(doc_tab_rows(doc_tabs, pages, active_page))
          box w=fill h=34.0 pl=8.0 pr=8.0 bg=sidebar border=separator border-w=1.0
            scroll dir=horizontal w=fill h=fill bar=hidden
              row h=fill gap=2.0 align=center
                for tab in doc_tab_rows(doc_tabs, pages, active_page)
                  row gap=0.0 align=center
                    button label="Open page tab" h=26.0 p=5.0 @ghost_action -> emit(choose_page, tab.id)
                      row h=fill gap=5.0 align=center
                        if tab.active
                          text tab.title size=13.0 wrap=none font=medium @text-fg
                        if !tab.active
                          text tab.title size=13.0 wrap=none @text-muted
                      active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                      hovered bg=fg/5 text=fg
                      pressed bg=fg/8
                    button "×" label="Close page tab" w=24.0 h=24.0 p=0.0 @icon_action -> emit(close_doc_tab, tab.id)
                      active bg=transparent text=muted r=6.0
                      hovered bg=fg/8 text=fg
                      pressed bg=fg/12
        stack w=fill h=fill clip=true
          if !connected
            EmptyState title="Not connected" description="Click the network name in the titlebar to pick or reconnect a network."
          if connected && !loading && empty(active_page)
            EmptyState title="No page selected" description="Create a page from the sidebar."
          if connected && !empty(active_page)
            scroll dir=vertical w=fill h=fill bar=hidden
              box w=fill max-w=720.0 mx=auto pl=22.0 pr=22.0 pt=26.0 pb=120.0
                col w=fill gap=8.0
                  if !empty(page_search_hits)
                    box w=fill h=148.0 p=5.0 bg=elevated border=fg/8 border-w=1.0 r=9.0
                      scroll dir=vertical w=fill h=fill
                        col w=fill gap=1.0
                          for hit in page_search_hits
                            PageSearchResult hit=hit
                              forward
                                open_page_search_hit
                  // A REFUSED WRITE SAYS SO, IN THE DOCUMENT. The buffer has
                  // already been rolled back to the canonical text by the
                  // time this paints, so the line explains a change that
                  // just visibly undid itself.
                  if !empty(page_refusal)
                    box w=fill px=12.0 py=9.0 bg=alert_bg border=alert_line border-w=1.0 r=9.0
                      text page_refusal w=fill size=12.5 wrap=word @text-alert_fg
                  if !empty(orphaned_comment_drafts)
                    box w=fill p=7.0 bg=elevated border=fg/9 border-w=1.0 r=9.0
                      col w=fill gap=5.0
                        text "Recovered drafts" size=13.0 font=medium @text-fg
                        for recovered_comment in orphaned_comment_drafts
                          row w=fill gap=5.0 align=center
                            text recovered_comment w=fill size=13.5 @text-muted
                            button "Use" label="Use as comment" disabled=(loading || mutation_phase != "idle") h=26.0 p=5.0 @ghost_action -> emit(use_orphaned_comment_draft, recovered_comment)
                              active bg=fg/9 text=fg border=fg/12 border-w=1.0 r=7.0
                              hovered bg=fg/14
                              pressed bg=fg/18
                            button "Discard" disabled=(loading || mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> emit(discard_orphaned_comment_draft, recovered_comment)
                  // THE PAGE. One editor, the whole document — see the file
                  // header. It is never disabled while connected: a page you
                  // can read is a page you can type in.
                  extern page_document(page_editor, dark, (loading || !connected)) #document -> emit(page_edited, _)
                  // Subpages: navigation, listed rather than typed.
                  if !empty(subpage_blocks(blocks))
                    col w=fill gap=2.0 pt=18.0
                      text "Subpages" size=10.5 wrap=none font=code_medium @text-hint
                      for child in subpage_blocks(blocks)
                        button label="Open subpage" description=child.text w=fill p=6.0 @ghost_action -> emit(choose_page, child.id)
                          row w=fill gap=8.0 align=center
                            Icon name="doc" tone="label" px=14.0
                            if empty(child.text)
                              text "Untitled" w=fill size=13.5 wrap=none font=medium @text-muted
                            if !empty(child.text)
                              text child.text w=fill size=13.5 wrap=none font=medium @text-fg
                            text "›" size=13.0 wrap=none @text-label
                          active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
                          hovered bg=fg/4 text=fg border=fg/7
                          pressed bg=fg/8 text=fg
          overlay when=page_delete_armed dismiss=emit(disarm_page_delete) backdrop=scrim p=30.0 align-x=center align-y=center
            content
              space w=fill h=fill
            layer
              ConfirmDelete title="Delete this page" subject=active_page_title note="The page and every block on it are deleted for every member. This cannot be undone from the app." action="Delete page" busy=(mutation_phase != "idle")
                events
                  cancel -> emit(disarm_page_delete)
                  confirm -> emit(delete_page_submit)
      // The artifact hangs a 306px rail off the document, not a
      // floating card. The Spec tab is omitted: pages carry no kind,
      // no last-editor and no derivation pipeline (see omissions).
      if connected && !empty(active_page) && block_comments_open
        box w=1.0 h=fill bg=separator
          space w=1.0 h=1.0
        box w=306.0 h=fill bg=sidebar clip=true
          col w=fill h=fill
            box w=fill h=50.0 pl=16.0 pr=16.0
              row w=fill h=fill gap=18.0 align=center
                TabLabel label="Comments" count=block_comment_thread_total active=true
                space w=fill
                button label="Close comments" disabled=(mutation_phase != "idle") w=24.0 h=24.0 p=4.0 @icon_action -> emit(close_block_comments)
                  box w=fill h=fill align-x=center align-y=center
                    text "×" size=13.0 wrap=none @text-muted
                  active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle text=fg
            box w=fill h=1.0 bg=separator
              space w=1.0 h=1.0
            col w=fill h=fill p=12.0 gap=6.0
              if empty(active_block_comment_thread)
                scroll dir=vertical w=fill h=fill
                  col w=fill gap=1.0
                    if empty(block_comment_threads) && !block_comment_threads_loading
                      text "No comments yet" w=fill size=12.5 align-x=center @text-muted
                    for comment_thread in block_comment_threads
                      PageCommentThreadButton thread=comment_thread
                        forward
                          open_block_comment_thread
                    if block_comment_threads_has_more
                      button "More" disabled=(block_comment_threads_loading || mutation_phase != "idle") h=24.0 p=4.0 @secondary_action -> emit(load_more_block_threads)
                        active bg=transparent text=muted r=6.0
                        hovered bg=fg/9 text=fg
                        pressed bg=fg/14
              if !empty(active_block_comment_thread)
                row w=fill gap=5.0 align=center
                  button "← Threads" disabled=(block_thread_comments_loading || mutation_phase != "idle") h=24.0 p=4.0 @secondary_action -> emit(close_block_comment_thread)
                    active bg=transparent text=muted r=6.0
                    hovered bg=fg/9 text=fg
                    pressed bg=fg/14
                scroll dir=vertical w=fill h=fill
                  col w=fill gap=1.0
                    for page_comment in block_thread_comments
                      PageCommentCard comment=page_comment
                    if block_thread_comments_has_more
                      button "More" disabled=(block_thread_comments_loading || mutation_phase != "idle") h=24.0 p=4.0 @secondary_action -> emit(load_more_block_comments)
                        active bg=transparent text=muted r=6.0
                        hovered bg=fg/9 text=fg
                        pressed bg=fg/14
              row w=fill gap=5.0 align=center
                input "" #page-comment(scope_key(connected_rpc, active_page)) label="New page comment" <-> block_comment_draft hint="Add a comment…" disabled=(mutation_phase != "idle" || block_comment_threads_loading || block_thread_comments_loading) submit=emit(post_block_comment_submit) w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                  active bg=transparent border=fg/8 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                  hovered bg=fg/4 border=fg/11
                  disabled value=muted
                button "Post" disabled=(mutation_phase != "idle" || empty(trim(block_comment_draft)) || block_comment_threads_loading || block_thread_comments_loading) h=28.0 p=5.0 @primary_action -> emit(post_block_comment_submit)
