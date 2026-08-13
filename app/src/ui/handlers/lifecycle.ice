// EVERY BOOT OPENS THE LAUNCH WINDOW — sign in, pick a network, and only
// then does a console window exist. `onboarding_opened` (handlers/
// onboarding.ice) loads the hub state and appearance once the window is up;
// `connect` runs only after a pick, from `console_opened`.
on mount
  task window open onboarding -> onboarding_opened _

// The persisted reading, applied on boot. The light block runs first and the
// dark block reverses it — the handler grammar has no branches, so the
// terminal-case return IS the branch.
on appearance_loaded(mode)
  return if empty(mode)
  appearance = mode
  app_palette = AppTheme.app
  app_background = "#fdfdfb"
  app_text = "#2c2b27"
  return if appearance != "dark"
  app_palette = AppTheme.app_dark
  app_background = "#1b1a16"
  app_text = "#e8e6df"

// The Settings toggle: pin a reading and persist it — one event per button,
// so each handler is linear and its persistence run sits last (E141).
// `save_appearance` is fire-and-forget: a failed write costs the NEXT boot's
// default, nothing this session shows.
on set_appearance_light
  appearance = "light"
  app_palette = AppTheme.app
  app_background = "#fdfdfb"
  app_text = "#2c2b27"
  run replace lane=appearance_save save_appearance(appearance) -> appearance_saved _

on set_appearance_dark
  appearance = "dark"
  app_palette = AppTheme.app_dark
  app_background = "#1b1a16"
  app_text = "#e8e6df"
  run replace lane=appearance_save save_appearance(appearance) -> appearance_saved _

on appearance_saved(_written)
  error = error

// A SAME-ENDPOINT retry: the launch window's picker owns which network, so
// reconnect no longer changes endpoints — the per-endpoint draft retention
// that lived here collapsed to identity calls and is gone. Typed drafts
// deliberately survive (the park below + orphan lines); the network
// lists are re-fetched.
on reconnect
  return if loading || (mutation_phase != MutationPhase.idle && mutation_phase != MutationPhase.recovering)
  invalidate lane=chat_search
  invalidate lane=page_search
  invalidate lane=palette_search
  invalidate lane=workspace_search
  invalidate lane=chat_load
  invalidate lane=page_load
  invalidate lane=history
  invalidate lane=thread
  invalidate lane=live_thread
  invalidate lane=block_threads
  invalidate lane=block_comments
  invalidate lane=live_resync
  invalidate lane=forge_code
  invalidate lane=files_preview
  invalidate lane=page_autosave
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], active_page, block_comment_draft)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = MutationPhase.idle
  loading = true
  connected = false
  channels = []
  rooms = []
  dm_rows = []
  messages = []
  has_older_history = false
  // The history lane was invalidated above, so its old socket and button state
  // end together even when that socket would never have answered.
  history_loading = false
  // And the window load with it: same dropped socket, and the reply that would
  // lower this term may never come — see `chat_window_loading`.
  chat_window_loading = false
  channel_reads = []
  unread_boundary = 0
  // A RECONNECT IS A ROOM SWITCH SPREAD OVER TWO HANDLERS, so it parks like the
  // four pickers do — here, while `active_channel` and `active_thread_seq` still
  // name the room and thread she is in — and `workspace_connected` restores
  // below its own landing write. The line below blanks the key, so a park under
  // it would file both composers under "" and drop them. Without this the live
  // composer rode the reconnect into `landing_channel(channels)` — the first
  // room with traffic, not the one she left — armed to send there, and the
  // next pick parked her words under THAT room's id. The park reads the live
  // editor directly: the `message_draft` harvest that used to sit above it was
  // this park's predecessor, and its leftover stash is what `live_resynced`'s
  // failed-draft plate later offered to a room she never typed it in.
  message_drafts = park_message_draft(message_drafts, active_channel, trim(editor_text(message_editor)))
  reply_drafts = park_reply_draft(reply_drafts, active_channel, active_thread_seq, trim(editor_text(reply_editor)))
  active_channel = ""
  // The room is gone, so its two readings go with it: no peer names an empty
  // channel, and no window survives a reconnect.
  active_dm_peer = ""
  active_dm = no_dm_peer()
  history_view = false
  active_channel_name = ""
  active_channel_archived = false
  active_channel_members_only = false
  active_channel_huddle_count = 0
  channel_members = []
  post_refusal = ""
  channel_settings_open = false
  channel_name_draft = ""
  member_key_draft = ""
  selected_message_seq = 0
  selected_message_rev = 0
  message_action = MessageAction.toolbar
  message_edit_draft = ""
  active_thread_seq = 0
  thread_target_seq = 0
  thread_messages = []
  thread_next_reply_offset = 0
  thread_has_more = false
  thread_generation = thread_generation + 1
  invalidate lane=live_thread
  thread_loading = false
  reply_draft = ""
  // THE RAIL CLOSES AND ITS WORDS DO NOT GO WITH IT. This blank used to be the
  // one composer a reconnect still ate, two dozen lines below a handler that
  // goes out of its way to carry the stream's — the park above holds it under
  // `channel#seq`, and the next `open_thread_for` on that thread hands it back.
  reply_editor = editor("")
  pending_reply = ""
  // Both composers above are new empty boxes and the rail is gone. `connected`
  // goes false here, which only MUTES the chord — the stale claim would ride
  // straight through the reconnect and mark a rebuilt draft on the first
  // Cmd+B after it lands.
  composer_focus = ComposerFocus.unfocused
  pending_channel = ""
  pending_message = ""
  chat_search_hits = []
  chat_search_phase = SearchPhase.idle
  pages = []
  doc_tabs = []
  blocks = []
  active_page = ""
  active_page_title = ""
  active_page_parent = ""
  pending_page = ""
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_rows = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  active_thread_target = ""
  active_thread_anchor = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  page_editor = editor("")
  page_saved_text = ""
  buffer_page = ""
  page_refusal = ""
  block_autosave_status = AutosaveStatus.idle
  page_delete_armed = false
  page_search_hits = []
  page_searching = false
  error = ""
  status = "Connecting…"
  connect_generation = connect_generation + 1
  run replace lane=connect connect(connected_rpc, hydration_retry_attempt, connect_generation) -> workspace_connected _ | connect_failed _

on workspace_connected(next)
  // A connect answering for an endpoint you have since left is not an answer.
  return if next.generation != connect_generation
  rpc = next.rpc
  connected_rpc = next.rpc
  network_name = network_label(account_name, connected_rpc)
  status = next.status
  block_height = next.height
  channels = next.channels
  channel_reads = initial_channel_reads(next.channels, channel_reads)
  rooms = chat_sidebar_rooms(channels, dm_peers, settings_user_key, channel_reads)
  dm_rows = chat_sidebar_dms(channels, dm_peers, channel_reads)
  unread_boundary = 0
  // A connect answers with the LATEST page of whatever room it landed on, so
  // whatever window a search hit had put on screen is gone — see
  // `chat_hit_loaded`.
  history_view = false
  messages = merge_pending_messages(next.messages, messages, active_channel, next.active_channel, "")
  has_older_history = history_has_older(messages)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  active_channel = next.active_channel
  // AND THE LANDING ROOM'S COMPOSER IS THE LANDING ROOM'S — the restore half of
  // the park `reconnect` files. The room above is `landing_channel(channels)`,
  // the first room with traffic and rarely the one she left, so without this
  // her half-typed line stood over a stranger's Send. `message_drafts` survives
  // the reconnect (same endpoint, no store reset), so this hands back whatever
  // THIS room was left holding, and nothing else.
  message_editor = editor(parked_message_draft(message_drafts, active_channel))
  // A reconnect lands on `channels.first()`, which is nobody's DM unless the
  // derivation says so — see `dm_peer_of_channel`.
  active_dm_peer = dm_peer_of_channel(active_dm_peer, settings_user_key, active_channel)
  active_dm = dm_peer_named(dm_peers, active_dm_peer)
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  active_channel_huddle_count = next.active_channel_huddle_count
  // Am I in the active channel's huddle — same stanza as `chat_updated`. A
  // process that reconnects while already on the roster renders LIVE here;
  // without this the pill only appeared after a manual channel re-pick.
  huddle_joined_at = keep_i64(huddle_joined, huddle_joined_at, huddle_now)
  huddle_joined = huddle_self(next.huddle_roster)
  huddle_roster = keep_roster(huddle_joined, next.huddle_roster)
  huddle_rows = huddle_tile_rows(huddle_roster, call_peers, call_muted)
  huddle_channel = keep_str(huddle_joined, active_channel, "")
  huddle_channel_name = keep_str(huddle_joined, active_channel_name, "")
  channel_members = next.channel_members
  post_refusal = post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key)
  pages = next.pages
  blocks = merge_pending_blocks(next.blocks, blocks, buffer_page, next.active_page, "")
  active_page = next.active_page
  block_comment_rows = page_comment_thread_rows(blocks, block_comment_threads, active_page)
  active_thread_anchor = comment_anchor_label(blocks, active_thread_target, active_page)
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  // The blocks in hand are this page's, and every route into a connect blanks
  // the buffer first — so this is the page the document state belongs to.
  buffer_page = next.active_page
  connected = true
  loading = false
  mutation_phase = MutationPhase.idle
  hydration_retry_attempt = 0
  error = ""
  explorer_generation = explorer_generation + 1
  fs_generation = fs_generation + 1
  members_generation = members_generation + 1
  gov_generation = gov_generation + 1
  agents_generation = agents_generation + 1
  account_generation = account_generation + 1
  forge_generation = forge_generation + 1
  forge_list_phase = keep_forge_phase(shell_tab == ShellTab.forge, ForgePhase.loading, forge_list_phase)
  settings_generation = settings_generation + 1
  node_peers_generation = node_peers_generation + 1
  dm_peers_generation = dm_peers_generation + 1
  // Shell may have stayed selected while the endpoint reconnected. Refresh
  // its device-local credential names here; tab selection alone will not fire
  // again after the console comes back online.
  shell_credentials_generation = shell_credentials_generation + 1
  shell_credentials_loading = shell_tab == ShellTab.shell
  parallel
    run replace lane=doc_tabs_load load_doc_tabs(connected_rpc) -> doc_tabs_loaded _
    run replace lane=dm_peers_load load_dm_peers(connected_rpc, dm_peers_generation) -> dm_peers_loaded _ | dm_peers_failed _
    run replace lane=node_facts_load load_node_facts(connected_rpc) -> node_facts_loaded _ | node_facts_failed _
    run replace lane=bell_load load_bell(connected_rpc) -> bell_loaded _ | bell_failed _
    run replace lane=explorer_load load_explorer(connected_rpc, explorer_generation) -> explorer_loaded _ | explorer_failed _
    run replace lane=files_list files_ls(connected_rpc, fs_path, fs_generation) -> fs_listed _ | fs_failed _
    run replace lane=members_load load_members(connected_rpc, members_generation) -> members_loaded _ | members_failed _
    run replace lane=governance_load load_governance(connected_rpc, gov_generation) -> governance_loaded _ | governance_failed _
    run replace lane=settings_load load_settings_facts(connected_rpc, settings_generation) -> settings_loaded _ | settings_failed _
    flow
      from done load_request(shell_tab == ShellTab.settings && node_tab == NodeTab.overview, connected_rpc, "", node_peers_generation)
      try request -> done request
      done -> peers_load_selected _
    run replace lane=agents_load load_agents(connected_rpc, agents_generation) -> agents_loaded _ | agents_failed _
    run replace lane=account_load load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _
    run replace lane=forge_load load_forge(connected_rpc, forge_generation) -> forge_loaded _ | forge_list_failed _
    flow
      from done load_request(shell_tab == ShellTab.shell, connected_rpc, "", shell_credentials_generation)
      try request -> done request
      done -> shell_credentials_load_selected _
    // The huddle window mirrors the old popped-card gate: it closes the
    // moment a fold finds `huddle_joined` false. A no-op while still joined.
    task window close target=window_target_unless(huddle_joined, huddle_win)

on live_updated(next)
  status = next.status
  // THE HEAD IS ASSIGNED ABOVE THE GUARD BECAUSE THE TIP STOPS AT IT.
  //
  // A tip carries a height and an EMPTY delta, and it arrives once per block —
  // on `bin/node` that is 1 Hz of nop fillers plus the 3s idle beat, forever,
  // on a chain where nothing happened. Every `apply_*` below takes its list BY
  // VALUE (`extern/backend.ice`), so letting a tip walk down to the `return if`
  // at the bottom clones `messages`, `channels` and `thread_messages` a dozen
  // times over to fold a delta that is empty by construction — then rebuilds
  // the view. Per second. Idle.
  //
  return if next.kind == "retry"
  block_height = keep_i64(next.height >= 0, next.height, block_height)
  return if next.kind == "tip"
  channels = apply_chat_channels(channels, next.chat)
  // Settle-✓ choreography, read against the PRE-fold rows: the settle delta
  // pops the tick (true), any later live event — the next block at the
  // latest — starts its fade (false). Same-value writes are no-ops.
  //
  // ONE CALL FOR THREE ANSWERS, because the ABI charges by the argument: the
  // four scans this replaced took `messages` and `thread_messages` by value
  // twice each, so a busy channel deep-cloned the timeline and the open rail
  // twice per incoming message before a single row was folded.
  live_settle = chat_settle(messages, thread_messages, next.chat, active_channel, send_flash_id, thread_send_flash_id)
  send_flash = live_settle.flashed
  send_flash_id = live_settle.send_id
  thread_send_flash_id = live_settle.reply_id
  // A HISTORY WINDOW IS A SNAPSHOT, NOT A LIVE TAIL. The rows in hand are a
  // window around one old message, so a new post's seq is past every one of
  // them and `insert_committed_root` puts it at the END of the window — a
  // message from today drawn directly under one from six months ago, folded
  // into the same author run, with no gap marker and nothing in "Load older"
  // that would say the middle is missing. Marking the channel read from that
  // window is the same lie in the sidebar: the reader is not caught up.
  //
  // Naming the channel is what gates both — the folds are already inert for a
  // delta belonging to another room, so a window that belongs to no room takes
  // no live rows and no read cursor. "Jump to latest" is the way back to the
  // tail, and it reloads canonically.
  //
  // HER OWN SETTLE IS THE ONE DELTA THE WINDOW STILL TAKES. The composer posts
  // from a history window too, and its optimistic row is spliced in there
  // regardless (`chat_composer_event`) — so refusing that row's settle strands
  // it `pending: true` forever, under a ✓ that `send_flash` pops on the very
  // delta the fold just dropped. `send_flash` IS that answer, already computed
  // one line above: no second pass over the timeline by value. A settling
  // delta carries exactly the one row it settles, so nothing else rides in.
  //
  // Everything else the window misses — edits, deletes, reactions and
  // reply-count bumps on rows that ARE on screen — waits for "Jump to latest",
  // which the banner is offering one click away.
  //
  // The READ CURSOR takes the strict gate either way: posting into a room you
  // are reading backwards is not being caught up on it.
  //
  // AND NOBODY READS A PANE THAT IS NOT MOUNTED. The live feed is subscribed on
  // `connected`, not on the tab, so an arrival while the reader is in Settings
  // or Files used to mark the last-opened room read on the spot: she came back
  // to no divider, and every OTHER room badged while that one stayed dark. The
  // rows still fold in (`live_fold_channel` below is untouched) — only the
  // cursor waits, and `select_shell_tab` moves it when she actually returns.
  let live_tail_channel = keep_str(!history_view && shell_tab == ShellTab.chat, active_channel, "")
  let live_fold_channel = keep_str(!history_view || animation.value(send_flash), active_channel, "")
  messages = apply_chat_messages(messages, next.chat, live_fold_channel)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  thread_messages = apply_chat_thread(thread_messages, next.chat, active_channel, active_thread_seq)
  channel_members = apply_chat_members(channel_members, next.chat, active_channel)
  thread_next_reply_offset = thread_offset_after_live(thread_next_reply_offset, thread_has_more, next.chat, active_channel, active_thread_seq)
  active_channel_name = channel_display_name(channels, active_channel, active_channel_name)
  active_channel_archived = channel_flag_archived(channels, active_channel, active_channel_archived)
  active_channel_members_only = channel_flag_members_only(channels, active_channel, active_channel_members_only)
  active_channel_huddle_count = channel_live_huddle_count(channels, active_channel, active_channel_huddle_count)
  post_refusal = post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key)
  channel_reads = mark_channel_read(channel_reads, live_tail_channel, channel_head_seq(channels, live_tail_channel))
  rooms = chat_sidebar_rooms(channels, dm_peers, settings_user_key, channel_reads)
  dm_rows = chat_sidebar_dms(channels, dm_peers, channel_reads)
  // THE PAGES FOLD. A committed text edit lands in the open document's blocks
  // with no query at all — the autosave commits one per tick while a reader
  // types, and each used to buy three sequential reads of the very document
  // being typed into. A delta for a block this list does not hold is a no-op.
  //
  // The buffer moves on the SAME shared decision the resync path uses below:
  // the canonical text replaces it only when the editor is CLEAN and actually
  // differs, so a reader mid-sentence keeps their words and their caret, and
  // their own settled echo is a no-op because the baseline already matches.
  // A RENAME IS AN `UpdateText` TOO — on the page's own block, which is the
  // only rename op pages has. It folds here and NOT in `apply_page_text`: the
  // block list drops the page head, so the block fold can never see it, and
  // nothing else on this path would have moved the title. Both writes sit
  // ABOVE the buffer rebuild for the reason the resync path states below —
  // the title is line 0, so it has to land before the text is rebuilt from it.
  //
  // THE FOLD MOVES A CLOCK THE RESYNC REPLY MUST READ (#1041). A text fold
  // bumps no generation, so a `live_resync_load` reply already in flight
  // still passes `live_resynced`'s guard — carrying a pre-fold snapshot. The
  // serial is the fold's own ordering token: snapshotted into every resync
  // request, echoed back on the reply, and a mismatch gates ONLY the
  // fold-owned fields (titles and block texts), never the structural half
  // the read was issued for.
  pages_fold_serial = keep_i64(pages_delta_folds(next.pages), pages_fold_serial + 1, pages_fold_serial)
  pages = apply_page_rename(pages, next.pages)
  active_page_title = apply_page_title(active_page_title, next.pages, active_page)
  blocks = apply_page_text(blocks, next.pages)
  block_comment_rows = page_comment_thread_rows(blocks, block_comment_threads, active_page)
  active_thread_anchor = comment_anchor_label(blocks, active_thread_target, active_page)
  let folded_saved = refreshed_page_saved(page_editor, active_page_title, blocks, page_saved_text)
  page_editor = refreshed_page_editor(page_editor, active_page_title, blocks, page_saved_text)
  page_saved_text = folded_saved
  bell_unread = bell_unread_after(bell_unread, bell_items, next.bell)
  bell_items = apply_bell(bell_items, next.bell)
  forge_discussion = apply_chat_messages(forge_discussion, next.chat, forge_item_channel)
  // A huddle change on the ACTIVE channel forces the chat resync the delta
  // path cannot carry: `huddle_joined`/`huddle_roster` are answered only by
  // a full chat load (see `huddle_refresh_hits`).
  // ONE PLANE WENT STALE AND THE OP SAID WHICH. `plane_live_hit` is an extern
  // for the same reason `forge_live_hit` is: the Ice checker cannot type a
  // subscription payload's field inside a `let` (see handlers/overlays.ice).
  return if next.kind != "plane" && !forge_live_hit(next.kind, next.module) && !next.load_chat && !next.load_pages && !huddle_refresh_hits(next.chat, active_channel)
  members_generation = keep_i64(plane_live_hit(next.kind, next.module, "valset"), members_generation + 1, members_generation)
  gov_generation = keep_i64(plane_live_hit(next.kind, next.module, "governance"), gov_generation + 1, gov_generation)
  account_generation = keep_i64(plane_live_hit(next.kind, next.module, "identity"), account_generation + 1, account_generation)
  dm_peers_generation = keep_i64(plane_live_hit(next.kind, next.module, "identity"), dm_peers_generation + 1, dm_peers_generation)
  agents_generation = keep_i64(plane_live_hit(next.kind, next.module, "agent"), agents_generation + 1, agents_generation)
  fs_generation = keep_i64(plane_live_hit(next.kind, next.module, "files"), fs_generation + 1, fs_generation)
  forge_generation = keep_i64(forge_live_hit(next.kind, next.module), forge_generation + 1, forge_generation)
  hydration_generation = keep_i64(next.load_chat || next.load_pages || huddle_refresh_hits(next.chat, active_channel), hydration_generation + 1, hydration_generation)
  hydration_retry_attempt = keep_i64(next.load_chat || next.load_pages || huddle_refresh_hits(next.chat, active_channel), 0, hydration_retry_attempt)
  parallel
    run replace lane=live_resync live_resync_load(connected_rpc, active_channel, active_page, resync_planes((next.load_chat || huddle_refresh_hits(next.chat, active_channel)), next.load_pages), next.debounce, hydration_generation, pages_fold_serial, 0) -> live_resynced _ | live_resync_failed _
    run replace lane=forge_live forge_live_refresh(connected_rpc, forge_repo, forge_item_number, next.kind, next.module, next.forge, (shell_tab == ShellTab.forge), forge_generation) -> forge_refreshed _ | forge_live_failed _
    // A task-flow `try` turns an unselected optional request into Task::none.
    // No refused request is launched, so an unrelated in-flight replace lane
    // is not aborted. Chip planes refresh off-tab; Files remains tab-owned.
    flow
      from done load_request(plane_live_hit(next.kind, next.module, "valset"), connected_rpc, "", members_generation)
      try request -> done request
      done -> members_load_selected _
    flow
      from done load_request(plane_live_hit(next.kind, next.module, "governance"), connected_rpc, "", gov_generation)
      try request -> done request
      done -> governance_load_selected _
    flow
      from done load_request(plane_live_hit(next.kind, next.module, "identity"), connected_rpc, "", account_generation)
      try request -> done request
      done -> account_load_selected _
    flow
      from done load_request(plane_live_hit(next.kind, next.module, "identity"), connected_rpc, "", dm_peers_generation)
      try request -> done request
      done -> dm_peers_load_selected _
    flow
      from done load_request(plane_live_hit(next.kind, next.module, "agent"), connected_rpc, "", agents_generation)
      try request -> done request
      done -> agents_load_selected _
    flow
      from done load_request(plane_live_hit(next.kind, next.module, "files") && shell_tab == ShellTab.files, connected_rpc, fs_path, fs_generation)
      try request -> done request
      done -> files_list_selected _

on live_resynced(next)
  return if next.generation != hydration_generation
  hydration_retry_attempt = 0
  // FOLD, DO NOT REPLACE — the same rule `chat_updated` states, for the same
  // reason. This read left the node several queries ago, and `channel_reads` is
  // NOT reverted with it, so a flat assignment walked a third room's `head_seq`
  // back under a cursor that stayed put: `head_seq > last_read` went false and
  // the badge a mid-flight post lit went out, dark until that room got another
  // message. `upsert_channel_rows` — which `keep_channels` runs BEHIND its
  // loaded pick, so a plane-only resync never pays for the fold — keeps
  // `head_seq` monotonic and keeps a row the answer does not carry at all: a
  // channel created while it was in flight.
  //
  // IT IS NOT A FULL MERGE, and the rest of the row is the snapshot's: a rename
  // or an archive folded during the round trip is overwritten here. That one
  // self-heals — the next chat-carrying resync re-reads the renamed row — where
  // the badge did not, which is why only the cursor's invariant is enforced.
  channels = keep_channels(next.chat_loaded, next.channels, channels)
  channel_reads = initial_channel_reads(channels, channel_reads)
  // SAME FOLD, ONE LINE ABOVE THE BANNER, because it reads `history_view` while
  // it is still the window's own answer. `load_chat_data` replies with the
  // LATEST page however far back the reader has paged, so assigning it back
  // dropped every "Load older" page she had loaded — on a huddle join in the
  // room on screen, a ws reconnect, or any chat op the delta path cannot fold.
  // `resynced_messages` splices the fresh tail onto the rows she is looking at,
  // and falls back to the replace whenever the two do not overlap — a history
  // window, or a tail the client lagged too far behind to still reach — because
  // a merge across a gap leaves a hole nothing can page in. It takes
  // `chat_loaded` itself rather than sitting under an outer loaded-pick: most
  // resyncs are plane-only, and the merge is a full copy of the window.
  messages = resynced_messages(next.chat_loaded, next.messages, messages, active_channel, next.active_channel, history_view)
  // A resync that replaced the window left the banner describing rows that are
  // no longer on screen — see `chat_hit_loaded`. One that carried no chat kept
  // the window and keeps the banner with it.
  history_view = history_view && !next.chat_loaded
  has_older_history = history_has_older(messages)
  // A resync can move the room WITHOUT a launch that abandoned the request, so
  // this is the one dropper that must ask. Conditional, not a flat clear: a
  // same-channel resync leaves a legitimate page in flight, and `history_loaded`
  // refuses any page that arrives with the flag already down. Same shape as
  // `thread_has_more` below, and it must read `active_channel` while it is still
  // the OLD room.
  history_loading = history_loading && active_channel == keep_str(next.chat_loaded, next.active_channel, active_channel)
  failed_message_draft = remember_failed_draft(failed_message_draft, "channel", message_draft, active_channel == keep_str(next.chat_loaded, next.active_channel, active_channel))
  selected_message_seq = refreshed_required_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), selected_message_seq)
  failed_message_draft = remember_failed_draft(failed_message_draft, keep_str(message_action == MessageAction.editing, "editing", ""), message_edit_draft, selected_message_seq > 0 || message_action != MessageAction.editing)
  selected_message_rev = message_seq_after_failure(selected_message_rev, MutationPhase.message_edit, selected_message_seq <= 0)
  message_action = message_action_after_failure(message_action, MutationPhase.message_edit, selected_message_seq <= 0)
  message_edit_draft = message_text_after_failure(message_edit_draft, MutationPhase.message_edit, selected_message_seq <= 0)
  channel_settings_open = channel_settings_open && active_channel == keep_str(next.chat_loaded, next.active_channel, active_channel)
  channel_name_draft = retain_for_endpoint(channel_name_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  member_key_draft = retain_for_endpoint(member_key_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  thread_generation = thread_generation_after_refresh(thread_generation, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq, refreshed_known_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq))
  thread_loading = thread_loading_after_refresh(thread_loading, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq, refreshed_known_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq))
  failed_reply_draft = retain_for_endpoint(failed_reply_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  // PARK ABOVE THE LINE THAT CAN CLOSE THE RAIL, while `active_channel` and
  // `active_thread_seq` still name the thread being typed in. The line below
  // zeroes the seq when the root was deleted or the room moved under her — and
  // a park read AFTER it is `thread_seq <= 0`, which `park_reply_draft` refuses
  // outright, so the next `open_thread_for` cannot harvest what is no longer
  // addressable. The stash this replaced took `reply_draft`, the SETTLED mirror
  // that reads "" the whole time someone is typing, into a plate mounted INSIDE
  // the rail: it captured nothing and had nowhere to render it. Idempotent on
  // the ordinary resync that leaves the rail alone — same key, same text, and
  // the entry drops itself once the box is empty.
  reply_drafts = park_reply_draft(reply_drafts, active_channel, active_thread_seq, trim(editor_text(reply_editor)))
  active_thread_seq = refreshed_known_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq)
  // NO RESTORE BESIDE IT: a resync either leaves the rail on the thread it was
  // already on — where `reply_editor` is untouched and the live buffer is the
  // truth — or closes it, and a closed rail has no composer to fill.
  thread_target_seq = refreshed_channel_value(active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), thread_target_seq)
  thread_next_reply_offset = refreshed_channel_value(active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), thread_next_reply_offset)
  thread_messages = retain_thread_messages(thread_messages, active_thread_seq)
  thread_has_more = thread_has_more && active_channel == keep_str(next.chat_loaded, next.active_channel, active_channel) && active_thread_seq > 0
  reply_draft = retain_for_endpoint(reply_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  pending_reply = retain_for_endpoint(pending_reply, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  // `message_draft` is the SETTLED stash (the body a failed send hands back) —
  // it never tracks keystrokes, so it reads
  // "" the whole time someone is typing. Rebuilding the composer from it here
  // emptied a half-written message on every resync: a `files` write in another
  // window, a teammate joining the huddle, any plane op at all. The composer
  // owns its own text and no resync produces a new one, so nothing writes it —
  // the same answer `choose_channel` already gives, and the same one
  // `refreshed_page_editor` gives the page buffer below.
  message_draft = retain_for_endpoint(message_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  pending_message = retain_for_endpoint(pending_message, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  active_channel = keep_str(next.chat_loaded, next.active_channel, active_channel)
  // The one landing with NO launch behind it, so it is the one that could move
  // the room under a DM header nobody cleared — see `dm_peer_of_channel`.
  //
  // ONLY WHEN THE ROOM ACTUALLY MOVED, for the same reason every line above it
  // is gated: `choose_dm` names the peer optimistically and leaves the room
  // being left in `active_channel` for the several blocks `open_dm` takes to
  // answer (a CreateChannel write plus two membership seats). A pages-only
  // resync landing in that window derives the peer against the OLD room and
  // blanks it, and `chat_updated` then derives "" from "" — the DM opens under
  // a `#` and the channel's own name, until the reader re-clicks it.
  //
  // AND ONLY WHEN NO LANDING IS IN FLIGHT: a CHAT-carrying resync inside that
  // same window carries the OLD room too (`live_resync_load` is launched with
  // today's `active_channel`), so `chat_loaded` alone still blanks him.
  // `loading` is true for precisely the `choose_dm` -> `chat_updated`/`failed`
  // window, and the landing it names re-derives the peer itself.
  active_dm_peer = keep_str(next.chat_loaded && !loading, dm_peer_of_channel(active_dm_peer, settings_user_key, active_channel), active_dm_peer)
  active_dm = dm_peer_named(dm_peers, active_dm_peer)
  active_channel_name = keep_str(next.chat_loaded, next.active_channel_name, active_channel_name)
  active_channel_archived = keep_bool(next.chat_loaded, next.active_channel_archived, active_channel_archived)
  active_channel_members_only = keep_bool(next.chat_loaded, next.active_channel_members_only, active_channel_members_only)
  active_channel_huddle_count = keep_i64(next.chat_loaded, next.active_channel_huddle_count, active_channel_huddle_count)
  // The join/leave acks land here: a huddle op forces a chat resync
  // (`live.rs` sets load_chat on roster changes), and this is where the
  // roster it fetched finally answers "am I in it". Without these lines the
  // LIVE pill never appeared until a manual channel re-pick.
  huddle_joined_at = keep_i64(huddle_joined, huddle_joined_at, huddle_now)
  huddle_joined = keep_bool(next.chat_loaded, huddle_self(next.huddle_roster), huddle_joined)
  huddle_roster = keep_roster(huddle_joined, keep_participants(next.chat_loaded, next.huddle_roster, huddle_roster))
  huddle_rows = huddle_tile_rows(huddle_roster, call_peers, call_muted)
  huddle_channel = keep_str(huddle_joined, active_channel, "")
  huddle_channel_name = keep_str(huddle_joined, active_channel_name, "")
  channel_members = keep_members(next.chat_loaded, next.channel_members, channel_members)
  post_refusal = post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key)
  // THE READ CURSOR TAKES THE LIVE FOLD'S TWO GATES, and it has to: a
  // PLANE-ONLY resync lands here on every files write, valset change, identity
  // or agent or governance op — routine traffic the reader causes herself by
  // saving a file — and an ungated mark from one of those undid both of the
  // answers this file just gave. Off-tab: the arrival `live_updated` refused to
  // mark read got marked read anyway on her next save, so the divider
  // `select_shell_tab` freezes had nothing left to name. On a search hit: the
  // room was marked read to a head she never reached while her
  // `MessageWindow::Around` window — and the banner — were still on screen.
  //
  // A CHAT-CARRYING resync is the one landing that legitimately catches up:
  // `history_view` is lowered above and the tail is what she is looking at, so
  // it takes the mark like `chat_updated` does — if she is on the tab.
  let resync_tail_channel = keep_str(!history_view && shell_tab == ShellTab.chat, active_channel, "")
  unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, active_channel, unread_boundary)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  channel_reads = mark_channel_read(channel_reads, resync_tail_channel, channel_head_seq(channels, resync_tail_channel))
  rooms = chat_sidebar_rooms(channels, dm_peers, settings_user_key, channel_reads)
  dm_rows = chat_sidebar_dms(channels, dm_peers, channel_reads)
  // A resync carries whatever page was active WHEN IT WAS ISSUED and takes
  // several queries to answer, so a mutation landing in between leaves it
  // speaking for a document nobody is on — measured on a page create. The page
  // LIST's structure is never stale (it is the whole index either way) and
  // still lands; everything scoped to ONE page waits for a reply that
  // answers for the page in hand.
  pages_answer_is_current = next.pages_loaded && pages_reply_answers_current(next.pages, next.active_page, active_page)
  // A TEXT FOLD THAT LANDED WHILE THIS REPLY WAS IN FLIGHT OWNS WHAT IT WROTE
  // (#1041). The serial the request snapshotted no longer matching means a
  // rename or a body edit folded after the reply's reads left — and text
  // folds are the ONLY pages writes that can land inside a still-current
  // window, because every structural delta sets `load_pages`, bumps the
  // generation, and orphans this very reply at the guard above. So the
  // divergence is exactly the folded titles and block texts: those keep the
  // fold's value, while the structure the read was issued for still lands
  // from the reply. Discarding the pages half wholesale here would trade one
  // staleness for the other — the defect both of #1041's rejected designs
  // shared.
  pages_fold_outran_reply = next.fold_serial != pages_fold_serial
  pages = keep_pages(next.pages_loaded, keep_folded_page_titles(pages_fold_outran_reply, next.pages, pages), pages)
  blocks = keep_blocks(pages_answer_is_current, merge_pending_blocks(keep_folded_block_texts(pages_fold_outran_reply, next.blocks, blocks), blocks, buffer_page, next.active_page, ""), blocks)
  orphaned_comment_drafts = remember_orphaned_page_comment(orphaned_comment_drafts, pages, block_comments_target, block_comment_draft)
  // THE COMMENTS RAIL IS DOCUMENT-SCOPED (handlers/pages.ice:300). Its anchor is
  // the PAGE it was opened on, never a block selection — keyed on
  // `selected_block_id` it closed itself, and threw the half-typed comment away,
  // the moment the user clicked off the block whose ⋮ menu opened it. So the
  // target is the one thing reconciled against the page identity here, and every
  // other rail field keys on the target: one line decides the whole rail.
  block_comments_target = retain_for_endpoint(block_comments_target, active_page, keep_str(pages_answer_is_current, next.active_page, active_page))
  block_comments_open = block_comments_open && !empty(block_comments_target)
  block_comment_threads = retain_selected_comment_threads(block_comment_threads, block_comments_target)
  block_comment_thread_total = retain_selected_i64(block_comment_thread_total, block_comments_target)
  block_comment_threads_next_from = retain_selected_i64(block_comment_threads_next_from, block_comments_target)
  block_comment_threads_has_more = block_comment_threads_has_more && !empty(block_comments_target)
  block_comment_threads_loading = block_comment_threads_loading && !empty(block_comments_target)
  active_block_comment_thread = retain_selected_string(active_block_comment_thread, block_comments_target)
  block_thread_comments = retain_selected_comments(block_thread_comments, block_comments_target)
  block_thread_comments_next_from = retain_selected_i64(block_thread_comments_next_from, block_comments_target)
  block_thread_comments_has_more = block_thread_comments_has_more && !empty(block_comments_target)
  block_thread_comments_loading = block_thread_comments_loading && !empty(block_comments_target)
  block_comment_draft = retain_selected_string(block_comment_draft, block_comments_target)
  pending_block_comment = retain_selected_string(pending_block_comment, block_comments_target)
  page_delete_armed = page_delete_armed && active_page == keep_str(pages_answer_is_current, next.active_page, active_page)
  block_comment_thread_total = keep_i64(pages_answer_is_current, next.comment_thread_total, block_comment_thread_total)
  commented_block_hits = keep_strs(pages_answer_is_current, next.commented_block_hits, commented_block_hits)
  active_page = keep_str(pages_answer_is_current, next.active_page, active_page)
  block_comment_rows = page_comment_thread_rows(blocks, block_comment_threads, active_page)
  active_thread_anchor = comment_anchor_label(blocks, active_thread_target, active_page)
  // The header title is fold-owned too — same #1041 rule as the row above,
  // and it must hold HERE because the editor rebuild below reads it as line 0.
  // `active_page_parent` is not: no fold writes a parent, so it stays the
  // reply's.
  active_page_title = keep_str(pages_answer_is_current && !pages_fold_outran_reply, next.active_page_title, active_page_title)
  active_page_parent = keep_str(pages_answer_is_current, next.active_page_parent, active_page_parent)
  // AFTER the title lands, because the title is line 0 of the buffer. The
  // canonical text only replaces the buffer when the editor is CLEAN and the
  // text actually differs — a rebuilt `Content` throws the cursor to the
  // origin, so the saved baseline and the buffer move on one shared decision.
  let resynced_saved = refreshed_page_saved(page_editor, active_page_title, blocks, page_saved_text)
  page_editor = refreshed_page_editor(page_editor, active_page_title, blocks, page_saved_text)
  page_saved_text = resynced_saved
  // The buffer's own page follows the buffer, and only when this resync
  // actually carried page news AND the buffer moved with it.
  //
  // A dirty buffer refused the refresh above and still belongs to the page it
  // was typed in — a resync that lands on another page (this one was deleted)
  // must not claim it, or the next load would read the switch as a refresh and
  // keep the old text under the new page's title.
  //
  // `pages_answer_is_current` is the other half and it is the load-bearing one.
  // A CHAT-ONLY resync arrives with `pages_loaded == false`, so `blocks` keeps
  // whatever it holds — which, in the window `choose_page` opens, is empty —
  // and the refresh above canonicalises `title + []` into a document that never
  // came from the node. Claiming that as the new page's buffer hands
  // `page_autosave_tick` a fabricated document it is willing to write: the
  // page would be overwritten with a blank one it never loaded.
  let resynced_buffer_is_clean = page_text(page_editor) == page_saved_text
  buffer_page = keep_str(resynced_buffer_is_clean && pages_answer_is_current, active_page, buffer_page)
  // THE RECOVERY'S TERMINAL. `mutation_failed` parks the lock at "recovering"
  // for a write the node COMMITTED and then failed to read back, and launches
  // this resync to learn what actually landed — but nothing ever released it:
  // every other writer of "idle" sits behind a `mutation_phase != MutationPhase.idle` guard
  // it can no longer pass, so the whole sidebar stayed disabled (and the
  // titlebar stuck on "Syncing…") until Settings → Reconnect. The state the
  // lock was protecting is known good exactly here, and `live_resync_failed`
  // retries forever, so this is the one landing that can end it — the same
  // shape `block_threads_recovered`/`_recovery_failed` already uses in pages.
  //
  // IT RELEASES EITHER ORIGIN'S RECOVERY — `block_comment_post_failed` parks the
  // same phase, and this landing cannot tell whose it is holding. That is a
  // sidebar unlocked while the comment rail is still refetching, no worse; what
  // it must NOT become is a lock released twice, so those two pages terminals
  // now take the same `== "recovering"` term rather than flatly writing "idle"
  // over whatever mutation started in the gap.
  //
  // AND IT IS STILL ORPHANABLE — the known ceiling, named here because the guard
  // at the top of this handler is where it bites. `mutation_failed` bumps
  // `hydration_generation` to launch its recovery, and ANY later bump makes the
  // answer stale, so `return if next.generation != hydration_generation` drops
  // it and the `live_resync_failed` retry chain with it. Almost every bumper is
  // behind a `mutation_phase != MutationPhase.idle` guard "recovering" cannot pass; the ones
  // that are NOT are the two acts that deliberately run outside the lock — a
  // message send (`chat_composer_event`, `reply_composer_event`) and a
  // reaction tap (`add_reaction_submit`, `add_reaction_at`,
  // `remove_reaction_at`) — plus two LANDINGS rather than acts,
  // `chat_load_failed` and `failed` (the `load_page` error arm below): their
  // launchers are all gated on the lock, but a switch already in flight when
  // `mutation_failed` parked it lands under "recovering", and `mutation_failed`
  // invalidates neither the `chat_load` nor the `page_load` lane. None of the
  // seven launch a replacement on the `live_resync` lane, so any of them inside
  // the recovery's round trip leaves the lock held with no terminal again, and
  // Settings → Reconnect (whitelisted for "recovering" at the top of this file)
  // is the escape. `connect_failed` bumps ungated too and is deliberately NOT on
  // that list: its own forever-retry ends at `workspace_connected`, which writes
  // `mutation_phase = MutationPhase.idle` unconditionally, so that chain releases the lock
  // itself. Closing the rest means giving the recovery a generation of its own
  // rather than riding this one, which is a wider change than the lock is worth
  // today: this is strictly the residue of a case that used to be
  // unconditional.
  mutation_phase = mutation_phase_after_recovery(mutation_phase)
  error = ""
  block_comments_generation = block_comments_generation + 1
  // The rail's live refresh must ask the SAME question the rail was filled
  // from. The old refresh asked `ThreadsForTargets` for the target ALONE, so
  // with a page target it found only page-anchored threads and wiped every
  // block-anchored one out of the open rail on the next pages event.
  // `load_page_threads` fans out over the page AND its blocks, and answers on
  // the handler pages.ice already routes its own loads through. It ignores a
  // closed rail, so a page event with no rail open costs one refused query and
  // never touches the banner.
  //
  // The list is all that refreshes live. An OPEN thread's replies do not: a task
  // group must be the final statement in a handler, so the comment-page load
  // cannot be guarded on `active_block_comment_thread`, and firing it unguarded
  // asks the node for thread "" — whose failure paints `block_comment_page_failed`
  // over the rail every time anyone edits the page. Replies still arrive on post
  // and on reopen; a page-scoped comment refresh in backend.rs closes the gap.
  parallel
    run replace lane=live_thread refresh_live_thread(connected_rpc, active_channel, active_thread_seq, thread_target_seq, thread_next_reply_offset) -> live_thread_refreshed _ | live_thread_refresh_failed _
    run replace lane=block_threads load_page_threads(connected_rpc, block_comments_target, block_comments_generation) -> block_threads_loaded _ | block_threads_failed _
    // Same close-if-ended mirror as `workspace_connected` — this is the fold
    // the steady state pays (a roster change forces a chat resync into here).
    task window close target=window_target_unless(huddle_joined, huddle_win)

on live_resync_failed(cause)
  return if cause.generation != hydration_generation
  status = "Sync delayed"
  error = "Live sync interrupted. Retrying…"
  hydration_retry_attempt = hydration_retry_attempt + 1
  run replace lane=live_resync live_resync_load(connected_rpc, active_channel, active_page, "both", false, hydration_generation, pages_fold_serial, hydration_retry_attempt) -> live_resynced _ | live_resync_failed _

on live_thread_refreshed(next)
  return if next.channel_id != active_channel || next.root_seq != active_thread_seq
  return if thread_loading || mutation_phase != MutationPhase.idle
  thread_target_seq = next.target_seq
  thread_messages = merge_pending_messages(next.messages, thread_messages, active_channel, next.channel_id, "")
  thread_next_reply_offset = next.next_reply_offset
  thread_has_more = next.has_more

on live_thread_refresh_failed(_cause)
  error = error

on select_shell_tab(next)
  shell_tab = next
  // A credential read belongs to the Shell visit that issued it. Bump on
  // EVERY move, including the chat/pages early return below, so a late reply
  // cannot repaint a screen the reader already left.
  shell_credentials_generation = shell_credentials_generation + 1
  shell_credentials_loading = connected && shell_tab == ShellTab.shell
  // A TAB MOVE UNMOUNTS THE CHAT COMPOSERS, so the caret they claimed is gone
  // — and a `shell_tab == ShellTab.chat` term on the chord could only mute it while
  // she is away, then hand the stale claim straight back on the return trip.
  // Every handler that writes `shell_tab` retires it, linted in tests.rs.
  composer_focus = ComposerFocus.unfocused
  has_older_history = history_has_older(messages)
  // A RETURN TO THE CHAT TAB IS A CHANNEL ENTRY, and it is the other half of
  // `live_updated`'s tab gate: the cursor stood still while the pane was
  // unmounted, so this is where the room she is coming back to is caught up —
  // after the divider naming what arrived in her absence is frozen, which has
  // to happen BEFORE the cursor moves past it.
  //
  // `chat_tab_channel` carries all three gates: on any other tab it is "" and
  // every line is inert (`mark_channel_read` refuses an empty channel), and a
  // history window is no more caught up here than it is in the live fold. The
  // boundary re-freezes only when the room actually grew unread rows, so a tab
  // round trip with nothing new keeps the divider she left on screen.
  let chat_tab_channel = keep_str(shell_tab == ShellTab.chat && !history_view, active_channel, "")
  let chat_tab_arrivals = channel_head_seq(channels, chat_tab_channel) > channel_last_read(channel_reads, chat_tab_channel)
  unread_boundary = keep_i64(chat_tab_arrivals, channel_last_read(channel_reads, chat_tab_channel), unread_boundary)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  channel_reads = mark_channel_read(channel_reads, chat_tab_channel, channel_head_seq(channels, chat_tab_channel))
  rooms = chat_sidebar_rooms(channels, dm_peers, settings_user_key, channel_reads)
  dm_rows = chat_sidebar_dms(channels, dm_peers, channel_reads)
  // A hydration error belongs to the pane that raised it. Leaving it up after
  // a navigation tells the user the pane they just opened is broken, which is
  // a lie the banner has no way to walk back — it is dismissed by hand or not
  // at all. Clearing here, ABOVE both early returns, is what makes that true
  // for every tab: the `!connected` return and the chat/pages return each skip
  // the generation bumps below, but neither should keep a stale banner alive.
  error = ""
  return if !connected
  return if shell_tab == ShellTab.chat || shell_tab == ShellTab.pages
  explorer_generation = explorer_generation + 1
  fs_generation = fs_generation + 1
  members_generation = members_generation + 1
  gov_generation = gov_generation + 1
  agents_generation = agents_generation + 1
  account_generation = account_generation + 1
  forge_generation = forge_generation + 1
  forge_list_phase = keep_forge_phase(shell_tab == ShellTab.forge, ForgePhase.loading, forge_list_phase)
  // THE SETTINGS BUMP IS GATED TOO, AND IT IS THE ONE THAT HAS TO BE. Every
  // other loader here draws only its own tab, so a bump that discards a
  // still-flying CONNECT load is re-earned the moment that tab is opened. The
  // settings facts are not: chat mounts `settings_user_key` as `me`, and chat
  // returns above this block, so nothing chat does ever re-issues them. Bump
  // unconditionally and a move into Members while the connect load is in
  // flight orphans it, and `me` stays "" for the session, which
  // `chat_sidebar_rooms` reads as "show every DM under CHANNELS"
  // and `post_gate` as "not seated", refusing the composer on every DM.
  settings_generation = keep_i64(shell_tab == ShellTab.settings, settings_generation + 1, settings_generation)
  node_peers_generation = node_peers_generation + 1
  explorer_loading = shell_tab == ShellTab.explorer
  fs_loading = shell_tab == ShellTab.files
  // No `files_find` here. This block runs for EVERY tab but chat and pages, so
  // a whole-workspace prefix walk was issued on the way into Settings, Forge,
  // Members and Agents — for a `files_tree` no view reads, on a route whose
  // failure paints the GLOBAL error banner over a screen with no file operation
  // in sight. `fs_wrote` still refreshes the tree from inside the files tab.
  //
  // Optional request payloads select only the destination's effects. `try`
  // lowers an unselected request to Task::none, so changing tabs cannot abort
  // an unrelated replace lane with a synthetic refusal.
  parallel
    run replace lane=node_facts_load load_node_facts(connected_rpc) -> node_facts_loaded _ | node_facts_failed _
    flow
      from done load_request(shell_tab == ShellTab.explorer, connected_rpc, "", explorer_generation)
      try request -> done request
      done -> explorer_load_selected _
    flow
      from done load_request(shell_tab == ShellTab.files, connected_rpc, fs_path, fs_generation)
      try request -> done request
      done -> files_list_selected _
    flow
      from done load_request(shell_tab == ShellTab.files, connected_rpc, "", fs_generation)
      try request -> done request
      done -> files_history_selected _
    flow
      from done load_request(tab_reads_plane(shell_tab, "members"), connected_rpc, "", members_generation)
      try request -> done request
      done -> members_load_selected _
    flow
      from done load_request(tab_reads_plane(shell_tab, "governance"), connected_rpc, "", gov_generation)
      try request -> done request
      done -> governance_load_selected _
    flow
      from done load_request(shell_tab == ShellTab.settings, connected_rpc, "", settings_generation)
      try request -> done request
      done -> settings_load_selected _
    flow
      from done load_request(shell_tab == ShellTab.settings && node_tab == NodeTab.overview, connected_rpc, "", node_peers_generation)
      try request -> done request
      done -> peers_load_selected _
    flow
      from done load_request(tab_reads_plane(shell_tab, "agents"), connected_rpc, "", agents_generation)
      try request -> done request
      done -> agents_load_selected _
    flow
      from done load_request(tab_reads_plane(shell_tab, "account"), connected_rpc, "", account_generation)
      try request -> done request
      done -> account_load_selected _
    flow
      from done load_request(shell_tab == ShellTab.forge, connected_rpc, "", forge_generation)
      try request -> done request
      done -> forge_load_selected _
    flow
      from done load_request(shell_tab == ShellTab.shell, connected_rpc, "", shell_credentials_generation)
      try request -> done request
      done -> shell_credentials_load_selected _

// Conditional effects are selected one update before launch. The selector's
// optional `try` emits no message when false. A newer intent, tab, or network
// can land before the selected message, so each destination rejects an
// obsolete request before it starts the normal compiler `run replace` lane.
on explorer_load_selected(request)
  let obsolete_request = request.rpc != connected_rpc || request.generation != explorer_generation
  let unmounted = shell_tab != ShellTab.explorer
  return if obsolete_request || unmounted
  run replace lane=explorer_load load_explorer(request.rpc, request.generation) -> explorer_loaded _ | explorer_failed _

on files_list_selected(request)
  let obsolete_request = request.rpc != connected_rpc || request.generation != fs_generation
  let unmounted = shell_tab != ShellTab.files
  return if obsolete_request || unmounted
  run replace lane=files_list files_ls(request.rpc, request.key, request.generation) -> fs_listed _ | fs_failed _

on files_history_selected(request)
  let obsolete_request = request.rpc != connected_rpc || request.generation != fs_generation
  let unmounted = shell_tab != ShellTab.files
  return if obsolete_request || unmounted
  run replace lane=files_history files_history(request.rpc, request.generation) -> fs_history_loaded _ | fs_failed _

on members_load_selected(request)
  let obsolete_request = request.rpc != connected_rpc || request.generation != members_generation
  return if obsolete_request
  run replace lane=members_load load_members(request.rpc, request.generation) -> members_loaded _ | members_failed _

on governance_load_selected(request)
  let obsolete_request = request.rpc != connected_rpc || request.generation != gov_generation
  return if obsolete_request
  run replace lane=governance_load load_governance(request.rpc, request.generation) -> governance_loaded _ | governance_failed _

on settings_load_selected(request)
  let obsolete_request = request.rpc != connected_rpc || request.generation != settings_generation
  let unmounted = shell_tab != ShellTab.settings
  return if obsolete_request || unmounted
  run replace lane=settings_load load_settings_facts(request.rpc, request.generation) -> settings_loaded _ | settings_failed _

on peers_load_selected(request)
  let obsolete_request = request.rpc != connected_rpc || request.generation != node_peers_generation
  let unmounted = shell_tab != ShellTab.settings || node_tab != NodeTab.overview
  return if obsolete_request || unmounted
  run replace lane=peers_load load_peers(request.rpc, request.generation) -> peers_loaded _ | peers_failed _

on agents_load_selected(request)
  let obsolete_request = request.rpc != connected_rpc || request.generation != agents_generation
  return if obsolete_request
  run replace lane=agents_load load_agents(request.rpc, request.generation) -> agents_loaded _ | agents_failed _

on account_load_selected(request)
  let obsolete_request = request.rpc != connected_rpc || request.generation != account_generation
  return if obsolete_request
  run replace lane=account_load load_account(request.rpc, request.generation) -> account_loaded _ | account_failed _

on dm_peers_load_selected(request)
  let obsolete_request = request.rpc != connected_rpc || request.generation != dm_peers_generation
  return if obsolete_request
  run replace lane=dm_peers_load load_dm_peers(request.rpc, request.generation) -> dm_peers_loaded _ | dm_peers_failed _

on forge_load_selected(request)
  let obsolete_request = request.rpc != connected_rpc || request.generation != forge_generation
  let unmounted = shell_tab != ShellTab.forge
  return if obsolete_request || unmounted
  run replace lane=forge_load load_forge(request.rpc, request.generation) -> forge_loaded _ | forge_list_failed _

on shell_credentials_load_selected(request)
  let obsolete_request = request.rpc != connected_rpc || request.generation != shell_credentials_generation
  let unmounted = shell_tab != ShellTab.shell
  return if obsolete_request || unmounted
  run replace lane=shell_credentials load_agent_credentials(request.rpc, request.generation) -> shell_credentials_loaded _ | shell_credentials_failed _

// The huddle's elapsed clock is a LOCAL session fact: one tick per second for
// as long as SHE is in the huddle, never a chain value. `huddle_joined_at` is
// stamped from `huddle_now` when she joins, so mm:ss is their difference.
on tick
  huddle_now = huddle_now + 1

on wall_tick
  wall_now = current_wall_seconds()

// The app has exactly ONE subscribe block. Component handler files may not
// declare another, so every new subscription lands here.
//
// The node log stream follows the node body into Settings — the rail seat it
// used to key on is gone, and a predicate that can never be true again would
// have taken the log console dark without a word.
subscribe
  run live_events(connected_rpc) when connected -> live_updated _
  agent_terminal_events(shell_terminal) when (connected && shell_tab == ShellTab.shell && shell_mode == ShellMode.raw && shell_terminal_running) -> shell_terminal_notice _
  // THE CALL SESSION IS THIS SUBSCRIPTION. Joining a huddle flips
  // `huddle_joined` and the media leg connects; leaving (or disconnecting)
  // stops the subscription, the stream drops, and the websocket + audio
  // threads tear down with it. No imperative start/stop anywhere.
  run call_session(connected_rpc, huddle_channel) when (connected && huddle_joined && !empty(huddle_channel)) -> call_event _
  // Video has NO subscription: the tile strip is a self-redrawing widget
  // that repaints only its own window at the capture cadence.
  // `status=ignored` IS THIS SUBSCRIPTION'S PRICE TAG, not a nicety. An
  // unfiltered `keyboard press` fires for keys a focused widget already
  // CONSUMED, and the message it publishes cannot join the batch that widget's
  // own message is in: it leaves through the event-loop proxy and comes back as
  // a winit user event on the NEXT turn, where iced 0.14 rebuilds
  // unconditionally (there is no dirty check). So every character typed into a
  // composer bought a SECOND full ChatScreen build+layout — twice the
  // allocations the frame-cost gate reports, because the gate drives only the
  // widget's message.
  //
  // Every chord this handler routes bubbles by construction: the rich editor
  // lets command-letter presses through (`command_shortcut_bubbles`) and drops
  // Escape uncaptured (`Binding::Unfocus`), and iced's single-line input
  // consumes neither.
  keyboard press status=ignored when (connected || palette_open) -> global_key_pressed _
  // EXCEPT ESCAPE OUT OF A PLAIN `input`, which iced's text_input DOES consume
  // — the palette's own field, the create-channel modal, the details drawer.
  // The captured half is routed for exactly that, gated on the ESCAPE LADDER'S
  // OWN reading of whether any transient layer is up: with none open a captured
  // key has nothing to dismiss, and that is precisely the state a reader typing
  // into a composer is in, so the hot path pays nothing for this line.
  //
  // IT CARRIES NO `connected` TERM, and that asymmetry with the line above is
  // deliberate. `topmost_overlay` IS this half's precondition — a layer that is
  // up ate the key and must come down whether or not the socket is alive, and a
  // drawer that refused Escape while disconnected would be a trap. Nothing else
  // gets through: `palette_key_action`'s open arm is separately gated on
  // `connected` in `overlays.ice`, as are the mark and page chords, and the
  // chord keys bubble anyway so they arrive on the IGNORED half.
  //
  // `key=escape` is the key-level gate: typing into an open layer's own field
  // no longer publishes a redundant captured-key update per character.
  keyboard press key=escape status=captured when !empty(topmost_overlay(palette_open, bell_open, channel_create_open, thread_message_action, message_action, channel_settings_open, forge_repo_menu)) -> global_key_pressed _
  // THE PANE SCROLL'S KEYS ARE THE LEFTOVERS. `status=ignored` drops every key
  // a focused widget CONSUMED — Home in a text field, an arrow in an open
  // list — but it is only half the arbitration: iced's single-line input drops
  // Up/Down uncaptured, so the router itself refuses the arrows and every key
  // under an open overlay (`content_scroll_step`). Ungated on purpose: the
  // launch and huddle windows mount no content pane, and a scroll operation
  // whose target is not on screen is a no-op.
  keyboard press status=ignored -> content_scroll_key _
  window file-dropped -> fs_file_dropped _
  // A daemon outlives its windows, so process exit is an explicit decision:
  // when the LAST tracked window closes, leave.
  window closed with-id -> window_was_closed _
  run node_logs(connected_rpc) when (connected && shell_tab == ShellTab.settings && node_tab == NodeTab.activity) -> node_log_line _
  // THE NODE'S OWN TWO PLANES. Peers and the consensus facts have no op behind
  // them — nothing in the index names a mesh connection or a checkpoint height
  // — so no module topic can carry them, which is why they were the last two
  // surfaces the console left cold, refreshed only by a connect or a tab
  // switch.
  //
  // Not a poll. The node re-samples these only while this subscription is
  // held, so THIS GATE IS THE BUDGET: `/v1/peers` composes its sample by
  // encoding the whole metrics registry (485 KB, ~10 ms a call, measured), and
  // leaving the tab stops that at the source rather than throttling it here.
  // STATUS RIDES EVERYWHERE. The node answers it from a cell it publishes at
  // each boundary, so a console holding it on every tab costs one read per
  // heartbeat — and the node's phase is a fact about the NODE, not about the
  // surface the reader happens to have open.
  run node_status_live(connected_rpc) when connected -> node_status_pushed _
  // PEERS DOES NOT. Each sample encodes the whole metrics registry, so this
  // gate is the budget: leaving the tab stops the encode at the source.
  run node_peers_live(connected_rpc) when (connected && shell_tab == ShellTab.settings && node_tab == NodeTab.overview) -> node_peers_pushed _
  every 1s when huddle_joined -> tick
  every 1s when console_win != none -> wall_tick
  every 300ms when !empty(toast) -> toast_tick
  // The settle-✓'s dismissal clock: tick one holds the tick on screen and
  // starts its fade, tick two unmounts it. Gated on an anchored ✓ — it costs
  // nothing outside the seconds after a send settles. A live delta may start
  // the fade earlier; this clock is the floor a quiet network needs.
  every 1200ms when (!empty(send_flash_id)) || (!empty(thread_send_flash_id)) -> send_flash_tick
  // The page document's autosave: the editor's edits never pass through a
  // handler, so the gate IS the dirty test — the tick only exists while the
  // buffer has drifted from the last text known written.
  every 900ms when (connected && !empty(active_page) && page_text(page_editor) != page_saved_text) -> page_autosave_tick

// The daemon's exit rule: closing a window unregisters it, and the process
// leaves with the last one. The handoff paths (`console_opened`,
// `onboarding_reopened`) close their predecessor AFTER the successor is
// registered, so this never fires with a survivor still tracked.
// The huddle window is deliberately NOT in the survivor guard: closing it is a
// dock, not an exit, and a lone huddle window must never keep the daemon alive
// after its console is gone.
on window_was_closed(id)
  onboarding_win = without_window(onboarding_win, id)
  console_win = without_window(console_win, id)
  huddle_win = without_window(huddle_win, id)
  return if (onboarding_win != none) || (console_win != none)
  exit

on mutation_failed(cause)
  selected_message_seq = message_seq_after_failure(selected_message_seq, mutation_phase, cause.committed)
  selected_message_rev = message_seq_after_failure(selected_message_rev, mutation_phase, cause.committed)
  message_action = message_action_after_failure(message_action, mutation_phase, cause.committed)
  message_edit_draft = message_text_after_failure(message_edit_draft, mutation_phase, cause.committed)
  thread_selected_seq = message_seq_after_failure(thread_selected_seq, mutation_phase, cause.committed)
  thread_selected_rev = message_seq_after_failure(thread_selected_rev, mutation_phase, cause.committed)
  thread_message_action = message_action_after_failure(thread_message_action, mutation_phase, cause.committed)
  thread_edit_draft = message_text_after_failure(thread_edit_draft, mutation_phase, cause.committed)
  mutation_phase = mutation_failure_phase(cause.committed)
  channel_draft = restore_draft(channel_draft, pending_channel, cause.committed)
  page_draft = restore_draft(page_draft, pending_page, cause.committed)
  pending_channel = ""
  pending_page = ""
  error = cause.message
  return if !cause.committed
  // The lane invalidation discards the in-flight save's reply without
  // aborting its write, so the status reset is the inseparable other half.
  // Otherwise the "saving" guard holds the tick forever.
  invalidate lane=page_autosave
  block_autosave_status = AutosaveStatus.idle
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run replace lane=live_resync live_resync_load(connected_rpc, active_channel, active_page, "both", false, hydration_generation, pages_fold_serial, 0) -> live_resynced _ | live_resync_failed _

on dismiss_error
  error = ""

on restore_failed_message
  return if empty(failed_message_draft) || !empty(trim(editor_text(message_editor))) || mutation_phase != MutationPhase.idle
  message_draft = failed_message_draft
  message_editor = editor(message_draft)
  failed_message_draft = ""

on dismiss_failed_message
  failed_message_draft = ""

on restore_failed_reply
  return if empty(failed_reply_draft) || !empty(trim(editor_text(reply_editor)))
  reply_draft = failed_reply_draft
  reply_editor = editor(reply_draft)
  failed_reply_draft = ""
  // The reply composer is an extern `rich_composer` mount now, and `task
  // widget focus` cannot target an extern component — restoring no longer
  // auto-focuses. Re-add when ui-lang grows extern focus targets.

on dismiss_failed_reply
  failed_reply_draft = ""

// THE CONNECT RETRIES; IT USED TO GIVE UP AFTER ONE FAILURE. The steady-state
// path has always retried forever (`live_resync_failed` below), so the console
// healed itself from every interruption EXCEPT the one that gets it running.
// One transient failure left it Offline against a node answering `/v1/status`
// in under a millisecond, with no way back but the network picker — and issue
// #1018 makes that failure ordinary: a `/v1/query` can block until the node
// writes its next checkpoint, which outlasts the RPC client's 30s timeout.
//
// The backoff is `live_resync_load`'s own — 1s doubling to a 16s cap — so a
// genuinely dead endpoint costs one request every 16s, and the degraded
// surfaces keep telling the truth while it retries: `connected` stays false,
// the band and the "Not connected" plate stay up, and the plate still names the
// way out for a reader who does not want to wait.
on connect_failed(cause)
  // ONE CHAIN AT A TIME. Every route into a connect bumps the generation, so a
  // failure carrying an older one belongs to a chain that was abandoned — a
  // second network chosen, a reconnect pressed. Retrying it would leave two
  // chains alive forever, and each chain's own bump could then reject the
  // other's success. Measured before this guard existed: a dead endpoint drew
  // two interleaved retry series 5.2s and 10.8s apart, summing to one 16s cap.
  return if cause.generation != connect_generation
  hydration_generation = hydration_generation + 1
  connect_generation = connect_generation + 1
  hydration_retry_attempt = hydration_retry_attempt + 1
  loading = false
  status = "Offline"
  error = cause.message
  run replace lane=connect connect(connected_rpc, hydration_retry_attempt, connect_generation) -> workspace_connected _ | connect_failed _

// ONE LOAD FAILED; THE CONNECTION DID NOT SAY ANYTHING. This is the failed arm
// of `load_chat`, `open_dm`, `load_chat_hit` and the three page routes, and it
// used to write `status = "Offline"` — the connection's own word, over a live
// socket. `connected` stays true, so nothing reconnects and nothing corrects
// it: the sidebar dot goes red and the titlebar pill reads Offline until the
// next block's `live_updated` overwrites the status, up to 3s on a quiet chain.
// A single `/v1/query` blocking past the RPC timeout is ordinary (see
// `connect_failed` above), so this arm says what it knows — the load failed —
// and leaves the connection's word to the connection's own handlers.
on failed(cause)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = false
  error = cause.message
