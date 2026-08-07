// FILES — the duckfs read surface, in the artifact's three panes: a 206px
// content-addressed tree, an object table with real columns, and a 306px
// object panel. Everything here is painted from what `EntryInfo` actually
// carries — path, name, kind, size and the content address.
//
// FOUR DELIBERATE OMISSIONS, decided by the campaign and not to be
// re-litigated per screen. Each is a fact the wire does not carry, not a layout
// we skipped:
//
// 1. The table's HEIGHT and BY columns. `EntryInfo`
//    (crates/duckfs/core/src/wire.rs:238) has no author and no height — only
//    `SnapshotInfo` does — so a per-ROW stamp costs one diff walk per row on
//    every listing. The table stays NAME / SIZE / OBJECT. The single selected
//    path pays that walk once and shows it in the panel (see `ObjectPanel`).
// 2. The PINNED badge and the GC pin toggle. duckfs pins are SNAPSHOT-scoped
//    (name -> snapshot, wire.rs:285); there is no per-path pin to toggle, so a
//    per-object Pin button would be a different verb wearing the same word.
// 3. REFERENCED FROM. Nothing indexes "what points at this path".
// 4. The per-object HISTORY rail. The panel names the LATEST change under the
//    path; a rail would need every snapshot that touched it, which is the whole
//    bounded history diffed one snapshot at a time.
//
// The artifact's file-row `note` is omitted for the same reason: its only home
// would be `EntryInfo.meta`, which no writer populates today.
//
// Nothing here reaches an app handler directly. Every navigation leaves as a
// named component event carrying the path, and the screen that mounts the row
// decides where it lands; the event names match the handlers the Files screen
// routes them to (`fs_open_dir`, `fs_open_file`) so the wiring reads as identity.

// One directory in the whole-tree sidebar. The indent is the artifact's own
// ladder — 11px, then 14px per level of depth — so the tree reads as a tree
// without drawing a single guide line.
component FsTreeRow(entry:FsEntry, selected:bool, depth:f64)
  emits
    fs_open_dir(str)
  col #root w=fill
    if selected
      button -> emit(fs_open_dir, entry.path)
        with
          label="Open directory"
          w=fill
          p=0.0
          @ghost_action
        FsTreeFace
          with
            name=entry.name
            depth
            dimmed=false
        active bg=subtle text=fg border=transparent border-w=1.0 r=7.0
        hovered bg=subtle
        pressed bg=rail_hover
    if !selected
      button -> emit(fs_open_dir, entry.path)
        with
          label="Open directory"
          w=fill
          p=0.0
          @ghost_action
        FsTreeFace
          with
            name=entry.name
            depth
            dimmed=true
        active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
        hovered bg=rail_hover
        pressed bg=subtle

// The tree row's contents, in one place so the selected and idle buttons can
// never drift apart. The folder gold has no step in the ink ramp; `warning`
// (#a07b32) is the ramp's own muted gold and the nearest true step to the
// artifact's #a08a5a.
component FsTreeFace(name:str, depth:f64, dimmed:bool)
  row #root
    with
      w=fill
      gap=7.0
      align=center
      pt=6.0
      pb=6.0
      pr=12.0
      pl=(11.0 + depth * 14.0)
    Icon
      with
        name="folder"
        tone="warning"
        px=13.0
    if dimmed
      text name
        with
          w=fill
          size=12.0
          wrap=none
          font=code_medium
          @text-muted
    if !dimmed
      text name
        with
          w=fill
          size=12.0
          wrap=none
          font=code_medium
          @text-fg

// The crumb bar over the object table: where you are, what is here, and who is
// allowed to write under it.
//
// MOUNTED at the head of the Files arm in view.ice, replacing
// `ScreenHeader title="Files" meta=fs_path` and wired `fs_open_dir ->
// fs_open_dir _`. `path` is `fs_path`; the two counts are `fs_dir_count` /
// `fs_file_count` (backend.rs), pure folds over the already-resident
// `fs_entries` — Ice cannot filter a list by field, and a second listing call
// to count what is already on screen would be a lie waiting to go stale.
//
// The root crumb navigates; the segments do not. Ice's expression language has
// no string split (len/empty/trim/some are the whole builtin set), and the
// frozen signature carries the path as one `str` rather than a segment list, so
// a per-segment crumb cannot be built from these props. The path prints as one
// mono run beside a root crumb that does navigate.
component CrumbBar(path:str, dirs:i64, files:i64)
  emits
    fs_open_dir(str)
  col #root w=fill
    box
      with
        w=fill
        h=50.0
        px=20.0
      row
        with
          w=fill
          h=fill
          gap=6.0
          align=center
        button -> emit(fs_open_dir, "")
          with
            label="Go to the duckfs root"
            p=0.0
            @ghost_action
          text "duckfs"
            with
              size=12.5
              wrap=none
              font=code_semibold
              @text-primary
          active bg=transparent text=primary border=transparent border-w=1.0 r=6.0
          hovered bg=elevated
          pressed bg=subtle
        if !empty(path)
          text path
            with
              size=12.5
              wrap=none
              font=code_semibold
              @text-primary
        row
          with
            gap=4.0
            align=center
            pl=4.0
          text files
            with
              size=11.0
              wrap=none
              font=code
              @text-hint
          text "files ·"
            with
              size=11.0
              wrap=none
              font=code
              @text-hint
          text dirs
            with
              size=11.0
              wrap=none
              font=code
              @text-hint
          text "dirs"
            with
              size=11.0
              wrap=none
              font=code
              @text-hint
        space w=fill
        // duckfs write authority, stated in full rather than per-path, in the
        // terms check_authority actually uses (crates/duckfs/core/src/paths.rs).
        // The owner segment is an ACTOR string — a module id for a module, and
        // `ext:<key>` for the person writing from this app — so a member does
        // own a home tree. Both roots reject a write on their own. A path
        // prefix test is not expressible here, and one honest rule beats a
        // guessed branch.
        text "writes · /home/<owner>/** by that owner · /shared/** by any member · roots not writable"
          with
            size=10.5
            wrap=none
            @text-meta
    box
      with
        w=fill
        h=1.0
        bg=separator
      space w=1.0 h=1.0

// The object table's column header. It owns the column widths, and `ObjectRow`
// reads the same numbers — the two must be mounted together or the table stops
// lining up.
component ObjectTableHeader()
  col #root w=fill
    box
      with
        w=fill
        h=50.0
        px=20.0
      row
        with
          w=fill
          h=fill
          gap=12.0
          align=center
        text "NAME"
          with
            w=fill
            size=9.0
            wrap=none
            font=code_semibold
            @text-gutter_ink
        text "SIZE"
          with
            w=72.0
            align-x=right
            size=9.0
            wrap=none
            font=code_semibold
            @text-gutter_ink
        text "OBJECT"
          with
            w=92.0
            align-x=right
            size=9.0
            wrap=none
            font=code_semibold
            @text-gutter_ink
    // `separator`, not `elevated`: at 50px this rule meets the duckfs pane's
    // rule at the seam, and one line cannot change colour halfway across.
    box
      with
        w=fill
        h=1.0
        bg=separator
      space w=1.0 h=1.0

// One entry. A directory opens; a file selects into the object panel. Only a
// file is ever the selected row, so the three branches are the whole space.
component ObjectRow(entry:FsEntry, selected:bool)
  emits
    fs_open_dir(str)
    fs_open_file(str)
  col #root w=fill
    if entry.kind == "dir"
      button -> emit(fs_open_dir, entry.path)
        with
          label="Open directory"
          w=fill
          p=0.0
          @ghost_action
        ObjectRowFace entry=entry
        active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
        hovered bg=row_hover
        pressed bg=subtle
    if entry.kind != "dir" && selected
      button -> emit(fs_open_file, entry.path)
        with
          label="Show object"
          w=fill
          p=0.0
          @ghost_action
        ObjectRowFace entry=entry
        active bg=elevated text=fg border=transparent border-w=1.0 r=0.0
        hovered bg=elevated
        pressed bg=subtle
    if entry.kind != "dir" && !selected
      button -> emit(fs_open_file, entry.path)
        with
          label="Show object"
          w=fill
          p=0.0
          @ghost_action
        ObjectRowFace entry=entry
        active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
        hovered bg=row_hover
        pressed bg=subtle
    box
      with
        w=fill
        h=1.0
        bg=muted_bg
      space w=1.0 h=1.0

// The row's contents. A directory's SIZE reads as an em dash because
// `EntryInfo.size` is a CHILD COUNT for a directory, not bytes (duckfs
// tree.rs) — `3 B` would be a lie. Its OBJECT is a real value and is shown: a
// directory's id is its TreeObj digest.
//
// EVERY CELL IS CLIPPED. iced has no text-overflow:ellipsis, so a `wrap=none`
// run wider than its column paints straight over its neighbours — a full
// blake3 hex is ~420px of glyphs pinned at a 92px right edge, which walked
// across SIZE and the file name on every row. The clip box is the column; the
// text fills it.
component ObjectRowFace(entry:FsEntry)
  row #root
    with
      w=fill
      gap=12.0
      align=center
      px=20.0
      py=11.0
      clip=true
    row
      with
        w=fill
        gap=9.0
        align=center
        clip=true
      if entry.kind == "dir"
        Icon
          with
            name="folder"
            tone="warning"
            px=16.0
      if entry.kind != "dir"
        Icon
          with
            name="file"
            tone="label"
            px=16.0
      text entry.name
        with
          w=fill
          size=12.5
          wrap=none
          font=code_medium
          @text-accent_fg
    box w=72.0 clip=true
      col w=fill
        if entry.kind == "dir"
          text "—"
            with
              w=fill
              align-x=right
              size=11.0
              wrap=none
              font=code
              @text-input
        if entry.kind != "dir"
          text size_label(entry.size)
            with
              w=fill
              align-x=right
              size=11.0
              wrap=none
              font=code
              @text-input
    box w=92.0 clip=true
      col w=fill
        if empty(entry.object)
          text "—"
            with
              w=fill
              align-x=right
              size=11.0
              wrap=none
              font=code
              @text-hint
        if !empty(entry.object)
          text entry.object
            with
              w=fill
              align-x=right
              size=11.0
              wrap=none
              font=code
              @text-hint

// The 306px object panel: identity, then the machine values behind it. The
// artifact's kind chip is an uppercased file extension; Ice cannot split a
// string, so the chip carries the kind duckfs itself reports.
//
// `changed_by` / `changed_height` are the newest SNAPSHOT whose diff touches
// this path — `SnapshotInfo` carries both an author and a height, the history
// is bounded, and a diff takes a path prefix. That is a real fact and it is
// labelled for exactly what it is: LAST CHANGED AT THIS PATH. It is NOT blob
// authorship — a snapshot's author is whoever committed the tree, and the blob
// under this name may have been written by someone else in an earlier one — so
// the panel never says "author" or "by" alone.
component ObjectPanel(entry:FsEntry, changed_by:str, changed_height:i64)
  row #root w=306.0 h=fill
    box
      with
        w=1.0
        h=fill
        bg=separator
      space w=1.0 h=1.0
    box
      with
        w=fill
        h=fill
        bg=sidebar
      col w=fill h=fill
        box
          with
            w=fill
            h=50.0
            px=16.0
          row
            with
              w=fill
              h=fill
              gap=8.0
              align=center
            text "Object"
              with
                size=13.0
                wrap=none
                font=display
                @text-fg
            space w=fill
            if entry.kind == "dir"
              box
                with
                  px=7.0
                  py=3.0
                  bg=elevated
                  r=5.0
                text "DIR"
                  with
                    size=9.0
                    wrap=none
                    font=code_semibold
                    @text-input
            if entry.kind != "dir"
              box
                with
                  px=7.0
                  py=3.0
                  bg=elevated
                  r=5.0
                text "FILE"
                  with
                    size=9.0
                    wrap=none
                    font=code_semibold
                    @text-input
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
          col w=fill p=16.0
            text entry.name
              with
                w=fill
                size=13.5
                font=code_semibold
                @text-primary
            box w=fill pt=4.0
              text entry.path
                with
                  w=fill
                  size=10.5
                  font=code
                  @text-hint
            col
              with
                w=fill
                gap=7.0
                pt=14.0
              // a directory has a tree digest too, so this arm is only the
              // defensive default for a reply that omitted the field
              if empty(entry.object)
                ObjectFact label="object id" value="—"
              if !empty(entry.object)
                ObjectFact label="object id" value=entry.object
              if entry.kind == "dir"
                ObjectFact label="size" value="—"
              if entry.kind != "dir"
                ObjectFact label="size" value=size_label(entry.size)
              // Height 0 is the genesis block and no snapshot lands on it, so
              // it is the honest "no stamp yet" — the walk has not answered, or
              // no snapshot in the bounded history touches this path. Nothing
              // is drawn then; a dash here would read as "nobody ever wrote
              // this", which is a different claim than "we do not know".
              if changed_height > 0
                col w=fill gap=7.0
                  ObjectFact label="last changed at this path" value=height_label(changed_height)
                  ObjectFact label="in the snapshot by" value=changed_by

// One machine value, in the artifact's own r8 pill rather than the app's
// `KeyValueRow` — the pills are separate outlines, not a divided card. Clipped
// for the same reason the table cells are: a 64-hex object id is wider than
// this 306px panel and would otherwise paint outside the pill's own border.
component ObjectFact(label:str, value:str)
  box #root
    with
      w=fill
      px=12.0
      py=9.0
      border=card_line
      border-w=1.0
      r=8.0
      clip=true
    row
      with
        w=fill
        gap=10.0
        align=center
      text label
        with
          size=11.0
          wrap=none
          font=code
          @text-meta
      space w=fill
      text value
        with
          size=11.0
          wrap=none
          font=code
          @text-secondary_fg

// EXPLORER — one result. Presentational on purpose: the card carries no route,
// because where a hit opens is the screen's decision, so `view.ice` wraps this
// in the button that navigates.
component ExplorerCard(hit:ExplorerHit)
  box #root
    with
      w=fill
      px=15.0
      py=13.0
      bg=surface
      border=separator
      border-w=1.0
      r=11.0
      clip=true
    row
      with
        w=fill
        gap=12.0
        align=start
      ExplorerKindPlate kind=hit.kind code=hit.code
      col w=fill gap=3.0
        row
          with
            w=fill
            gap=8.0
            align=center
          text hit.title
            with
              w=fill
              size=13.0
              wrap=none
              font=display
              @text-primary
          ExplorerKindBadge kind=hit.kind
        // the snippet is raw message/page body of any length — it wraps, the
        // same way chat's own search hits do. The title and meta stay single
        // runs and are held inside the card by the clip.
        text hit.snippet
          with
            w=fill
            size=12.0
            wrap=word
            line-h=1.5
            @text-input
        text hit.meta
          with
            w=fill
            size=10.5
            wrap=none
            font=code
            @text-label

// The 28px mono plate. Kind reads before the text does: one ink and one wash
// per source, never a shared grey. `message` is the fallback tone.
component ExplorerKindPlate(kind:str, code:str)
  col #root
    match kind
      "page"
        box
          with
            w=28.0
            h=28.0
            align-x=center
            align-y=center
            bg=kind_page_bg
            r=8.0
          text code
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-kind_page
      "code"
        box
          with
            w=28.0
            h=28.0
            align-x=center
            align-y=center
            bg=kind_code_bg
            r=8.0
          text code
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-kind_code
      "file"
        box
          with
            w=28.0
            h=28.0
            align-x=center
            align-y=center
            bg=kind_file_bg
            r=8.0
          text code
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-kind_file
      "run"
        box
          with
            w=28.0
            h=28.0
            align-x=center
            align-y=center
            bg=kind_run_bg
            r=8.0
          text code
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-kind_run
      "task"
        box
          with
            w=28.0
            h=28.0
            align-x=center
            align-y=center
            bg=kind_task_bg
            r=8.0
          text code
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-kind_task
      _
        box
          with
            w=28.0
            h=28.0
            align-x=center
            align-y=center
            bg=info_bg
            r=8.0
          text code
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-info

// The badge beside the title, on the same tint pair as the plate. The artifact
// sets it at 8.5px, which is not a step on the type scale; 9px is the badge
// step and the nearest one.
component ExplorerKindBadge(kind:str)
  col #root
    match kind
      "page"
        box
          with
            px=5.0
            py=2.0
            bg=kind_page_bg
            r=4.0
          text "PAGE"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-kind_page
      "code"
        box
          with
            px=5.0
            py=2.0
            bg=kind_code_bg
            r=4.0
          text "CODE"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-kind_code
      "file"
        box
          with
            px=5.0
            py=2.0
            bg=kind_file_bg
            r=4.0
          text "FILE"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-kind_file
      "run"
        box
          with
            px=5.0
            py=2.0
            bg=kind_run_bg
            r=4.0
          text "RUN"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-kind_run
      "task"
        box
          with
            px=5.0
            py=2.0
            bg=kind_task_bg
            r=4.0
          text "TASK"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-kind_task
      _
        box
          with
            px=5.0
            py=2.0
            bg=info_bg
            r=4.0
          text "MESSAGE"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-info
