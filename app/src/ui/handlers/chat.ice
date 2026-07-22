on search_chat_submit
  return if chat_searching || empty(trim(chat_search_draft))
  chat_searching = true
  error = ""
  run search_chat(connected_rpc, "", trim(chat_search_draft)) -> chat_search_loaded _ | search_failed _

on chat_search_loaded(next)
  chat_search_hits = next.hits
  chat_searching = false
  error = ""

on clear_chat_search
  chat_search_draft = ""
  chat_search_hits = []
  chat_searching = false

on open_chat_search_hit(channel_id)
  return if loading || mutation_phase != "idle"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
  chat_search_hits = []
  selected_message_seq = 0
  active_thread_seq = 0
  thread_messages = []
  error = ""
  run load_chat(connected_rpc, channel_id) -> chat_updated _ | failed _

on choose_channel(id)
  return if loading || mutation_phase != "idle"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
  chat_search_hits = []
  selected_message_seq = 0
  selected_message_rev = 0
  message_edit_draft = ""
  channel_settings_open = false
  channel_name_draft = ""
  member_key_draft = ""
  active_thread_seq = 0
  thread_messages = []
  reply_draft = ""
  pending_reply = ""
  error = ""
  run load_chat(connected_rpc, id) -> chat_updated _ | failed _

on create_channel_submit
  return if loading || mutation_phase != "idle" || empty(trim(channel_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "channel"
  pending_channel = trim(channel_draft)
  channel_draft = ""
  error = ""
  run create_channel(connected_rpc, password, pending_channel) -> chat_mutated _ | mutation_failed _

on toggle_channel_settings
  return if empty(active_channel)
  channel_settings_open = !channel_settings_open
  channel_name_draft = active_channel_name

on rename_channel_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || empty(trim(channel_name_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "channel-rename"
  error = ""
  run rename_channel(connected_rpc, password, active_channel, trim(channel_name_draft)) -> chat_mutated _ | mutation_failed _

on archive_channel_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "channel-archive"
  error = ""
  run archive_channel(connected_rpc, password, active_channel) -> chat_mutated _ | mutation_failed _

on unarchive_channel_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || !active_channel_archived
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "channel-unarchive"
  error = ""
  run unarchive_channel(connected_rpc, password, active_channel) -> chat_mutated _ | mutation_failed _

on add_channel_member_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || empty(trim(member_key_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "channel-member"
  error = ""
  run add_channel_member(connected_rpc, password, active_channel, trim(member_key_draft)) -> chat_mutated _ | mutation_failed _

on remove_channel_member_submit(key)
  return if loading || mutation_phase != "idle" || empty(active_channel)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "channel-member"
  error = ""
  run remove_channel_member(connected_rpc, password, active_channel, key) -> chat_mutated _ | mutation_failed _

on join_huddle_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "huddle"
  error = ""
  run join_huddle(connected_rpc, password, active_channel) -> chat_mutated _ | mutation_failed _

on leave_huddle_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_huddle_count <= 0
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "huddle"
  error = ""
  run leave_huddle(connected_rpc, password, active_channel) -> chat_mutated _ | mutation_failed _

on send_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived || empty(trim(message_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "message"
  pending_message = trim(message_draft)
  message_draft = ""
  messages = optimistic_message(messages, pending_message)
  error = ""
  run send_message(connected_rpc, password, active_channel, pending_message) -> chat_mutated _ | mutation_failed _

on chat_updated(next)
  channels = next.channels
  messages = next.messages
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  active_channel_huddle_count = next.active_channel_huddle_count
  channel_members = next.channel_members
  loading = false
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on chat_mutated(next)
  channels = next.channels
  messages = next.messages
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  active_channel_huddle_count = next.active_channel_huddle_count
  channel_members = next.channel_members
  channel_name_draft = next.active_channel_name
  member_key_draft = ""
  selected_message_seq = 0
  selected_message_rev = 0
  message_edit_draft = ""
  active_thread_seq = 0
  thread_messages = []
  reply_draft = ""
  pending_reply = ""
  pending_channel = ""
  pending_message = ""
  mutation_phase = "idle"
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on select_message(seq, body, rev)
  return if seq <= 0 || mutation_phase != "idle"
  selected_message_seq = seq
  selected_message_rev = rev
  message_edit_draft = body

on clear_message_selection
  selected_message_seq = 0
  selected_message_rev = 0
  message_edit_draft = ""

on open_thread
  return if thread_loading || mutation_phase != "idle" || empty(active_channel) || selected_message_seq <= 0
  thread_loading = true
  error = ""
  run load_thread(connected_rpc, active_channel, selected_message_seq) -> thread_loaded _ | thread_failed _

on thread_loaded(next)
  active_thread_seq = next.root_seq
  thread_messages = next.messages
  thread_loading = false
  reply_draft = ""
  pending_reply = ""
  error = ""

on thread_failed(cause)
  thread_loading = false
  error = cause.message

on close_thread
  active_thread_seq = 0
  thread_messages = []
  thread_loading = false
  reply_draft = ""
  pending_reply = ""

on edit_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived || selected_message_seq <= 0 || empty(trim(message_edit_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "message-edit"
  error = ""
  run edit_message(connected_rpc, password, active_channel, selected_message_seq, selected_message_rev, trim(message_edit_draft)) -> chat_mutated _ | mutation_failed _

on delete_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived || selected_message_seq <= 0
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "message-delete"
  error = ""
  run delete_message(connected_rpc, password, active_channel, selected_message_seq) -> chat_mutated _ | mutation_failed _

on add_reaction_submit(emoji)
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived || selected_message_seq <= 0
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "reaction"
  error = ""
  run add_reaction(connected_rpc, password, active_channel, selected_message_seq, emoji) -> chat_mutated _ | mutation_failed _

on send_reply_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived || active_thread_seq <= 0 || empty(trim(reply_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "reply"
  pending_reply = trim(reply_draft)
  reply_draft = ""
  thread_messages = optimistic_message(thread_messages, pending_reply)
  error = ""
  run send_reply(connected_rpc, password, active_channel, active_thread_seq, pending_reply) -> thread_mutated _ | mutation_failed _

on thread_mutated(next)
  active_thread_seq = next.root_seq
  thread_messages = next.messages
  pending_reply = ""
  mutation_phase = "idle"
  live_dirty = false
  error = ""
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _
