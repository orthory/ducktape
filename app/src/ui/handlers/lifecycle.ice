on mount
  loading = true
  run connect(rpc) -> workspace_connected _ | failed _

on reconnect
  return if loading || (mutation_phase != "idle" && mutation_phase != "recovering")
  rpc = canonical_endpoint(rpc)
  block_autosave_generation = cancel_autosaves(connected_rpc, block_autosave_generation)
  password = retain_for_endpoint(password, connected_rpc, rpc)
  channel_draft = retain_for_endpoint(channel_draft, connected_rpc, rpc)
  message_draft = retain_for_endpoint(trim(editor_text(message_editor)), connected_rpc, rpc)
  message_editor = editor(message_draft)
  failed_message_draft = retain_for_endpoint(failed_message_draft, connected_rpc, rpc)
  failed_reply_draft = retain_for_endpoint(failed_reply_draft, connected_rpc, rpc)
  chat_search_draft = retain_for_endpoint(chat_search_draft, connected_rpc, rpc)
  page_draft = retain_for_endpoint(page_draft, connected_rpc, rpc)
  block_draft = retain_for_endpoint(block_draft, connected_rpc, rpc)
  page_search_draft = retain_for_endpoint(page_search_draft, connected_rpc, rpc)
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, [], selected_block_id, block_edit_draft, block_autosave_status)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], selected_block_id, block_comment_draft)
  orphaned_block_drafts = retain_drafts_for_endpoint(orphaned_block_drafts, connected_rpc, rpc)
  orphaned_comment_drafts = retain_drafts_for_endpoint(orphaned_comment_drafts, connected_rpc, rpc)
  connected_rpc = rpc
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "idle"
  loading = true
  connected = false
  channels = []
  messages = []
  channel_reads = []
  unread_boundary = 0
  active_channel = ""
  active_channel_name = ""
  active_channel_archived = false
  active_channel_members_only = false
  active_channel_huddle_count = 0
  channel_members = []
  channel_settings_open = false
  channel_name_draft = ""
  member_key_draft = ""
  selected_message_seq = 0
  hovered_message_seq = 0
  selected_message_rev = 0
  message_action = "toolbar"
  message_edit_draft = ""
  active_thread_seq = 0
  thread_target_seq = 0
  thread_messages = []
  thread_next_reply_offset = 0
  thread_has_more = false
  thread_generation = thread_generation + 1
  live_thread_generation = live_thread_generation + 1
  thread_loading = false
  reply_draft = ""
  reply_editor = editor("")
  pending_reply = ""
  pending_channel = ""
  pending_message = ""
  chat_search_hits = []
  chat_search_generation = chat_search_generation + 1
  chat_searching = false
  pages = []
  doc_tabs = []
  blocks = []
  active_page = ""
  active_page_title = ""
  active_page_parent = ""
  pending_page = ""
  pending_block = ""
  block_insert_after_id = ""
  block_insert_open = !empty(block_draft)
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
  page_delete_armed = false
  block_delete_armed = false
  page_search_hits = []
  page_search_generation = page_search_generation + 1
  page_searching = false
  error = ""
  status = "Connecting…"
  run connect(connected_rpc) -> workspace_connected _ | failed _

on workspace_connected(next)
  rpc = next.rpc
  connected_rpc = next.rpc
  status = next.status
  block_height = next.height
  channels = next.channels
  channel_reads = initial_channel_reads(next.channels, channel_reads)
  unread_boundary = 0
  messages = merge_pending_messages(next.messages, messages, active_channel, next.active_channel, "")
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  active_channel_huddle_count = next.active_channel_huddle_count
  channel_members = next.channel_members
  pages = next.pages
  blocks = merge_pending_blocks(next.blocks, blocks, active_page, next.active_page, "")
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  connected = true
  loading = false
  mutation_phase = "idle"
  hydration_retry_attempt = 0
  error = ""
  run load_doc_tabs(connected_rpc) -> doc_tabs_loaded _

on live_updated(next)
  status = next.status
  return if next.kind == "retry"
  block_height = keep_i64(next.height >= 0, next.height, block_height)
  channels = apply_chat_channels(channels, next.chat)
  messages = apply_chat_messages(messages, next.chat, active_channel)
  thread_messages = apply_chat_thread(thread_messages, next.chat, active_channel, active_thread_seq)
  channel_members = apply_chat_members(channel_members, next.chat, active_channel)
  thread_next_reply_offset = thread_offset_after_live(thread_next_reply_offset, thread_has_more, next.chat, active_channel, active_thread_seq)
  active_channel_name = channel_display_name(channels, active_channel, active_channel_name)
  active_channel_archived = channel_flag_archived(channels, active_channel, active_channel_archived)
  active_channel_members_only = channel_flag_members_only(channels, active_channel, active_channel_members_only)
  active_channel_huddle_count = channel_live_huddle_count(channels, active_channel, active_channel_huddle_count)
  channel_reads = mark_channel_read(channel_reads, active_channel, channel_head_seq(channels, active_channel))
  return if !next.load_chat && !next.load_pages
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run live_resync_load(connected_rpc, active_channel, active_page, resync_planes(next.load_chat, next.load_pages), next.debounce, hydration_generation, 0) -> live_resynced _ | live_resync_failed _

on live_resynced(next)
  return if next.generation != hydration_generation
  hydration_retry_attempt = 0
  channels = keep_channels(next.chat_loaded, next.channels, channels)
  channel_reads = initial_channel_reads(channels, channel_reads)
  messages = keep_messages(next.chat_loaded, merge_pending_messages(next.messages, messages, active_channel, next.active_channel, ""), messages)
  failed_message_draft = remember_failed_draft(failed_message_draft, "channel", message_draft, active_channel == keep_str(next.chat_loaded, next.active_channel, active_channel))
  selected_message_seq = refreshed_required_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), selected_message_seq)
  failed_message_draft = remember_failed_draft(failed_message_draft, message_action, message_edit_draft, selected_message_seq > 0 || message_action != "editing")
  selected_message_rev = message_seq_after_failure(selected_message_rev, "message-edit", selected_message_seq <= 0)
  message_action = message_action_after_failure(message_action, "message-edit", selected_message_seq <= 0)
  message_edit_draft = message_text_after_failure(message_edit_draft, "message-edit", selected_message_seq <= 0)
  hovered_message_seq = refreshed_required_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), hovered_message_seq)
  channel_settings_open = channel_settings_open && active_channel == keep_str(next.chat_loaded, next.active_channel, active_channel)
  channel_name_draft = retain_for_endpoint(channel_name_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  member_key_draft = retain_for_endpoint(member_key_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  thread_generation = thread_generation_after_refresh(thread_generation, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq, refreshed_known_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq))
  thread_loading = thread_loading_after_refresh(thread_loading, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq, refreshed_known_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq))
  failed_reply_draft = retain_for_endpoint(failed_reply_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  active_thread_seq = refreshed_known_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq)
  failed_reply_draft = remember_failed_draft(failed_reply_draft, "thread", reply_draft, active_thread_seq > 0)
  thread_target_seq = refreshed_channel_value(active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), thread_target_seq)
  thread_next_reply_offset = refreshed_channel_value(active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), thread_next_reply_offset)
  thread_messages = retain_thread_messages(thread_messages, active_thread_seq)
  thread_has_more = thread_has_more && active_channel == keep_str(next.chat_loaded, next.active_channel, active_channel) && active_thread_seq > 0
  reply_draft = retain_for_endpoint(reply_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  pending_reply = retain_for_endpoint(pending_reply, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  message_draft = retain_for_endpoint(message_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  message_editor = editor(message_draft)
  pending_message = retain_for_endpoint(pending_message, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  active_channel = keep_str(next.chat_loaded, next.active_channel, active_channel)
  active_channel_name = keep_str(next.chat_loaded, next.active_channel_name, active_channel_name)
  active_channel_archived = keep_bool(next.chat_loaded, next.active_channel_archived, active_channel_archived)
  active_channel_members_only = keep_bool(next.chat_loaded, next.active_channel_members_only, active_channel_members_only)
  active_channel_huddle_count = keep_i64(next.chat_loaded, next.active_channel_huddle_count, active_channel_huddle_count)
  channel_members = keep_members(next.chat_loaded, next.channel_members, channel_members)
  unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, active_channel, unread_boundary)
  channel_reads = mark_channel_read(channel_reads, active_channel, channel_head_seq(channels, active_channel))
  pages = keep_pages(next.pages_loaded, next.pages, pages)
  blocks = keep_blocks(next.pages_loaded, merge_pending_blocks(next.blocks, blocks, active_page, next.active_page, ""), blocks)
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, blocks, selected_block_id, block_edit_draft, block_autosave_status)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, blocks, selected_block_id, block_comment_draft)
  block_edit_draft = refreshed_block_draft(blocks, selected_block_id, block_edit_draft, block_autosave_status)
  block_autosave_generation = cancel_missing_block_autosave(connected_rpc, block_autosave_generation, blocks, selected_block_id)
  selected_block_id = refreshed_selected_block(blocks, selected_block_id)
  selected_block_kind = retain_selected_string(selected_block_kind, selected_block_id)
  selected_block_checked = selected_block_checked && !empty(selected_block_id)
  block_comments_open = block_comments_open && !empty(selected_block_id)
  block_comments_target = retain_selected_string(block_comments_target, selected_block_id)
  block_comment_threads = retain_selected_comment_threads(block_comment_threads, selected_block_id)
  block_comment_thread_total = retain_selected_i64(block_comment_thread_total, selected_block_id)
  block_comment_threads_next_from = retain_selected_i64(block_comment_threads_next_from, selected_block_id)
  block_comment_threads_has_more = block_comment_threads_has_more && !empty(selected_block_id)
  block_comment_threads_loading = block_comment_threads_loading && !empty(selected_block_id)
  active_block_comment_thread = retain_selected_string(active_block_comment_thread, selected_block_id)
  block_thread_comments = retain_selected_comments(block_thread_comments, selected_block_id)
  block_thread_comments_next_from = retain_selected_i64(block_thread_comments_next_from, selected_block_id)
  block_thread_comments_has_more = block_thread_comments_has_more && !empty(selected_block_id)
  block_thread_comments_loading = block_thread_comments_loading && !empty(selected_block_id)
  block_comment_draft = retain_selected_string(block_comment_draft, selected_block_id)
  pending_block_comment = retain_selected_string(pending_block_comment, selected_block_id)
  block_edit_draft = retain_selected_string(block_edit_draft, selected_block_id)
  block_delete_armed = block_delete_armed && !empty(selected_block_id)
  block_actions_open = block_actions_open && !empty(selected_block_id)
  block_insert_open = block_insert_open && active_page == keep_str(next.pages_loaded, next.active_page, active_page)
  block_insert_after_id = refreshed_selected_block(blocks, block_insert_after_id)
  page_delete_armed = page_delete_armed && active_page == keep_str(next.pages_loaded, next.active_page, active_page)
  page_title_selected = page_title_selected && active_page == keep_str(next.pages_loaded, next.active_page, active_page)
  active_page = keep_str(next.pages_loaded, next.active_page, active_page)
  active_page_title = keep_str(next.pages_loaded, next.active_page_title, active_page_title)
  active_page_parent = keep_str(next.pages_loaded, next.active_page_parent, active_page_parent)
  error = ""
  block_comments_generation = block_comments_generation + 1
  live_thread_generation = live_thread_generation + 1
  parallel
    run refresh_live_thread(connected_rpc, active_channel, active_thread_seq, thread_target_seq, thread_next_reply_offset, live_thread_generation) -> live_thread_refreshed _ | live_thread_refresh_failed _
    run refresh_block_comments(connected_rpc, block_comments_target, active_block_comment_thread, block_comments_generation) -> live_block_comments_refreshed _ | live_block_comments_failed _

on live_resync_failed(cause)
  return if cause.generation != hydration_generation
  status = "Sync delayed"
  error = "Live sync interrupted. Retrying…"
  hydration_retry_attempt = hydration_retry_attempt + 1
  run live_resync_load(connected_rpc, active_channel, active_page, "both", false, hydration_generation, hydration_retry_attempt) -> live_resynced _ | live_resync_failed _

on live_thread_refreshed(next)
  return if next.generation != live_thread_generation
  return if next.channel_id != active_channel || next.root_seq != active_thread_seq
  return if thread_loading || mutation_phase != "idle"
  thread_target_seq = next.target_seq
  thread_messages = merge_pending_messages(next.messages, thread_messages, active_channel, next.channel_id, "")
  thread_next_reply_offset = next.next_reply_offset
  thread_has_more = next.has_more

on live_thread_refresh_failed(cause)
  return if cause.generation != live_thread_generation

on live_block_comments_refreshed(next)
  return if next.generation != block_comments_generation || next.target != block_comments_target
  block_comment_threads_loading = false
  block_thread_comments_loading = false
  block_comment_threads = next.threads
  block_comment_thread_total = next.total
  block_comment_threads_next_from = next.threads_next_from
  block_comment_threads_has_more = next.threads_has_more
  active_block_comment_thread = next.thread_id
  block_thread_comments = next.comments
  block_thread_comments_next_from = next.comments_next_from
  block_thread_comments_has_more = next.comments_has_more

on live_block_comments_failed(cause)
  return if cause.generation != block_comments_generation
  block_comment_threads_loading = false
  block_thread_comments_loading = false
  error = cause.message

on select_shell_tab(next)
  shell_tab = next
  return if shell_tab != "explorer" || !connected
  explorer_generation = explorer_generation + 1
  explorer_loading = true
  run load_explorer(connected_rpc, explorer_generation) -> explorer_loaded _ | explorer_failed _

on refresh_explorer
  return if !connected || explorer_loading
  explorer_generation = explorer_generation + 1
  explorer_loading = true
  run load_explorer(connected_rpc, explorer_generation) -> explorer_loaded _ | explorer_failed _

on explorer_loaded(next)
  return if next.generation != explorer_generation
  explorer_loading = false
  explorer_blocks = next.blocks
  explorer_ops = next.ops

on explorer_failed(cause)
  return if cause.generation != explorer_generation
  explorer_loading = false
  error = cause.message

on select_explorer_block(height)
  explorer_selected = height

on toggle_palette
  return if !connected
  palette_open = !palette_open
  palette_draft = ""
  palette_chat_hits = []
  palette_page_hits = []
  palette_generation = palette_generation + 1
  palette_searching = false
  return if !palette_open
  task widget focus #workspace-tabs/palette-input

on close_palette
  palette_open = false

on global_key_pressed(event)
  palette_key = palette_key_action(event.physical_key, event.modifiers, palette_open)
  return if palette_key == "none" || !connected
  palette_open = palette_key == "open"
  palette_key = ""
  palette_draft = ""
  palette_chat_hits = []
  palette_page_hits = []
  palette_generation = palette_generation + 1
  palette_searching = false
  return if !palette_open
  task widget focus #workspace-tabs/palette-input

on palette_changed(next)
  palette_draft = next
  palette_generation = palette_generation + 1
  palette_searching = !empty(trim(palette_draft))
  return if empty(trim(palette_draft))
  parallel
    run search_chat(connected_rpc, "", trim(palette_draft), palette_generation) -> palette_chat_loaded _ | palette_search_failed _
    run search_pages(connected_rpc, "", trim(palette_draft), palette_generation) -> palette_page_loaded _ | palette_search_failed _

on palette_chat_loaded(next)
  return if next.generation != palette_generation
  palette_chat_hits = next.hits
  palette_searching = false

on palette_page_loaded(next)
  return if next.generation != palette_generation
  palette_page_hits = next.hits
  palette_searching = false

on palette_search_failed(cause)
  return if cause.generation != palette_generation
  palette_searching = false

subscribe
  run live_events(connected_rpc) when connected -> live_updated _
  keyboard press when connected -> global_key_pressed _

on mutation_failed(cause)
  selected_message_seq = message_seq_after_failure(selected_message_seq, mutation_phase, cause.committed)
  selected_message_rev = message_seq_after_failure(selected_message_rev, mutation_phase, cause.committed)
  message_action = message_action_after_failure(message_action, mutation_phase, cause.committed)
  message_edit_draft = message_text_after_failure(message_edit_draft, mutation_phase, cause.committed)
  thread_selected_seq = message_seq_after_failure(thread_selected_seq, mutation_phase, cause.committed)
  thread_selected_rev = message_seq_after_failure(thread_selected_rev, mutation_phase, cause.committed)
  thread_message_action = message_action_after_failure(thread_message_action, mutation_phase, cause.committed)
  thread_edit_draft = message_text_after_failure(thread_edit_draft, mutation_phase, cause.committed)
  mutation_phase = mutation_failure_phase(cause.committed)
  channel_draft = restore_draft(channel_draft, pending_channel, cause.committed)
  page_draft = restore_draft(page_draft, pending_page, cause.committed)
  pending_channel = ""
  pending_page = ""
  error = cause.message
  return if !cause.committed
  block_autosave_generation = cancel_autosaves(connected_rpc, block_autosave_generation)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run live_resync_load(connected_rpc, active_channel, active_page, "both", false, hydration_generation, 0) -> live_resynced _ | live_resync_failed _

on dismiss_error
  error = ""

on restore_failed_message
  return if empty(failed_message_draft) || !empty(trim(editor_text(message_editor))) || mutation_phase != "idle"
  message_draft = failed_message_draft
  message_editor = editor(message_draft)
  failed_message_draft = ""

on dismiss_failed_message
  failed_message_draft = ""

on restore_failed_reply
  return if empty(failed_reply_draft) || !empty(trim(editor_text(reply_editor)))
  reply_draft = failed_reply_draft
  reply_editor = editor(reply_draft)
  failed_reply_draft = ""
  task widget focus #workspace-tabs/reply

on dismiss_failed_reply
  failed_reply_draft = ""

on failed(cause)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = false
  status = "Offline"
  error = cause.message
