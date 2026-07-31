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

// FIRST RUN ASKS DISK, NOT THE DEFAULT. `phase` used to initialize to
// "console" and `mount` went straight to `connect`, so a device with no
// workspace booted an empty shell over a connection error and the five
// onboarding screens were reachable only by `Leave workspace`. The disk answer
// is the discriminant: no registered, unforgotten workspace means "welcome".
on mount
  phase = onboarding_phase()
  return if phase != "console"
  loading = true
  run connect(rpc) -> workspace_connected _ | failed _

on reconnect
  return if loading || (mutation_phase != "idle" && mutation_phase != "recovering")
  rpc = canonical_endpoint(rpc)
  block_autosave_generation = cancel_autosaves(connected_rpc, block_autosave_generation)
  password = retain_for_endpoint(password, connected_rpc, rpc)
  channel_draft = retain_for_endpoint(channel_draft, connected_rpc, rpc)
  message_draft = retain_for_endpoint(trim(editor_text(message_editor)), connected_rpc, rpc)
  message_editor = editor(message_draft)
  failed_message_draft = retain_for_endpoint(failed_message_draft, connected_rpc, rpc)
  failed_reply_draft = retain_for_endpoint(failed_reply_draft, connected_rpc, rpc)
  chat_search_draft = retain_for_endpoint(chat_search_draft, connected_rpc, rpc)
  page_draft = retain_for_endpoint(page_draft, connected_rpc, rpc)
  block_draft = retain_for_endpoint(block_draft, connected_rpc, rpc)
  page_search_draft = retain_for_endpoint(page_search_draft, connected_rpc, rpc)
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, [], selected_block_id, block_edit_draft, block_autosave_status)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, [], selected_block_id, block_comment_draft)
  orphaned_block_drafts = retain_drafts_for_endpoint(orphaned_block_drafts, connected_rpc, rpc)
  orphaned_comment_drafts = retain_drafts_for_endpoint(orphaned_comment_drafts, connected_rpc, rpc)
  connected_rpc = rpc
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "idle"
  loading = true
  connected = false
  channels = []
  messages = []
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
  hovered_message_seq = 0
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
  pending_block = ""
  block_insert_after_id = ""
  block_insert_open = !empty(block_draft)
  selected_block_id = ""
  hovered_block_id = ""
  selected_block_kind = ""
  selected_block_checked = false
  block_actions_open = false
  block_comments_generation = block_comments_generation + 1
  block_comments_open = false
  block_comments_target = ""
  block_comment_threads = []
  block_comment_thread_total = 0
  block_comment_threads_next_from = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  block_thread_comments = []
  block_thread_comments_next_from = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  page_title_selected = false
  block_edit_draft = ""
  block_autosave_status = "idle"
  page_delete_armed = false
  block_delete_armed = false
  page_search_hits = []
  page_search_generation = page_search_generation + 1
  page_searching = false
  error = ""
  status = "Connecting…"
  run connect(connected_rpc) -> workspace_connected _ | failed _

on workspace_connected(next)
  rpc = next.rpc
  connected_rpc = next.rpc
  status = next.status
  block_height = next.height
  channels = next.channels
  channel_reads = initial_channel_reads(next.channels, channel_reads)
  unread_boundary = 0
  messages = merge_pending_messages(next.messages, messages, active_channel, next.active_channel, "")
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  active_channel = next.active_channel
  active_channel_name = next.active_channel_name
  active_channel_archived = next.active_channel_archived
  active_channel_members_only = next.active_channel_members_only
  active_channel_huddle_count = next.active_channel_huddle_count
  channel_members = next.channel_members
  pages = next.pages
  blocks = merge_pending_blocks(next.blocks, blocks, active_page, next.active_page, "")
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
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

on live_updated(next)
  status = next.status
  return if next.kind == "retry"
  block_height = keep_i64(next.height >= 0, next.height, block_height)
  channels = apply_chat_channels(channels, next.chat)
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
  bell_unread = bell_unread_after(bell_unread, bell_items, next.bell)
  bell_items = apply_bell(bell_items, next.bell)
  forge_discussion = apply_chat_messages(forge_discussion, next.chat, forge_item_channel)
  return if !forge_live_hit(next.kind, next.module) && !next.load_chat && !next.load_pages
  forge_generation = keep_i64(forge_live_hit(next.kind, next.module), forge_generation + 1, forge_generation)
  hydration_generation = keep_i64(next.load_chat || next.load_pages, hydration_generation + 1, hydration_generation)
  hydration_retry_attempt = keep_i64(next.load_chat || next.load_pages, 0, hydration_retry_attempt)
  parallel
    run live_resync_load(connected_rpc, active_channel, active_page, resync_planes(next.load_chat, next.load_pages), next.debounce, hydration_generation, 0) -> live_resynced _ | live_resync_failed _
    run forge_live_refresh(connected_rpc, forge_repo, forge_item_number, next.kind, next.module, next.forge, forge_generation) -> forge_refreshed _ | forge_failed _

on live_resynced(next)
  return if next.generation != hydration_generation
  hydration_retry_attempt = 0
  channels = keep_channels(next.chat_loaded, next.channels, channels)
  channel_reads = initial_channel_reads(channels, channel_reads)
  messages = keep_messages(next.chat_loaded, merge_pending_messages(next.messages, messages, active_channel, next.active_channel, ""), messages)
  failed_message_draft = remember_failed_draft(failed_message_draft, "channel", message_draft, active_channel == keep_str(next.chat_loaded, next.active_channel, active_channel))
  selected_message_seq = refreshed_required_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), selected_message_seq)
  failed_message_draft = remember_failed_draft(failed_message_draft, message_action, message_edit_draft, selected_message_seq > 0 || message_action != "editing")
  selected_message_rev = message_seq_after_failure(selected_message_rev, "message-edit", selected_message_seq <= 0)
  message_action = message_action_after_failure(message_action, "message-edit", selected_message_seq <= 0)
  message_edit_draft = message_text_after_failure(message_edit_draft, "message-edit", selected_message_seq <= 0)
  hovered_message_seq = refreshed_required_message_seq(messages, active_channel, keep_str(next.chat_loaded, next.active_channel, active_channel), hovered_message_seq)
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
  channel_members = keep_members(next.chat_loaded, next.channel_members, channel_members)
  unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, active_channel, unread_boundary)
  unread_marker_seq = first_unread_seq(messages, unread_boundary)
  channel_reads = mark_channel_read(channel_reads, active_channel, channel_head_seq(channels, active_channel))
  pages = keep_pages(next.pages_loaded, next.pages, pages)
  blocks = keep_blocks(next.pages_loaded, merge_pending_blocks(next.blocks, blocks, active_page, next.active_page, ""), blocks)
  orphaned_block_drafts = remember_orphaned_block_drafts(orphaned_block_drafts, blocks, selected_block_id, block_edit_draft, block_autosave_status)
  orphaned_comment_drafts = remember_orphaned_comment_drafts(orphaned_comment_drafts, blocks, selected_block_id, block_comment_draft)
  block_edit_draft = refreshed_block_draft(blocks, selected_block_id, block_edit_draft, block_autosave_status)
  block_autosave_generation = cancel_missing_block_autosave(connected_rpc, block_autosave_generation, blocks, selected_block_id)
  selected_block_id = refreshed_selected_block(blocks, selected_block_id)
  selected_block_kind = retain_selected_string(selected_block_kind, selected_block_id)
  selected_block_checked = selected_block_checked && !empty(selected_block_id)
  // THE COMMENTS RAIL IS DOCUMENT-SCOPED (handlers/pages.ice:300). Its anchor is
  // the PAGE it was opened on, never a block selection — keyed on
  // `selected_block_id` it closed itself, and threw the half-typed comment away,
  // the moment the user clicked off the block whose ⋮ menu opened it. So the
  // target is the one thing reconciled against the page identity here, and every
  // other rail field keys on the target: one line decides the whole rail.
  block_comments_target = retain_for_endpoint(block_comments_target, active_page, keep_str(next.pages_loaded, next.active_page, active_page))
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
  block_edit_draft = retain_selected_string(block_edit_draft, selected_block_id)
  block_delete_armed = block_delete_armed && !empty(selected_block_id)
  block_actions_open = block_actions_open && !empty(selected_block_id)
  block_insert_open = block_insert_open && active_page == keep_str(next.pages_loaded, next.active_page, active_page)
  block_insert_after_id = refreshed_selected_block(blocks, block_insert_after_id)
  page_delete_armed = page_delete_armed && active_page == keep_str(next.pages_loaded, next.active_page, active_page)
  page_title_selected = page_title_selected && active_page == keep_str(next.pages_loaded, next.active_page, active_page)
  active_page = keep_str(next.pages_loaded, next.active_page, active_page)
  active_page_title = keep_str(next.pages_loaded, next.active_page_title, active_page_title)
  active_page_parent = keep_str(next.pages_loaded, next.active_page_parent, active_page_parent)
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
  keyboard press when (connected || palette_open) -> global_key_pressed _
  keyboard modifiers -> modifiers_changed _
  window file-dropped -> fs_file_dropped _
  run node_logs(connected_rpc) when (connected && shell_tab == "settings" && node_tab == "activity") -> node_log_line _
  every 1s when huddle_joined -> tick
  every 2800ms when !empty(toast) -> dismiss_toast

on modifiers_changed(value)
  shift_held = value.shift

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
  block_autosave_generation = cancel_autosaves(connected_rpc, block_autosave_generation)
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

on failed(cause)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = false
  status = "Offline"
  error = cause.message
