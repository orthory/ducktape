// THE BLOCK EXPLORER, as a module-owned view: the ledger the desktop app
// pushes — blocks, their ops, the head — plus one workspace search the app
// runs on the view's behalf. The screen body is the app's own
// (screens/storage.ice's ExplorerScreen before the port); the search state
// that used to live in the component is split: the draft, the kind filter
// and the selected block stay here, the answer comes back as props.
app ExplorerView
  title "Explorer"
  palette active_palette
  id "dev.ducktape.view.explorer"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"
use "../../../../../app/src/ui/ducktape-ui/recipes.ice"
use "../../../../../app/src/ui/components/icon.ice"
use "kit.ice"

extern crate::host
  HostError(message:str)
  ExplorerBlock(height:i64, hash:str, commit:str, op_count:i64)
  ExplorerOp(height:i64, proposer:str, target:str, disposition:str, op_hash:str, payload:str, trace:str)
  ExplorerHit(kind:str, code:str, title:str, snippet:str, meta:str, target:str)
  KindCount(kind:str, label:str, count:i64)
  ExplorerProps(connected:bool, loading:bool, dark:bool, blocks:[ExplorerBlock], ops:[ExplorerOp], head:i64, sync_line:str, hits:[ExplorerHit], kinds:[KindCount], partial:str, searching:bool, sent_query:str)
  stream props() -> ExplorerProps ! HostError
  pure refresh_ledger() -> bool
  pure copy(text:&str, label:&str) -> bool
  pure search(query:&str) -> bool
  pure clear_search() -> bool
  pure icon(name:&str) -> bytes
  pure explorer_ops_at(ops:&[ExplorerOp], height:i64) -> [ExplorerOp]
  pure height_label(height:i64) -> str
  pure plural(count:i64, one:&str, many:&str) -> str
  pure search_answer_stands(query:&str, draft:&str, searching:bool) -> bool

state
  active_palette:palette[AppTheme] = AppTheme.app
  connected = false
  loading = false
  blocks:[ExplorerBlock] = []
  ops:[ExplorerOp] = []
  head:i64 = 0
  sync_line = ""
  // the answer to the last search the app ran for this view
  hits:[ExplorerHit] = []
  kinds:[KindCount] = []
  partial = ""
  searching = false
  // the query that answer is speaking for, "" while none stands
  sent_query = ""
  // the reader's own: the draft, the kind filter, the block they opened
  query = ""
  kind = "all"
  selected:i64 = 0
  host_error = ""
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false

on mount
  stream every props() -> props_changed _ | props_failed _

on props_changed(next)
  connected = next.connected
  loading = next.loading
  blocks = next.blocks
  ops = next.ops
  head = next.head
  sync_line = next.sync_line
  hits = next.hits
  kinds = next.kinds
  partial = next.partial
  searching = next.searching
  sent_query = next.sent_query
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

on props_failed(error)
  host_error = error.message

on refresh
  sent = refresh_ledger()

on copy_to_clipboard(text, label)
  sent = copy(text, label)

// ENTER-TO-SUBMIT: the box is two-way bound with no `change=` route, so a
// keystroke writes `query` and runs nothing; the app runs the search and
// answers with `hits`, `kinds`, `partial` and the `sent_query` it stands for.
on search_submit
  let blocked = !connected || searching || empty(trim(query))
  return if blocked
  kind = "all"
  sent = search(trim(query))

on clear_explorer_search
  query = ""
  kind = "all"
  sent = clear_search()

on pick_explorer_kind(next)
  kind = next

on select_explorer_block(height)
  selected = height

view
  box #root
    with
      w=fill
      h=fill
      bg=bg
    col w=fill h=fill
      col
        with
          w=fill
          pl=24.0
          pr=24.0
          pt=22.0
          gap=16.0
        // THE LEDGER IS NOT THE CHAIN, and this line used to say it was. It
        // claimed "the blocks this node verified for itself" while the rows
        // below are the node's op-carrying blocks ONLY: `GET /v1/blocks` serves
        // the derived-index rows, and `bin/noded`'s projection stores `None` for
        // a block whose members are all the `consensus.nop` heartbeat — "a pure
        // nop/idle block — the explorer hides it". Measured on the demo node:
        // `/v1/status` height 419718, and the hundred rows `/v1/blocks` served
        // spanned heights 102907-366045 — a hundred-row "recent" window covering
        // 263k heights, which no lag explains. A reader who compares the newest
        // row against the height in the titlebar concludes the node is fifty
        // thousand blocks behind. The list was right; the sentence was wrong.
        ScreenTitle
          with
            title="Explorer"
            detail="Search everything this workspace has recorded, or read the blocks that carried operations — an idle block keeps no row, so heights skip — newest first, each one openable for the ops it carried."
        // THE LIVE HEAD, not the newest row of the window below it.
        //
        // The list is op-carrying blocks only, so its top row lags the chain by
        // however many idle blocks have gone by — on a quiet chain, forever.
        // This register moves on the ws heartbeat, every block, nop fillers
        // included, which is the thing a reader is actually watching for. The
        // sync reading rides beside it because a head that is not advancing and
        // a node that is still catching up are different facts.
        row
          with
            w=fill
            gap=10.0
            align=center
          text height_label(head) size=13.0 @text-muted
          if !empty(sync_line)
            text sync_line size=13.0 @text-muted
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
                    submit=search_submit
                    w=fill
                    p=6.2
                    text-size=13.0
                    line-h=1.2
                    @control
                  active bg=transparent border=transparent value=fg placeholder=hint selection=fg/18 border-w=0.0 r=0.0
                  hovered bg=transparent border=transparent
                  disabled value=muted
                if !empty(trim(query))
                  button #explorer-clear -> clear_explorer_search
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
            button "Refresh" -> refresh
              with
                label="Refresh"
                disabled=loading
                h=30.0
                p=7.0
                @outline_action
        // A PARTIAL ANSWER SAYS SO. Each of the six sources behind a
        // workspace search fails silently, and the node's per-module cold
        // start runs tens of seconds against a 30s client ceiling — so a
        // search that reached the node and timed out on three of them used
        // to render the survivors as the whole truth: a confident count, a
        // full chip strip reading 0 for what was never read, and "Nothing
        // matched that query in this workspace" when the survivors were
        // empty. This line names what went unanswered; the strip below
        // drops those kinds rather than count them, and the empty plate
        // stands down.
        if connected && !empty(partial)
          box w=fill max-w=860.0
            box #explorer-partial
              with
                w=fill
                pl=12.0
                pr=12.0
                pt=8.0
                pb=8.0
                bg=warning_bg
                border=warning_line
                border-w=1.0
                r=9.0
              text partial
                with
                  w=fill
                  size=12.0
                  @text-warning
        // THE KIND STRIP. Drawn FROM the reply, never from a fixed list
        // of labels: every chip here names a kind `search_workspace`
        // genuinely ran, so a count of 0 means "nothing matched", never
        // "no loader". TASKS IS BACK — it was cut for having no text
        // search, but the tasks index answers by status and the filtering
        // is ours to do, which is the same deal every other kind takes.
        // The kind filter itself is client-side over hits already in
        // hand: a second round trip to narrow a list you are holding is
        // waste.
        if connected && !empty(kinds)
          box w=fill max-w=860.0
            flex
              with
                w=fill
                wrap=wrap
                gap-x=7.0
                gap-y=7.0
                items=start
              button -> pick_explorer_kind("all")
                with
                  label="Show every result"
                  checked=(kind == "all")
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
                button -> pick_explorer_kind(kind_count.kind)
                  with
                    label="Filter results by kind"
                    description=kind_count.label
                    checked=(kind == kind_count.kind)
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
        if connected && !empty(hits)
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
        if connected && !empty(hits)
          for kind_count in kinds
            if kind == kind_count.kind && kind_count.count <= 0
              EmptyPlate
                with
                  message="Nothing of that kind matched — the other chips still hold results."
        // "Nothing matched" is a claim about the WORKSPACE, and only the sources
        // that answered can support it. With `partial` standing, the banner above
        // already says why the screen is empty.
        //
        // ON THE QUERY THAT WAS SENT, NOT ON THE ONE IN THE BOX: keyed on the
        // live draft, one more keystroke after a zero-hit answer re-aimed this
        // sentence at a string the node was never asked about.
        // `search_answer_stands` is that comparison, shared with pages and chat.
        if connected && empty(hits) && empty(partial) && search_answer_stands(sent_query, query, searching)
          EmptyPlate message="Nothing matched that query in this workspace." #explorer-nothing-matched
        // NOT CONNECTED IS NOT EMPTY. `connected` already disables the query box
        // above; the ledger below it still asserted "No blocks yet" off a node
        // that answered nothing. The head and the query box stay.
        if !connected
          EmptyState
            with
              title="Not connected"
              description="Click the network name in the titlebar to pick or reconnect a network."
        if connected && empty(hits) && empty(blocks) && !loading && empty(trim(query))
          // Same set, same words as the subtitle above. This plate already said
          // "non-empty", which was right and was the only place on the screen
          // that knew — two dialects for one set is how the subtitle drifted
          // without anyone noticing.
          EmptyState
            with
              title="No blocks yet"
              description="Blocks that carried operations appear here as they finalize."
        // The ledger itself, which the plate above replaces rather than sits
        // over: with the node down these are blocks from a chain nobody read.
        if connected && empty(hits) && !empty(blocks)
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
                    ExplorerBlockRow block selected=(block.height == selected)
                      events
                        select_explorer_block -> select_explorer_block _
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
                            // TWO HASHES, ONE SCREEN, NEITHER NAMED. The list
                            // on the left prints `block.hash` (the frame id);
                            // this prints `op.op_hash` (the sha256 of the op
                            // payload — `project_root_op` keys it by
                            // `put_chunk`, which is also the `GET
                            // /v1/files/blob/{op_hash}` key). They never match,
                            // and an unlabelled hex that changes when you open
                            // a row reads as a contradiction. The label form is
                            // the `by` beside the proposer, one row down.
                            //
                            // `hash`, not `op`: this pane sits beside a list
                            // whose third column counts `1 op` / `3 ops`, and
                            // one screen must not spend the same word on a
                            // count noun and a field name. Inside an op card
                            // `hash` can only mean this op's, and the block
                            // hash it could be confused with carries no label
                            // to collide with.
                            //
                            // The hash is FULL and lives on its OWN row: it is
                            // the blob key (QA: the explorer never exposed the
                            // whole thing), and 64 code chars at 12.0 are
                            // ~461px against the pane's 532px minimum (1040 −
                            // 74 rail − 48 screen padding − 340 list − 10 gap
                            // − 18 box − 18 card) — too wide to share a row
                            // with the target, wide enough to own one.
                            // `word-or-glyph` wraps rather than clips anything
                            // narrower. Clicking it copies the key whole.
                            row
                              with
                                w=fill
                                gap=8.0
                                align=center
                              text "hash"
                                with
                                  size=11.0
                                  wrap=none
                                  font=code_medium
                                  @text-muted
                              button -> copy_to_clipboard(op.op_hash, "Op hash copied")
                                with
                                  label="Copy op hash"
                                  p=2.0
                                  @ghost_action
                                text op.op_hash
                                  with
                                    size=12.0
                                    wrap=word-or-glyph
                                    font=code
                                    @text-muted
                                active bg=transparent text=fg border=transparent border-w=1.0 r=5.0
                                hovered bg=row_hover text=fg
                                pressed bg=accent
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
                            // `chat(+0m/+0e)` sat here naked. `dispatch` is the
                            // word this screen's own "Select a block" plate
                            // already uses for it ("Its operations and dispatch
                            // traces appear here"); the units come out of
                            // `explorer_trace` now instead of being a legend the
                            // reader has to have been told.
                            if !empty(op.trace)
                              row
                                with
                                  w=fill
                                  gap=8.0
                                  align=start
                                text "dispatch"
                                  with
                                    size=11.0
                                    wrap=none
                                    font=code_medium
                                    @text-muted
                                text op.trace
                                  with
                                    w=fill
                                    size=12.0
                                    font=code
                                    @text-muted
                            // Pretty-printed JSON (or verbatim text) from
                            // `explorer_payload` — a code plane, so it reads in
                            // the code face, whole, wrapping instead of
                            // clipping.
                            text op.payload
                              with
                                w=fill
                                size=12.0
                                wrap=word-or-glyph
                                font=code
                                @text-fg

component ExplorerBlockRow(block:ExplorerBlock, selected:bool)
  emits
    select_explorer_block(i64)
  col #root w=fill
    if selected
      button -> emit(select_explorer_block, block.height)
        with
          label="Inspect block"
          checked=selected
          w=fill
          p=6.0
          @ghost_action
        ExplorerBlockFace block=block
        active bg=selected_row text=fg border=transparent border-w=1.0 r=7.0
        hovered bg=row_hover text=fg
        pressed bg=accent
    if !selected
      button -> emit(select_explorer_block, block.height)
        with
          label="Inspect block"
          checked=selected
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
