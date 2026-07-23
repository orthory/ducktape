on search_pages_submit
  return if page_searching || empty(trim(page_search_draft))
  page_search_generation = page_search_generation + 1
  page_searching = true
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

on open_page_search_hit(page_id, block_id)
  return if loading || mutation_phase != "idle"
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, [], selected_block_id, block_edit_draft, block_autosave_status)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], selected_block_id, block_comment_draft)
  block_autosave_generation = block_autosave_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
  page_search_hits = []
  selected_block_id = ""
  hovered_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
  block_actions_open = false
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
  page_title_selected = false
  block_edit_draft = ""
  block_autosave_status = "idle"
  block_insert_after_id = ""
  block_insert_open = !empty(block_draft)
  page_delete_armed = false
  block_delete_armed = false
  error = ""
  run load_page(connected_rpc, page_id, block_id) -> pages_updated _ | failed _

on choose_page(id)
  return if loading || mutation_phase != "idle"
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, [], selected_block_id, block_edit_draft, block_autosave_status)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], selected_block_id, block_comment_draft)
  block_autosave_generation = block_autosave_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
  page_search_hits = []
  selected_block_id = ""
  hovered_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
  block_actions_open = false
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
  page_title_selected = false
  block_edit_draft = ""
  block_autosave_status = "idle"
  block_insert_after_id = ""
  block_insert_open = !empty(block_draft)
  page_delete_armed = false
  block_delete_armed = false
  error = ""
  run load_page(connected_rpc, id, "") -> pages_updated _ | failed _

on create_page_submit
  return if loading || mutation_phase != "idle" || empty(trim(page_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "page"
  pending_page = trim(page_draft)
  page_draft = ""
  error = ""
  run create_page(connected_rpc, password, pending_page) -> pages_mutated _ | mutation_failed _

on toggle_page_create
  page_create_open = !page_create_open
  return if !page_create_open
  task widget focus #workspace-tabs/new-page

on focus_page_title(current_scope)
  task widget focus #workspace-tabs/page-title(current_scope)/title-input

on arm_page_delete
  return if loading || mutation_phase != "idle" || empty(active_page)
  page_delete_armed = true

on delete_page_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || !page_delete_armed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "page-delete"
  page_delete_armed = false
  error = ""
  run delete_page(connected_rpc, password, active_page) -> pages_mutated _ | mutation_failed _

on new_block_kind_changed(next)
  new_block_kind = next

on block_entered(id)
  hovered_block_id = id

on block_exited(id)
  return if hovered_block_id != id
  hovered_block_id = ""

on pages_pointer_moved(x, y)
  pages_pointer_x = x
  pages_pointer_y = y

on pages_resized(_, height)
  pages_height = height

on open_block_insert(key, after_id)
  return if loading || mutation_phase != "idle" || empty(active_page)
  block_insert_after_id = after_id
  block_insert_open = true
  task widget focus #workspace-tabs/key(key)/block-insert-row(block_insert_after_id)/block-insert

on open_root_block_insert
  return if loading || mutation_phase != "idle" || empty(active_page)
  block_insert_after_id = ""
  block_insert_open = true
  task widget focus #workspace-tabs/block-insert-row(block_insert_after_id)/block-insert

on close_block_insert
  return if mutation_phase != "idle"
  block_insert_open = false
  block_insert_after_id = ""

on use_orphaned_block_draft(draft)
  return if loading || mutation_phase != "idle" || !empty(block_draft)
  block_draft = draft
  block_insert_after_id = selected_block_id
  block_insert_open = true
  orphaned_block_drafts = remove_recovered_draft(orphaned_block_drafts, block_draft)

on discard_orphaned_block_draft(draft)
  orphaned_block_drafts = remove_recovered_draft(orphaned_block_drafts, draft)

on use_orphaned_comment_draft(draft)
  return if loading || mutation_phase != "idle" || !empty(block_draft)
  block_draft = draft
  block_insert_after_id = selected_block_id
  block_insert_open = true
  orphaned_comment_drafts = remove_recovered_draft(orphaned_comment_drafts, block_draft)

on discard_orphaned_comment_draft(draft)
  orphaned_comment_drafts = remove_recovered_draft(orphaned_comment_drafts, draft)

on add_block_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || (new_block_kind != "Divider" && empty(trim(block_draft)))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "block"
  pending_block = block_draft
  block_draft = ""
  blocks = optimistic_block(blocks, block_insert_after_id, new_block_kind, pending_block)
  error = ""
  run add_block(connected_rpc, password, active_page, block_insert_after_id, new_block_kind, pending_block) -> pages_mutated _ | mutation_failed _

on select_block(key, id, kind, text, checked, open_actions)
  return if mutation_phase != "idle"
  block_menu_x = pages_pointer_x
  block_menu_y = block_action_menu_y(pages_pointer_y, pages_height)
  block_actions_open = open_actions
  return if id == selected_block_id
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, [], selected_block_id, block_edit_draft, block_autosave_status)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], selected_block_id, block_comment_draft)
  block_autosave_generation = block_autosave_generation + 1
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
  selected_block_id = id
  selected_block_kind = kind
  selected_block_checked = checked
  page_title_selected = false
  block_edit_draft = text
  block_autosave_status = "idle"
  block_delete_armed = false
  return if open_actions
  task widget focus #workspace-tabs/key(key)/block(selected_block_id)/BlockLine/block-edit

on close_block_actions
  block_actions_open = false

on selected_block_kind_changed(next)
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(selected_block_id) || next == selected_block_kind
  selected_block_kind = next
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "block-kind"
  error = ""
  run save_block(connected_rpc, password, active_page, selected_block_id, selected_block_kind, block_edit_draft) -> pages_mutated _ | mutation_failed _

on clear_block_selection
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, [], selected_block_id, block_edit_draft, block_autosave_status)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], selected_block_id, block_comment_draft)
  block_autosave_generation = block_autosave_generation + 1
  selected_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
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
  block_edit_draft = ""
  block_autosave_status = "idle"
  block_delete_armed = false
  block_actions_open = false

on open_block_comments
  return if loading || mutation_phase != "idle" || empty(selected_block_id)
  block_comments_generation = block_comments_generation + 1
  block_actions_open = false
  block_comments_open = true
  block_comments_target = selected_block_id
  block_comment_threads = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = true
  active_block_comment_thread = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  error = ""
  run load_block_threads(connected_rpc, block_comments_target, 0, block_comments_generation) -> block_threads_loaded _ | block_threads_failed _

on close_block_comments
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], selected_block_id, block_comment_draft)
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
  return if next.generation != block_comments_generation || next.target != block_comments_target || next.target != selected_block_id || !block_comments_open
  block_comment_threads = next.threads
  block_comment_thread_total = next.total
  block_comment_threads_next_from = next.next_from
  block_comment_threads_has_more = next.has_more
  block_comment_threads_loading = false
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on load_more_block_threads
  return if block_comment_threads_loading || block_thread_comments_loading || mutation_phase != "idle" || !block_comments_open || !block_comment_threads_has_more
  block_comments_generation = block_comments_generation + 1
  block_comment_threads_loading = true
  error = ""
  run load_block_threads(connected_rpc, block_comments_target, block_comment_threads_next_from, block_comments_generation) -> block_threads_page_loaded _ | block_threads_failed _

on block_threads_page_loaded(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target || next.target != selected_block_id || !block_comments_open
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
  return if next.generation != block_comments_generation || next.target != block_comments_target || next.target != selected_block_id || next.thread_id != active_block_comment_thread || !block_comments_open
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
  return if next.generation != block_comments_generation || next.target != block_comments_target || next.target != selected_block_id || next.thread_id != active_block_comment_thread || !block_comments_open
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

on create_block_thread_submit
  return if loading || block_comment_threads_loading || block_thread_comments_loading || mutation_phase != "idle" || !block_comments_open || block_comments_target != selected_block_id || empty(trim(block_comment_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "block-comment"
  pending_block_comment = trim(block_comment_draft)
  block_comment_draft = ""
  block_comments_generation = block_comments_generation + 1
  block_thread_comments_loading = true
  error = ""
  run create_block_thread(connected_rpc, password, block_comments_target, pending_block_comment, block_comments_generation) -> block_thread_created _ | block_thread_create_failed _

on block_thread_created(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target || next.target != selected_block_id || !block_comments_open
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
  run load_block_threads(connected_rpc, block_comments_target, 0, block_comments_generation) -> block_threads_loaded _ | block_threads_failed _

on block_thread_create_failed(cause)
  block_comment_draft = restore_draft(block_comment_draft, pending_block_comment, cause.committed)
  pending_block_comment = ""
  mutation_phase = mutation_failure_phase(cause.committed)
  block_thread_comments_loading = false
  live_dirty = live_dirty || cause.committed
  error = cause.message
  block_comments_generation = block_comments_generation + 1
  block_comment_threads_loading = true
  run load_block_threads(connected_rpc, block_comments_target, 0, block_comments_generation) -> block_threads_recovered _ | block_threads_recovery_failed _

on block_threads_recovered(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target || next.target != selected_block_id || !block_comments_open
  block_comment_threads = next.threads
  block_comment_thread_total = next.total
  block_comment_threads_next_from = next.next_from
  block_comment_threads_has_more = next.has_more
  block_comment_threads_loading = false
  mutation_phase = "idle"
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on block_threads_recovery_failed(cause)
  return if cause.generation != block_comments_generation || !block_comments_open
  block_comment_threads_loading = false
  mutation_phase = "idle"
  error = cause.message
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on block_text_changed(next)
  block_edit_draft = next
  return if loading || empty(selected_block_id) || selected_block_kind == "Divider"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  block_autosave_status = "saving"
  block_autosave_generation = block_autosave_generation + 1
  error = ""
  run autosave_block_text(connected_rpc, password, selected_block_id, selected_block_kind, block_edit_draft, block_autosave_generation) -> block_text_saved _ | block_text_save_failed _

on block_text_saved(next)
  return if next.generation != block_autosave_generation
  return if !next.written
  block_autosave_status = "saved"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  live_dirty = false
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on block_text_save_failed(cause)
  return if cause.generation != block_autosave_generation
  block_autosave_status = "error"
  error = cause.message

on toggle_block_checked
  return if loading || mutation_phase != "idle" || selected_block_kind != "Todo" || empty(selected_block_id)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "block-check"
  error = ""
  run set_block_checked(connected_rpc, password, active_page, selected_block_id, !selected_block_checked) -> pages_mutated _ | mutation_failed _

on move_block_submit(direction)
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(selected_block_id)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "block-move"
  error = ""
  run move_block(connected_rpc, password, active_page, selected_block_id, direction) -> pages_mutated _ | mutation_failed _

on arm_block_delete
  return if loading || mutation_phase != "idle" || empty(selected_block_id)
  block_delete_armed = true

on remove_block_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(selected_block_id) || !block_delete_armed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "block-delete"
  block_delete_armed = false
  error = ""
  run remove_block(connected_rpc, password, active_page, selected_block_id) -> pages_mutated _ | mutation_failed _

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
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  selected_block_id = next.selected_block_id
  selected_block_kind = next.selected_block_kind
  selected_block_checked = next.selected_block_checked
  page_title_selected = next.page_title_selected
  block_edit_draft = next.selected_block_text
  block_autosave_status = "idle"
  block_autosave_generation = block_autosave_generation + 1
  block_delete_armed = false
  loading = false
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on pages_mutated(next)
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, [], selected_block_id, block_edit_draft, block_autosave_status)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], selected_block_id, block_comment_draft)
  block_autosave_generation = block_autosave_generation + 1
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  pending_page = ""
  page_create_open = false
  pending_block = ""
  block_insert_open = false
  block_insert_after_id = ""
  block_actions_open = false
  selected_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
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
  page_title_selected = false
  block_edit_draft = ""
  block_autosave_status = "idle"
  page_delete_armed = false
  block_delete_armed = false
  mutation_phase = "idle"
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _
