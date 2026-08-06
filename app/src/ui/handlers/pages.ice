on search_pages_submit
  return if page_searching || empty(trim(page_search_draft))
  page_search_generation = page_search_generation + 1
  page_searching = true
  page_search_hits = []
  error = ""
  run search_pages(connected_rpc, "", trim(page_search_draft), page_search_generation) -> page_search_loaded _ | page_search_failed _

on page_search_loaded(next)
  return if next.generation != page_search_generation
  page_search_hits = next.hits
  page_searching = false
  error = ""

on page_search_failed(cause)
  return if cause.generation != page_search_generation
  page_searching = false
  error = cause.message

on clear_page_search
  page_search_generation = page_search_generation + 1
  page_search_draft = ""
  page_search_hits = []
  page_searching = false

on open_page_search_hit(page_id, _block_id)
  return if loading || mutation_phase != "idle"
  palette_open = false
  shell_tab = "pages"
  page_search_generation = page_search_generation + 1
  page_searching = false
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], active_page, block_comment_draft)
  block_autosave_generation = block_autosave_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = true
  page_search_hits = []
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  block_autosave_status = "idle"
  page_delete_armed = false
  error = ""
  run load_page(connected_rpc, page_id) -> pages_updated _ | failed _

on choose_page(id)
  return if loading || mutation_phase != "idle"
  page_search_generation = page_search_generation + 1
  page_searching = false
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], active_page, block_comment_draft)
  block_autosave_generation = block_autosave_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = true
  page_search_hits = []
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  block_autosave_status = "idle"
  page_delete_armed = false
  error = ""
  run load_page(connected_rpc, id) -> pages_updated _ | failed _

on create_page_submit
  return if loading || mutation_phase != "idle" || empty(trim(page_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "page"
  pending_page = trim(page_draft)
  page_draft = ""
  error = ""
  run create_page(connected_rpc, password, pending_page) -> pages_mutated _ | mutation_failed _

on toggle_page_create
  page_create_open = !page_create_open
  return if !page_create_open
  task widget focus #workspace-tabs/content/pages/new-page

on arm_page_delete
  return if loading || mutation_phase != "idle" || empty(active_page)
  page_delete_armed = true

on disarm_page_delete
  page_delete_armed = false

on delete_page_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || !page_delete_armed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "page-delete"
  page_delete_armed = false
  error = ""
  run delete_page(connected_rpc, password, active_page) -> pages_mutated _ | mutation_failed _
on use_orphaned_comment_draft(draft)
  return if loading || mutation_phase != "idle" || !empty(trim(block_comment_draft))
  block_comment_draft = draft
  block_comments_open = true
  orphaned_comment_drafts = remove_recovered_draft(orphaned_comment_drafts, draft)

on discard_orphaned_comment_draft(draft)
  orphaned_comment_drafts = remove_recovered_draft(orphaned_comment_drafts, draft)
// THE COMMENTS RAIL IS DOCUMENT-SCOPED. The artifact lists every comment on the
// page under one `N comments` label and never involves a block selection
// (Liquid Glass:940-941). `load_page_threads` asks the node's own plural
// `ThreadsForTargets` query for the page AND all of its blocks at once, so the
// rail opens on a page, not on a block.
//
// The comment TARGET is therefore the page: a thread opened from here anchors
// on the document, which the module explicitly allows ("a block or page id",
// pages/src/interface.rs:278). A thread that some earlier build anchored on a
// block still LISTS here, but its comment page cannot be opened — the node
// validates the thread's own target against the one asked for, and `ThreadRow`
// reaches the app without it.
// One header control, so one handler: the open state flips first and every
// rail field is reset from it, then the guard below decides whether there is
// anything to load. Closing keeps the half-typed comment through the orphan
// guard, exactly as the rail's own × does.
on toggle_block_comments
  return if loading || mutation_phase != "idle" || empty(active_page)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], active_page, block_comment_draft)
  block_comments_generation = block_comments_generation + 1
  block_comments_open = !block_comments_open
  block_comments_target = keep_str(block_comments_open, active_page, "")
  block_comment_threads = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = block_comments_open
  active_block_comment_thread = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  error = ""
  return if !block_comments_open
  run load_page_threads(connected_rpc, active_page, block_comments_generation) -> block_threads_loaded _ | block_threads_failed _

on close_block_comments
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], active_page, block_comment_draft)
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""

on block_threads_loaded(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target || !block_comments_open
  block_comment_threads = next.threads
  block_comment_thread_total = next.total
  block_comment_threads_next_from = next.next_from
  block_comment_threads_has_more = next.has_more
  block_comment_threads_loading = false
  error = ""

// The pagination machinery stays wired, and the document query answers in one
// page (`has_more` false), so this only fires if that ever changes.
on load_more_block_threads
  return if block_comment_threads_loading || block_thread_comments_loading || mutation_phase != "idle" || !block_comments_open || !block_comment_threads_has_more
  block_comments_generation = block_comments_generation + 1
  block_comment_threads_loading = true
  error = ""
  run load_page_threads(connected_rpc, active_page, block_comments_generation) -> block_threads_page_loaded _ | block_threads_failed _

on block_threads_page_loaded(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target || !block_comments_open
  block_comment_threads = append_page_comment_threads(block_comment_threads, next.threads)
  block_comment_thread_total = next.total
  block_comment_threads_next_from = next.next_from
  block_comment_threads_has_more = next.has_more
  block_comment_threads_loading = false
  error = ""

on block_threads_failed(cause)
  return if cause.generation != block_comments_generation || !block_comments_open
  block_comment_threads_loading = false
  error = cause.message

on open_block_comment_thread(id)
  return if block_comment_threads_loading || block_thread_comments_loading || mutation_phase != "idle" || !block_comments_open || empty(id)
  block_comments_generation = block_comments_generation + 1
  active_block_comment_thread = id
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = true
  error = ""
  run load_block_comment_page(connected_rpc, block_comments_target, active_block_comment_thread, 0, block_comments_generation) -> block_comment_page_loaded _ | block_comment_page_failed _

on block_comment_page_loaded(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target || next.thread_id != active_block_comment_thread || !block_comments_open
  block_thread_comments = next.comments
  block_thread_comments_next_from = next.next_from
  block_thread_comments_has_more = next.has_more
  block_thread_comments_loading = false
  error = ""

on load_more_block_comments
  return if block_thread_comments_loading || block_comment_threads_loading || mutation_phase != "idle" || empty(active_block_comment_thread) || !block_thread_comments_has_more
  block_comments_generation = block_comments_generation + 1
  block_thread_comments_loading = true
  error = ""
  run load_block_comment_page(connected_rpc, block_comments_target, active_block_comment_thread, block_thread_comments_next_from, block_comments_generation) -> block_comment_page_appended _ | block_comment_page_failed _

on block_comment_page_appended(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target || next.thread_id != active_block_comment_thread || !block_comments_open
  block_thread_comments = append_page_comments(block_thread_comments, next.comments)
  block_thread_comments_next_from = next.next_from
  block_thread_comments_has_more = next.has_more
  block_thread_comments_loading = false
  error = ""

on block_comment_page_failed(cause)
  return if cause.generation != block_comments_generation || !block_comments_open
  block_thread_comments_loading = false
  error = cause.message

on close_block_comment_thread
  block_comments_generation = block_comments_generation + 1
  active_block_comment_thread = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false

on post_block_comment_submit
  return if loading || block_comment_threads_loading || block_thread_comments_loading || mutation_phase != "idle" || !block_comments_open || block_comments_target != active_page || empty(trim(block_comment_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "block-comment"
  pending_block_comment = trim(block_comment_draft)
  block_comment_draft = ""
  block_comments_generation = block_comments_generation + 1
  block_thread_comments_loading = true
  error = ""
  run post_block_comment(connected_rpc, password, block_comments_target, active_block_comment_thread, pending_block_comment, block_comments_generation) -> block_comment_posted _ | block_comment_post_failed _

on block_comment_posted(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target || !block_comments_open
  active_block_comment_thread = next.thread_id
  block_thread_comments = next.comments
  block_thread_comments_next_from = next.next_from
  block_thread_comments_has_more = next.has_more
  block_thread_comments_loading = false
  pending_block_comment = ""
  mutation_phase = "idle"
  error = ""
  block_comments_generation = block_comments_generation + 1
  block_comment_threads_loading = true
  run load_page_threads(connected_rpc, active_page, block_comments_generation) -> block_threads_loaded _ | block_threads_failed _

on block_comment_post_failed(cause)
  block_comment_draft = restore_draft(block_comment_draft, pending_block_comment, cause.committed)
  pending_block_comment = ""
  mutation_phase = mutation_failure_phase(cause.committed)
  block_thread_comments_loading = false
  error = cause.message
  block_comments_generation = block_comments_generation + 1
  block_comment_threads_loading = true
  run load_page_threads(connected_rpc, active_page, block_comments_generation) -> block_threads_recovered _ | block_threads_recovery_failed _

on block_threads_recovered(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target || !block_comments_open
  block_comment_threads = next.threads
  block_comment_thread_total = next.total
  block_comment_threads_next_from = next.next_from
  block_comment_threads_has_more = next.has_more
  block_comment_threads_loading = false
  mutation_phase = "idle"
  error = ""

on block_threads_recovery_failed(cause)
  return if cause.generation != block_comments_generation || !block_comments_open
  block_comment_threads_loading = false
  mutation_phase = "idle"
  error = cause.message
on pages_updated(next)
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  pages = next.pages
  // ONE install decision, decided against the PREVIOUS page identity and
  // applied to buffer and baseline together: the incoming page's text lands
  // when the page MOVED or a clean buffer actually differs; a dirty buffer on
  // the SAME page is the user mid-typing through a reload, and a reload must
  // never eat keystrokes.
  page_landing = page_document_text(next.active_page_title, next.blocks)
  page_install = install_decision(page_editor, active_page, next.active_page, page_saved_text, page_landing)
  blocks = merge_pending_blocks(next.blocks, blocks, active_page, next.active_page, "")
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  page_editor = installed_page_editor(page_editor, page_install, page_landing)
  page_saved_text = keep_str(page_install, page_landing, page_saved_text)
  page_refusal = ""
  block_autosave_status = "idle"
  block_autosave_generation = block_autosave_generation + 1
  loading = false
  error = ""
  doc_tabs = doc_tabs_with(doc_tabs, active_page)
  run save_doc_tabs(connected_rpc, doc_tabs) -> doc_tabs_saved _
on pages_mutated(next)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], active_page, block_comment_draft)
  block_autosave_generation = block_autosave_generation + 1
  pages = next.pages
  // The same one-decision install as `pages_updated` — create/delete moves
  // the selection to another page, and the buffer must follow it; a mutation
  // that stays on this page must not eat mid-flight keystrokes. Computed
  // BEFORE the assignments so both reads see the pre-move state (the pair
  // must move on one shared decision).
  page_landing = page_document_text(next.active_page_title, next.blocks)
  page_install = install_decision(page_editor, active_page, next.active_page, page_saved_text, page_landing)
  blocks = merge_pending_blocks(next.blocks, blocks, active_page, next.active_page, "")
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  pending_page = ""
  page_create_open = false
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  page_editor = installed_page_editor(page_editor, page_install, page_landing)
  page_saved_text = keep_str(page_install, page_landing, page_saved_text)
  page_refusal = ""
  block_autosave_status = "idle"
  page_delete_armed = false
  mutation_phase = "idle"
  error = ""

on doc_tabs_saved(_result)
  error = error

on doc_tabs_loaded(tabs)
  doc_tabs = tabs

on close_doc_tab(id)
  return if loading || mutation_phase != "idle"
  closing_doc_tab = id
  active_page = next_doc_tab(doc_tabs, closing_doc_tab, active_page)
  doc_tabs = doc_tabs_without(doc_tabs, closing_doc_tab)
  closing_doc_tab = ""
  // The same prologue as `choose_page`: `active_page` just moved under the
  // buffer, and without `loading` the next 900ms tick would write the OLD
  // page's text into the NEW page. `pages_updated` clears it and decides the
  // install; closing a background tab reloads the same page, which the
  // install decision keeps harmless for a dirty buffer.
  block_autosave_generation = block_autosave_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = true
  parallel
    run save_doc_tabs(connected_rpc, doc_tabs) -> doc_tabs_saved _
    run load_page(connected_rpc, active_page) -> pages_updated _ | failed _

// THE DOCUMENT'S ONE EDIT ROUTE. Every key lands here: `apply_page_action`
// resolves the list/indent behaviours in the buffer and NOTHING reaches the
// node — the save tick below is the only write path, which is what keeps
// typing at buffer speed on a consensus-backed document.
on page_edited(action)
  page_editor = apply_page_action(page_editor, action)
  // The refusal describes an edit that was already rolled back; the next
  // keystroke is the user moving on from it.
  page_refusal = ""

// THE PAGE SAVES ON A GATED TICK, not per keystroke: the editor's edits land
// in `page_editor` without passing through a handler on the way to the node,
// so dirtiness is the buffer's drift from `page_saved_text` and the subscribe
// block's `every` line only exists while that drift does.
on page_autosave_tick
  return if loading || empty(active_page) || mutation_phase != "idle"
  // One op chain at a time: a multi-op save routinely outlives the 900ms
  // tick, and a second chain against the same page defeats the ordering
  // rule the awaited loop exists for (backend/document.rs).
  return if block_autosave_status == "saving"
  let text = page_text(page_editor)
  return if text == page_saved_text
  // An open ``` swallows every line under it when parsed — the save waits
  // for the close instead of writing (or refusing) a half-typed fence, and
  // SAYS SO: a stale "✓ synced" over held-back text would be a lie.
  let fence_open = has_unclosed_fence(text)
  block_autosave_status = keep_str(!fence_open, block_autosave_status, "idle")
  page_refusal = keep_str(!fence_open, page_refusal, "the ``` fence is open — close it to save")
  return if fence_open
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  block_autosave_status = "saving"
  block_autosave_generation = block_autosave_generation + 1
  page_inflight_text = text
  error = ""
  run save_page_document(connected_rpc, password, active_page, text, block_autosave_generation) -> page_document_saved _ | page_document_save_failed _

// The baseline is the node's own text after a write, and the submitted text
// after a no-op — `saved_baseline` carries the reasoning. Either way anything
// typed during the round trip stays dirty, and a depth change that takes one
// `MoveBlock` per tick keeps ticking until the buffer and the node agree.
on page_document_saved(next)
  return if next.generation != block_autosave_generation
  pages = next.data.pages
  blocks = next.data.blocks
  active_page_title = next.data.active_page_title
  active_page_parent = next.data.active_page_parent
  page_refusal = next.refusal
  page_saved_text = saved_baseline(next.written, next.document, page_inflight_text)
  block_autosave_status = "saved"
  error = ""
  return if empty(next.refusal)
  // A REFUSED WRITE ROLLS THE BUFFER BACK — but only when nothing was typed
  // since the tick submitted. Otherwise the buffer is kept (the newest words
  // must survive), the baseline moves to the node's text, and the still-dirty
  // buffer re-plans on the next tick with the refusal line explaining why.
  let untouched = page_text(page_editor) == page_inflight_text
  page_editor = rolled_back_editor(page_editor, untouched, next.document)
  page_saved_text = next.document
  block_autosave_status = "idle"

on page_document_save_failed(cause)
  return if cause.generation != block_autosave_generation
  block_autosave_status = "error"
  error = cause.message
