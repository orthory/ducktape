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

on open_page_search_hit(page_id)
  return if loading || mutation_phase != "idle"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
  page_search_hits = []
  selected_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
  block_edit_draft = ""
  block_autosave_status = "idle"
  page_delete_armed = false
  block_delete_armed = false
  error = ""
  run load_page(connected_rpc, page_id) -> pages_updated _ | failed _

on choose_page(id)
  return if loading || mutation_phase != "idle"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
  page_search_hits = []
  selected_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
  block_edit_draft = ""
  page_delete_armed = false
  block_delete_armed = false
  error = ""
  run load_page(connected_rpc, id) -> pages_updated _ | failed _

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

on add_block_submit
  return if loading || mutation_phase != "idle" || empty(active_page) || (new_block_kind != "Divider" && empty(trim(block_draft)))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "block"
  pending_block = block_draft
  block_draft = ""
  blocks = optimistic_block(blocks, new_block_kind, pending_block)
  error = ""
  run add_block(connected_rpc, password, active_page, selected_block_id, new_block_kind, pending_block) -> pages_mutated _ | mutation_failed _

on select_block(id, kind, text, checked)
  return if mutation_phase != "idle"
  selected_block_id = id
  selected_block_kind = kind
  selected_block_checked = checked
  block_edit_draft = text
  block_autosave_status = "idle"
  block_autosave_generation = block_autosave_generation + 1
  block_delete_armed = false

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
  selected_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
  block_edit_draft = ""
  block_autosave_status = "idle"
  block_autosave_generation = block_autosave_generation + 1
  block_delete_armed = false

on block_text_changed(next)
  block_edit_draft = next
  return if loading || empty(selected_block_id) || selected_block_kind == "Divider"
  block_autosave_status = "saving"
  block_autosave_generation = block_autosave_generation + 1
  error = ""
  run autosave_block_text(connected_rpc, password, selected_block_id, selected_block_kind, block_edit_draft, block_autosave_generation) -> block_text_saved _ | block_text_save_failed _

on block_text_saved(next)
  return if next.generation != block_autosave_generation
  return if !next.written
  block_autosave_status = "saved"

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
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  loading = false
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on pages_mutated(next)
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  pending_page = ""
  pending_block = ""
  selected_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
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
