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
  palette_open = false
  shell_tab = "chat"
  chat_search_generation = chat_search_generation + 1
  chat_searching = false
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = true
  chat_search_hits = []
  selected_message_seq = 0
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
  reply_editor = editor("")
  pending_reply = ""
  error = ""
  run load_chat_hit(connected_rpc, channel_id, root_seq, target_seq) -> chat_hit_loaded _ | failed _

on choose_channel(id)
  return if loading || mutation_phase != "idle"
  active_dm_peer = ""
  // FREEZE THE DIVIDER HERE, while the previous room is still `active_channel`
  // — the optimistic assignment below makes current == next by the time
  // `chat_updated` runs its own freeze, which then correctly keeps this value.
  unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, id, unread_boundary)
  // The switch is visible NOW: the clicked room takes the header and the
  // sidebar highlight, and the previous room's messages leave the pane before
  // the round-trip — a click that repaints nothing reads as a dead app.
  active_channel = id
  active_channel_name = channel_display_name(channels, active_channel, active_channel_name)
  messages = []
  unread_marker_seq = 0
  chat_search_generation = chat_search_generation + 1
  chat_searching = false
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = true
  chat_search_hits = []
  selected_message_seq = 0
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
  reply_editor = editor("")
  pending_reply = ""
  error = ""
  run load_chat(connected_rpc, active_channel) -> chat_updated _ | failed _

// A DM is not a second message plane: it is the two-party members-only channel
// at `dm_channel_id(me, peer)`, resolved or created on the way in. Everything
// downstream of `chat_updated` is the ordinary channel path.
on choose_dm(peer_key)
  return if loading || mutation_phase != "idle" || empty(peer_key)
  active_dm_peer = peer_key
  // Same visible switch as `choose_channel`: the stale room leaves the pane
  // immediately; the DM header already derives from `dm_peers`.
  messages = []
  chat_search_generation = chat_search_generation + 1
  chat_searching = false
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = true
  chat_search_hits = []
  selected_message_seq = 0
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
  reply_editor = editor("")
  pending_reply = ""
  error = ""
  // Reads the peer back from state: `active_dm_peer = peer_key` above already
  // moved the payload, so passing `peer_key` here would be a use after move.
  run open_dm(connected_rpc, password, active_dm_peer) -> chat_updated _ | failed _

on create_channel_submit
  return if loading || mutation_phase != "idle" || empty(trim(channel_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "channel"
  pending_channel = trim(channel_draft)
  channel_draft = ""
  error = ""
  run create_channel(connected_rpc, password, pending_channel, channel_create_members_only) -> channel_created _ | mutation_failed _

on toggle_channel_create_members_only
  channel_create_members_only = !channel_create_members_only

on toggle_channel_create
  channel_create_open = !channel_create_open
  // No focus task: the artifact's create-channel input now lives inside the
  // ModalShell component, and a widget target cannot reach into a nested
  // component's slot fill — every working path in this app stops at the first
  // component boundary. Restore the focus when the language can address it.

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
  reply_editor = editor("")
  pending_reply = ""

on rename_channel_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || empty(trim(channel_name_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "channel-rename"
  error = ""
  run rename_channel(connected_rpc, password, active_channel, trim(channel_name_draft)) -> chat_acked _ | mutation_failed _

on archive_channel_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "channel-archive"
  error = ""
  run archive_channel(connected_rpc, password, active_channel) -> chat_acked _ | mutation_failed _

on unarchive_channel_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || !active_channel_archived
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "channel-unarchive"
  error = ""
  run unarchive_channel(connected_rpc, password, active_channel) -> chat_acked _ | mutation_failed _

on add_channel_member_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || empty(trim(member_key_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "channel-member"
  error = ""
  run add_channel_member(connected_rpc, password, active_channel, trim(member_key_draft)) -> chat_acked _ | mutation_failed _

on remove_channel_member_submit(key)
  return if loading || mutation_phase != "idle" || empty(active_channel)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "channel-member"
  error = ""
  run remove_channel_member(connected_rpc, password, active_channel, key) -> chat_acked _ | mutation_failed _

// JOINING IS A SIGNED CHAIN WRITE, SO IT NEEDS AN INVERSE THE UI CAN REACH.
// `huddle_joined` is the discriminant that splits the header's "Huddle" start
// control from the LIVE pill carrying ✕ Leave, and it had no writer at all —
// every reply that carries a channel's state now answers it from that
// channel's roster, which is the chain's answer and never a local flag.
//
// It answers for the channel ON SCREEN only: `ChatData` carries the roster of
// the active channel, and nothing on the wire says whether she is in a huddle
// in some OTHER channel. So the docked titlebar pill and the "live elsewhere"
// affordance stay dark rather than guess (see the report).
on join_huddle_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "huddle"
  error = ""
  run join_huddle(connected_rpc, password, active_channel) -> chat_acked _ | mutation_failed _

// Leaving is `leave_huddle_here` in handlers/huddle.ice, which leaves the
// HUDDLE'S channel rather than the one on screen — the same button serves the
// channel-header ✕ and the popped panel, so a second leave that targets
// `active_channel` would be a way to leave the wrong huddle.

// One handler carries the whole composer: every rich-editor event lands here,
// the apply is a no-op on Submit, and the single guard below is the send/edit
// fork. The Send button emits a synthetic Submit through the same route, so
// there is exactly ONE send path.
// A toolbar mark is an edit, not a send: wrap the selection (or park the
// cursor inside a fresh marker pair) and hand the editor back. The same
// disabled gate as the editor guards the buttons in the view.
on composer_mark(kind)
  return if loading || !connected || empty(active_channel)
  message_editor = composer_toggle_mark(message_editor, kind)

on chat_composer_event(event)
  message_editor = apply_composer_event(message_editor, event)
  return if !composer_submits(event)
  return if loading || empty(active_channel) || active_channel_archived || empty(trim(editor_text(message_editor)))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  pending_message = trim(editor_text(message_editor))
  pending_message_id = fresh_operation_id("message")
  message_draft = ""
  message_editor = editor("")
  messages = optimistic_message(messages, pending_message, pending_message_id)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  error = ""
  run send_message(connected_rpc, password, active_channel, pending_message_id, pending_message, channel_members) -> message_sent _ | message_send_failed _

on message_sent(next)
  return if active_channel != next.channel_id
  error = ""

on message_send_failed(cause)
  return if active_channel != cause.scope_id
  messages = rollback_pending_message(messages, cause.operation_id, cause.committed)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  failed_message_draft = remember_failed_draft(failed_message_draft, trim(editor_text(message_editor)), cause.body, cause.committed)
  message_draft = restore_draft(trim(editor_text(message_editor)), cause.body, cause.committed)
  message_editor = editor(message_draft)
  error = cause.message
  return if !cause.committed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run live_resync_load(connected_rpc, active_channel, active_page, "chat", false, hydration_generation, 0) -> live_resynced _ | live_resync_failed _

on chat_updated(next)
  history_view = false
  channels = next.channels
  messages = merge_pending_messages(next.messages, messages, active_channel, next.active_channel, "")
  unread_boundary = frozen_unread_boundary(channel_reads, next.channels, active_channel, next.active_channel, unread_boundary)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  channel_reads = mark_channel_read(channel_reads, next.active_channel, channel_head_seq(next.channels, next.active_channel))
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  active_channel_huddle_count = next.active_channel_huddle_count
  // Am I in it — see `join_huddle_submit` above. Stamp first: it reads the
  // PREVIOUS `huddle_joined`, so a refresh that finds her still in keeps the
  // clock and one that finds her out re-takes it for the next join.
  huddle_joined_at = keep_i64(huddle_joined, huddle_joined_at, huddle_now)
  huddle_joined = huddle_self(next.huddle_roster)
  huddle_roster = keep_roster(huddle_joined, next.huddle_roster)
  huddle_channel = keep_str(huddle_joined, active_channel, "")
  huddle_channel_name = keep_str(huddle_joined, active_channel_name, "")
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
  // The huddle ended under this fold (or the roster she is on is another
  // channel's): the window mirrors the old popped-card gate, which also
  // vanished the moment `huddle_joined` dropped. A no-op while still joined.
  task window close target=window_target_unless(huddle_joined, huddle_win)

on chat_hit_loaded(next)
  history_view = true
  channels = next.channels
  messages = merge_pending_messages(next.messages, messages, active_channel, next.active_channel, "")
  unread_boundary = frozen_unread_boundary(channel_reads, next.channels, active_channel, next.active_channel, unread_boundary)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  channel_reads = mark_channel_read(channel_reads, next.active_channel, channel_head_seq(next.channels, next.active_channel))
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  active_channel_huddle_count = next.active_channel_huddle_count
  // Am I in it — see `join_huddle_submit` above. Stamp first: it reads the
  // PREVIOUS `huddle_joined`, so a refresh that finds her still in keeps the
  // clock and one that finds her out re-takes it for the next join.
  huddle_joined_at = keep_i64(huddle_joined, huddle_joined_at, huddle_now)
  huddle_joined = huddle_self(next.huddle_roster)
  huddle_roster = keep_roster(huddle_joined, next.huddle_roster)
  huddle_channel = keep_str(huddle_joined, active_channel, "")
  huddle_channel_name = keep_str(huddle_joined, active_channel_name, "")
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
  // Same close-if-ended mirror as `chat_updated` above.
  task window close target=window_target_unless(huddle_joined, huddle_win)

on channel_created(next)
  pending_channel = ""
  channel_create_open = false
  channel_create_members_only = false
  mutation_phase = "idle"
  channels = next.channels
  messages = merge_pending_messages(next.messages, messages, active_channel, next.active_channel, "")
  unread_boundary = frozen_unread_boundary(channel_reads, next.channels, active_channel, next.active_channel, unread_boundary)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  channel_reads = mark_channel_read(channel_reads, next.active_channel, channel_head_seq(next.channels, next.active_channel))
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  active_channel_huddle_count = next.active_channel_huddle_count
  // Am I in it — see `join_huddle_submit` above. Stamp first: it reads the
  // PREVIOUS `huddle_joined`, so a refresh that finds her still in keeps the
  // clock and one that finds her out re-takes it for the next join.
  huddle_joined_at = keep_i64(huddle_joined, huddle_joined_at, huddle_now)
  huddle_joined = huddle_self(next.huddle_roster)
  huddle_roster = keep_roster(huddle_joined, next.huddle_roster)
  huddle_channel = keep_str(huddle_joined, active_channel, "")
  huddle_channel_name = keep_str(huddle_joined, active_channel_name, "")
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
  error = ""
  // Same close-if-ended mirror as `chat_updated` above.
  task window close target=window_target_unless(huddle_joined, huddle_win)

on chat_acked(_result)
  selected_message_seq = message_seq_after_failure(selected_message_seq, mutation_phase, true)
  selected_message_rev = message_seq_after_failure(selected_message_rev, mutation_phase, true)
  message_action = message_action_after_failure(message_action, mutation_phase, true)
  message_edit_draft = message_text_after_failure(message_edit_draft, mutation_phase, true)
  thread_selected_seq = message_seq_after_failure(thread_selected_seq, mutation_phase, true)
  thread_selected_rev = message_seq_after_failure(thread_selected_rev, mutation_phase, true)
  thread_message_action = message_action_after_failure(thread_message_action, mutation_phase, true)
  thread_edit_draft = message_text_after_failure(thread_edit_draft, mutation_phase, true)
  pending_channel = ""
  channel_create_open = false
  mutation_phase = "idle"
  error = ""
// PRESSES, NOT MOVES. The pointer y is read exactly once — when an action
// menu opens, to anchor its float — so it is captured per left press by
// `press-at`, which reports through a captured press (the ⋯ button's own
// press-down lands here first, then its click opens the menu one event
// later). The old `move=` stream republished on every cursor pixel and
// rebuilt the whole view each time; hovering a busy channel was a rebuild
// storm.
on chat_pointer_pressed(_x, y)
  chat_pointer_y = y

on chat_resized(_width, height)
  chat_height = height

on thread_pointer_pressed(_x, y)
  thread_pointer_y = y

on thread_resized(_width, height)
  thread_height = height

on open_thread_message_actions(seq, body, rev)
  return if seq <= 0
  thread_menu_y = block_action_menu_y(thread_pointer_y, thread_height)
  thread_selected_seq = seq
  thread_selected_rev = rev
  thread_message_action = "more"
  thread_edit_draft = body
  sequential
    task widget focus #workspace-tabs/content/chat/thread-action-focus
    task widget focus-next

on open_thread_message_reactions(seq, body, rev)
  return if seq <= 0
  thread_menu_y = block_action_menu_y(thread_pointer_y, thread_height)
  thread_selected_seq = seq
  thread_selected_rev = rev
  thread_message_action = "reactions"
  thread_edit_draft = body
  sequential
    task widget focus #workspace-tabs/content/chat/thread-reaction-focus
    task widget focus-next

on arm_thread_message_delete(seq, body, rev)
  return if seq <= 0
  thread_selected_seq = seq
  thread_selected_rev = rev
  thread_message_action = "delete"
  thread_edit_draft = body
  sequential
    task widget focus #workspace-tabs/content/chat/thread-delete-focus
    task widget focus-next

on begin_thread_message_edit(seq, body, rev)
  return if seq <= 0
  thread_selected_seq = seq
  thread_selected_rev = rev
  thread_message_action = "editing"
  thread_edit_draft = body
  task widget focus #workspace-tabs/content/chat/thread-edit

on clear_thread_message_selection
  thread_selected_seq = 0
  thread_selected_rev = 0
  thread_message_action = "toolbar"
  thread_edit_draft = ""

on edit_thread_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || thread_selected_seq <= 0 || empty(trim(thread_edit_draft))
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "message-edit"
  error = ""
  run edit_message(connected_rpc, password, active_channel, thread_selected_seq, thread_selected_rev, trim(thread_edit_draft), channel_members) -> chat_acked _ | mutation_failed _

on delete_thread_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || thread_selected_seq <= 0 || thread_message_action != "delete"
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "message-delete"
  error = ""
  run delete_message(connected_rpc, password, active_channel, thread_selected_seq) -> chat_acked _ | mutation_failed _

on open_message_actions(seq, body, rev)
  return if seq <= 0
  message_menu_y = block_action_menu_y(chat_pointer_y, chat_height)
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = "more"
  message_edit_draft = body
  sequential
    task widget focus #workspace-tabs/content/chat/message-action-focus
    task widget focus-next

on open_message_reactions(seq, body, rev)
  return if seq <= 0
  message_menu_y = block_action_menu_y(chat_pointer_y, chat_height)
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = "reactions"
  message_edit_draft = body
  sequential
    task widget focus #workspace-tabs/content/chat/message-reaction-focus
    task widget focus-next

on arm_message_delete(seq, body, rev)
  return if seq <= 0
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = "delete"
  message_edit_draft = body
  sequential
    task widget focus #workspace-tabs/content/chat/message-delete-focus
    task widget focus-next

on begin_message_edit(seq, body, rev)
  return if seq <= 0
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = "editing"
  message_edit_draft = body
  task widget focus #workspace-tabs/content/chat/message-edit

// THE INSPECTOR IS THE FINALITY MARK'S TARGET. The shield in the hover bar and
// the settled chip on my own bubble both land here, and both name the same
// right rail — so opening one closes the other.


on open_thread_for(seq)
  return if seq <= 0 || empty(active_channel)
  channel_settings_open = false
  selected_message_seq = 0
  selected_message_rev = 0
  message_action = "toolbar"
  message_edit_draft = ""
  thread_selected_seq = 0
  thread_selected_rev = 0
  thread_message_action = "toolbar"
  thread_edit_draft = ""
  thread_generation = thread_generation + 1
  live_thread_generation = live_thread_generation + 1
  thread_loading = true
  active_thread_seq = seq
  thread_target_seq = 0
  thread_messages = []
  thread_next_reply_offset = 0
  thread_has_more = false
  reply_draft = ""
  reply_editor = editor("")
  pending_reply = ""
  error = ""
  run load_thread(connected_rpc, active_channel, seq, 0, 0, thread_generation) -> thread_loaded _ | thread_failed _

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

on thread_page_failed(cause)
  return if cause.generation != thread_generation || !thread_loading
  thread_loading = false
  error = cause.message

on load_more_history
  return if history_loading || loading || mutation_phase != "idle" || empty(active_channel) || empty(messages) || !history_has_older(messages)
  history_generation = history_generation + 1
  history_loading = true
  error = ""
  run load_older_messages(connected_rpc, active_channel, oldest_message_seq(messages), history_generation) -> history_loaded _ | history_failed _

on history_loaded(next)
  return if next.generation != history_generation || !history_loading
  messages = prepend_history(messages, next.messages)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
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

on close_thread
  thread_generation = thread_generation + 1
  live_thread_generation = live_thread_generation + 1
  active_thread_seq = 0
  thread_target_seq = 0
  thread_messages = []
  thread_next_reply_offset = 0
  thread_has_more = false
  thread_loading = false
  thread_selected_seq = 0
  thread_selected_rev = 0
  thread_message_action = "toolbar"
  thread_edit_draft = ""
  reply_draft = ""
  reply_editor = editor("")
  pending_reply = ""

on edit_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || selected_message_seq <= 0 || empty(trim(message_edit_draft))
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "message-edit"
  error = ""
  run edit_message(connected_rpc, password, active_channel, selected_message_seq, selected_message_rev, trim(message_edit_draft), channel_members) -> chat_acked _ | mutation_failed _

on delete_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || selected_message_seq <= 0 || message_action != "delete"
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "message-delete"
  error = ""
  run delete_message(connected_rpc, password, active_channel, selected_message_seq) -> chat_acked _ | mutation_failed _

on add_reaction_submit(emoji)
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived || selected_message_seq <= 0
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "reaction"
  error = ""
  messages = reaction_applied(messages, selected_message_seq, emoji, true)
  thread_messages = reaction_applied(thread_messages, selected_message_seq, emoji, true)
  run add_reaction(connected_rpc, password, active_channel, selected_message_seq, emoji) -> chat_acked _ | reaction_failed _

// One-tap reactions do NOT select the row: the tap is its own complete act,
// and parking the selection tint on the message until the next Esc read as
// a leftover highlight (QA). The picker path still selects, because its
// overlay is anchored to the selection.
on add_reaction_at(seq, emoji)
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived || seq <= 0
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "reaction"
  error = ""
  messages = reaction_applied(messages, seq, emoji, true)
  thread_messages = reaction_applied(thread_messages, seq, emoji, true)
  run add_reaction(connected_rpc, password, active_channel, seq, emoji) -> chat_acked _ | reaction_failed _

on remove_reaction_at(seq, emoji)
  return if loading || mutation_phase != "idle" || active_channel_archived || seq <= 0
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "reaction"
  error = ""
  messages = reaction_applied(messages, seq, emoji, false)
  thread_messages = reaction_applied(thread_messages, seq, emoji, false)
  run remove_reaction(connected_rpc, password, active_channel, seq, emoji) -> chat_acked _ | reaction_failed _

// The settle-✓'s two-beat teardown. Beat one reads the animation still
// aimed at visible, keeps its anchor and flips the fade; beat two reads it
// aimed at hidden and drops the anchor, unmounting the ✓ (the 400ms fade
// always finishes inside one 1200ms beat).
on send_flash_tick
  send_flash_id = keep_str(animation.value(send_flash), send_flash_id, "")
  thread_send_flash_id = keep_str(animation.value(send_flash), thread_send_flash_id, "")
  send_flash = false

// A reaction failure leaves the optimistic fold as a LIE on screen; there is
// no rollback token (the fold is not invertible under concurrent deltas), so
// the canonical refetch IS the revert — committed or not.
on reaction_failed(cause)
  mutation_phase = mutation_failure_phase(cause.committed)
  error = cause.message
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run live_resync_load(connected_rpc, active_channel, active_page, "chat", false, hydration_generation, 0) -> live_resynced _ | live_resync_failed _

on reply_composer_event(event)
  reply_editor = apply_composer_event(reply_editor, event)
  return if !composer_submits(event)
  return if loading || thread_loading || empty(active_channel) || active_channel_archived || active_thread_seq <= 0 || empty(trim(editor_text(reply_editor)))
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  pending_reply = trim(editor_text(reply_editor))
  pending_reply_id = fresh_operation_id("reply")
  reply_draft = ""
  reply_editor = editor("")
  thread_messages = optimistic_message(thread_messages, pending_reply, pending_reply_id)
  error = ""
  run send_reply(connected_rpc, password, active_channel, active_thread_seq, pending_reply_id, pending_reply, channel_members) -> thread_reply_sent _ | thread_reply_send_failed _

on thread_reply_send_failed(cause)
  return if active_channel != cause.scope_id
  return if !contains_pending_message(thread_messages, cause.operation_id)
  thread_messages = rollback_pending_message(thread_messages, cause.operation_id, cause.committed)
  failed_reply_draft = remember_failed_draft(failed_reply_draft, trim(editor_text(reply_editor)), cause.body, cause.committed)
  reply_draft = restore_draft(trim(editor_text(reply_editor)), cause.body, cause.committed)
  reply_editor = editor(reply_draft)
  thread_next_reply_offset = thread_offset_after_reply(thread_next_reply_offset, thread_has_more, cause.committed)
  error = cause.message
  return if !cause.committed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run live_resync_load(connected_rpc, active_channel, active_page, "chat", false, hydration_generation, 0) -> live_resynced _ | live_resync_failed _

on thread_reply_sent(next)
  return if active_channel != next.channel_id
  error = ""
