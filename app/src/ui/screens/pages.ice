// PAGES — the whole document surface: the 230px page sidebar, the 50px document
// header, the doc-tab strip, the block canvas with its inline insert and block
// actions, and the 306px comments rail the artifact hangs off the document.
//
// A screen is a component like any other, which means it cannot reach app state
// — every reading it draws arrives as a prop, and every act it offers leaves as
// a named event that `view.ice` routes back to the handler of the same name.
// The page COMPONENTS this screen arranges (PageButton, PageTitleEditor,
// DocumentBlock, BlockLine, BlockContents, InlineBlockInsert, BlockActionsMenu,
// PageCommentThreadButton, PageCommentCard) live in `components/pages.ice` and
// are unchanged; this file is only the screen.
//
// FIVE DRAFTS ARE `bind` PROPS, because the writes are the app's, not this
// screen's: the sidebar's new-page field, the document search field, the block
// insert field, the selected block's editor and the rail's composer all write
// back to the state their handlers read and clear. The doc TITLE editor is not
// among them — `PageTitleEditor` owns a local draft and autosaves through its
// own `run`, so `title` stays an ordinary read-only prop.
//
// The size sensor is the canvas stack's FIRST child, so it measures the canvas
// and not the whole tab. It reports through this screen's own `pages_resized`;
// it was briefly a caller-filled slot, because a sensor could not carry a
// component event until ui-lang#239.
component PagesScreen(pages:[PageItem], page_create_open:bool, loading:bool, mutation_phase:str, connected:bool, connected_rpc:str, password:str, bind page_draft:str, active_page:str, active_page_title:str, active_page_parent:str, bind page_search_draft:str, page_searching:bool, page_search_hits:[PageSearchHit], page_delete_armed:bool, block_autosave_status:str, doc_tabs:[str], blocks:[PageBlock], orphaned_block_drafts:[str], orphaned_comment_drafts:[str], bind block_draft:str, block_insert_open:bool, block_insert_after_id:str, new_block_kind:str, block_kinds:[str], editable_block_kinds:[str], selected_block_id:str, selected_block_kind:str, hovered_block_id:str, bind block_editor:editor, block_actions_open:bool, block_menu_x:f64, block_menu_y:f64, block_delete_armed:bool, block_comments_open:bool, block_comment_thread_total:i64, block_comment_threads:[PageCommentThread], block_comment_threads_loading:bool, block_comment_threads_has_more:bool, active_block_comment_thread:str, block_thread_comments:[PageComment], block_thread_comments_loading:bool, block_thread_comments_has_more:bool, bind block_comment_draft:str)
  emits
    toggle_page_create()
    create_page_submit()
    choose_page(str)
    pages_pointer_moved(f64, f64)
    search_pages_submit()
    clear_page_search()
    arm_page_delete()
    disarm_page_delete()
    delete_page_submit()
    close_doc_tab(str)
    open_page_search_hit(str, str)
    use_orphaned_block_draft(str)
    discard_orphaned_block_draft(str)
    use_orphaned_comment_draft(str)
    discard_orphaned_comment_draft(str)
    open_root_block_insert()
    new_block_kind_changed(str)
    close_block_insert()
    add_block_submit()
    pick_slash_kind(str)
    block_entered(str)
    block_exited(str)
    open_block_insert(i64, str)
    select_block(i64, str, str, str, bool, bool)
    set_todo_checked(str, bool)
    block_key(BlockKeyEvent)
    block_draft_changed(str)
    close_block_actions()
    selected_block_kind_changed(str)
    move_block_submit(str)
    open_block_comments()
    arm_block_delete()
    remove_block_submit()
    close_block_comments()
    open_block_comment_thread(str)
    load_more_block_threads()
    close_block_comment_thread()
    load_more_block_comments()
    post_block_comment_submit()
    pages_resized(f64, f64)
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
    mouse move=emit(pages_pointer_moved, _, _)
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
                  // artifact's B1 bar is title + meta + the sync chip and
                  // its block canvas opens on the doc H1; these three sat
                  // in the canvas instead, floating above the title with
                  // no plate under them while this 50px bar ran empty.
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
                  // still sitting in the draft ("idle") carries no mark
                  // at all. The old predicate read "nothing in flight",
                  // which is true of both of those. The `offline` pill
                  // goes with it — this bar only draws inside
                  // `if connected && !empty(active_page)`, so it never
                  // could paint.
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
            sensor show=emit(pages_resized, _, _) resize=emit(pages_resized, _, _)
              space w=fill h=fill
            if !connected
              EmptyState title="Connect to a node" description="Open Settings from the rail below and set the node endpoint."
            if connected && !loading && empty(active_page)
              EmptyState title="No page selected" description="Create a page from the sidebar."
            if connected && !empty(active_page)
              stack w=fill h=fill
                scroll dir=vertical w=fill h=fill bar=hidden
                  box w=fill max-w=720.0 mx=auto pl=22.0 pr=22.0 pt=26.0 pb=40.0
                    col w=fill gap=8.0
                      box w=fill pl=56.0
                        PageTitleEditor rpc=connected_rpc password=password page_id=active_page title=active_page_title disabled=(loading || !connected || mutation_phase != "idle") #page-title(scope_key(connected_rpc, active_page))
                      if !empty(orphaned_block_drafts) || !empty(orphaned_comment_drafts)
                        box w=fill p=7.0 bg=elevated border=fg/9 border-w=1.0 r=9.0
                          col w=fill gap=5.0
                            text "Recovered drafts" size=13.0 font=medium @text-fg
                            for recovered_block in orphaned_block_drafts
                              row w=fill gap=5.0 align=center
                                text recovered_block w=fill size=13.5 @text-muted
                                button "Use" label="Use as block" disabled=(loading || mutation_phase != "idle" || !empty(block_draft)) h=26.0 p=5.0 @ghost_action -> emit(use_orphaned_block_draft, recovered_block)
                                  active bg=fg/9 text=fg border=fg/12 border-w=1.0 r=7.0
                                  hovered bg=fg/14
                                  pressed bg=fg/18
                                button "Discard" disabled=(loading || mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> emit(discard_orphaned_block_draft, recovered_block)
                            for recovered_comment in orphaned_comment_drafts
                              row w=fill gap=5.0 align=center
                                text recovered_comment w=fill size=13.5 @text-muted
                                button "Use" label="Use as block" disabled=(loading || mutation_phase != "idle" || !empty(block_draft)) h=26.0 p=5.0 @ghost_action -> emit(use_orphaned_comment_draft, recovered_comment)
                                  active bg=fg/9 text=fg border=fg/12 border-w=1.0 r=7.0
                                  hovered bg=fg/14
                                  pressed bg=fg/18
                                button "Discard" disabled=(loading || mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> emit(discard_orphaned_comment_draft, recovered_comment)
                      if empty(blocks) && !block_insert_open
                        box w=fill pl=56.0
                          button "Write something…" label="Start writing" disabled=loading w=fill p=6.0 @ghost_action -> emit(open_root_block_insert)
                            active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                            hovered bg=fg/4 text=fg border=fg/7
                            pressed bg=fg/8
                      if block_insert_open && empty(block_insert_after_id)
                        InlineBlockInsert kind=new_block_kind kinds=block_kinds disabled=loading prefix="" #block-insert-row(block_insert_after_id)
                          forward
                            new_block_kind_changed
                            close_block_insert
                          stack w=fill
                            if new_block_kind != "Divider"
                              col w=fill gap=2.0
                                input "" #block-insert label="New block" <-> block_draft change=emit(block_draft_changed, _) hint="Type, or / for a block kind…" disabled=loading submit=emit(add_block_submit) w=fill p=5.0 text-size=13.5 line-h=1.3 @control
                                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=6.0
                                  hovered bg=fg/2 border=fg/5
                                  disabled value=muted
                                if !empty(slash_kind_matches(block_draft, editable_block_kinds))
                                  box w=fill p=3.0 bg=surface border=border border-w=1.0 r=8.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
                                    col w=fill gap=1.0
                                      for kind in slash_kind_matches(block_draft, editable_block_kinds)
                                        button label="Set block kind" w=fill h=24.0 p=4.0 @ghost_action -> emit(pick_slash_kind, kind)
                                          row w=fill h=fill gap=6.0 align=center
                                            text kind w=fill size=13.0 wrap=none @text-fg
                                          active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                          hovered bg=brand/14 text=fg
                                          pressed bg=brand/20
                            if new_block_kind == "Divider"
                              button "Insert divider" disabled=loading w=fill h=28.0 p=5.0 @secondary_action -> emit(add_block_submit)
                      keyed block in blocks by=block.key
                        col w=fill gap=1.0
                          DocumentBlock block=block selected=(block.id == selected_block_id) hovered=(block.id == hovered_block_id) disabled=loading #block(block.id)
                            forward
                              block_entered
                              block_exited
                              open_block_insert
                              select_block
                            col w=fill
                              if block.pending
                                box w=fill p=5.0 bg=fg/3 r=6.0
                                  BlockContents block=block
                                    forward
                                      set_todo_checked
                              if !block.pending && block.kind == "Page"
                                button label=block.kind description=block.text w=fill p=5.0 @ghost_action -> emit(choose_page, block.id)
                                  BlockContents block=block
                                    forward
                                      set_todo_checked
                                  active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                  hovered bg=fg/3 text=fg border=transparent
                                  pressed bg=fg/6 text=fg
                              if !block.pending && block.kind != "Page" && block.id != selected_block_id
                                button label=block.kind description=block.text w=fill p=5.0 @ghost_action -> emit(select_block, block.key, block.id, block.kind, block.text, block.checked, false)
                                  BlockContents block=block
                                    forward
                                      set_todo_checked
                                  active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                  hovered bg=fg/3 text=fg border=transparent
                                  pressed bg=fg/6 text=fg
                              if !block.pending && block.kind != "Page" && block.id == selected_block_id
                                BlockLine block=block #line
                                  forward
                                    set_todo_checked
                                  col w=fill
                                    if block.kind == "Divider"
                                      Separator
                                    if block.kind != "Divider"
                                      // THE EDITOR WEARS THE BLOCK'S OWN CLOTHES. Each kind
                                      // edits at the metrics BlockContents renders it with —
                                      // same size, rhythm (pt), font and ink — so selecting a
                                      // block swaps a caret in, not a costume. The structural
                                      // keys route through `block_key_press` (Enter splits,
                                      // Backspace-on-empty deletes, Tab indents, Esc leaves);
                                      // everything else stays native editing, and the
                                      // highlighter is the composers' own markdown table.
                                      // Code alone drops it: marks inside code are content.
                                      match block.kind
                                        "Heading 1"
                                          box w=fill pt=20.0
                                            editor #block-edit(block.kind) <-> block_editor hint="Heading 1" disabled=(mutation_phase != "idle") size=20.0 line-h=1.25 p=0.0 wrap=word font=display key-binding=block_key_press(block.kind, empty(trim(editor_text(block_editor))), (block.child_count > 0)) highlighter=page_inline_marks() -> emit(block_key, _)
                                              active bg=transparent border=transparent value=primary placeholder=muted selection=fg/18
                                              disabled value=muted
                                        "Heading 2"
                                          box w=fill pt=20.0
                                            editor #block-edit(block.kind) <-> block_editor hint="Heading 2" disabled=(mutation_phase != "idle") size=16.0 line-h=1.3 p=0.0 wrap=word font=display key-binding=block_key_press(block.kind, empty(trim(editor_text(block_editor))), (block.child_count > 0)) highlighter=page_inline_marks() -> emit(block_key, _)
                                              active bg=transparent border=transparent value=primary placeholder=muted selection=fg/18
                                              disabled value=muted
                                        "Heading 3"
                                          box w=fill pt=16.0
                                            editor #block-edit(block.kind) <-> block_editor hint="Heading 3" disabled=(mutation_phase != "idle") size=14.0 line-h=1.35 p=0.0 wrap=word font=display key-binding=block_key_press(block.kind, empty(trim(editor_text(block_editor))), (block.child_count > 0)) highlighter=page_inline_marks() -> emit(block_key, _)
                                              active bg=transparent border=transparent value=primary placeholder=muted selection=fg/18
                                              disabled value=muted
                                        "Quote"
                                          box w=fill pt=16.0
                                            row w=fill gap=0.0 align=start
                                              box w=2.0 h=fill bg=control_line
                                                space w=1.0 h=1.0
                                              box w=fill pl=14.0
                                                editor #block-edit(block.kind) <-> block_editor hint="Quote" disabled=(mutation_phase != "idle") size=14.0 line-h=1.6 p=0.0 wrap=word font=italic key-binding=block_key_press(block.kind, empty(trim(editor_text(block_editor))), (block.child_count > 0)) highlighter=page_inline_marks() -> emit(block_key, _)
                                                  active bg=transparent border=transparent value=muted placeholder=muted selection=fg/18
                                                  disabled value=muted
                                        "Code"
                                          box w=fill pt=14.0
                                            box w=fill pl=15.0 pr=15.0 pt=13.0 pb=13.0 bg=primary r=10.0 clip=true
                                              editor #block-edit(block.kind) <-> block_editor hint="Code" disabled=(mutation_phase != "idle") size=12.0 line-h=1.6 p=0.0 wrap=none font=code key-binding=block_key_press(block.kind, empty(trim(editor_text(block_editor))), (block.child_count > 0)) -> emit(block_key, _)
                                                active bg=transparent border=transparent value=chevron_idle placeholder=muted selection=fg/30
                                                disabled value=muted
                                        "Callout"
                                          box w=fill pt=14.0
                                            box w=fill pl=15.0 pr=15.0 pt=13.0 pb=13.0 bg=card_wash border=separator border-w=1.0 r=11.0
                                              row w=fill gap=11.0 align=start
                                                box w=20.0 h=20.0 align-x=center align-y=center bg=brand_bg r=6.0
                                                  text "i" size=10.0 wrap=none font=code_semibold @text-brand
                                                editor #block-edit(block.kind) <-> block_editor hint="Callout" disabled=(mutation_phase != "idle") size=13.0 line-h=1.6 p=0.0 wrap=word key-binding=block_key_press(block.kind, empty(trim(editor_text(block_editor))), (block.child_count > 0)) highlighter=page_inline_marks() -> emit(block_key, _)
                                                  active bg=transparent border=transparent value=panel_tile placeholder=muted selection=fg/18
                                                  disabled value=muted
                                        _
                                          box w=fill pt=8.0
                                            editor #block-edit(block.kind) <-> block_editor hint="Type something…" disabled=(mutation_phase != "idle") size=14.0 line-h=1.65 p=0.0 wrap=word key-binding=block_key_press(block.kind, empty(trim(editor_text(block_editor))), (block.child_count > 0)) highlighter=page_inline_marks() -> emit(block_key, _)
                                              active bg=transparent border=transparent value=accent_fg placeholder=muted selection=fg/18
                                              disabled value=muted
                          if block_insert_open && block.id == block_insert_after_id
                            InlineBlockInsert kind=new_block_kind kinds=block_kinds disabled=loading prefix=block.prefix #block-insert-row(block_insert_after_id)
                              forward
                                new_block_kind_changed
                                close_block_insert
                              stack w=fill
                                if new_block_kind != "Divider"
                                  col w=fill gap=2.0
                                    input "" #block-insert label="New block" <-> block_draft change=emit(block_draft_changed, _) hint="Type, or / for a block kind…" disabled=loading submit=emit(add_block_submit) w=fill p=5.0 text-size=13.5 line-h=1.3 @control
                                      active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=6.0
                                      hovered bg=fg/2 border=fg/5
                                      disabled value=muted
                                    if !empty(slash_kind_matches(block_draft, editable_block_kinds))
                                      box w=fill p=3.0 bg=surface border=border border-w=1.0 r=8.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
                                        col w=fill gap=1.0
                                          for kind in slash_kind_matches(block_draft, editable_block_kinds)
                                            button label="Set block kind" w=fill h=24.0 p=4.0 @ghost_action -> emit(pick_slash_kind, kind)
                                              row w=fill h=fill gap=6.0 align=center
                                                text kind w=fill size=13.0 wrap=none @text-fg
                                              active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                              hovered bg=brand/14 text=fg
                                              pressed bg=brand/20
                                if new_block_kind == "Divider"
                                  button "Insert divider" disabled=loading w=fill h=28.0 p=5.0 @secondary_action -> emit(add_block_submit)
                // THE RESULTS FLOAT — the same fix as the chat stream:
                // inline under the title this card reflowed the whole
                // document down by 148px per search.
                if !empty(page_search_hits)
                  box w=fill h=fill align-y=start
                    box w=fill max-w=720.0 mx=auto pl=22.0 pr=22.0 pt=26.0
                      box w=fill h=148.0 p=5.0 bg=elevated border=fg/8 border-w=1.0 r=9.0 shadow=shadow_popover shadow-y=8.0 shadow-blur=24.0
                        scroll dir=vertical w=fill h=fill
                          col w=fill gap=1.0
                            for hit in page_search_hits
                              PageSearchResult hit=hit
                                forward
                                  open_page_search_hit
            overlay when=(connected && !empty(active_page) && !empty(selected_block_id) && block_actions_open) dismiss=emit(close_block_actions) backdrop=transparent p=0.0 align-x=start align-y=start
              content
                space w=fill h=fill
              layer
                float x=(block_menu_x + 10.0) y=block_menu_y
                  BlockActionsMenu block_id=selected_block_id kind=selected_block_kind disabled=(loading || mutation_phase != "idle") delete_armed=block_delete_armed editable_kinds=editable_block_kinds
                    forward
                      selected_block_kind_changed
                      choose_page
                      move_block_submit
                      open_block_comments
                      arm_block_delete
                      remove_block_submit
                      close_block_actions
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
                  input "" #block-comment(scope_key(connected_rpc, selected_block_id)) label="New block comment" <-> block_comment_draft hint="Add a comment…" disabled=(mutation_phase != "idle" || block_comment_threads_loading || block_thread_comments_loading) submit=emit(post_block_comment_submit) w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                    active bg=transparent border=fg/8 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                    hovered bg=fg/4 border=fg/11
                    disabled value=muted
                  button "Post" disabled=(mutation_phase != "idle" || empty(trim(block_comment_draft)) || block_comment_threads_loading || block_thread_comments_loading) h=28.0 p=5.0 @primary_action -> emit(post_block_comment_submit)
