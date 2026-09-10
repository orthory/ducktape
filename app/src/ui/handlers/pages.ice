// THE PAGES VIEW'S ACTS, one arm per intent (`PagesIntent`). The screen
// lives in the `pages` view now (crates/views/pages); every handler below
// this one is reached from here by a `flow` so its body stays where it was.
// A draft crosses only with the act that reads it: the arm writes it into
// the app-side field the handler always read, then routes. The three acts
// that abandon the rail's comment (a pick, a hit, the rail's toggle/close)
// carry it so the handler can keep it as a recovered draft.
on pages_view_event(event)
  match pages_intent(event)
    PagesIntent.toggle_create
      page_create_open = !page_create_open
    PagesIntent.create
      page_draft = event_text(event, "title")
      block_comment_draft = event_text(event, "comment_draft")
      flow
        from done true
        done -> create_page_submit()
    PagesIntent.choose
      block_comment_draft = event_text(event, "comment_draft")
      flow
        from done event_text(event, "id")
        done -> choose_page _
    PagesIntent.search
      page_search_draft = event_text(event, "query")
      flow
        from done true
        done -> search_pages_submit()
    PagesIntent.clear_search
      flow
        from done true
        done -> clear_page_search()
    PagesIntent.arm_delete
      flow
        from done true
        done -> arm_page_delete()
    PagesIntent.disarm_delete
      flow
        from done true
        done -> disarm_page_delete()
    PagesIntent.delete
      block_comment_draft = event_text(event, "comment_draft")
      flow
        from done true
        done -> delete_page_submit()
    // The same echo the chat plane uses to reach this two-arg handler: a
    // flow route takes one `_`, a run route takes the literal.
    PagesIntent.open_hit
      block_comment_draft = event_text(event, "comment_draft")
      run every duck_echo_str(event_text(event, "page_id")) -> open_page_search_hit(_, "") | external_url_failed _
    PagesIntent.use_draft
      block_comment_draft = event_text(event, "comment_draft")
      flow
        from done event_text(event, "draft")
        done -> use_orphaned_comment_draft _
    PagesIntent.discard_draft
      flow
        from done event_text(event, "draft")
        done -> discard_orphaned_comment_draft _
    // Recheck the accepted source and instance at the document handler.
    PagesIntent.edited
      flow
        from done event
        done -> page_edited _
    PagesIntent.toggle_comments
      block_comment_draft = event_text(event, "comment_draft")
      flow
        from done true
        done -> toggle_block_comments()
    PagesIntent.close_comments
      block_comment_draft = event_text(event, "comment_draft")
      flow
        from done true
        done -> close_block_comments()
    PagesIntent.open_thread
      flow
        from done event
        done -> open_block_comment_thread _
    PagesIntent.resolve
      flow
        from done event_flag(event, "resolved")
        done -> resolve_thread_submit _
    PagesIntent.more_threads
      flow
        from done true
        done -> load_more_block_threads()
    PagesIntent.close_thread
      flow
        from done true
        done -> close_block_comment_thread()
    PagesIntent.more_comments
      flow
        from done true
        done -> load_more_block_comments()
    PagesIntent.post
      block_comment_draft = event_text(event, "text")
      flow
        from done true
        done -> post_block_comment_submit()
    PagesIntent.copy
      toast = event_text(event, "label")
      toast_age = 0
      task clipboard write event_text(event, "text")

on search_pages_submit
  return if page_searching || empty(trim(page_search_draft))
  page_searching = true
  page_search_hits = []
  // The query the zero-hit plate will speak for, captured at the last place
  // the draft and the sent string are known to match — the full rationale
  // lives on the plate arm in `screens/pages.ice`.
  page_search_query = trim(page_search_draft)
  error = ""
  run replace lane=page_search search_pages(connected_rpc, "", page_search_query) -> page_search_loaded _ | page_search_failed _

// AN EMPTY QUERY MEANS NO SEARCH IS STANDING — every dismissal path clears
// `page_search_query`, so this guard drops a late reply the dismissal could
// not invalidate instead of restoring the hits float over the page just
// landed on and clobbering `error` (or, on the failure route, raising a
// banner for a search nobody is waiting on).
on page_search_loaded(next)
  return if empty(page_search_query)
  page_search_hits = next.hits
  page_searching = false
  error = ""

on page_search_failed(cause)
  return if empty(page_search_query)
  page_searching = false
  // A FAILED SEARCH FOUND NOTHING BECAUSE IT NEVER RAN. Dropping the query
  // takes the plate down; `error` carries the cause to the console column's
  // banner (nothing sits over it here — unlike the palette, see its arm).
  page_search_query = ""
  error = cause.message

on clear_page_search
  invalidate lane=page_search
  page_search_draft = ""
  page_search_hits = []
  page_searching = false
  page_search_query = ""

on open_page_search_hit(page_id, _block_id)
  return if loading || mutation_phase != MutationPhase.idle
  invalidate lane=page_search
  invalidate lane=page_autosave
  invalidate lane=block_threads
  invalidate lane=block_comments
  palette_open = false
  invalidate lane=account_ceremony
  invalidate lane=account_desktop_ceremony
  account_busy = account_busy && empty(account_ceremony_phase)
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""
  shell_tab = ShellTab.pages
  // Same tab-move rule as `select_shell_tab`.
  page_searching = false
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], active_page, block_comment_draft)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = true
  page_search_hits = []
  page_search_query = ""
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_rows = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  active_thread_target = ""
  active_thread_anchor = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  block_autosave_status = AutosaveStatus.idle
  page_delete_armed = false
  error = ""
  run replace lane=page_load load_page(connected_rpc, page_id) -> pages_updated _ | failed _

on choose_page(id)
  return if loading || mutation_phase != MutationPhase.idle
  invalidate lane=page_search
  invalidate lane=page_autosave
  invalidate lane=block_threads
  invalidate lane=block_comments
  page_searching = false
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], active_page, block_comment_draft)
  // THE SWITCH IS VISIBLE NOW — the same choreography as `choose_channel`. The
  // clicked page takes the sidebar highlight and the header title, and the
  // previous document leaves the pane, before the round trip: a click that
  // repaints nothing for the seconds a page load takes reads as a dead app.
  // Only `active_page` moves; `buffer_page` stays where the text came from,
  // which is what keeps the landing load a MOVE rather than a refresh.
  // Re-clicking the page already open moves nothing, so a same-page reload
  // still meets a buffer that the install decision can protect.
  let page_moved = id != active_page
  active_page = id
  active_page_title = page_display_title(pages, id, active_page_title)
  active_page_parent = keep_str(page_moved, "", active_page_parent)
  blocks = keep_blocks(page_moved, [], blocks)
  // The buffer and its baseline move together, always — a blank buffer with a
  // stale baseline would read as dirty and the save tick would write it back.
  page_text = installed_page_text(page_text, page_moved, "")
  page_saved_text = keep_str(page_moved, "", page_saved_text)
  buffer_page = keep_str(page_moved, "", buffer_page)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = true
  page_search_hits = []
  page_search_query = ""
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_rows = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  active_thread_target = ""
  active_thread_anchor = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  block_autosave_status = AutosaveStatus.idle
  page_delete_armed = false
  error = ""
  run replace lane=page_load load_page(connected_rpc, id) -> pages_updated _ | failed _

on create_page_submit
  return if loading || mutation_phase != MutationPhase.idle || empty(trim(page_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = MutationPhase.page
  pending_page = trim(page_draft)
  page_draft = ""
  error = ""
  run every create_page(connected_rpc, password, pending_page) -> pages_mutated _ | mutation_failed _

on toggle_page_create
  page_create_open = !page_create_open

on arm_page_delete
  return if loading || mutation_phase != MutationPhase.idle || empty(active_page)
  page_delete_armed = true

on disarm_page_delete
  page_delete_armed = false

on delete_page_submit
  return if loading || mutation_phase != MutationPhase.idle || empty(active_page) || !page_delete_armed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = MutationPhase.page_delete
  page_delete_armed = false
  error = ""
  run every delete_page(connected_rpc, password, active_page) -> pages_mutated _ | mutation_failed _
on use_orphaned_comment_draft(draft)
  return if loading || mutation_phase != MutationPhase.idle || !empty(trim(block_comment_draft))
  block_comment_draft = draft
  pages_seed_rev = pages_seed_rev + 1
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
  return if loading || mutation_phase != MutationPhase.idle || empty(active_page)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], active_page, block_comment_draft)
  block_comments_generation = block_comments_generation + 1
  block_comments_open = !block_comments_open
  block_comments_target = keep_str(block_comments_open, active_page, "")
  block_comment_threads = []
  block_comment_rows = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = block_comments_open
  active_block_comment_thread = ""
  active_thread_target = ""
  active_thread_anchor = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  error = ""
  return if !block_comments_open
  run replace lane=block_threads load_page_threads(connected_rpc, active_page, block_comments_generation) -> block_threads_loaded _ | block_threads_failed _

on close_block_comments
  invalidate lane=block_threads
  invalidate lane=block_comments
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], active_page, block_comment_draft)
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_rows = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  active_thread_target = ""
  active_thread_anchor = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""

on block_threads_loaded(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target || !block_comments_open
  block_comment_threads = next.threads
  block_comment_rows = page_comment_thread_rows(blocks, block_comment_threads, active_page)
  block_comment_thread_total = next.total
  commented_block_hits = commented_targets_of(next.threads, active_page)
  block_comment_threads_next_from = next.next_from
  block_comment_threads_has_more = next.has_more
  block_comment_threads_loading = false
  error = ""

// The pagination machinery stays wired, and the document query answers in one
// page (`has_more` false), so this only fires if that ever changes.
on load_more_block_threads
  return if block_comment_threads_loading || block_thread_comments_loading || mutation_phase != MutationPhase.idle || !block_comments_open || !block_comment_threads_has_more
  block_comments_generation = block_comments_generation + 1
  block_comment_threads_loading = true
  error = ""
  run replace lane=block_threads load_page_threads(connected_rpc, active_page, block_comments_generation) -> block_threads_page_loaded _ | block_threads_failed _

on block_threads_page_loaded(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target || !block_comments_open
  block_comment_threads = append_page_comment_threads(block_comment_threads, next.threads)
  block_comment_rows = page_comment_thread_rows(blocks, block_comment_threads, active_page)
  block_comment_thread_total = next.total
  block_comment_threads_next_from = next.next_from
  block_comment_threads_has_more = next.has_more
  block_comment_threads_loading = false
  error = ""

on block_threads_failed(cause)
  return if cause.generation != block_comments_generation || !block_comments_open
  block_comment_threads_loading = false
  error = cause.message

on open_block_comment_thread(event)
  let id = event_text(event, "id")
  let target = event_text(event, "target")
  return if block_comment_threads_loading || block_thread_comments_loading || mutation_phase != MutationPhase.idle || !block_comments_open || empty(id)
  block_comments_generation = block_comments_generation + 1
  active_block_comment_thread = id
  // The thread's OWN anchor, not the page: the node validates a comment read
  // against the thread's target, so a block-anchored thread opened with the
  // page id was refused — the rail could list it but never open it.
  active_thread_target = target
  active_thread_anchor = comment_anchor_label(blocks, active_thread_target, active_page)
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = true
  error = ""
  run replace lane=block_comments load_block_comment_page(connected_rpc, active_thread_target, active_block_comment_thread, 0, block_comments_generation) -> block_comment_page_loaded _ | block_comment_page_failed _

on block_comment_page_loaded(next)
  return if next.generation != block_comments_generation || next.target != active_thread_target || next.thread_id != active_block_comment_thread || !block_comments_open
  block_thread_comments = next.comments
  block_thread_comments_next_from = next.next_from
  block_thread_comments_has_more = next.has_more
  block_thread_comments_loading = false
  error = ""

on load_more_block_comments
  return if block_thread_comments_loading || block_comment_threads_loading || mutation_phase != MutationPhase.idle || empty(active_block_comment_thread) || !block_thread_comments_has_more
  block_comments_generation = block_comments_generation + 1
  block_thread_comments_loading = true
  error = ""
  run replace lane=block_comments load_block_comment_page(connected_rpc, active_thread_target, active_block_comment_thread, block_thread_comments_next_from, block_comments_generation) -> block_comment_page_appended _ | block_comment_page_failed _

on block_comment_page_appended(next)
  return if next.generation != block_comments_generation || next.target != active_thread_target || next.thread_id != active_block_comment_thread || !block_comments_open
  block_thread_comments = append_page_comments(block_thread_comments, next.comments)
  block_thread_comments_next_from = next.next_from
  block_thread_comments_has_more = next.has_more
  block_thread_comments_loading = false
  error = ""

on block_comment_page_failed(cause)
  return if cause.generation != block_comments_generation || !block_comments_open
  block_thread_comments_loading = false
  error = cause.message

on resolve_thread_submit(resolved)
  return if loading || mutation_phase != MutationPhase.idle || !block_comments_open || empty(active_block_comment_thread)
  mutation_phase = MutationPhase.comment_resolve
  error = ""
  run every resolve_comment_thread(connected_rpc, password, active_block_comment_thread, resolved) -> thread_resolved _ | thread_resolve_failed _

on thread_resolved(_written)
  mutation_phase = MutationPhase.idle
  block_comments_generation = block_comments_generation + 1
  block_comment_threads_loading = true
  error = ""
  run replace lane=block_threads load_page_threads(connected_rpc, active_page, block_comments_generation) -> block_threads_loaded _ | block_threads_failed _

on thread_resolve_failed(cause)
  mutation_phase = MutationPhase.idle
  error = cause.message

on close_block_comment_thread
  invalidate lane=block_comments
  block_comments_generation = block_comments_generation + 1
  active_block_comment_thread = ""
  active_thread_target = ""
  active_thread_anchor = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false

on post_block_comment_submit
  return if loading || block_comment_threads_loading || block_thread_comments_loading || mutation_phase != MutationPhase.idle || !block_comments_open || empty(active_page) || empty(trim(block_comment_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = MutationPhase.block_comment
  pending_block_comment = trim(block_comment_draft)
  block_comment_draft = ""
  // A reply stays on its thread's anchor; a NEW comment anchors on the block
  // the caret sits in — the Notion gesture — and on the page from the title
  // line (or before any edit placed the caret).
  let fresh_target = keep_str(!empty(caret_comment_target), caret_comment_target, active_page)
  active_thread_target = keep_str(!empty(active_block_comment_thread), active_thread_target, fresh_target)
  active_thread_anchor = comment_anchor_label(blocks, active_thread_target, active_page)
  block_comments_generation = block_comments_generation + 1
  block_thread_comments_loading = true
  error = ""
  run every post_block_comment(connected_rpc, password, active_thread_target, active_block_comment_thread, pending_block_comment, block_comments_generation) -> block_comment_posted _ | block_comment_post_failed _

on block_comment_posted(next)
  return if next.generation != block_comments_generation || next.target != active_thread_target || !block_comments_open
  active_block_comment_thread = next.thread_id
  block_thread_comments = next.comments
  block_thread_comments_next_from = next.next_from
  block_thread_comments_has_more = next.has_more
  block_thread_comments_loading = false
  pending_block_comment = ""
  mutation_phase = MutationPhase.idle
  error = ""
  block_comments_generation = block_comments_generation + 1
  block_comment_threads_loading = true
  run replace lane=block_threads load_page_threads(connected_rpc, active_page, block_comments_generation) -> block_threads_loaded _ | block_threads_failed _

on block_comment_post_failed(cause)
  block_comment_draft = restore_draft(block_comment_draft, pending_block_comment, cause.committed)
  pages_seed_rev = pages_seed_rev + 1
  pending_block_comment = ""
  mutation_phase = mutation_failure_phase(cause.committed)
  block_thread_comments_loading = false
  error = cause.message
  block_comments_generation = block_comments_generation + 1
  block_comment_threads_loading = true
  run replace lane=block_threads load_page_threads(connected_rpc, active_page, block_comments_generation) -> block_threads_recovered _ | block_threads_recovery_failed _

// A RECOVERY TERMINAL RELEASES ONLY A LOCK THAT IS STILL RECOVERING. "recovering"
// has a SECOND terminal now — `live_resynced` ends the one `mutation_failed`
// parks (lifecycle.ice), and it cannot tell whose recovery it is landing on top
// of — so this pair can arrive to find the lock already released and a fresh
// mutation (a channel create, a page delete) holding it. A flat `"idle"` there
// unlocks a write that is still in flight and lets its button be pressed twice.
// Same term on both arms: a failed recovery is no more entitled to it.
on block_threads_recovered(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target || !block_comments_open
  block_comment_threads = next.threads
  block_comment_rows = page_comment_thread_rows(blocks, block_comment_threads, active_page)
  block_comment_thread_total = next.total
  block_comment_threads_next_from = next.next_from
  block_comment_threads_has_more = next.has_more
  block_comment_threads_loading = false
  mutation_phase = mutation_phase_after_recovery(mutation_phase)
  error = ""

on block_threads_recovery_failed(cause)
  return if cause.generation != block_comments_generation || !block_comments_open
  block_comment_threads_loading = false
  mutation_phase = mutation_phase_after_recovery(mutation_phase)
  error = cause.message
on pages_updated(next)
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_rows = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  active_thread_target = ""
  active_thread_anchor = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  pages = next.pages
  // ONE install decision, decided against the page the BUFFER holds — never
  // against `active_page`, which moved to the clicked page the moment it was
  // clicked — and applied to buffer and baseline together: the incoming page's
  // text lands when the page MOVED or a clean buffer actually differs; a dirty
  // buffer on the SAME page is the user mid-typing through a reload, and a
  // reload must never eat keystrokes.
  let page_landing = page_document_text(next.active_page_title, next.blocks)
  let observed = current_page_document(network_chain_id, buffer_page, page_text)
  page_text = observed.text
  let page_install = (buffer_page != next.active_page || observed.ready) && install_decision(page_text, buffer_page, next.active_page, page_saved_text, page_landing)
  blocks = merge_pending_blocks(next.blocks, blocks, buffer_page, next.active_page, "")
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  page_text = installed_page_text(page_text, page_install, page_landing)
  page_saved_text = keep_str(page_install, page_landing, page_saved_text)
  // The buffer now holds THIS page. Unconditional on purpose: the install is
  // refused only when the decision already found the page unchanged.
  buffer_page = next.active_page
  page_refusal = ""
  block_comment_thread_total = next.comment_thread_total
  commented_block_hits = next.commented_block_hits
  caret_comment_target = ""
  block_autosave_status = AutosaveStatus.idle
  invalidate lane=page_autosave
  loading = false
  error = ""
on pages_mutated(next)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], active_page, block_comment_draft)
  // A create/delete moves the selection to another page — a navigation, so the
  // search answer is dismissed with it, hits included, and the lane is
  // invalidated so a reply in flight cannot put them back. A DISMISSAL POLICY,
  // not a truth requirement: page search passes an EMPTY scope and is
  // workspace-wide, so the answer would still be true here. The reason to drop
  // it is that a card left floating over the page you just landed on is in the
  // way. NOT hoisted into `pages_updated`: that also fires on a plain same-page
  // refresh, which would clear a standing search you are still reading.
  invalidate lane=page_search
  // THE INVALIDATE MUST LOWER THE FLAG WITH IT: the dropped reply was the only
  // thing that would ever clear `page_searching`, and the input is
  // `disabled=(!connected || page_searching)` — the field would stay dead with
  // no spinner and no explanation.
  page_searching = false
  page_search_hits = []
  page_search_query = ""
  invalidate lane=page_autosave
  pages = next.pages
  // The same one-decision install as `pages_updated` — create/delete moves
  // the selection to another page, and the buffer must follow it; a mutation
  // that stays on this page must not eat mid-flight keystrokes. Computed
  // BEFORE the assignments so both reads see the pre-move state (the pair
  // must move on one shared decision).
  let page_landing = page_document_text(next.active_page_title, next.blocks)
  let observed = current_page_document(network_chain_id, buffer_page, page_text)
  page_text = observed.text
  let page_install = (buffer_page != next.active_page || observed.ready) && install_decision(page_text, buffer_page, next.active_page, page_saved_text, page_landing)
  blocks = merge_pending_blocks(next.blocks, blocks, buffer_page, next.active_page, "")
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  pending_page = ""
  page_create_open = false
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_rows = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  active_thread_target = ""
  active_thread_anchor = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  page_text = installed_page_text(page_text, page_install, page_landing)
  page_saved_text = keep_str(page_install, page_landing, page_saved_text)
  buffer_page = next.active_page
  page_refusal = ""
  block_comment_thread_total = next.comment_thread_total
  commented_block_hits = next.commented_block_hits
  block_autosave_status = AutosaveStatus.idle
  page_delete_armed = false
  mutation_phase = MutationPhase.idle
  error = ""

// Accepted guest edits update the save buffer only after the host resolves
// their exact canonical reference in the current page and connection.
on page_edited(event)
  let document = accept_page_document(event, network_chain_id, active_page)
  return if !document.accepted
  page_text = document.text
  page_cursor_line = document.cursor_line
  caret_comment_target = block_at_line_target(blocks, keep_i64(document.comment_line >= 0, document.comment_line, page_cursor_line))
  // The refusal describes an edit that was already rolled back; the next
  // keystroke is the user moving on from it.
  page_refusal = ""
  // A margin-badge press opens the comments rail. Every rail field is already
  // at its reset value whenever the rail is closed (every close path resets
  // them), so opening is just the flip plus the thread load. A badge press
  // with the rail already open is a no-op.
  let page_rail_open = document.comment_line >= 0 && !block_comments_open && !loading && mutation_phase == MutationPhase.idle && !empty(active_page)
  block_comments_generation = block_comments_generation + keep_i64(page_rail_open, 1, 0)
  block_comments_open = block_comments_open || page_rail_open
  block_comments_target = keep_str(page_rail_open, active_page, block_comments_target)
  block_comment_threads_loading = block_comment_threads_loading || page_rail_open
  // A link press goes through the ONE open plane (`open_message_link`), not
  // straight to the OS: a page cites `duck://` addresses as readily as a chat
  // message does, and only that plane knows the module table and the
  // network scope. It never touched the buffer either way. The two runs are
  // exclusive by event kind; each backend treats an empty argument as "not my
  // turn" and answers without side effects.
  let page_link = document.link
  return if empty(page_link) && !page_rail_open
  parallel
    run every duck_echo_str(page_link) -> open_message_link _ | external_url_failed _
    run replace lane=block_threads load_page_threads(connected_rpc, keep_str(page_rail_open, active_page, ""), block_comments_generation) -> block_threads_loaded _ | block_threads_failed _

on external_url_opened(_opened)

on external_url_failed(cause)
  error = cause.message

// THE PAGE SAVES ON A GATED TICK, not per keystroke: the editor's edits land
// in `page_text` without passing through a handler on the way to the node,
// so dirtiness is the buffer's drift from `page_saved_text` and the subscribe
// block's `every` line only exists while that drift does.
on page_autosave_tick
  return if loading || empty(active_page) || mutation_phase != MutationPhase.idle
  // NEVER WRITE A BUFFER INTO A PAGE IT DOES NOT BELONG TO. `active_page` moves
  // the instant the reader clicks; the buffer only becomes that page's when a
  // load lands and stamps `buffer_page`. Between those two moments the pane is
  // blank, and it stays typable if the load FAILS — `on failed` clears
  // `loading` without clearing `connected` or putting `active_page` back. One
  // keystroke into that blank pane used to reach this tick, and a save of an
  // empty document against a real page is a `RemoveBlock` for every line it
  // had: the page the reader never got to see would be destroyed by the act of
  // failing to open it.
  return if active_page != buffer_page
  let observed = current_page_document(network_chain_id, buffer_page, page_text)
  page_text = observed.text
  return if !observed.ready
  // One op chain at a time: a multi-op save routinely outlives the 900ms
  // tick, and a second chain against the same page defeats the ordering
  // rule the awaited loop exists for (backend/document.rs).
  return if block_autosave_status == AutosaveStatus.saving
  let text = page_text
  return if text == page_saved_text
  // An open ``` swallows every line under it when parsed — the save waits
  // for the close instead of writing (or refusing) a half-typed fence, and
  // SAYS SO: a stale "✓ synced" over held-back text would be a lie.
  let fence_open = has_unclosed_fence(text)
  block_autosave_status = AutosaveStatus.idle
  page_refusal = keep_str(!fence_open, page_refusal, "the ``` fence is open — close it to save")
  return if fence_open
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  block_autosave_status = AutosaveStatus.saving
  page_inflight_text = text
  error = ""
  run latest lane=page_autosave save_page_document(connected_rpc, password, active_page, text, page_saved_text) -> page_document_saved _ | page_document_save_failed _

// The baseline is the node's own text after a write, and the submitted text
// after a no-op — `saved_baseline` carries the reasoning. Either way anything
// typed during the round trip stays dirty, and a depth change that takes one
// `MoveBlock` per tick keeps ticking until the buffer and the node agree.
on page_document_saved(next)
  pages = next.data.pages
  blocks = next.data.blocks
  block_comment_rows = page_comment_thread_rows(blocks, block_comment_threads, active_page)
  active_thread_anchor = comment_anchor_label(blocks, active_thread_target, active_page)
  active_page_title = next.data.active_page_title
  active_page_parent = next.data.active_page_parent
  page_refusal = next.refusal
  page_saved_text = baseline_at_submitted_title(saved_baseline(next.written, next.document, page_inflight_text), page_inflight_text)
  block_autosave_status = AutosaveStatus.saved
  error = ""
  return if empty(next.refusal)
  // A REFUSED WRITE ROLLS THE BUFFER BACK — but only when nothing was typed
  // since the tick submitted. Otherwise the buffer is kept (the newest words
  // must survive), the baseline moves to the node's text, and the still-dirty
  // buffer re-plans on the next tick with the refusal line explaining why.
  let observed = current_page_document(network_chain_id, buffer_page, page_text)
  page_text = observed.text
  let untouched = observed.ready && page_text == page_inflight_text
  page_text = rolled_back_text(page_text, untouched, next.document)
  // THE SUBMITTED TEXT, never the live buffer: she keeps typing through the
  // round trip, and `untouched` above exists because of it. Adopting her
  // unsaved line 0 here would make the document read clean and retire the very
  // tick that owes the node her rename.
  page_saved_text = baseline_at_submitted_title(next.document, page_inflight_text)
  block_autosave_status = AutosaveStatus.idle

on page_document_save_failed(cause)
  block_autosave_status = AutosaveStatus.error
  error = cause.message
