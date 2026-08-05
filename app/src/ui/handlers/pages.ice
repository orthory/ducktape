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

on open_page_search_hit(page_id, block_id)
  return if loading || mutation_phase != "idle"
  palette_open = false
  shell_tab = "pages"
  page_search_generation = page_search_generation + 1
  page_searching = false
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, [], selected_block_id, trim(editor_text(block_editor)), selected_block_saved_text, block_autosave_status)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], selected_block_id, block_comment_draft)
  block_autosave_generation = block_autosave_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
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
  block_editor = editor("")
  selected_block_saved_text = ""
  block_autosave_status = "idle"
  block_insert_after_id = ""
  block_insert_open = !empty(block_draft)
  page_delete_armed = false
  block_delete_armed = false
  error = ""
  run load_page(connected_rpc, page_id, block_id) -> pages_updated _ | failed _

on choose_page(id)
  return if loading || mutation_phase != "idle"
  page_search_generation = page_search_generation + 1
  page_searching = false
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, [], selected_block_id, trim(editor_text(block_editor)), selected_block_saved_text, block_autosave_status)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], selected_block_id, block_comment_draft)
  block_autosave_generation = block_autosave_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
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
  block_editor = editor("")
  selected_block_saved_text = ""
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

on new_block_kind_changed(next)
  new_block_kind = next

on pick_slash_kind(kind)
  new_block_kind = kind
  block_draft = ""
  task widget focus #workspace-tabs/content/pages/block-insert-row(block_insert_after_id)/block-insert

on block_entered(id)
  hovered_block_id = id

on block_exited(id)
  return if hovered_block_id != id
  hovered_block_id = ""

// Captured per left press by `press-at` — see `chat_pointer_pressed` for why
// this is not a `move=` stream.
on pages_pointer_pressed(x, y)
  pages_pointer_x = x
  pages_pointer_y = y

on pages_resized(_width, height)
  pages_height = height

on open_block_insert(key, after_id)
  return if loading || empty(active_page)
  block_insert_after_id = after_id
  block_insert_open = true
  task widget focus #workspace-tabs/content/pages/key(key)/block-insert-row(block_insert_after_id)/block-insert

on open_root_block_insert
  return if loading || empty(active_page)
  block_insert_after_id = ""
  block_insert_open = true
  task widget focus #workspace-tabs/content/pages/block-insert-row(block_insert_after_id)/block-insert

on close_block_insert
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
  return if loading || empty(active_page) || (new_block_kind != "Divider" && empty(trim(block_draft)))
  return if !empty(slash_kind_matches(block_draft, editable_block_kinds))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  pending_block = block_draft
  pending_block_id = fresh_operation_id("block")
  block_draft = ""
  blocks = optimistic_block(blocks, block_insert_after_id, new_block_kind, pending_block, pending_block_id)
  error = ""
  run add_block(connected_rpc, password, active_page, block_insert_after_id, new_block_kind, pending_block_id, pending_block) -> block_added _ | block_add_failed _

on block_added(next)
  pages = next.data.pages
  return if active_page != next.page_id || next.data.active_page != next.page_id
  blocks = merge_block_insert_result(next.data.blocks, blocks, active_page, next.data.active_page, next.operation_id)
  block_insert_after_id = next.operation_id
  active_page_title = next.data.active_page_title
  active_page_parent = next.data.active_page_parent
  // List kinds continue themselves; a heading, quote or code block hands the
  // next row back to Text — the Notion cadence.
  new_block_kind = follow_kind(new_block_kind)
  error = ""
  // The settle moved the insert row under the block it just made, which
  // remounts the input — hand focus back so Enter-type-Enter keeps flowing.
  block_focus_key = block_key_of(blocks, block_insert_after_id)
  task widget focus #workspace-tabs/content/pages/key(block_focus_key)/block-insert-row(block_insert_after_id)/block-insert

on block_add_failed(cause)
  return if active_page != cause.scope_id
  blocks = rollback_pending_block(blocks, cause.operation_id, cause.committed)
  orphaned_block_drafts = remember_failed_block(orphaned_block_drafts, block_draft, cause.body, cause.committed)
  block_draft = restore_draft(block_draft, cause.body, cause.committed)
  error = cause.message
  return if !cause.committed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run live_resync_load(connected_rpc, active_channel, active_page, "pages", false, hydration_generation, 0) -> live_resynced _ | live_resync_failed _

on select_block(key, id, kind, text, checked, open_actions)
  block_menu_x = pages_pointer_x
  block_menu_y = block_action_menu_y(pages_pointer_y, pages_height)
  block_actions_open = open_actions
  return if id == selected_block_id
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, [], selected_block_id, trim(editor_text(block_editor)), selected_block_saved_text, block_autosave_status)
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
  block_editor = editor(text)
  selected_block_saved_text = text
  block_autosave_status = "idle"
  block_delete_armed = false
  return if open_actions
  task widget focus #workspace-tabs/content/pages/key(key)/block(selected_block_id)/line/block-edit(selected_block_kind)

on close_block_actions
  block_actions_open = false

on selected_block_kind_changed(next)
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(selected_block_id) || next == selected_block_kind
  selected_block_kind = next
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "block-kind"
  error = ""
  run save_block(connected_rpc, password, active_page, selected_block_id, selected_block_kind, trim(editor_text(block_editor))) -> pages_mutated _ | mutation_failed _

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
on open_block_comments
  return if loading || mutation_phase != "idle" || empty(active_page)
  block_comments_generation = block_comments_generation + 1
  block_actions_open = false
  block_comments_open = true
  block_comments_target = active_page
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
  run load_page_threads(connected_rpc, active_page, block_comments_generation) -> block_threads_loaded _ | block_threads_failed _

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

// THE BLOCK EDITOR SAVES ON A GATED TICK, not per keystroke: the stock
// editor's edits land in `block_editor` without passing through a handler, so
// dirtiness is the text's drift from `selected_block_saved_text` and the
// subscribe block's `every` line only fires while that drift exists. Every
// exit from editing (Enter, Esc, switching blocks or pages) still runs the
// orphan-draft guard, so a keystroke can never outlive its block unsaved.
on block_autosave_tick
  return if loading || empty(selected_block_id) || selected_block_kind == "Divider"
  let text = trim(editor_text(block_editor))
  return if text == selected_block_saved_text
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  block_autosave_status = "saving"
  block_autosave_generation = block_autosave_generation + 1
  block_autosave_inflight_text = text
  error = ""
  run autosave_block_text(connected_rpc, password, selected_block_id, selected_block_kind, text, block_autosave_generation) -> block_text_saved _ | block_text_save_failed _

on block_text_saved(next)
  return if next.generation != block_autosave_generation
  return if !next.written
  block_autosave_status = "saved"
  selected_block_saved_text = block_autosave_inflight_text

on block_text_save_failed(cause)
  return if cause.generation != block_autosave_generation
  block_autosave_status = "error"
  error = cause.message

// THE STRUCTURAL KEYS. One editor route carries them; the classify hop is the
// dispatch a flat handler cannot spell — state-only keys return on the Ok
// route, node-backed keys on the Err route (an identity run, not a failure).
on block_key(event)
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(selected_block_id)
  run classify_block_key(event) -> block_key_local _ | block_key_op _

// "split" opens the insert row under the block with the continuing kind and
// hands it focus; "escape" drops the selection (an unsaved draft goes to the
// recovered-drafts plate — the orphan guard inside `block_key_step`). The
// step is decided in Rust and applied field by field; the focus target of a
// non-split key names a row that is not mounted, which is a no-op.
on block_key_local(local)
  return if loading || mutation_phase != "idle" || empty(selected_block_id)
  block_key_action = local.action
  let next = block_key_step(block_key_action, blocks, selected_block_id, selected_block_kind, selected_block_checked, block_insert_open, block_insert_after_id, new_block_kind, trim(editor_text(block_editor)), selected_block_saved_text, block_autosave_status, orphaned_block_drafts)
  orphaned_block_drafts = next.orphaned
  block_autosave_generation = block_autosave_generation + next.autosave_bump
  selected_block_id = next.selected_id
  selected_block_kind = next.selected_kind
  selected_block_checked = next.selected_checked
  block_editor = retained_block_editor(block_editor, next.selected_id)
  selected_block_saved_text = retain_selected_string(selected_block_saved_text, next.selected_id)
  block_autosave_status = "idle"
  block_delete_armed = block_delete_armed && !empty(next.selected_id)
  block_actions_open = block_actions_open && !empty(next.selected_id)
  new_block_kind = next.insert_kind
  block_insert_open = next.insert_open
  block_insert_after_id = next.insert_after_id
  block_focus_key = next.focus_key
  task widget focus #workspace-tabs/content/pages/key(block_focus_key)/block-insert-row(block_insert_after_id)/block-insert

// Backspace-on-empty, Tab and Shift+Tab against the node. Delete lands the
// selection on the block above (`previous_block_id` rode along), so a
// Backspace chain walks up the page; `pages_mutated` refocuses the editor.
on block_key_op(op)
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(selected_block_id)
  block_key_action = op.action
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "block-key"
  error = ""
  run block_key_structure(connected_rpc, password, active_page, selected_block_id, block_key_action, previous_block_id(blocks, selected_block_id)) -> pages_mutated _ | mutation_failed _

// The insert draft's markdown shorthand: `# ` through `### `, `- `, `1. `,
// `[] `, `> `, three backticks and `---` convert the row's kind and strip
// themselves — the slash menu's faster sibling.
on block_draft_changed(next)
  let formatted = autoformat_block_draft(next, new_block_kind)
  block_draft = formatted.draft
  new_block_kind = formatted.kind

// ONE CLICK FINALIZES THE TICK. The artifact's todo box is the control itself
// (Liquid Glass:920-921), so the tick carries the block it belongs to and needs
// no selection round-trip; `checked` is the value being written, not the one
// being read back.
on set_todo_checked(id, checked)
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(id)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "block-check"
  error = ""
  run set_block_checked(connected_rpc, password, active_page, id, checked) -> pages_mutated _ | mutation_failed _

on move_block_submit(direction)
  return if loading || mutation_phase != "idle" || empty(active_page) || empty(selected_block_id)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
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
  mutation_phase = "block-delete"
  block_delete_armed = false
  error = ""
  run remove_block(connected_rpc, password, active_page, selected_block_id, previous_block_id(blocks, selected_block_id)) -> pages_mutated _ | mutation_failed _

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
  blocks = merge_pending_blocks(next.blocks, blocks, active_page, next.active_page, "")
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  selected_block_id = next.selected_block_id
  selected_block_kind = next.selected_block_kind
  selected_block_checked = next.selected_block_checked
  page_title_selected = next.page_title_selected
  block_editor = editor(next.selected_block_text)
  selected_block_saved_text = next.selected_block_text
  block_autosave_status = "idle"
  block_autosave_generation = block_autosave_generation + 1
  block_delete_armed = false
  loading = false
  error = ""
  doc_tabs = doc_tabs_with(doc_tabs, active_page)
  run save_doc_tabs(connected_rpc, doc_tabs) -> doc_tabs_saved _
on pages_mutated(next)
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, [], selected_block_id, trim(editor_text(block_editor)), selected_block_saved_text, block_autosave_status)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], selected_block_id, block_comment_draft)
  block_autosave_generation = block_autosave_generation + 1
  pages = next.pages
  blocks = merge_pending_blocks(next.blocks, blocks, active_page, next.active_page, "")
  block_insert_open = block_insert_open && active_page == next.active_page
  block_insert_after_id = refreshed_selected_block(next.blocks, block_insert_after_id)
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  pending_page = ""
  page_create_open = false
  block_actions_open = false
  selected_block_id = next.selected_block_id
  selected_block_kind = next.selected_block_kind
  selected_block_checked = next.selected_block_checked
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
  page_title_selected = next.page_title_selected
  block_editor = editor(next.selected_block_text)
  selected_block_saved_text = next.selected_block_text
  block_autosave_status = "idle"
  page_delete_armed = false
  block_delete_armed = false
  mutation_phase = "idle"
  error = ""
  // A mutation that keeps a selection (kind change, move, a Backspace delete
  // landing on the block above) hands the caret back to that block's editor;
  // with no selection the key is -1 and the task is a no-op.
  block_focus_key = block_key_of(blocks, selected_block_id)
  task widget focus #workspace-tabs/content/pages/key(block_focus_key)/block(selected_block_id)/line/block-edit(selected_block_kind)

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
  parallel
    run save_doc_tabs(connected_rpc, doc_tabs) -> doc_tabs_saved _
    run load_page(connected_rpc, active_page, "") -> pages_updated _ | failed _
