// PAGES, as a module-owned view. The kernel pushes session facts only
// (`session()` — one item per change, plus the page a `duck://` link asked
// for); the workspace, the open document, its comment threads and the search
// are read here through `rpc.view`, re-read on every pages block
// (`rpc.live`), and every write — a create, a delete, a comment, a resolve,
// and each op of a document save — leaves as `op.submit` the kernel signs.
// The document editor, its history and its presentation have always been
// this view's; now so is the text in it.
app PagesView
  title "Pages"
  palette active_palette
  id "dev.ducktape.view.pages"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"
use "../../../../../app/src/ui/ducktape-ui/recipes.ice"
use "../../../../../app/src/ui/components/icon.ice"
use "pages.ice"
use "rows.ice"
use "kit.ice"

extern crate::host
  PageItem(id:str, title:str, parent:str, prefix:str, child_count:i64)
  Subpage(id:str, title:str)
  PageSearchHit(page_id:str, page_title:str, block_id:str, kind:str, text:str)
  PageComment(id:str, ordinal:i64, author:str, meta:str, text:str)
  PageCommentThread(id:str, target:str, author:str, meta:str, resolved:bool, comment_count:i64, comments:[PageComment])
  PageCommentThreadRow(thread:PageCommentThread, anchor:str)
  PageCommentGroup(target:str, anchor:str, threads:[PageCommentThread])
  Session(connected:bool, dark:bool, chain:str, route_page:str, route_serial:i64)
  SessionItem(next:Session, error:str)
  RegisterItem(pages:[PageItem], active_page:str, active_page_title:str, active_page_parent:str, blocks:[PageBlock], subpages:[Subpage], document:str, comment_rows:[PageCommentThreadRow], thread_total:i64, commented_hits:[str], error:str)
  SearchItem(query:str, hits:[PageSearchHit], error:str)
  ActItem(page:str, error:str)
  SaveItem(written:bool, refusal:str, document:str, error:str)
  subscription session() -> SessionItem
  // the workspace, read by this view: once per connection and per page
  // picked, then again on every pages block. The threads come back WITH
  // their comments, so there is no per-thread read at all.
  subscription register(page:str, serial:i64) -> RegisterItem
  // one answer per query the reader sends
  subscription search(query:str, serial:i64) -> SearchItem
  // every write's outcome, as the kernel answers it
  subscription acts() -> ActItem
  subscription saves() -> SaveItem
  pure connection_serial_after(was_connected:bool, connected:bool, serial:i64) -> i64
  pure route_arrived(serial:i64, seen:i64) -> bool
  sync create(title:&str) -> bool
  sync delete(page_id:&str) -> bool
  sync post(text:&str, target:&str, thread_id:&str) -> bool
  sync resolve(thread_id:&str, resolved:bool) -> bool
  sync save(page_id:&str, text:&str, saved:&str) -> bool
  sync copy(text:&str, label:&str) -> bool
  sync open_link(link:&str) -> bool
  pure icon(name:&str) -> bytes
  pure count_label(count:i64) -> str
  pure keep_str(keep:bool, next:&str, current:&str) -> str
  pure keep_i64(keep:bool, next:i64, current:i64) -> i64
  pure sidebar_width_after_delta(width:f64, delta:f64, viewport:f64) -> f64
  pure search_answer_stands(query:&str, draft:&str, searching:bool) -> bool
  pure initials_of(name:&str) -> str
  pure page_address(page_id:&str, chain:&str) -> str
  pure page_display_title(pages:&[PageItem], id:&str, current:&str) -> str
  pure compose_hint_of(blocks:&[PageBlock], target:&str, page_id:&str) -> str
  pure comment_marks(blocks:&[PageBlock], hits:&[str]) -> [CommentMark]
  pure commented_lines(blocks:&[PageBlock], hits:&[str]) -> [i64]
  pure block_at_line(blocks:&[PageBlock], line:i64) -> str
  pure document_text(document:&editor) -> str
  pure document_editor(text:&str) -> editor
  pure install_decision(text:&str, current_page:&str, next_page:&str, saved:&str, canonical:&str) -> bool
  pure has_unclosed_fence(text:&str) -> bool
  pure saved_baseline(written:bool, canonical:&str, submitted:&str) -> str
  pure baseline_at_submitted_title(canonical:&str, submitted:&str) -> str
  pure remember_draft(drafts:&[str], draft:&str) -> [str]
  pure forget_draft(drafts:&[str], draft:&str) -> [str]
  pure navigation_link(interaction:bytes) -> str
  pure navigation_comment_line(interaction:bytes) -> i64
  pure comment_card_offset(anchor_y:f64, viewport_height:f64) -> f64
  pure comment_card_height(anchor_y:f64, viewport_height:f64) -> f64
  pure comment_scope_label(blocks:&[PageBlock], scope:&str, page_id:&str, open_threads:i64) -> str
  pure comment_post_target(rows:&[PageCommentThreadRow], thread_id:&str, scope:&str) -> str
  pure scope_groups(rows:[PageCommentThreadRow], scope:&str, page_id:&str) -> [PageCommentGroup]
  pure scope_resolved(rows:[PageCommentThreadRow], scope:&str) -> [PageCommentThreadRow]
  pure kept_ids(keep:bool, ids:[str]) -> [str]
  pure resolved_label(rows:&[PageCommentThreadRow]) -> str
  pure empty_scope_label(scope:&str) -> str
  pure opener_text(thread:&PageCommentThread) -> str
  pure thread_replies(thread:&PageCommentThread, expanded:bool) -> [PageComment]
  pure reply_toggle_label(thread:&PageCommentThread, expanded:bool) -> str
  pure reply_thread_after_press(current:&str, pressed:&str) -> str
  pure expanded(ids:&[str], id:&str) -> bool
  pure toggled(ids:[str], id:&str) -> [str]

extern crate::editor_binding
  HistoryState(snapshot:bytes)
  MenuState(snapshot:bytes)
  EditorUpdate(notice:str, history:HistoryState, menu:MenuState, reference:bytes, interaction:bytes)
  pure initial_history() -> HistoryState
  pure initial_menu() -> MenuState
  editor-binding keys(history:HistoryState, menu:MenuState) -> EditorUpdate

extern crate::document_sync
  PageBlock(key:i64, id:str, parent:str, kind:str, text:str, checked:bool, prefix:str, child_count:i64)
  CommentMark(line:i64, count:i64)

extern crate::editor_view
  PreparedPresentation(reference:bytes, data:bytes, notice:str)
  pure presentation_notice(document:&editor, prepared:&PreparedPresentation) -> str
  pure empty_presentation() -> PreparedPresentation
  editor-highlighter paint(prepared:PreparedPresentation)
  pure document_presentation(document:&editor, menu:MenuState, dark:bool, commented:[i64], marks:[CommentMark], focused:bool) -> PreparedPresentation

state
  pointer_y:f64 = 0.0
  comment_anchor_y:f64 = -1.0
  document_focused = false
  focus_query:i64 = 0
  document_paint:PreparedPresentation = empty_presentation()
  document:editor = ""
  document_history:HistoryState = initial_history()
  document_menu:MenuState = initial_menu()
  document_error = ""
  document_dark = false
  document_commented:[i64] = []
  document_marks:[CommentMark] = []
  active_palette:palette[AppTheme] = AppTheme.app
  // the session, as the kernel pushes it
  connected = false
  chain = ""
  route_serial:i64 = 0
  // moves when the session comes up, when a page is picked, and after every
  // write: the register is read afresh
  register_serial:i64 = 0
  loading = false
  // ONE WRITE AT A TIME. Every act takes this lock and the act's outcome
  // releases it; the document save keeps its own (`autosave`).
  busy = false
  host_error = ""
  page_link = ""
  pages:[PageItem] = []
  blocks:[PageBlock] = []
  // CHROME, NOT FACTS: how wide the reader dragged the page list and whether
  // she has the `⋯` menu open. Neither leaves this view, and neither is
  // persisted — a fresh window opens on the default again.
  pages_viewport_width = 1280.0
  pages_viewport_height = 700.0
  sidebar_width = 230.0
  page_menu_open = false
  page_create_open = false
  active_page = ""
  active_page_title = ""
  active_page_parent = ""
  page_searching = false
  page_search_hits:[PageSearchHit] = []
  page_search_query = ""
  page_search_serial:i64 = 0
  page_delete_armed = false
  autosave = "idle"
  page_refusal = ""
  subpages:[Subpage] = []
  orphaned_comment_drafts:[str] = []
  block_comments_open = false
  // THE CARD IS ONE SCOPE'S THREADS, listed expanded: the page's whole
  // conversation from the header chip, one block's from a margin badge. A
  // badge-opened card is PINNED — the page is not what the badge pointed at,
  // so the way back out to it is withheld.
  scope_target = ""
  scope_pinned = false
  // the page's OUTSTANDING threads: what the header chip counts, and what the
  // card's title says the page is carrying
  thread_total:i64 = 0
  comment_rows:[PageCommentThreadRow] = []
  threads_loading = false
  commented_hits:[str] = []
  // CARD CHROME, THE READER'S OWN: which thread's reply box is open, which
  // threads she unfolded, and whether the settled ones are showing. None of
  // it leaves the view, and none of it is a fact about the workspace.
  reply_thread = ""
  expanded_threads:[str] = []
  resolved_open = false
  // the reader's own: the four drafts the screen edits, and what a refused
  // write hands back
  page_draft = ""
  page_search_draft = ""
  block_comment_draft = ""
  reply_draft = ""
  pending_page = ""
  pending_comment = ""
  // The document is one editor buffer. Drift from the last saved text is the
  // dirty signal; `buffer_page` names what that buffer actually contains.
  page_saved_text = ""
  buffer_page = ""
  page_inflight_text = ""
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false

// Subscriptions, not mount tasks, so a replacement restored from this view's
// state asks for the session and re-reads the workspace on its own.
subscribe
  mouse moved status=any -> comment_pointer_moved _ _
  mouse released status=any -> document_pointer_released _
  keyboard release status=any -> document_key_released _
  window focused -> document_window_focused
  window unfocused -> document_window_unfocused
  session() -> session_arrived _
  register(active_page, register_serial) when connected -> register_arrived _
  search(page_search_query, page_search_serial) when connected && !empty(page_search_query) -> search_arrived _
  acts() -> act_done _
  saves() -> save_done _
  // THE PAGE SAVES ON A GATED TICK, not per keystroke: the editor's edits
  // land in `document` without passing through a handler on the way to the
  // node, so dirtiness is the buffer's drift from `page_saved_text`.
  every 900ms when (connected && !loading && !busy && !empty(active_page) && active_page == buffer_page) -> page_autosave_tick

on session_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  let next = item.next
  register_serial = connection_serial_after(connected, next.connected, register_serial)
  connected = next.connected
  chain = next.chain
  document_dark = next.dark
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused)
  // A `duck://page/…` link the app was asked to open. The serial moves once
  // per ask, so the same link twice opens the page twice. The register
  // re-keys on `active_page`, so moving it IS the navigation — and the
  // page-move reset lives in `register_arrived`, where EVERY way a page can
  // move is observed.
  let route_moved = route_arrived(next.route_serial, route_serial) && !empty(next.route_page) && next.route_page != active_page
  route_serial = next.route_serial
  active_page = keep_str(route_moved, next.route_page, active_page)
  loading = loading || route_moved
  page_link = page_address(active_page, chain)
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

// The pointer's last y, so a comment opened off the document floats beside
// the line it was asked for.
on comment_pointer_moved(_x, y)
  pointer_y = y

// A PICK ABANDONS THE RAIL'S DRAFT: it is kept as a recovered draft on the
// page it belonged to, offered back when the reader returns.
//
// THE SWITCH IS VISIBLE NOW: the clicked page takes the sidebar highlight and
// the header title, and the previous document leaves the pane, before the
// round trip — a click that repaints nothing for the seconds a page load
// takes reads as a dead app. Only `active_page` moves; `buffer_page` stays
// where the text came from, which is what keeps the landing load a MOVE
// rather than a refresh.
on choose_page(id)
  return if !empty(host_error) || empty(id)
  return if loading || busy
  return if id == active_page
  active_page = id
  active_page_title = page_display_title(pages, id, active_page_title)
  active_page_parent = ""
  blocks = []
  page_link = page_address(id, chain)
  loading = true

on register_arrived(item)
  host_error = item.error
  loading = false
  return if !empty(item.error)
  // THE PAGE MOVED WHEN THE BUFFER IS NOT ALREADY THIS PAGE'S — a pick, a
  // `duck://` link, a create landing, a delete falling back. One place, so
  // every route through it drops the rail, the search and the armed delete
  // belonging to the page being left, and keeps its unsent comment as a
  // recovered draft.
  let page_moved = item.active_page != buffer_page
  // A CARD BELONGS TO ONE PAGE. It closing takes its scope, its pin, its
  // unfolded threads and the half-typed reply with it — read BEFORE
  // `block_comments_open` moves, so every field answers the same question.
  let comments_carry = block_comments_open && !page_moved
  orphaned_comment_drafts = remember_draft(orphaned_comment_drafts, keep_str(page_moved, block_comment_draft, ""))
  block_comment_draft = keep_str(page_moved, "", block_comment_draft)
  block_comments_open = comments_carry
  scope_target = keep_str(comments_carry, scope_target, "")
  scope_pinned = scope_pinned && comments_carry
  reply_thread = keep_str(comments_carry, reply_thread, "")
  reply_draft = keep_str(comments_carry, reply_draft, "")
  expanded_threads = kept_ids(comments_carry, expanded_threads)
  resolved_open = resolved_open && comments_carry
  page_searching = page_searching && !page_moved
  // The hits and the comments themselves need no clearing: both panels are
  // gated on the strings above, so dropping those takes them off the screen.
  page_search_query = keep_str(page_moved, "", page_search_query)
  page_delete_armed = page_delete_armed && !page_moved
  autosave = keep_str(page_moved, "idle", autosave)
  pages = item.pages
  blocks = item.blocks
  subpages = item.subpages
  active_page_title = item.active_page_title
  active_page_parent = item.active_page_parent
  thread_total = item.thread_total
  comment_rows = item.comment_rows
  commented_hits = item.commented_hits
  threads_loading = false
  document_commented = commented_lines(item.blocks, item.commented_hits)
  document_marks = comment_marks(item.blocks, item.commented_hits)
  page_link = page_address(item.active_page, chain)
  // ONE INSTALL DECISION, decided against the page the BUFFER holds — never
  // against `active_page`, which moved to the clicked page the moment it was
  // clicked — and applied to buffer and baseline together: the incoming
  // page's text lands when the page MOVED or a clean buffer actually differs;
  // a dirty buffer on the SAME page is the reader mid-typing through a
  // reload, and a reload must never eat keystrokes.
  let install = install_decision(document_text(document), buffer_page, item.active_page, page_saved_text, item.document)
  active_page = item.active_page
  // The buffer now holds THIS page. Unconditional on purpose: the install is
  // refused only when the decision already found the page unchanged.
  buffer_page = item.active_page
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused)
  return if !install
  page_saved_text = item.document
  page_refusal = ""
  document = document_editor(item.document)
  document_menu = initial_menu()
  document_focused = false
  document_error = ""
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused)

on search_arrived(item)
  host_error = item.error
  page_searching = false
  return if item.query != page_search_query
  page_search_hits = item.hits

on act_done(item)
  busy = false
  threads_loading = false
  host_error = item.error
  register_serial = register_serial + 1
  // A REFUSED WRITE HANDS ITS WORDS BACK: the field cleared when the act
  // left, and the reader must not have to retype them. A refused post lands
  // back in the box it left — the reply's thread, or the card's composer.
  let refused = !empty(item.error)
  page_draft = keep_str(refused, pending_page, page_draft)
  reply_draft = keep_str(refused && !empty(reply_thread), pending_comment, reply_draft)
  block_comment_draft = keep_str(refused && empty(reply_thread), pending_comment, block_comment_draft)
  pending_page = ""
  pending_comment = ""
  return if refused
  page_create_open = false
  page_delete_armed = false
  active_page = keep_str(!empty(item.page), item.page, active_page)

on save_done(item)
  autosave = "error"
  host_error = item.error
  return if !empty(item.error)
  host_error = ""
  page_refusal = item.refusal
  // The baseline is the node's own text after a write, and the submitted text
  // after a no-op. Either way anything typed during the round trip stays
  // dirty, and a depth change that takes one nest step per tick keeps ticking
  // until the buffer and the node agree.
  page_saved_text = baseline_at_submitted_title(saved_baseline(item.written, item.document, page_inflight_text), page_inflight_text)
  autosave = "saved"
  register_serial = register_serial + keep_i64(item.written, 1, 0)
  return if empty(item.refusal)
  // A REFUSED PLAN ROLLS THE BUFFER BACK — but only when nothing was typed
  // since the tick submitted. Otherwise the buffer is kept (the newest words
  // must survive), the baseline moves to the node's text, and the still-dirty
  // buffer re-plans on the next tick with the refusal line explaining why.
  let untouched = document_text(document) == page_inflight_text
  page_saved_text = baseline_at_submitted_title(item.document, page_inflight_text)
  autosave = "idle"
  return if !untouched
  document = document_editor(item.document)
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused)

on page_autosave_tick
  return if !empty(host_error)
  // NEVER WRITE A BUFFER INTO A PAGE IT DOES NOT BELONG TO. `active_page`
  // moves the instant the reader clicks; the buffer only becomes that page's
  // when a load lands and stamps `buffer_page`.
  return if busy || loading || empty(active_page) || active_page != buffer_page
  // One op chain at a time: a multi-op save routinely outlives the tick, and
  // a second chain against the same page defeats the ordering rule the
  // awaited loop exists for.
  return if autosave == "saving"
  let text = document_text(document)
  return if text == page_saved_text
  // An open ``` swallows every line under it when parsed: the plan would
  // REMOVE every block below it, and removing a block purges its comment
  // threads. The save waits for the close — quietly. The status drops to idle
  // (no "✓ synced" over held-back text), and the next tick after the close
  // writes; no banner lectures the writer about Markdown mid-sentence.
  autosave = "idle"
  return if has_unclosed_fence(text)
  autosave = "saving"
  page_inflight_text = text
  sent = save(active_page, text, page_saved_text)

on toggle_page_create
  return if !empty(host_error)
  page_create_open = !page_create_open

// The title leaves with the act; the field clears here — a refused create
// hands it back.
on create_page_submit
  return if !empty(host_error)
  return if loading || busy || !connected || empty(trim(page_draft))
  busy = true
  pending_page = trim(page_draft)
  page_draft = ""
  sent = create(pending_page)

on arm_page_delete
  return if !empty(host_error)
  return if loading || busy || empty(active_page)
  page_menu_open = false
  page_delete_armed = true

on disarm_page_delete
  page_delete_armed = false

on delete_page_submit
  return if !empty(host_error)
  return if loading || busy || empty(active_page) || !page_delete_armed
  busy = true
  page_delete_armed = false
  orphaned_comment_drafts = remember_draft(orphaned_comment_drafts, block_comment_draft)
  block_comment_draft = ""
  sent = delete(active_page)

on search_pages_submit
  return if !empty(host_error)
  return if page_searching || empty(trim(page_search_draft))
  page_searching = true
  page_search_hits = []
  page_search_query = trim(page_search_draft)
  page_search_serial = page_search_serial + 1

on clear_page_search
  page_search_draft = ""
  page_search_hits = []
  page_searching = false
  page_search_query = ""

on open_page_search_hit(page_id, _block_id)
  return if !empty(host_error)
  return if loading || busy
  flow
    from done page_id
    done -> choose_page _

on use_orphaned_comment_draft(draft)
  return if !empty(host_error)
  return if loading || busy || !empty(trim(block_comment_draft))
  block_comment_draft = draft
  // A RECOVERED DRAFT IS THE PAGE'S, not a block's: it was typed into the
  // composer of a card that is gone, and the card it lands in is the page's
  // own conversation.
  block_comments_open = true
  scope_target = ""
  scope_pinned = false
  orphaned_comment_drafts = forget_draft(orphaned_comment_drafts, draft)

on discard_orphaned_comment_draft(draft)
  orphaned_comment_drafts = forget_draft(orphaned_comment_drafts, draft)

// THE HEADER CHIP OPENS THE PAGE'S CONVERSATION, unpinned, so its group
// headers can narrow and the way back out stays open. The register already
// read every thread anchored to the page or any of its blocks, so opening the
// card is the flip alone. Closing keeps the half-typed comment through the
// orphan guard, exactly as the card's own × does.
on toggle_block_comments
  comment_anchor_y = -1.0
  return if !empty(host_error)
  return if loading || busy || empty(active_page)
  orphaned_comment_drafts = remember_draft(orphaned_comment_drafts, block_comment_draft)
  block_comment_draft = ""
  block_comments_open = !block_comments_open
  scope_target = ""
  scope_pinned = false
  reply_thread = ""
  reply_draft = ""
  resolved_open = false

on close_block_comments
  comment_anchor_y = -1.0
  orphaned_comment_drafts = remember_draft(orphaned_comment_drafts, block_comment_draft)
  block_comment_draft = ""
  block_comments_open = false
  scope_target = ""
  scope_pinned = false
  reply_thread = ""
  reply_draft = ""
  resolved_open = false

// NARROWING NEEDS NO ROUND TRIP: the register answered for the page AND every
// block on it, so a group header only re-slices the rows already in hand. The
// reply in progress belongs to a thread that may be about to leave the list.
on narrow_comment_scope(target)
  reply_thread = ""
  reply_draft = ""
  return if !empty(host_error)
  return if loading || busy || empty(target)
  scope_target = target

// A PINNED CARD KEEPS ITS BLOCK: the margin badge was not pointing at the
// page, so widening back out to it is withheld.
on widen_comment_scope
  reply_thread = ""
  reply_draft = ""
  return if !empty(host_error)
  return if loading || busy || scope_pinned
  scope_target = ""

on resolve_thread_submit(id, resolved)
  return if !empty(host_error)
  return if loading || busy || threads_loading || !block_comments_open || empty(id)
  busy = true
  threads_loading = true
  sent = resolve(id, resolved)

// THE READER PICKS WHICH THREAD SHE IS ANSWERING, and a second press on the
// same one puts the box away. One box holds a draft at a time, so moving it is
// what drops the last one.
on select_reply_thread(id)
  reply_draft = ""
  reply_thread = reply_thread_after_press(reply_thread, id)

on toggle_thread_replies(id)
  expanded_threads = toggled(expanded_threads, id)

on toggle_resolved_comments
  resolved_open = !resolved_open

// A REPLY NAMES ITS THREAD AND INHERITS ITS ANCHOR — the node validates the
// pair, so a block-anchored thread replied to with the page id is refused.
// `comment_post_target` answers "" for a thread id the list no longer carries,
// and a stale card posts nowhere rather than somewhere else.
on post_thread_reply(id)
  return if !empty(host_error)
  return if loading || busy || threads_loading || !block_comments_open || empty(trim(reply_draft))
  let reply_target = comment_post_target(comment_rows, id, scope_target)
  return if empty(reply_target)
  busy = true
  threads_loading = true
  pending_comment = trim(reply_draft)
  reply_draft = ""
  sent = post(pending_comment, reply_target, id)

// THE COMPOSER AT THE CARD'S FOOT ALWAYS OPENS A NEW THREAD on the scope the
// card is showing — page scope anchors on the page itself. The words leave
// with the act and the field clears; a post the node refused hands them back.
on post_block_comment_submit
  return if !empty(host_error)
  return if loading || busy || threads_loading || !block_comments_open || empty(active_page) || empty(trim(block_comment_draft))
  let fresh_target = keep_str(!empty(scope_target), scope_target, active_page)
  busy = true
  threads_loading = true
  pending_comment = trim(block_comment_draft)
  block_comment_draft = ""
  sent = post(pending_comment, fresh_target, "")

on copy_to_clipboard(text, label)
  sent = copy(text, label)

on document_pointer_released(_button)
  focus_query = focus_query + 1
  let query = focus_query
  task widget focused #root/pages/document -> document_focus_checked query _

on document_key_released(_key)
  focus_query = focus_query + 1
  let query = focus_query
  task widget focused #root/pages/document -> document_focus_checked query _

on document_window_focused
  focus_query = focus_query + 1
  let query = focus_query
  task widget focused #root/pages/document -> document_focus_checked query _

on document_window_unfocused
  focus_query = focus_query + 1
  document_focused = false
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused)

on document_focus_checked(query, focused)
  return if query != focus_query || focused == document_focused
  document_focused = focused
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused)

on sidebar_resized(dx, _dy)
  sidebar_width = sidebar_width_after_delta(sidebar_width, dx, pages_viewport_width)

on pages_viewport_changed(width, _height)
  pages_viewport_width = width
  sidebar_width = sidebar_width_after_delta(sidebar_width, 0.0, width)

on toggle_page_menu
  page_menu_open = !page_menu_open

on close_page_menu
  page_menu_open = false

// A link press goes through the app's ONE open plane; a margin badge press
// opens that block's conversation.
on document_committed(next)
  document_history = next.history
  document_menu = next.menu
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused)
  let comment_line = navigation_comment_line(next.interaction)
  // The refusal describes an edit that was already rolled back; the next
  // keystroke is the reader moving on from it.
  page_refusal = ""
  sent = open_link(navigation_link(next.interaction))
  return if comment_line < 0 || loading || busy || empty(active_page)
  // A MARGIN BADGE OPENS ITS BLOCK'S CONVERSATION, and that block is the whole
  // card: the pin withholds the way back out to the page, because the page is
  // not what the badge was pointing at. The title line carries no badge, so an
  // empty target here is a line the page itself owns — page scope, unpinned.
  orphaned_comment_drafts = remember_draft(orphaned_comment_drafts, block_comment_draft)
  block_comment_draft = ""
  reply_thread = ""
  reply_draft = ""
  resolved_open = false
  scope_target = block_at_line(blocks, comment_line)
  scope_pinned = !empty(scope_target)
  // THE CARD FLOATS AT THE PRESS, not at the top of the pane: the comment
  // belongs to the line the reader pointed at, and the pointer is where that
  // line is on screen.
  comment_anchor_y = pointer_y
  block_comments_open = true

// The sensor is the window measure the sidebar clamp needs, and it keys
// nothing — `#root/pages/document` is still the editor's path.
view
  sensor show=pages_viewport_changed resize=pages_viewport_changed
    box #root
      with
        w=fill
        h=fill
        bg=bg
      PagesScreen page_draft<->page_draft page_search_draft<->page_search_draft block_comment_draft<->block_comment_draft reply_draft<->reply_draft #pages
        with
          host_error
          page_link
          sidebar_width
          page_menu_open
          pages
          page_create_open
          loading
          busy=(busy || !empty(host_error))
          connected
          active_page
          active_page_title
          active_page_parent
          page_searching
          page_search_hits
          page_search_query
          page_delete_armed
          autosave
          page_refusal
          subpages
          orphaned_comment_drafts
          block_comments_open
          comments_height=comment_card_height(comment_anchor_y, pages_viewport_height)
          comments_offset=comment_card_offset(comment_anchor_y, pages_viewport_height)
          scope_target
          scope_pinned
          // ONE SCOPE DECIDES THE WHOLE CARD — its title, its groups, its
          // settled threads and what the composer will anchor on. Derived
          // here, so narrowing and widening are the one state field moving
          // and no handler can leave a stale half behind.
          scope_label=comment_scope_label(blocks, scope_target, active_page, thread_total)
          thread_total
          comment_groups=scope_groups(comment_rows, scope_target, active_page)
          resolved_comment_rows=scope_resolved(comment_rows, scope_target)
          resolved_open
          reply_thread
          expanded_threads
          threads_loading
          compose_hint=compose_hint_of(blocks, scope_target, active_page)
        events
          toggle_page_create -> toggle_page_create
          create_page_submit -> create_page_submit
          choose_page -> choose_page _
          search_pages_submit -> search_pages_submit
          clear_page_search -> clear_page_search
          resize_sidebar -> sidebar_resized _ _
          toggle_page_menu -> toggle_page_menu
          close_page_menu -> close_page_menu
          arm_page_delete -> arm_page_delete
          disarm_page_delete -> disarm_page_delete
          delete_page_submit -> delete_page_submit
          open_page_search_hit -> open_page_search_hit _ _
          use_orphaned_comment_draft -> use_orphaned_comment_draft _
          discard_orphaned_comment_draft -> discard_orphaned_comment_draft _
          toggle_block_comments -> toggle_block_comments
          close_block_comments -> close_block_comments
          narrow_comment_scope -> narrow_comment_scope _
          widen_comment_scope -> widen_comment_scope
          resolve_thread_submit -> resolve_thread_submit _ _
          select_reply_thread -> select_reply_thread _
          toggle_thread_replies -> toggle_thread_replies _
          toggle_resolved_comments -> toggle_resolved_comments
          post_thread_reply -> post_thread_reply _
          post_block_comment_submit -> post_block_comment_submit
          copy_to_clipboard -> copy_to_clipboard _ _
        document:
          col w=fill h=fill
            if !empty(presentation_notice(document, document_paint))
              text presentation_notice(document, document_paint) @text-muted
            if !empty(document_error)
              text document_error @text-danger
            editor #document <-> document -> document_committed _
              with
                key-binding=keys(document_history, document_menu)
                highlighter=paint(document_paint)
                size=14.0
                line-h=1.65
                wrap=word
                font=ui
                hint="Write something… `#` for a heading, `-` for a list"
                disabled=(!empty(host_error) || loading || !connected || empty(active_page) || active_page != buffer_page)
              active bg=fg/0 value=document_ink placeholder=hint selection=document_selection border-w=0.0
              disabled bg=fg/0 value=document_ink placeholder=hint selection=document_selection border-w=0.0
