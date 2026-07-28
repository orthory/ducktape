// THE TWO STORAGE SCREENS: what this workspace has stored under duckfs, and
// everything it has recorded anywhere. Files is the three-pane duckfs browser
// plus the write bar; Explorer is workspace search over the block ledger it
// falls back to.
//
// See `screens/roster.ice` for the screen contract: a screen is a component, so
// it cannot reach app state — every reading arrives as a prop and every act
// leaves as a named event that `view.ice` routes back to the handler of the
// same name.

component FilesScreen(path:str, entries:[FsEntry], loading:bool, bind new_name:str, preview_path:str, delete_target:str, history_open:bool, diff_from:str, diff:[FsDiffEntry], history:[FsSnapshot], preview_truncated:bool, preview_binary:bool, editing:bool, bind draft:editor, preview_text:str)
  emits
    fs_open_dir(str)
    fs_open_file(str)
    fs_open_parent()
    fs_new_name_changed(str)
    fs_mkdir_submit()
    fs_new_file_submit()
    fs_arm_delete(str)
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
    // listing already on screen — never a second `files_ls`.
    CrumbBar path=path dirs=fs_dir_count(entries) files=fs_file_count(entries)
      forward
        fs_open_dir
    // WHERE THE WRITE CONTROLS LIVE — decided here, once. The artifact's
    // Files screen is a read-only browser, but this app ships a working
    // mkdir / new file / delete / edit and dropping them would be a
    // regression. They sit in ONE bar under the header, never as per-row
    // hover affordances, so the three panes below stay the artifact's read
    // surface and the destructive verb always names the selected object.
    box w=fill pl=20.0 pr=20.0 pt=10.0 pb=10.0
      row w=fill h=28.0 gap=8.0 align=center
        button "↑" label="Parent directory" disabled=(loading || empty(path)) w=26.0 h=26.0 p=0.0 @icon_action -> emit(fs_open_parent)
          active bg=surface text=muted border=card_line border-w=1.0 r=7.0
          hovered bg=elevated text=fg
          pressed bg=subtle
        input "" #fs-new label="New entry name" <-> new_name change=emit(fs_new_name_changed, _) hint="new name…" disabled=loading w=160.0 p=5.0 text-size=13.0 line-h=1.2 @control
          active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=7.0
          hovered bg=muted_bg border=control_line
          disabled bg=muted_bg/54 value=muted
        button "+ Folder" disabled=(loading || empty(trim(new_name))) h=26.0 p=5.0 @secondary_action -> emit(fs_mkdir_submit)
        button "+ File" disabled=(loading || empty(trim(new_name))) h=26.0 p=5.0 @secondary_action -> emit(fs_new_file_submit)
        space w=fill
        if loading
          text "Loading…" size=12.5 wrap=none @text-caption
        if !empty(preview_path) && preview_path != delete_target
          button "Delete object" disabled=loading h=26.0 p=5.0 @secondary_action -> emit(fs_arm_delete, preview_path)
            active bg=transparent text=muted border=card_line border-w=1.0 r=7.0
            hovered bg=danger_zone_bg text=fg border=danger_zone_line
            pressed bg=danger_zone_bg
        if !empty(preview_path) && preview_path == delete_target
          button "Delete for real" disabled=loading h=26.0 p=5.0 @danger_action -> emit(fs_delete_submit)
        button "History" h=26.0 p=5.0 @secondary_action -> emit(fs_toggle_history)
          active bg=surface text=muted border=card_line border-w=1.0 r=7.0
          hovered bg=elevated text=fg
          pressed bg=subtle
    box w=fill h=1.0 bg=separator
      space w=1.0 h=1.0
    row w=fill h=fill
      // 206px directory pane. `files_ls` loads one level at a time, so this
      // is the current level's directories, not a recursively expanded tree
      // — depth stays 0 until a per-level expansion state exists.
      box w=206.0 h=fill bg=sidebar clip=true
        col w=fill h=fill
          // The artifact's A1 header. Without it a level with no
          // subdirectories rendered a blank 206px column that reads as a
          // broken pane rather than an empty one. The subtitle states
          // what duckfs IS and needs no reading to back it.
          box w=fill pl=14.0 pr=14.0 pt=14.0 pb=11.0
            col w=fill gap=2.0
              text "duckfs" size=13.5 wrap=none font=display @text-fg
              text "content-addressed · replicated" size=9.5 wrap=none font=code @text-hint
          box w=fill h=1.0 bg=separator
            space w=1.0 h=1.0
          scroll dir=vertical w=fill h=fill bar=hidden
            col w=fill pl=6.0 pr=6.0 pt=8.0 pb=8.0 gap=1.0
              if fs_dir_count(entries) <= 0
                box w=fill pl=12.0 pr=12.0 pt=6.0 pb=6.0
                  text "No folders here." size=11.0 @text-hint
              for entry in entries
                if entry.kind == "dir"
                  FsTreeRow entry=entry selected=false depth=0.0
                    forward
                      fs_open_dir
      box w=1.0 h=fill bg=separator
        space w=1.0 h=1.0
      col w=fill h=fill
        if history_open
          scroll dir=vertical w=fill h=fill
            col w=fill p=18.0 gap=8.0
              if !empty(diff_from)
                col w=fill gap=6.0
                  row w=fill gap=8.0 align=center
                    GroupLabel label="CHANGES VS HEAD"
                    space w=fill
                    button "Back" h=22.0 p=4.0 @secondary_action -> emit(fs_close_diff)
                      active bg=surface text=muted border=card_line border-w=1.0 r=6.0
                      hovered bg=elevated text=fg
                      pressed bg=subtle
                  if empty(diff)
                    text "No differences." size=12.5 @text-caption
                  for entry in diff
                    row w=fill gap=8.0 align=center
                      text entry.kind w=64.0 size=12.0 wrap=none font=code @text-meta
                      text entry.path w=fill size=12.0 wrap=none font=code @text-fg
              if empty(diff_from)
                col w=fill gap=8.0
                  GroupLabel label="SNAPSHOTS"
                  for snapshot in history
                    box w=fill p=11.0 bg=surface border=card_line border-w=1.0 r=10.0
                      col w=fill gap=3.0
                        row w=fill gap=8.0 align=center
                          text snapshot.short_id size=12.0 wrap=none font=code @text-fg
                          text height_label(snapshot.height) size=12.0 wrap=none font=code @text-meta
                          space w=fill
                          text snapshot.author size=12.0 wrap=none font=code @text-meta
                          button "Diff" h=20.0 p=3.0 @ghost_action -> emit(fs_show_diff, snapshot.id)
                            active bg=surface text=muted border=card_line border-w=1.0 r=6.0
                            hovered bg=elevated text=fg
                            pressed bg=subtle
                        if !empty(snapshot.message)
                          text snapshot.message size=13.5 @text-fg
        if !history_open
          col w=fill h=fill
            ObjectTableHeader
            if empty(entries) && !loading
              box w=fill p=22.0
                EmptyPlate message="Empty directory — nothing is committed under this path."
            if !empty(entries)
              scroll dir=vertical w=fill h=fill
                col w=fill
                  for entry in entries
                    ObjectRow entry=entry selected=(entry.path == preview_path)
                      forward
                        fs_open_dir
                        fs_open_file
            if !empty(preview_path)
              col w=fill h=300.0
                box w=fill h=1.0 bg=separator
                  space w=1.0 h=1.0
                box w=fill h=fill p=16.0
                  col w=fill h=fill gap=8.0
                    row w=fill gap=8.0 align=center
                      text preview_path w=fill size=12.0 wrap=none font=code @text-meta
                      if preview_truncated
                        text "first 64 KiB" size=12.5 wrap=none @text-caption
                      if !preview_binary && !editing && !preview_truncated
                        button "Edit" h=22.0 p=4.0 @secondary_action -> emit(fs_begin_edit)
                          active bg=surface text=muted border=card_line border-w=1.0 r=6.0
                          hovered bg=elevated text=fg
                          pressed bg=subtle
                      if editing
                        button "Cancel" h=22.0 p=4.0 @secondary_action -> emit(fs_cancel_edit)
                          active bg=surface text=muted border=card_line border-w=1.0 r=6.0
                          hovered bg=elevated text=fg
                          pressed bg=subtle
                      if editing
                        button "Save" disabled=loading h=22.0 p=4.0 @primary_action -> emit(fs_save_edit)
                    stack w=fill h=fill
                      if editing
                        editor #fs-editor <-> draft hint="File contents…" disabled=loading min-h=200.0 size=12.0 line-h=1.3 p=6.6 wrap=word
                          active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                          hovered bg=muted_bg border=control_line
                          focused bg=muted_bg border=ring border-w=1.0
                      if !editing
                        scroll dir=vertical w=fill h=fill
                          col w=fill gap=6.0
                            if preview_binary
                              text preview_text size=12.0 font=code @text-meta
                            if !preview_binary
                              text preview_text size=12.0 font=code @text-fg
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
            ObjectPanel entry=entry changed_by="" changed_height=0

component ExplorerScreen(bind query:str, connected:bool, searching:bool, loading:bool, kinds:[KindCount], kind:str, hits:[ExplorerHit], blocks:[ExplorerBlock], selected:i64, ops:[ExplorerOp])
  emits
    explorer_search_submit()
    clear_explorer_search()
    refresh_explorer()
    pick_explorer_kind(str)
    select_explorer_block(i64)
  col w=fill h=fill
    col w=fill pl=24.0 pr=24.0 pt=22.0 gap=16.0
      ScreenTitle title="Explorer" detail="Search everything this workspace has recorded, or read the blocks this node verified for itself — newest first, each one openable for the ops it carried."
      // THE QUERY BOX, on the artifact's own 1.5px ink outline.
      box w=fill max-w=860.0
        row w=fill gap=10.0 align=center
          box w=fill pl=14.0 pr=14.0 pt=2.0 pb=2.0 bg=surface border=primary border-w=1.5 r=11.0
            row w=fill gap=10.0 align=center
              Icon name="search" tone="label" px=16.0
              input "" #explorer-search label="Search this workspace" <-> query hint="Search messages, pages, issues, files, runs…" disabled=(!connected || searching) submit=emit(explorer_search_submit) w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                active bg=transparent border=transparent value=fg placeholder=hint selection=fg/18 border-w=0.0 r=0.0
                hovered bg=transparent border=transparent
                disabled value=muted
              if !empty(trim(query))
                button label="Clear workspace search" w=22.0 h=22.0 p=0.0 @icon_action -> emit(clear_explorer_search)
                  box w=fill h=fill align-x=center align-y=center
                    text "×" size=14.0 wrap=none @text-muted
                  active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle text=fg
          if searching
            text "Searching…" size=12.5 wrap=none @text-caption
          if loading
            text "Loading…" size=12.5 wrap=none @text-caption
          button "Refresh" disabled=loading h=30.0 p=7.0 @outline_action -> emit(refresh_explorer)
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
          flex w=fill wrap=wrap gap-x=7.0 gap-y=7.0 items=start
            button label="Show every result" p=0.0 @ghost_action -> emit(pick_explorer_kind, "all")
              FilterChip label="All" count=len(hits) selected=(kind == "all")
              active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
              hovered bg=row_hover text=fg
              pressed bg=elevated text=fg
            for kind_count in kinds
              button label="Filter results by kind" description=kind_count.label p=0.0 @ghost_action -> emit(pick_explorer_kind, kind_count.kind)
                FilterChip label=kind_count.label count=kind_count.count selected=(kind == kind_count.kind)
                active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                hovered bg=row_hover text=fg
                pressed bg=elevated text=fg
    col w=fill h=fill p=18.0 gap=11.0
      // RESULTS TAKE THE SCREEN while a query stands; the block ledger
      // is what the screen falls back to. A hit is a READING, not a
      // route: nothing here dispatches on `hit.target` yet, so the card
      // is not wrapped in a button that would go nowhere.
      if !empty(hits)
        scroll dir=vertical w=fill h=fill
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
      if empty(hits) && empty(blocks) && !loading && empty(trim(query))
        EmptyState title="No blocks yet" description="Non-empty blocks appear here as they finalize."
      if empty(hits) && !empty(blocks)
        row w=fill h=fill gap=10.0
          box w=340.0 h=fill p=6.0 bg=muted_bg border=fg/10 border-w=1.0 r=10.0
            scroll dir=vertical w=fill h=fill
              // `pr` is the scrollbar's gutter: the bar paints OVER the
              // content, and without it the op count on every row was
              // clipped by the track.
              col w=fill pr=10.0 gap=1.0
                for block in blocks
                  button label="Inspect block" w=fill p=6.0 @ghost_action -> emit(select_explorer_block, block.height)
                    row w=fill h=fill gap=8.0 align=center
                      text block.height size=12.0 wrap=none font=code @text-fg
                      text block.hash w=fill size=12.0 wrap=none font=code @text-muted
                      text block.op_count size=12.0 wrap=none font=code @text-muted
                    active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
                    hovered bg=row_hover text=fg
                    pressed bg=accent
          box w=fill h=fill p=8.0 bg=muted_bg border=fg/10 border-w=1.0 r=10.0
            stack w=fill h=fill
              if selected <= 0
                EmptyState title="Select a block" description="Its operations and dispatch traces appear here."
              if selected > 0
                scroll dir=vertical w=fill h=fill
                  col w=fill gap=6.0
                    for op in explorer_ops_at(ops, selected)
                      box w=fill p=8.0 bg=surface border=fg/10 border-w=1.0 r=9.0
                        col w=fill gap=3.0
                          row w=fill gap=8.0 align=center
                            text op.target size=14.0 wrap=none font=display @text-fg
                            StatusBadge label=op.disposition
                            space w=fill
                            text op.op_hash size=12.0 wrap=none font=code @text-muted
                          row w=fill gap=8.0 align=center
                            text "by" size=11.0 wrap=none font=code_medium @text-muted
                            text op.proposer size=12.0 wrap=none font=code @text-muted
                          if !empty(op.trace)
                            text op.trace size=12.0 font=code @text-muted
                          text op.payload size=13.5 @text-fg
