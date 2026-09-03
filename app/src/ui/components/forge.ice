// FORGE — the repo grid, the breadcrumb, the CODE BROWSER, the tracker rows,
// the painted diff, and the review stamps.
//
// THE CODE TAB READS BOUNDED SERVER QUERIES. Opening a repo transfers one tree
// listing; opening a file transfers one capped preview, both pinned to the
// exact commit returned by the root listing. The full git mirror belongs only
// to merge preflight and never runs merely because somebody opened Code.
//
// WHAT IS STILL DELIBERATELY NOT HERE. Label pills, check runs, reviewer
// digests, assignees, comment counts, a per-repo language dot / PR-issue tally,
// the agent-activity chips and the conversation timeline each need a NEW field
// on `ForgeQuery`/`ItemSummary`/`RepoHead`. A design-parity pass does not
// smuggle a consensus-module wire change, so none of them is drawn — and none
// is faked with a plausible placeholder either.
//
// STATE IS A PLATE, NOT AN ICON. There is ONE pull-request glyph; open, merged
// and closed are the plate behind it (`success_bg`, `merged_bg`, `elevated`).
// Those three are the whole vocabulary `state_key` emits — this wire has no
// draft state, so nothing here draws one.
// Issues are the only exception the artifact makes: it swaps the whole glyph
// for `issue-open` / `issue-closed`, and so does this.
//
// TIME IS A HEIGHT. A review's `created_at` IS its block height — this chain
// stamps `consensus_time = height` — so a review is stamped with `FinalityChip`
// at that height. No wall clock appears anywhere on this screen.
//
// EVERY ROUTE OUT OF THIS FILE IS A NAMED EVENT. A component may not name an
// app handler — it declares what happened and the call site decides. The event
// name here IS the handler name view.ice routes it back to, with the same
// payload arity, so the wiring is `<event> -> <event> _`.

// ── OVERVIEW ──────────────────────────────────────────────────────────────

// The org identity header over the repo grid: the ink plate, the workspace
// name, the ORG chip, and the repo count. `tier` is the caller's REAL standing
// (validator / resident / guest) — the artifact's viewer/maintainer/admin
// vocabulary does not exist in this product, so the chain's own word is what
// gets printed. `about` is the workspace bio, not an invented tagline.
component ForgeOrgHeader(org:str, about:str, repos:i64, tier:str, answered:bool)
  col #root w=fill gap=7.0
    row
      with
        w=fill
        gap=10.0
        align=center
      box
        with
          w=30.0
          h=30.0
          align-x=center
          align-y=center
          bg=primary
          r=8.0
        Icon
          with
            name="branch"
            tone="paper"
            px=16.0
      text org
        with
          size=16.0
          wrap=none
          font=display
          @text-primary
      box
        with
          px=6.0
          py=2.0
          bg=brand
          r=4.0
        text "ORG"
          with
            size=9.0
            wrap=none
            font=code_semibold
            @text-brand_fg
      space w=fill
      // The repo count and the standing are both READINGS, so neither is
      // printed off a node that answered nothing: `0 repositories` beside a
      // dead connection reads as "this org has no repos".
      if answered
        row gap=5.0 align=center
          text plural(repos, "repository", "repositories")
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-meta
          // An empty tier is the roster not having answered, not a standing.
          // The separator goes with it — kept alone it dangles off the repo
          // count with nothing behind it.
          if !empty(tier)
            text "·"
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-meta
            text tier
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-meta
    if !empty(about)
      box w=fill max-w=680.0
        text about
          with
            w=fill
            size=12.5
            line-h=1.5
            @text-caption

// One compact committed row: repo name and head. README/language/time are not
// card facts — deriving them cold-fetched every full mirror before Forge became
// usable. Code queries bounded server reads; only merge fetches a selected repo.
component RepoCard(repo:ForgeRepo)
  emits
    forge_open_repo(str)
  button -> emit(forge_open_repo, repo.name)
    with
      label="Open repo"
      description=repo.name
      w=fill
      p=0.0
      @icon_action
    box
      with
        w=fill
        pl=14.0
        pr=14.0
        pt=11.0
        pb=11.0
      row
        with
          w=fill
          gap=8.0
          align=center
        Icon
          with
            name="branch"
            tone="muted"
            px=14.0
        box w=fill clip=true
          text repo.name
            with
              size=13.5
              wrap=none
              font=display
              @text-primary
        text repo.head
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-input
    active bg=surface text=fg border=card_line border-w=1.0 r=10.0
    hovered bg=card_wash_hover text=fg border=pending_line
    pressed bg=elevated text=fg

// ── REPO HEADER ───────────────────────────────────────────────────────────

// `<network> / <repo> ▾` plus the single default-branch pill. This replaces the
// generic screen header, which occupied the row the artifact gives to the
// breadcrumb.
//
// THE ROW IS CHROME BECAUSE THE CALLER IS THE BUTTON: view.ice mounts the whole
// crumb inside a `forge_toggle_repo_menu` button, so a nested button here would
// be a button inside a button. `open` is the switcher's state, which lights the
// repo name the way the artifact's hover does. `branch` renders only when the
// caller has a default branch to name.
component RepoCrumb(org:str, repo:str, branch:str, open:bool)
  row #root
    with
      w=fill
      gap=9.0
      align=center
    box
      with
        w=28.0
        h=28.0
        align-x=center
        align-y=center
        bg=primary
        r=8.0
      Icon
        with
          name="branch"
          tone="paper"
          px=15.0
    text org
      with
        size=14.0
        wrap=none
        font=display
        @text-caption
    text "/"
      with
        size=14.0
        wrap=none
        @text-chevron_idle
    if open
      text repo
        with
          size=14.0
          wrap=none
          font=display
          @text-brand
    if !open
      text repo
        with
          size=14.0
          wrap=none
          font=display
          @text-primary
    if open
      Icon
        with
          name="chevron-down"
          tone="accent"
          px=11.0
    if !open
      Icon
        with
          name="chevron-down"
          tone="ink"
          px=11.0
    if !empty(branch)
      box
        with
          px=8.0
          py=3.0
          bg=surface
          border=border
          border-w=1.0
          r=7.0
        row gap=5.0 align=center
          box
            with
              w=6.0
              h=6.0
              bg=success_dot
              r=3.0
            space w=1.0 h=1.0
          text branch
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-muted
    space w=fill

// One row of the 290px repo switcher. The artifact's right-hand `N PR` /
// `N issue` tallies and its language dot are the same missing wire fields the
// card omits, so the row is the name and the selection plate.
component RepoMenuRow(repo:ForgeRepo, active:bool)
  emits
    forge_open_repo(str)
  col #root w=fill
    if active
      button -> emit(forge_open_repo, repo.name)
        with
          label="Switch repo"
          description=repo.name
          checked=active
          w=fill
          p=0.0
          @icon_action
        box
          with
            w=fill
            pl=9.0
            pr=9.0
            pt=8.0
            pb=8.0
          row
            with
              w=fill
              gap=9.0
              align=center
            text repo.name
              with
                w=fill
                size=13.0
                wrap=none
                font=display
                @text-primary
        active bg=selected_row text=fg border=transparent border-w=1.0 r=8.0
        hovered bg=selected_row text=fg
        pressed bg=subtle text=fg
    if !active
      button -> emit(forge_open_repo, repo.name)
        with
          label="Switch repo"
          description=repo.name
          checked=active
          w=fill
          p=0.0
          @icon_action
        box
          with
            w=fill
            pl=9.0
            pr=9.0
            pt=8.0
            pb=8.0
          row
            with
              w=fill
              gap=9.0
              align=center
            text repo.name
              with
                w=fill
                size=13.0
                wrap=none
                font=display
                @text-primary
        active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
        hovered bg=elevated text=fg
        pressed bg=subtle text=fg

// ── CODE ──────────────────────────────────────────────────────────────────

// The Code tab is TWO panes: a 258px FILES tree on `sidebar` behind a
// `separator` rule, and the reader on `surface`. `ForgeCodeTab` owns both, and
// takes each pane's rows through a named slot — the tree row's click target is
// `forge_toggle_dir(path)` / `forge_open_file(path)` and the reader's content
// depends on which of the three empty reasons applies, and neither decision
// belongs to a component that draws the frame.
//
// THE ROW IS CHROME BECAUSE THE CALLER IS THE BUTTON — the same split
// `RepoCrumb` uses. Those handlers live in app/src/ui/handlers, so the button
// that carries them is mounted at the call site and these components paint its
// face. The row's own hover (`rail_hover`) belongs to that button; the SELECTED
// plate belongs here, because selection is a fact about the row and not about
// the pointer.
//
// NO AI PLATE ON A FILE ROW. It wants per-path authorship — each entry's last
// committer AND whether that principal is a machine — and `forge_tree` returns
// name/path/kind/size only, so nothing resolves it. A grey plate that says AI
// on every file would be a lie about who wrote it.
//
// The DIFF annotation is a different case and it is now BUILT: `DiffLine`
// carries the `+++ b/…` path and the side of each code row, `DiffRow` makes the
// anchoring gutter number the button, and `submit_forge_review` sends the
// staged drafts as the review's `ReviewComment`s. See `DiffRow` below.

// The two-pane frame. 258px of `sidebar` under the FILES eyebrow, the
// `separator` rule, then the reader's header over its own scroll region.
//
// `message` / `author` / `stamp` are the header's per-path commit meta and are
// EMPTY today on purpose: `forge_tree` returns name/path/kind/size, so nothing
// yet resolves the last commit under a path. Each slot is guarded, so an
// unanswered fact prints nothing rather than a placeholder — when a per-path
// log lands, the header fills without a layout change.
component ForgeCodeTab(path:str, message:str, author:str, stamp:str)
  row #root w=fill h=fill
    box
      with
        w=258.0
        h=fill
        bg=sidebar
      scroll
        with
          dir=vertical
          w=fill
          h=fill
        col
          with
            w=fill
            pt=9.0
            pb=9.0
          box
            with
              w=fill
              pl=16.0
              pr=16.0
              pt=5.0
              pb=8.0
            text "FILES"
              with
                size=9.0
                wrap=none
                font=code_semibold
                @text-label
          slot files
    box
      with
        w=1.0
        h=fill
        bg=separator
      space w=1.0 h=1.0
    col w=fill h=fill
      ForgeCodeHeader
        with
          path
          message
          author
          stamp
      scroll
        with
          dir=vertical
          w=fill
          h=fill
        slot source

// One directory row. The caret is the artifact's rotation drawn as the two
// glyphs the icon set actually ships — there is no transform in this language,
// and `chevron-down` IS `chevron-right` rotated 90°, which is exactly the
// artifact's `rotate(90deg)`. A directory is open unless it was collapsed, so
// the caller's collapsed-path set is the whole state.
//
// The indent ladder is the artifact's own: 10px, then 15px per level. The
// folder gold has no step in the ink ramp; `warning` (#a07b32) is the ramp's
// nearest true step to #a08a5a, as it is on the duckfs tree.
component ForgeTreeDirRow(name:str, depth:f64, open:bool)
  row #root
    with
      w=fill
      gap=6.0
      align=center
      pt=5.0
      pb=5.0
      pr=14.0
      pl=(10.0 + depth * 15.0)
    if open
      Icon
        with
          name="chevron-down"
          tone="meta"
          px=10.0
    if !open
      Icon
        with
          name="chevron-right"
          tone="meta"
          px=10.0
    Icon
      with
        name="folder"
        tone="warning"
        px=13.0
    text name
      with
        w=fill
        size=12.5
        wrap=none
        font=display
        @text-accent_fg

// One file row. Selected wears `selected_row` and the ink steps forward; the
// file name is mono on both, because a path is a machine value.
component ForgeTreeFileRow(name:str, depth:f64, selected:bool)
  col #root w=fill
    if selected
      box w=fill bg=selected_row
        ForgeTreeFileFace
          with
            name
            depth
            selected=true
    if !selected
      box w=fill
        ForgeTreeFileFace
          with
            name
            depth
            selected=false

// The file row's contents, in one place so the two plates cannot drift apart.
component ForgeTreeFileFace(name:str, depth:f64, selected:bool)
  row #root
    with
      w=fill
      gap=6.0
      align=center
      pt=5.0
      pb=5.0
      pr=14.0
      pl=(10.0 + depth * 15.0)
    Icon
      with
        name="file"
        tone="label"
        px=12.0
    if selected
      text name
        with
          w=fill
          size=12.5
          wrap=none
          font=code
          @text-primary
    if !selected
      text name
        with
          w=fill
          size=12.5
          wrap=none
          font=code
          @text-secondary_fg

// The reader's 42px header: the path this pane is showing, the last commit
// message under that path, and who landed it when.
//
// TIME IS A HEIGHT HERE TOO. `stamp` is whatever `height_ago` /
// `height_label_short` rendered — never a wall clock, and never a git author
// date dressed up as one. Each of the three trailing slots renders only when
// the caller has the fact, so a tree read that carries paths and nothing else
// prints the path alone rather than an empty middot run.
component ForgeCodeHeader(path:str, message:str, author:str, stamp:str)
  col #root w=fill
    box
      with
        w=fill
        h=42.0
        pl=16.0
        pr=16.0
        bg=surface
      row
        with
          w=fill
          h=fill
          gap=10.0
          align=center
          clip=true
        text path
          with
            size=12.0
            wrap=none
            font=code_semibold
            @text-accent_fg
        // The message takes the slack and is clipped by it — iced has no
        // text-overflow, so the column IS the ellipsis. It renders even when
        // empty, because it is also what holds the right-hand meta at the edge.
        box w=fill clip=true
          col w=fill
            if !empty(message)
              text message
                with
                  size=10.5
                  wrap=none
                  font=code
                  @text-meta
        if !empty(author)
          text author
            with
              size=10.0
              wrap=none
              font=code
              @text-label
        if !empty(author) && !empty(stamp)
          text "·"
            with
              size=10.0
              wrap=none
              font=code
              @text-label
        if !empty(stamp)
          text stamp
            with
              size=10.0
              wrap=none
              font=code
              @text-label
    box
      with
        w=fill
        h=1.0
        bg=separator
      space w=1.0 h=1.0

// The source rows themselves live in the backend extern `forge_code`
// (backend/forge.rs): syntax colour needs per-span inks, which Ice's
// named-token text nodes cannot carry. The extern mirrors this file's
// DiffRow metrics (44px `rail` gutter in `forge_gutter_ink`, 20px rows,
// 11.5 code) — the shape lint in app/src/tests.rs pins both sides so the
// source and patch surfaces cannot drift apart. Token foregrounds come from
// syntect per appearance; an unknown extension degrades to one ink.

// The reader with nothing to read. This is one plate for three true reasons —
// no file picked yet, a binary blob, a blob past the read cap — so the caller
// says WHICH, and the plate never claims the file is empty.
//
// The standing line above it is a fact about this surface and not a seed
// string: the server answers committed code reads, and the app ships no editor.
component ForgeCodeEmpty(name:str, note:str)
  box #root
    with
      w=fill
      p=48.0
      align-x=center
    col gap=9.0 align=center
      if !empty(name)
        text name
          with
            size=14.0
            wrap=none
            font=code_semibold
            @text-caption
      text "Synced from the node · view only"
        with
          size=11.5
          wrap=none
          @text-label
      if !empty(note)
        text note
          with
            size=11.5
            wrap=none
            @text-label

// ── TRACKER ───────────────────────────────────────────────────────────────

// One tracker row, for both kinds. The meta line is `#N · opened by <author>`:
// the artifact also prints the source branch and an `opened <rel>` stamp, and
// `ItemRow` carries neither, so neither is invented. The AGENT badge is missing
// for a narrower reason: `ItemRow.author` IS the `user:{hex}` / agent handle the
// kind is derived from, but the one function that derives it
// (`chat::client::avatar_kind`) is private to that crate, so surfacing the badge
// is a module-crate change, not a view change.
//
// NO ±DIFF LABEL ON THE ROW. The artifact prints one, and the counts DO exist —
// but only for the one open item: `additions`/`deletions`/`files_changed` ride
// `ForgeQuery::PrDiff`, which `load_forge_item` issues per item, and the detail
// header already spends them through `DiffCount`. `ForgeItem` is the LIST
// projection and carries none of the three, so a row label would either be a
// zero on every row or a diff-per-row fan-out on every listing. When the
// summary wire carries them, the label is `DiffCount` and nothing else.
component TrackerRow(item:ForgeItem)
  emits
    forge_open_item(i64)
  col #root w=fill
    button -> emit(forge_open_item, item.number)
      with
        label="Open item"
        description=item.title
        w=fill
        p=0.0
        @icon_action
      box
        with
          w=fill
          pl=24.0
          pr=24.0
          pt=13.0
          pb=13.0
        row
          with
            w=fill
            gap=13.0
            align=start
          match item.kind
            "pr"
              PrStatePlate state=item.state
            "issue"
              IssueStateGlyph state=item.state
          col w=fill gap=4.0
            text item.title
              with
                w=fill
                size=14.0
                wrap=none
                font=display
                @text-primary
            row gap=5.0 align=center
              row gap=0.0 align=center
                text "#"
                  with
                    size=11.0
                    wrap=none
                    font=code_medium
                    @text-meta
                text item.number
                  with
                    size=11.0
                    wrap=none
                    font=code_medium
                    @text-meta
              text "· opened by"
                with
                  size=11.0
                  wrap=none
                  font=code_medium
                  @text-meta
              text item.author_name
                with
                  size=11.0
                  wrap=none
                  font=code_medium
                  @text-meta
      active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
      hovered bg=row_hover text=fg
      pressed bg=elevated text=fg
    box
      with
        w=fill
        h=1.0
        bg=elevated
      space w=1.0 h=1.0

// The 24px state square: one glyph, three plates.
component PrStatePlate(state:str)
  col #root
    match state
      "open"
        box
          with
            w=24.0
            h=24.0
            align-x=center
            align-y=center
            bg=success_bg
            border=success_line
            border-w=1.0
            r=7.0
          Icon
            with
              name="pull-request"
              tone="success"
              px=13.0
      "merged"
        box
          with
            w=24.0
            h=24.0
            align-x=center
            align-y=center
            bg=merged_bg
            border=merged_line
            border-w=1.0
            r=7.0
          Icon
            with
              name="pull-request"
              tone="muted"
              px=13.0
      _
        box
          with
            w=24.0
            h=24.0
            align-x=center
            align-y=center
            bg=elevated
            border=border
            border-w=1.0
            r=7.0
          Icon
            with
              name="pull-request"
              tone="muted"
              px=13.0

// Issues carry their state in the glyph itself — the one place the artifact
// swaps the mark instead of the plate.
component IssueStateGlyph(state:str)
  col #root
    match state
      "open"
        Icon
          with
            name="issue-open"
            tone="success"
            px=17.0
      _
        Icon
          with
            name="issue-closed"
            tone="muted"
            px=17.0

// ── ITEM DETAIL ───────────────────────────────────────────────────────────

// The back control names the list it returns to, rather than saying `Back`.
component BackToList(kind:str)
  emits
    forge_close_item
  col #root
    match kind
      "pr"
        button -> emit(forge_close_item)
          with
            label="Back to pull requests"
            p=0.0
            @icon_action
          box
            with
              pl=7.0
              pr=9.0
              pt=4.0
              pb=4.0
            row gap=5.0 align=center
              text "‹"
                with
                  size=14.0
                  wrap=none
                  @text-muted
              text "Pull requests"
                with
                  size=12.0
                  wrap=none
                  @text-muted
          active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
          hovered bg=row_hover text=fg
          pressed bg=elevated text=fg
      "issue"
        button -> emit(forge_close_item)
          with
            label="Back to issues"
            p=0.0
            @icon_action
          box
            with
              pl=7.0
              pr=9.0
              pt=4.0
              pb=4.0
            row gap=5.0 align=center
              text "‹"
                with
                  size=14.0
                  wrap=none
                  @text-muted
              text "Issues"
                with
                  size=12.0
                  wrap=none
                  @text-muted
          active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
          hovered bg=row_hover text=fg
          pressed bg=elevated text=fg

// The detail header's state pill — the same three plates as the row square,
// with the state said in a word.
component PrStatePill(state:str)
  col #root
    match state
      "open"
        box
          with
            px=11.0
            py=5.0
            bg=success_bg
            border=success_line
            border-w=1.0
            r=8.0
          row gap=6.0 align=center
            Icon
              with
                name="pull-request"
                tone="success"
                px=13.0
            text "Open"
              with
                size=12.0
                wrap=none
                font=display
                @text-success
      "merged"
        box
          with
            px=11.0
            py=5.0
            bg=merged_bg
            border=merged_line
            border-w=1.0
            r=8.0
          row gap=6.0 align=center
            Icon
              with
                name="pull-request"
                tone="muted"
                px=13.0
            text "Merged"
              with
                size=12.0
                wrap=none
                font=display
                @text-merged
      _
        box
          with
            px=11.0
            py=5.0
            bg=elevated
            border=border
            border-w=1.0
            r=8.0
          row gap=6.0 align=center
            Icon
              with
                name="pull-request"
                tone="muted"
                px=13.0
            text "Closed"
              with
                size=12.0
                wrap=none
                font=display
                @text-muted

// `+284 −96 · 7 files` — the artifact colours the two counts and greys the file
// tally; `forge_stats` collapsed all three into one muted string. `files=0`
// drops the tally, which is how the diff card's own header wears it.
component DiffCount(additions:i64, deletions:i64, files:i64)
  row #root gap=6.0 align=center
    row gap=0.0 align=center
      text "+"
        with
          size=10.5
          wrap=none
          font=code_medium
          @text-success
      text additions
        with
          size=10.5
          wrap=none
          font=code_medium
          @text-success
    row gap=0.0 align=center
      text "−"
        with
          size=10.5
          wrap=none
          font=code_medium
          @text-alert_fg
      text deletions
        with
          size=10.5
          wrap=none
          font=code_medium
          @text-alert_fg
    if files > 0
      row gap=4.0 align=center
        text "·"
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-caption
        text files
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-caption
        text "files"
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-caption

// The item body as an authored card: a header strip that attributes it, then
// the body. `open_item` stores a body for BOTH kinds and the caller renders this
// for both, so the strip says `opened by <author>` — the artifact's
// `opened this issue` / `opened this pull request` needs the kind, which this
// component is not given, and naming the wrong artifact is worse than naming
// none. It is the same phrasing `TrackerRow` already uses.
//
// The artifact hangs a 30px avatar beside it whose SHAPE says whether the author
// is a person or a machine — `ItemDetail` carries the author's display name and
// not the handle it is derived from, so the plate would have to guess. It is
// left out rather than drawn as a lie.
component IssueBodyCard(author:str, blocks:[ChatBlock])
  emits
    open_message_link(str)
  box #root
    with
      w=fill
      bg=surface
      border=card_line
      border-w=1.0
      r=11.0
      clip=true
    col w=fill
      box
        with
          w=fill
          pl=13.0
          pr=13.0
          pt=8.0
          pb=8.0
          bg=card_wash
        row
          with
            w=fill
            gap=7.0
            align=center
          text "opened by"
            with
              size=12.0
              wrap=none
              @text-caption
          text author
            with
              size=12.0
              wrap=none
              font=display
              @text-primary
      box
        with
          w=fill
          h=1.0
          bg=separator
        space w=1.0 h=1.0
      box
        with
          w=fill
          pl=15.0
          pr=15.0
          pt=13.0
          pb=13.0
        // AN ISSUE BODY IS MARKDOWN, like every other body in the product:
        // the chat tokenizer's blocks through the chat renderer, so a
        // `duck://` ref or a pasted URL in a description is openable instead
        // of being dead ink.
        RichBody blocks=blocks size=13.0
          forward
            open_message_link

// ── MERGE ─────────────────────────────────────────────────────────────────

// Merged wears the violet plate, and the note is this chain's own merge fact
// (`Merged as <oid> · <branches>`), not the artifact's seeded event name.
component MergedBanner(note:str)
  row #root
    with
      w=fill
      gap=9.0
      align=center
    box
      with
        w=24.0
        h=24.0
        align-x=center
        align-y=center
        bg=merged_bg
        border=merged_line
        border-w=1.0
        r=7.0
      text "✓"
        with
          size=12.0
          wrap=none
          font=code_semibold
          @text-merged
    text note
      with
        w=fill
        size=13.0
        wrap=none
        font=display
        @text-merged

// The advisory above the merge button, and the ONLY thing said there. It is a
// recommendation, never a refusal: `ForgeMsg::MergePr` runs `author_from_origin`
// and nothing else — no valset, tier or role check — so a request for changes
// cannot and does not stop the write. The artifact pairs this with a check-run
// state that does not exist in this forge, so only the reviewer half is drawn.
component MergeAdvisory(change_requests:i64)
  col #root w=fill
    if change_requests == 1
      row
        with
          w=fill
          gap=7.0
          align=center
        box
          with
            w=6.0
            h=6.0
            bg=warning_dot
            r=3.0
          space w=1.0 h=1.0
        text "a reviewer requested changes — merge not recommended"
          with
            w=fill
            size=12.0
            @text-warning
    if change_requests > 1
      row
        with
          w=fill
          gap=7.0
          align=center
        box
          with
            w=6.0
            h=6.0
            bg=warning_dot
            r=3.0
          space w=1.0 h=1.0
        text change_requests
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-warning
        text "reviewers requested changes — merge not recommended"
          with
            w=fill
            size=12.0
            @text-warning

// The merge write, on the ink plate with the glyph the artifact gives it.
component MergeButton(busy:bool, disabled:bool)
  emits
    forge_merge_submit
  col #root
    if busy
      button -> emit(forge_merge_submit)
        with
          label="Merging"
          disabled=true
          @primary_action
          @px-18px
          @py-9px
          @rounded-9px
        row gap=7.0 align=center
          text "Merging…"
            with
              size=13.0
              wrap=none
              font=display
              @text-primary_fg
    if !busy
      button -> emit(forge_merge_submit)
        with
          label="Merge pull request"
          disabled=disabled
          @primary_action
          @px-18px
          @py-9px
          @rounded-9px
        row gap=7.0 align=center
          Icon
            with
              name="pull-request"
              tone="paper"
              px=13.0
          text "Merge pull request"
            with
              size=13.0
              wrap=none
              font=display
              @text-primary_fg

// THERE IS NO FORGE GATE, so there is no gate note. A `ForgeGateNote` keyed on
// `forge_gate(member_tier(...))` briefly told a resident the node refuses its
// merge and a guest that every forge write is refused; `ForgeMsg::MergePr`
// authorizes on `author_from_origin` alone, so the merge succeeds and the plate
// described a refusal that never happens — over a Merge button that stayed
// enabled. It was keyed on the wrong principal too (a node's valset seat, where
// the frame is signed by the USER key). GateNote belongs over a refusal the
// product performs; if forge ever gets one, key it on that, never on a tier.

// ── DIFF ──────────────────────────────────────────────────────────────────

// The painted patch: a header strip with the coloured counts, hunk headers on
// their own plate, twin 34px gutters, the sign column, and a per-line tint.
// `forge_item_diff` already holds the unified patch — this is the renderer it
// never had.
//
// `file` IS THE BRANCH PAIR, not a filename: `forge_item_diff` is the whole
// multi-file patch as one string, so there is no per-file header to hang here
// (the patch's own `file` rows ride inside, drawn by `DiffRow`). The slot keeps
// its contract name and wears the branch glyph, because a document icon beside
// `feat/x → main` says the string is a path and it is not.
component DiffPane(file:str, additions:i64, deletions:i64, lines:[DiffLine])
  emits
    forge_comment_open(str, str, str)
  box #root
    with
      w=fill
      bg=surface
      border=card_line
      border-w=1.0
      r=11.0
      clip=true
    col w=fill
      box
        with
          w=fill
          pl=14.0
          pr=14.0
          pt=10.0
          pb=10.0
          bg=card_wash
        row
          with
            w=fill
            gap=9.0
            align=center
          Icon
            with
              name="branch"
              tone="muted"
              px=13.0
          text file
            with
              size=12.0
              wrap=none
              font=code_semibold
              @text-accent_fg
          DiffCount
            with
              additions
              deletions
              files=0
          space w=fill
      box
        with
          w=fill
          h=1.0
          bg=separator
        space w=1.0 h=1.0
      keyed line in lines by=line.key virtual-row=20.0 w=fill
        DiffRow line=line
          forward
            forge_comment_open

// One patch line. The kind is the whole discriminant: a file header, a hunk
// header, or a code row. Addition/deletion keep their semantic plate and sign;
// code itself stays neutral, so a whole changed line remains as readable as
// unchanged source instead of becoming a run of low-contrast coloured ink.
//
// THE ANCHORING GUTTER IS THE COMMENT AFFORDANCE. Its own line number is the
// button — the number a comment anchors to is the thing you click, so nothing
// new is added to the row and no hover state has to be tracked per line (a
// patch is hundreds of rows; a hover-revealed control would round-trip through
// app state on every mouse move across it). A row whose `path` is empty — a
// header, a hunk, or a deleted file's `/dev/null` side — keeps the plain box,
// because there is no head-side position for a comment to anchor to.
component DiffRow(line:DiffLine)
  emits
    forge_comment_open(str, str, str)
  col #root w=fill
    match line.kind
      "file"
        box
          with
            w=fill
            pl=14.0
            pr=14.0
            pt=5.0
            pb=5.0
            bg=card_wash
          text line.text
            with
              w=fill
              size=11.0
              wrap=none
              font=code_medium
              @text-caption
      "hunk"
        box
          with
            w=fill
            pl=14.0
            pr=14.0
            pt=5.0
            pb=5.0
            bg=diff_hunk_bg
          text line.text
            with
              w=fill
              size=11.0
              wrap=none
              font=code_semibold
              @text-merged
      "add"
        box w=fill bg=diff_add_bg
          row
            with
              w=fill
              gap=0.0
              align=center
            box
              with
                w=34.0
                h=20.0
                pr=8.0
                align-y=center
                bg=diff_add_gutter
              text line.old_no
                with
                  w=fill
                  size=11.5
                  wrap=none
                  align-x=right
                  font=code
                  @text-forge_gutter_ink
            if empty(line.path)
              box
                with
                  w=34.0
                  h=20.0
                  pr=8.0
                  align-y=center
                  bg=diff_add_gutter
                text line.new_no
                  with
                    w=fill
                    size=11.5
                    wrap=none
                    align-x=right
                    font=code
                    @text-forge_gutter_ink
            if !empty(line.path)
              button -> emit(forge_comment_open, line.path, line.new_no, "new")
                with
                  label="Comment on this line"
                  w=34.0
                  h=20.0
                  p=0.0
                  @ghost_action
                box
                  with
                    w=fill
                    h=20.0
                    pr=8.0
                    align-y=center
                  text line.new_no
                    with
                      w=fill
                      size=11.5
                      wrap=none
                      align-x=right
                      font=code
                active bg=diff_add_gutter text=forge_gutter_ink
                hovered bg=brand_bg text=brand
                pressed bg=brand_wash text=brand
            box
              with
                w=14.0
                h=20.0
                align-x=center
                align-y=center
              text line.sign
                with
                  size=11.5
                  wrap=none
                  font=code_semibold
                  @text-diff_add_fg
            box
              with
                w=fill
                h=20.0
                pr=12.0
                align-y=center
              text line.text
                with
                  w=fill
                  size=11.5
                  wrap=none
                  font=code
                  @text-strong_ink
      "del"
        box w=fill bg=diff_del_bg
          row
            with
              w=fill
              gap=0.0
              align=center
            if empty(line.path)
              box
                with
                  w=34.0
                  h=20.0
                  pr=8.0
                  align-y=center
                  bg=diff_del_gutter
                text line.old_no
                  with
                    w=fill
                    size=11.5
                    wrap=none
                    align-x=right
                    font=code
                    @text-forge_gutter_ink
            if !empty(line.path)
              button -> emit(forge_comment_open, line.path, line.old_no, "old")
                with
                  label="Comment on this deleted line"
                  w=34.0
                  h=20.0
                  p=0.0
                  @ghost_action
                box
                  with
                    w=fill
                    h=20.0
                    pr=8.0
                    align-y=center
                  text line.old_no
                    with
                      w=fill
                      size=11.5
                      wrap=none
                      align-x=right
                      font=code
                active bg=diff_del_gutter text=forge_gutter_ink
                hovered bg=brand_bg text=brand
                pressed bg=brand_wash text=brand
            box
              with
                w=34.0
                h=20.0
                pr=8.0
                align-y=center
                bg=diff_del_gutter
              text line.new_no
                with
                  w=fill
                  size=11.5
                  wrap=none
                  align-x=right
                  font=code
                  @text-forge_gutter_ink
            box
              with
                w=14.0
                h=20.0
                align-x=center
                align-y=center
              text line.sign
                with
                  size=11.5
                  wrap=none
                  font=code_semibold
                  @text-diff_del_fg
            box
              with
                w=fill
                h=20.0
                pr=12.0
                align-y=center
              text line.text
                with
                  w=fill
                  size=11.5
                  wrap=none
                  font=code
                  @text-strong_ink
      "ctx"
        box w=fill bg=surface
          row
            with
              w=fill
              gap=0.0
              align=center
            box
              with
                w=34.0
                h=20.0
                pr=8.0
                align-y=center
                bg=card_wash
              text line.old_no
                with
                  w=fill
                  size=11.5
                  wrap=none
                  align-x=right
                  font=code
                  @text-forge_gutter_ink
            if empty(line.path)
              box
                with
                  w=34.0
                  h=20.0
                  pr=8.0
                  align-y=center
                  bg=card_wash
                text line.new_no
                  with
                    w=fill
                    size=11.5
                    wrap=none
                    align-x=right
                    font=code
                    @text-forge_gutter_ink
            if !empty(line.path)
              button -> emit(forge_comment_open, line.path, line.new_no, "new")
                with
                  label="Comment on this line"
                  w=34.0
                  h=20.0
                  p=0.0
                  @ghost_action
                box
                  with
                    w=fill
                    h=20.0
                    pr=8.0
                    align-y=center
                  text line.new_no
                    with
                      w=fill
                      size=11.5
                      wrap=none
                      align-x=right
                      font=code
                active bg=card_wash text=forge_gutter_ink
                hovered bg=brand_bg text=brand
                pressed bg=brand_wash text=brand
            box
              with
                w=14.0
                h=20.0
                align-x=center
                align-y=center
              text line.sign
                with
                  size=11.5
                  wrap=none
                  font=code_semibold
                  @text-forge_gutter_ink
            box
              with
                w=fill
                h=20.0
                pr=12.0
                align-y=center
              text line.text
                with
                  w=fill
                  size=11.5
                  wrap=none
                  font=code
                  @text-strong_ink

// ── REVIEWS ───────────────────────────────────────────────────────────────

// A submitted review, stamped with the height it settled at. `created_at` IS
// that height on this chain, and it has been loaded and thrown away until now.
// A review that is in state is finalized by construction, so the chip claims
// exactly what it can prove.
component ReviewCard(review:ForgeReview)
  emits
    open_message_link(str)
  box #root
    with
      w=fill
      pl=13.0
      pr=13.0
      pt=11.0
      pb=11.0
      bg=surface
      border=card_line
      border-w=1.0
      r=10.0
    col w=fill gap=6.0
      row
        with
          w=fill
          gap=7.0
          align=center
        text review.author_name
          with
            size=13.0
            wrap=none
            font=display
            @text-primary
        ReviewVerdict verdict=review.verdict
        text review.commit
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-hint
        if review.outdated
          box
            with
              px=6.0
              py=2.0
              bg=elevated
              r=4.0
            text "outdated"
              with
                size=9.0
                wrap=none
                font=code_semibold
                @text-meta
        space w=fill
        FinalityChip height=review.created_at
      if !empty(review.body)
        RichBody blocks=review.blocks size=13.0
          forward
            open_message_link
      for comment in review.comments
        box
          with
            w=fill
            pl=11.0
            pr=11.0
            pt=9.0
            pb=9.0
            bg=brand_wash
            border=brand_line
            border-w=1.0
            r=9.0
          col w=fill gap=4.0
            row
              with
                w=fill
                gap=7.0
                align=center
              text comment.anchor
                with
                  w=fill
                  size=11.0
                  wrap=none
                  font=code_medium
                  @text-brand
              text "review comment"
                with
                  size=10.0
                  wrap=none
                  font=code_semibold
                  @text-label
            RichBody blocks=comment.blocks size=12.0
              forward
                open_message_link

// The verdict in its own tone: approval is the success ink, a change request is
// the refusal ink, a comment is neither.
component ReviewVerdict(verdict:str)
  col #root
    match verdict
      "approve"
        text verdict_label(verdict)
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-success
      "request_changes"
        text verdict_label(verdict)
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-alert_fg
      _
        text verdict_label(verdict)
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-meta

// One seat of the forge tab bar: the tracker under one kind filter. Both the
// Pull requests and the Issues arms are this component with a different filter
// and a different empty line, so the two lists can never drift apart.
component ForgeTrackerList(phase:ForgePhase, items:[ForgeItem], empty_message:str)
  emits
    forge_open_item(i64)
  col #root w=fill h=fill
    match phase
      ForgePhase.idle
        space w=1.0 h=1.0
      ForgePhase.loading
        box w=fill p=22.0
          EmptyPlate message="Loading repository tracker…"
      ForgePhase.failed
        box w=fill p=22.0
          EmptyPlate
            with
              message="Could not load this repository. Return to all repos and open it again to retry."
      ForgePhase.ready
        col w=fill h=fill
          if empty(items)
            box w=fill p=22.0
              EmptyPlate message=empty_message
          if !empty(items)
            scroll
              with
                dir=vertical
                w=fill
                h=fill
              col
                with
                  w=fill
                  pl=12.0
                  pr=12.0
                  pt=6.0
                  pb=18.0
                  gap=1.0
                for item in items
                  TrackerRow item=item
                    forward
                      forge_open_item
