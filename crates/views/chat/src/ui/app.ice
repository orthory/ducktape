// CHAT, as a module-owned view on the kernel contract. The app pushes SESSION
// facts only (`session()` — one item per change): the connection, the reader,
// the sidebar the bell and the tray share, the huddle's native media, this
// process's agent runs, and the room the app has navigated to. Everything
// ABOUT that room — its record, its roster, its messages, its threads and its
// search — is read HERE through `rpc.view`, re-read on every chat block
// (`rpc.live`), and a reaction, a delete, a rename, an archive or a membership
// change leaves as `op.submit` the kernel signs.
//
// What still crosses as an intent is what the kernel has no door for: the two
// composers (host surfaces, whose submit the app signs — a body carries
// markdown and `@handles` one parser must resolve for a post and an edit
// alike), the huddle (native media), the clipboard and the link opener, and
// the navigation the whole app shares.
app ChatView
  title "Chat"
  palette active_palette
  id "dev.ducktape.view.chat"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"
use "../../../../../app/src/ui/ducktape-ui/recipes.ice"
use "../../../../../app/src/ui/components/icon.ice"
use "../../../../../app/src/ui/components/richbody.ice"
use "kit.ice"
use "dm.ice"
use "components.ice"
use "chat.ice"

enum SearchPhase
  idle
  searching
  done

enum MessageAction
  toolbar
  more
  reactions
  editing
  delete

enum CopySurface
  nowhere
  timeline
  thread

enum RowPlate
  plain
  selected
  ranged

// which palette the app's `dark` names — a handler branches on an enum only
enum Tone
  light
  dark

// did the session move the reader to another room (or another landing in it)
enum RoomMove
  stayed
  moved

// did the node answer the search, or refuse it
enum SearchOutcome
  answered
  refused

// did the landing's window seat a thread — a hit on a reply does
enum LandingThread
  absent
  seated

extern crate::host
  ChatChannel(id:str, name:str, archived:bool, members_only:bool, huddle_count:i64, head_seq:i64)
  ChatReaction(emoji:str, count:i64, reacted_by_me:bool)
  ChatMember(key:str, label:str)
  ChatSpan(mention:str, mention_link:str, link_text:str, link:str, bold_italic:str, bold:str, italic:str, plain:str)
  ChatBlock(kind:str, text:str, lang:str, rich:bool, spans:[ChatSpan])
  ChatMessage(id:str, view_key:i64, seq:i64, author:str, meta:str, body:str, edit_body:str, blocks:[ChatBlock], pending:bool, rev:i64, edited:bool, deleted:bool, reply_count:i64, thread_seq:i64, show_author:bool, initial:str, avatar_kind:str, height:i64, time:i64, reactions:[ChatReaction], render_rev:i64)
  ChatSidebarRow(channel:ChatChannel, unread:bool)
  DmPeer(key:str, name:str, initials:str, is_agent:bool, channel_id:str)
  DmSidebarRow(peer:DmPeer, unread:bool)
  ChatSearchHit(channel_id:str, seq:i64, root_seq:i64, author:str, text:str, meta:str)
  LiveRunHint(anchor_seq:i64, thread_root:i64, run_id:str, dispatch_id:str, agent:str, status:str)
  PendingSend(id:str, body:str, thread_seq:i64)
  // THE TWO LISTS THAT DRAW TOGETHER CROSS THE MEMO BOUNDARY TOGETHER — see
  // `host::Timeline`. Folding a run's status into `live_agents` alone leaves
  // the timeline memo's key unmoved, and the hint never repaints.
  Timeline(messages:[ChatMessage], live_agents:[LiveRunHint])
  CopyRange(anchor:i64, head:i64, surface:str)
  Session(dark:bool, connected:bool, endpoint:str, network_name:str, network_chain_id:str, status:str, block_height:i64, me:str, me_key:str, names_serial:i64, rooms:[ChatSidebarRow], dm_rows:[DmSidebarRow], channel_create_open:bool, active_channel:str, active_dm_peer:str, active_dm:DmPeer, land_seq:i64, unread_boundary:i64, busy:bool, loading:bool, huddle_joined:bool, huddle_channel:str, huddle_channel_name:str, huddle_joined_at:i64, huddle_now:i64, call_muted:bool, shift_held:bool, copy_chord_serial:i64, sent_serial:i64, pending_sends:[PendingSend], live_agents:[LiveRunHint])
  SessionItem(next:Session, error:str)
  RoomKey(serial:i64, names:i64, channel:str, land:i64, pages:i64)
  RoomItem(channel:str, name:str, archived:bool, members_only:bool, members:[ChatMember], messages:[ChatMessage], has_older:bool, thread_root:i64, error:str)
  ThreadKey(serial:i64, names:i64, channel:str, root:i64, target:i64, pages:i64)
  ThreadItem(root_seq:i64, target_seq:i64, messages:[ChatMessage], has_more:bool, next_reply_seq:i64, error:str)
  SearchKey(serial:i64, names:i64, query:str)
  SearchItem(query:str, hits:[ChatSearchHit], error:str)
  ActItem(error:str)
  subscription session() -> SessionItem
  // the room on screen, read by this view: once per key, then again on every
  // chat block
  subscription room(key:RoomKey) -> RoomItem
  subscription thread(key:ThreadKey) -> ThreadItem
  subscription search(key:SearchKey) -> SearchItem
  // every write's outcome, as the kernel answers it
  subscription acts() -> ActItem
  pure connection_serial_after(was_connected:bool, connected:bool, serial:i64) -> i64
  pure room_move(moved:bool) -> RoomMove
  pure search_outcome(answered:bool) -> SearchOutcome
  pure landing_thread(root:i64) -> LandingThread
  pure room_key(serial:i64, names:i64, channel:&str, land:i64, pages:i64) -> RoomKey
  pure thread_key(serial:i64, names:i64, channel:&str, root:i64, target:i64, pages:i64) -> ThreadKey
  pure search_key(serial:i64, names:i64, query:&str) -> SearchKey
  sync seat_reader(handle:&str, key_hex:&str) -> bool
  // the writes, signed by the kernel with the seated key
  sync write_reaction(channel:&str, seq:i64, emoji:&str, add:bool) -> bool
  sync write_delete(channel:&str, seq:i64) -> bool
  sync write_rename(channel:&str, name:&str) -> bool
  sync write_archived(channel:&str, archived:bool) -> bool
  sync write_membership(channel:&str, member_key:&str, member:bool) -> bool
  // the doors the kernel has no contract for yet
  pure send_open_hit(channel:&str, target_seq:i64) -> bool
  pure send_toggle_create() -> bool
  pure send_choose_channel(id:&str) -> bool
  pure send_choose_dm(key:&str) -> bool
  pure send_show_huddle() -> bool
  pure send_leave_huddle() -> bool
  pure send_join_huddle() -> bool
  pure send_scrolled(absolute_x:f64, absolute_y:f64, relative_x:f64, relative_y:f64) -> bool
  pure send_open_link(url:&str) -> bool
  pure send_copy(text:&str, label:&str) -> bool
  pure send_copy_link(link:&str) -> bool
  pure send_cancel_run(run_id:&str) -> bool
  pure send_open_run(dispatch_id:&str) -> bool
  pure send_begin_edit(scope:&str, body:&str, seq:i64, rev:i64) -> bool
  pure edit_body_of(messages:&[ChatMessage], seq:i64, rev:i64) -> str
  pure copy_surface_of(name:&str) -> CopySurface
  pure tone_of(dark:bool) -> Tone
  pure no_dm_peer() -> DmPeer
  pure run_of_message(id:&str) -> str
  pure icon(name:&str) -> bytes
  pure connection_degraded(status:&str) -> bool
  pure message_plate(deleted:bool, selected:bool, in_range:bool) -> RowPlate
  pure seq_in_copy_range(seq:i64, anchor:i64, head:i64, surface:CopySurface, mine:CopySurface) -> bool
  pure copy_range_after_press(anchor:i64, surface:CopySurface, seq:i64, pressed_in:CopySurface) -> CopyRange
  pure copy_range_rows(messages:&[ChatMessage], thread_messages:&[ChatMessage], surface:CopySurface) -> [ChatMessage]
  pure copy_range_text(messages:&[ChatMessage], anchor:i64, head:i64) -> str
  pure copy_range_count(messages:&[ChatMessage], anchor:i64, head:i64) -> i64
  pure copy_range_label(count:i64) -> str
  pure timeline_of(messages:&[ChatMessage], live_agents:&[LiveRunHint]) -> Timeline
  pure with_pending(messages:&[ChatMessage], pending:&[PendingSend], thread_seq:i64, me:&str) -> [ChatMessage]
  pure first_unread_seq(messages:&[ChatMessage], boundary:i64) -> i64
  pure post_gate(archived:bool, members_only:bool, members:&[ChatMember], me:&str) -> str
  pure reaction_refusal(archived:bool, banner:&str) -> str
  pure reaction_applied(messages:&[ChatMessage], seq:i64, emoji:&str, added:bool) -> [ChatMessage]
  pure near_scroll_top(relative_offset:f64) -> bool
  pure near_scroll_tail(relative_offset:f64) -> bool
  pure live_thread_label(agent:&str) -> str
  pure run_in_thread(live:&LiveRunHint, active_thread_seq:i64) -> bool
  pure message_target_key(messages:&[ChatMessage], target:i64, changed:bool) -> i64
  pure sidebar_width_after_delta(width:f64, delta:f64, viewport:f64) -> f64
  pure details_width_after_delta(width:f64, delta:f64, viewport:f64, sidebar:f64) -> f64
  pure thread_width_after_delta(width:f64, delta:f64, viewport:f64, sidebar:f64) -> f64
  pure block_action_menu_y(pointer_y:f64, viewport_height:f64) -> f64
  pure search_answer_stands(query:&str, draft:&str, searching:bool) -> bool
  pure reaction_palette() -> [str]
  pure plural(count:i64, one:&str, many:&str) -> str
  pure height_label(height:i64) -> str
  pure height_label_short(height:i64) -> str
  pure duck_channel_link(channel:str, chain_id:str) -> str
  pure duck_channel_message_link(channel:str, seq:i64, chain_id:str) -> str
  pure mmss(seconds:i64) -> str
  pure count_label(count:i64) -> str
  pure composer_scope(endpoint:&str, channel_id:&str) -> str
  pure thread_scope(endpoint:&str, channel_id:&str, thread_seq:i64) -> str
  pure edit_scope(endpoint:&str, channel_id:&str, seq:i64) -> str
  // The composers are the app's: it keeps each room's and thread's words,
  // and a submit reaches it without passing through here.
  component chat_composer(scope:str, kind:str, compact:bool, hint:str, blocked:bool, restore_blocked:bool, failed_note:str) -> unit

state
  active_palette:palette[AppTheme] = AppTheme.app
  // ---- the session, as the app pushes it ----
  endpoint = ""
  network_name = ""
  network_chain_id = ""
  status = ""
  block_height = 0
  connected = false
  session_loading = false
  session_busy = false
  rooms:[ChatSidebarRow] = []
  dm_rows:[DmSidebarRow] = []
  channel_create_open = false
  active_channel = ""
  active_dm_peer = ""
  active_dm:DmPeer = no_dm_peer()
  huddle_joined = false
  huddle_channel = ""
  huddle_channel_name = ""
  huddle_joined_at = 0
  huddle_now = 0
  call_muted = false
  unread_boundary = 0
  live_agents:[LiveRunHint] = []
  shift_held = false
  copy_chord_serial:i64 = 0
  sent_serial:i64 = 0
  pending_sends:[PendingSend] = []
  me = ""
  me_key = ""
  names_serial:i64 = 0
  land_seq:i64 = 0
  // ---- the reads this view owns ----
  // moves when the session comes up: the room is read afresh
  connection_serial:i64 = 0
  // moves when a write lands: the room is re-read without waiting for a block
  room_serial:i64 = 0
  // how many older pages the reader has asked for beyond the first
  history_pages:i64 = 0
  room_key:RoomKey = room_key(0, 0, "", 0, 0)
  // the room the reading in hand is FOR: a room the app has moved on from is
  // still loading, and the plate says so
  room_channel = ""
  room_messages:[ChatMessage] = []
  messages:[ChatMessage] = []
  channel_members:[ChatMember] = []
  active_channel_name = ""
  active_channel_archived = false
  active_channel_members_only = false
  post_refusal = ""
  has_older_history = false
  loading = false
  busy = false
  timeline:Timeline = timeline_of([], [])
  thread_pages:i64 = 0
  thread_key:ThreadKey = thread_key(0, 0, "", 0, 0, 0)
  active_thread_seq = 0
  thread_target_seq = 0
  thread_reveal_key = 0
  stream_reveal_key = 0
  thread_messages:[ChatMessage] = []
  thread_has_more = false
  // `thread_next_reply_seq` is the rail's "there is another page" reading; the
  // cursor itself is the loader's, so this is 1 while one exists and 0 when not
  thread_next_reply_seq = 0
  thread_loading = false
  search_key:SearchKey = search_key(0, 0, "")
  search_phase:SearchPhase = SearchPhase.idle
  search_query = ""
  search_hits:[ChatSearchHit] = []
  // ---- this screen's own UI state ----
  history_view = false
  at_live_tail = true
  history_loading = false
  unread_marker_seq = 0
  selected_message_seq = 0
  selected_message_rev = 0
  message_action:MessageAction = MessageAction.toolbar
  channel_settings_open = false
  thread_selected_seq = 0
  thread_selected_rev = 0
  thread_message_action:MessageAction = MessageAction.toolbar
  copy_anchor_seq = 0
  copy_head_seq = 0
  copy_surface:CopySurface = CopySurface.nowhere
  chat_viewport_width = 1280.0
  sidebar_width = 236.0
  details_width = 320.0
  thread_width = 330.0
  // the reader's own: the five drafts the screen edits. An edit draft is
  // seeded from the message the menu was opened on and leaves as text on
  // submit; the app never sees a keystroke.
  search_draft = ""
  message_edit_draft = ""
  channel_name_draft = ""
  member_key_draft = ""
  thread_edit_draft = ""
  host_error = ""
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false

// Subscriptions, not mount tasks, so a replacement restored from this view's
// state asks for the session and its reads again on its own.
subscribe
  session() -> session_arrived _
  room(room_key) when connected -> room_arrived _
  thread(thread_key) when connected && active_thread_seq > 0 -> thread_arrived _
  search(search_key) when connected && !empty(search_query) -> search_arrived _
  acts() -> act_done _

on sidebar_resized(dx, _dy)
  sidebar_width = sidebar_width_after_delta(sidebar_width, dx, chat_viewport_width)
  details_width = details_width_after_delta(details_width, 0.0, chat_viewport_width, sidebar_width)
  thread_width = thread_width_after_delta(thread_width, 0.0, chat_viewport_width, sidebar_width)

on details_resized(dx, _dy)
  details_width = details_width_after_delta(details_width, -dx, chat_viewport_width, sidebar_width)

on thread_resized(dx, _dy)
  thread_width = thread_width_after_delta(thread_width, -dx, chat_viewport_width, sidebar_width)

on chat_viewport_changed(width, _height)
  chat_viewport_width = width
  sidebar_width = sidebar_width_after_delta(sidebar_width, 0.0, width)
  details_width = details_width_after_delta(details_width, 0.0, width, sidebar_width)
  thread_width = thread_width_after_delta(thread_width, 0.0, width, sidebar_width)

on session_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  let next = item.next
  let sent_now = next.sent_serial != sent_serial
  let chord_now = next.copy_chord_serial != copy_chord_serial
  // A ROOM MOVE RESTARTS THE READS, and a landing is a room move of its own:
  // both are inputs to the key, so the subscription re-reads on either.
  let moved_room = next.active_channel != active_channel || next.land_seq != land_seq
  sent_serial = next.sent_serial
  copy_chord_serial = next.copy_chord_serial
  connection_serial = connection_serial_after(connected, next.connected, connection_serial)
  connected = next.connected
  endpoint = next.endpoint
  network_name = next.network_name
  network_chain_id = next.network_chain_id
  status = next.status
  block_height = next.block_height
  me = next.me
  me_key = next.me_key
  sent = seat_reader(next.me, next.me_key)
  names_serial = next.names_serial
  rooms = next.rooms
  dm_rows = next.dm_rows
  channel_create_open = next.channel_create_open
  active_channel = next.active_channel
  active_dm_peer = next.active_dm_peer
  active_dm = next.active_dm
  land_seq = next.land_seq
  unread_boundary = next.unread_boundary
  session_loading = next.loading
  session_busy = next.busy
  busy = session_busy
  huddle_joined = next.huddle_joined
  huddle_channel = next.huddle_channel
  huddle_channel_name = next.huddle_channel_name
  huddle_joined_at = next.huddle_joined_at
  huddle_now = next.huddle_now
  call_muted = next.call_muted
  shift_held = next.shift_held
  pending_sends = next.pending_sends
  live_agents = next.live_agents
  loading = session_loading || (!empty(active_channel) && room_channel != active_channel)
  // Four one-shot dispatches, composed: a handler's `match` and its `flow`
  // must each be its last statement, so every branch the session item implies
  // is a named step of its own.
  parallel
    // the room's own facts, which branch on whether the reader moved
    flow
      from done moved_room
      done -> session_settled _
    // SENDING IS A JUMP TO NOW. The row lands at the tail, and a reader who
    // had scrolled up would otherwise get her own send below the fold.
    flow
      from done sent_now
      done -> snap_stream _
    flow
      from done chord_now
      done -> copy_chord _
    flow
      from done next.dark
      done -> tone_changed _

on tone_changed(dark)
  match tone_of(dark)
    Tone.light
      active_palette = AppTheme.app
    Tone.dark
      active_palette = AppTheme.app_dark

// A ROOM MOVE ENDS EVERYTHING THAT NAMED THE OLD ROOM: the paged window, the
// selection, the menus, the rail and the copy range are all keyed by seqs in
// the room she left, and carried next door they would address other messages.
// Staying re-keys the reads in place, which is what picks up a reconnect, a
// name-directory change, or a landing inside the room already open.
on session_settled(moved_room)
  match room_move(moved_room)
    RoomMove.stayed
      messages = with_pending(room_messages, pending_sends, 0, me)
      unread_marker_seq = first_unread_seq(messages, unread_boundary)
      timeline = timeline_of(messages, live_agents)
      room_key = room_key(connection_serial + room_serial, names_serial, active_channel, land_seq, history_pages)
      thread_key = thread_key(connection_serial + room_serial, names_serial, active_channel, active_thread_seq, thread_target_seq, thread_pages)
    RoomMove.moved
      history_pages = 0
      selected_message_seq = 0
      selected_message_rev = 0
      message_action = MessageAction.toolbar
      message_edit_draft = ""
      active_thread_seq = 0
      thread_target_seq = 0
      thread_pages = 0
      thread_messages = []
      thread_has_more = false
      thread_next_reply_seq = 0
      thread_loading = false
      thread_selected_seq = 0
      thread_selected_rev = 0
      thread_message_action = MessageAction.toolbar
      thread_edit_draft = ""
      copy_anchor_seq = 0
      copy_head_seq = 0
      copy_surface = CopySurface.nowhere
      channel_settings_open = false
      // A fresh window mounts at the tail, so the "Jump to latest" float
      // starts down.
      at_live_tail = true
      room_messages = []
      messages = []
      timeline = timeline_of([], [])
      unread_marker_seq = 0
      channel_members = []
      post_refusal = ""
      has_older_history = false
      room_key = room_key(connection_serial + room_serial, names_serial, active_channel, land_seq, 0)
      thread_key = thread_key(connection_serial + room_serial, names_serial, active_channel, 0, 0, 0)

on snap_stream(moved)
  return if !moved
  task widget snap #chat/message-stream 0.0 0.0

on reveal_stream(target_key)
  return if target_key <= 0
  task widget scroll-to-key #chat/message-stream target_key

on reveal_thread(target_key)
  return if target_key <= 0
  task widget scroll-to-key #chat/thread-pane/thread-stream target_key

on room_arrived(item)
  host_error = item.error
  history_loading = false
  return if item.channel != active_channel
  room_channel = item.channel
  loading = session_loading
  return if !empty(item.error)
  active_channel_name = item.name
  active_channel_archived = item.archived
  active_channel_members_only = item.members_only
  channel_members = item.members
  post_refusal = post_gate(item.archived, item.members_only, item.members, me)
  room_messages = item.messages
  messages = with_pending(room_messages, pending_sends, 0, me)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  timeline = timeline_of(messages, live_agents)
  has_older_history = item.has_older
  // A WINDOW AROUND ONE OLD MESSAGE IS HISTORY, and so is a paged-back
  // scrollback: the amber band and the "Jump to latest" float both read this.
  history_view = land_seq > 0 || history_pages > 0
  // A LANDING NAMES A ROW, AND THE ROW HAS TO BE ON SCREEN. The window is read
  // AROUND that seq, so the row sits in the middle of a scrollback anchored at
  // its end — the reader would arrive looking at the newest message in the
  // window instead of the one the link named. `auto=` is off under
  // `history_view` for the same reason, so nothing else moves the offset.
  stream_reveal_key = message_target_key(messages, land_seq, land_seq > 0)
  // A HIT ON A REPLY SEATS ITS THREAD. The landing named a seq; only the node
  // knows whether that seq is a root or a reply inside one.
  match landing_thread(item.thread_root)
    LandingThread.absent
      thread_key = thread_key(connection_serial + room_serial, names_serial, active_channel, active_thread_seq, thread_target_seq, thread_pages)
      flow
        from done stream_reveal_key
        done -> reveal_stream _
    LandingThread.seated
      active_thread_seq = item.thread_root
      thread_target_seq = land_seq
      thread_pages = 0
      thread_loading = true
      thread_key = thread_key(connection_serial + room_serial, names_serial, active_channel, item.thread_root, land_seq, 0)
      flow
        from done stream_reveal_key
        done -> reveal_stream _

on thread_arrived(item)
  host_error = item.error
  thread_loading = false
  return if item.root_seq != active_thread_seq
  return if !empty(item.error)
  thread_messages = with_pending(item.messages, pending_sends, active_thread_seq, me)
  thread_target_seq = item.target_seq
  thread_has_more = item.has_more
  thread_next_reply_seq = item.next_reply_seq
  // The rail's twin of the stream's landing: a hit on a REPLY seats the thread
  // and names a row inside it, and the rail is end-anchored too.
  thread_reveal_key = message_target_key(thread_messages, item.target_seq, item.target_seq > 0)
  flow
    from done thread_reveal_key
    done -> reveal_thread _

on search_arrived(item)
  host_error = item.error
  return if empty(item.query) || item.query != search_query
  search_hits = item.hits
  // BACK TO "idle", NOT "done", on a failure: a phase that stayed non-idle
  // floats "No messages match" over a search that never ran, and a confident
  // zero-result card beside the error banner says the opposite of it.
  match search_outcome(empty(item.error))
    SearchOutcome.answered
      search_phase = SearchPhase.done
    SearchOutcome.refused
      search_phase = SearchPhase.idle
      search_query = ""

on act_done(item)
  busy = session_busy
  host_error = item.error
  // The menu the write was launched from is finished with either way — the
  // refusal speaks in the banner.
  selected_message_seq = 0
  selected_message_rev = 0
  message_action = MessageAction.toolbar
  message_edit_draft = ""
  thread_selected_seq = 0
  thread_selected_rev = 0
  thread_message_action = MessageAction.toolbar
  thread_edit_draft = ""
  member_key_draft = ""
  // READ IT BACK NOW rather than at the next block: the write is applied, and
  // the index answers for it as soon as the fold reaches its height.
  room_serial = room_serial + 1
  room_key = room_key(connection_serial + room_serial, names_serial, active_channel, land_seq, history_pages)
  thread_key = thread_key(connection_serial + room_serial, names_serial, active_channel, active_thread_seq, thread_target_seq, thread_pages)

// ---------- search ----------

on search_chat_submit
  return if empty(trim(search_draft))
  search_phase = SearchPhase.searching
  search_hits = []
  // Captured at the last place the draft and the string being sent are known
  // to match, so the two cannot drift apart.
  search_query = trim(search_draft)
  host_error = ""
  search_key = search_key(connection_serial, names_serial, search_query)

on clear_chat_search
  search_draft = ""
  search_query = ""
  search_hits = []
  search_phase = SearchPhase.idle
  search_key = search_key(connection_serial, names_serial, "")

// A hit lands the whole app in its room: the tray, `duck://` links and
// notifications steer the same field, so the room stays the app's to move.
on open_chat_search_hit(channel_id, _root_seq, target_seq)
  search_phase = SearchPhase.idle
  search_hits = []
  search_query = ""
  sent = send_open_hit(channel_id, target_seq)

// ---------- navigation the whole app shares ----------

on toggle_channel_create
  sent = send_toggle_create()

on choose_channel(id)
  sent = send_choose_channel(id)

on choose_dm(peer_key)
  sent = send_choose_dm(peer_key)

on toggle_channel_settings
  return if empty(active_channel)
  channel_name_draft = active_channel_name
  channel_settings_open = !channel_settings_open

on show_huddle
  sent = send_show_huddle()

on leave_huddle_here
  sent = send_leave_huddle()

on join_huddle_submit
  sent = send_join_huddle()

on open_message_link(url)
  sent = send_open_link(url)

on copy_to_clipboard(text, label)
  sent = send_copy(text, label)

on copy_message_link(link)
  message_action = MessageAction.toolbar
  thread_message_action = MessageAction.toolbar
  return if empty(link)
  sent = send_copy_link(link)

on cancel_run(run_id)
  sent = send_cancel_run(run_id)

on open_run(dispatch_id)
  sent = send_open_run(dispatch_id)

// ---------- the stream's window ----------

// PREFETCH BEFORE THE HARD STOP. The offset arrives relative to the
// scrollable's ANCHOR, and the stream is `anchor-y=end`, so 1.0 IS the top:
// the older page starts inside the last tenth of the scrollback. The button
// stays as the explicit fallback and repeats these terms.
on chat_scrolled(absolute_x, absolute_y, relative_x, relative_y)
  // ABOVE THE GUARD: the same offset that says "she is near the top" also says
  // whether she is at the tail, and the "Jump to latest" float is the only
  // reading of it. Below the early return it would freeze at whatever it held
  // when she left the last tenth of the scrollback.
  at_live_tail = near_scroll_tail(relative_y)
  sent = send_scrolled(absolute_x, absolute_y, relative_x, relative_y)
  return if !near_scroll_top(relative_y) || history_loading || loading || busy || empty(active_channel) || !has_older_history
  history_loading = true
  history_pages = history_pages + 1
  room_key = room_key(connection_serial + room_serial, names_serial, active_channel, land_seq, history_pages)

on load_more_history
  return if history_loading || loading || busy || empty(active_channel) || empty(messages) || !has_older_history
  history_loading = true
  history_pages = history_pages + 1
  room_key = room_key(connection_serial + room_serial, names_serial, active_channel, land_seq, history_pages)

// ---------- the message menus ----------

on open_message_actions(seq, body, rev)
  return if seq <= 0
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = MessageAction.more
  message_edit_draft = body
  sequential
    task widget focus #chat/message-action-focus
    task widget focus-next

// ♡ ON AN ARCHIVED CHANNEL OPENS NOTHING: its 32 cells are all disabled there,
// so the picker was a dead-end overlay whose only exit was Esc. Opening it is a
// READ, so the refusal hands the standing banner back untouched — a failed send
// is not cleared by the reach for a reaction.
on open_message_reactions(seq, body, rev)
  return if seq <= 0
  host_error = reaction_refusal(active_channel_archived, host_error)
  return if active_channel_archived
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = MessageAction.reactions
  message_edit_draft = body
  sequential
    task widget focus #chat/message-reaction-focus
    task widget focus-next

// THE EDITOR IS A HOST SURFACE, seeded from the row's own stable markdown —
// never from the display text, which has already resolved every mention to the
// name it renders as today. An empty seed is the refusal: a deleted row has no
// body, and a revision that moved under the open menu would be saved over.
on begin_message_edit(seq, body, rev)
  return if seq <= 0
  let seed = edit_body_of(messages, seq, rev)
  return if empty(seed)
  sent = send_begin_edit(edit_scope(endpoint, active_channel, seq), seed, seq, rev)
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = MessageAction.editing
  message_edit_draft = body

on arm_message_delete(seq, body, rev)
  return if seq <= 0
  selected_message_seq = seq
  selected_message_rev = rev
  message_action = MessageAction.delete
  message_edit_draft = body
  sequential
    task widget focus #chat/message-delete-focus
    task widget focus-next

on clear_message_selection
  selected_message_seq = 0
  selected_message_rev = 0
  message_action = MessageAction.toolbar
  message_edit_draft = ""

on open_thread_message_actions(seq, body, rev)
  return if seq <= 0
  thread_selected_seq = seq
  thread_selected_rev = rev
  thread_message_action = MessageAction.more
  thread_edit_draft = body
  sequential
    task widget focus #chat/thread-pane/thread-action-focus
    task widget focus-next

on open_thread_message_reactions(seq, body, rev)
  return if seq <= 0
  host_error = reaction_refusal(active_channel_archived, host_error)
  return if active_channel_archived
  thread_selected_seq = seq
  thread_selected_rev = rev
  thread_message_action = MessageAction.reactions
  thread_edit_draft = body
  sequential
    task widget focus #chat/thread-pane/thread-reaction-focus
    task widget focus-next

on begin_thread_message_edit(seq, body, rev)
  return if seq <= 0
  let seed = edit_body_of(thread_messages, seq, rev)
  return if empty(seed)
  sent = send_begin_edit(edit_scope(endpoint, active_channel, seq), seed, seq, rev)
  thread_selected_seq = seq
  thread_selected_rev = rev
  thread_message_action = MessageAction.editing
  thread_edit_draft = body

on arm_thread_message_delete(seq, body, rev)
  return if seq <= 0
  thread_selected_seq = seq
  thread_selected_rev = rev
  thread_message_action = MessageAction.delete
  thread_edit_draft = body
  sequential
    task widget focus #chat/thread-pane/thread-delete-focus
    task widget focus-next

on clear_thread_message_selection
  thread_selected_seq = 0
  thread_selected_rev = 0
  thread_message_action = MessageAction.toolbar
  thread_edit_draft = ""

// ---------- the thread rail ----------

on open_thread_for(seq)
  return if seq <= 0 || empty(active_channel)
  channel_settings_open = false
  selected_message_seq = 0
  selected_message_rev = 0
  message_action = MessageAction.toolbar
  message_edit_draft = ""
  thread_selected_seq = 0
  thread_selected_rev = 0
  thread_message_action = MessageAction.toolbar
  thread_edit_draft = ""
  // The rail's list goes with the rail, and so does the range: a reply's seq
  // comes from the CHANNEL's sequence, so a range left standing would tint
  // replies in whichever thread opens next.
  copy_anchor_seq = 0
  copy_head_seq = 0
  copy_surface = CopySurface.nowhere
  thread_loading = true
  thread_messages = []
  thread_has_more = false
  thread_next_reply_seq = 0
  thread_pages = 0
  active_thread_seq = seq
  thread_target_seq = 0
  thread_key = thread_key(connection_serial + room_serial, names_serial, active_channel, seq, 0, 0)

on close_thread
  active_thread_seq = 0
  thread_target_seq = 0
  thread_messages = []
  thread_pages = 0
  thread_has_more = false
  thread_next_reply_seq = 0
  thread_loading = false
  thread_selected_seq = 0
  thread_selected_rev = 0
  thread_message_action = MessageAction.toolbar
  thread_edit_draft = ""
  copy_anchor_seq = 0
  copy_head_seq = 0
  copy_surface = CopySurface.nowhere
  thread_key = thread_key(connection_serial + room_serial, names_serial, active_channel, 0, 0, 0)

on load_more_thread
  return if thread_loading || busy || active_thread_seq <= 0 || !thread_has_more
  thread_loading = true
  thread_pages = thread_pages + 1
  thread_key = thread_key(connection_serial + room_serial, names_serial, active_channel, active_thread_seq, thread_target_seq, thread_pages)

// ---------- the writes ----------

// REACTIONS DO NOT TAKE THE MUTATION LOCK. They are additive reactor-set ops
// with no rev CAS, and a disabled cell captures no press — holding the lock
// made the SECOND click of a picking session fall through to the backdrop and
// dismiss the picker. The reactor-set fold is idempotent, so even a double tap
// of the same emoji is safe.
//
// AND AN ARCHIVED CHANNEL REFUSES OUT LOUD. The module refuses the op, but the
// quiet message rows are `lazy` on ONE dependency, so `active_channel_archived`
// never reaches a chip or a one-tap bar: the refusal has to speak here.
on add_reaction_submit(emoji)
  return if empty(active_channel) || selected_message_seq <= 0
  host_error = reaction_refusal(active_channel_archived, host_error)
  return if active_channel_archived
  host_error = ""
  room_messages = reaction_applied(room_messages, selected_message_seq, emoji, true)
  messages = with_pending(room_messages, pending_sends, 0, me)
  timeline = timeline_of(messages, live_agents)
  thread_messages = reaction_applied(thread_messages, selected_message_seq, emoji, true)
  sent = write_reaction(active_channel, selected_message_seq, emoji, true)

// One-tap reactions do NOT select the row: the tap is its own complete act,
// and parking the selection tint on the message until the next Esc read as a
// leftover highlight. The picker path still selects, because its overlay is
// anchored to the selection.
on add_reaction_at(seq, emoji)
  return if empty(active_channel) || seq <= 0
  host_error = reaction_refusal(active_channel_archived, host_error)
  return if active_channel_archived
  host_error = ""
  room_messages = reaction_applied(room_messages, seq, emoji, true)
  messages = with_pending(room_messages, pending_sends, 0, me)
  timeline = timeline_of(messages, live_agents)
  thread_messages = reaction_applied(thread_messages, seq, emoji, true)
  sent = write_reaction(active_channel, seq, emoji, true)

on remove_reaction_at(seq, emoji)
  return if empty(active_channel) || seq <= 0
  host_error = reaction_refusal(active_channel_archived, host_error)
  return if active_channel_archived
  host_error = ""
  room_messages = reaction_applied(room_messages, seq, emoji, false)
  messages = with_pending(room_messages, pending_sends, 0, me)
  timeline = timeline_of(messages, live_agents)
  thread_messages = reaction_applied(thread_messages, seq, emoji, false)
  sent = write_reaction(active_channel, seq, emoji, false)

// AN EDIT IS A COMPOSER DOCUMENT, and so it never passes through here: the
// editor is a host surface keyed by `edit_scope`, and its submit reaches the
// app as the `composer` intent — the same parser resolves markdown and the
// `<@n>` mention tokens for a post and an edit alike.

on delete_message_submit
  return if busy || empty(active_channel) || selected_message_seq <= 0 || message_action != MessageAction.delete
  host_error = ""
  busy = write_delete(active_channel, selected_message_seq)

on delete_thread_message_submit
  return if busy || empty(active_channel) || thread_selected_seq <= 0 || thread_message_action != MessageAction.delete
  host_error = ""
  busy = write_delete(active_channel, thread_selected_seq)

on rename_channel_submit
  return if busy || empty(active_channel) || empty(trim(channel_name_draft))
  host_error = ""
  busy = write_rename(active_channel, trim(channel_name_draft))

on archive_channel_submit
  return if busy || empty(active_channel) || active_channel_archived
  host_error = ""
  busy = write_archived(active_channel, true)

on unarchive_channel_submit
  return if busy || empty(active_channel) || !active_channel_archived
  host_error = ""
  busy = write_archived(active_channel, false)

on add_channel_member_submit
  return if busy || empty(active_channel) || empty(trim(member_key_draft))
  host_error = ""
  busy = write_membership(active_channel, trim(member_key_draft), true)

on remove_channel_member_submit(key)
  return if busy || empty(active_channel) || empty(key)
  host_error = ""
  busy = write_membership(active_channel, key, false)

// ---------- the copy range ----------

// A press on a message's prose, in either surface. Plain, it is only a press —
// a reader clicking around a room must not keep lighting a one-message range
// and its bar. With ⇧ held it starts a range here, or keeps the anchor and
// moves the far end of the one already open. The modifier is the app's to
// read (a press carries none of its own), so it arrives as a session fact.
on press_message(seq, surface)
  return if !shift_held
  let range = copy_range_after_press(copy_anchor_seq, copy_surface, seq, surface)
  copy_anchor_seq = range.anchor
  copy_head_seq = range.head
  copy_surface = copy_surface_of(range.surface)

on clear_copy_range
  copy_anchor_seq = 0
  copy_head_seq = 0
  copy_surface = CopySurface.nowhere

// TWO DOORS, ONE ACT: the bar's button and ⌘C. The chord is a keyboard
// subscription, which is the app's door, so it arrives as a moved serial.
on copy_selected_messages
  let rows = copy_range_rows(messages, thread_messages, copy_surface)
  let count = copy_range_count(rows, copy_anchor_seq, copy_head_seq)
  return if count == 0
  sent = send_copy(copy_range_text(rows, copy_anchor_seq, copy_head_seq), copy_range_label(count))

on copy_chord(fired)
  return if !fired
  let rows = copy_range_rows(messages, thread_messages, copy_surface)
  let count = copy_range_count(rows, copy_anchor_seq, copy_head_seq)
  return if count == 0
  sent = send_copy(copy_range_text(rows, copy_anchor_seq, copy_head_seq), copy_range_label(count))

view
  sensor show=chat_viewport_changed resize=chat_viewport_changed
    ChatScreen search_draft<->search_draft message_edit_draft<->message_edit_draft channel_name_draft<->channel_name_draft member_key_draft<->member_key_draft thread_edit_draft<->thread_edit_draft #chat
      with
        sidebar_width
        details_width
        thread_width
        endpoint
        network_name
        network_chain_id
        status
        block_height
        search_phase
        search_query
        search_hits
        rooms
        dm_rows
        channel_create_open
        connected
        loading
        busy
        active_channel
        active_dm_peer
        active_dm
        active_channel_name
        active_channel_archived
        active_channel_members_only
        channel_members
        post_refusal
        huddle_joined
        huddle_channel
        huddle_channel_name
        huddle_joined_at
        huddle_now
        call_muted
        messages
        has_older_history
        history_view
        at_live_tail
        history_loading
        unread_boundary
        unread_marker_seq
        selected_message_seq
        selected_message_rev
        message_action
        channel_settings_open
        active_thread_seq
        thread_target_seq
        thread_messages
        live_agents
        timeline
        thread_selected_seq
        thread_selected_rev
        thread_message_action
        thread_has_more
        thread_next_reply_seq
        thread_loading
        copy_anchor_seq
        copy_head_seq
        copy_surface
      events
        press_message -> press_message _ _
        clear_copy_range -> clear_copy_range
        copy_selected_messages -> copy_selected_messages
        search_chat_submit -> search_chat_submit
        clear_chat_search -> clear_chat_search
        open_chat_search_hit -> open_chat_search_hit _ _ _
        toggle_channel_create -> toggle_channel_create
        choose_channel -> choose_channel _
        choose_dm -> choose_dm _
        toggle_channel_settings -> toggle_channel_settings
        show_huddle -> show_huddle
        leave_huddle_here -> leave_huddle_here
        join_huddle_submit -> join_huddle_submit
        load_more_history -> load_more_history
        chat_scrolled -> chat_scrolled _ _ _ _
        open_message_link -> open_message_link _
        copy_to_clipboard -> copy_to_clipboard _ _
        copy_message_link -> copy_message_link _
        add_reaction_at -> add_reaction_at _ _
        remove_reaction_at -> remove_reaction_at _ _
        open_thread_for -> open_thread_for _
        open_message_actions -> open_message_actions _ _ _
        open_message_reactions -> open_message_reactions _ _ _
        begin_message_edit -> begin_message_edit _ _ _
        arm_message_delete -> arm_message_delete _ _ _
        clear_message_selection -> clear_message_selection
        add_reaction_submit -> add_reaction_submit _
        delete_message_submit -> delete_message_submit
        rename_channel_submit -> rename_channel_submit
        archive_channel_submit -> archive_channel_submit
        unarchive_channel_submit -> unarchive_channel_submit
        add_channel_member_submit -> add_channel_member_submit
        remove_channel_member_submit -> remove_channel_member_submit _
        resize_sidebar -> sidebar_resized _ _
        resize_details -> details_resized _ _
        resize_thread -> thread_resized _ _
        close_thread -> close_thread
        open_thread_message_actions -> open_thread_message_actions _ _ _
        open_thread_message_reactions -> open_thread_message_reactions _ _ _
        begin_thread_message_edit -> begin_thread_message_edit _ _ _
        arm_thread_message_delete -> arm_thread_message_delete _ _ _
        clear_thread_message_selection -> clear_thread_message_selection
        delete_thread_message_submit -> delete_thread_message_submit
        load_more_thread -> load_more_thread
        cancel_run -> cancel_run _
        open_run -> open_run _
