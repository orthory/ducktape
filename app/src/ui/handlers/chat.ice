on search_chat_submit
  return if chat_searching || empty(trim(chat_search_draft))
  chat_search_generation = chat_search_generation + 1
  chat_searching = true
  chat_search_hits = []
  error = ""
  run search_chat(connected_rpc, "", trim(chat_search_draft), chat_search_generation) -> chat_search_loaded _ | chat_search_failed _

on chat_search_loaded(next)
  return if next.generation != chat_search_generation
  chat_search_hits = next.hits
  chat_searching = false
  error = ""

on chat_search_failed(cause)
  return if cause.generation != chat_search_generation
  chat_searching = false
  error = cause.message

on clear_chat_search
  chat_search_generation = chat_search_generation + 1
  chat_search_draft = ""
  chat_search_hits = []
  chat_searching = false

on open_chat_search_hit(channel_id, root_seq, target_seq)
  return if loading || mutation_phase != "idle"
  chat_search_generation = chat_search_generation + 1
  chat_searching = false
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
  chat_search_hits = []
  selected_message_seq = 0
  hovered_message_seq = 0
  selected_message_rev = 0
  message_action = "toolbar"
  message_edit_draft = ""
  channel_settings_open = false
  channel_name_draft = ""
  member_key_draft = ""
  active_thread_seq = 0
  thread_target_seq = 0
  thread_messages = []
  thread_next_reply_offset = 0
  thread_has_more = false
  thread_generation = thread_generation + 1
  live_thread_generation = live_thread_generation + 1
  thread_loading = false
  reply_draft = ""
  pending_reply = ""
  error = ""
  run load_chat_hit(connected_rpc, channel_id, root_seq, target_seq) -> chat_updated _ | failed _

on choose_channel(id)
  return if loading || mutation_phase != "idle"
  chat_search_generation = chat_search_generation + 1
  chat_searching = false
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  loading = true
  chat_search_hits = []
  selected_message_seq = 0
  hovered_message_seq = 0
  selected_message_rev = 0
  message_action = "toolbar"
  message_edit_draft = ""
  channel_settings_open = false
  channel_name_draft = ""
  member_key_draft = ""
  active_thread_seq = 0
  thread_target_seq = 0
  thread_messages = []
  thread_next_reply_offset = 0
  thread_has_more = false
  thread_generation = thread_generation + 1
  live_thread_generation = live_thread_generation + 1
  thread_loading = false
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

on toggle_channel_create
  channel_create_open = !channel_create_open
  return if !channel_create_open
  task widget focus #workspace-tabs/new-channel

on toggle_channel_settings
  return if empty(active_channel)
  channel_settings_open = !channel_settings_open
  channel_name_draft = active_channel_name
  return if !channel_settings_open
  thread_generation = thread_generation + 1
  live_thread_generation = live_thread_generation + 1
  active_thread_seq = 0
  thread_target_seq = 0
  thread_messages = []
  thread_next_reply_offset = 0
  thread_has_more = false
  thread_loading = false
  reply_draft = ""
  pending_reply = ""

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
  return if loading || empty(active_channel) || active_channel_archived || empty(trim(message_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  pending_message = trim(message_draft)
  pending_message_id = fresh_operation_id("message")
  message_draft = ""
  messages = optimistic_message(messages, pending_message, pending_message_id)
  error = ""
  run send_message(connected_rpc, password, active_channel, pending_message_id, pending_message) -> message_sent _ | message_send_failed _

on message_sent(next)
  channels = next.data.channels
  return if active_channel != next.channel_id || next.data.active_channel != next.channel_id
  messages = merge_message_send_result(next.data.messages, messages, active_channel, next.data.active_channel, next.operation_id)
  active_channel_name = next.data.active_channel_name
  active_channel_archived = next.data.active_channel_archived
  active_channel_members_only = next.data.active_channel_members_only
  active_channel_huddle_count = next.data.active_channel_huddle_count
  channel_members = next.data.channel_members
  error = ""

on message_send_failed(cause)
  return if active_channel != cause.scope_id
  messages = rollback_pending_message(messages, cause.operation_id, cause.committed)
  failed_message_draft = remember_failed_draft(failed_message_draft, message_draft, cause.body, cause.committed)
  message_draft = restore_draft(message_draft, cause.body, cause.committed)
  error = cause.message
  live_dirty = live_dirty || cause.committed
  return if !live_dirty || loading || sync_phase == "refreshing"
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on chat_updated(next)
  channels = next.channels
  messages = merge_pending_messages(next.messages, messages, active_channel, next.active_channel, "")
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  active_channel_huddle_count = next.active_channel_huddle_count
  channel_members = next.channel_members
  selected_message_seq = next.selected_message_seq
  selected_message_rev = next.selected_message_rev
  message_action = "toolbar"
  message_edit_draft = next.selected_message_body
  active_thread_seq = next.active_thread_seq
  thread_target_seq = next.thread_target_seq
  thread_messages = next.thread_messages
  thread_next_reply_offset = next.thread_next_reply_offset
  thread_has_more = next.thread_has_more
  thread_generation = thread_generation + 1
  live_thread_generation = live_thread_generation + 1
  thread_loading = false
  loading = false
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on chat_mutated(next)
  channels = next.channels
  failed_message_draft = remember_failed_draft(failed_message_draft, "channel", message_draft, active_channel == next.active_channel)
  selected_message_seq = refreshed_required_message_seq(next.messages, active_channel, next.active_channel, selected_message_seq)
  failed_message_draft = remember_failed_draft(failed_message_draft, message_action, message_edit_draft, selected_message_seq > 0 || message_action != "editing" || mutation_phase == "message-edit" || mutation_phase == "message-delete")
  selected_message_seq = message_seq_after_failure(selected_message_seq, mutation_phase, true)
  selected_message_rev = message_seq_after_failure(selected_message_rev, mutation_phase, true)
  selected_message_rev = message_seq_after_failure(selected_message_rev, "message-edit", selected_message_seq <= 0)
  message_action = message_action_after_failure(message_action, mutation_phase, true)
  message_action = message_action_after_failure(message_action, "message-edit", selected_message_seq <= 0)
  message_edit_draft = message_text_after_failure(message_edit_draft, mutation_phase, true)
  message_edit_draft = message_text_after_failure(message_edit_draft, "message-edit", selected_message_seq <= 0)
  hovered_message_seq = refreshed_required_message_seq(next.messages, active_channel, next.active_channel, hovered_message_seq)
  channel_settings_open = channel_settings_open && active_channel == next.active_channel
  channel_name_draft = retain_for_endpoint(channel_name_draft, active_channel, next.active_channel)
  member_key_draft = retain_for_endpoint(member_key_draft, active_channel, next.active_channel)
  thread_generation = thread_generation_after_refresh(thread_generation, active_channel, next.active_channel, active_thread_seq, refreshed_known_message_seq(next.messages, active_channel, next.active_channel, active_thread_seq))
  thread_loading = thread_loading_after_refresh(thread_loading, active_channel, next.active_channel, active_thread_seq, refreshed_known_message_seq(next.messages, active_channel, next.active_channel, active_thread_seq))
  failed_reply_draft = retain_for_endpoint(failed_reply_draft, active_channel, next.active_channel)
  active_thread_seq = refreshed_known_message_seq(next.messages, active_channel, next.active_channel, active_thread_seq)
  failed_reply_draft = remember_failed_draft(failed_reply_draft, "thread", reply_draft, active_thread_seq > 0)
  thread_target_seq = refreshed_channel_value(active_channel, next.active_channel, thread_target_seq)
  thread_next_reply_offset = refreshed_channel_value(active_channel, next.active_channel, thread_next_reply_offset)
  thread_target_seq = message_seq_after_failure(thread_target_seq, "message-edit", active_thread_seq <= 0)
  thread_next_reply_offset = message_seq_after_failure(thread_next_reply_offset, "message-edit", active_thread_seq <= 0)
  thread_messages = retain_thread_messages(thread_messages, active_thread_seq)
  thread_has_more = thread_has_more && active_channel == next.active_channel && active_thread_seq > 0
  reply_draft = retain_for_endpoint(reply_draft, active_channel, next.active_channel)
  pending_reply = retain_for_endpoint(pending_reply, active_channel, next.active_channel)
  reply_draft = message_text_after_failure(reply_draft, "message-edit", active_thread_seq <= 0)
  pending_reply = message_text_after_failure(pending_reply, "message-edit", active_thread_seq <= 0)
  message_draft = retain_for_endpoint(message_draft, active_channel, next.active_channel)
  messages = merge_pending_messages(next.messages, messages, active_channel, next.active_channel, "")
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  active_channel_huddle_count = next.active_channel_huddle_count
  channel_members = next.channel_members
  live_thread_generation = live_thread_generation + 1
  pending_channel = ""
  channel_create_open = false
  mutation_phase = "idle"
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on message_entered(seq)
  hovered_message_seq = seq

on message_exited(seq)
  return if hovered_message_seq != seq
  hovered_message_seq = 0

on chat_pointer_moved(_, y)
  chat_pointer_y = y

on chat_resized(_, height)
  chat_height = height

on open_message_actions(seq, body, rev)
  return if seq <= 0
  message_menu_y = block_action_menu_y(chat_pointer_y, chat_height)
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = "more"
  message_edit_draft = body
  sequential
    task widget focus #workspace-tabs/message-action-focus
    task widget focus-next

on open_message_actions_accessibly(seq, body, rev)
  return if seq <= 0
  message_menu_y = 0.0
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = "more"
  message_edit_draft = body
  sequential
    task widget focus #workspace-tabs/message-action-focus
    task widget focus-next

on open_message_reactions(seq, body, rev)
  return if seq <= 0
  message_menu_y = block_action_menu_y(chat_pointer_y, chat_height)
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = "reactions"
  message_edit_draft = body
  sequential
    task widget focus #workspace-tabs/message-reaction-focus
    task widget focus-next

on arm_message_delete(seq, body, rev)
  return if seq <= 0
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = "delete"
  message_edit_draft = body
  sequential
    task widget focus #workspace-tabs/message-delete-focus
    task widget focus-next

on begin_message_edit(seq, body, rev)
  return if seq <= 0
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = "editing"
  message_edit_draft = body
  task widget focus #workspace-tabs/message-edit

on open_thread_for(seq)
  return if seq <= 0 || empty(active_channel)
  channel_settings_open = false
  selected_message_seq = 0
  selected_message_rev = 0
  message_action = "toolbar"
  message_edit_draft = ""
  thread_generation = thread_generation + 1
  live_thread_generation = live_thread_generation + 1
  thread_loading = true
  active_thread_seq = seq
  thread_target_seq = 0
  thread_messages = []
  thread_next_reply_offset = 0
  thread_has_more = false
  reply_draft = ""
  pending_reply = ""
  error = ""
  run load_thread(connected_rpc, active_channel, seq, 0, 0, thread_generation) -> thread_loaded _ | thread_failed _

on cancel_message_action
  return if selected_message_seq <= 0
  message_action = "toolbar"

on clear_message_selection
  selected_message_seq = 0
  selected_message_rev = 0
  message_action = "toolbar"
  message_edit_draft = ""

on thread_loaded(next)
  return if next.generation != thread_generation || !thread_loading
  active_thread_seq = next.root_seq
  thread_target_seq = next.target_seq
  thread_messages = next.messages
  thread_next_reply_offset = next.next_reply_offset
  thread_has_more = next.has_more
  thread_loading = false
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on load_more_thread
  return if thread_loading || mutation_phase != "idle" || active_thread_seq <= 0 || thread_next_reply_offset < 0 || !thread_has_more
  thread_generation = thread_generation + 1
  live_thread_generation = live_thread_generation + 1
  thread_loading = true
  error = ""
  run load_thread_page(connected_rpc, active_channel, active_thread_seq, thread_next_reply_offset, thread_generation) -> thread_page_loaded _ | thread_page_failed _

on thread_page_loaded(next)
  return if next.generation != thread_generation || !thread_loading
  thread_messages = append_thread_page(thread_messages, next.messages)
  thread_next_reply_offset = next.next_reply_offset
  thread_has_more = next.has_more
  thread_loading = false
  error = ""
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on thread_page_failed(cause)
  return if cause.generation != thread_generation || !thread_loading
  thread_loading = false
  error = cause.message
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on load_more_history
  return if history_loading || loading || mutation_phase != "idle" || empty(active_channel) || empty(messages) || !history_has_older(messages)
  history_generation = history_generation + 1
  history_loading = true
  error = ""
  run load_older_messages(connected_rpc, active_channel, oldest_message_seq(messages), history_generation) -> history_loaded _ | history_failed _

on history_loaded(next)
  return if next.generation != history_generation || !history_loading
  messages = prepend_history(messages, next.messages)
  history_loading = false
  error = ""

on history_failed(cause)
  return if cause.generation != history_generation
  history_loading = false
  error = cause.message

on thread_failed(cause)
  return if cause.generation != thread_generation || !thread_loading
  thread_loading = false
  error = cause.message
  return if !live_dirty
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on close_thread
  thread_generation = thread_generation + 1
  live_thread_generation = live_thread_generation + 1
  active_thread_seq = 0
  thread_target_seq = 0
  thread_messages = []
  thread_next_reply_offset = 0
  thread_has_more = false
  thread_loading = false
  reply_draft = ""
  pending_reply = ""

on edit_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || selected_message_seq <= 0 || empty(trim(message_edit_draft))
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "message-edit"
  error = ""
  run edit_message(connected_rpc, password, active_channel, selected_message_seq, selected_message_rev, trim(message_edit_draft)) -> chat_mutated _ | mutation_failed _

on delete_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || selected_message_seq <= 0 || message_action != "delete"
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "message-delete"
  error = ""
  run delete_message(connected_rpc, password, active_channel, selected_message_seq) -> chat_mutated _ | mutation_failed _

on add_reaction_submit(emoji)
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived || selected_message_seq <= 0
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "reaction"
  error = ""
  run add_reaction(connected_rpc, password, active_channel, selected_message_seq, emoji) -> chat_mutated _ | mutation_failed _

on remove_reaction_submit(emoji)
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived || selected_message_seq <= 0
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  mutation_phase = "reaction"
  error = ""
  run remove_reaction(connected_rpc, password, active_channel, selected_message_seq, emoji) -> chat_mutated _ | mutation_failed _

on send_reply_submit
  return if loading || thread_loading || empty(active_channel) || active_channel_archived || active_thread_seq <= 0 || empty(trim(reply_draft))
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  sync_phase = "idle"
  pending_reply = trim(reply_draft)
  pending_reply_id = fresh_operation_id("reply")
  reply_draft = ""
  thread_messages = optimistic_message(thread_messages, pending_reply, pending_reply_id)
  error = ""
  run send_reply(connected_rpc, password, active_channel, active_thread_seq, pending_reply_id, pending_reply) -> thread_reply_sent _ | thread_reply_send_failed _

on thread_reply_send_failed(cause)
  return if active_channel != cause.scope_id
  return if !contains_pending_message(thread_messages, cause.operation_id)
  thread_messages = rollback_pending_message(thread_messages, cause.operation_id, cause.committed)
  failed_reply_draft = remember_failed_draft(failed_reply_draft, reply_draft, cause.body, cause.committed)
  reply_draft = restore_draft(reply_draft, cause.body, cause.committed)
  thread_next_reply_offset = thread_offset_after_reply(thread_next_reply_offset, thread_has_more, cause.committed)
  error = cause.message
  live_dirty = live_dirty || cause.committed
  return if !live_dirty || loading || sync_phase == "refreshing"
  live_dirty = false
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _

on thread_reply_sent(next)
  return if !contains_pending_message(thread_messages, next.id)
  thread_target_seq = 0
  thread_messages = merge_thread_reply(thread_messages, next)
  thread_next_reply_offset = thread_offset_after_reply(thread_next_reply_offset, thread_has_more, true)
  error = ""
  hydration_generation = hydration_generation + 1
  sync_phase = "refreshing"
  run refresh(connected_rpc, active_channel, active_page, hydration_generation) -> workspace_refreshed _ | refresh_failed _
