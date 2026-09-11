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

// WHERE THE COMMENT CARD GOES, read off the document pane's width alone. A
// wide pane floats it in the margin; a narrower one squeezes the document left
// to make that margin; below that there is no margin and the card drops
// full-width into the text, under the block it belongs to. Nothing the host
// holds changes with the placement — the rail, its scope and its drafts are
// the same in all three.
enum CommentsMode
  beside
  squeeze
  inline

extern crate::host
  PageItem(id:str, title:str, parent:str, prefix:str, child_count:i64)
  Subpage(id:str, title:str)
  PageSearchHit(page_id:str, page_title:str, block_id:str, kind:str, text:str)
  PageComment(id:str, ordinal:i64, author:str, meta:str, text:str)
  PageCommentThread(id:str, target:str, author:str, meta:str, resolved:bool, comment_count:i64, comments:[PageComment])
  PageCommentThreadRow(thread:PageCommentThread, anchor:str)
  PageCommentGroup(target:str, anchor:str, threads:[PageCommentThread])
  PagesProps(comment_marks:[CommentMark], document_source:bytes, document_error:str, commented_lines:[i64], dark:bool, connected:bool, loading:bool, busy:bool, page_link:str, pages:[PageItem], page_create_open:bool, active_page:str, active_page_title:str, active_page_parent:str, page_searching:bool, page_search_hits:[PageSearchHit], page_search_query:str, page_delete_armed:bool, autosave:str, page_refusal:str, subpages:[Subpage], orphaned_comment_drafts:[str], block_comments_open:bool, scope_target:str, scope_pinned:bool, scope_label:str, thread_total:i64, comment_rows:[PageCommentThreadRow], threads_loading:bool, compose_hint:str, seed_rev:i64, page_seed:str, comment_seed:str)
  PropsItem(next:PagesProps, error:str)
  subscription props() -> PropsItem
  pure edited(source:bytes, reference:bytes, navigation:bytes, comment_draft:&str) -> bool
  pure installed(document:&editor, source:bytes) -> bool
  pure toggle_create() -> bool
  pure create(title:&str, comment_draft:&str) -> bool
  pure choose(id:&str, comment_draft:&str) -> bool
  pure search(query:&str) -> bool
  pure sidebar_width_after_delta(width:f64, delta:f64, viewport:f64) -> f64
  pure clear_search() -> bool
  pure arm_delete() -> bool
  pure disarm_delete() -> bool
  pure delete(comment_draft:&str) -> bool
  pure open_hit(page_id:&str, block_id:&str, comment_draft:&str) -> bool
  pure use_draft(draft:&str, comment_draft:&str) -> bool
  pure discard_draft(draft:&str) -> bool
  pure toggle_comments(comment_draft:&str) -> bool
  pure close_comments(comment_draft:&str) -> bool
  pure narrow(target:&str) -> bool
  pure widen() -> bool
  pure resolve(id:&str, resolved:bool) -> bool
  pure post(text:&str, thread_id:&str) -> bool
  pure copy(text:&str, label:&str) -> bool
  pure icon(name:&str) -> bytes
  pure count_label(count:i64) -> str
  pure keep_str(keep:bool, next:&str, current:&str) -> str
  pure search_answer_stands(query:&str, draft:&str, searching:bool) -> bool
  pure initials_of(name:&str) -> str
  pure comment_anchor_after_props(current_page:&str, next_page:&str, open:bool, anchor:f64) -> f64
  pure comment_anchor_after_navigation(opens:bool, pointer:f64, anchor:f64) -> f64
  pure comment_navigation(navigation:bytes) -> bool
  pure comment_card_offset(pane:f64, anchor_y:f64, viewport_height:f64) -> f64
  pure comment_card_height(anchor_y:f64, viewport_height:f64) -> f64
  pure comments_mode(pane:f64) -> CommentsMode
  pure document_width(pane:f64, open:bool) -> f64
  pure comments_card_width(pane:f64) -> f64
  pure comments_right_anchor(pane:f64) -> f64
  pure comments_left_inset(pane:f64) -> f64
  pure comments_reserve(pane:f64, open:bool, anchor_line:i64, card_height:f64) -> EditorReserve
  pure measured_card_height(current:f64, measured:f64) -> f64
  pure comment_line_after_navigation(navigation:bytes, line:i64) -> i64
  pure comment_line_after_props(current_page:&str, next_page:&str, open:bool, line:i64) -> i64
  pure comment_groups(rows:[PageCommentThreadRow], page_id:&str) -> [PageCommentGroup]
  pure resolved_rows(rows:[PageCommentThreadRow]) -> [PageCommentThreadRow]
  pure resolved_label(rows:&[PageCommentThreadRow]) -> str
  pure empty_scope_label(scope:&str) -> str
  pure opener_text(thread:&PageCommentThread) -> str
  pure thread_replies(thread:&PageCommentThread, expanded:bool) -> [PageComment]
  pure reply_toggle_label(thread:&PageCommentThread, expanded:bool) -> str
  pure reply_thread_after_press(current:&str, pressed:&str) -> str
  pure expanded(ids:&[str], id:&str) -> bool
  pure toggled(ids:[str], id:&str) -> [str]
  pure seeded(moved:bool, seed:&str, draft:&str) -> str

extern crate::editor_binding
  HistoryState(snapshot:bytes)
  MenuState(snapshot:bytes)
  EditorUpdate(notice:str, history:HistoryState, menu:MenuState, reference:bytes, interaction:bytes)
  pure initial_history() -> HistoryState
  pure initial_menu() -> MenuState
  editor-binding keys(history:HistoryState, menu:MenuState) -> EditorUpdate

extern crate::document_source
  CommentMark(line:i64, count:i64)

extern crate::editor_view
  PreparedPresentation(reference:bytes, data:bytes, notice:str)
  EditorReserve(line:i64, height:i64)
  pure presentation_notice(document:&editor, prepared:&PreparedPresentation) -> str
  pure empty_presentation() -> PreparedPresentation
  pure no_reserve() -> EditorReserve
  editor-highlighter paint(prepared:PreparedPresentation)
  pure document_presentation(document:&editor, menu:MenuState, dark:bool, commented:[i64], marks:[CommentMark], focused:bool, reserve:EditorReserve) -> PreparedPresentation

extern crate::document_ingress
  DocumentSource(reference:bytes)
  DocumentItem(notice:str, source:bytes, text:str, cursor:bytes, error:str)
  pure source_reference(reference:bytes) -> DocumentSource
  pure empty_source() -> DocumentSource
  pure document_editor(text:str, cursor:bytes) -> editor
  subscription document_source(source:DocumentSource) -> DocumentItem

state
  pointer_y:f64 = 0.0
  comment_anchor_y:f64 = -1.0
  // The block the open rail anchors to, and the gap its inline card holds open
  // in the document. Line 0 — the title — is the page-scoped rail's anchor.
  comment_anchor_line:i64 = 0
  comments_card_height:f64 = 0.0
  document_reserve:EditorReserve = no_reserve()
  document_focused = false
  focus_query:i64 = 0
  document_paint:PreparedPresentation = empty_presentation()
  document:editor = ""
  document_history:HistoryState = initial_history()
  document_menu:MenuState = initial_menu()
  document_source_ref:DocumentSource = empty_source()
  document_installed:bytes = bytes()
  document_error = ""
  document_source_error = ""
  document_dark = false
  document_commented:[i64] = []
  document_marks:[CommentMark] = []
  active_palette:palette[AppTheme] = AppTheme.app
  connected = false
  loading = false
  busy = false
  page_link = ""
  pages:[PageItem] = []
  // CHROME, NOT FACTS: how wide the reader dragged the page list and whether
  // she has the `⋯` menu open. Neither leaves this view, and neither is
  // persisted — a fresh window opens on the default again.
  pages_viewport_width = 1280.0
  pages_viewport_height = 700.0
  // The document pane — the stack the sensor in `pages.ice` measures, not the
  // window — because the card is placed against the room the document has.
  // The default is a WIDE one on purpose: until the sensor has reported, the
  // card floats in the margin, which is the one placement that moves nothing.
  pages_pane_width = 1280.0
  sidebar_width = 230.0
  page_menu_open = false
  page_create_open = false
  active_page = ""
  active_page_title = ""
  active_page_parent = ""
  page_searching = false
  page_search_hits:[PageSearchHit] = []
  page_search_query = ""
  page_delete_armed = false
  autosave = "idle"
  page_refusal = ""
  subpages:[Subpage] = []
  orphaned_comment_drafts:[str] = []
  block_comments_open = false
  scope_target = ""
  scope_pinned = false
  scope_label = ""
  thread_total:i64 = 0
  comment_rows:[PageCommentThreadRow] = []
  comment_groups:[PageCommentGroup] = []
  resolved_comment_rows:[PageCommentThreadRow] = []
  threads_loading = false
  compose_hint = ""
  // CARD CHROME, THE READER'S OWN: which thread's reply box is open, which
  // threads she unfolded, and whether the settled ones are showing. None of
  // it leaves the view — the app holds no opinion about any of them.
  reply_thread = ""
  expanded_threads:[str] = []
  resolved_open = false
  // the last seed the app pushed: the count moves once per hand-back
  seed_rev:i64 = 0
  // the reader's own: the four drafts the screen edits
  page_draft = ""
  page_search_draft = ""
  block_comment_draft = ""
  reply_draft = ""
  host_error = ""
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false

// The facts are the host's: one subscription, one item per change. A
// subscription, not a mount task, so a replacement restored from this
// view's state asks for the facts again on its own.
subscribe
  mouse moved status=any -> comment_pointer_moved _ _
  mouse released status=any -> document_pointer_released _
  keyboard release status=any -> document_key_released _
  window focused -> document_window_focused
  window unfocused -> document_window_unfocused
  document_source(document_source_ref) when !empty(document_source_ref.reference) && document_source_ref.reference != document_installed -> document_arrived _
  props() -> props_arrived _

on comment_pointer_moved(_x, y)
  pointer_y = y

on document_pointer_released(_button)
  focus_query = focus_query + 1
  let query = focus_query
  let source = document_installed
  task widget focused #root/pages/document -> document_focus_checked query source _

on document_key_released(_key)
  focus_query = focus_query + 1
  let query = focus_query
  let source = document_installed
  task widget focused #root/pages/document -> document_focus_checked query source _

on document_window_focused
  focus_query = focus_query + 1
  let query = focus_query
  let source = document_installed
  task widget focused #root/pages/document -> document_focus_checked query source _

on document_window_unfocused
  focus_query = focus_query + 1
  document_focused = false
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused, document_reserve)

on document_focus_checked(query, source, focused)
  return if query != focus_query || source != document_installed || focused == document_focused
  document_focused = focused
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused, document_reserve)

on sidebar_resized(dx, _dy)
  sidebar_width = sidebar_width_after_delta(sidebar_width, dx, pages_viewport_width)

on pages_viewport_changed(width, height)
  pages_viewport_width = width
  pages_viewport_height = height
  sidebar_width = sidebar_width_after_delta(sidebar_width, 0.0, width)

// THE PANE DECIDES THE PLACEMENT, and the placement decides whether the
// document owes the card a gap. Both are recomputed on the sensor's own tick,
// so dragging a window across a threshold moves the card without the rail
// closing, reloading, or losing what is typed in it.
//
// A REPAINT IS THE WHOLE DOCUMENT, so neither this handler nor the one below
// costs one unless the GAP moved. A sensor reports on every layout the host
// does and the document behind it can be half a megabyte: repainting it per
// report spends the guest's tick fuel on a picture identical to the one
// already on screen, and a large document then traps mid-bootstrap.
on pages_pane_resized(width, _height)
  pages_pane_width = width
  let next = comments_reserve(width, block_comments_open, comment_anchor_line, comments_card_height)
  return if next.line == document_reserve.line && next.height == document_reserve.height
  document_reserve = next
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused, document_reserve)

// The gap is laid out from the card's own measured height, so the card is
// measured where it is drawn. `measured_card_height` refuses a move under a
// pixel: the gap must not chase its own occupant.
on comments_card_measured(_width, height)
  let measured = measured_card_height(comments_card_height, height)
  return if measured == comments_card_height
  comments_card_height = measured
  let next = comments_reserve(pages_pane_width, block_comments_open, comment_anchor_line, measured)
  return if next.line == document_reserve.line && next.height == document_reserve.height
  document_reserve = next
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused, document_reserve)

on toggle_page_menu
  page_menu_open = !page_menu_open

on close_page_menu
  page_menu_open = false

on props_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  let next = item.next
  connected = next.connected
  loading = next.loading
  busy = next.busy
  page_link = next.page_link
  pages = next.pages
  page_create_open = next.page_create_open
  comment_anchor_y = comment_anchor_after_props(active_page, next.active_page, next.block_comments_open, comment_anchor_y)
  comment_anchor_line = comment_line_after_props(active_page, next.active_page, next.block_comments_open, comment_anchor_line)
  document_reserve = comments_reserve(pages_pane_width, next.block_comments_open, comment_anchor_line, comments_card_height)
  // A REPLY IN PROGRESS BELONGS TO ONE CARD ON ONE PAGE. The card closing, or
  // the selection moving, takes the half-typed reply with it — read BEFORE
  // `active_page` moves, the same place the anchor is read.
  let comments_carry = active_page == next.active_page && next.block_comments_open
  reply_thread = keep_str(comments_carry, reply_thread, "")
  reply_draft = keep_str(comments_carry, reply_draft, "")
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  page_searching = next.page_searching
  page_search_hits = next.page_search_hits
  page_search_query = next.page_search_query
  page_delete_armed = next.page_delete_armed
  autosave = next.autosave
  page_refusal = next.page_refusal
  subpages = next.subpages
  orphaned_comment_drafts = next.orphaned_comment_drafts
  block_comments_open = next.block_comments_open
  scope_target = next.scope_target
  scope_pinned = next.scope_pinned
  scope_label = next.scope_label
  thread_total = next.thread_total
  // MIRRORED ONCE PER PUSH, not per frame: the split and the grouping are the
  // same answer for every frame the rows stand for.
  comment_rows = next.comment_rows
  comment_groups = comment_groups(next.comment_rows, next.active_page)
  resolved_comment_rows = resolved_rows(next.comment_rows)
  threads_loading = next.threads_loading
  compose_hint = next.compose_hint
  // THE APP HANDS A DRAFT BACK ONLY WHEN THE SEED MOVED — a recovered
  // comment taken up, a failed post returned, a failed page create
  // returned. Every other push leaves the reader's fields alone. A refused
  // post lands back in the box it left: the reply's thread, or the composer.
  let moved = next.seed_rev != seed_rev
  seed_rev = next.seed_rev
  page_draft = seeded(moved, next.page_seed, page_draft)
  block_comment_draft = seeded(moved && empty(reply_thread), next.comment_seed, block_comment_draft)
  reply_draft = seeded(moved && !empty(reply_thread), next.comment_seed, reply_draft)
  document_source_ref = source_reference(next.document_source)
  document_source_error = next.document_error
  document_dark = next.dark
  document_commented = next.commented_lines
  document_marks = next.comment_marks
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused, document_reserve)
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

// Failed facts may leave a different target on the host. Keep drafts and
// readable content, but reject even queued actions until valid facts recover.
on toggle_page_create
  return if !empty(host_error)
  sent = toggle_create()

// The title leaves with the act; the field clears here, as the app's
// handler used to clear it — a refused create hands it back as a seed.
on create_page_submit
  return if !empty(host_error)
  return if loading || busy || !connected || empty(trim(page_draft))
  sent = create(trim(page_draft), block_comment_draft)
  page_draft = ""

// A PICK ABANDONS THE RAIL'S DRAFT: it leaves with the act, for the app to
// keep as a recovered draft on the page it belonged to.
on choose_page(id)
  return if !empty(host_error)
  return if loading || busy
  sent = choose(id, block_comment_draft)
  block_comment_draft = ""

on search_pages_submit
  return if !empty(host_error)
  return if page_searching || empty(trim(page_search_draft))
  sent = search(trim(page_search_draft))

on clear_page_search
  return if !empty(host_error)
  sent = clear_search()
  page_search_draft = ""

// The menu's one item leaves with the act: the dialog it opens is the next
// thing the reader answers, and the menu must not be waiting behind it.
on arm_page_delete
  return if !empty(host_error)
  page_menu_open = false
  sent = arm_delete()

on disarm_page_delete
  return if !empty(host_error)
  sent = disarm_delete()

on delete_page_submit
  return if !empty(host_error)
  sent = delete(block_comment_draft)

on open_page_search_hit(page_id, block_id)
  return if !empty(host_error)
  return if loading || busy
  sent = open_hit(page_id, block_id, block_comment_draft)
  block_comment_draft = ""

on use_orphaned_comment_draft(draft)
  return if !empty(host_error)
  return if loading || busy || !empty(trim(block_comment_draft))
  sent = use_draft(draft, block_comment_draft)

on discard_orphaned_comment_draft(draft)
  return if !empty(host_error)
  sent = discard_draft(draft)

on toggle_block_comments
  comment_anchor_y = -1.0
  comment_anchor_line = 0
  reply_thread = ""
  reply_draft = ""
  return if !empty(host_error)
  return if loading || busy || empty(active_page)
  sent = toggle_comments(block_comment_draft)
  block_comment_draft = ""

on close_block_comments
  comment_anchor_y = -1.0
  comment_anchor_line = 0
  reply_thread = ""
  reply_draft = ""
  return if !empty(host_error)
  sent = close_comments(block_comment_draft)
  block_comment_draft = ""

// Narrowing and widening re-slice threads already in hand — the reply in
// progress belongs to a thread that may be about to leave the list.
on narrow_comment_scope(target)
  reply_thread = ""
  reply_draft = ""
  return if !empty(host_error)
  return if loading || busy
  sent = narrow(target)

on widen_comment_scope
  reply_thread = ""
  reply_draft = ""
  return if !empty(host_error)
  return if loading || busy
  sent = widen()

on resolve_thread_submit(id, resolved)
  return if !empty(host_error)
  sent = resolve(id, resolved)

// THE READER PICKS WHICH THREAD SHE IS ANSWERING, and a second press on the
// same one puts the box away. One box holds a draft at a time, so moving it
// is what drops the last one.
on select_reply_thread(id)
  reply_draft = ""
  reply_thread = reply_thread_after_press(reply_thread, id)

on toggle_thread_replies(id)
  expanded_threads = toggled(expanded_threads, id)

on toggle_resolved_comments
  resolved_open = !resolved_open

// A REPLY NAMES ITS THREAD; the composer at the card's foot names none, and
// the app opens a new thread on the card's scope for it. Either way the words
// leave with the act and the field clears — a refused post hands them back.
on post_thread_reply(id)
  return if !empty(host_error)
  return if busy || threads_loading || empty(trim(reply_draft))
  sent = post(trim(reply_draft), id)
  reply_draft = ""

on post_block_comment_submit
  return if !empty(host_error)
  return if busy || threads_loading || empty(trim(block_comment_draft))
  sent = post(trim(block_comment_draft), "")
  block_comment_draft = ""

on copy_to_clipboard(text, label)
  return if !empty(host_error)
  sent = copy(text, label)

on document_arrived(item)
  return if item.source != document_source_ref.reference
  document_error = item.error
  return if !empty(item.error)
  document = document_editor(item.text, item.cursor)
  document_installed = item.source
  sent = installed(document, document_installed)
  document_menu = initial_menu()
  document_focused = false
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused, document_reserve)
  focus_query = focus_query + 1
  let query = focus_query
  let source = document_installed
  task widget focused #root/pages/document -> document_focus_checked query source _

on document_committed(next)
  return if !empty(host_error)
  document_history = next.history
  document_menu = next.menu
  // The badge press carries its own line, and the paint below is what holds a
  // gap open under it — so the anchor is taken BEFORE the repaint, not after.
  let opens_comment = comment_navigation(next.interaction) && !busy
  comment_anchor_y = comment_anchor_after_navigation(opens_comment, pointer_y, comment_anchor_y)
  comment_anchor_line = comment_line_after_navigation(next.interaction, comment_anchor_line)
  document_paint = document_presentation(document, document_menu, document_dark, document_commented, document_marks, document_focused, document_reserve)
  sent = edited(document_installed, next.reference, next.interaction, seeded(opens_comment, block_comment_draft, ""))

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
          pane_width=pages_pane_width
          comments_right_anchor=comments_right_anchor(pages_pane_width)
          comments_left_inset=comments_left_inset(pages_pane_width)
          comments_height=comment_card_height(comment_anchor_y, pages_viewport_height)
          comments_offset=comment_card_offset(pages_pane_width, comment_anchor_y, pages_viewport_height)
          scope_target
          scope_pinned
          scope_label
          thread_total
          comment_groups
          resolved_comment_rows
          resolved_open
          reply_thread
          expanded_threads
          threads_loading
          compose_hint
        events
          toggle_page_create -> toggle_page_create
          create_page_submit -> create_page_submit
          choose_page -> choose_page _
          search_pages_submit -> search_pages_submit
          clear_page_search -> clear_page_search
          resize_sidebar -> sidebar_resized _ _
          resize_pane -> pages_pane_resized _ _
          measure_comments_card -> comments_card_measured _ _
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
            if !empty(document_source_error)
              text document_source_error @text-danger
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
                disabled=(!empty(host_error) || loading || !connected || empty(document_source_ref.reference) || document_installed != document_source_ref.reference)
              active bg=fg/0 value=document_ink placeholder=hint selection=document_selection border-w=0.0
              disabled bg=fg/0 value=document_ink placeholder=hint selection=document_selection border-w=0.0
