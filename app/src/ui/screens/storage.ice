// THE TWO STORAGE SCREENS: what this workspace has stored under duckfs, and
// everything it has recorded anywhere. Files is the three-pane duckfs browser
// plus the write bar; Explorer is workspace search over the block ledger it
// falls back to.
//
// See `screens/roster.ice` for the screen contract: a screen is a component, so
// it cannot reach app state — every reading arrives as a prop and every act
// leaves as a named event that `view.ice` routes back to the handler of the
// same name.

component FilesScreen(path:str, entries:[FsEntry], connected:bool, loading:bool, bind new_name:str, preview_path:str, delete_target:str, history_open:bool, diff_from:str, diff:[FsDiffEntry], history:[FsSnapshot], preview_truncated:bool, preview_binary:bool, editing:bool, bind draft:editor, preview_text:str)
  emits
    fs_open_dir(str)
    fs_open_file(str)
    fs_open_parent()
    fs_new_name_changed(str)
    fs_mkdir_submit()
    fs_new_file_submit()
    fs_arm_delete(str)
    fs_disarm_delete()
    fs_delete_submit()
    fs_toggle_history()
    fs_close_diff()
    fs_show_diff(str)
    fs_begin_edit()
    fs_cancel_edit()
    fs_save_edit()
  col w=fill h=fill
    // THE CRUMB BAR, not a screen header: where you are, what is here,
    // and who may write under it. The counts are pure folds over the
    // listing already on screen — never a second `files_ls` — and they
    // go silent with the node down rather than folding an unfetched
    // listing into `0 files · 0 dirs`.
    CrumbBar
      with
        path
        meta=fs_counts_summary(connected, entries)
      forward
        fs_open_dir
    // WHERE THE WRITE CONTROLS LIVE — decided here, once. The artifact's
    // Files screen is a read-only browser, but this app ships a working
    // mkdir / new file / delete / edit and dropping them would be a
    // regression. They sit in ONE bar under the header, never as per-row
    // hover affordances, so the three panes below stay the artifact's read
    // surface and the destructive verb always names the selected object.
    box
      with
        w=fill
        pl=20.0
        pr=20.0
        pt=10.0
        pb=10.0
      row
        with
          w=fill
          h=28.0
          gap=8.0
          align=center
        button "↑" -> emit(fs_open_parent)
          with
            label="Parent directory"
            disabled=(loading || empty(path))
            w=26.0
            h=26.0
            p=0.0
            @icon_action
          active bg=surface text=muted border=card_line border-w=1.0 r=7.0
          hovered bg=elevated text=fg
          pressed bg=subtle
        input "" #fs-new <-> new_name
          with
            label="New entry name"
            change=emit(fs_new_name_changed, _)
            hint="new name…"
            disabled=loading
            w=160.0
            p=5.0
            text-size=13.0
            line-h=1.2
            @control
          active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=7.0
          hovered bg=muted_bg border=control_line
          disabled bg=muted_bg/54 value=muted
        button "+ Folder" -> emit(fs_mkdir_submit)
          with
            disabled=(loading || empty(trim(new_name)))
            h=26.0
            p=5.0
            @secondary_action
        button "+ File" -> emit(fs_new_file_submit)
          with
            disabled=(loading || empty(trim(new_name)))
            h=26.0
            p=5.0
            @secondary_action
        space w=fill
        if loading
          text "Loading…"
            with
              size=12.5
              wrap=none
              @text-caption
        // The trigger STAYS a trigger: arming opens the named confirm
        // dialog below instead of morphing into the red button in place.
        if !empty(preview_path)
          button "Delete object" -> emit(fs_arm_delete, preview_path)
            with
              disabled=(loading || !empty(delete_target))
              h=26.0
              p=5.0
              @secondary_action
            active bg=transparent text=muted border=card_line border-w=1.0 r=7.0
            hovered bg=danger_zone_bg text=fg border=danger_zone_line
            pressed bg=danger_zone_bg
        overlay
          with
            when=(!empty(delete_target))
            dismiss=emit(fs_disarm_delete)
            backdrop=scrim
            p=30.0
            align-x=center
            align-y=center
          content
            space w=fill h=fill
          layer
            ConfirmDelete
              with
                title="Delete this object"
                subject=delete_target
                note="The committed object is removed from duckfs for every member. Earlier snapshots keep their copies."
                action="Delete object"
                busy=loading
              events
                cancel -> emit(fs_disarm_delete)
                confirm -> emit(fs_delete_submit)
        button "History" -> emit(fs_toggle_history)
          with
            h=26.0
            p=5.0
            @secondary_action
          active bg=surface text=muted border=card_line border-w=1.0 r=7.0
          hovered bg=elevated text=fg
          pressed bg=subtle
    box
      with
        w=fill
        h=1.0
        bg=separator
      space w=1.0 h=1.0
    row w=fill h=fill
      // 206px directory pane. `files_ls` loads one level at a time, so this
      // is the current level's directories, not a recursively expanded tree
      // — depth stays 0 until a per-level expansion state exists.
      box
        with
          w=206.0
          h=fill
          bg=sidebar
          clip=true
        col w=fill h=fill
          // The artifact's A1 header. Without it a level with no
          // subdirectories rendered a blank 206px column that reads as a
          // broken pane rather than an empty one. The subtitle states
          // what duckfs IS and needs no reading to back it.
          //
          // 50px like every other pane header, and like `ObjectTableHeader`
          // across the separator: padding-sized, the two-line title stood 57
          // tall against the table header's 30, so the two rules that meet at
          // this seam were 27px apart.
          box
            with
              w=fill
              h=50.0
              pl=14.0
              pr=14.0
              align-y=center
            col w=fill gap=2.0
              text "duckfs"
                with
                  size=13.5
                  wrap=none
                  font=display
                  @text-fg
              text "content-addressed · replicated"
                with
                  size=9.5
                  wrap=none
                  font=code
                  @text-hint
          box
            with
              w=fill
              h=1.0
              bg=separator
            space w=1.0 h=1.0
          scroll
            with
              dir=vertical
              w=fill
              h=fill
              bar=hidden
            col
              with
                w=fill
                pl=6.0
                pr=6.0
                pt=8.0
                pb=8.0
                gap=1.0
              // "No folders here." is a reading of a listing; with the node
              // down there is no listing, and the main pane already says so.
              if connected && fs_dir_count(entries) <= 0
                box
                  with
                    w=fill
                    pl=12.0
                    pr=12.0
                    pt=6.0
                    pb=6.0
                  text "No folders here." size=11.0 @text-hint
              for entry in entries
                if entry.kind == "dir"
                  FsTreeRow
                    with
                      entry
                      selected=false
                      depth=0.0
                    forward
                      fs_open_dir
      box
        with
          w=1.0
          h=fill
          bg=separator
        space w=1.0 h=1.0
      col w=fill h=fill
        // NOT CONNECTED IS NOT EMPTY. The listing and the snapshot log both
        // arrive over the node; with it down this pane used to plate "Empty
        // directory — nothing is committed under this path.", which is a claim
        // about CONTENT made from a request that never went out. Same words
        // Chat and Pages use, so the six data screens read as one app. The
        // crumb bar and the write bar above stay — they are how the reader
        // gets back out.
        if !connected
          box w=fill h=fill p=22.0
            EmptyState
              with
                title="Not connected"
                description="Click the network name in the titlebar to pick or reconnect a network."
        if connected && history_open
          scroll
            with
              dir=vertical
              w=fill
              h=fill
            col
              with
                w=fill
                p=18.0
                gap=8.0
              if !empty(diff_from)
                col w=fill gap=6.0
                  row
                    with
                      w=fill
                      gap=8.0
                      align=center
                    GroupLabel label="CHANGES VS HEAD"
                    space w=fill
                    button "Back" -> emit(fs_close_diff)
                      with
                        h=22.0
                        p=4.0
                        @secondary_action
                      active bg=surface text=muted border=card_line border-w=1.0 r=6.0
                      hovered bg=elevated text=fg
                      pressed bg=subtle
                  if empty(diff)
                    text "No differences." size=12.5 @text-caption
                  for entry in diff
                    row
                      with
                        w=fill
                        gap=8.0
                        align=center
                      text entry.kind
                        with
                          w=64.0
                          size=12.0
                          wrap=none
                          font=code
                          @text-meta
                      text entry.path
                        with
                          w=fill
                          size=12.0
                          wrap=none
                          font=code
                          @text-fg
              if empty(diff_from)
                col w=fill gap=8.0
                  // The eyebrow labels a list, so it only earns its place once
                  // there is one: hung over nothing it reads as a load that
                  // failed. Same trade as the "No differences." arm above.
                  if !empty(history)
                    GroupLabel label="SNAPSHOTS"
                  if empty(history)
                    text "No snapshots yet." size=12.5 @text-caption
                  for snapshot in history
                    box
                      with
                        w=fill
                        p=11.0
                        bg=surface
                        border=card_line
                        border-w=1.0
                        r=10.0
                      col w=fill gap=3.0
                        row
                          with
                            w=fill
                            gap=8.0
                            align=center
                          text snapshot.short_id
                            with
                              size=12.0
                              wrap=none
                              font=code
                              @text-fg
                          text height_label(snapshot.height)
                            with
                              size=12.0
                              wrap=none
                              font=code
                              @text-meta
                          space w=fill
                          text snapshot.author
                            with
                              size=12.0
                              wrap=none
                              font=code
                              @text-meta
                          button "Diff" -> emit(fs_show_diff, snapshot.id)
                            with
                              h=20.0
                              p=3.0
                              @ghost_action
                            active bg=surface text=muted border=card_line border-w=1.0 r=6.0
                            hovered bg=elevated text=fg
                            pressed bg=subtle
                        if !empty(snapshot.message)
                          text snapshot.message size=13.5 @text-fg
        if connected && !history_open
          col w=fill h=fill
            ObjectTableHeader
            if empty(entries) && !loading
              box w=fill p=22.0
                EmptyPlate message="Empty directory — nothing is committed under this path."
            if !empty(entries)
              scroll
                with
                  dir=vertical
                  w=fill
                  h=fill
                col w=fill
                  for entry in entries
                    ObjectRow entry=entry selected=(entry.path == preview_path)
                      forward
                        fs_open_dir
                        fs_open_file
            if !empty(preview_path)
              col w=fill h=300.0
                box
                  with
                    w=fill
                    h=1.0
                    bg=separator
                  space w=1.0 h=1.0
                box
                  with
                    w=fill
                    h=fill
                    p=16.0
                  col
                    with
                      w=fill
                      h=fill
                      gap=8.0
                    row
                      with
                        w=fill
                        gap=8.0
                        align=center
                      text preview_path
                        with
                          w=fill
                          size=12.0
                          wrap=none
                          font=code
                          @text-meta
                      if preview_truncated
                        text "first 64 KiB"
                          with
                            size=12.5
                            wrap=none
                            @text-caption
                      if !preview_binary && !editing && !preview_truncated
                        button "Edit" -> emit(fs_begin_edit)
                          with
                            h=22.0
                            p=4.0
                            @secondary_action
                          active bg=surface text=muted border=card_line border-w=1.0 r=6.0
                          hovered bg=elevated text=fg
                          pressed bg=subtle
                      if editing
                        button "Cancel" -> emit(fs_cancel_edit)
                          with
                            h=22.0
                            p=4.0
                            @secondary_action
                          active bg=surface text=muted border=card_line border-w=1.0 r=6.0
                          hovered bg=elevated text=fg
                          pressed bg=subtle
                      if editing
                        button "Save" -> emit(fs_save_edit)
                          with
                            disabled=loading
                            h=22.0
                            p=4.0
                            @primary_action
                    stack w=fill h=fill
                      if editing
                        editor #fs-editor <-> draft
                          with
                            hint="File contents…"
                            disabled=loading
                            min-h=200.0
                            size=12.0
                            line-h=1.3
                            p=6.6
                            wrap=word
                          active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                          hovered bg=muted_bg border=control_line
                          focused bg=muted_bg border=ring border-w=1.0
                      if !editing
                        scroll
                          with
                            dir=vertical
                            w=fill
                            h=fill
                          col w=fill gap=6.0
                            if preview_binary
                              text preview_text
                                with
                                  size=12.0
                                  font=code
                                  @text-meta
                            if !preview_binary
                              text preview_text
                                with
                                  size=12.0
                                  font=code
                                  @text-fg
      if !empty(preview_path)
        for entry in entries
          if entry.path == preview_path
            // `changed_by` / `changed_height` are fed the "not answered"
            // pair on purpose. `last_changed_at_path` exists and the
            // panel gates its rows on `changed_height > 0`, but the
            // walk has to fire when the selected path CHANGES, and the
            // only place that can watch a state field is the one
            // `subscribe` block in handlers/lifecycle.ice — which this
            // file does not own. Wiring the load there lights these two
            // rows with no change here.
            ObjectPanel
              with
                entry
                changed_by=""
                changed_height=0

component ExplorerScreen(bind query:str, connected:bool, searching:bool, loading:bool, kinds:[KindCount], kind:str, hits:[ExplorerHit], blocks:[ExplorerBlock], selected:i64, ops:[ExplorerOp])
  emits
    explorer_search_submit()
    clear_explorer_search()
    refresh_explorer()
    pick_explorer_kind(str)
    select_explorer_block(i64)
  col w=fill h=fill
    col
      with
        w=fill
        pl=24.0
        pr=24.0
        pt=22.0
        gap=16.0
      ScreenTitle
        with
          title="Explorer"
          detail="Search everything this workspace has recorded, or read the blocks this node verified for itself — newest first, each one openable for the ops it carried."
      // THE QUERY BOX, on the artifact's own 1.5px ink outline.
      box w=fill max-w=860.0
        row
          with
            w=fill
            gap=10.0
            align=center
          box
            with
              w=fill
              pl=14.0
              pr=14.0
              pt=2.0
              pb=2.0
              bg=surface
              border=primary
              border-w=1.5
              r=11.0
            row
              with
                w=fill
                gap=10.0
                align=center
              Icon
                with
                  name="search"
                  tone="label"
                  px=16.0
              input "" #explorer-search <-> query
                with
                  label="Search this workspace"
                  hint="Search messages, pages, issues, files, runs…"
                  disabled=(!connected || searching)
                  submit=emit(explorer_search_submit)
                  w=fill
                  p=6.2
                  text-size=13.0
                  line-h=1.2
                  @control
                active bg=transparent border=transparent value=fg placeholder=hint selection=fg/18 border-w=0.0 r=0.0
                hovered bg=transparent border=transparent
                disabled value=muted
              if !empty(trim(query))
                button -> emit(clear_explorer_search)
                  with
                    label="Clear workspace search"
                    w=22.0
                    h=22.0
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
                  active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle text=fg
          if searching
            text "Searching…"
              with
                size=12.5
                wrap=none
                @text-caption
          if loading
            text "Loading…"
              with
                size=12.5
                wrap=none
                @text-caption
          button "Refresh" -> emit(refresh_explorer)
            with
              disabled=loading
              h=30.0
              p=7.0
              @outline_action
      // THE KIND STRIP. Drawn FROM the reply, never from a fixed list
      // of labels: every chip here names a kind `search_workspace`
      // genuinely ran, so a count of 0 means "nothing matched", never
      // "no loader". TASKS IS BACK — it was cut for having no text
      // search, but the tasks index answers by status and the filtering
      // is ours to do, which is the same deal every other kind takes.
      // The kind filter itself is client-side over hits already in
      // hand: a second round trip to narrow a list you are holding is
      // waste.
      if !empty(kinds)
        box w=fill max-w=860.0
          flex
            with
              w=fill
              wrap=wrap
              gap-x=7.0
              gap-y=7.0
              items=start
            button -> emit(pick_explorer_kind, "all")
              with
                label="Show every result"
                p=0.0
                @ghost_action
              FilterChip
                with
                  label="All"
                  count=len(hits)
                  selected=(kind == "all")
              active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
              hovered bg=row_hover text=fg
              pressed bg=elevated text=fg
            for kind_count in kinds
              button -> emit(pick_explorer_kind, kind_count.kind)
                with
                  label="Filter results by kind"
                  description=kind_count.label
                  p=0.0
                  @ghost_action
                FilterChip
                  with
                    label=kind_count.label
                    count=kind_count.count
                    selected=(kind == kind_count.kind)
                active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                hovered bg=row_hover text=fg
                pressed bg=elevated text=fg
    // 24 horizontally, matching the head above it — at `p=18` the ledger and
    // every card in it stood 6px left of the query box they answer.
    col
      with
        w=fill
        h=fill
        pl=24.0
        pr=24.0
        pt=18.0
        pb=18.0
        gap=11.0
      // RESULTS TAKE THE SCREEN while a query stands; the block ledger
      // is what the screen falls back to. A hit is a READING, not a
      // route: nothing here dispatches on `hit.target` yet, so the card
      // is not wrapped in a button that would go nowhere.
      if !empty(hits)
        scroll
          with
            dir=vertical
            w=fill
            h=fill
          box w=fill max-w=860.0
            col w=fill gap=8.0
              for hit in hits
                if kind == "all" || hit.kind == kind
                  ExplorerCard hit=hit
      // A chip whose count is 0 is still selectable — the artifact draws
      // every kind — so the pane it opens says so instead of going
      // blank. The count is read back off the same strip the chip came
      // from; there is no second source to disagree with.
      if !empty(hits)
        for kind_count in kinds
          if kind == kind_count.kind && kind_count.count <= 0
            EmptyPlate message="Nothing of that kind matched — the other chips still hold results."
      if empty(hits) && !searching && !empty(trim(query))
        EmptyPlate message="Nothing matched that query in this workspace."
      // NOT CONNECTED IS NOT EMPTY. `connected` already disables the query box
      // above; the ledger below it still asserted "No blocks yet" off a node
      // that answered nothing. The head and the query box stay.
      if !connected
        EmptyState
          with
            title="Not connected"
            description="Click the network name in the titlebar to pick or reconnect a network."
      if connected && empty(hits) && empty(blocks) && !loading && empty(trim(query))
        EmptyState
          with
            title="No blocks yet"
            description="Non-empty blocks appear here as they finalize."
      if empty(hits) && !empty(blocks)
        row
          with
            w=fill
            h=fill
            gap=10.0
          box
            with
              w=340.0
              h=fill
              p=6.0
              bg=muted_bg
              border=fg/10
              border-w=1.0
              r=10.0
            scroll
              with
                dir=vertical
                w=fill
                h=fill
              // `pr` is the scrollbar's gutter: the bar paints OVER the
              // content, and without it the op count on every row was
              // clipped by the track.
              col
                with
                  w=fill
                  pr=10.0
                  gap=1.0
                for block in blocks
                  ExplorerBlockRow
                    with
                      block
                      selected=(block.height == selected)
                    forward
                      select_explorer_block
          box
            with
              w=fill
              h=fill
              p=8.0
              bg=muted_bg
              border=fg/10
              border-w=1.0
              r=10.0
            stack w=fill h=fill
              if selected <= 0
                EmptyState
                  with
                    title="Select a block"
                    description="Its operations and dispatch traces appear here."
              if selected > 0
                scroll
                  with
                    dir=vertical
                    w=fill
                    h=fill
                  col w=fill gap=6.0
                    for op in explorer_ops_at(ops, selected)
                      box
                        with
                          w=fill
                          p=8.0
                          bg=surface
                          border=fg/10
                          border-w=1.0
                          r=9.0
                        col w=fill gap=3.0
                          row
                            with
                              w=fill
                              gap=8.0
                              align=center
                            text op.target
                              with
                                size=14.0
                                wrap=none
                                font=display
                                @text-fg
                            StatusBadge label=op.disposition
                            space w=fill
                            text op.op_hash
                              with
                                size=12.0
                                wrap=none
                                font=code
                                @text-muted
                          row
                            with
                              w=fill
                              gap=8.0
                              align=center
                            text "by"
                              with
                                size=11.0
                                wrap=none
                                font=code_medium
                                @text-muted
                            text op.proposer
                              with
                                size=12.0
                                wrap=none
                                font=code
                                @text-muted
                          if !empty(op.trace)
                            text op.trace
                              with
                                size=12.0
                                font=code
                                @text-muted
                          text op.payload size=13.5 @text-fg

// ONE BLOCK IN THE CHAIN LIST, AND WHETHER IT IS THE ONE YOU OPENED. The row
// carried no selected state at all: clicking filled the detail pane on the
// right and left the list identical to the pixel, measured — the only visible
// difference was `hovered`, which follows the pointer, not the selection. In a
// list where every row is a height and a truncated hash there is no landmark to
// re-find your place by, so losing the mark means reading hex until it matches.
//
// `tree_selected` is the token the Forge file tree already uses for exactly
// this, and it is the one NAMED for it. Note the app has not settled here: the
// Files tree reaches for `subtle` instead, a plate 2.3/255 away in dark — two
// tokens, one meaning. Unifying them touches two other screens and is left for
// a design pass rather than smuggled in behind an Explorer fix.
//
// The plate is LIGHTER than the surface, which is the direction every selected
// row in this app moves; the measured lift here is +13/255, against the chat
// sidebar's +19 for its active channel.
component ExplorerBlockRow(block:ExplorerBlock, selected:bool)
  emits
    select_explorer_block(i64)
  col #root w=fill
    if selected
      button -> emit(select_explorer_block, block.height)
        with
          label="Inspect block"
          w=fill
          p=6.0
          @ghost_action
        ExplorerBlockFace block=block
        active bg=tree_selected text=fg border=transparent border-w=1.0 r=7.0
        hovered bg=row_hover text=fg
        pressed bg=accent
    if !selected
      button -> emit(select_explorer_block, block.height)
        with
          label="Inspect block"
          w=fill
          p=6.0
          @ghost_action
        ExplorerBlockFace block=block
        active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
        hovered bg=row_hover text=fg
        pressed bg=accent

// The row's three columns, in one place so the two plates cannot drift apart.
component ExplorerBlockFace(block:ExplorerBlock)
  row #root
    with
      w=fill
      h=fill
      gap=8.0
      align=center
    text block.height
      with
        size=12.0
        wrap=none
        font=code
        @text-fg
    text block.hash
      with
        w=fill
        size=12.0
        wrap=none
        font=code
        @text-muted
    // `1 op` / `3 ops`, not a bare `1`. The three columns carry no header, and
    // of the three only this one is unreadable without one — a height and a
    // hash say what they are. Labelling the VALUE beats a header row here: the
    // height column is variable width, so any header would need a magic fixed
    // width that a seven-digit chain outgrows.
    text plural(block.op_count, "op", "ops")
      with
        size=12.0
        wrap=none
        font=code
        @text-muted
