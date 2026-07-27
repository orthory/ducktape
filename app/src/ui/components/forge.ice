// FORGE — the repo grid, the breadcrumb, the tracker rows, the painted diff,
// and the review stamps. Everything the artifact draws for this screen that
// `ForgeQuery` can actually answer today.
//
// WHAT IS DELIBERATELY NOT HERE. The artifact's forge is richer than the wire:
// label pills, check runs, reviewer digests, assignees, comment counts, a
// per-repo language dot / about line / updated stamp, the agent-activity chips,
// the conversation timeline and the Code tab's file tree each need a NEW field
// on `ForgeQuery`/`ItemSummary`/`RepoHead`. A design-parity pass does not
// smuggle a consensus-module wire change, so none of them is drawn — and none
// is faked with a plausible placeholder either.
//
// STATE IS A PLATE, NOT AN ICON. There is ONE pull-request glyph; open, merged
// and draft are the plate behind it (`success_bg`, `merged_bg`, `elevated`).
// Issues are the only exception the artifact makes: it swaps the whole glyph
// for `issue-open` / `issue-closed`, and so does this.
//
// TIME IS A HEIGHT. A review's `created_at` IS its block height — this chain
// stamps `consensus_time = height` — so a review is stamped with `FinalityChip`
// at that height. No wall clock appears anywhere on this screen.

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

// One repo card. The title is org-qualified the way the artifact writes it, and
// the only meta the wire carries is the head digest — the language dot, the
// PR/issue tallies and the `updated` stamp all want fields `RepoHead` does not
// have, so the row holds what is true instead of what would look full.
component RepoCard(repo:ForgeRepo)
  button label="Open repo" description=repo.name w=fill p=0.0 @icon_action -> forge_open_repo(repo.name)
    box w=fill pl=17.0 pr=17.0 pt=15.0 pb=15.0
      col w=fill gap=10.0
        row w=fill gap=8.0 align=center
          Icon name="branch" tone="muted" px=14.0
          row gap=0.0 align=center
            text "ducktape/" size=14.0 wrap=none font=display @text-primary
            text repo.name size=14.0 wrap=none font=display @text-primary
        row w=fill gap=14.0 align=center
          text repo.head w=fill size=10.5 wrap=none font=code_medium @text-input
    active bg=surface text=fg border=card_line border-w=1.0 r=13.0
    hovered bg=card_wash_hover text=fg border=pending_line
    pressed bg=elevated text=fg

// ── REPO HEADER ───────────────────────────────────────────────────────────

// `ducktape / <repo> ▾` plus the single default-branch pill. This replaces both
// the generic screen header AND the every-branch chip scroller, which occupied
// the row the artifact gives to the breadcrumb.
//
// CHROME ONLY, ON PURPOSE. Making the two crumbs pressable needs a
// `forge_close_repo` and a `forge_toggle_repo_menu` handler that
// `handlers/lifecycle.ice` does not have yet; `open` is the switcher's state,
// which lights the repo name the way the artifact's hover does.
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
  col #root w=fill
    if active
      button label="Switch repo" description=repo.name w=fill p=0.0 @icon_action -> forge_open_repo(repo.name)
        box w=fill pl=9.0 pr=9.0 pt=8.0 pb=8.0
          row w=fill gap=9.0 align=center
            text repo.name w=fill size=13.0 wrap=none font=display @text-primary
        active bg=elevated text=fg border=transparent border-w=1.0 r=8.0
        hovered bg=elevated text=fg
        pressed bg=subtle text=fg
    if !active
      button label="Switch repo" description=repo.name w=fill p=0.0 @icon_action -> forge_open_repo(repo.name)
        box w=fill pl=9.0 pr=9.0 pt=8.0 pb=8.0
          row w=fill gap=9.0 align=center
            text repo.name w=fill size=13.0 wrap=none font=display @text-primary
        active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
        hovered bg=elevated text=fg
        pressed bg=subtle text=fg

// ── TRACKER ───────────────────────────────────────────────────────────────

// One tracker row, for both kinds. The meta line is `#N · opened by <author>`:
// the artifact also prints the source branch and an `opened <rel>` stamp, and
// `ItemRow` carries neither, so neither is invented. The AGENT badge is missing
// for the same reason — `ItemRow` drops the authorship kind that
// `chat::client::avatar_kind` already derives.
component TrackerRow(item:ForgeItem)
  col #root w=fill
    button label="Open item" description=item.title w=fill p=0.0 @icon_action -> forge_open_item(item.number)
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
  col #root
    match kind
      "pr"
        button label="Back to pull requests" p=0.0 @icon_action -> forge_close_item
          box pl=7.0 pr=9.0 pt=4.0 pb=4.0
            row gap=5.0 align=center
              text "‹" size=14.0 wrap=none @text-muted
              text "Pull requests" size=12.0 wrap=none @text-muted
          active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
          hovered bg=row_hover text=fg
          pressed bg=elevated text=fg
      "issue"
        button label="Back to issues" p=0.0 @icon_action -> forge_close_item
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

// The issue body as an authored card: a header strip that attributes it, then
// the body. The artifact hangs a 30px avatar beside it whose SHAPE says whether
// the author is a person or a machine — `ItemDetail` carries the author's
// display name and not the handle it is derived from, so the plate would have
// to guess. It is left out rather than drawn as a lie.
component IssueBodyCard(author:str, body:str)
  box #root w=fill max-w=660.0 bg=surface border=card_line border-w=1.0 r=11.0 clip=true
    col w=fill
      box w=fill pl=13.0 pr=13.0 pt=8.0 pb=8.0 bg=card_wash
        row w=fill gap=7.0 align=center
          text author size=12.0 wrap=none font=display @text-primary
          text "opened this issue" size=12.0 wrap=none @text-caption
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

// The advisory above the merge button. The screen used to state the OPPOSITE
// unconditionally — `Approvals are advisory — merging is never gated` — while a
// reviewer's request for changes sat one card above it. This says the true half
// the wire supports; the artifact's other half is a check-run state that does
// not exist in this forge.
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
  col #root
    if busy
      button label="Merging" disabled=true @primary_action px-18px py-9px rounded-9px -> forge_merge_submit
        row gap=7.0 align=center
          text "Merging…" size=13.0 wrap=none font=display @text-primary_fg
    if !busy
      button label="Merge pull request" disabled=disabled @primary_action px-18px py-9px rounded-9px -> forge_merge_submit
        row gap=7.0 align=center
          Icon name="pull-request" tone="paper" px=13.0
          text "Merge pull request" size=13.0 wrap=none font=display @text-primary_fg

// WHY a forge action is unavailable, in the chain's own tiers. `forge_gate`
// returns "" when this node may write, so the note renders nothing at all for a
// validator — a refusal plate over an action you are allowed to take is worse
// than no plate.
component ForgeGateNote(gate:str)
  col #root w=fill
    match gate
      "resident_cannot_merge"
        GateNote reason="This node is a resident — it may open, comment and review, but the node refuses the merge write itself." next="Merging needs a validator seat on this network."
      "guest_read_only"
        GateNote reason="This node is a guest — every forge write is refused at the node, not merely disabled here." next="A resident may comment and review; merging needs a validator seat."

// ── DIFF ──────────────────────────────────────────────────────────────────

// The painted patch: a header strip with the coloured counts, hunk headers on
// their own plate, twin 34px gutters, the sign column, and a per-line tint.
// `forge_item_diff` already holds the unified patch — this is the renderer it
// never had.
component DiffPane(file:str, additions:i64, deletions:i64, lines:[DiffLine])
  box #root w=fill max-w=720.0 bg=surface border=card_line border-w=1.0 r=11.0 clip=true
    col w=fill
      box w=fill pl=14.0 pr=14.0 pt=10.0 pb=10.0 bg=card_wash
        row w=fill gap=9.0 align=center
          Icon name="file" tone="muted" px=13.0
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
