# Pages — One Document — Specification of Record

Status: specification of record, **as shipped** through #913 → #914 → #934
(2026-08-06). This is the first written spec for the Pages surface — #913 and
#914 were both built without one — so it records the contract the code now
implements, the invariants the tests pin, and the deliberate gaps. Amend this
document when the contract moves; it is the acceptance baseline for any future
Pages change.

The QA verdict that produced this shape (recorded verbatim in the #914 PR
body) rejected every form of click-to-edit: a line that is a button until
clicked, a block-type dropdown parked at the margin, a `/` kind menu, a
`+`/`⋮⋮` hover cluster, and comments buried behind a hover menu. **Any design
that reintroduces one of those is rejected by prior QA, not merely by taste.**
Source-parsing tests in `app/src/tests.rs`
(`the_page_surface_is_one_editor_with_no_click_to_edit_left`) refuse their
return mechanically.

## 1. Surface model

- A page is **one document**: a single `ui_lang_runtime::RichTextEditor`
  mounted as the Ice extern `page_document` over the page's markdown
  (`app/src/pages/mod.rs`, mounted in `app/src/ui/screens/pages.ice`). The
  caret lands where you click; there is nothing to select first. `# ` IS the
  block-type menu.
- **The title is line 0 of the same buffer.** It is a page property on the
  wire (the page block's own text), not a body block — but in the buffer it is
  simply the first line, so Enter at its end and Backspace at the body's start
  are ordinary edits. Line 0 is never parsed as markdown
  (`markdown_on_the_title_line_stays_literal_…` in `app/src/pages/sync.rs`).
- **Subpages are navigation, not prose.** They have no markdown spelling,
  never enter the buffer, and are listed under the document
  (`is_prose`, `subpage_blocks`). A text diff therefore can never decide a
  subpage was deleted.
- The editor **fills the canvas and scrolls itself** (`.height(Fill)`, no
  outer `scroll`). An outer scrollable hands the widget infinite height, which
  turns its caret-reveal into a no-op — typing below the fold would walk the
  caret off screen. This is a layout invariant, not a styling choice.
- The stock Ice `editor` widget cannot serve this surface: iced's
  `highlighter::Format` is color+font only (no per-span size, no plates).
  Everything below assumes `RichTextEditor`'s `Format`
  (size / line_height / highlight / line_highlight / line_padding /
  strikethrough).

## 2. The markdown projection (`app/src/pages/sync.rs`)

One block ↔ one line, except Code (one block ↔ fence + body lines + fence).
The grammar is the CHAT renderer's, not CommonMark: pages and chat must agree
on inline marks, so `crate::editor::inline_marks` is the single inline parser
and no markdown dependency exists.

| Block kind | Line spelling | Notes |
|---|---|---|
| Heading 1–3 | `# ` / `## ` / `### ` | 4+ hashes are prose; `#tag` is prose |
| Bullet | `- ` (also reads `* `) | renders back as `- ` (normalization) |
| Number | `1. ` (also reads `1) `) | stored number is positional; **≤ 2 digits** reads as a list — `1997. A great year` stays prose, because the digits would otherwise be destroyed on the round trip |
| Todo | `- [ ] ` / `- [x] ` / `- [X] ` | `checked` is a Todo-only fact |
| Toggle | `+ ` | a legal CommonMark bullet reserved so the kind survives round trips |
| Quote | `> ` | |
| Callout | `!> ` | extension; Callout predates this surface and must not silently degrade to Quote |
| Divider | `---` | |
| Code | ```` ``` ```` fence pair | body is **verbatim** past the fence's own indent; rendered with `split('\n')`, never `.lines()`, so a trailing newline is content |
| Text | anything else | an EMPTY line is an empty Text block — Enter-Enter must be writable |

**Depth.** Two spaces or one tab per nesting step (`split_indent`); a leftover
odd space belongs to the text. Depth is **clamped at parse** to the previous
line's depth + 1 and the first body line to 0 — the only shapes the tree can
hold. An unclamped depth becomes a `MoveBlock` the module rejects forever
(§4). Parse with `split('\n')`, never `.lines()`: the final empty line of a
document IS a block.

**Round-trip invariants** (pinned in `sync.rs` tests):
- every kind survives render→parse unchanged;
- code bodies keep their own indentation and trailing newline;
- markdown inside a fence is code, never structure;
- `page_text` (`app/src/pages/mod.rs`) is **verbatim** `Content::text()` —
  iced 0.14's `text()` is a pure join with NO synthetic trailing newline, so
  trimming would delete the final empty block just for opening the page.

**Accepted normalizations** (documented behavior, not defects): `* ` → `- `,
tabs → two-space steps, a paragraph typed with a marker prefix converts kind
on the next save (the same gesture the type menu used to perform), a
multi-line paragraph arriving from another writer splits into N blocks.

## 3. Paint (`app/src/pages/markdown.rs`)

- **Syntax carries the formatting and hides itself off the caret's line.**
  Markers paint transparent at 0.01px (`HIDDEN_SIZE` — a zero-size span drops
  out of the shaped run and takes the caret's column with it). List markers
  stay visible: they are the bullet the reader sees.
- Metrics are the Pages design tokens: title 22/1.15, H1 20/1.25, H2 16/1.3,
  H3 14/1.35, body 14/1.65, quote 14/1.6 italic muted, callout 13/1.6 on its
  tile, code 12/1.6 mono on its plate.
- **Every plate is a translucent wash — never opaque.** The runtime paints a
  fully opaque `line_highlight` ABOVE its own glyphs (ducktape-ui bug,
  reproduced in the pinned `examples/markdown-editor` by setting its wash's
  alpha to 1.0; upstream fix outstanding). Code plate, callout tile and the
  comment wash are ink-at-low-alpha washes for this reason AND for the #927
  design verdict (quiet wash + hairline, never a slab).
- The highlighter's `Settings` is the caret (line/column/dark) **plus the
  commented line set** (§6); a settings change re-highlights from the earliest
  affected line. `fences: Vec<bool>` is the per-line inside-code carry so
  incremental relayout can resume anywhere.
- The content version hashes the rope **line by line, borrowed** — a
  full-document `String` per frame is not acceptable on a page-sized buffer.
  (Ceiling: still O(n) hashing per frame; an app-side revision counter +
  `change_hint` is the upgrade if profiling ever names it.)

## 4. The edit layer (`app/src/pages/mod.rs`)

Structural keys are intercepted on the emitted Action (the widget has no
key-binding hook), BEFORE the native edit:

- **Enter** on a list line carries the marker down; a task marker carries
  **unticked**. Enter on an EMPTY list item ends the list. Enter at the end of
  an **unmatched ``` line auto-closes the fence** with the caret inside — the
  reason an open fence is a transient state, not a data hazard.
- **Backspace** at the first character of list content removes the marker
  (the standard escape that does not eat the line above).
- **Tab / Shift+Tab** shift one two-space step. An indent the tree cannot
  hold is **refused as a consumed no-op**: line 0/1, or deeper than the line
  above + 1 — allowing it would strand the buffer at a depth no save can
  persist.
- Everything else — including clicks (`MoveTo`) and IME — falls through to
  the native edit. NOTHING in this layer talks to the node.

## 5. The save discipline

**One write path**: the dirty-gated 900 ms tick
(`page_autosave_tick` in `app/src/ui/handlers/pages.ice` +
`save_page_document` in `app/src/backend/document.rs`). Dirtiness IS
`page_text(page_editor) != page_saved_text`; the subscription only exists
while drift does.

**Tick gates**, in order: `loading`, `mutation_phase != idle`,
**in-flight save** (`block_autosave_status == "saving"` — a multi-op chain
routinely outlives 900 ms, and a second chain against the same page defeats
the ordering rule), dirty check, **open-fence hold** (an open ``` folds
everything under it into one code block on parse; the tick waits and SAYS so —
status drops to idle and the refusal line reads "the ``` fence is open").

**The plan** (`document_plan`): prefix/suffix trim (never an LCS — head and
tail keep their ids so comment anchors survive), per-field ops
(`SetText`/`SetKind`/`SetChecked`/`Nest`/`Insert`/`Remove`):
- `SetChecked` is emitted only when the WANTED kind is Todo; `stored_lines`
  clears phantom ticks off non-Todo kinds (the module rejects `NotTodo`).
- `Nest` is ONE step per plan, and only when the stored tree can PERFORM it
  (a previous sibling to indent under). An unperformable step is **deferred**,
  not submitted: the next tick re-plans, and a plan that comes back empty
  settles the baseline instead of retrying forever.
- Removals walk the doomed run **leaves-first** (reverse document order), so
  deleting a whole subtree together is one legal gesture. A doomed parent
  whose subtree extends PAST the removed run is **refused** — `RemoveBlock`
  takes the subtree, and destroying survivors on an append-only chain is the
  one thing this surface must never do.
- A refusal writes NOTHING, and rolls the buffer back **only if untouched**
  (buffer still equals the submitted text) — otherwise the user's newest words
  survive, the baseline moves to canonical, and the still-dirty buffer
  re-plans with the refusal line explaining why.

**Ordering**: `/v1/submit` gives no cross-request ordering, so ops apply in
ONE async fn, each `.await`ed — never `join_all`. Only the first op may report
an uncommitted failure. Inserts chain their anchors; an anchorless insert
parents on the **page id itself** (`blocks` never contains the page's own
record — a lookup there can only ever find a subpage).

**The title write** goes FIRST (a rename lands even if a body op is refused
after it) and is **direct** — the old debounce path returned `Ok(false)` on
supersession to a caller that never read it (silently dropped renames) and
cost 400 ms per save.

**The baseline** (`saved_baseline`): a WRITE moves `page_saved_text` to the
node's canonical text — anything typed mid-flight stays dirty, and a
multi-step indent keeps ticking until buffer and node agree. A NO-OP save
adopts the submitted text — `* item` vs `- item` parse identically, and a
canonical baseline there would tick forever over spelling.

**Buffer installs** (`install_decision`): loads and mutations decide ONCE,
against the PREVIOUS page identity, applied to buffer and baseline together —
the incoming page's text lands when the page MOVED or a clean buffer actually
differs; a dirty buffer on the SAME page survives any reload (a reload must
never eat keystrokes); a clean identical buffer is left alone (a rebuilt
`Content` throws the caret to the origin). `close_doc_tab` takes
`choose_page`'s full prologue (`loading = true` + generation bumps) — without
it the next tick writes the old page's text into the new page.

**The chip is earned**: `saving…` while a chain is in flight, `not saved` on
error, `✓ synced` only for a write the node accepted; the fence hold clears
the chip rather than letting a stale ✓ lie over held-back text.

## 6. Comments (#934)

- **The rail is document-scoped**; the header entry is a real glyph
  (`nav-chat`) with a label and a count that is correct **from first paint**:
  `PagesData` carries the open-thread total and the commented block ids on
  every page load (one grouped `ThreadsForTargets` ride-along).
- **`ThreadRow.target` survives projection** (`PageCommentThread.target`).
  Opening a thread rides the thread's OWN target — the node validates a
  comment read against it, which is why block-anchored threads were unopenable
  when the app asked with the page id. Every rail row and the open thread
  carry an anchor label: `this page` or `line N · <block snippet>`
  (`comment_anchor_label`).
- **A new comment anchors on the block the caret sits in** — the Notion
  gesture. `caret_comment_target` is tracked on every edit/move action via the
  borrow-based `editor_cursor_line` inspector + `block_at_line_target`
  (line↔block mapping from `line_spans`, which mirrors the rendered shape —
  a code block owns its fences and body lines). The composer announces the
  anchor before you post ("New comment on line 6 · …"). A reply stays on its
  thread's anchor; the title line (or an untouched caret) anchors on the page.
- **Every line of a block carrying an unresolved thread wears a quiet brand
  wash** in the document (highlighter settings, §3). Resolving drops the wash.
- **Resolve / Reopen** from the open thread (`ResolveThread` — a node op that
  previously had no caller). The list reload recomputes count and washes.
- Draft protection: the half-typed comment is orphaned into "Recovered
  drafts" only when its PAGE vanishes — never merely because a live resync
  ran (`remember_orphaned_page_comment`).

**Trap (load-bearing):** an `editor`-valued Ice sync argument is a
`Content::clone`, and **iced's `Content::clone` rebuilds from text — the
cursor resets to the origin**. Anything cursor-shaped must ride the
borrow-based inspectors (`editor_cursor_line` et al.), never a by-value
editor argument.

## 7. Wire contract (what the app leans on)

- Op vocabulary used: `CreatePage`, `InsertBlock`, `UpdateText`, `SetKind`,
  `SetChecked`, `MoveBlock`, `RemoveBlock`, `AddComment`, `ResolveThread`.
  There is NO document-level replace and no batch endpoint; a document save is
  N ordered ops (§5). `SetSpanMark`/`marks` stay deliberately unwired:
  markdown-as-source and the module's UTF-16 span marks are two competing
  representations of the same formatting — this surface chose markdown.
- No CAS anywhere: block writes are last-writer-wins; the only cross-client
  protections are structural (`DuplicateBlock`, `AnchorNotFound`,
  `CycleMove`, subtree preflight).
- Bounds the surface respects: 23 KiB app-signed payload (tighter than the
  node's 64 KiB text bound), depth 64, `ThreadsForTargets` ≤ 512 targets.
- Unsigned `/v1/submit` attributes ops to the node's own key (`origin` is
  provenance, not authorship) — a QA/seeding lane, never the app's.

## 8. Known limits, in order of pain

1. **No undo.** The stock `Content` has no history; the reference
   (`ducktape-ui/examples/markdown-editor`) carries a bounded delta history
   with coalescing — the obvious next Pages PR.
2. **Ticking a todo is typing `x`** between the brackets. A clickable gutter
   affordance needs ducktape-ui to expose per-visual-line geometry
   (`mouse_interaction` already computes the hovered line and throws it
   away) — the same upstream visit as the opaque-`line_highlight` paint fix.
3. **Links are painted, not clickable** (the reference does link hit-testing;
   the app does not yet).
4. Comment-wash liveness with the rail CLOSED rides the pages-delta reload
   only (the rail-open path refreshes on every thread load).
5. Concurrent edits from two clients converge last-writer-wins per block; the
   positional middle-pairing can re-text blocks across a big top-of-page
   insert (comment anchors then point at shifted content). Acceptable at
   dogfood scale; an LCS pairing is the upgrade.

## 9. Where the invariants are pinned

- `app/src/pages/sync.rs` tests — round trips, clamping, spans, plan shapes,
  refusals, deferrals, line↔block mapping.
- `app/src/pages/mod.rs` tests — list/fence/indent keys, `page_text`
  verbatimness, anchor labels, commented-line washes.
- `app/src/pages/markdown.rs` tests — prefix classification, fence carry,
  caret reveal, hidden-marker measurability, the code-body full-size probe.
- `app/src/tests.rs` — the no-click-to-edit refusals, tick gates
  (in-flight + fence hold), install decisions across resync/mutation, comment
  handler contracts.
- `app/src/backend/tests.rs` — text bounds (empty block writable, empty title
  not), `saved_baseline`, signed round trips over a simnode.

Related records: `[[pages-one-document-914]]`-era history lives in the #913/
#914/#934 PR bodies; the ducktape-ui asks (paint order, line geometry) are
tracked against the pinned `examples/markdown-editor` reference.
