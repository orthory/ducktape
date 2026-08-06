state
  app_theme = "app"
  // Which palette reading of AppTheme paints the console. `appearance` is the
  // persisted override: "" follows the OS, "light"/"dark" pin one reading.
  // `app_background`/`app_text` are the WINDOW's own base coat — they must
  // flip with the palette or a dark console boots inside a paper window.
  app_palette:palette[AppTheme] = AppTheme.app
  appearance = ""
  app_background = "#fdfdfb"
  app_text = "#2c2b27"
  rpc = ""
  connected_rpc = ""
  password = ""
  status = "Connecting…"
  connected = false
  loading = false
  // Live shift state from the `keyboard modifiers` subscription — the rich
  // composer classifies Enter with it (plain Enter sends, ⇧↵ breaks a line).
  shift_held = false
  block_height:i64 = -1
  hydration_generation:i64 = 0
  hydration_retry_attempt:i64 = 0
  mutation_phase = "idle"
  error = ""
  channels:[ChatChannel] = []
  messages:[ChatMessage] = []
  channel_reads:[ChannelRead] = []
  unread_boundary:i64 = 0
  // The seq wearing the "New" divider — first_unread_seq(messages,
  // unread_boundary), recomputed WHERE those two change, never in the view:
  // called inside `for message in messages` the extern's by-value ABI deep-
  // cloned the whole timeline once per row, per frame (O(n²) allocations).
  unread_marker_seq:i64 = 0
  active_channel = ""
  active_channel_name = ""
  active_channel_archived = false
  active_channel_members_only = false
  active_channel_huddle_count:i64 = 0
  channel_members:[ChatMember] = []
  channel_settings_open = false
  channel_name_draft = ""
  member_key_draft = ""
  selected_message_seq:i64 = 0
  chat_pointer_y = 0.0
  chat_height = 720.0
  message_menu_y = 0.0
  message_action_focus = ""
  selected_message_rev:i64 = 0
  message_action = "toolbar"
  message_edit_draft = ""
  active_thread_seq:i64 = 0
  thread_target_seq:i64 = 0
  thread_messages:[ChatMessage] = []
  thread_next_reply_offset:i64 = 0
  thread_has_more = false
  thread_loading = false
  thread_generation:i64 = 0
  live_thread_generation:i64 = 0
  thread_selected_seq:i64 = 0
  thread_selected_rev:i64 = 0
  thread_message_action = "toolbar"
  thread_edit_draft = ""
  thread_pointer_y = 0.0
  thread_height = 720.0
  thread_menu_y = 0.0
  history_loading = false
  history_generation:i64 = 0
  reply_draft = ""
  pending_reply = ""
  pending_reply_id = ""
  channel_draft = ""
  channel_create_open = false
  channel_create_members_only = false
  pending_channel = ""
  message_draft = ""
  message_editor:editor = ""
  reply_editor:editor = ""
  pending_message = ""
  pending_message_id = ""
  // The transient settle ✓: `send_flash_id` anchors it to the row whose
  // optimistic send just landed, `send_flash` drives its opacity — true pops
  // it in on the settle delta, the NEXT live event flips it false and fades
  // it out. No timer: consensus itself is the metronome (blocks keep coming).
  send_flash_id = ""
  // Its thread-rail twin: anchors the same fade to a settled REPLY row. One
  // shared `send_flash` drives both opacities — two lanes settling in the
  // same beat share one fade, which reads fine and needs no second animation.
  thread_send_flash_id = ""
  send_flash:animation[bool] = false
    easing ease-in-out
    duration 400ms
  failed_message_draft = ""
  failed_reply_draft = ""
  chat_search_draft = ""
  chat_search_hits:[ChatSearchHit] = []
  chat_searching = false
  chat_search_generation:i64 = 0
  history_view = false
  // Whether older pages exist below the loaded timeline — mirrored on every
  // write that can move the list's OLDEST row, because computing it in the
  // view means deep-cloning the whole timeline through the extern ABI on
  // every single frame.
  has_older_history = false
  shell_tab = "chat"
  explorer_blocks:[ExplorerBlock] = []
  explorer_ops:[ExplorerOp] = []
  explorer_generation:i64 = 0
  explorer_loading = false
  explorer_selected:i64 = 0
  // ANSWERED = the plane's loader returned at least once (rows or refusal).
  // The empty plates guard on these: "No members yet" is a lie while the
  // first load is still in flight. Set by the loaded AND failed arms.
  members_answered = false
  agents_answered = false
  gov_answered = false
  forge_answered = false
  members_rows:[MemberRow] = []
  members_validators:i64 = 0
  members_residents:i64 = 0
  members_generation:i64 = 0
  gov_rows:[ProposalRow] = []
  gov_generation:i64 = 0
  gov_voting = ""
  agents_rows:[AgentRow] = []
  agents_generation:i64 = 0
  forge_repos:[ForgeRepo] = []
  forge_repo = ""
  forge_branches:[str] = []
  forge_items:[ForgeItem] = []
  forge_item_number:i64 = 0
  forge_item_title = ""
  forge_item_state = ""
  forge_item_kind = ""
  forge_item_body = ""
  forge_item_author = ""
  forge_item_branches = ""
  forge_item_channel = ""
  forge_item_source_branch = ""
  forge_item_source_oid = ""
  forge_item_target_oid = ""
  forge_item_merge_oid = ""
  forge_item_diff = ""
  forge_item_diff_truncated:bool = false
  forge_item_files_changed:i64 = 0
  forge_item_additions:i64 = 0
  forge_item_deletions:i64 = 0
  forge_item_reviews:[ForgeReview] = []
  forge_item_approvals:i64 = 0
  forge_item_change_requests:i64 = 0
  forge_review_verdict = "comment"
  forge_review_draft = ""
  forge_review_busy:bool = false
  // The line the diff gutter last picked, and the comment being written for it.
  // A picked line is the composer's whole visibility condition — an empty
  // `forge_comment_path` means no line is open, so there is no separate flag.
  forge_comment_path = ""
  forge_comment_line = ""
  forge_comment_side = ""
  forge_comment_draft = ""
  // Staged and NOT yet on the wire: a review carries its line comments in the
  // same transaction as its body, so these ride along at submit.
  forge_comment_staged:[ForgeDraftComment] = []
  forge_merge_busy:bool = false
  forge_merge_conflicts:[str] = []
  forge_discussion:[ChatMessage] = []
  forge_discussion_members:[ChatMember] = []
  forge_discussion_editor:editor = ""
  forge_discussion_pending = ""
  forge_discussion_generation:i64 = 0
  forge_generation:i64 = 0
  settings_endpoint = ""
  settings_node_key = ""
  settings_height:i64 = 0
  settings_data_dir = ""
  settings_key_path = ""
  settings_key_state = ""
  // THIS DEVICE'S OWN USER KEY, full hex — the `me` the post gate tests against
  // `channel_members`. `account_id` cannot serve: it is a short_label of the
  // identity module's account id, not the key a membership row carries.
  settings_user_key = ""
  settings_open_tabs:i64 = 0
  settings_generation:i64 = 0
  account_bound = false
  account_id = ""
  account_name = ""
  account_bio = ""
  account_members:i64 = 0
  account_nodes:i64 = 0
  account_generation:i64 = 0
  account_name_draft = ""
  account_renaming = false
  node_log_lines:[NodeLogLine] = []
  node_log_filter = ""
  node_peers:[PeerRow] = []
  node_peers_generation:i64 = 0
  fs_path = "/shared"
  fs_entries:[FsEntry] = []
  fs_generation:i64 = 0
  fs_loading = false
  fs_preview_path = ""
  fs_preview_text = ""
  fs_preview_truncated = false
  fs_preview_binary = false
  fs_history:[FsSnapshot] = []
  fs_history_open = false
  fs_new_name = ""
  fs_delete_target = ""
  fs_editor:editor = ""
  fs_editing = false
  fs_diff_from = ""
  fs_diff:[FsDiffEntry] = []
  palette_open = false
  bell_open = false
  bell_unread:i64 = 0
  bell_items:[BellItem] = []
  bell_generation:i64 = 0
  palette_draft = ""
  palette_key = ""
  palette_generation:i64 = 0
  palette_searching = false
  palette_chat_hits:[ChatSearchHit] = []
  palette_page_hits:[PageSearchHit] = []
  pages:[PageItem] = []
  doc_tabs:[str] = []
  closing_doc_tab = ""
  blocks:[PageBlock] = []
  active_page = ""
  active_page_title = ""
  active_page_parent = ""
  page_draft = ""
  page_create_open = false
  pending_page = ""
  block_comments_open = false
  block_comments_target = ""
  block_comments_generation:i64 = 0
  block_comment_threads:[PageCommentThread] = []
  block_comment_thread_total:i64 = 0
  block_comment_threads_next_from:i64 = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  block_thread_comments:[PageComment] = []
  block_thread_comments_next_from:i64 = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""
  // THE PAGE IS ONE BUFFER. `page_editor` holds the whole document; its drift
  // from `page_saved_text` (the last text known written) IS the dirty signal,
  // because the editor's own edits never pass through a handler. The save tick
  // only exists while that drift does.
  page_editor:editor = ""
  page_saved_text = ""
  // Scratch pair for the one-decision buffer install (E151: a run-route
  // payload's fields do not type inside `let`, so the decision lands here).
  page_landing = ""
  page_install = false
  page_inflight_text = ""
  // Why a write was NOT attempted — see DocumentSaveResult. Cleared by the
  // next edit, because it describes an edit that has already been undone.
  page_refusal = ""
  block_autosave_status = "idle"
  block_autosave_generation:i64 = 0
  orphaned_comment_drafts:[str] = []
  page_delete_armed = false
  page_search_draft = ""
  page_search_hits:[PageSearchHit] = []
  page_searching = false
  page_search_generation:i64 = 0
  // THE THREE WINDOWS — the daemon's whole routing state. The launch window
  // opens on mount; the console opens on a network pick; the huddle opens
  // when the call is popped out. Whichever id a window carries decides what
  // it renders, and when the launch window and the console are both `none`
  // the process exits — a lone huddle window never keeps the daemon alive.
  onboarding_win:window-id? = none
  console_win:window-id? = none
  huddle_win:window-id? = none
  // The titlebar's chain label, computed ONCE per connection (and on the
  // account fold, its fallback) — `network_label` scans the workspaces dir
  // and parses tomls, which is a per-frame disk tax when called from a view
  // mount. The view reads this mirror instead.
  network_name = ""
  // THE LAUNCH FLOW — one discriminant: loading -> (create | unlock)
  // -> [reveal | restore] -> networks -> [join -> provisioning -> live].
  hub_step = "loading"
  hub_key_state = ""
  hub_networks:[HubNetwork] = []
  hub_selected = ""
  hub_hidden:i64 = 0
  hub_probe_generation:i64 = 0
  // The 24 recovery words, held ONLY between key creation and the "I saved
  // them" confirm — never persisted, cleared on leaving the reveal step.
  reveal_words = ""
  onboarding_name = ""
  onboarding_invite = ""
  onboarding_error = ""
  invite_link = ""
  workspace_slug = ""
  provision_settled = false
  // NODE FACTS — everything /v1/status already publishes that the app dropped.
  // Backs the status-pill hover card, the FINALITY/ROUND cards and the gc line.
  node_facts_generation:i64 = 0
  node_version = ""
  node_root_hash = ""
  // The consensus trio is NOT here: it lives in handlers/lifecycle.ice as the
  // already-rendered `node_view_label` / `node_quorum_label` /
  // `node_reachable_label`, because /v1/status reports each as optional and an
  // absent reading must print `—`, never a measured `0`.
  node_last_finalized:i64 = 0
  node_checkpoint:i64 = 0
  node_tab = "overview"
  status_card_open = false
  // ROSTER — members and agents share one screen, so they share one filter.
  members_filter = "all"
  members_selected = ""
  agents_selected = ""
  // FORGE — which tracker list, which repo menu, which half of an item.
  // Code is the artifact's first seat, and opening a repo to its file tree is
  // what "open a repo" means; landing on Pull requests answered a question the
  // reader had not asked yet.
  forge_tab = "code"
  forge_repo_menu = false
  // DIRECT — a DM is a two-party members-only channel; `active_dm_peer` names
  // the peer and `dm_peers` carries the rest of him, so the header plate is a
  // filter over that list. There is deliberately no `active_dm_name` /
  // `active_dm_is_agent` pair: nothing ever wrote them, and a header fed from
  // two fields no handler fills renders a blank name.
  dm_peers_generation:i64 = 0
  active_dm_peer = ""
  // HUDDLE — whether SHE is in it, where, since when, the tick that drives the
  // elapsed clock, and who else is on the call. There is no `popped` bool:
  // the huddle window's own existence (`huddle_win`) is that state.
  huddle_joined = false
  huddle_channel = ""
  huddle_channel_name = ""
  huddle_joined_at:i64 = 0
  huddle_now:i64 = 0
  // The live call session's surface: status prose, the local mute, and the
  // peers' 1 Hz beacons (kind="peer" rows keyed by node key).
  call_status = ""
  call_muted = false
  call_peers:[CallEvent] = []
  call_steered = false
  call_camera = false
  call_frame_generation:i64 = 0
  // Whether any camera in the call is live — the tile strip's mount gate and
  // the 15 Hz repaint tick's own gate.
  call_video_live = false
  huddle_roster:[HuddleParticipant] = []
  // The event inspector every finality mark opens.
  // EXPLORER — one query across every module, filtered by result kind.
  explorer_query = ""
  explorer_kind = "all"
  files_selected = ""
  // Overlays: the mention popup, the invite modal, the toast, the arm-then-act
  // guard on destructive buttons.
  invite_role = "resident"
  invite_ttl:i64 = 7
  toast = ""
  toast_tone = "info"
  toast_age:i64 = 0
  leave_armed = false
  // MOTION — deliberately none. A `repeat forever` animation is a one-way
  // ratchet in the runtime: lilt reports it animating from its first
  // transition until process exit, which holds the window-frames subscription
  // open and rebuilds the ENTIRE view at display refresh rate — here for
  // pixels no view read (`spin`/`overlay_in` were never written, `pulse` was
  // never rendered). Motion returns when the runtime gates frames on
  // animations a view actually consumes.
