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

on mount
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
    run load_bool_pref(connected_rpc, "receipts") -> receipts_pref_loaded _
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
  parallel
    run load_node_facts(connected_rpc, node_facts_generation) -> node_facts_loaded _ | node_facts_failed _
    run load_explorer(connected_rpc, explorer_generation) -> explorer_loaded _ | explorer_failed _
    run files_ls(connected_rpc, fs_path, fs_generation) -> fs_listed _ | fs_failed _
    run files_history(connected_rpc, fs_generation) -> fs_history_loaded _ | fs_failed _
    run load_members(connected_rpc, members_generation) -> members_loaded _ | members_failed _
    run load_governance(connected_rpc, gov_generation) -> governance_loaded _ | governance_failed _
    run load_settings_facts(connected_rpc, settings_generation) -> settings_loaded _ | settings_failed _
    run load_peers(connected_rpc, node_peers_generation) -> peers_loaded _ | peers_failed _
    run load_agents(connected_rpc, agents_generation) -> agents_loaded _ | agents_failed _
    run load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _
    run load_forge(connected_rpc, forge_generation) -> forge_loaded _ | forge_failed _

on forge_loaded(next)
  return if next.generation != forge_generation
  forge_repos = next.repos

on forge_failed(cause)
  return if cause.generation != forge_generation

// Picking a repo also DISMISSES the switcher. Nothing else clears it on this
// route, so the popover stayed pinned over the first rows of the tracker list
// the user just navigated to, with the crumb as the only way out.
on forge_open_repo(name)
  return if !connected
  forge_repo_menu = false
  forge_repo = name
  forge_item_number = 0
  forge_item_diff = ""
  forge_generation = forge_generation + 1
  run load_forge_repo(connected_rpc, forge_repo, forge_generation) -> forge_repo_loaded _ | forge_failed _

on forge_repo_loaded(next)
  return if next.generation != forge_generation
  forge_repo = next.repo
  forge_branches = next.branches
  forge_items = next.items

on forge_open_item(number)
  return if !connected || empty(forge_repo)
  forge_item_number = number
  forge_review_verdict = "comment"
  forge_review_draft = ""
  forge_merge_conflicts = []
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
  forge_discussion_editor = editor("")
  forge_generation = forge_generation + 1
  run load_forge_item(connected_rpc, forge_repo, forge_item_number, forge_generation) -> forge_item_loaded _ | forge_failed _

on forge_item_loaded(next)
  return if next.generation != forge_generation
  forge_item_number = next.number
  forge_item_title = next.title
  forge_item_state = next.state
  forge_item_kind = next.kind
  forge_item_body = next.body
  forge_item_author = next.author_name
  forge_item_branches = next.branches
  forge_item_channel = next.channel_id
  forge_item_source_branch = next.source_branch
  forge_item_source_oid = next.source_oid
  forge_item_target_oid = next.target_oid
  forge_item_merge_oid = next.merge_oid
  forge_item_diff = next.diff
  forge_item_diff_truncated = next.diff_truncated
  forge_item_files_changed = next.files_changed
  forge_item_additions = next.additions
  forge_item_deletions = next.deletions
  forge_item_reviews = next.reviews
  forge_item_approvals = next.approvals
  forge_item_change_requests = next.change_requests
  forge_discussion_generation = forge_discussion_generation + 1
  return if empty(forge_item_channel)
  run load_forge_discussion(connected_rpc, forge_item_channel, forge_discussion_generation) -> forge_discussion_loaded _ | forge_discussion_failed _

on forge_discussion_loaded(next)
  return if next.generation != forge_discussion_generation || next.channel_id != forge_item_channel
  forge_discussion = next.messages
  forge_discussion_members = next.members

on forge_discussion_failed(cause)
  return if cause.generation != forge_discussion_generation
  error = cause.message

on forge_review_pick(verdict)
  forge_review_verdict = verdict

on forge_review_submit
  return if !connected || forge_review_busy || empty(forge_repo) || forge_item_number <= 0
  forge_review_busy = true
  run submit_forge_review(connected_rpc, password, forge_repo, forge_item_number, forge_review_verdict, forge_review_draft, forge_item_source_oid) -> forge_review_submitted _ | forge_review_failed _

on forge_review_submitted(_result)
  forge_review_busy = false
  forge_review_draft = ""
  forge_review_verdict = "comment"
  error = ""

on forge_review_failed(cause)
  forge_review_busy = false
  error = cause.message

on forge_merge_submit
  return if !connected || forge_merge_busy || empty(forge_repo) || forge_item_number <= 0
  forge_merge_busy = true
  forge_merge_conflicts = []
  run merge_forge_pr(connected_rpc, password, forge_repo, forge_item_number, forge_item_source_branch, forge_item_source_oid, forge_item_target_oid) -> forge_merged _ | forge_merge_failed _

on forge_merged(next)
  return if next.repo != forge_repo || next.number != forge_item_number
  forge_merge_busy = false
  forge_merge_conflicts = next.conflicts
  error = ""

on forge_merge_failed(cause)
  forge_merge_busy = false
  error = cause.message

on forge_note_submit
  return if loading || !connected || empty(forge_item_channel) || !empty(forge_discussion_pending) || empty(trim(editor_text(forge_discussion_editor)))
  forge_discussion_pending = fresh_operation_id("forge-note")
  run send_message(connected_rpc, password, forge_item_channel, forge_discussion_pending, trim(editor_text(forge_discussion_editor)), forge_discussion_members) -> forge_note_sent _ | forge_note_failed _

on forge_note_sent(next)
  return if next.channel_id != forge_item_channel
  forge_discussion_pending = ""
  forge_discussion_editor = editor("")
  error = ""

on forge_note_failed(cause)
  return if cause.scope_id != forge_item_channel
  forge_discussion_pending = ""
  error = cause.message

on forge_refreshed(next)
  return if next.generation != forge_generation
  forge_repos = keep_forge_repos(next.repos_loaded, next.repos, forge_repos)
  forge_branches = keep_branches(next.repo_loaded, next.branches, forge_branches)
  forge_items = keep_forge_items(next.repo_loaded, next.items, forge_items)
  forge_item_title = keep_str(next.item_loaded, next.item.title, forge_item_title)
  forge_item_state = keep_str(next.item_loaded, next.item.state, forge_item_state)
  forge_item_kind = keep_str(next.item_loaded, next.item.kind, forge_item_kind)
  forge_item_body = keep_str(next.item_loaded, next.item.body, forge_item_body)
  forge_item_author = keep_str(next.item_loaded, next.item.author_name, forge_item_author)
  forge_item_branches = keep_str(next.item_loaded, next.item.branches, forge_item_branches)
  forge_item_channel = keep_str(next.item_loaded, next.item.channel_id, forge_item_channel)
  forge_item_source_branch = keep_str(next.item_loaded, next.item.source_branch, forge_item_source_branch)
  forge_item_source_oid = keep_str(next.item_loaded, next.item.source_oid, forge_item_source_oid)
  forge_item_target_oid = keep_str(next.item_loaded, next.item.target_oid, forge_item_target_oid)
  forge_item_merge_oid = keep_str(next.item_loaded, next.item.merge_oid, forge_item_merge_oid)
  forge_item_diff = keep_str(next.item_loaded, next.item.diff, forge_item_diff)
  forge_item_diff_truncated = keep_bool(next.item_loaded, next.item.diff_truncated, forge_item_diff_truncated)
  forge_item_files_changed = keep_i64(next.item_loaded, next.item.files_changed, forge_item_files_changed)
  forge_item_additions = keep_i64(next.item_loaded, next.item.additions, forge_item_additions)
  forge_item_deletions = keep_i64(next.item_loaded, next.item.deletions, forge_item_deletions)
  forge_item_reviews = keep_forge_reviews(next.item_loaded, next.item.reviews, forge_item_reviews)
  forge_item_approvals = keep_i64(next.item_loaded, next.item.approvals, forge_item_approvals)
  forge_item_change_requests = keep_i64(next.item_loaded, next.item.change_requests, forge_item_change_requests)

// The breadcrumb home. Nothing else clears `forge_repo`, so without this the
// repo grid is unreachable for the rest of the session once a repo is opened.
on forge_close_repo
  forge_repo = ""
  forge_branches = []
  forge_items = []
  forge_repo_menu = false
  forge_item_number = 0
  forge_item_diff = ""
  forge_item_channel = ""
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
  forge_merge_conflicts = []

on forge_toggle_repo_menu
  forge_repo_menu = !forge_repo_menu

on forge_close_item
  forge_item_number = 0
  forge_item_diff = ""
  forge_item_channel = ""
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
  forge_merge_conflicts = []

on account_loaded(next)
  return if next.generation != account_generation
  account_bound = next.bound
  account_id = next.account_id
  account_name = next.display_name
  account_bio = next.bio
  account_members = next.members
  account_nodes = next.nodes

on account_failed(cause)
  return if cause.generation != account_generation

on account_name_draft_changed(next)
  account_name_draft = next

on account_rename_submit
  return if !connected || !account_bound || account_renaming || empty(trim(account_name_draft))
  account_renaming = true
  error = ""
  run set_account_name(connected_rpc, password, trim(account_name_draft)) -> account_renamed _ | account_rename_failed _

on account_renamed(_result)
  account_renaming = false
  account_name_draft = ""
  account_generation = account_generation + 1
  run load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _

on account_rename_failed(cause)
  account_renaming = false
  error = cause.message

on agents_loaded(next)
  return if next.generation != agents_generation
  agents_rows = next.agents
  // `pulse` is the console's only breathing dot and it repeats forever, so a
  // live agent is the one fact that starts it — and the ABSENCE of one is the
  // fact that has to stop it. Written on both edges: an early return here would
  // leave the dot lit for the rest of the session after the last agent pauses.
  pulse = 0.0
  return if !any_agent_active(next.agents)
  pulse = 1.0

on agents_failed(cause)
  return if cause.generation != agents_generation

on node_log_line(line)
  node_log_lines = push_log_line(node_log_lines, line)

on node_log_filter_changed(next)
  node_log_filter = next

on peers_loaded(next)
  return if next.generation != node_peers_generation
  node_peers = next.peers

on peers_failed(cause)
  return if cause.generation != node_peers_generation

// The consensus facts /v1/status already publishes and the console dropped:
// app-hash, view, quorum, reachable validators, finality and the gc watermark.
//
// `view`, `quorum` and `reachable_validators` arrive as `i64?` — a resident
// publishes no consensus block at all — and Ice cannot read an optional into an
// `i64`. `optional_number` is the seam: it renders the number, or `—` when the
// node genuinely has no reading, so the console prints an absence as an absence
// instead of as a measured zero.
on node_facts_loaded(next)
  return if next.generation != node_facts_generation
  node_root_hash = next.root_hash
  node_last_finalized = next.last_finalized_at
  node_checkpoint = next.checkpoint_height
  node_view_label = optional_number(next.view)
  node_quorum_label = optional_number(next.quorum)
  node_reachable_label = optional_number(next.reachable_validators)

on node_facts_failed(cause)
  return if cause.generation != node_facts_generation

// Overview | Permissions | Activity, inside Settings now that the Node rail
// seat is gone. The log stream below subscribes on this tab.
on select_node_tab(tab)
  node_tab = tab

on settings_loaded(next)
  return if next.generation != settings_generation
  settings_endpoint = next.endpoint
  settings_node_key = next.node_key
  settings_height = next.height
  settings_key_path = next.key_path
  settings_key_state = next.key_state
  settings_open_tabs = next.open_tabs

on settings_failed(cause)
  return if cause.generation != settings_generation

on settings_clear_tabs
  doc_tabs = []
  run clear_doc_tabs(connected_rpc) -> doc_tabs_saved _

// PREFERENCES — device-local, one endpoint at a time.
on receipts_pref_loaded(enabled)
  pref_receipts = enabled

on toggle_receipts_pref
  pref_receipts = !pref_receipts
  run save_bool_pref(connected_rpc, "receipts", pref_receipts) -> receipts_pref_saved _

on receipts_pref_saved(saved)
  return if saved
  error = "This device could not save the preference."

// DANGER ZONE — forget this workspace on THIS DEVICE and go back to onboarding.
on forget_workspace_submit
  return if !connected || mutation_phase != "idle"
  mutation_phase = "forget-workspace"
  error = ""
  run forget_workspace(connected_rpc) -> workspace_forgotten _ | mutation_failed _

// `forget_workspace` answers false when the prefs file could not be written.
// Throwing her out to onboarding on that answer meant the workspace was back in
// the picker at the next launch, looking like the app had ignored her.
on workspace_forgotten(forgotten)
  mutation_phase = "idle"
  error = "This device could not forget the workspace."
  return if !forgotten
  connected = false
  status = "Not connected"
  error = ""
  phase = "welcome"

// The app's one clipboard action: every Copy button routes here so the toast
// copy lives at the call site and the write itself stays native.
on copy_to_clipboard(text, label)
  toast = label
  toast_tone = "info"
  task clipboard write text

on dismiss_toast
  toast = ""

on governance_loaded(next)
  return if next.generation != gov_generation
  gov_rows = next.proposals

on governance_failed(cause)
  return if cause.generation != gov_generation

on gov_vote(proposal_id, approve)
  return if !connected || !empty(gov_voting)
  gov_voting = proposal_id
  run governance_vote(connected_rpc, password, gov_voting, approve) -> gov_acted _ | gov_act_failed _

on gov_execute(proposal_id)
  return if !connected || !empty(gov_voting)
  gov_voting = proposal_id
  run governance_execute(connected_rpc, password, gov_voting) -> gov_acted _ | gov_act_failed _

// The quorum-gated membership actions the roster detail panel offers. They
// share `gov_voting` with vote/execute: one governance write is in flight.
on gov_propose(action, target_key)
  return if !connected || !empty(gov_voting)
  gov_voting = target_key
  run governance_propose(connected_rpc, password, action, gov_voting) -> gov_acted _ | gov_act_failed _

on gov_acted(_result)
  gov_voting = ""
  gov_generation = gov_generation + 1
  run load_governance(connected_rpc, gov_generation) -> governance_loaded _ | governance_failed _

on gov_act_failed(cause)
  gov_voting = ""
  error = cause.message

on members_loaded(next)
  return if next.generation != members_generation
  members_rows = next.members
  members_validators = next.validators
  members_residents = next.residents

on members_failed(cause)
  return if cause.generation != members_generation

// The DIRECT peer directory. Loaded with the workspace, because the sidebar
// section that reads it is on screen from the first frame.
on dm_peers_loaded(next)
  return if next.generation != dm_peers_generation
  dm_peers = next.peers

on dm_peers_failed(cause)
  return if cause.generation != dm_peers_generation

// ROSTER — one screen, one filter, one detail panel. An empty key closes it.
on open_member(key)
  members_selected = key

on pick_members_filter(filter)
  members_filter = filter

// The invite modal is pure view state — minting is a separate, explicit act.
on open_invite_modal
  invite_modal_open = true

on close_invite_modal
  invite_modal_open = false

// Pause or resume an agent. The payload is the DESIRED state and it is named
// for the backend parameter it becomes: `true` PAUSES, `false` resumes. The
// roster's Pause control passes `true` and its Resume control passes `false`;
// a row wired from `agent.status` would have to invert. Only its owner may ask:
// the view offers this on `is_mine` rows, and the node refuses anyone else.
on agent_set_status(agent_id, paused)
  return if !connected
  run set_agent_status(connected_rpc, password, agent_id, paused) -> agent_status_set _ | mutation_failed _

on agent_status_set(_result)
  agents_generation = agents_generation + 1
  error = ""
  run load_agents(connected_rpc, agents_generation) -> agents_loaded _ | agents_failed _

on fs_open_dir(path)
  return if fs_loading || !connected
  fs_path = path
  fs_generation = fs_generation + 1
  fs_loading = true
  fs_preview_path = ""
  fs_preview_text = ""
  run files_ls(connected_rpc, fs_path, fs_generation) -> fs_listed _ | fs_failed _

on fs_open_parent
  return if fs_loading || !connected || empty(fs_path)
  fs_path = fs_parent(fs_path)
  fs_generation = fs_generation + 1
  fs_loading = true
  fs_preview_path = ""
  fs_preview_text = ""
  run files_ls(connected_rpc, fs_path, fs_generation) -> fs_listed _ | fs_failed _

on fs_open_file(path)
  return if fs_loading || !connected
  fs_preview_path = path
  fs_generation = fs_generation + 1
  run files_preview(connected_rpc, fs_preview_path, fs_generation) -> fs_previewed _ | fs_failed _

on fs_toggle_history
  fs_history_open = !fs_history_open

on fs_listed(next)
  return if next.generation != fs_generation
  fs_loading = false
  fs_path = next.path
  fs_entries = next.entries

on fs_previewed(next)
  return if next.generation != fs_generation
  fs_preview_path = next.path
  fs_preview_text = next.text
  fs_preview_truncated = next.truncated
  fs_preview_binary = next.binary

on fs_history_loaded(next)
  return if next.generation != fs_generation
  fs_history = next.snapshots

// The whole path list behind the tree sidebar — a prefix walk, not a listing.
on fs_tree_loaded(next)
  return if next.generation != fs_generation
  files_tree = next.entries

on fs_failed(cause)
  return if cause.generation != fs_generation
  fs_loading = false
  error = cause.message

on fs_new_name_changed(next)
  fs_new_name = next

on fs_mkdir_submit
  return if fs_loading || !connected || empty(trim(fs_new_name))
  fs_loading = true
  error = ""
  run files_mkdir(connected_rpc, fs_child(fs_path, trim(fs_new_name))) -> fs_wrote _ | fs_write_failed _

on fs_new_file_submit
  return if fs_loading || !connected || empty(trim(fs_new_name))
  fs_loading = true
  error = ""
  run files_write_text(connected_rpc, fs_child(fs_path, trim(fs_new_name)), "") -> fs_wrote _ | fs_write_failed _

on fs_arm_delete(path)
  fs_delete_target = path

on fs_delete_submit
  return if fs_loading || !connected || empty(fs_delete_target)
  fs_loading = true
  error = ""
  run files_remove(connected_rpc, fs_delete_target) -> fs_wrote _ | fs_write_failed _

on fs_begin_edit
  return if fs_preview_binary || empty(fs_preview_path)
  fs_editing = true
  fs_editor = editor(fs_preview_text)

on fs_cancel_edit
  fs_editing = false

on fs_save_edit
  return if fs_loading || !connected || !fs_editing || empty(fs_preview_path)
  fs_loading = true
  fs_editing = false
  fs_preview_text = editor_text(fs_editor)
  error = ""
  run files_write_text(connected_rpc, fs_preview_path, editor_text(fs_editor)) -> fs_wrote _ | fs_write_failed _

on fs_wrote(_result)
  fs_new_name = ""
  fs_delete_target = ""
  fs_generation = fs_generation + 1
  fs_loading = true
  parallel
    run files_ls(connected_rpc, fs_path, fs_generation) -> fs_listed _ | fs_failed _
    run files_find(connected_rpc, "", fs_generation) -> fs_tree_loaded _ | fs_failed _
    run files_history(connected_rpc, fs_generation) -> fs_history_loaded _ | fs_failed _

on fs_write_failed(cause)
  fs_loading = false
  error = cause.message

on fs_file_dropped(path)
  return if shell_tab != "files" || fs_loading || !connected
  fs_loading = true
  error = ""
  run files_upload(connected_rpc, fs_path, path) -> fs_wrote _ | fs_write_failed _

on fs_show_diff(from)
  return if fs_loading || !connected
  fs_diff_from = from
  fs_generation = fs_generation + 1
  run files_diff(connected_rpc, fs_diff_from, fs_generation) -> fs_diffed _ | fs_failed _

on fs_close_diff
  fs_diff_from = ""
  fs_diff = []

on fs_diffed(next)
  return if next.generation != fs_generation
  fs_diff = next.entries

on refresh_explorer
  return if !connected || explorer_loading
  explorer_generation = explorer_generation + 1
  explorer_loading = true
  run load_explorer(connected_rpc, explorer_generation) -> explorer_loaded _ | explorer_failed _

on explorer_loaded(next)
  return if next.generation != explorer_generation
  explorer_loading = false
  explorer_blocks = next.blocks
  explorer_ops = next.ops

on explorer_failed(cause)
  return if cause.generation != explorer_generation
  explorer_loading = false
  error = cause.message

on select_explorer_block(height)
  explorer_selected = height

on toggle_palette
  return if !connected
  palette_open = !palette_open
  palette_draft = ""
  palette_chat_hits = []
  palette_page_hits = []
  palette_generation = palette_generation + 1
  palette_searching = false
  return if !palette_open
  task widget focus #workspace-tabs/palette-input

on close_palette
  palette_open = false

// Opening the bell only opens it. Marking read is the Mark-all-read button's
// job — doing it here cleared the badge and every unread row before the list
// painted, and left that button with nothing to do.
on toggle_bell
  bell_open = !bell_open

on close_bell
  bell_open = false

on mark_bell_read_submit
  return if bell_unread <= 0
  run mark_bell_read(connected_rpc, password, bell_head(bell_items)) -> bell_marked _ | mutation_failed _

on bell_loaded(next)
  return if next.generation != bell_generation
  bell_unread = next.unread
  bell_items = next.items

on bell_failed(cause)
  return if cause.generation != bell_generation

on bell_marked(_result)
  error = error

on global_key_pressed(event)
  palette_key = palette_key_action(event.key, event.physical_key, event.modifiers, palette_open)
  return if palette_key == "none"
  return if palette_key == "open" && !connected
  palette_open = palette_key == "open"
  palette_key = ""
  palette_draft = ""
  palette_chat_hits = []
  palette_page_hits = []
  palette_generation = palette_generation + 1
  palette_searching = false
  return if !palette_open
  task widget focus #workspace-tabs/palette-input

on palette_changed(next)
  palette_draft = next
  palette_generation = palette_generation + 1
  palette_searching = !empty(trim(palette_draft))
  return if empty(trim(palette_draft))
  parallel
    run search_chat(connected_rpc, "", trim(palette_draft), palette_generation) -> palette_chat_loaded _ | palette_search_failed _
    run search_pages(connected_rpc, "", trim(palette_draft), palette_generation) -> palette_page_loaded _ | palette_search_failed _

on palette_chat_loaded(next)
  return if next.generation != palette_generation
  palette_chat_hits = next.hits
  palette_searching = false

on palette_page_loaded(next)
  return if next.generation != palette_generation
  palette_page_hits = next.hits
  palette_searching = false

on palette_search_failed(cause)
  return if cause.generation != palette_generation
  palette_searching = false

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
  window file-dropped -> fs_file_dropped _
  run node_logs(connected_rpc) when (connected && shell_tab == "settings" && node_tab == "activity") -> node_log_line _
  every 1s when huddle_joined -> tick
  every 2800ms when !empty(toast) -> dismiss_toast

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
  task widget focus #workspace-tabs/content/reply

on dismiss_failed_reply
  failed_reply_draft = ""

on failed(cause)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  loading = false
  status = "Offline"
  error = cause.message
