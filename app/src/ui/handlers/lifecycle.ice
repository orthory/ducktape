on mount
  loading = true
  run connect(rpc) -> workspace_connected _ | failed _

on reconnect
  return if loading || mutation_phase != "idle"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
  connected = false
  error = ""
  status = "Connecting…"
  run connect(trim(rpc)) -> workspace_connected _ | failed _

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
  active_page_parent = next.active_page_parent
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on live_updated(next)
  status = next.status
  return if next.kind == "retrying"
  live_dirty = true
  return if loading || mutation_phase != "idle" || sync_phase == "refreshing"
  live_dirty = false
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on refresh_failed(cause)
  return if cause.generation != hydration_generation
  return if sync_phase != "refreshing"
  status = "Sync delayed"
  error = cause.message
  hydration_retry_attempt = hydration_retry_attempt + 1
  run retry_refresh(connected_rpc, active_channel, active_page, hydration_generation, hydration_retry_attempt) -> workspace_refreshed _ | refresh_failed _

subscribe
  run live_events(connected_rpc) when connected -> live_updated _

on search_failed(cause)
  chat_searching = false
  page_searching = false
  error = cause.message

on mutation_failed(cause)
  mutation_phase = "idle"
  channel_draft = restore_draft(channel_draft, pending_channel)
  message_draft = restore_draft(message_draft, pending_message)
  page_draft = restore_draft(page_draft, pending_page)
  block_draft = restore_draft(block_draft, pending_block)
  reply_draft = restore_draft(reply_draft, pending_reply)
  messages = rollback_messages(messages)
  thread_messages = rollback_messages(thread_messages)
  blocks = rollback_blocks(blocks)
  pending_channel = ""
  pending_message = ""
  pending_page = ""
  pending_block = ""
  pending_reply = ""
  error = cause.message
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on dismiss_error
  error = ""

on failed(cause)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = false
  sync_phase = "idle"
  status = "Offline"
  error = cause.message
