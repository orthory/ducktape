// Two lists this file loads that state.ice has no field for. A handler file may
// declare app state, and the loaders that fill these live here.
state
  // The DIRECT section of the chat sidebar: every principal you can open a
  // two-party channel with.
  dm_peers:[DmPeer] = []
  // The duckfs path list behind the 206px tree, which is a different question
  // from `fs_entries` (one directory) and comes from a different call. It is
  // the find route's FIRST page — 256 paths (duckfs wire.rs MAX_PAGE) — so a
  // workspace larger than that shows a tree that stops, until the tree itself
  // learns to page on its `next` cursor.
  files_tree:[FsEntry] = []
  // The consensus trio off /v1/status, held as the TEXT the console prints.
  // `NodeFacts` carries them as `i64?` on purpose — a resident has no consensus
  // block at all — and `optional_number` renders `—` for absent. Storing the
  // rendered label is what keeps an absent reading from arriving as a measured
  // zero, which is what `view 0 · 0/0 certs` on a healthy chain was.
  node_view_label = "—"
  node_quorum_label = "—"
  node_reachable_label = "—"

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
  run save_appearance(appearance) -> appearance_saved _

on set_appearance_dark
  appearance = "dark"
  app_palette = AppTheme.app_dark
  app_background = "#1b1a16"
  app_text = "#e8e6df"
  run save_appearance(appearance) -> appearance_saved _

on appearance_saved(_written)
  error = error

// A SAME-ENDPOINT retry: the launch window's picker owns which network, so
// reconnect no longer changes endpoints — the per-endpoint draft retention
// that lived here collapsed to identity calls and is gone. Typed drafts
// deliberately survive (the editor harvest + orphan lines); the network
// lists are re-fetched.
on reconnect
  return if loading || (mutation_phase != "idle" && mutation_phase != "recovering")
  block_autosave_generation = cancel_autosaves(connected_rpc, block_autosave_generation)
  message_draft = trim(editor_text(message_editor))
  message_editor = editor(message_draft)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], active_page, block_comment_draft)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "idle"
  loading = true
  connected = false
  channels = []
  messages = []
  has_older_history = false
  // Same abandoned request, same dead button — see `choose_channel`. A reconnect
  // drops the socket the page was requested on, so it may never answer at all.
  history_loading = false
  channel_reads = []
  unread_boundary = 0
  active_channel = ""
  active_channel_name = ""
  active_channel_archived = false
  active_channel_members_only = false
  active_channel_huddle_count = 0
  channel_members = []
  channel_settings_open = false
  channel_name_draft = ""
  member_key_draft = ""
  selected_message_seq = 0
  selected_message_rev = 0
  message_action = "toolbar"
  message_edit_draft = ""
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
  // Both composers above are new empty boxes and the rail is gone. `connected`
  // goes false here, which only MUTES the chord — the stale claim would ride
  // straight through the reconnect and mark a rebuilt draft on the first
  // Cmd+B after it lands.
  composer_focus = "none"
  pending_channel = ""
  pending_message = ""
  chat_search_hits = []
  chat_search_generation = chat_search_generation + 1
  chat_searching = false
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
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  active_thread_target = ""
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
  block_autosave_status = "idle"
  page_delete_armed = false
  page_search_hits = []
  page_search_generation = page_search_generation + 1
  page_searching = false
  error = ""
  status = "Connecting…"
  connect_generation = connect_generation + 1
  run connect(connected_rpc, hydration_retry_attempt, connect_generation) -> workspace_connected _ | connect_failed _

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
  unread_boundary = 0
  messages = merge_pending_messages(next.messages, messages, active_channel, next.active_channel, "")
  has_older_history = history_has_older(messages)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  active_channel = next.active_channel
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
  huddle_channel = keep_str(huddle_joined, active_channel, "")
  huddle_channel_name = keep_str(huddle_joined, active_channel_name, "")
  channel_members = next.channel_members
  pages = next.pages
  blocks = merge_pending_blocks(next.blocks, blocks, buffer_page, next.active_page, "")
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  // The blocks in hand are this page's, and every route into a connect blanks
  // the buffer first — so this is the page the document state belongs to.
  buffer_page = next.active_page
  connected = true
  loading = false
  mutation_phase = "idle"
  hydration_retry_attempt = 0
  error = ""
  bell_generation = bell_generation + 1
  explorer_generation = explorer_generation + 1
  fs_generation = fs_generation + 1
  members_generation = members_generation + 1
  gov_generation = gov_generation + 1
  agents_generation = agents_generation + 1
  account_generation = account_generation + 1
  forge_generation = forge_generation + 1
  settings_generation = settings_generation + 1
  node_peers_generation = node_peers_generation + 1
  node_facts_generation = node_facts_generation + 1
  dm_peers_generation = dm_peers_generation + 1
  parallel
    run load_doc_tabs(connected_rpc) -> doc_tabs_loaded _
    run load_dm_peers(connected_rpc, dm_peers_generation) -> dm_peers_loaded _ | dm_peers_failed _
    run load_node_facts(connected_rpc, node_facts_generation) -> node_facts_loaded _ | node_facts_failed _
    run load_bell(connected_rpc, bell_generation) -> bell_loaded _ | bell_failed _
    run load_explorer(connected_rpc, explorer_generation) -> explorer_loaded _ | explorer_failed _
    run files_ls(connected_rpc, fs_path, fs_generation) -> fs_listed _ | fs_failed _
    run load_members(connected_rpc, members_generation) -> members_loaded _ | members_failed _
    run load_governance(connected_rpc, gov_generation) -> governance_loaded _ | governance_failed _
    run load_settings_facts(connected_rpc, settings_generation) -> settings_loaded _ | settings_failed _
    run load_peers(connected_rpc, node_peers_generation) -> peers_loaded _ | peers_failed _
    run load_agents(connected_rpc, agents_generation) -> agents_loaded _ | agents_failed _
    run load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _
    run load_forge(connected_rpc, forge_generation) -> forge_loaded _ | forge_failed _
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
  send_flash = (send_settled_by(messages, next.chat, active_channel)) || (reply_settled_by(thread_messages, next.chat, active_channel))
  send_flash_id = settled_send_id(messages, next.chat, active_channel, send_flash_id)
  thread_send_flash_id = settled_reply_id(thread_messages, next.chat, active_channel, thread_send_flash_id)
  messages = apply_chat_messages(messages, next.chat, active_channel)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  thread_messages = apply_chat_thread(thread_messages, next.chat, active_channel, active_thread_seq)
  channel_members = apply_chat_members(channel_members, next.chat, active_channel)
  thread_next_reply_offset = thread_offset_after_live(thread_next_reply_offset, thread_has_more, next.chat, active_channel, active_thread_seq)
  active_channel_name = channel_display_name(channels, active_channel, active_channel_name)
  active_channel_archived = channel_flag_archived(channels, active_channel, active_channel_archived)
  active_channel_members_only = channel_flag_members_only(channels, active_channel, active_channel_members_only)
  active_channel_huddle_count = channel_live_huddle_count(channels, active_channel, active_channel_huddle_count)
  channel_reads = mark_channel_read(channel_reads, active_channel, channel_head_seq(channels, active_channel))
  // THE PAGES FOLD. A committed text edit lands in the open document's blocks
  // with no query at all — the autosave commits one per tick while a reader
  // types, and each used to buy three sequential reads of the very document
  // being typed into. A delta for a block this list does not hold is a no-op.
  //
  // The buffer moves on the SAME shared decision the resync path uses below:
  // the canonical text replaces it only when the editor is CLEAN and actually
  // differs, so a reader mid-sentence keeps their words and their caret, and
  // their own settled echo is a no-op because the baseline already matches.
  blocks = apply_page_text(blocks, next.pages)
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
    run live_resync_load(connected_rpc, active_channel, active_page, resync_planes((next.load_chat || huddle_refresh_hits(next.chat, active_channel)), next.load_pages), next.debounce, hydration_generation, 0) -> live_resynced _ | live_resync_failed _
    run forge_live_refresh(connected_rpc, forge_repo, forge_item_number, next.kind, next.module, next.forge, forge_generation) -> forge_refreshed _ | forge_failed _
    // The `keep_i64(shell_tab == …, gen, -1)` on the heavy three is the SAME
    // off-screen refusal the tab-switch path uses: the backend refuses a
    // generation of -1 and the failed arm's guard drops the refusal unread, so
    // an Approvals tab nobody is looking at costs a dead call, not a query.
    // Members, account and the DM directory stay ungated — all three feed
    // always-visible chrome.
    run load_members(connected_rpc, keep_i64(plane_live_hit(next.kind, next.module, "valset"), members_generation, -1)) -> members_loaded _ | members_failed _
    run load_governance(connected_rpc, keep_i64(plane_live_hit(next.kind, next.module, "governance") && shell_tab == "governance", gov_generation, -1)) -> governance_loaded _ | governance_failed _
    run load_account(connected_rpc, keep_i64(plane_live_hit(next.kind, next.module, "identity"), account_generation, -1)) -> account_loaded _ | account_failed _
    run load_dm_peers(connected_rpc, keep_i64(plane_live_hit(next.kind, next.module, "identity"), dm_peers_generation, -1)) -> dm_peers_loaded _ | dm_peers_failed _
    run load_agents(connected_rpc, keep_i64(plane_live_hit(next.kind, next.module, "agent") && shell_tab == "agents", agents_generation, -1)) -> agents_loaded _ | agents_failed _
    run files_ls(connected_rpc, fs_path, keep_i64(plane_live_hit(next.kind, next.module, "files") && shell_tab == "files", fs_generation, -1)) -> fs_listed _ | fs_failed _

on live_resynced(next)
  return if next.generation != hydration_generation
  hydration_retry_attempt = 0
  channels = keep_channels(next.chat_loaded, next.channels, channels)
  channel_reads = initial_channel_reads(channels, channel_reads)
  messages = keep_messages(next.chat_loaded, merge_pending_messages(next.messages, messages, active_channel, next.active_channel, ""), messages)
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
  failed_message_draft = remember_failed_draft(failed_message_draft, message_action, message_edit_draft, selected_message_seq > 0 || message_action != "editing")
  selected_message_rev = message_seq_after_failure(selected_message_rev, "message-edit", selected_message_seq <= 0)
  message_action = message_action_after_failure(message_action, "message-edit", selected_message_seq <= 0)
  message_edit_draft = message_text_after_failure(message_edit_draft, "message-edit", selected_message_seq <= 0)
  channel_settings_open = channel_settings_open && active_channel == keep_str(next.chat_loaded, next.active_channel, active_channel)
  channel_name_draft = retain_for_endpoint(channel_name_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  member_key_draft = retain_for_endpoint(member_key_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  thread_generation = thread_generation_after_refresh(thread_generation, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq, refreshed_known_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq))
  thread_loading = thread_loading_after_refresh(thread_loading, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq, refreshed_known_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq))
  failed_reply_draft = retain_for_endpoint(failed_reply_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  active_thread_seq = refreshed_known_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), active_thread_seq)
  failed_reply_draft = remember_failed_draft(failed_reply_draft, "thread", reply_draft, active_thread_seq > 0)
  thread_target_seq = refreshed_channel_value(active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), thread_target_seq)
  thread_next_reply_offset = refreshed_channel_value(active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), thread_next_reply_offset)
  thread_messages = retain_thread_messages(thread_messages, active_thread_seq)
  thread_has_more = thread_has_more && active_channel == keep_str(next.chat_loaded, next.active_channel, active_channel) && active_thread_seq > 0
  reply_draft = retain_for_endpoint(reply_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  pending_reply = retain_for_endpoint(pending_reply, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  message_draft = retain_for_endpoint(message_draft, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  message_editor = editor(message_draft)
  pending_message = retain_for_endpoint(pending_message, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel))
  active_channel = keep_str(next.chat_loaded, next.active_channel, active_channel)
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
  huddle_channel = keep_str(huddle_joined, active_channel, "")
  huddle_channel_name = keep_str(huddle_joined, active_channel_name, "")
  channel_members = keep_members(next.chat_loaded, next.channel_members, channel_members)
  unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, active_channel, unread_boundary)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  channel_reads = mark_channel_read(channel_reads, active_channel, channel_head_seq(channels, active_channel))
  // A resync carries whatever page was active WHEN IT WAS ISSUED and takes
  // several queries to answer, so a mutation landing in between leaves it
  // speaking for a document nobody is on — measured on a page create. The page
  // LIST is never stale (it is the whole index either way) and still lands
  // unconditionally; everything scoped to ONE page waits for a reply that
  // answers for the page in hand.
  pages_answer_is_current = next.pages_loaded && pages_reply_answers_current(next.pages, next.active_page, active_page)
  pages = keep_pages(next.pages_loaded, next.pages, pages)
  blocks = keep_blocks(pages_answer_is_current, merge_pending_blocks(next.blocks, blocks, buffer_page, next.active_page, ""), blocks)
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
  active_page_title = keep_str(pages_answer_is_current, next.active_page_title, active_page_title)
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
  error = ""
  block_comments_generation = block_comments_generation + 1
  live_thread_generation = live_thread_generation + 1
  // The rail's live refresh must ask the SAME question the rail was filled
  // from. `refresh_block_comments` asks `ThreadsForTargets` for the target
  // ALONE, so with a page target it found only page-anchored threads and wiped
  // every block-anchored one out of the open rail on the next pages event.
  // `load_page_threads` fans out over the page AND its blocks, and answers on
  // the handler pages.ice already routes its own loads through. Both routes
  // ignore a closed rail, so a page event with no rail open costs one refused
  // query and never touches the banner.
  //
  // The list is all that refreshes live. An OPEN thread's replies do not: a task
  // group must be the final statement in a handler, so the comment-page load
  // cannot be guarded on `active_block_comment_thread`, and firing it unguarded
  // asks the node for thread "" — whose failure paints `block_comment_page_failed`
  // over the rail every time anyone edits the page. Replies still arrive on post
  // and on reopen; a page-scoped comment refresh in backend.rs closes the gap.
  parallel
    run refresh_live_thread(connected_rpc, active_channel, active_thread_seq, thread_target_seq, thread_next_reply_offset, live_thread_generation) -> live_thread_refreshed _ | live_thread_refresh_failed _
    run load_page_threads(connected_rpc, block_comments_target, block_comments_generation) -> block_threads_loaded _ | block_threads_failed _
    // Same close-if-ended mirror as `workspace_connected` — this is the fold
    // the steady state pays (a roster change forces a chat resync into here).
    task window close target=window_target_unless(huddle_joined, huddle_win)

on live_resync_failed(cause)
  return if cause.generation != hydration_generation
  status = "Sync delayed"
  error = "Live sync interrupted. Retrying…"
  hydration_retry_attempt = hydration_retry_attempt + 1
  run live_resync_load(connected_rpc, active_channel, active_page, "both", false, hydration_generation, hydration_retry_attempt) -> live_resynced _ | live_resync_failed _

on live_thread_refreshed(next)
  return if next.generation != live_thread_generation
  return if next.channel_id != active_channel || next.root_seq != active_thread_seq
  return if thread_loading || mutation_phase != "idle"
  thread_target_seq = next.target_seq
  thread_messages = merge_pending_messages(next.messages, thread_messages, active_channel, next.channel_id, "")
  thread_next_reply_offset = next.next_reply_offset
  thread_has_more = next.has_more

on live_thread_refresh_failed(cause)
  return if cause.generation != live_thread_generation

on select_shell_tab(next)
  shell_tab = next
  // A TAB MOVE UNMOUNTS THE CHAT COMPOSERS, so the caret they claimed is gone
  // — and a `shell_tab == "chat"` term on the chord could only mute it while
  // she is away, then hand the stale claim straight back on the return trip.
  // Every handler that writes `shell_tab` retires it, linted in tests.rs.
  composer_focus = "none"
  // Leaving Chat prunes paged-in scrollback to one load's worth: the return
  // trip cold-rebuilds every mounted row in one frame, so the mount cost must
  // not compound with how far she once paged back. "Load older" re-earns it.
  messages = trim_timeline_on_leave(next, messages)
  has_older_history = history_has_older(messages)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  // A hydration error belongs to the pane that raised it. Leaving it up after
  // a navigation tells the user the pane they just opened is broken, which is
  // a lie the banner has no way to walk back — it is dismissed by hand or not
  // at all. Clearing here, ABOVE both early returns, is what makes that true
  // for every tab: the `!connected` return and the chat/pages return each skip
  // the generation bumps below, but neither should keep a stale banner alive.
  error = ""
  return if !connected
  return if shell_tab == "chat" || shell_tab == "pages"
  explorer_generation = explorer_generation + 1
  fs_generation = fs_generation + 1
  members_generation = members_generation + 1
  gov_generation = gov_generation + 1
  agents_generation = agents_generation + 1
  account_generation = account_generation + 1
  forge_generation = forge_generation + 1
  settings_generation = settings_generation + 1
  node_peers_generation = node_peers_generation + 1
  node_facts_generation = node_facts_generation + 1
  explorer_loading = shell_tab == "explorer"
  fs_loading = shell_tab == "files"
  // No `files_find` here. This block runs for EVERY tab but chat and pages, so
  // a whole-workspace prefix walk was issued on the way into Settings, Forge,
  // Members and Agents — for a `files_tree` no view reads, on a route whose
  // failure paints the GLOBAL error banner over a screen with no file operation
  // in sight. `fs_wrote` still refreshes the tree from inside the files tab.
  //
  // The HEAVY loaders load only for their own tab: forge walks a git mirror
  // per repo, explorer flattens the ops of 100 blocks, files walks the tree —
  // all three used to run on the way into Settings or Members. A `keep_i64`
  // sends the off-screen ones generation -1; the backend refuses it and the
  // failed arm's generation guard drops the refusal unread. The light loaders
  // stay unconditional: members/governance/agents feed the always-visible
  // titlebar chips and have no other refresh path.
  parallel
    run load_node_facts(connected_rpc, node_facts_generation) -> node_facts_loaded _ | node_facts_failed _
    run load_explorer(connected_rpc, keep_i64(shell_tab == "explorer", explorer_generation, -1)) -> explorer_loaded _ | explorer_failed _
    run files_ls(connected_rpc, fs_path, keep_i64(shell_tab == "files", fs_generation, -1)) -> fs_listed _ | fs_failed _
    run files_history(connected_rpc, keep_i64(shell_tab == "files", fs_generation, -1)) -> fs_history_loaded _ | fs_failed _
    run load_members(connected_rpc, members_generation) -> members_loaded _ | members_failed _
    run load_governance(connected_rpc, gov_generation) -> governance_loaded _ | governance_failed _
    run load_settings_facts(connected_rpc, settings_generation) -> settings_loaded _ | settings_failed _
    run load_peers(connected_rpc, node_peers_generation) -> peers_loaded _ | peers_failed _
    run load_agents(connected_rpc, agents_generation) -> agents_loaded _ | agents_failed _
    run load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _
    run load_forge(connected_rpc, keep_i64(shell_tab == "forge", forge_generation, -1)) -> forge_loaded _ | forge_failed _

// The huddle's elapsed clock is a LOCAL session fact: one tick per second for
// as long as SHE is in the huddle, never a chain value. `huddle_joined_at` is
// stamped from `huddle_now` when she joins, so mm:ss is their difference.
on tick
  huddle_now = huddle_now + 1

// The app has exactly ONE subscribe block. Component handler files may not
// declare another, so every new subscription lands here.
//
// The node log stream follows the node body into Settings — the rail seat it
// used to key on is gone, and a predicate that can never be true again would
// have taken the log console dark without a word.
subscribe
  run live_events(connected_rpc) when connected -> live_updated _
  // THE CALL SESSION IS THIS SUBSCRIPTION. Joining a huddle flips
  // `huddle_joined` and the media leg connects; leaving (or disconnecting)
  // stops the subscription, the stream drops, and the websocket + audio
  // threads tear down with it. No imperative start/stop anywhere.
  run call_session(connected_rpc, huddle_channel) when (connected && huddle_joined && !empty(huddle_channel)) -> call_event _
  // Video has NO subscription: the tile strip is a self-redrawing widget
  // that repaints only its own window at the capture cadence.
  keyboard press when (connected || palette_open) -> global_key_pressed _
  // THE PANE SCROLL'S KEYS ARE THE LEFTOVERS. `status=ignored` drops every key
  // a focused widget CONSUMED — Home in a text field, an arrow in an open
  // list — but it is only half the arbitration: iced's single-line input drops
  // Up/Down uncaptured, so the router itself refuses the arrows and every key
  // under an open overlay (`content_scroll_step`). Ungated on purpose: the
  // launch and huddle windows mount no content pane, and a scroll operation
  // whose target is not on screen is a no-op.
  keyboard press status=ignored -> content_scroll_key _
  keyboard modifiers -> modifiers_changed _
  window file-dropped -> fs_file_dropped _
  // A daemon outlives its windows, so process exit is an explicit decision:
  // when the LAST tracked window closes, leave.
  window closed with-id -> window_was_closed _
  run node_logs(connected_rpc) when (connected && shell_tab == "settings" && node_tab == "activity") -> node_log_line _
  every 1s when huddle_joined -> tick
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

on modifiers_changed(value)
  shift_held = value.shift

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
  // The bump discards the in-flight save's reply, so ITS status reset can
  // never arrive — bump and reset are one inseparable pair, or the "saving"
  // guard holds the tick forever.
  block_autosave_generation = cancel_autosaves(connected_rpc, block_autosave_generation)
  block_autosave_status = "idle"
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  run live_resync_load(connected_rpc, active_channel, active_page, "both", false, hydration_generation, 0) -> live_resynced _ | live_resync_failed _

on dismiss_error
  error = ""

on restore_failed_message
  return if empty(failed_message_draft) || !empty(trim(editor_text(message_editor))) || mutation_phase != "idle"
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
  run connect(connected_rpc, hydration_retry_attempt, connect_generation) -> workspace_connected _ | connect_failed _

on failed(cause)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = false
  status = "Offline"
  error = cause.message
