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
  // Same abandoned request, same dead button — see `choose_channel`. This route
  // lands in a DIFFERENT channel via `chat_hit_loaded`, so the page still in
  // flight belongs to the room she jumped out of.
  history_loading = false
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
  // Both composers are rebuilt under the caret and the tab moved besides.
  composer_focus = "none"
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
  has_older_history = false
  // THE FLAG BELONGS TO THE REQUEST, AND THE REQUEST BELONGS TO THE ROOM YOU
  // LEFT. `load_more_history` returns early on it and nothing else here lowers
  // it, so "Load older" is dead in the room you land in until the abandoned
  // page lands — forever if it hangs. `history_loaded` drops that page on its
  // channel check anyway.
  history_loading = false
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
  // A new room's composer is a new box: the caret does not come with it.
  composer_focus = "none"
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
  has_older_history = false
  // Same abandoned request, same dead button — see `choose_channel`.
  history_loading = false
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
  // Same new box as `choose_channel`.
  composer_focus = "none"
  error = ""
  // Reads the peer back from state: `active_dm_peer = peer_key` above already
  // moved the payload, so passing `peer_key` here would be a use after move.
  run open_dm(connected_rpc, password, active_dm_peer) -> chat_updated _ | failed _

on create_channel_submit
  return if loading || mutation_phase != "idle" || empty(trim(channel_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "channel"
  // Creating lands you IN the new channel (`channel_created` moves
  // `active_channel`), so the page in flight for the room you were reading is
  // abandoned here. `mutation_phase` masks the dead button only until the
  // create settles.
  history_loading = false
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
  //
  // The RETIRE does not wait for that task, and is why this handler is on the
  // named list rather than derived: the modal lays a text input OVER the chat
  // composer, which stays mounted underneath. The press that got here was on
  // the New-channel button, and the next one goes into the modal's field — the
  // caret is in no composer either way, opening or cancelling.
  composer_focus = "none"

on toggle_channel_settings
  return if empty(active_channel)
  channel_settings_open = !channel_settings_open
  // Same modal-over-a-live-composer as `toggle_channel_create`: the drawer lays
  // its own inputs over a chat composer that stays mounted, so the claim on the
  // caret retires whether the panel is opening or closing. This handler is on
  // the NAMED list for that reason — it no longer writes `active_thread_seq = 0`
  // and so the rail rule cannot derive it.
  composer_focus = "none"
  channel_name_draft = active_channel_name
  // AND IT DOES NOT TEAR THE RAIL DOWN. It used to clear the thread, its
  // messages and `reply_editor` — so opening this drawer DISCARDED a reply you
  // were part-way through typing, and closing it again gave you an empty
  // composer. Nobody asked to close the thread; they asked to see the channel.
  //
  // The teardown was never needed to hide the rail either: the screen already
  // draws it under `if active_thread_seq > 0 && !channel_settings_open`, so the
  // drawer covers it either way. `close_thread` stays the one route that
  // discards a reply, because that one is a request to.
  //
  // The app's own rule, from the other direction: `reconnect` harvests the
  // composer draft and puts it back rather than letting a transition eat it.

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
// The two mark handlers are the ONLY unambiguous route: the button names its
// editor at the mount, so no focus reading is involved. The chord in
// `handlers/overlays.ice` has no such luxury and reads `composer_focus`.
on composer_mark(kind)
  return if loading || !connected || empty(active_channel)
  message_editor = composer_toggle_mark(message_editor, kind)

// Its rail twin. `thread_loading` replaces `loading` and the open-rail check
// replaces the channel check, matching the reply composer's own gate; a mark
// is a local edit, so `post_gate` stays out of it exactly as it does above.
on reply_composer_mark(kind)
  return if thread_loading || !connected || active_thread_seq <= 0
  reply_editor = composer_toggle_mark(reply_editor, kind)

on chat_composer_event(event)
  // THE CLAIM. Every editor interaction — a click into one included — lands in
  // one of these two handlers, so they are the only two things that may say
  // the caret is in a composer; the chord arrives on the app's ONE keyboard
  // subscription, which cannot see widget focus. A claim lasts until a handler
  // that moves the caret RETIRES it (grep `composer_focus = "none"`), because
  // the editor widget drops its own focus on any press landing outside it and
  // publishes nothing when it does.
  composer_focus = "message"
  message_editor = apply_composer_event(message_editor, event)
  return if !composer_submits(event)
  // Same apply-time re-read as `reply_composer_event` below: the composer's
  // `disabled=` was decided a frame ago.
  return if loading || !connected || empty(active_channel) || !empty(post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key)) || empty(trim(editor_text(message_editor)))
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
  has_older_history = history_has_older(messages)
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
  has_older_history = history_has_older(messages)
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
  has_older_history = history_has_older(messages)
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
  // THE MENU TAKES THE CARET — the focus task below is the app moving it by
  // hand, and dismissing the menu does not move it back. Every handler with a
  // `task widget focus` retires the discriminant for exactly this reason;
  // `tests.rs` lints the rule so a ninth one cannot forget it.
  composer_focus = "none"
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
  composer_focus = "none"
  sequential
    task widget focus #workspace-tabs/content/chat/thread-reaction-focus
    task widget focus-next

on arm_thread_message_delete(seq, body, rev)
  return if seq <= 0
  thread_selected_seq = seq
  thread_selected_rev = rev
  thread_message_action = "delete"
  thread_edit_draft = body
  composer_focus = "none"
  sequential
    task widget focus #workspace-tabs/content/chat/thread-delete-focus
    task widget focus-next

on begin_thread_message_edit(seq, body, rev)
  return if seq <= 0
  thread_selected_seq = seq
  thread_selected_rev = rev
  thread_message_action = "editing"
  thread_edit_draft = body
  composer_focus = "none"
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
  composer_focus = "none"
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
  composer_focus = "none"
  sequential
    task widget focus #workspace-tabs/content/chat/message-reaction-focus
    task widget focus-next

on arm_message_delete(seq, body, rev)
  return if seq <= 0
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = "delete"
  message_edit_draft = body
  composer_focus = "none"
  sequential
    task widget focus #workspace-tabs/content/chat/message-delete-focus
    task widget focus-next

on begin_message_edit(seq, body, rev)
  return if seq <= 0
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = "editing"
  message_edit_draft = body
  composer_focus = "none"
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
  // A rail that just opened has an UNTOUCHED reply composer, and the click
  // that opened it was on a message row, not on either editor — so the caret
  // is in NEITHER. Without this the previous thread's "reply" outlives it and
  // steers the first Cmd+B into an empty box.
  composer_focus = "none"
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
  // A page belongs to the channel that asked for it. Only `load_more_history`
  // bumps `history_generation` — switching channels does not — so the
  // generation guard alone lets a page still in flight for #a prepend into #b's
  // timeline. Same channel check `message_sent` makes on its own late arrival.
  // The flag is released ABOVE that guard: a page dropped for landing in the
  // wrong room must still free the button, or "Load older" stays dead in the
  // room she switched to.
  history_loading = false
  return if next.channel_id != active_channel
  messages = prepend_history(messages, next.messages)
  has_older_history = history_has_older(messages)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
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
  // The composer the caret may have been in is gone from the tree.
  composer_focus = "none"

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

// REACTIONS DO NOT TAKE THE MUTATION LOCK. They are additive reactor-set ops
// with no rev CAS — like message sends, which already run concurrently on
// operation ids. When they held `mutation_phase`, every in-flight reaction
// disabled all 32 picker cells for the whole sign-and-submit round trip, and
// a disabled cell captures no press, so the SECOND click of a picking
// session fell through to the backdrop and dismissed the picker; the hover
// bar's one-tap reactions silently no-op'd through the same window. The
// reactor-set fold is idempotent, so even a double-tap of the same emoji is
// safe, and the settled delta replays canonically over any interleaving.
on add_reaction_submit(emoji)
  return if loading || empty(active_channel) || active_channel_archived || selected_message_seq <= 0
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  error = ""
  messages = reaction_applied(messages, selected_message_seq, emoji, true)
  thread_messages = reaction_applied(thread_messages, selected_message_seq, emoji, true)
  run add_reaction(connected_rpc, password, active_channel, selected_message_seq, emoji) -> reaction_acked _ | reaction_failed _

// One-tap reactions do NOT select the row: the tap is its own complete act,
// and parking the selection tint on the message until the next Esc read as
// a leftover highlight (QA). The picker path still selects, because its
// overlay is anchored to the selection.
on add_reaction_at(seq, emoji)
  return if loading || empty(active_channel) || active_channel_archived || seq <= 0
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  error = ""
  messages = reaction_applied(messages, seq, emoji, true)
  thread_messages = reaction_applied(thread_messages, seq, emoji, true)
  run add_reaction(connected_rpc, password, active_channel, seq, emoji) -> reaction_acked _ | reaction_failed _

on remove_reaction_at(seq, emoji)
  return if loading || active_channel_archived || seq <= 0
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  error = ""
  messages = reaction_applied(messages, seq, emoji, false)
  thread_messages = reaction_applied(thread_messages, seq, emoji, false)
  run remove_reaction(connected_rpc, password, active_channel, seq, emoji) -> reaction_acked _ | reaction_failed _

// The settle-✓'s two-beat teardown. Beat one reads the animation still
// aimed at visible, keeps its anchor and flips the fade; beat two reads it
// aimed at hidden and drops the anchor, unmounting the ✓ (the 400ms fade
// always finishes inside one 1200ms beat).
on send_flash_tick
  send_flash_id = keep_str(animation.value(send_flash), send_flash_id, "")
  thread_send_flash_id = keep_str(animation.value(send_flash), thread_send_flash_id, "")
  send_flash = false

// A reaction's ack has nothing to restore: the optimistic fold is already on
// screen and the settled delta replays over it. Reactions never touch
// `mutation_phase` (see `add_reaction_submit`), so the shared `chat_acked`
// phase teardown has no business here — only a stale error to clear.
on reaction_acked(_result)
  error = ""

// A reaction failure leaves the optimistic fold as a LIE on screen; there is
// no rollback token (the fold is not invertible under concurrent deltas), so
// the canonical refetch IS the revert — committed or not. No phase to reset:
// reactions run outside the mutation lock.
on reaction_failed(cause)
  error = cause.message
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run live_resync_load(connected_rpc, active_channel, active_page, "chat", false, hydration_generation, 0) -> live_resynced _ | live_resync_failed _

// A composer's `disabled=` is decided at RENDER time; this runs at APPLY time,
// and the refusal can change in between — a subscription drop flips `connected`,
// an archive or a members-only delta lands, and the frame that drew a live Send
// is already stale. Re-read the gate here or the reply is optimistically
// appended, refused by the module, and rolled back under a raw 400. Same terms
// as the view; `settings_user_key` is what the screen mounts as `user_key`.
on reply_composer_event(event)
  composer_focus = "reply"
  reply_editor = apply_composer_event(reply_editor, event)
  return if !composer_submits(event)
  return if loading || thread_loading || !connected || empty(active_channel) || active_thread_seq <= 0 || !empty(post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key)) || empty(trim(editor_text(reply_editor)))
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
