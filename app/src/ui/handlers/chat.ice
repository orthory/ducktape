// NO `chat_search_phase` TERM, for the same reason the field no longer disables
// on it: a refined query pressed while the first one is still out must run, not be
// swallowed. `chat_search_loaded`/`chat_search_failed` both guard on
// `chat_search_generation`, which this bumps, so the superseded reply is what
// gets dropped — the last Enter wins, exactly as the last click does.
on search_chat_submit
  return if empty(trim(chat_search_draft))
  chat_search_generation = chat_search_generation + 1
  chat_search_phase = "searching"
  chat_search_hits = []
  error = ""
  run replace lane=chat_search search_chat(connected_rpc, "", trim(chat_search_draft), chat_search_generation) -> chat_search_loaded _ | chat_search_failed _

on chat_search_loaded(next)
  return if next.generation != chat_search_generation
  chat_search_hits = next.hits
  chat_search_phase = "done"
  error = ""

on chat_search_failed(cause)
  return if cause.generation != chat_search_generation
  // BACK TO "idle", NOT "done". `search_chat_submit` already emptied the hits,
  // so a phase that stayed non-idle here floats "No messages match" over a
  // search that never ran — a confident zero-result card beside the error
  // banner that says the opposite.
  chat_search_phase = "idle"
  error = cause.message

on clear_chat_search
  invalidate lane=chat_search
  chat_search_generation = chat_search_generation + 1
  chat_search_draft = ""
  chat_search_hits = []
  chat_search_phase = "idle"

on open_chat_search_hit(channel_id, root_seq, target_seq)
  // NO `loading` TERM. A hit clicked while another room is still loading used
  // to be discarded outright — see `choose_channel`. The load this launches
  // carries `chat_generation`, so the one it supersedes is dropped on arrival
  // instead of this click being dropped on the way out.
  return if mutation_phase != "idle"
  invalidate lane=chat_search
  invalidate lane=history
  invalidate lane=thread
  invalidate lane=live_thread
  // PARK THE ROOM SHE IS LEAVING, exactly as the two pickers do — the way back
  // OUT of a search hit is a click on that room, and without this it is the one
  // navigation in the pane that still pays a full round trip.
  message_cache = cache_channel_window(message_cache, active_channel, messages, channel_members, history_view)
  // AND PARK HER UNSENT WORDS WITH IT — see `message_drafts`. Both composers:
  // the rail belongs to this room too, and the reset below closes it.
  message_drafts = park_message_draft(message_drafts, active_channel, trim(editor_text(message_editor)))
  reply_drafts = park_reply_draft(reply_drafts, active_channel, active_thread_seq, trim(editor_text(reply_editor)))
  // FREEZE THE DIVIDER WHILE `active_channel` STILL NAMES THE ROOM SHE LEAVES —
  // same reason as `choose_channel`.
  unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, channel_id, unread_boundary)
  // THE HIT LANDS ON THE CLICK. Every one of these used to move only in
  // `chat_hit_loaded`, so a hit that lives in another room left that room's
  // header, its sidebar highlight and its rows on screen for the whole walk —
  // the "did my click land?" void #1059 removed from the pickers, still live on
  // the one navigation whose entire purpose is to jump somewhere else.
  active_channel = channel_id
  active_dm_peer = dm_peer_of_channel(active_dm_peer, settings_user_key, active_channel)
  active_dm = dm_peer_named(dm_peers, active_dm_peer)
  active_channel_name = channel_display_name(channels, active_channel, active_channel_name)
  active_channel_archived = channel_is_archived(channels, active_channel)
  active_channel_members_only = channel_is_members_only(channels, active_channel)
  // A HIT IS A HISTORY WINDOW, so the park is NOT restored here: those rows are
  // a live tail and these are a page around one old message. An empty timeline
  // under the skeleton is the honest state, and `history_view` says up front
  // which kind of window is coming.
  //
  // A HIT THAT FAILS LEAVES IT RAISED, and that is the honest reading too: the
  // amber "Viewing history" banner is gated on `!empty(messages)`
  // (`screens/chat.ice`), so the empty room shows the error banner alone, and
  // the raised flag keeps the read cursor off a room whose window never
  // arrived — the three `!history_view` gates in `lifecycle.ice`. It is lowered
  // by the next `choose_channel`/`choose_dm` or by any chat-carrying resync.
  history_view = true
  messages = []
  channel_members = []
  post_refusal = ""
  has_older_history = false
  unread_marker_seq = 0
  palette_open = false
  shell_tab = "chat"
  chat_search_generation = chat_search_generation + 1
  chat_search_phase = "idle"
  // Same abandoned request, same dead button — see `choose_channel`. This route
  // lands in a DIFFERENT channel via `chat_hit_loaded`, so the page still in
  // flight belongs to the room she jumped out of.
  history_loading = false
  // AND THE WINDOW IT LANDS IN IS STILL OUT — see `chat_window_loading`.
  chat_window_loading = true
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
  // AND THE MESSAGE COMPOSER COMES OFF ITS OWN PARK — the line that made this
  // comment true. It said "both composers are rebuilt" while only the rail's
  // was, so the stream's draft walked into the hit's room armed to send there.
  message_editor = editor(parked_message_draft(message_drafts, active_channel))
  // Both composers are rebuilt under the caret and the tab moved besides.
  composer_focus = "none"
  error = ""
  chat_generation = chat_generation + 1
  // Reads the room back from state, like `choose_dm` does: `active_channel =
  // channel_id` above already moved the payload.
  run replace lane=chat_load load_chat_hit(connected_rpc, channels, active_channel, root_seq, target_seq, chat_generation) -> chat_hit_loaded _ | chat_load_failed _

// THE LAST CLICK WINS. This used to open `return if loading`, and `loading` is
// true for the entire switch it starts — so the second click of a fast A→B→C
// was discarded with no sidebar move, no header change and no busy affordance
// anywhere, and the reader clicked again into the same void. The click is taken
// unconditionally now and the SUPERSEDED LOAD is what gets dropped, on
// `chat_generation` — the guard `thread_loaded`, `history_loaded` and
// `chat_search_loaded` have always used.
//
// `mutation_phase` stays: it is a mutation lock, not a load, and
// `channel_created` lands the reader in the room it just made. The sidebar rows
// disable on exactly that term, so the guard and the affordance agree.
on choose_channel(id)
  return if mutation_phase != "idle"
  invalidate lane=chat_search
  invalidate lane=history
  invalidate lane=thread
  invalidate lane=live_thread
  // PARK THE ROOM SHE IS LEAVING, while `active_channel` still names it.
  message_cache = cache_channel_window(message_cache, active_channel, messages, channel_members, history_view)
  // AND PARK HER UNSENT WORDS WITH IT — see `message_drafts`. Both composers:
  // the rail belongs to this room too, and the reset below closes it.
  message_drafts = park_message_draft(message_drafts, active_channel, trim(editor_text(message_editor)))
  reply_drafts = park_reply_draft(reply_drafts, active_channel, active_thread_seq, trim(editor_text(reply_editor)))
  active_dm_peer = ""
  active_dm = no_dm_peer()
  // "Jump to latest" IS this handler, aimed at the room already on screen — so
  // the window the banner describes ends here, not at the reply.
  history_view = false
  // FREEZE THE DIVIDER HERE, while the previous room is still `active_channel`
  // — the optimistic assignment below makes current == next by the time
  // `chat_updated` runs its own freeze, which then correctly keeps this value.
  unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, id, unread_boundary)
  // The switch is visible NOW: the clicked room takes the header and the
  // sidebar highlight, and — when it is one of the rooms the cache remembers —
  // its rows too, in this frame, with `loading` false. The refetch replays over
  // them through `merge_pending_messages` exactly as a live delta would.
  active_channel = id
  active_channel_name = channel_display_name(channels, active_channel, active_channel_name)
  // BOTH GATE FACTS RIDE THE CLICK. `post_refusal` is recomputed here, and
  // computing it from the room she LEFT is how a public channel came up
  // refusing her post for a whole round trip.
  active_channel_archived = channel_is_archived(channels, active_channel)
  active_channel_members_only = channel_is_members_only(channels, active_channel)
  // ONE WALK OF THE CACHE, NOT TWO. Both answers ride the same parked window
  // and the ABI charges by the argument — see `cached_window`.
  let parked = cached_window(message_cache, active_channel)
  messages = parked.messages
  channel_members = parked.members
  // A SEAT IS EITHER KNOWN OR NOT CLAIMED. The roll is [] on a cache miss, and
  // `post_gate` reads an empty roll as "not seated" — which would refuse the
  // composer of a members-only room she IS in, for a whole round trip. Archived
  // and open rooms need no roll to answer, so only the members-only miss is
  // unknown, and the honest answer there is nothing: a miss leaves `messages`
  // empty, so `loading` below is true and holds the composer anyway.
  let seat_known = !active_channel_members_only || !empty(channel_members)
  post_refusal = keep_str(seat_known, post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key), "")
  has_older_history = history_has_older(messages)
  // THE FLAG BELONGS TO THE REQUEST, AND THE REQUEST BELONGS TO THE ROOM YOU
  // LEFT. Invalidating `history` above ends both before the new room paints.
  history_loading = false
  // AND THE ROWS ON SCREEN ARE PROVISIONAL UNTIL THE WALK ANSWERS. A cache hit
  // paints the PREVIOUS window with `loading` false below, and the reply
  // replaces it wholesale — so the two history routes read this rather than
  // `loading`, or a page requested against a seq the fresh window no longer
  // starts at prepends a gap into the middle of the timeline. Only those two
  // routes read it: the switch handlers stay unswallowed.
  chat_window_loading = true
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  chat_search_generation = chat_search_generation + 1
  chat_search_phase = "idle"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  // A CACHE HIT IS NOT LOADING. The plate is `loading && empty(messages)`, so
  // painting parked rows already retires it — but `loading` also disables both
  // composers, and a room that repaints instantly with a dead message box is
  // the same "did my click land?" question one step later.
  loading = empty(messages)
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
  // A NEW ROOM'S COMPOSER IS THAT ROOM'S COMPOSER. Its own parked words, or an
  // empty box — never the ones she was writing next door.
  message_editor = editor(parked_message_draft(message_drafts, active_channel))
  // A new room's composer is a new box: the caret does not come with it.
  composer_focus = "none"
  error = ""
  chat_generation = chat_generation + 1
  // THE CHANNEL LIST GOES DOWN, IT DOES NOT COME BACK. `load_chat_data`
  // re-paged the whole list on every switch — a round trip in front of the
  // first row, for a list this handler is reading two statements above and the
  // live fold keeps current.
  //
  // AND THE TAIL IS ASSERTED, NOT INHERITED. The reset used to be a side effect
  // of the stream UNMOUNTING while `messages` was empty — and a cache hit never
  // empties it, so the `if` around the stack stays true across the whole
  // switch, iced diffs the surviving `scrollable::State` into the room being
  // entered, and it opens at the offset the last room was left at, often
  // clamped to the top of the cached window with the newest rows off screen.
  // Absolute 0.0 is the TAIL under `anchor-y=end` (a relative `snap-end` would
  // be the TOP of scrollback), and it is a no-op on the miss, where the stream
  // really does remount at 0. Composed because a `run` and a widget operation
  // each want to be the handler's last statement.
  parallel
    run replace lane=chat_load load_channel_window(connected_rpc, channels, active_channel, chat_generation) -> chat_updated _ | chat_load_failed _
    task widget scroll-to #workspace-tabs/content/chat/message-stream 0.0 0.0

// A DM is not a second message plane: it is the two-party members-only channel
// at `dm_channel_id(me, peer)`, resolved or created on the way in. Everything
// downstream of `chat_updated` is the ordinary channel path.
on choose_dm(peer_key)
  // Same last-click-wins rule as `choose_channel`, and the same reason.
  return if mutation_phase != "idle" || empty(peer_key)
  invalidate lane=chat_search
  invalidate lane=history
  invalidate lane=thread
  invalidate lane=live_thread
  invalidate lane=chat_load
  // The room she is leaving is parked here too — she may come back to it from
  // the DM.
  message_cache = cache_channel_window(message_cache, active_channel, messages, channel_members, history_view)
  // AND PARK HER UNSENT WORDS WITH IT — see `message_drafts`. Both composers:
  // the rail belongs to this room too, and the reset below closes it.
  message_drafts = park_message_draft(message_drafts, active_channel, trim(editor_text(message_editor)))
  reply_drafts = park_reply_draft(reply_drafts, active_channel, active_thread_seq, trim(editor_text(reply_editor)))
  active_dm_peer = peer_key
  active_dm = dm_peer_named(dm_peers, active_dm_peer)
  // A DM IS A CHANNEL AND ITS ID IS DERIVABLE. `dm_channel_id` is the same
  // deterministic hash `open_dm` resolves on the node side, so the room can
  // land on the CLICK here exactly as it does in `choose_channel`. Leaving
  // `active_channel` on the room she left is how the peer's face came up beside
  // that room's "Archived" badge, its "· 7 added" count and its composer
  // refusal for the several blocks a DM open takes.
  let dm_room = dm_channel_id(settings_user_key, active_dm_peer)
  // WITH NO USER KEY BOUND, `dm_room` IS A PHANTOM — `dm_channel_id` hashes ""
  // against the peer and answers an id no channel in the list carries, while
  // the node resolves the real one from its OWN key. That degrades to exactly
  // the behaviour this line replaced and no further: the cache misses, the
  // header keeps its previous name (`channel_display_name` falls back to
  // `current`), no sidebar row highlights, and `chat_updated` lands the real
  // room a round trip later. The DM header still draws, because it reads
  // `active_dm_peer`, which is the payload.
  // A DM open is a live tail, never a history window — see `history_view`.
  history_view = false
  // FREEZE THE DIVIDER WHILE `active_channel` STILL NAMES THE ROOM SHE LEAVES,
  // for the reason `choose_channel` gives: after the assignment below current
  // == next, and `chat_updated`'s own freeze then correctly keeps this value.
  unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, dm_room, unread_boundary)
  active_channel = dm_room
  active_channel_name = channel_display_name(channels, active_channel, active_channel_name)
  active_channel_archived = channel_is_archived(channels, active_channel)
  active_channel_members_only = channel_is_members_only(channels, active_channel)
  // AND THE DM COMES OFF THE PARK LIKE ANY OTHER ROOM. It is an ordinary
  // channel in `message_cache` once its id is known here, so a DM re-opened a
  // minute later paints in one frame instead of paying the full list walk to
  // become readable.
  let parked = cached_window(message_cache, active_channel)
  messages = parked.messages
  channel_members = parked.members
  // Same seat reading as `choose_channel`, and the same reason.
  let seat_known = !active_channel_members_only || !empty(channel_members)
  post_refusal = keep_str(seat_known, post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key), "")
  has_older_history = history_has_older(messages)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  // Same lane cancellation as `choose_channel`.
  history_loading = false
  // Same window still out — see `chat_window_loading`.
  chat_window_loading = true
  chat_search_generation = chat_search_generation + 1
  chat_search_phase = "idle"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  // A CACHE HIT IS NOT LOADING — see `choose_channel`.
  loading = empty(messages)
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
  // Same per-room composer as `choose_channel`. A DM is an ordinary channel and
  // its draft parks under `dm_room` like any other.
  message_editor = editor(parked_message_draft(message_drafts, active_channel))
  // Same new box as `choose_channel`.
  composer_focus = "none"
  error = ""
  // Reads the peer back from state: `active_dm_peer = peer_key` above already
  // moved the payload, so passing `peer_key` here would be a use after move.
  //
  // STILL `run every`, and deliberately: `open_dm` is a WRITE chain — create
  // the channel, then seat both keys — and a `replace` lane aborts a superseded
  // start mid-chain, leaving a members-only DM with nobody seated that
  // `open_dm`'s own "it already exists" early return would then treat as
  // finished forever. `chat_generation` drops the superseded REPLY instead.
  chat_generation = chat_generation + 1
  // Same asserted tail as `choose_channel`, and now for the same reason: a DM
  // that comes off the park keeps the stream mounted across the switch.
  parallel
    run every open_dm(connected_rpc, password, active_dm_peer, chat_generation) -> chat_updated _ | chat_load_failed _
    task widget scroll-to #workspace-tabs/content/chat/message-stream 0.0 0.0

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
  // AND THE SWITCH IT ABANDONS TOO. The bump above discards the reply of any
  // window load still out, so nothing will ever lower this term for it, and
  // "Load older" would stay refused in the new room for the rest of the
  // session. `mutation_phase` guards the window this create replaces.
  chat_window_loading = false
  pending_channel = trim(channel_draft)
  channel_draft = ""
  error = ""
  chat_generation = chat_generation + 1
  run every create_channel(connected_rpc, password, pending_channel, channel_create_members_only, chat_generation) -> channel_created _ | mutation_failed _

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
  run every rename_channel(connected_rpc, password, active_channel, trim(channel_name_draft)) -> chat_acked _ | mutation_failed _

on archive_channel_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || active_channel_archived
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "channel-archive"
  error = ""
  run every archive_channel(connected_rpc, password, active_channel) -> chat_acked _ | mutation_failed _

on unarchive_channel_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || !active_channel_archived
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "channel-unarchive"
  error = ""
  run every unarchive_channel(connected_rpc, password, active_channel) -> chat_acked _ | mutation_failed _

on add_channel_member_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || empty(trim(member_key_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "channel-member"
  error = ""
  run every add_channel_member(connected_rpc, password, active_channel, trim(member_key_draft)) -> chat_acked _ | mutation_failed _

on remove_channel_member_submit(key)
  return if loading || mutation_phase != "idle" || empty(active_channel)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "channel-member"
  error = ""
  run every remove_channel_member(connected_rpc, password, active_channel, key) -> chat_acked _ | mutation_failed _

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
  run every join_huddle(connected_rpc, password, active_channel) -> chat_acked _ | mutation_failed _

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
  // `disabled=` was decided a frame ago. `post_refusal` IS that re-read now —
  // it is state written by the handlers that move the gate's four inputs, so it
  // is as current here as a call would be, without the member-roll clone.
  return if loading || !connected || empty(active_channel) || !empty(post_refusal) || empty(trim(editor_text(message_editor)))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  pending_message = trim(editor_text(message_editor))
  pending_message_id = fresh_operation_id("message")
  message_draft = ""
  message_editor = editor("")
  // The mint does not re-mark the runs — the rail mints through the same call
  // and its `[root] ++ replies` vec must keep the first reply's header. This
  // vec is a plain run, so it re-marks here: the pending row groups under the
  // reader's previous message instead of drawing a header that vanishes the
  // moment the settle delta replaces it.
  messages = mark_author_runs(optimistic_message(messages, pending_message, pending_message_id))
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  error = ""
  run every send_message(connected_rpc, password, active_channel, pending_message_id, pending_message, channel_members) -> message_sent _ | message_send_failed _

on message_sent(next)
  return if active_channel != next.channel_id
  error = ""

// A FAILED SEND IS A FACT ABOUT THE USER'S TEXT, NOT ABOUT THE PANE ON SCREEN.
// The reader can leave the room while the write is in flight — a channel pick,
// a search hit, or a reconnect that lands on `channels.first()` — and the whole
// handler used to return on that, so the body, the error and every trace of it
// went away while the last thing she saw was her message in the timeline.
//
// So the room check scopes the TIMELINE SURGERY only. The unsent stash and the
// error banner are written first, unconditionally, above the guard.
//
// `remember_failed_draft` routes the body to the stash whenever the composer
// cannot take it back; the composer of the room she moved to cannot, and a
// non-empty `current` is how that is said here (`live_resynced` says it with
// the literal "channel").
on message_send_failed(cause)
  error = cause.message
  failed_message_draft = remember_failed_draft(failed_message_draft, keep_str(active_channel == cause.scope_id, trim(editor_text(message_editor)), "another room"), cause.body, cause.committed)
  return if active_channel != cause.scope_id
  messages = rollback_pending_message(messages, cause.operation_id, cause.committed)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  message_draft = restore_draft(trim(editor_text(message_editor)), cause.body, cause.committed)
  message_editor = editor(message_draft)
  return if !cause.committed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run replace lane=live_resync live_resync_load(connected_rpc, active_channel, active_page, "chat", false, hydration_generation, 0) -> live_resynced _ | live_resync_failed _

on chat_updated(next)
  // THE ROOM SHE IS IN NOW, OR NOTHING. Two clicks in flight land in the order
  // the node answers, not the order she clicked, so without this the FIRST
  // reply won and A→B→C settled on B. `loading` is deliberately NOT released
  // here: the load this one lost to is still running, and clearing it would
  // swap the loading plate for "No messages yet" mid-switch.
  return if next.generation != chat_generation
  history_view = false
  // FOLD, DO NOT REPLACE. `load_channel_window` answers with the one row it
  // refreshed, and the list it was handed is the PRE-CLICK snapshot: assigning
  // it back reverted every delta `live_updated` folded during the round trip —
  // a peer's post in a third room and the badge it lit, a channel created,
  // renamed or archived — and nothing re-pages the list to heal it.
  channels = upsert_channel_rows(channels, next.channels)
  messages = merge_pending_messages(next.messages, messages, active_channel, next.active_channel, "")
  has_older_history = history_has_older(messages)
  unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, next.active_channel, unread_boundary)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  channel_reads = mark_channel_read(channel_reads, next.active_channel, channel_head_seq(channels, next.active_channel))
  rooms = chat_sidebar_rooms(channels, dm_peers, settings_user_key, channel_reads)
  dm_rows = chat_sidebar_dms(channels, dm_peers, channel_reads)
  active_channel = next.active_channel
  // A LANDING ANSWERS FOR THE PEER TOO. The DM header suppresses the `#` and
  // the channel name, so a peer that outlives the room it named leaves the room
  // on screen unnamed under someone else's face — see `dm_peer_of_channel`.
  active_dm_peer = dm_peer_of_channel(active_dm_peer, settings_user_key, active_channel)
  active_dm = dm_peer_named(dm_peers, active_dm_peer)
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
  huddle_rows = huddle_tile_rows(huddle_roster, call_peers, call_muted)
  huddle_channel = keep_str(huddle_joined, active_channel, "")
  huddle_channel_name = keep_str(huddle_joined, active_channel_name, "")
  channel_members = next.channel_members
  post_refusal = post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key)
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
  // THE WINDOW ON SCREEN IS THE ANSWER NOW, so history may page against its
  // seqs again. Below the generation guard on purpose: a superseded reply must
  // not open the routes against rows the winning load is still about to
  // replace.
  chat_window_loading = false
  error = ""
  // The huddle ended under this fold (or the roster she is on is another
  // channel's): the window mirrors the old popped-card gate, which also
  // vanished the moment `huddle_joined` dropped. A no-op while still joined.
  task window close target=window_target_unless(huddle_joined, huddle_win)

on chat_hit_loaded(next)
  // Superseded by a later switch — see `chat_updated`.
  return if next.generation != chat_generation
  // THE ONE HANDLER THAT RAISES IT. `history_view` is a property of HOW the
  // rows in hand were fetched — a window around one old message — so every
  // other writer of `messages` lowers it, or the amber "Viewing history"
  // banner sits over a live tail with a "Jump to latest" that reloads the
  // channel you are already at the end of.
  history_view = true
  // Same fold as `chat_updated`, same loader, same reason.
  channels = upsert_channel_rows(channels, next.channels)
  messages = merge_pending_messages(next.messages, messages, active_channel, next.active_channel, "")
  has_older_history = history_has_older(messages)
  unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, next.active_channel, unread_boundary)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  // AND NO READ MARK, which is the one line this handler must NOT copy from
  // `chat_updated`. The rows in hand are `MessageWindow::Around(hit)`, not the
  // tail, and search is workspace-wide — so a hit clicked in a room with 80
  // unread would move the cursor to a head the reader has demonstrably not
  // reached, and `mark_channel_read` only ever moves forward, so the badge
  // `chat_sidebar_rooms` paints off that cursor would go out for good.
  // `live_updated` refuses exactly this write for a history window, and
  // `history_view = true` above says this is one. "Jump to latest" routes
  // through `choose_channel` -> `chat_updated`, which marks the room read when
  // she actually reaches the tail. The sidebar mirrors still refresh: the
  // `channels` fold above moved, even though the cursor did not.
  rooms = chat_sidebar_rooms(channels, dm_peers, settings_user_key, channel_reads)
  dm_rows = chat_sidebar_dms(channels, dm_peers, channel_reads)
  active_channel = next.active_channel
  // Same landing answer as `chat_updated`: a hit in another room retires the
  // peer, a hit inside the DM keeps him.
  active_dm_peer = dm_peer_of_channel(active_dm_peer, settings_user_key, active_channel)
  active_dm = dm_peer_named(dm_peers, active_dm_peer)
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
  huddle_rows = huddle_tile_rows(huddle_roster, call_peers, call_muted)
  huddle_channel = keep_str(huddle_joined, active_channel, "")
  huddle_channel_name = keep_str(huddle_joined, active_channel_name, "")
  channel_members = next.channel_members
  post_refusal = post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key)
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
  // THE WINDOW ON SCREEN IS THE ANSWER NOW, so history may page against its
  // seqs again. Below the generation guard on purpose: a superseded reply must
  // not open the routes against rows the winning load is still about to
  // replace.
  chat_window_loading = false
  error = ""
  // Same close-if-ended mirror as `chat_updated` above.
  task window close target=window_target_unless(huddle_joined, huddle_win)

// A SWITCH'S FAILURE BELONGS TO THAT SWITCH. The generic `failed` arm is not
// guarded — it could not be, it serves the page routes too — and while the room
// pickers were serialized by `return if loading` there was at most one chat load
// in flight, so it never had to be. That invariant is gone: the last click wins
// and the ones it passed are still out. Without this guard, A→B→C where B errors
// clears `loading` under C — swapping C's loading plate for "No messages yet" —
// and writes B's message into the banner until C lands.
on chat_load_failed(cause)
  return if cause.generation != chat_generation
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = false
  // Nothing is coming to replace the rows on screen, so they are as canonical
  // as they will get and history may page against them — same release as
  // `chat_updated`, under the same generation guard.
  chat_window_loading = false
  error = cause.message

on channel_created(next)
  // The lock and the modal come down whether or not this landing still counts:
  // a create she has since clicked away from must not leave the sidebar's
  // buttons dead. Same order `history_loaded` releases `history_loading` in.
  pending_channel = ""
  channel_create_open = false
  channel_create_members_only = false
  mutation_phase = "idle"
  // Superseded by a later switch — see `chat_updated`. The channel is still
  // created; it arrives in the list on the next fold.
  return if next.generation != chat_generation
  // A brand-new channel's latest page IS the whole channel — see
  // `chat_hit_loaded`.
  history_view = false
  channels = next.channels
  messages = merge_pending_messages(next.messages, messages, active_channel, next.active_channel, "")
  has_older_history = history_has_older(messages)
  unread_boundary = frozen_unread_boundary(channel_reads, next.channels, active_channel, next.active_channel, unread_boundary)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  channel_reads = mark_channel_read(channel_reads, next.active_channel, channel_head_seq(next.channels, next.active_channel))
  rooms = chat_sidebar_rooms(channels, dm_peers, settings_user_key, channel_reads)
  dm_rows = chat_sidebar_dms(channels, dm_peers, channel_reads)
  // A CREATE IS A ROOM SWITCH, so both composers park here like they do in the
  // three pickers — the line below lands her IN the new channel. Both keys must
  // be read while `active_channel` still names the room she is leaving.
  message_drafts = park_message_draft(message_drafts, active_channel, trim(editor_text(message_editor)))
  reply_drafts = park_reply_draft(reply_drafts, active_channel, active_thread_seq, trim(editor_text(reply_editor)))
  active_channel = next.active_channel
  // Creating lands you in the new room, which is nobody's DM.
  active_dm_peer = dm_peer_of_channel(active_dm_peer, settings_user_key, active_channel)
  active_dm = dm_peer_named(dm_peers, active_dm_peer)
  // AND THE NEW ROOM'S COMPOSER IS THE NEW ROOM'S. Without this the sentence
  // she was writing in the room she left followed her into the channel she just
  // made, sat above a live Send there, and the NEXT switch parked it under the
  // new room's id — silently reattributing it, so it was gone when she went
  // back for it.
  message_editor = editor(parked_message_draft(message_drafts, active_channel))
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
  huddle_rows = huddle_tile_rows(huddle_roster, call_peers, call_muted)
  huddle_channel = keep_str(huddle_joined, active_channel, "")
  huddle_channel_name = keep_str(huddle_joined, active_channel_name, "")
  channel_members = next.channel_members
  post_refusal = post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key)
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
  // The rail's ♡ is the stream's ♡ — same dead 32-cell picker on an archived
  // channel, same refusal. See `open_message_reactions` below for why the read
  // hands the standing banner back untouched.
  error = reaction_refusal(active_channel_archived, error)
  return if active_channel_archived
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
  run every edit_message(connected_rpc, password, active_channel, thread_selected_seq, thread_selected_rev, trim(thread_edit_draft), channel_members) -> chat_acked _ | mutation_failed _

on delete_thread_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || thread_selected_seq <= 0 || thread_message_action != "delete"
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "message-delete"
  error = ""
  run every delete_message(connected_rpc, password, active_channel, thread_selected_seq) -> chat_acked _ | mutation_failed _

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
  // ♡ ON AN ARCHIVED CHANNEL OPENS NOTHING. Its 32 cells are all disabled
  // there, so the picker was a dead-end overlay whose only exit was Esc.
  // Opening it is a READ, so the live arm hands the banner back untouched —
  // a failed send is not cleared by the reach for a reaction.
  error = reaction_refusal(active_channel_archived, error)
  return if active_channel_archived
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

// A LINK PRESS IS A HAND-OFF TO THE OS, and nothing else: no selection, no
// draft, no rail. Same route the page renderer's link press takes
// (`handlers/pages.ice`), and it shares that handler's two result arms.
on open_message_link(url)
  return if empty(url)
  run every open_external_url(url) -> external_url_opened _ | external_url_failed _

// THE INSPECTOR IS THE FINALITY MARK'S TARGET. The shield in the hover bar and
// the settled chip on my own bubble both land here, and both name the same
// right rail — so opening one closes the other.

on open_thread_for(seq)
  return if seq <= 0 || empty(active_channel)
  // PARK THE REPLY OF THE THREAD SHE IS LEAVING, while `active_thread_seq`
  // still names it — see `reply_drafts`. A no-op on an ordinary open, where the
  // rail was closed or its composer empty.
  reply_drafts = park_reply_draft(reply_drafts, active_channel, active_thread_seq, trim(editor_text(reply_editor)))
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
  // THE RAIL OPENS ON THE MESSAGE IT IS ABOUT. Emptying this was a 330px pane
  // of bare `sidebar` background for the whole round trip — no root row, no
  // skeleton, no busy hint anywhere (both loading arms below the loop are
  // gated on `thread_has_more`, which the next line clears), so the reader
  // could not tell WHICH thread she had opened. The clicked message is already
  // in hand — that is where the click came from — and `ThreadParentBlock`
  // draws it with no view change, because the root arm keys on
  // `thread_message.seq == active_thread_seq`. `thread_loaded` replaces the
  // vec wholesale, and a load that FAILS now leaves the root standing instead
  // of a pane that stays blank until Close.
  thread_messages = thread_root_seed(messages, thread_messages, seq)
  thread_next_reply_offset = 0
  thread_has_more = false
  // A HALF-TYPED REPLY IS NOT THE PRICE OF LOOKING AT ANOTHER THREAD. This
  // handler is what every "N replies" row in the timeline beside the rail
  // emits, and it blanks the LIVE buffer — so a click meant to check something
  // destroyed text nobody asked to discard, with no banner and no way back. The
  // park above holds it; the line below hands back whatever THIS thread was
  // left holding, which is the only thread those words can be posted in.
  // `close_thread` stays the one route that discards a reply, because that one
  // is a request to.
  reply_draft = ""
  reply_editor = editor(parked_reply_draft(reply_drafts, active_channel, active_thread_seq))
  pending_reply = ""
  // A rail that just opened has an UNTOUCHED reply composer, and the click
  // that opened it was on a message row, not on either editor — so the caret
  // is in NEITHER. Without this the previous thread's "reply" outlives it and
  // steers the first Cmd+B into an empty box.
  composer_focus = "none"
  error = ""
  run replace lane=thread load_thread(connected_rpc, active_channel, seq, 0, 0, thread_generation) -> thread_loaded _ | thread_failed _

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
  run replace lane=thread load_thread_page(connected_rpc, active_channel, active_thread_seq, thread_next_reply_offset, thread_generation) -> thread_page_loaded _ | thread_page_failed _

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

// PREFETCH BEFORE THE HARD STOP. Paging older history was reachable only by
// scrolling to the very top and hunting for a button: the reader hit a wall,
// found it, clicked, and only then ate the frame that mounts the page. The
// offset arrives relative to the scrollable's ANCHOR, and the stream is
// `anchor-y=end`, so 1.0 IS the top — the page starts inside the last tenth of
// the scrollback and is usually in by the time she reaches it. The button stays
// as the explicit fallback and every other term below is its guard verbatim.
//
// `has_older_history` rather than `history_has_older(messages)`: this fires per
// scroll step, and the extern takes the timeline BY VALUE. The mirror in state
// is written by every handler that moves `messages` for exactly this reason.
on chat_scrolled(_absolute_x, _absolute_y, _relative_x, relative_y)
  return if !near_scroll_top(relative_y) || history_loading || loading || chat_window_loading || mutation_phase != "idle" || empty(active_channel) || !has_older_history
  history_generation = history_generation + 1
  history_loading = true
  error = ""
  run replace lane=history load_older_messages(connected_rpc, active_channel, oldest_message_seq(messages), history_generation) -> history_loaded _ | history_failed _

// `chat_window_loading` rides with `loading` in BOTH guards, and it is the one
// term that survived the cache hit: `loading` is false for the whole round trip
// of a switch back into a parked room, and the parked rows this would page from
// are about to be replaced by the walk's own window. Paging against them
// prepends under a window that no longer starts there — seqs vanish from the
// MIDDLE of the timeline with no gap marker and `has_older_history` keeps
// walking backwards past the hole. It gates the two history routes and nothing
// else: the switch handlers themselves take every click.
on load_more_history
  return if history_loading || loading || chat_window_loading || mutation_phase != "idle" || empty(active_channel) || empty(messages) || !history_has_older(messages)
  history_generation = history_generation + 1
  history_loading = true
  error = ""
  run replace lane=history load_older_messages(connected_rpc, active_channel, oldest_message_seq(messages), history_generation) -> history_loaded _ | history_failed _

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
  invalidate lane=thread
  invalidate lane=live_thread
  thread_generation = thread_generation + 1
  live_thread_generation = live_thread_generation + 1
  // CLOSE IS A REQUEST TO DISCARD, and the park has to hear it: an empty park
  // DROPS the entry, so the reply she closed away does not come back the next
  // time she opens that thread. Read while `active_thread_seq` still names it.
  reply_drafts = park_reply_draft(reply_drafts, active_channel, active_thread_seq, "")
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
  run every edit_message(connected_rpc, password, active_channel, selected_message_seq, selected_message_rev, trim(message_edit_draft), channel_members) -> chat_acked _ | mutation_failed _

on delete_message_submit
  return if loading || mutation_phase != "idle" || empty(active_channel) || selected_message_seq <= 0 || message_action != "delete"
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "message-delete"
  error = ""
  run every delete_message(connected_rpc, password, active_channel, selected_message_seq) -> chat_acked _ | mutation_failed _

// REACTIONS DO NOT TAKE THE MUTATION LOCK. They are additive reactor-set ops
// with no rev CAS — like message sends, which already run concurrently on
// operation ids. When they held `mutation_phase`, every in-flight reaction
// disabled all 32 picker cells for the whole sign-and-submit round trip, and
// a disabled cell captures no press, so the SECOND click of a picking
// session fell through to the backdrop and dismissed the picker; the hover
// bar's one-tap reactions silently no-op'd through the same window. The
// reactor-set fold is idempotent, so even a double-tap of the same emoji is
// safe, and the settled delta replays canonically over any interleaving.
//
// AND AN ARCHIVED CHANNEL REFUSES OUT LOUD. All five reaction routes answer it
// with the banner instead of a silent `return` — the three mutations below
// (`add_reaction_submit`, `add_reaction_at`, `remove_reaction_at`) and both
// picker openers above (`open_message_reactions` in the stream,
// `open_thread_message_reactions` in the rail); `tests.rs` walks the five:
// the module refuses the op (`check_post_policy` via `reaction_target`), but
// the surface cannot carry that refusal — the quiet message rows are `lazy` on
// ONE dependency, so `active_channel_archived` never reaches a chip or a
// one-tap bar, and each of them kept its full hover/press ramp for an act that
// never happened. `reaction_refusal` hands the banner back on a live channel,
// so the refusing line changes nothing there and each mutation still clears the
// banner on its own line, below.
on add_reaction_submit(emoji)
  return if loading || empty(active_channel) || selected_message_seq <= 0
  error = reaction_refusal(active_channel_archived, error)
  return if active_channel_archived
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  error = ""
  messages = reaction_applied(messages, selected_message_seq, emoji, true)
  thread_messages = reaction_applied(thread_messages, selected_message_seq, emoji, true)
  run every add_reaction(connected_rpc, password, active_channel, selected_message_seq, emoji) -> reaction_acked _ | reaction_failed _

// One-tap reactions do NOT select the row: the tap is its own complete act,
// and parking the selection tint on the message until the next Esc read as
// a leftover highlight (QA). The picker path still selects, because its
// overlay is anchored to the selection.
on add_reaction_at(seq, emoji)
  return if loading || empty(active_channel) || seq <= 0
  error = reaction_refusal(active_channel_archived, error)
  return if active_channel_archived
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  error = ""
  messages = reaction_applied(messages, seq, emoji, true)
  thread_messages = reaction_applied(thread_messages, seq, emoji, true)
  run every add_reaction(connected_rpc, password, active_channel, seq, emoji) -> reaction_acked _ | reaction_failed _

on remove_reaction_at(seq, emoji)
  return if loading || seq <= 0
  error = reaction_refusal(active_channel_archived, error)
  return if active_channel_archived
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  error = ""
  messages = reaction_applied(messages, seq, emoji, false)
  thread_messages = reaction_applied(thread_messages, seq, emoji, false)
  run every remove_reaction(connected_rpc, password, active_channel, seq, emoji) -> reaction_acked _ | reaction_failed _

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
//
// IT REVERTS WHAT THE CANONICAL PAGE COVERS, which is the tail. A tap on a row
// the reader had paged BACK to is outside `load_chat_data`'s last-N-roots
// answer, so `resynced_messages` finds no canonical row to win on `rev` with
// and the phantom chip rides along until she re-enters the room. Taking it back
// there needs the failing (seq, emoji) carried to the landing — this cause
// carries a message and nothing else — and the alternative, replacing the whole
// window on this one path, throws away the scrollback that fold exists to keep.
// One stale chip on an old row is the cheaper lie.
on reaction_failed(cause)
  error = cause.message
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run replace lane=live_resync live_resync_load(connected_rpc, active_channel, active_page, "chat", false, hydration_generation, 0) -> live_resynced _ | live_resync_failed _

// A composer's `disabled=` is decided at RENDER time; this runs at APPLY time,
// and the refusal can change in between — a subscription drop flips `connected`,
// an archive or a members-only delta lands, and the frame that drew a live Send
// is already stale. Re-read the gate here or the reply is optimistically
// appended, refused by the module, and rolled back under a raw 400. Same terms
// as the view; `settings_user_key` is what the screen mounts as `user_key`.
//
// AND THE STREAM'S LOAD FLAG IS NOT THE RAIL'S. `loading` used to open this
// guard, and it is in NEITHER of the rail's `disabled=` expressions — so the
// one state that could raise it under an open rail met a fully lit Send that
// swallowed the click with no error and no banner. Every chat-plane writer of
// `loading = true` zeroes `active_thread_seq` in the same handler, so the term
// never fired for a chat load at all; the only state it caught was a PAGES load
// still out behind a cross-tab bounce (`open_page_search_hit`, `choose_page`),
// during which a reply is perfectly valid. `thread_loading` is the rail's own
// readiness, and it is what the button wears.
on reply_composer_event(event)
  composer_focus = "reply"
  reply_editor = apply_composer_event(reply_editor, event)
  return if !composer_submits(event)
  return if thread_loading || !connected || empty(active_channel) || active_thread_seq <= 0 || !empty(post_refusal) || empty(trim(editor_text(reply_editor)))
  live_thread_generation = live_thread_generation + 1
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  pending_reply = trim(editor_text(reply_editor))
  pending_reply_id = fresh_operation_id("reply")
  reply_draft = ""
  reply_editor = editor("")
  thread_messages = optimistic_message(thread_messages, pending_reply, pending_reply_id)
  error = ""
  run every send_reply(connected_rpc, password, active_channel, active_thread_seq, pending_reply_id, pending_reply, channel_members) -> thread_reply_sent _ | thread_reply_send_failed _

// Same rule as `message_send_failed`, and the hole is wider here: `close_thread`
// clears `thread_messages`, so merely closing the rail under an in-flight reply
// made the pending check fail and threw the text away with no error at all.
on thread_reply_send_failed(cause)
  error = cause.message
  failed_reply_draft = remember_failed_draft(failed_reply_draft, keep_str(active_channel == cause.scope_id && contains_pending_message(thread_messages, cause.operation_id), trim(editor_text(reply_editor)), "another thread"), cause.body, cause.committed)
  return if active_channel != cause.scope_id
  return if !contains_pending_message(thread_messages, cause.operation_id)
  thread_messages = rollback_pending_message(thread_messages, cause.operation_id, cause.committed)
  reply_draft = restore_draft(trim(editor_text(reply_editor)), cause.body, cause.committed)
  reply_editor = editor(reply_draft)
  // AND IT DOES NOT MOVE THE REPLY CURSOR. A committed reply grows the loaded
  // run by exactly one row, and `live_updated`'s `thread_offset_after_live`
  // already counts it when that reply's delta lands — which it does for every
  // committed reply, this one included. Counting it here too advanced the
  // cursor past a row nobody had loaded, and the next "Load more replies"
  // started one reply late: the skipped reply was never rendered at all.
  return if !cause.committed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run replace lane=live_resync live_resync_load(connected_rpc, active_channel, active_page, "chat", false, hydration_generation, 0) -> live_resynced _ | live_resync_failed _

on thread_reply_sent(next)
  return if active_channel != next.channel_id
  error = ""
