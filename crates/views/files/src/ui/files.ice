// THE FILES SCREEN: the three-pane duckfs browser plus the write bar, as the
// desktop app drew it natively (screens/storage.ice before the port). Shared
// readings arrive as props, interaction-local state stays here, and only
// application effects leave as named events the view root turns into intents.

component FilesScreen(path:str, listed:bool, entries:[FsEntry], directories:[FsEntry], connected:bool, loading:bool, bind new_name:str, preview_path:str, preview_entry:FsEntry, delete_target:str, diff_from:str, diff:[FsDiffEntry], history:[FsSnapshot], preview_truncated:bool, preview_binary:bool, editing:bool, bind draft:editor, preview_text:str, dark:bool, preview_picture:bool, preview_width:i64, preview_height:i64, write_refusal:str)
  lifetime retained
  emits
    open_message_link(str)
    fs_open_dir(str)
    fs_open_file(str)
    fs_open_parent()
    fs_mkdir_submit()
    fs_new_file_submit()
    fs_arm_delete(str)
    fs_disarm_delete()
    fs_delete_submit()
    fs_close_diff()
    fs_show_diff(str)
    fs_begin_edit()
    fs_cancel_edit()
    fs_save_edit()
  state
    history_open = false
  on fs_toggle_history
    history_open = !history_open
  col w=fill h=fill
    // THE CRUMB BAR, not a screen header: where you are, what is here,
    // and who may write under it. The counts are pure folds over the
    // listing already on screen — never a second `files_ls` — and they
    // go silent rather than counting a listing that is not this path's:
    // with the node down nobody asked, and mid-navigation the rows still
    // belong to the directory you left. The crumb has already moved, so
    // `0 files · 1 dir` beside it would be the OLD directory's tally
    // printed under the NEW directory's name.
    CrumbBar #crumb path meta=fs_counts_summary(connected, listed, entries)
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
        button -> emit(fs_open_parent)
          with
            label="Parent directory"
            disabled=(loading || path == "/")
            w=26.0
            h=26.0
            p=0.0
            @icon_action
          text "↑" size=12.5 font=ui
          active bg=surface text=muted border=card_line border-w=1.0 r=7.0
          hovered bg=elevated text=fg
          pressed bg=subtle
        input "" #fs-new <-> new_name
          with
            label="New entry name"
            hint="new name…"
            disabled=loading
            w=160.0
            p=5.0
            text-size=13.0
            line-h=1.2
            @control
          active bg=surface value=fg placeholder=hint selection=fg/18 border-w=1.0 r=7.0
          hovered bg=muted_bg border=control_line
          disabled bg=muted_bg/54 value=muted
        button "+ Folder" -> emit(fs_mkdir_submit)
          with
            disabled=(loading || empty(trim(new_name)) || !empty(write_refusal))
            h=26.0
            p=5.0
            @secondary_action
        button "+ File" -> emit(fs_new_file_submit)
          with
            disabled=(loading || empty(trim(new_name)) || !empty(write_refusal))
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
        button "History" -> fs_toggle_history
          with
            expanded=history_open
            h=26.0
            p=5.0
            @secondary_action
          active bg=surface text=muted border=card_line border-w=1.0 r=7.0
          hovered bg=elevated text=fg
          pressed bg=subtle
    // THE MODULE'S OWN ANSWER, BEFORE THE ROUND TRIP: a root, or another
    // member's home, says under the bar why nothing can be written here.
    // Its words name whole keys, so the line wraps below the fixed-height
    // bar instead of riding in it.
    if !empty(write_refusal)
      box
        with
          w=fill
          pl=20.0
          pr=20.0
          pb=10.0
        text write_refusal
          with
            size=12.5
            w=fill
            wrap=word
            @text-caption
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
              // "No folders here." is a reading of a listing; without one for
              // THIS path there is nothing to read — the node is down, or the
              // rows still describe the directory you just left — and the main
              // pane already says so.
              if connected && listed && empty(directories)
                box
                  with
                    w=fill
                    pl=12.0
                    pr=12.0
                    pt=6.0
                    pb=6.0
                  text "No folders here." size=11.0 @text-hint
              // The tree itself is the same reading. Its empty case stood down
              // above and the listing beside it did not, so the sidebar kept
              // drawing folders from a duckfs nobody could reach — and, after
              // a navigation, folders that live somewhere else.
              if connected && listed
                keyed entry in directories by=entry.key virtual-row=27.0 w=fill
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
          box
            with
              w=fill
              h=fill
              p=22.0
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
            // Both arms read the listing, so both wait for one that belongs to
            // the path in the crumb. Until it lands the pane holds only its
            // header — the write bar above carries the "Loading…" word, and a
            // plate that said "Empty directory" or a list of the previous
            // directory's objects would each be a claim about a path nobody
            // has answered for yet.
            if listed && empty(entries)
              box w=fill p=22.0
                EmptyPlate message="Empty directory — nothing is committed under this path."
            if listed && !empty(entries)
              scroll
                with
                  dir=vertical
                  w=fill
                  h=fill
                keyed entry in entries by=entry.key virtual-row=39.0 w=fill
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
                        text "first 48 KiB"
                          with
                            size=12.5
                            wrap=none
                            @text-caption
                      if !preview_binary && !preview_picture && !editing && !preview_truncated
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
                            // The same split the forge reader makes: a
                            // Markdown path reads as a document through the
                            // shell's `agent_markdown`, any other text as
                            // numbered, syntect-coloured rows through
                            // `forge_code`. Binary-or-text is the wire's
                            // call; markdown-vs-code is the path's.
                            // A picture draws from the Files surface's slot
                            // (`picture.rs`); its caption is the drawn size.
                            if preview_picture
                              extern picture("files", preview_path) #fs-picture
                              text picture_caption(preview_width, preview_height)
                                with
                                  size=12.0
                                  wrap=none
                                  @text-meta
                            if !preview_binary && !preview_picture && markdown_path(preview_path)
                              lazy preview_text by preview_text, preview_path, dark as cached_doc
                                extern agent_markdown(cached_doc, dark) #fs-markdown -> emit(open_message_link, _)
                            if !preview_binary && !preview_picture && !markdown_path(preview_path)
                              lazy preview_text by preview_text, preview_path, dark as cached_source
                                extern forge_code(cached_source, preview_path, dark) #fs-code
      if connected
        if !empty(preview_entry.path)
          ObjectPanel entry=preview_entry
