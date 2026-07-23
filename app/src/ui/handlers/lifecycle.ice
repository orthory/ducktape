on mount
  loading = true
  run connect(rpc) -> workspace_connected _ | failed _

on reconnect
  return if loading || (mutation_phase != "idle" && mutation_phase != "recovering")
  rpc = canonical_endpoint(rpc)
  block_autosave_generation = cancel_autosaves(connected_rpc, block_autosave_generation)
  password = retain_for_endpoint(password, connected_rpc, rpc)
  channel_draft = retain_for_endpoint(channel_draft, connected_rpc, rpc)
  message_draft = retain_for_endpoint(message_draft, connected_rpc, rpc)
  failed_message_draft = retain_for_endpoint(failed_message_draft, connected_rpc, rpc)
  chat_search_draft = retain_for_endpoint(chat_search_draft, connected_rpc, rpc)
  page_draft = retain_for_endpoint(page_draft, connected_rpc, rpc)
  block_draft = retain_for_endpoint(block_draft, connected_rpc, rpc)
  page_search_draft = retain_for_endpoint(page_search_draft, connected_rpc, rpc)
  connected_rpc = rpc
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "idle"
  live_dirty = false
  loading = true
  connected = false
  channels = []
  messages = []
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
  selected_message_rev = 0
  message_action = "toolbar"
  message_edit_draft = ""
  active_thread_seq = 0
  thread_target_seq = 0
  thread_messages = []
  thread_next_reply_offset = 0
  thread_has_more = false
  thread_generation = thread_generation + 1
  thread_loading = false
  reply_draft = ""
  pending_reply = ""
  pending_channel = ""
  pending_message = ""
  chat_search_hits = []
  chat_search_generation = chat_search_generation + 1
  chat_searching = false
  pages = []
  blocks = []
  active_page = ""
  active_page_title = ""
  active_page_parent = ""
  pending_page = ""
  pending_block = ""
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
  messages = next.messages
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  active_channel_huddle_count = next.active_channel_huddle_count
  channel_members = next.channel_members
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  connected = true
  loading = false
  mutation_phase = "idle"
  sync_phase = "idle"
  hydration_retry_attempt = 0
  error = ""

on workspace_refreshed(next)
  return if next.generation != hydration_generation
  return if sync_phase != "refreshing"
  mutation_phase = "idle"
  sync_phase = "idle"
  hydration_retry_attempt = 0
  status = next.status
  block_height = next.height
  channels = next.channels
  messages = next.messages
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  active_channel_huddle_count = next.active_channel_huddle_count
  channel_members = next.channel_members
  pages = next.pages
  blocks = next.blocks
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  block_edit_draft = refreshed_block_draft(next.blocks, selected_block_id, block_edit_draft, block_autosave_status)
  page_delete_armed = false
  block_delete_armed = false
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  block_comments_generation = block_comments_generation + 1
  run refresh_block_comments(connected_rpc, block_comments_target, active_block_comment_thread, block_comments_generation) -> live_block_comments_refreshed _ | live_block_comments_failed _

on live_updated(next)
  status = next.status
  return if next.kind == "retrying"
  live_dirty = true
  return if loading || mutation_phase != "idle" || sync_phase == "refreshing"
  live_dirty = false
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "refreshing"
  block_comments_generation = block_comments_generation + 1
  run refresh_block_comments(connected_rpc, block_comments_target, active_block_comment_thread, block_comments_generation) -> live_block_comments_refreshed _ | live_block_comments_failed _

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
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on live_block_comments_failed(cause)
  return if cause.generation != block_comments_generation
  block_comment_threads_loading = false
  block_thread_comments_loading = false
  error = cause.message
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on refresh_failed(cause)
  return if cause.generation != hydration_generation
  return if sync_phase != "refreshing"
  status = "Sync delayed"
  error = "Live sync interrupted. Retrying…"
  hydration_retry_attempt = hydration_retry_attempt + 1
  run retry_refresh(connected_rpc, active_channel, active_page, hydration_generation, hydration_retry_attempt) -> workspace_refreshed _ | refresh_failed _

subscribe
  run live_events(connected_rpc) when connected -> live_updated _

on mutation_failed(cause)
  selected_message_seq = message_seq_after_failure(selected_message_seq, mutation_phase, cause.committed)
  selected_message_rev = message_seq_after_failure(selected_message_rev, mutation_phase, cause.committed)
  message_action = message_action_after_failure(message_action, mutation_phase, cause.committed)
  message_edit_draft = message_text_after_failure(message_edit_draft, mutation_phase, cause.committed)
  mutation_phase = mutation_failure_phase(cause.committed)
  channel_draft = restore_draft(channel_draft, pending_channel, cause.committed)
  failed_message_draft = remember_failed_draft(failed_message_draft, message_draft, pending_message, cause.committed)
  message_draft = restore_draft(message_draft, pending_message, cause.committed)
  page_draft = restore_draft(page_draft, pending_page, cause.committed)
  block_draft = restore_draft(block_draft, pending_block, cause.committed)
  reply_draft = restore_draft(reply_draft, pending_reply, cause.committed)
  messages = rollback_messages(messages, cause.committed)
  thread_messages = rollback_messages(thread_messages, cause.committed)
  blocks = rollback_blocks(blocks, cause.committed)
  pending_channel = ""
  pending_message = ""
  pending_page = ""
  pending_block = ""
  pending_reply = ""
  error = cause.message
  live_dirty = live_dirty || cause.committed
  return if !live_dirty
  block_autosave_generation = cancel_autosaves(connected_rpc, block_autosave_generation)
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on dismiss_error
  error = ""

on restore_failed_message
  return if empty(failed_message_draft) || !empty(message_draft) || mutation_phase != "idle"
  message_draft = failed_message_draft
  failed_message_draft = ""

on dismiss_failed_message
  failed_message_draft = ""

on failed(cause)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = false
  sync_phase = "idle"
  status = "Offline"
  error = cause.message
