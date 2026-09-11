// THE CHAT TAB IS A MODULE-OWNED VIEW ON THE KERNEL CONTRACT
// (`crates/views/chat`). Everything ABOUT a room — its record, its roster,
// its messages, its threads, its search, its reactions, edits, deletes,
// renames and memberships — is the VIEW's: it reads the index through
// `rpc.view`, re-reads on every chat block through `rpc.live`, and signs its
// writes through `op.submit`.
//
// WHAT IS LEFT HERE is what another plane of the app steers or owns:
//
//   * the room the app is in, because `duck://` links, notifications, the
//     tray, the palette and the forge discussion all move it;
//   * the sidebar rows and the read cursors, because the bell and the tray
//     read the same fold;
//   * the huddle, which is native media, and the channel create, which is a
//     modal the palette also opens;
//   * the two composers, which are HOST SURFACES (`crate::composer_surface`):
//     the words never cross the wire, so the send, the edit, the mention menu
//     and the failed-send stash stay here;
//   * the OS doors — the clipboard and the link opener.

// A LANDING IS A ROOM MOVE WITH A SEQ. A search hit or a
// `duck://channel/<id>#<seq>` names one old message; `chat_land_seq` is what
// the view opens its window around, and it is an input to that view's own
// read key — so moving it re-reads the room without this handler naming a
// single message.
on open_chat_search_hit(channel_id, target_seq)
  // NO `loading` TERM. A hit clicked while another room is still loading used
  // to be discarded outright — see `choose_channel`. The load this launches
  // carries `chat_generation`, so the one it supersedes is dropped on arrival
  // instead of this click being dropped on the way out.
  return if mutation_phase != MutationPhase.idle
  // FREEZE THE DIVIDER WHILE `active_channel` STILL NAMES THE ROOM SHE LEAVES
  // — same reason as `choose_channel`.
  let next_channel = channel_switch_facts(channel_reads, channels, active_channel, channel_id, unread_boundary, active_channel_name)
  unread_boundary = next_channel.unread_boundary
  // THE HIT LANDS ON THE CLICK, not when the window arrives: a hit that lives
  // in another room must not leave the room she left in the header and the
  // sidebar for the whole walk.
  active_channel = channel_id
  chat_land_seq = target_seq
  active_dm_peer = dm_peer_of_channel(active_dm_peer, dm_peers, active_channel)
  active_dm = dm_peer_named(dm_peers, active_dm_peer)
  active_channel_name = next_channel.name
  active_channel_archived = next_channel.archived
  active_channel_members_only = next_channel.members_only
  // A WINDOW AROUND ONE OLD MESSAGE IS HISTORY, and the read cursor stays off
  // a room she has not reached the tail of — the live fold refuses the mark
  // on exactly this flag.
  history_view = true
  chat_at_tail = false
  channel_members = []
  let post_gate_known = !active_channel_members_only
  post_refusal = keep_str(post_gate_known, post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key), "")
  palette_open = false
  invalidate lane=account_ceremony
  invalidate lane=account_desktop_ceremony
  account_busy = account_busy && empty(account_ceremony_phase)
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""
  shell_tab = ShellTab.chat
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = true
  error = ""
  chat_generation = chat_generation + 1
  // ONE READ, AND IT IS NOT THE TIMELINE'S. The window is the view's; this
  // asks the node for the room's record and its huddle roster, which are the
  // app's own facts (the call's media leg hangs on the roster).
  run replace lane=chat_load load_channel_window(connected_rpc, active_channel, chat_generation) -> chat_updated _ | chat_load_failed _

// THE LAST CLICK WINS. This used to open `return if loading`, and `loading` is
// true for the entire switch it starts — so the second click of a fast A→B→C
// was discarded with no sidebar move, no header change and no busy affordance
// anywhere, and the reader clicked again into the same void. The click is taken
// unconditionally now and the superseded room load is rejected by
// `chat_generation`.
//
// `mutation_phase` stays: it is a mutation lock, not a load, and
// `channel_created` lands the reader in the room it just made. The sidebar rows
// disable on exactly that term, so the guard and the affordance agree.
on choose_channel(id)
  return if mutation_phase != MutationPhase.idle
  active_dm_peer = ""
  active_dm = no_dm_peer()
  // "Jump to latest" IS this handler, aimed at the room already on screen — so
  // the window the banner describes ends here, and the landing with it.
  history_view = false
  chat_at_tail = true
  chat_land_seq = 0
  // FREEZE THE DIVIDER HERE, while the previous room is still `active_channel`
  // — the optimistic assignment below makes current == next by the time
  // `chat_updated` runs its own freeze, which then correctly keeps this value.
  let next_channel = channel_switch_facts(channel_reads, channels, active_channel, id, unread_boundary, active_channel_name)
  unread_boundary = next_channel.unread_boundary
  // The switch is visible NOW: the clicked room takes the header and sidebar
  // highlight, then paints an empty loading state until its window lands.
  active_channel = id
  active_channel_name = next_channel.name
  // BOTH GATE FACTS RIDE THE CLICK. `post_refusal` is the composer's
  // delivery-time verdict, and computing it from the room she LEFT is how a
  // public channel came up refusing her post for a whole round trip.
  active_channel_archived = next_channel.archived
  active_channel_members_only = next_channel.members_only
  channel_members = []
  let post_gate_known = !active_channel_members_only
  post_refusal = keep_str(post_gate_known, post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key), "")
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = true
  // THE COMPOSER DOES NOT APPEAR IN THIS HANDLER, and that is the point
  // (ducktape-ui#697): `#composer(active_channel)` keys one retained instance
  // per room, so the words she was writing next door stay next door and the
  // arriving room shows its own — no park, no restore, no ordering rule.
  error = ""
  chat_generation = chat_generation + 1
  // THE CHANNEL LIST GOES DOWN, IT DOES NOT COME BACK. `load_chat_data`
  // re-paged the whole list on every switch — a round trip in front of the
  // first row, for a list this handler is reading two statements above and the
  // live fold keeps current.
  run replace lane=chat_load load_channel_window(connected_rpc, active_channel, chat_generation) -> chat_updated _ | chat_load_failed _

// A DM is not a second message plane: it is the two-party members-only channel
// at `dm_channel_id(me, peer)`, resolved or created on the way in. Everything
// downstream of `chat_updated` is the ordinary channel path.
on choose_dm(peer_key)
  // Same last-click-wins rule as `choose_channel`, and the same reason.
  return if mutation_phase != MutationPhase.idle || empty(peer_key)
  invalidate lane=chat_load
  active_dm_peer = peer_key
  active_dm = dm_peer_named(dm_peers, active_dm_peer)
  // A DM IS A CHANNEL AND ITS ID IS DERIVABLE. `dm_channel_id` is the same
  // deterministic hash `open_dm` resolves on the node side, so the room can
  // land on the CLICK here exactly as it does in `choose_channel`. Leaving
  // `active_channel` on the room she left is how the peer's face came up beside
  // that room's "Archived" badge and its composer refusal for the several
  // blocks a DM open takes.
  let dm_room = dm_room_of_peer(dm_peers, active_dm_peer)
  // A DM open is a live tail, never a landing — see `history_view`.
  history_view = false
  chat_at_tail = true
  chat_land_seq = 0
  // FREEZE THE DIVIDER WHILE `active_channel` STILL NAMES THE ROOM SHE LEAVES,
  // for the reason `choose_channel` gives.
  let next_channel = channel_switch_facts(channel_reads, channels, active_channel, dm_room, unread_boundary, active_channel_name)
  unread_boundary = next_channel.unread_boundary
  active_channel = dm_room
  active_channel_name = next_channel.name
  active_channel_archived = next_channel.archived
  active_channel_members_only = next_channel.members_only
  channel_members = []
  post_refusal = ""
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = true
  // Same per-room composer as `choose_channel`: a DM is an ordinary channel,
  // so its composer instance keys under `dm_room` like any other.
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
  run every open_dm(connected_rpc, password, active_dm_peer, chat_generation) -> chat_updated _ | chat_load_failed _

on create_channel_submit
  return if loading || mutation_phase != MutationPhase.idle || empty(trim(channel_draft))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = MutationPhase.channel
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

// JOINING IS A SIGNED CHAIN WRITE, SO IT NEEDS AN INVERSE THE UI CAN REACH.
// `huddle_joined` is the discriminant that splits the header's "Huddle" start
// control from the LIVE pill carrying ✕ Leave, and it is answered from the
// channel's own roster, which is the chain's answer and never a local flag.
on join_huddle_submit
  return if loading || mutation_phase != MutationPhase.idle || empty(active_channel) || active_channel_archived
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = MutationPhase.huddle
  error = ""
  run every join_huddle(connected_rpc, password, active_channel) -> huddle_joined_ack _ | mutation_failed _

// THE JOIN OPENS THE CALL'S WINDOW. That window is the only surface a huddle
// has, so a join that opened none would be a call with nowhere to be seen.
// It goes through `show_huddle` rather than opening outright: if a window is
// somehow already up, a second open would leak the first as an untracked
// window, so the summon raises that one instead.
//
// AND THE ACK IS WHAT LANDS THE JOINED STATE. `huddle_after_load` stays the
// RECONCILER — it is what takes the huddle away again if the roster does not
// have you on it — and it keeps the stamp below, because it reads the
// `huddle_joined` this handler has already set.
on huddle_joined_ack(_result)
  mutation_phase = MutationPhase.idle
  error = ""
  huddle_joined = true
  huddle_channel = active_channel
  huddle_channel_name = active_channel_name
  // The clock starts where THIS process saw the join land — see the header of
  // handlers/huddle.ice for why it is never the roster row's `joined_at`.
  huddle_joined_at = huddle_now
  // The roster the tiles are drawn from, and the reconciler's own input.
  chat_generation = chat_generation + 1
  // A window task is terminal, so the summon runs beside the load rather than
  // after it.
  parallel
    flow
      from done true
      done -> show_huddle()
    run replace lane=chat_load load_channel_window(connected_rpc, active_channel, chat_generation) -> chat_updated _ | chat_load_failed _

// Leaving is `leave_huddle_here` in handlers/huddle.ice, which leaves the
// HUDDLE'S channel rather than the one on screen — the same button serves the
// channel-header ✕ and the popped panel, so a second leave that targets
// `active_channel` would be a way to leave the wrong huddle.

// AN EDIT OPENS THE HOST'S EDITOR ON THE VIEW'S OWN READING. The view holds
// the revisions, so it decides WHETHER a row may be edited and hands over the
// markdown it opens on; what it cannot do is type, because the editor is a
// host surface with an IME and a retained document. The seq and revision are
// remembered here so the submit that comes back can be checked against the
// row the menu was armed on.
on chat_begin_edit(scope, body, seq, rev)
  return if empty(body) || seq <= 0
  chat_edit_seq = seq
  chat_edit_rev = rev
  composer_seeded = chat_composer_seed(scope, body)

// THE COMPOSERS ARE HOST SURFACES (`crate::composer_surface`): the chat view
// leaves a slot per room, per thread and per edit, the app paints the editor
// there and keeps every box's words, and a submit arrives as the view's
// `composer` intent — kind, trimmed body, fresh operation id — routed here.
// Marks and the formatting chords are the surface's own; a refused or failed
// body goes back to its box through `chat_composer_unsent`.
//
// ONE EVENT, ONE HANDLER, ONE DISPATCH. Every composer fires the same intent,
// and the kind says which composer it was rather than the route naming one of
// four near-identical handlers.
on composer_submitted(kind, pending_body, pending_id, scope)
  match kind
    ComposerKind.message
      // THE GATE, RE-READ AT DELIVERY. The instance refused with the verdict
      // its frame drew (`blocked` rode the route); this is the fresh one, and
      // the two answers do DIFFERENT WORK — so it is a discriminant with an
      // arm each, not a bool read twice. A refused body cannot go back into
      // the box by itself (the composer cleared itself before emitting), so
      // the refusing arm hands it to that room's own plate (ducktape-ui#698).
      match submit_verdict(loading, connected, active_channel, post_refusal, true, scope, composer_scope(connected_rpc, active_channel))
        SubmitVerdict.refused
          composer_stashed = chat_composer_unsent(scope, pending_body, false)
        SubmitVerdict.admitted
          hydration_generation = hydration_generation + 1
          hydration_retry_attempt = 0
          // THE ROW IS ON SCREEN BEFORE THE BLOCK IS. The timeline is the
          // view's own reading of the index, which cannot know about an
          // operation the node has not committed — so the admitted send is
          // held here and the view paints it at the tail of the room.
          chat_pending_sends = send_pending(chat_pending_sends, pending_id, pending_body, 0)
          error = ""
          // Sending is a jump to now: the minted row lands at the tail, and a
          // reader who had scrolled up would otherwise get her own send below
          // the fold. The serial is the view's cue to snap.
          chat_at_tail = true
          history_view = false
          chat_sent_serial = chat_sent_serial + 1
          run every send_message(connected_rpc, password, active_channel, pending_id, pending_body) -> message_sent _ | message_send_failed _
    ComposerKind.reply
      // THE RAIL TWIN, AND ITS SCOPE IS ITS THREAD. The rail is the view's, so
      // the thread a reply belongs to is read back off the box it was written
      // in — which is also what proves the box belongs to the room on screen.
      let thread_seq = scope_thread_seq(scope)
      // NOTHING BUSIES THE RAIL FROM HERE. The rail's readiness is the VIEW's
      // `thread_loading`, not this plane's `loading` — the app's flag is the
      // workspace hydration one, which a pages load raises behind a cross-tab
      // bounce while the rail sits there with a fully lit Send. Refusing on a
      // term the mount does not wear is a dead control that eats a reply.
      match submit_verdict(false, connected, active_channel, post_refusal, thread_seq > 0, scope, thread_scope(connected_rpc, active_channel, thread_seq))
        SubmitVerdict.refused
          composer_stashed = chat_composer_unsent(scope, pending_body, false)
        SubmitVerdict.admitted
          hydration_generation = hydration_generation + 1
          hydration_retry_attempt = 0
          chat_pending_sends = send_pending(chat_pending_sends, pending_id, pending_body, thread_seq)
          error = ""
          run every send_reply(connected_rpc, password, active_channel, thread_seq, pending_id, pending_body) -> thread_reply_sent _ | thread_reply_send_failed _
    // BOTH EDIT BOXES SAVE THE SAME WAY — the row is named by the scope the
    // menu armed, and the stream's row and the rail's are one row.
    ComposerKind.edit
      return if scope != edit_scope(connected_rpc, active_channel, chat_edit_seq)
      flow
        from done trim(pending_body)
        done -> edit_message_submit _
    ComposerKind.thread_edit
      return if scope != edit_scope(connected_rpc, active_channel, chat_edit_seq)
      flow
        from done trim(pending_body)
        done -> edit_message_submit _

on edit_message_submit(text)
  return if loading || mutation_phase != MutationPhase.idle || empty(active_channel) || chat_edit_seq <= 0 || empty(trim(text))
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = MutationPhase.message_edit
  error = ""
  run every edit_message(connected_rpc, password, active_channel, chat_edit_seq, chat_edit_rev, trim(text)) -> chat_acked _ | mutation_failed _

on message_sent(next)
  chat_pending_sends = send_settled(chat_pending_sends, next.operation_id)
  return if active_channel != next.channel_id
  error = ""

// A FAILED SEND IS A FACT ABOUT THE USER'S TEXT, NOT ABOUT THE PANE ON SCREEN.
// The reader can leave the room while the write is in flight — a channel pick,
// a search hit, or a reconnect that lands on `channels.first()` — and the whole
// handler used to return on that, so the body, the error and every trace of it
// went away while the last thing she saw was her message in the timeline.
//
// So the room check scopes the RESYNC only. The unsent stash, the error banner
// and the pending row's retirement are written first, unconditionally.
on message_send_failed(cause)
  error = cause.message
  chat_pending_sends = send_failed(chat_pending_sends, cause.operation_id, cause.committed)
  // THE ROOM IT WAS WRITTEN IN GETS ITS WORDS BACK — not whatever room she
  // has moved to since. The plate is the composer instance's own state now
  // (ducktape-ui#698) and `cause.scope_id` names the room the send was for,
  // so the failure reaches exactly that box.
  composer_stashed = chat_composer_unsent(composer_scope(connected_rpc, cause.scope_id), cause.body, cause.committed)
  return if active_channel != cause.scope_id || !cause.committed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run replace lane=live_resync live_resync_load(connected_rpc, active_channel, active_page, "chat", false, hydration_generation, pages_fold_serial, 0) -> live_resynced _ | live_resync_failed _

// Same rule as `message_send_failed`: a reply belongs to its thread, and
// `cause.thread_seq` is the only thing that can name the box it came from
// once the rail has moved on.
on thread_reply_send_failed(cause)
  error = cause.message
  chat_pending_sends = send_failed(chat_pending_sends, cause.operation_id, cause.committed)
  composer_stashed = chat_composer_unsent(thread_scope(connected_rpc, cause.scope_id, cause.thread_seq), cause.body, cause.committed)
  return if active_channel != cause.scope_id || !cause.committed
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run replace lane=live_resync live_resync_load(connected_rpc, active_channel, active_page, "chat", false, hydration_generation, pages_fold_serial, 0) -> live_resynced _ | live_resync_failed _

on thread_reply_sent(next)
  chat_pending_sends = send_settled(chat_pending_sends, next.operation_id)
  return if active_channel != next.channel_id
  error = ""

on chat_updated(next)
  // THE ROOM SHE IS IN NOW, OR NOTHING. Two clicks in flight land in the order
  // the node answers, not the order she clicked, so without this the FIRST
  // reply won and A→B→C settled on B. `loading` is deliberately NOT released
  // here: the load this one lost to is still running, and clearing it would
  // swap the loading plate for an empty room mid-switch.
  return if next.generation != chat_generation
  // FOLD, DO NOT REPLACE. `load_channel_window` answers with the one row it
  // refreshed, and the list it was handed is the PRE-CLICK snapshot: assigning
  // it back reverted every delta `live_updated` folded during the round trip.
  channels = upsert_channel_rows(channels, next.channels)
  unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, next.active_channel, unread_boundary)
  channel_reads = mark_channel_read(channel_reads, next.active_channel, channel_head_seq(channels, next.active_channel))
  rooms = chat_sidebar_rooms(channels, dm_peers, channel_reads)
  dm_rows = chat_sidebar_dms(channels, dm_peers, channel_reads)
  // A LANDING THAT MOVED THE ROOM UNDER HER IS A LIVE TAIL. `load_channel_window`
  // answers a room this node cannot see with the landing channel instead, so the
  // window a search hit parked belongs to the room that was ASKED for — not to
  // the one that came back — and carrying it across would open the new room
  // somewhere in its middle with no way back but a re-click.
  let landed_elsewhere = active_channel != next.active_channel
  history_view = history_view && !landed_elsewhere
  chat_land_seq = keep_i64(landed_elsewhere, 0, chat_land_seq)
  active_channel = next.active_channel
  // A LANDING ANSWERS FOR THE PEER TOO. The DM header suppresses the `#` and
  // the channel name, so a peer that outlives the room it named leaves the room
  // on screen unnamed under someone else's face — see `dm_peer_of_channel`.
  active_dm_peer = dm_peer_of_channel(active_dm_peer, dm_peers, active_channel)
  active_dm = dm_peer_named(dm_peers, active_dm_peer)
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  // Am I in it — see `join_huddle_submit` above. Stamp first: it reads the
  // PREVIOUS `huddle_joined`, so a refresh that finds her still in keeps the
  // clock and one that finds her out re-takes it for the next join. The load
  // answers for the huddle only when it loaded the huddle's OWN channel —
  // clicking a second room does not end the call she is in, and the call's
  // media leg is subscribed on this very flag (`huddle_after_load`).
  huddle_joined_at = keep_i64(huddle_joined, huddle_joined_at, huddle_now)
  let huddle = huddle_after_load(true, huddle_joined, huddle_channel, huddle_channel_name, huddle_roster, active_channel, active_channel_name, next.huddle_roster)
  huddle_joined = huddle.joined
  huddle_roster = huddle.roster
  huddle_rows = huddle_tile_rows(huddle_roster, call_peers, call_muted)
  huddle_channel = huddle.channel
  huddle_channel_name = huddle.channel_name
  channel_members = next.channel_members
  composer_roster_set = chat_composer_roster(composer_scope(connected_rpc, active_channel), channel_members)
  post_refusal = post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key)
  loading = false
  error = ""
  // The huddle ended under this fold (or the roster she is on is another
  // channel's): the window mirrors the old popped-card gate, which also
  // vanished the moment `huddle_joined` dropped. A no-op while still joined.
  task window close target=window_target_unless(huddle_joined, huddle_win)

// A SWITCH'S FAILURE BELONGS TO THAT SWITCH. The generic `failed` arm is not
// guarded — it could not be, it serves the page routes too — and while the room
// pickers were serialized by `return if loading` there was at most one chat load
// in flight, so it never had to be. That invariant is gone: the last click wins
// and the ones it passed are still out.
on chat_load_failed(cause)
  return if cause.generation != chat_generation
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = false
  error = cause.message

on channel_created(next)
  // The lock and the modal come down whether or not this landing still counts:
  // a create she has since clicked away from must not leave the sidebar's
  // buttons dead.
  pending_channel = ""
  channel_create_open = false
  channel_create_members_only = false
  mutation_phase = MutationPhase.idle
  // Superseded by a later switch — see `chat_updated`. The mutation still
  // committed; the live stream owns whatever room the reader chose instead.
  return if next.generation != chat_generation
  // A brand-new channel's latest page IS the whole channel, and a create
  // lands you IN it — a navigation, so the landing ends here.
  history_view = false
  chat_at_tail = true
  chat_land_seq = 0
  channels = upsert_channel_rows(channels, next.channels)
  unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, next.active_channel, unread_boundary)
  channel_reads = mark_channel_read(channel_reads, next.active_channel, channel_head_seq(channels, next.active_channel))
  rooms = chat_sidebar_rooms(channels, dm_peers, channel_reads)
  dm_rows = chat_sidebar_dms(channels, dm_peers, channel_reads)
  // A CREATE IS A ROOM SWITCH — the line below lands her IN the new channel,
  // and the composer she was typing into stays keyed to the room she left
  // (ducktape-ui#697).
  active_channel = next.active_channel
  // Creating lands you in the new room, which is nobody's DM.
  active_dm_peer = dm_peer_of_channel(active_dm_peer, dm_peers, active_channel)
  active_dm = dm_peer_named(dm_peers, active_dm_peer)
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  // Am I in it — see `chat_updated`, which spells the same reconciliation.
  huddle_joined_at = keep_i64(huddle_joined, huddle_joined_at, huddle_now)
  let huddle = huddle_after_load(true, huddle_joined, huddle_channel, huddle_channel_name, huddle_roster, active_channel, active_channel_name, next.huddle_roster)
  huddle_joined = huddle.joined
  huddle_roster = huddle.roster
  huddle_rows = huddle_tile_rows(huddle_roster, call_peers, call_muted)
  huddle_channel = huddle.channel
  huddle_channel_name = huddle.channel_name
  channel_members = next.channel_members
  composer_roster_set = chat_composer_roster(composer_scope(connected_rpc, active_channel), channel_members)
  post_refusal = post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key)
  error = ""
  // Same close-if-ended mirror as `chat_updated` above.
  task window close target=window_target_unless(huddle_joined, huddle_win)

// EVERY PENDING RUN THIS NODE HOLDS, not this room's. Which of them reach the
// screen is decided once, in the chat seat, against the room on screen when
// the frame is built — so no handler that moves `active_channel` owes this
// lane anything, and none of them can forget.
//
// THE CONNECTION IS THE ONE THING THE FOLD STILL HAS TO ASK. Room ids are not
// unique across networks, so `general` on the connection she left would
// otherwise have drawn its runs under `general` on the one she is on.
//
// REFUSED, NOT ASSIGNED. The guard is a `return`, like every other generation
// guard in this file, because a stale reading's emptiness is not a fact about
// the connection she IS on.
on live_agents_event(next)
  return if live_agents_stale(next, connected_rpc, network_chain_id, connect_generation, signer_key)
  live_agents = next.rows

on live_cancel_acked(_ok)
  error = ""

on chat_acked(_result)
  chat_edit_seq = 0
  chat_edit_rev = 0
  pending_channel = ""
  channel_create_open = false
  mutation_phase = MutationPhase.idle
  error = ""

// COPY LINK IS A CLIPBOARD WRITE AND NOTHING ELSE — the menu it was pressed
// in is the view's, and it closes itself. The clipboard stays the app's single
// site (`handlers/node.ice`), reached the one way a handler reaches another.
on copy_message_link(link)
  return if empty(link)
  run every duck_echo_str(link) -> copy_to_clipboard(_, "Message link copied") | external_url_failed _

// A LINK PRESS IS A HAND-OFF TO THE OS, and nothing else: no selection, no
// draft, no rail. Same route the page renderer's link press takes
// (`handlers/pages.ice`), and it shares that handler's two result arms.
// THE duck:// OPEN PLANE. A clicked link goes through `resolve_duck_link`
// ONCE — the protocol's module table plus the network scope its grammar
// cannot check alone — and each kind maps onto navigation the app ALREADY
// has: the handler a click on the screen itself would reach, handed the
// link's field through an echo lane (the one way a handler reaches another).
// AN ADDRESS IS PUSHED, NOT NAVIGATED: the browsers live in the files and
// forge views, so the app moves the tab and hands the view the address as a
// session fact (`fs_route` / `forge_link`, each with a serial so the same
// address twice lands twice). The protocol adds addresses, never navigation.
// A LINK NAMES ITS NETWORK: one whose `?net=` digest is another network's
// addresses a store this app is not connected to, so it opens nothing and
// says which network it belongs to. A link with no `?net=` is the hand-typed
// case and resolves against the connected network as written.
on open_message_link(url)
  return if empty(url)
  let link = resolve_duck_link(url, network_chain_id)
  match link.kind
    DuckKind.unknown
      error = "this link names nothing the app can open"
    DuckKind.foreign_network
      error = foreign_network_error(link.net, network_chain_id)
    DuckKind.web
      run every open_external_url(url) -> external_url_opened _ | external_url_failed _
    DuckKind.page
      run every duck_echo_str(link.page) -> open_page_search_hit(_, link.block) | external_url_failed _
    DuckKind.run
      flow
        from done link.dispatch
        done -> open_run_panel _
    // The Files tab opens, and the PATH goes with it — as a SESSION fact,
    // not a navigation the app performs: the browser is the view's, so the
    // address the shell resolved is pushed in and the view lands on it.
    DuckKind.files
      fs_route = link.path
      fs_route_serial = fs_route_serial + 1
      invalidate lane=account_ceremony
      invalidate lane=account_desktop_ceremony
      account_busy = account_busy && empty(account_ceremony_phase)
      account_ceremony_phase = ""
      account_ceremony_qr = ""
      account_ceremony_detail = ""
      account_ceremony_left = ""
      shell_tab = ShellTab.files
    // THE FORGE ADDRESSES CROSS AS THE URL ITSELF. The forge view is a guest:
    // it holds every forge selection and parses the `duck://forge/...`
    // grammar out of the props it is handed. The host's whole job is to raise
    // the tab and hand the link over — the tick is what makes two clicks on
    // the same link two openings.
    DuckKind.forge_repo
      forge_link = url
      forge_link_tick = forge_link_tick + 1
      invalidate lane=account_ceremony
      invalidate lane=account_desktop_ceremony
      account_busy = account_busy && empty(account_ceremony_phase)
      account_ceremony_phase = ""
      account_ceremony_qr = ""
      account_ceremony_detail = ""
      account_ceremony_left = ""
      shell_tab = ShellTab.forge
    DuckKind.forge_item
      forge_link = url
      forge_link_tick = forge_link_tick + 1
      invalidate lane=account_ceremony
      invalidate lane=account_desktop_ceremony
      account_busy = account_busy && empty(account_ceremony_phase)
      account_ceremony_phase = ""
      account_ceremony_qr = ""
      account_ceremony_detail = ""
      account_ceremony_left = ""
      shell_tab = ShellTab.forge
    DuckKind.forge_blob
      forge_link = url
      forge_link_tick = forge_link_tick + 1
      invalidate lane=account_ceremony
      invalidate lane=account_desktop_ceremony
      account_busy = account_busy && empty(account_ceremony_phase)
      account_ceremony_phase = ""
      account_ceremony_qr = ""
      account_ceremony_detail = ""
      account_ceremony_left = ""
      shell_tab = ShellTab.forge
    DuckKind.channel
      invalidate lane=account_ceremony
      invalidate lane=account_desktop_ceremony
      account_busy = account_busy && empty(account_ceremony_phase)
      account_ceremony_phase = ""
      account_ceremony_qr = ""
      account_ceremony_detail = ""
      account_ceremony_left = ""
      shell_tab = ShellTab.chat
      run every duck_echo_str(link.channel) -> choose_channel _ | external_url_failed _
    DuckKind.channel_message
      run every duck_echo_str(link.channel) -> open_chat_search_hit(_, link.seq) | external_url_failed _
    // A MENTION OPENS THE DM. `duck://account/<n>` is what a mention plate
    // links to; the DM peer list is keyed by account number, so the address
    // is the peer key `choose_dm` takes. An account with no peer row — the
    // reader's own — lands on nothing, exactly as a click on it did before.
    DuckKind.account
      invalidate lane=account_ceremony
      invalidate lane=account_desktop_ceremony
      account_busy = account_busy && empty(account_ceremony_phase)
      account_ceremony_phase = ""
      account_ceremony_qr = ""
      account_ceremony_detail = ""
      account_ceremony_left = ""
      shell_tab = ShellTab.chat
      run every duck_echo_str(link.account) -> choose_dm _ | external_url_failed _

// IS THE READER LOOKING AT NOW? The view pages its own scrollback, so the one
// thing the app still takes off the offset is whether she has reached the
// tail: the live fold refuses to move a room's read cursor while she has not
// (`history_view`), or a badge would clear on a message she never saw.
on chat_scrolled(_absolute_x, _absolute_y, _relative_x, relative_y)
  chat_at_tail = near_scroll_tail(relative_y)
  history_view = !chat_at_tail || chat_land_seq > 0

// ⌘C OVER THE CHAT TAB. The rows a range covers and the text they lift are
// the VIEW's reading — it holds the timeline — so the chord is a serial the
// view answers, not a clipboard write made here. The keyboard is the app's
// door, which is why the chord is read at all.
on copy_chord_pressed(event)
  return if !is_copy_chord(event.key, event.physical_key, event.modifiers)
  return if shell_tab != ShellTab.chat
  chat_copy_chord_serial = chat_copy_chord_serial + 1

// ============================================================================
// THE VIEW'S INTENTS. Every act the app still owns comes back here as one
// intent carrying what the reader chose or typed, and each arm reaches the
// handler that always signed it — Ice has no handler-to-handler call, but a
// flow can route to one, and a handler that takes several values is reached
// the way `open_message_link` reaches one: through an echo lane, its other
// values read first. The composers are host surfaces, so a submit arrives as
// `composer` with the kind, the body and the operation id the surface minted.
// ============================================================================
on chat_view_event(event)
  match chat_intent(event)
    ChatIntent.open_hit
      let target_seq = event_int(event, "target_seq")
      run every duck_echo_str(event_text(event, "channel")) -> open_chat_search_hit(_, target_seq) | external_url_failed _
    ChatIntent.toggle_create
      flow
        from done true
        done -> toggle_channel_create()
    ChatIntent.choose_channel
      flow
        from done event_text(event, "id")
        done -> choose_channel _
    ChatIntent.choose_dm
      flow
        from done event_text(event, "key")
        done -> choose_dm _
    ChatIntent.show_huddle
      flow
        from done true
        done -> show_huddle()
    ChatIntent.leave_huddle
      flow
        from done true
        done -> leave_huddle_here()
    ChatIntent.join_huddle
      flow
        from done true
        done -> join_huddle_submit()
    ChatIntent.scrolled
      let absolute_y = event_num(event, "absolute_y")
      let relative_x = event_num(event, "relative_x")
      let relative_y = event_num(event, "relative_y")
      run every duck_echo_f64(event_num(event, "absolute_x")) -> chat_scrolled(_, absolute_y, relative_x, relative_y) | external_url_failed _
    ChatIntent.open_link
      flow
        from done event_text(event, "url")
        done -> open_message_link _
    ChatIntent.copy
      let label = event_text(event, "label")
      run every duck_echo_str(event_text(event, "text")) -> copy_to_clipboard(_, label) | external_url_failed _
    ChatIntent.copy_link
      flow
        from done event_text(event, "link")
        done -> copy_message_link _
    ChatIntent.begin_edit
      let body = event_text(event, "body")
      let seq = event_int(event, "seq")
      let rev = event_int(event, "rev")
      run every duck_echo_str(event_text(event, "scope")) -> chat_begin_edit(_, body, seq, rev) | external_url_failed _
    ChatIntent.cancel_run
      run every cancel_agent_run(connected_rpc, password, event_text(event, "run_id")) -> live_cancel_acked _ | mutation_failed _
    // "VIEW RUN" ON A LIVE HINT, or the run chip on a message a run posted:
    // the run panel, on that run.
    ChatIntent.open_run
      flow
        from done event_text(event, "dispatch_id")
        done -> open_run_panel _
    ChatIntent.composer
      let kind = chat_event_kind(event)
      let id = event_text(event, "id")
      let scope = event_text(event, "scope")
      run every duck_echo_str(event_text(event, "body")) -> composer_submitted(kind, _, id, scope) | external_url_failed _
