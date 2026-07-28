// FORGE — the repo grid, the breadcrumb, the CODE BROWSER, the tracker rows,
// the painted diff, and the review stamps.
//
// THE CODE TAB NEEDS NO WIRE CHANGE, and the campaign was wrong to refuse it as
// "blocked on new module queries". `sync_forge_mirror` (app/src/backend.rs:2957)
// keeps a bare git2 mirror of every branch of every repo under the key root and
// already fetches `+refs/heads/*` for the merge preflight. A tree listing, a
// blob read and a per-path last-commit are `git2` calls against a repository the
// app has already cloned — `ForgeQuery` is not on that path at all.
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
component ForgeOrgHeader(org:str, about:str, repos:i64, tier:str)
  col #root w=fill gap=7.0
    row w=fill gap=10.0 align=center
      box w=30.0 h=30.0 align-x=center align-y=center bg=primary r=8.0
        Icon name="branch" tone="paper" px=16.0
      text org size=16.0 wrap=none font=display @text-primary
      box px=6.0 py=2.0 bg=brand r=4.0
        text "ORG" size=9.0 wrap=none font=code_semibold @text-brand_fg
      space w=fill
      row gap=5.0 align=center
        text repos size=10.5 wrap=none font=code_medium @text-meta
        text "repositories ·" size=10.5 wrap=none font=code_medium @text-meta
        text tier size=10.5 wrap=none font=code_medium @text-meta
    if !empty(about)
      box w=fill max-w=680.0
        text about w=fill size=12.5 line-h=1.5 @text-caption

// One repo card. The artifact org-qualifies the title, but the org it would be
// qualified WITH is the network name the header and the breadcrumb are both fed
// (`network_label(...)`), and `RepoCard(repo)` has no seat for it — printing a
// literal `ducktape/` names a namespace that does not exist on a network called
// anything else, so the card prints the name the node actually returned. The
// only other meta the wire carries is the head digest; the language dot, the
// PR/issue tallies and the `updated` stamp all want fields `RepoHead` does not
// have, so the row holds what is true instead of what would look full.
component RepoCard(repo:ForgeRepo)
  emits
    forge_open_repo(str)
  button label="Open repo" description=repo.name w=fill p=0.0 @icon_action -> emit(forge_open_repo, repo.name)
    box w=fill pl=17.0 pr=17.0 pt=15.0 pb=15.0
      col w=fill gap=10.0
        row w=fill gap=8.0 align=center
          Icon name="branch" tone="muted" px=14.0
          text repo.name size=14.0 wrap=none font=display @text-primary
        row w=fill gap=14.0 align=center
          text repo.head w=fill size=10.5 wrap=none font=code_medium @text-input
    active bg=surface text=fg border=card_line border-w=1.0 r=13.0
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
  row #root w=fill gap=9.0 align=center
    box w=28.0 h=28.0 align-x=center align-y=center bg=primary r=8.0
      Icon name="branch" tone="paper" px=15.0
    text org size=14.0 wrap=none font=display @text-caption
    text "/" size=14.0 wrap=none @text-chevron_idle
    if open
      text repo size=14.0 wrap=none font=display @text-brand
    if !open
      text repo size=14.0 wrap=none font=display @text-primary
    if open
      Icon name="chevron-down" tone="accent" px=11.0
    if !open
      Icon name="chevron-down" tone="ink" px=11.0
    if !empty(branch)
      box px=8.0 py=3.0 bg=surface border=border border-w=1.0 r=7.0
        row gap=5.0 align=center
          box w=6.0 h=6.0 bg=success_dot r=3.0
            space w=1.0 h=1.0
          text branch size=10.5 wrap=none font=code_medium @text-muted
    space w=fill

// One row of the 290px repo switcher. The artifact's right-hand `N PR` /
// `N issue` tallies and its language dot are the same missing wire fields the
// card omits, so the row is the name and the selection plate.
component RepoMenuRow(repo:ForgeRepo, active:bool)
  emits
    forge_open_repo(str)
  col #root w=fill
    if active
      button label="Switch repo" description=repo.name w=fill p=0.0 @icon_action -> emit(forge_open_repo, repo.name)
        box w=fill pl=9.0 pr=9.0 pt=8.0 pb=8.0
          row w=fill gap=9.0 align=center
            text repo.name w=fill size=13.0 wrap=none font=display @text-primary
        active bg=elevated text=fg border=transparent border-w=1.0 r=8.0
        hovered bg=elevated text=fg
        pressed bg=subtle text=fg
    if !active
      button label="Switch repo" description=repo.name w=fill p=0.0 @icon_action -> emit(forge_open_repo, repo.name)
        box w=fill pl=9.0 pr=9.0 pt=8.0 pb=8.0
          row w=fill gap=9.0 align=center
            text repo.name w=fill size=13.0 wrap=none font=display @text-primary
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
// NO AI PLATE ON A FILE ROW, and no signed inline annotation beside a source
// line. Both want per-path authorship: the plate needs each entry's last
// committer AND whether that principal is a machine, and the annotation needs a
// (path, line) anchor. `ReviewComment` (crates/modules/apps/forge/src/interface.rs)
// stores its anchor as a pre-rendered display string, so there is nothing to
// key a line against. A grey plate that says AI on every file would be a lie
// about who wrote it.

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
    box w=258.0 h=fill bg=sidebar
      scroll dir=vertical w=fill h=fill
        col w=fill pt=9.0 pb=9.0
          box w=fill pl=16.0 pr=16.0 pt=5.0 pb=8.0
            text "FILES" size=9.0 wrap=none font=code_semibold @text-label
          slot files
    box w=1.0 h=fill bg=separator
      space w=1.0 h=1.0
    col w=fill h=fill
      ForgeCodeHeader path=path message=message author=author stamp=stamp
      scroll dir=vertical w=fill h=fill
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
  row #root w=fill gap=6.0 align=center pt=5.0 pb=5.0 pr=14.0 pl=(10.0 + depth * 15.0)
    if open
      Icon name="chevron-down" tone="meta" px=10.0
    if !open
      Icon name="chevron-right" tone="meta" px=10.0
    Icon name="folder" tone="warning" px=13.0
    text name w=fill size=12.5 wrap=none font=display @text-accent_fg

// One file row. Selected wears `tree_selected` and the ink steps forward; the
// file name is mono on both, because a path is a machine value.
component ForgeTreeFileRow(name:str, depth:f64, selected:bool)
  col #root w=fill
    if selected
      box w=fill bg=tree_selected
        ForgeTreeFileFace name=name depth=depth selected=true
    if !selected
      box w=fill
        ForgeTreeFileFace name=name depth=depth selected=false

// The file row's contents, in one place so the two plates cannot drift apart.
component ForgeTreeFileFace(name:str, depth:f64, selected:bool)
  row #root w=fill gap=6.0 align=center pt=5.0 pb=5.0 pr=14.0 pl=(10.0 + depth * 15.0)
    Icon name="file" tone="label" px=12.0
    if selected
      text name w=fill size=12.5 wrap=none font=code @text-primary
    if !selected
      text name w=fill size=12.5 wrap=none font=code @text-secondary_fg

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
    box w=fill h=42.0 pl=16.0 pr=16.0 bg=surface
      row w=fill h=fill gap=10.0 align=center clip=true
        text path size=12.0 wrap=none font=code_semibold @text-accent_fg
        // The message takes the slack and is clipped by it — iced has no
        // text-overflow, so the column IS the ellipsis. It renders even when
        // empty, because it is also what holds the right-hand meta at the edge.
        box w=fill clip=true
          col w=fill
            if !empty(message)
              text message size=10.5 wrap=none font=code @text-meta
        if !empty(author)
          text author size=10.0 wrap=none font=code @text-label
        if !empty(author) && !empty(stamp)
          text "·" size=10.0 wrap=none font=code @text-label
        if !empty(stamp)
          text stamp size=10.0 wrap=none font=code @text-label
    box w=fill h=1.0 bg=separator
      space w=1.0 h=1.0

// One source line: a 44px right-aligned gutter on `rail` at the artifact's own
// `#cbc9bf`, then the code. `number` is a string for the same reason
// `DiffLine.old_no` is — the renderer that splits the blob owns the numbering,
// and a blank gutter has to be expressible.
//
// IT HAS NO CALL SITE YET, and that is a missing splitter, not a missing
// decision. `forge_blob` returns `BlobView.text` as ONE string plus a line
// COUNT; Ice has no string ops, so nothing can walk it. The pane therefore
// prints the source as one mono run today (view.ice) and this row waits on the
// exact counterpart `diff_lines` already has for a patch —
// `sync source_lines(text:str) -> [SourceLine]` with `SourceLine(number:str,
// text:str)` in backend.ice. Numbering the lines in the view would mean
// guessing where they break.
//
// The code is ONE ink. The design system is explicit that this viewer uses no
// syntax colour ("코드는 구문 색을 쓰지 않고 단색") — emphasis is carried by the
// signed annotation card, which this wire cannot anchor, so nothing here tints
// a token.
component ForgeCodeLine(number:str, code:str)
  row #root w=fill gap=0.0 align=center
    box w=44.0 h=20.0 pr=12.0 align-y=center bg=rail
      text number w=fill size=12.0 wrap=none align-x=right font=code @text-icon_idle
    box w=fill h=20.0 pl=13.0 align-y=center clip=true
      text code w=fill size=12.0 wrap=none font=code @text-accent_fg

// The reader with nothing to read. This is one plate for three true reasons —
// no file picked yet, a binary blob, a blob past the read cap — so the caller
// says WHICH, and the plate never claims the file is empty.
//
// The standing line above it is a fact about this surface and not a seed
// string: the mirror is a fetch of the node's own forge remote, and the app
// ships no editor for it.
component ForgeCodeEmpty(name:str, note:str)
  box #root w=fill p=48.0 align-x=center
    col gap=9.0 align=center
      if !empty(name)
        text name size=14.0 wrap=none font=code_semibold @text-caption
      text "Synced from the node · view only" size=11.5 wrap=none @text-label
      if !empty(note)
        text note size=11.5 wrap=none @text-label

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
    button label="Open item" description=item.title w=fill p=0.0 @icon_action -> emit(forge_open_item, item.number)
      box w=fill pl=24.0 pr=24.0 pt=13.0 pb=13.0
        row w=fill gap=13.0 align=start
          match item.kind
            "pr"
              PrStatePlate state=item.state
            "issue"
              IssueStateGlyph state=item.state
          col w=fill gap=4.0
            text item.title w=fill size=14.0 wrap=none font=display @text-primary
            row gap=5.0 align=center
              row gap=0.0 align=center
                text "#" size=11.0 wrap=none font=code_medium @text-meta
                text item.number size=11.0 wrap=none font=code_medium @text-meta
              text "· opened by" size=11.0 wrap=none font=code_medium @text-meta
              text item.author_name size=11.0 wrap=none font=code_medium @text-meta
      active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
      hovered bg=row_hover text=fg
      pressed bg=elevated text=fg
    box w=fill h=1.0 bg=elevated
      space w=1.0 h=1.0

// The 24px state square: one glyph, three plates.
component PrStatePlate(state:str)
  col #root
    match state
      "open"
        box w=24.0 h=24.0 align-x=center align-y=center bg=success_bg border=success_line border-w=1.0 r=7.0
          Icon name="pull-request" tone="success" px=13.0
      "merged"
        box w=24.0 h=24.0 align-x=center align-y=center bg=merged_bg border=merged_line border-w=1.0 r=7.0
          Icon name="pull-request" tone="muted" px=13.0
      _
        box w=24.0 h=24.0 align-x=center align-y=center bg=elevated border=border border-w=1.0 r=7.0
          Icon name="pull-request" tone="muted" px=13.0

// Issues carry their state in the glyph itself — the one place the artifact
// swaps the mark instead of the plate.
component IssueStateGlyph(state:str)
  col #root
    match state
      "open"
        Icon name="issue-open" tone="success" px=17.0
      _
        Icon name="issue-closed" tone="muted" px=17.0

// ── ITEM DETAIL ───────────────────────────────────────────────────────────

// The back control names the list it returns to, rather than saying `Back`.
component BackToList(kind:str)
  emits
    forge_close_item
  col #root
    match kind
      "pr"
        button label="Back to pull requests" p=0.0 @icon_action -> emit(forge_close_item)
          box pl=7.0 pr=9.0 pt=4.0 pb=4.0
            row gap=5.0 align=center
              text "‹" size=14.0 wrap=none @text-muted
              text "Pull requests" size=12.0 wrap=none @text-muted
          active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
          hovered bg=row_hover text=fg
          pressed bg=elevated text=fg
      "issue"
        button label="Back to issues" p=0.0 @icon_action -> emit(forge_close_item)
          box pl=7.0 pr=9.0 pt=4.0 pb=4.0
            row gap=5.0 align=center
              text "‹" size=14.0 wrap=none @text-muted
              text "Issues" size=12.0 wrap=none @text-muted
          active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
          hovered bg=row_hover text=fg
          pressed bg=elevated text=fg

// The detail header's state pill — the same three plates as the row square,
// with the state said in a word.
component PrStatePill(state:str)
  col #root
    match state
      "open"
        box px=11.0 py=5.0 bg=success_bg border=success_line border-w=1.0 r=8.0
          row gap=6.0 align=center
            Icon name="pull-request" tone="success" px=13.0
            text "Open" size=12.0 wrap=none font=display @text-success
      "merged"
        box px=11.0 py=5.0 bg=merged_bg border=merged_line border-w=1.0 r=8.0
          row gap=6.0 align=center
            Icon name="pull-request" tone="muted" px=13.0
            text "Merged" size=12.0 wrap=none font=display @text-merged
      _
        box px=11.0 py=5.0 bg=elevated border=border border-w=1.0 r=8.0
          row gap=6.0 align=center
            Icon name="pull-request" tone="muted" px=13.0
            text "Closed" size=12.0 wrap=none font=display @text-muted

// `+284 −96 · 7 files` — the artifact colours the two counts and greys the file
// tally; `forge_stats` collapsed all three into one muted string. `files=0`
// drops the tally, which is how the diff card's own header wears it.
component DiffCount(additions:i64, deletions:i64, files:i64)
  row #root gap=6.0 align=center
    row gap=0.0 align=center
      text "+" size=10.5 wrap=none font=code_medium @text-success
      text additions size=10.5 wrap=none font=code_medium @text-success
    row gap=0.0 align=center
      text "−" size=10.5 wrap=none font=code_medium @text-alert_fg
      text deletions size=10.5 wrap=none font=code_medium @text-alert_fg
    if files > 0
      row gap=4.0 align=center
        text "·" size=10.5 wrap=none font=code_medium @text-caption
        text files size=10.5 wrap=none font=code_medium @text-caption
        text "files" size=10.5 wrap=none font=code_medium @text-caption

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
component IssueBodyCard(author:str, body:str)
  box #root w=fill max-w=660.0 bg=surface border=card_line border-w=1.0 r=11.0 clip=true
    col w=fill
      box w=fill pl=13.0 pr=13.0 pt=8.0 pb=8.0 bg=card_wash
        row w=fill gap=7.0 align=center
          text "opened by" size=12.0 wrap=none @text-caption
          text author size=12.0 wrap=none font=display @text-primary
      box w=fill h=1.0 bg=separator
        space w=1.0 h=1.0
      box w=fill pl=15.0 pr=15.0 pt=13.0 pb=13.0
        text body w=fill size=13.0 line-h=1.6 @text-accent_fg

// ── MERGE ─────────────────────────────────────────────────────────────────

// Merged wears the violet plate, and the note is this chain's own merge fact
// (`Merged as <oid> · <branches>`), not the artifact's seeded event name.
component MergedBanner(note:str)
  row #root w=fill gap=9.0 align=center
    box w=24.0 h=24.0 align-x=center align-y=center bg=merged_bg border=merged_line border-w=1.0 r=7.0
      text "✓" size=12.0 wrap=none font=code_semibold @text-merged
    text note w=fill size=13.0 wrap=none font=display @text-merged

// The advisory above the merge button, and the ONLY thing said there. It is a
// recommendation, never a refusal: `ForgeMsg::MergePr` runs `author_from_origin`
// and nothing else — no valset, tier or role check — so a request for changes
// cannot and does not stop the write. The artifact pairs this with a check-run
// state that does not exist in this forge, so only the reviewer half is drawn.
component MergeAdvisory(change_requests:i64)
  col #root w=fill
    if change_requests == 1
      row w=fill gap=7.0 align=center
        box w=6.0 h=6.0 bg=warning_dot r=3.0
          space w=1.0 h=1.0
        text "a reviewer requested changes — merge not recommended" w=fill size=12.0 @text-warning
    if change_requests > 1
      row w=fill gap=7.0 align=center
        box w=6.0 h=6.0 bg=warning_dot r=3.0
          space w=1.0 h=1.0
        text change_requests size=11.0 wrap=none font=code_medium @text-warning
        text "reviewers requested changes — merge not recommended" w=fill size=12.0 @text-warning

// The merge write, on the ink plate with the glyph the artifact gives it.
component MergeButton(busy:bool, disabled:bool)
  emits
    forge_merge_submit
  col #root
    if busy
      button label="Merging" disabled=true @primary_action px-18px py-9px rounded-9px -> emit(forge_merge_submit)
        row gap=7.0 align=center
          text "Merging…" size=13.0 wrap=none font=display @text-primary_fg
    if !busy
      button label="Merge pull request" disabled=disabled @primary_action px-18px py-9px rounded-9px -> emit(forge_merge_submit)
        row gap=7.0 align=center
          Icon name="pull-request" tone="paper" px=13.0
          text "Merge pull request" size=13.0 wrap=none font=display @text-primary_fg

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
  box #root w=fill max-w=720.0 bg=surface border=card_line border-w=1.0 r=11.0 clip=true
    col w=fill
      box w=fill pl=14.0 pr=14.0 pt=10.0 pb=10.0 bg=card_wash
        row w=fill gap=9.0 align=center
          Icon name="branch" tone="muted" px=13.0
          text file size=12.0 wrap=none font=code_semibold @text-accent_fg
          DiffCount additions=additions deletions=deletions files=0
          space w=fill
      box w=fill h=1.0 bg=separator
        space w=1.0 h=1.0
      col w=fill
        for line in lines
          DiffRow line=line

// One patch line. The kind is the whole discriminant: a file header, a hunk
// header, or a code row whose gutter, sign and ink are its tint.
component DiffRow(line:DiffLine)
  col #root w=fill
    match line.kind
      "file"
        box w=fill pl=14.0 pr=14.0 pt=5.0 pb=5.0 bg=card_wash
          text line.text w=fill size=11.0 wrap=none font=code_medium @text-caption
      "hunk"
        box w=fill pl=14.0 pr=14.0 pt=5.0 pb=5.0 bg=diff_hunk_bg
          text line.text w=fill size=11.0 wrap=none font=code_medium @text-merged
      "add"
        box w=fill bg=diff_add_bg
          row w=fill gap=0.0 align=center
            box w=34.0 h=20.0 pr=8.0 align-y=center bg=diff_add_gutter
              text line.old_no w=fill size=12.0 wrap=none align-x=right font=code @text-gutter_ink
            box w=34.0 h=20.0 pr=8.0 align-y=center bg=diff_add_gutter
              text line.new_no w=fill size=12.0 wrap=none align-x=right font=code @text-gutter_ink
            box w=14.0 h=20.0 align-x=center align-y=center
              text line.sign size=12.0 wrap=none font=code @text-diff_add_fg
            box w=fill h=20.0 pr=12.0 align-y=center
              text line.text w=fill size=12.0 wrap=none font=code @text-diff_add_fg
      "del"
        box w=fill bg=diff_del_bg
          row w=fill gap=0.0 align=center
            box w=34.0 h=20.0 pr=8.0 align-y=center bg=diff_del_gutter
              text line.old_no w=fill size=12.0 wrap=none align-x=right font=code @text-gutter_ink
            box w=34.0 h=20.0 pr=8.0 align-y=center bg=diff_del_gutter
              text line.new_no w=fill size=12.0 wrap=none align-x=right font=code @text-gutter_ink
            box w=14.0 h=20.0 align-x=center align-y=center
              text line.sign size=12.0 wrap=none font=code @text-diff_del_fg
            box w=fill h=20.0 pr=12.0 align-y=center
              text line.text w=fill size=12.0 wrap=none font=code @text-diff_del_fg
      "ctx"
        box w=fill bg=surface
          row w=fill gap=0.0 align=center
            box w=34.0 h=20.0 pr=8.0 align-y=center bg=card_wash
              text line.old_no w=fill size=12.0 wrap=none align-x=right font=code @text-gutter_ink
            box w=34.0 h=20.0 pr=8.0 align-y=center bg=card_wash
              text line.new_no w=fill size=12.0 wrap=none align-x=right font=code @text-gutter_ink
            box w=14.0 h=20.0 align-x=center align-y=center
              text line.sign size=12.0 wrap=none font=code @text-panel_tile
            box w=fill h=20.0 pr=12.0 align-y=center
              text line.text w=fill size=12.0 wrap=none font=code @text-panel_tile

// ── REVIEWS ───────────────────────────────────────────────────────────────

// A submitted review, stamped with the height it settled at. `created_at` IS
// that height on this chain, and it has been loaded and thrown away until now.
// A review that is in state is finalized by construction, so the chip claims
// exactly what it can prove.
component ReviewCard(review:ForgeReview)
  box #root w=fill pl=13.0 pr=13.0 pt=11.0 pb=11.0 bg=surface border=card_line border-w=1.0 r=10.0
    col w=fill gap=6.0
      row w=fill gap=7.0 align=center
        text review.author_name size=13.0 wrap=none font=display @text-primary
        ReviewVerdict verdict=review.verdict
        text review.commit size=11.0 wrap=none font=code_medium @text-hint
        if review.outdated
          box px=6.0 py=2.0 bg=elevated r=4.0
            text "outdated" size=9.0 wrap=none font=code_semibold @text-meta
        space w=fill
        FinalityChip phase="finalized" height=review.created_at
      if !empty(review.body)
        text review.body w=fill size=13.0 line-h=1.55 @text-accent_fg
      for comment in review.comments
        box w=fill pl=11.0 pr=11.0 pt=9.0 pb=9.0 bg=brand_wash border=brand_line border-w=1.0 r=9.0
          col w=fill gap=4.0
            row w=fill gap=7.0 align=center
              text comment.anchor w=fill size=11.0 wrap=none font=code_medium @text-brand
              text "review comment" size=10.0 wrap=none font=code_semibold @text-label
            text comment.body w=fill size=12.0 line-h=1.55 @text-panel_tile

// The verdict in its own tone: approval is the success ink, a change request is
// the refusal ink, a comment is neither.
component ReviewVerdict(verdict:str)
  col #root
    match verdict
      "approve"
        text verdict_label(verdict) size=10.5 wrap=none font=code_medium @text-success
      "request_changes"
        text verdict_label(verdict) size=10.5 wrap=none font=code_medium @text-alert_fg
      _
        text verdict_label(verdict) size=10.5 wrap=none font=code_medium @text-meta
