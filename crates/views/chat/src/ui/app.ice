// CHAT, as a module-owned view: the channel sidebar (rooms and DIRECT), the
// message stream, the thread rail and the channel-details drawer, drawn from
// the facts the desktop app pushes. The screen and its components are the
// app's own (screens/chat.ice, components/chat.ice, components/dm.ice before
// the port). The drafts are the view's: the app hears a search, a rename, a
// member key or an edited body only when the reader submits it. The two
// composers are the app's surfaces — it keeps every room's and thread's
// words, and a submit reaches it without passing through this view.
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
  // THE TWO LISTS THAT DRAW TOGETHER CROSS THE MEMO BOUNDARY TOGETHER — see
  // `host::Timeline`. Folding a run's status into `live_agents` alone leaves
  // the timeline memo's key unmoved, and the hint never repaints.
  Timeline(messages:[ChatMessage], live_agents:[LiveRunHint])
  ChatProps(dark:bool, endpoint:str, network_name:str, network_chain_id:str, status:str, block_height:i64, search_phase:str, search_query:str, search_hits:[ChatSearchHit], rooms:[ChatSidebarRow], dm_rows:[DmSidebarRow], channel_create_open:bool, connected:bool, loading:bool, busy:bool, active_channel:str, active_dm_peer:str, active_dm:DmPeer, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, channel_members:[ChatMember], post_refusal:str, huddle_joined:bool, huddle_channel:str, huddle_channel_name:str, huddle_joined_at:i64, huddle_now:i64, call_muted:bool, messages:[ChatMessage], has_older_history:bool, history_view:bool, at_live_tail:bool, history_loading:bool, unread_boundary:i64, unread_marker_seq:i64, selected_message_seq:i64, selected_message_rev:i64, message_action:str, channel_settings_open:bool, active_thread_seq:i64, thread_target_seq:i64, thread_messages:[ChatMessage], thread_selected_seq:i64, thread_selected_rev:i64, thread_message_action:str, thread_has_more:bool, thread_next_reply_seq:i64, thread_loading:bool, copy_anchor_seq:i64, copy_head_seq:i64, copy_surface:str, sent_serial:i64, live_agents:[LiveRunHint])
  PropsItem(next:ChatProps, error:str)
  subscription props() -> PropsItem
  pure search_phase_of(name:&str) -> SearchPhase
  pure message_action_of(name:&str) -> MessageAction
  pure copy_surface_of(name:&str) -> CopySurface
  pure tone_of(dark:bool) -> Tone
  pure no_dm_peer() -> DmPeer
  pure send_search(query:&str) -> bool
  pure send_clear_search() -> bool
  pure send_open_hit(channel:&str, root_seq:i64, target_seq:i64) -> bool
  pure send_toggle_create() -> bool
  pure send_choose_channel(id:&str) -> bool
  pure send_choose_dm(key:&str) -> bool
  pure send_toggle_settings() -> bool
  pure send_show_huddle() -> bool
  pure send_leave_huddle() -> bool
  pure send_join_huddle() -> bool
  pure send_load_history() -> bool
  pure send_scrolled(absolute_x:f64, absolute_y:f64, relative_x:f64, relative_y:f64) -> bool
  pure send_open_link(url:&str) -> bool
  pure send_copy(text:&str, label:&str) -> bool
  pure send_copy_link(link:&str) -> bool
  pure send_add_reaction(seq:i64, emoji:&str) -> bool
  pure send_remove_reaction(seq:i64, emoji:&str) -> bool
  pure send_open_thread(seq:i64) -> bool
  pure send_cancel_run(run_id:&str) -> bool
  pure send_open_run(dispatch_id:&str) -> bool
  pure run_of_message(id:&str) -> str
  pure send_message_actions(seq:i64, body:&str, rev:i64) -> bool
  pure send_message_reactions(seq:i64, body:&str, rev:i64) -> bool
  pure send_begin_edit(seq:i64, body:&str, rev:i64) -> bool
  pure send_arm_delete(seq:i64, body:&str, rev:i64) -> bool
  pure send_clear_selection() -> bool
  pure send_press(seq:i64, surface:CopySurface) -> bool
  pure send_clear_range() -> bool
  pure send_copy_range() -> bool
  pure send_reaction_submit(emoji:&str) -> bool
  pure send_delete() -> bool
  pure send_rename(name:&str) -> bool
  pure send_archive() -> bool
  pure send_unarchive() -> bool
  pure send_add_member(key:&str) -> bool
  pure send_remove_member(key:&str) -> bool
  pure send_close_thread() -> bool
  pure send_thread_actions(seq:i64, body:&str, rev:i64) -> bool
  pure send_thread_reactions(seq:i64, body:&str, rev:i64) -> bool
  pure send_thread_begin_edit(seq:i64, body:&str, rev:i64) -> bool
  pure send_thread_arm_delete(seq:i64, body:&str, rev:i64) -> bool
  pure send_thread_clear_selection() -> bool
  pure send_thread_delete() -> bool
  pure send_load_thread() -> bool
  pure icon(name:&str) -> bytes
  pure connection_degraded(status:&str) -> bool
  pure message_plate(deleted:bool, selected:bool, in_range:bool) -> RowPlate
  pure seq_in_copy_range(seq:i64, anchor:i64, head:i64, surface:CopySurface, mine:CopySurface) -> bool
  pure copy_range_count(messages:&[ChatMessage], anchor:i64, head:i64) -> i64
  pure timeline_of(messages:&[ChatMessage], live_agents:&[LiveRunHint]) -> Timeline
  pure live_thread_label(agent:&str) -> str
  pure run_in_thread(live:&LiveRunHint, active_thread_seq:i64) -> bool
  pure copy_range_label(count:i64) -> str
  pure message_target_key(messages:&[ChatMessage], target:i64, changed:bool) -> i64
  pure thread_width_after_delta(width:f64, delta:f64, viewport:f64) -> f64
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
  endpoint = ""
  network_name = ""
  network_chain_id = ""
  status = ""
  block_height = 0
  search_phase:SearchPhase = SearchPhase.idle
  search_query = ""
  search_hits:[ChatSearchHit] = []
  rooms:[ChatSidebarRow] = []
  dm_rows:[DmSidebarRow] = []
  channel_create_open = false
  connected = false
  loading = false
  busy = false
  active_channel = ""
  active_dm_peer = ""
  active_dm:DmPeer = no_dm_peer()
  active_channel_name = ""
  active_channel_archived = false
  active_channel_members_only = false
  channel_members:[ChatMember] = []
  post_refusal = ""
  huddle_joined = false
  huddle_channel = ""
  huddle_channel_name = ""
  huddle_joined_at = 0
  huddle_now = 0
  call_muted = false
  messages:[ChatMessage] = []
  has_older_history = false
  history_view = false
  at_live_tail = false
  history_loading = false
  unread_boundary = 0
  unread_marker_seq = 0
  selected_message_seq = 0
  selected_message_rev = 0
  message_action:MessageAction = MessageAction.toolbar
  channel_settings_open = false
  active_thread_seq = 0
  thread_target_seq = 0
  thread_reveal_key = 0
  stream_reveal_key = 0
  thread_messages:[ChatMessage] = []
  live_agents:[LiveRunHint] = []
  timeline:Timeline = timeline_of([], [])
  thread_selected_seq = 0
  thread_selected_rev = 0
  thread_message_action:MessageAction = MessageAction.toolbar
  chat_viewport_width = 1280.0
  thread_width = 330.0
  thread_has_more = false
  thread_next_reply_seq = 0
  thread_loading = false
  copy_anchor_seq = 0
  copy_head_seq = 0
  copy_surface:CopySurface = CopySurface.nowhere
  // the last admitted send the app reported: the cue to snap to the tail
  sent_serial:i64 = 0
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

// The facts are the host's: one subscription, one item per change. A
// subscription, not a mount task, so a replacement restored from this
// view's state asks for the facts again on its own.
subscribe
  props() -> props_arrived _

on thread_resized(dx, _dy)
  thread_width = thread_width_after_delta(thread_width, -dx, chat_viewport_width)

on chat_viewport_changed(width, _height)
  chat_viewport_width = width
  thread_width = thread_width_after_delta(thread_width, 0.0, width)

on props_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  let next = item.next
  let thread_changed = next.endpoint != endpoint || next.active_channel != active_channel || next.active_thread_seq != active_thread_seq || next.thread_target_seq != thread_target_seq
  thread_reveal_key = message_target_key(next.thread_messages, next.thread_target_seq, thread_changed)
  let stream_changed = next.history_view && !next.loading && (next.endpoint != endpoint || next.active_channel != active_channel || !history_view || loading)
  stream_reveal_key = message_target_key(next.messages, next.selected_message_seq, stream_changed)
  let sent_now = next.sent_serial != sent_serial
  sent_serial = next.sent_serial
  endpoint = next.endpoint
  network_name = next.network_name
  network_chain_id = next.network_chain_id
  status = next.status
  block_height = next.block_height
  search_phase = search_phase_of(next.search_phase)
  search_query = next.search_query
  search_hits = next.search_hits
  rooms = next.rooms
  dm_rows = next.dm_rows
  channel_create_open = next.channel_create_open
  connected = next.connected
  loading = next.loading
  busy = next.busy
  active_channel = next.active_channel
  active_dm_peer = next.active_dm_peer
  active_dm = next.active_dm
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  channel_members = next.channel_members
  post_refusal = next.post_refusal
  huddle_joined = next.huddle_joined
  huddle_channel = next.huddle_channel
  huddle_channel_name = next.huddle_channel_name
  huddle_joined_at = next.huddle_joined_at
  huddle_now = next.huddle_now
  call_muted = next.call_muted
  messages = next.messages
  has_older_history = next.has_older_history
  history_view = next.history_view
  at_live_tail = next.at_live_tail
  history_loading = next.history_loading
  unread_boundary = next.unread_boundary
  unread_marker_seq = next.unread_marker_seq
  selected_message_seq = next.selected_message_seq
  selected_message_rev = next.selected_message_rev
  message_action = message_action_of(next.message_action)
  channel_settings_open = next.channel_settings_open
  active_thread_seq = next.active_thread_seq
  thread_target_seq = next.thread_target_seq
  thread_messages = next.thread_messages
  live_agents = next.live_agents
  timeline = timeline_of(next.messages, next.live_agents)
  thread_selected_seq = next.thread_selected_seq
  thread_selected_rev = next.thread_selected_rev
  thread_message_action = message_action_of(next.thread_message_action)
  thread_has_more = next.thread_has_more
  thread_next_reply_seq = next.thread_next_reply_seq
  thread_loading = next.thread_loading
  copy_anchor_seq = next.copy_anchor_seq
  copy_head_seq = next.copy_head_seq
  copy_surface = copy_surface_of(next.copy_surface)
  // SENDING IS A JUMP TO NOW. The minted row lands at the tail, and a
  // reader who had scrolled up would otherwise get her own send below the
  // fold. The app admits the send and moves `sent_serial`; the snap is the
  // view's, and it rides the palette match because a handler branches
  // nowhere else.
  match tone_of(next.dark)
    Tone.light
      active_palette = AppTheme.app
      flow
        from done sent_now
        done -> position_streams _
    Tone.dark
      active_palette = AppTheme.app_dark
      flow
        from done sent_now
        done -> position_streams _

on position_streams(sent_now)
  parallel
    flow
      from done sent_now
      done -> snap_stream _
    flow
      from done thread_reveal_key
      done -> reveal_thread _
    flow
      from done stream_reveal_key
      done -> reveal_stream _

on reveal_stream(target_key)
  return if target_key <= 0
  task widget scroll-to-key #chat/message-stream target_key

on reveal_thread(target_key)
  return if target_key <= 0
  task widget scroll-to-key #chat/thread-pane/thread-stream target_key

on snap_stream(moved)
  return if !moved
  task widget snap #chat/message-stream 0.0 0.0

on press_message(seq, surface)
  sent = send_press(seq, surface)

on clear_copy_range
  sent = send_clear_range()

on copy_selected_messages
  sent = send_copy_range()

on search_chat_submit
  return if empty(trim(search_draft))
  sent = send_search(trim(search_draft))

on clear_chat_search
  search_draft = ""
  sent = send_clear_search()

on open_chat_search_hit(channel_id, root_seq, target_seq)
  sent = send_open_hit(channel_id, root_seq, target_seq)

on toggle_channel_create
  sent = send_toggle_create()

on choose_channel(id)
  sent = send_choose_channel(id)

on choose_dm(peer_key)
  sent = send_choose_dm(peer_key)

on toggle_channel_settings
  return if empty(active_channel)
  channel_name_draft = active_channel_name
  sent = send_toggle_settings()

on show_huddle
  sent = send_show_huddle()

on leave_huddle_here
  sent = send_leave_huddle()

on join_huddle_submit
  sent = send_join_huddle()

on load_more_history
  sent = send_load_history()

on chat_scrolled(absolute_x, absolute_y, relative_x, relative_y)
  sent = send_scrolled(absolute_x, absolute_y, relative_x, relative_y)

on open_message_link(url)
  sent = send_open_link(url)

on copy_to_clipboard(text, label)
  sent = send_copy(text, label)

on copy_message_link(link)
  sent = send_copy_link(link)

on add_reaction_at(seq, emoji)
  sent = send_add_reaction(seq, emoji)

on remove_reaction_at(seq, emoji)
  sent = send_remove_reaction(seq, emoji)

on open_thread_for(seq)
  message_edit_draft = ""
  thread_edit_draft = ""
  sent = send_open_thread(seq)

on open_message_actions(seq, body, rev)
  return if seq <= 0
  message_edit_draft = body
  sent = send_message_actions(seq, body, rev)
  sequential
    task widget focus #chat/message-action-focus
    task widget focus-next

on open_message_reactions(seq, body, rev)
  return if seq <= 0 || active_channel_archived
  message_edit_draft = body
  sent = send_message_reactions(seq, body, rev)
  sequential
    task widget focus #chat/message-reaction-focus
    task widget focus-next

on begin_message_edit(seq, body, rev)
  return if seq <= 0
  message_edit_draft = body
  sent = send_begin_edit(seq, body, rev)

on arm_message_delete(seq, body, rev)
  return if seq <= 0
  message_edit_draft = body
  sent = send_arm_delete(seq, body, rev)
  sequential
    task widget focus #chat/message-delete-focus
    task widget focus-next

on clear_message_selection
  message_edit_draft = ""
  sent = send_clear_selection()

on add_reaction_submit(emoji)
  sent = send_reaction_submit(emoji)

on delete_message_submit
  sent = send_delete()

on rename_channel_submit
  return if busy || empty(trim(channel_name_draft))
  sent = send_rename(trim(channel_name_draft))

on archive_channel_submit
  sent = send_archive()

on unarchive_channel_submit
  sent = send_unarchive()

on add_channel_member_submit
  return if busy || empty(trim(member_key_draft))
  sent = send_add_member(trim(member_key_draft))

on remove_channel_member_submit(key)
  sent = send_remove_member(key)

on close_thread
  thread_edit_draft = ""
  sent = send_close_thread()

on open_thread_message_actions(seq, body, rev)
  return if seq <= 0
  thread_edit_draft = body
  sent = send_thread_actions(seq, body, rev)
  sequential
    task widget focus #chat/thread-pane/thread-action-focus
    task widget focus-next

on open_thread_message_reactions(seq, body, rev)
  return if seq <= 0 || active_channel_archived
  thread_edit_draft = body
  sent = send_thread_reactions(seq, body, rev)
  sequential
    task widget focus #chat/thread-pane/thread-reaction-focus
    task widget focus-next

on begin_thread_message_edit(seq, body, rev)
  return if seq <= 0
  thread_edit_draft = body
  sent = send_thread_begin_edit(seq, body, rev)

on arm_thread_message_delete(seq, body, rev)
  return if seq <= 0
  thread_edit_draft = body
  sent = send_thread_arm_delete(seq, body, rev)
  sequential
    task widget focus #chat/thread-pane/thread-delete-focus
    task widget focus-next

on clear_thread_message_selection
  thread_edit_draft = ""
  sent = send_thread_clear_selection()

on delete_thread_message_submit
  sent = send_thread_delete()

on load_more_thread
  sent = send_load_thread()

on cancel_run(run_id)
  sent = send_cancel_run(run_id)

on open_run(dispatch_id)
  sent = send_open_run(dispatch_id)

view
  sensor show=chat_viewport_changed resize=chat_viewport_changed
    ChatScreen search_draft<->search_draft message_edit_draft<->message_edit_draft channel_name_draft<->channel_name_draft member_key_draft<->member_key_draft thread_edit_draft<->thread_edit_draft #chat
      with
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
