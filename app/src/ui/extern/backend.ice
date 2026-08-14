extern crate::backend
  LoadRequest(rpc:str, key:str, generation:i64)
  pure load_request(condition:bool, rpc:str, key:str, generation:i64) -> LoadRequest?
  ChatChannel(id:str, name:str, archived:bool, members_only:bool, huddle_count:i64, head_seq:i64)
  ChatReaction(emoji:str, count:i64, reacted_by_me:bool)
  ChatMember(key:str, label:str)
  ChannelRead(channel:str, seq:i64)
  ChannelSwitchFacts(unread_boundary:i64, name:str, archived:bool, members_only:bool)
  ChatSpan(mention:str, link_text:str, link:str, bold_italic:str, bold:str, italic:str, plain:str)
  ChatBlock(kind:str, text:str, lang:str, rich:bool, spans:[ChatSpan])
  ChatMessage(id:str, view_key:i64, seq:i64, author:str, meta:str, body:str, blocks:[ChatBlock], pending:bool, rev:i64, edited:bool, deleted:bool, reply_count:i64, thread_seq:i64, show_author:bool, initial:str, avatar_kind:str, height:i64, time:i64, reactions:[ChatReaction], render_rev:i64)
  MessageSelection(seq:i64, rev:i64, action:MessageAction, draft:str)
  HuddleParticipant(key:str, label:str, initials:str, is_agent:bool, is_you:bool, joined_at:i64, node:str)
  ChannelDraft(channel_id:str, text:str)
  ChatData(generation:i64, channels:[ChatChannel], messages:[ChatMessage], has_older_history:bool, active_channel:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, huddle_roster:[HuddleParticipant], channel_members:[ChatMember], selected_message_seq:i64, selected_message_rev:i64, selected_message_body:str, active_thread_seq:i64, thread_target_seq:i64, thread_messages:[ChatMessage], thread_has_more:bool)
  SendReceipt(operation_id:str, channel_id:str)
  ChatDelta()
  PagesDelta(kind:str, block_id:str, text:str)
  LiveRefresh(generation:i64, fold_serial:i64, chat_loaded:bool, channels:[ChatChannel], messages:[ChatMessage], has_older_history:bool, active_channel:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, huddle_roster:[HuddleParticipant], channel_members:[ChatMember], pages_loaded:bool, pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str, comment_thread_total:i64, commented_block_hits:[str])
  ThreadLoadData(generation:i64, root_seq:i64, target_seq:i64, messages:[ChatMessage], next_reply_seq:i64, has_more:bool)
  ThreadPageData(generation:i64, messages:[ChatMessage], next_reply_seq:i64, has_more:bool)
  LiveThreadData(channel_id:str, root_seq:i64, messages:[ChatMessage])
  HistoryPageData(channel_id:str, messages:[ChatMessage], has_more:bool)
  ChatSearchHit(channel_id:str, seq:i64, root_seq:i64, author:str, text:str, meta:str)
  ChatSearchData(hits:[ChatSearchHit])
  PageItem(id:str, title:str, parent:str, prefix:str, child_count:i64)
  PageBlock(key:i64, id:str, parent:str, kind:str, text:str, pending:bool, checked:bool, prefix:str, child_count:i64)
  PagesData(pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str, comment_thread_total:i64, commented_block_hits:[str])
  PageCommentThread(id:str, target:str, author:str, meta:str, resolved:bool, comment_count:i64)
  PageComment(id:str, ordinal:i64, author:str, meta:str, text:str)
  BlockThreadListData(generation:i64, target:str, from:i64, threads:[PageCommentThread], total:i64, next_from:i64, has_more:bool)
  BlockCommentData(generation:i64, target:str, thread_id:str, from:i64, comments:[PageComment], next_from:i64, has_more:bool)
  PageSearchHit(page_id:str, page_title:str, block_id:str, kind:str, text:str)
  PageSearchData(hits:[PageSearchHit])
  PaletteSearchData(chat_hits:[ChatSearchHit], page_hits:[PageSearchHit])
  // `refusal` is not a failure: the write was NOT attempted because carrying it
  // out would have destroyed records. `document` is the canonical text either
  // way — the buffer takes it, which is what rolls an illegal edit back.
  DocumentSaveResult(written:bool, refusal:str, data:PagesData, document:str)
  WorkspaceData(generation:i64, rpc:str, status:str, height:i64, channels:[ChatChannel], messages:[ChatMessage], has_older_history:bool, active_channel:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, huddle_roster:[HuddleParticipant], channel_members:[ChatMember], pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str, comment_thread_total:i64, commented_block_hits:[str])
  BellItem(seq:i64, kind:str, body:str, source:str, height:i64, read:bool)
  BellDelta(kind:str, item:BellItem, up_to_seq:i64)
  BellData(unread:i64, items:[BellItem])
  pure apply_bell(items:[BellItem], delta:BellDelta) -> [BellItem]
  pure bell_unread_after(unread:i64, items:[BellItem], delta:BellDelta) -> i64
  pure bell_head(items:[BellItem]) -> i64
  pure bell_severity(kind:str) -> str
  pure bell_title(kind:str) -> str
  pure bell_worst_severity(items:[BellItem]) -> str
  load_bell(rpc:str) -> BellData ! AppError
  mark_bell_read(rpc:str, password:str, up_to_seq:i64) -> bool ! AppError
  ForgeRefresh(repo:str, number:i64, refs_moved:bool)
  LiveUpdate(kind:LiveKind, status:str, height:i64, module:str, load_chat:bool, load_pages:bool, debounce:bool, chat:[ChatDelta], pages:PagesDelta, bell:BellDelta, forge:ForgeRefresh)
  ChatLiveFold(messages_changed:bool, thread_messages_changed:bool, has_older_history:bool, selected_message_seq:i64, selected_message_rev:i64, message_action:MessageAction, message_edit_draft:str, thread_selected_seq:i64, thread_selected_rev:i64, thread_message_action:MessageAction, thread_edit_draft:str, channels:[ChatChannel], messages:[ChatMessage], thread_messages:[ChatMessage], channel_members:[ChatMember], channel_reads:[ChannelRead], rooms:[ChatSidebarRow], dm_rows:[DmSidebarRow], unread_marker_seq:i64, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, post_refusal:str, forge_discussion:[ChatMessage], refresh_chat:bool)
  AppError(message:str, committed:bool)
  AgentTerminalSession()
  AgentTerminalNotice(running:bool, title:str)
  AgentTerminalStarted(session:AgentTerminalSession, title:str)
  AgentCredential(name:str, provider:str)
  AgentCredentialsData(generation:i64, rows:[AgentCredential])
  AgentChatEntry(id:i64, role:str, body:str, provider:str)
  AgentActivity(id:i64, title:str, detail:str, status:str)
  AgentChatEvent(id:i64, kind:str, title:str, detail:str, status:str, answer:str, saga_id:str)
  pure idle_agent_terminal() -> AgentTerminalSession
  start_agent_terminal(rpc:str, provider:str, credential:str) -> AgentTerminalStarted ! AppError
  task focus_agent_terminal(session:AgentTerminalSession) -> unit
  component agent_terminal_surface(session:&AgentTerminalSession) -> unit
  component agent_markdown(source:str, dark:bool) -> str
  subscription agent_terminal_events(session:AgentTerminalSession) -> AgentTerminalNotice
  load_agent_credentials(rpc:str, generation:i64) -> AgentCredentialsData ! HydrationError
  pure agent_credential_names(rows:[AgentCredential], provider:str) -> [str]
  pure agent_credential_choice(rows:[AgentCredential], provider:str, current:str) -> str
  pure agent_provider_label(provider:str) -> str
  pure agent_provider_initial(provider:str) -> str
  pure agent_credential_caption(provider:str, credential:str) -> str
  pure agent_register_hint(provider:str) -> str
  pure agent_composer_hint(provider:str) -> str
  pure agent_chat_push_user(entries:[AgentChatEntry], body:str, provider:str) -> [AgentChatEntry]
  pure agent_chat_finish(entries:[AgentChatEntry], body:str, provider:str) -> [AgentChatEntry]
  pure agent_activity_apply(rows:[AgentActivity], event:AgentChatEvent) -> [AgentActivity]
  pure agent_event_status(current:str, event:AgentChatEvent) -> str
  pure agent_event_detail(current:str, event:AgentChatEvent) -> str
  pure agent_event_saga(current:str, event:AgentChatEvent) -> str
  pure agent_event_live(current:str, event:AgentChatEvent) -> str
  pure agent_event_error(current:str, event:AgentChatEvent) -> str
  pure agent_event_busy(event:AgentChatEvent) -> bool
  pure agent_event_entries(entries:[AgentChatEntry], event:AgentChatEvent, provider:str) -> [AgentChatEntry]
  stream agent_chat_turn(rpc:str, provider:str, credential:str, entries:[AgentChatEntry]) -> AgentChatEvent
  OptimisticMutationError(message:str, committed:bool, operation_id:str, scope_id:str, body:str)
  HydrationError(generation:i64, message:str)
  box-style raised_style()
  svg-style icon_tint(tone:str)
  pure icon(name:str) -> bytes
  connect(rpc:str, attempt:i64, generation:i64) -> WorkspaceData ! HydrationError
  stream live_events(rpc:str) -> LiveUpdate
  pure fold_live_chat(deltas:[ChatDelta], channels:[ChatChannel], messages:[ChatMessage], thread_messages:[ChatMessage], channel_members:[ChatMember], channel_reads:[ChannelRead], dm_peers:[DmPeer], me:str, active_channel:str, active_thread_seq:i64, history_view:bool, chat_visible:bool, unread_boundary:i64, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, forge_discussion:[ChatMessage], forge_item_channel:str, selected_message_seq:i64, selected_message_rev:i64, message_action:MessageAction, message_edit_draft:str, thread_selected_seq:i64, thread_selected_rev:i64, thread_message_action:MessageAction, thread_edit_draft:str) -> ChatLiveFold
  pure resync_planes(load_chat:bool, load_pages:bool) -> str
  live_resync_load(rpc:str, channel_id:str, page_id:str, planes:str, debounce:bool, generation:i64, fold_serial:i64, attempt:i64) -> LiveRefresh ! HydrationError
  load_older_messages(rpc:str, channel_id:str, before_seq:i64) -> HistoryPageData ! AppError
  sync fresh_operation_id(prefix:str) -> str
  sync optimistic_message(messages:[ChatMessage], body:str, message_id:str) -> [ChatMessage]
  sync optimistic_thread_message(messages:[ChatMessage], body:str, message_id:str) -> [ChatMessage]
  pure mark_author_runs(messages:[ChatMessage]) -> [ChatMessage]
  pure merge_pending_messages(canonical:[ChatMessage], current:[ChatMessage], current_channel:str, next_channel:str) -> [ChatMessage]
  pure merge_landing_messages(canonical:[ChatMessage], current:[ChatMessage], current_channel:str, next_channel:str) -> [ChatMessage]
  pure merge_thread_refresh(canonical:[ChatMessage], current:[ChatMessage], current_channel:str, next_channel:str) -> [ChatMessage]
  pure resynced_messages(loaded:bool, next:[ChatMessage], current:[ChatMessage], current_channel:str, next_channel:str) -> [ChatMessage]
  pure rollback_pending_message(messages:[ChatMessage], pending_id:str, committed:bool) -> [ChatMessage]
  pure contains_pending_message(messages:[ChatMessage], pending_id:str) -> bool
  pure reaction_applied(messages:[ChatMessage], seq:i64, emoji:str, added:bool) -> [ChatMessage]
  pure append_thread_page(messages:[ChatMessage], next:[ChatMessage]) -> [ChatMessage]
  pure history_has_older(messages:[ChatMessage]) -> bool
  pure oldest_message_seq(messages:[ChatMessage]) -> i64
  pure prepend_history(messages:[ChatMessage], older:[ChatMessage]) -> [ChatMessage]
  pure message_selection_after_window(messages:[ChatMessage], seq:i64, rev:i64, action:MessageAction, draft:str) -> MessageSelection
  pure merge_pending_blocks(canonical:[PageBlock], current:[PageBlock], current_page:str, next_page:str, settled_id:str) -> [PageBlock]
  pure restore_draft(current:str, pending:str, keep_pending:bool) -> str
  // Chat's message/thread menus still place themselves this way; the name is
  // the pages block menu it was written for, which no longer exists.
  pure block_action_menu_y(pointer_y:f64, viewport_height:f64) -> f64
  pure append_page_comment_threads(threads:[PageCommentThread], next:[PageCommentThread]) -> [PageCommentThread]
  pure append_page_comments(comments:[PageComment], next:[PageComment]) -> [PageComment]
  pure remember_failed_draft(existing:str, current:str, pending:str, committed:bool) -> str
  sync canonical_endpoint(input:str) -> str
  WorkspaceInit(chain_id:str, workspace:str, rpc:str)
  join_network(blob:secret) -> WorkspaceInit ! AppError
  mint_invite(workspace:str, ttl_days:i64) -> str ! AppError
  ProvisionStep(index:i64, label:str, state:str, settled:bool)
  stream provision_progress(workspace:str, rpc:str) -> ProvisionStep
  HubNetwork(id:str, chain_id:str, name:str, endpoint:str, kind:str, last_used:i64, probed:bool, live:bool, height:i64)
  HubProbe(id:str, live:bool, height:i64)
  HubState(key_state:str, networks:[HubNetwork], preselect:str, hidden:i64)
  KeyCreated(words:str, pubkey:str)
  hub_state() -> HubState
  stream probe_known_networks() -> HubProbe
  pure apply_network_probe(networks:[HubNetwork], probe:HubProbe) -> [HubNetwork]
  pure network_run_hint(row:HubNetwork) -> str
  pure hub_entry_step(key_state:str) -> HubStep
  pure selected_network_endpoint(networks:[HubNetwork], id:str) -> str
  pure refreshed_hub_selection(networks:[HubNetwork], current:str, preselect:str) -> str
  pure password_problem(password:str, confirm:str) -> str
  pure without_window(current:window-id?, closed:window-id) -> window-id?
  sync window_target(current:window-id?) -> window-id
  sync window_target_unless(keep:bool, current:window-id?) -> window-id
  create_user_key(password:str) -> KeyCreated ! AppError
  restore_user_key(words:secret, password:str) -> str ! AppError
  unlock_user_key(password:str) -> str ! AppError
  lock_signer() -> bool
  remember_network(rpc:str) -> bool
  forget_network(id:str, kind:str) -> bool
  restore_hidden_networks() -> bool
  pure connection_degraded(status:str) -> bool
  pure titlebar_inset() -> f64
  pure palette_key_action(logical:key, physical:physical-key, modifiers:key-modifiers, open:bool) -> str
  pure topmost_overlay(tab:ShellTab, palette_open:bool, bell_open:bool, channel_create_open:bool, thread_message_action:MessageAction, message_action:MessageAction, channel_settings_open:bool, forge_repo_menu:bool) -> str
  pure escape_target(logical:key, tab:ShellTab, palette_open:bool, bell_open:bool, channel_create_open:bool, thread_message_action:MessageAction, message_action:MessageAction, channel_settings_open:bool, forge_repo_menu:bool) -> str
  pure close_message_action(close:bool, current:MessageAction) -> MessageAction
  pure content_scroll_step(logical:key, modifiers:key-modifiers, overlay:str) -> f64
  NavItem(id:ShellTab, title:str, icon:str, badge:i64, active:bool, live:bool)
  FsEntry(key:i64, path:str, name:str, kind:str, size:i64, object:str)
  FsSnapshot(id:str, short_id:str, author:str, height:i64, message:str)
  FsListing(generation:i64, path:str, entries:[FsEntry])
  FsPreview(generation:i64, path:str, text:str, truncated:bool, binary:bool)
  FsHistory(generation:i64, snapshots:[FsSnapshot])
  pure no_fs_entry() -> FsEntry
  pure fs_entry_named(entries:[FsEntry], path:str) -> FsEntry
  pure fs_directories(entries:[FsEntry]) -> [FsEntry]
  pure fs_counts_summary(connected:bool, listed:bool, entries:[FsEntry]) -> str
  pure fs_parent(path:str) -> str
  pure fs_child(path:str, name:str) -> str
  files_mkdir(rpc:str, path:str) -> bool ! AppError
  files_remove(rpc:str, path:str) -> bool ! AppError
  files_write_text(rpc:str, path:str, text:str) -> bool ! AppError
  files_upload(rpc:str, dir:str, dropped:str) -> bool ! AppError
  FsDiffEntry(path:str, kind:str)
  FsDiff(generation:i64, from:str, entries:[FsDiffEntry])
  files_diff(rpc:str, from:str, generation:i64) -> FsDiff ! HydrationError
  files_ls(rpc:str, path:str, generation:i64) -> FsListing ! HydrationError
  files_preview(rpc:str, path:str, generation:i64) -> FsPreview ! HydrationError
  files_history(rpc:str, generation:i64) -> FsHistory ! HydrationError
  pure size_label(bytes:i64) -> str
  pure shell_nav(tab:ShellTab, approvals:i64, agent_live:bool) -> [NavItem]
  pure open_proposals(rows:[ProposalRow]) -> i64
  pure plural(count:i64, one:str, many:str) -> str
  pure members_summary(connected:bool, rows:[MemberRow]) -> str
  pure agents_summary(connected:bool, rows:[AgentRow]) -> str
  pure proposals_summary(connected:bool, rows:[ProposalRow]) -> str
  QuorumSeat(filled:bool)
  pure quorum_dots(approvals:i64, required:i64) -> [QuorumSeat]
  pure tally_label(approvals:i64, required:i64) -> str
  pure reading_pair(left:str, right:str) -> str
  pure tally_tone(approvals:i64, required:i64) -> str
  pure tally_note(approvals:i64, required:i64) -> str
  pure approve_label(approvals:i64, required:i64) -> str
  pure proposal_kind_tone(action:str) -> str
  pure settled_proposals(rows:[ProposalRow]) -> [ProposalRow]
  pure pending_label(rows:[ProposalRow]) -> str
  pure expires_in_blocks(deadline_height:i64, height:i64, wall_now:i64) -> str
  pure relative_time(unix_seconds:i64, wall_now:i64) -> str
  sync current_wall_seconds() -> i64
  pure mmss(seconds:i64) -> str
  sync network_label(account_name:str, rpc:str) -> str
  pure height_label(height:i64) -> str
  pure height_label_short(height:i64) -> str
  pure height_ago(then_height:i64, now_height:i64, wall_now:i64) -> str
  pure doc_tabs_pruned(tabs:[str], pages:[PageItem]) -> [str]
  pure initial_of(name:str) -> str
  pure initials_of(name:str) -> str
  NodeLogLine(cursor:str, line:str)
  NodeLogTimelineState()
  NodeLogTimelineEvent()
  sync node_log_timeline_state() -> NodeLogTimelineState
  sync node_log_timeline_reset() -> NodeLogTimelineState
  pure node_log_timeline_push(state:NodeLogTimelineState, line:NodeLogLine) -> NodeLogTimelineState
  pure node_log_timeline_filter(state:NodeLogTimelineState, filter:str) -> NodeLogTimelineState
  pure node_log_timeline_apply(state:NodeLogTimelineState, event:NodeLogTimelineEvent) -> NodeLogTimelineState
  component node_log_timeline(state:&NodeLogTimelineState, source:&str) -> NodeLogTimelineEvent
  NodeFacts(public_key:str, version:str, root_hash:str, view:i64?, quorum:i64?, reachable_validators:i64?, last_finalized_at:i64, checkpoint_height:i64, height:i64, phase:str, phase_since:i64, sync_target:i64, sync_applied:i64, sync_retries:i64, sync_failures:i64, sync_last_error:str)
  load_node_facts(rpc:str) -> NodeFacts ! AppError
  pure optional_number(value:i64?) -> str
  PeerRow(key:str, role:str, live:bool)
  PeersData(generation:i64, peers:[PeerRow])
  stream node_logs(rpc:str) -> NodeLogLine
  pure sync_label(phase:str, applied:i64, target:i64) -> str
  stream node_status_live(rpc:str) -> NodeFacts
  stream node_peers_live(rpc:str) -> PeersData
  load_peers(rpc:str, generation:i64) -> PeersData ! HydrationError
  ModuleRow(id:str, category:str, root:str, code_hash:str, pending_hash:str, activation_height:i64, readiness:i64, ready:bool)
  ModulesData(rows:[ModuleRow])
  load_modules(rpc:str) -> ModulesData ! AppError
  AccountData(generation:i64, bound:bool, account_id:str, display_name:str, bio:str, members:i64, nodes:i64)
  load_account(rpc:str, generation:i64) -> AccountData ! HydrationError
  set_account_name(rpc:str, password:str, display_name:str) -> bool ! AppError
  SettingsFacts(generation:i64, key_path:str, key_state:str, data_dir:str, open_tabs:i64, user_key:str)
  load_settings_facts(rpc:str, generation:i64) -> SettingsFacts ! HydrationError
  clear_doc_tabs(rpc:str) -> bool
  forget_workspace(rpc:str) -> bool ! AppError
  ForgeRepo(name:str, head:str)
  ForgeItem(number:i64, kind:str, state:str, title:str, author:str, author_name:str)
  ForgeData(generation:i64, repos:[ForgeRepo])
  ForgeRepoData(generation:i64, repo:str, branches:[str], items:[ForgeItem])
  ForgeReviewComment(anchor:str, body:str)
  ForgeReview(author:str, author_name:str, verdict:str, body:str, commit:str, outdated:bool, created_at:i64, comments:[ForgeReviewComment])
  ForgeItemData(generation:i64, repo:str, number:i64, title:str, state:str, kind:str, body:str, author_name:str, branches:str, channel_id:str, source_branch:str, source_oid:str, target_oid:str, merge_oid:str, diff:str, diff_truncated:bool, files_changed:i64, additions:i64, deletions:i64, reviews:[ForgeReview], approvals:i64, change_requests:i64)
  ForgeDiscussionData(channel_id:str, messages:[ChatMessage], members:[ChatMember])
  ForgeMergeOutcome(merged:bool, merge_oid:str, conflicts:[str])
  ForgeLiveData(generation:i64, repos_loaded:bool, repos:[ForgeRepo], repo_loaded:bool, branches:[str], items:[ForgeItem], item_loaded:bool, item:ForgeItemData)
  load_forge(rpc:str, generation:i64) -> ForgeData ! HydrationError
  load_forge_repo(rpc:str, repo:str, generation:i64) -> ForgeRepoData ! HydrationError
  load_forge_item(rpc:str, repo:str, number:i64, generation:i64) -> ForgeItemData ! HydrationError
  load_forge_discussion(rpc:str, channel_id:str) -> ForgeDiscussionData ! AppError
  TreeEntry(name:str, path:str, kind:str)
  ForgeTreeData(repo:str, rev:str, path:str, born:bool, entries:[TreeEntry], truncated:bool)
  BlobView(repo:str, rev:str, path:str, text:str, truncated:bool, binary:bool, lines:i64)
  forge_tree(rpc:str, repo:str, rev:str, path:str) -> ForgeTreeData ! AppError
  forge_blob(rpc:str, repo:str, rev:str, path:str) -> BlobView ! AppError
  ForgeDraftComment(anchor:str, path:str, line:str, side:str, body:str)
  pure stage_forge_comment(staged:[ForgeDraftComment], path:str, line:str, side:str, body:str) -> [ForgeDraftComment]
  pure drop_forge_comment(staged:[ForgeDraftComment], anchor:str) -> [ForgeDraftComment]
  pure forge_comment_cap_reached(staged:[ForgeDraftComment]) -> bool
  pure keep_staged_comments(loaded:bool, next_oid:str, current_oid:str, staged:[ForgeDraftComment]) -> [ForgeDraftComment]
  pure keep_comment_text(loaded:bool, next_oid:str, current_oid:str, value:str) -> str
  pure staged_comment_drop_note(loaded:bool, next_oid:str, current_oid:str, staged:[ForgeDraftComment], error:str) -> str
  pure forge_comment_target(path:str, line:str, side:str) -> str
  submit_forge_review(rpc:str, password:str, repo:str, number:i64, verdict:ForgeReviewVerdict, body:str, commit_oid:str, comments:[ForgeDraftComment]) -> bool ! AppError
  merge_forge_pr(rpc:str, password:str, repo:str, number:i64, source_branch:str, expected_source_oid:str, prev_target_oid:str) -> ForgeMergeOutcome ! AppError
  forge_live_refresh(rpc:str, open_repo:str, open_item:i64, kind:LiveKind, module:str, scope:ForgeRefresh, forge_open:bool, generation:i64) -> ForgeLiveData ! HydrationError
  pure forge_live_hit(kind:LiveKind, module:str) -> bool
  pure forge_stats(files:i64, additions:i64, deletions:i64) -> str
  DiffLine(key:i64, kind:str, old_no:str, new_no:str, sign:str, text:str, path:str, side:str)
  pure forge_push_command(rpc:str) -> str
  pure diff_lines(diff:str) -> [DiffLine]
  SourceLine(number:str, text:str)
  pure source_lines(text:str) -> [SourceLine]
  pure markdown_path(path:str) -> bool
  pure filter_forge_items(items:[ForgeItem], tab:ForgeTab) -> [ForgeItem]
  pure forge_open_count(items:[ForgeItem], kind:str) -> i64
  pure forge_merge_note(merge_oid:str, branches:str) -> str
  pure verdict_label(verdict:str) -> str
  pure verdict_pick_label(current:ForgeReviewVerdict, key:ForgeReviewVerdict, label:str) -> str
  AgentRow(id:str, name:str, initials:str, capability:str, status:str, owner_handle:str, live:bool, skill_count:i64, cap_count:i64)
  AgentsData(generation:i64, agents:[AgentRow])
  load_agents(rpc:str, generation:i64) -> AgentsData ! HydrationError
  pure any_agent_active(rows:[AgentRow]) -> bool
  set_agent_status(rpc:str, password:str, agent_id:str, paused:bool) -> bool ! AppError
  ProposalRow(id:str, action:str, detail:str, proposer:str, status:str, deadline:i64, approvals:i64, rejections:i64, rule:str, required_yes:i64, electorate:i64, open:bool, settled_height:i64)
  GovernanceData(generation:i64, proposals:[ProposalRow])
  load_governance(rpc:str, generation:i64) -> GovernanceData ! HydrationError
  governance_vote(rpc:str, password:str, proposal_id:str, approve:bool) -> bool ! AppError
  governance_execute(rpc:str, password:str, proposal_id:str) -> bool ! AppError
  governance_propose(rpc:str, password:str, action:str, target_key:str) -> bool ! AppError
  MemberRow(key:str, label:str, role:str, is_this_node:bool, is_agent:bool, model:str, live:bool)
  MembersData(generation:i64, members:[MemberRow])
  load_members(rpc:str, generation:i64) -> MembersData ! HydrationError
  pure members_is_admin(rows:[MemberRow]) -> bool
  pure member_tier(rows:[MemberRow]) -> str
  pure filter_members(rows:[MemberRow], filter:MembersFilter) -> [MemberRow]
  ExplorerBlock(height:i64, hash:str, commit:str, op_count:i64)
  ExplorerOp(height:i64, proposer:str, target:str, disposition:str, op_hash:str, payload:str, trace:str)
  ExplorerData(generation:i64, blocks:[ExplorerBlock], ops:[ExplorerOp])
  pure explorer_ops_at(ops:[ExplorerOp], height:i64) -> [ExplorerOp]
  load_explorer(rpc:str, generation:i64) -> ExplorerData ! HydrationError
  ExplorerHit(kind:str, code:str, title:str, snippet:str, meta:str, target:str)
  KindCount(kind:str, label:str, count:i64)
  ExplorerResults(hits:[ExplorerHit], kinds:[KindCount], partial:str)
  search_workspace(rpc:str, text:str) -> ExplorerResults
  pure doc_tabs_with(tabs:[str], page_id:str) -> [str]
  pure doc_tabs_without(tabs:[str], page_id:str) -> [str]
  DocTab(id:str, title:str, active:bool)
  pure doc_tab_rows(tabs:[str], pages:[PageItem], active:str) -> [DocTab]
  pure next_doc_tab(tabs:[str], closed:str, active:str) -> str
  load_doc_tabs(rpc:str) -> [str]
  load_appearance() -> Appearance
  save_appearance(mode:Appearance) -> bool
  save_doc_tabs(rpc:str, tabs:[str]) -> bool
  pure retain_for_endpoint(value:str, current:str, next:str) -> str
  pure mutation_failure_phase(committed:bool) -> MutationPhase
  pure mutation_phase_after_recovery(current:MutationPhase) -> MutationPhase
  pure message_seq_after_failure(current:i64, phase:MutationPhase, committed:bool) -> i64
  pure message_text_after_failure(current:str, phase:MutationPhase, committed:bool) -> str
  pure message_action_after_failure(current:MessageAction, phase:MutationPhase, committed:bool) -> MessageAction
  pure keep_forge_phase(loaded:bool, next:ForgePhase, current:ForgePhase) -> ForgePhase
  pure refreshed_required_message_seq(messages:[ChatMessage], current_channel:str, next_channel:str, value:i64) -> i64
  pure refreshed_known_message_seq(messages:[ChatMessage], current_channel:str, next_channel:str, value:i64) -> i64
  pure refreshed_channel_value(current_channel:str, next_channel:str, value:i64) -> i64
  pure channel_last_read(reads:[ChannelRead], channel:str) -> i64
  pure channel_head_seq(channels:[ChatChannel], channel:str) -> i64
  pure mark_channel_read(reads:[ChannelRead], channel:str, seq:i64) -> [ChannelRead]
  ChatSidebarRow(channel:ChatChannel, unread:bool)
  DmSidebarRow(peer:DmPeer, unread:bool)
  pure chat_sidebar_rooms(channels:[ChatChannel], peers:[DmPeer], me:str, reads:[ChannelRead]) -> [ChatSidebarRow]
  pure chat_sidebar_dms(channels:[ChatChannel], peers:[DmPeer], reads:[ChannelRead]) -> [DmSidebarRow]
  pure channel_switch_facts(reads:[ChannelRead], channels:[ChatChannel], current_channel:str, next_channel:str, current_boundary:i64, current_name:str) -> ChannelSwitchFacts
  // A load's rows FOLD into the sidebar list; they do not replace it. The
  // switch loader answers with the one row it refreshed, against a list the
  // live stream is still folding into.
  pure upsert_channel_rows(channels:[ChatChannel], refreshed:[ChatChannel]) -> [ChatChannel]
  pure near_scroll_top(relative_offset:f64) -> bool
  // Composer drafts are the only per-room state retained across navigation.
  pure park_message_draft(drafts:[ChannelDraft], channel_id:str, text:str) -> [ChannelDraft]
  pure parked_message_draft(drafts:[ChannelDraft], channel_id:str) -> str
  // The rail's twin, keyed by room AND root. NOT a `failed_reply_draft` harvest
  // — that one is channel-scoped and would re-target the words at another
  // thread; see `park_reply_draft`.
  pure park_reply_draft(drafts:[ChannelDraft], channel_id:str, thread_seq:i64, text:str) -> [ChannelDraft]
  pure parked_reply_draft(drafts:[ChannelDraft], channel_id:str, thread_seq:i64) -> str
  // The page header title of a page that
  // has only just been clicked, read from the list already in hand.
  pure page_display_title(pages:[PageItem], page:str, current:str) -> str
  pure keep_channels(loaded:bool, next:[ChatChannel], current:[ChatChannel]) -> [ChatChannel]
  pure keep_members(loaded:bool, next:[ChatMember], current:[ChatMember]) -> [ChatMember]
  pure keep_roster(joined:bool, next:[HuddleParticipant]) -> [HuddleParticipant]
  pure keep_pages(loaded:bool, next:[PageItem], current:[PageItem]) -> [PageItem]
  pure pages_reply_answers_current(pages:[PageItem], replied:str, current:str) -> bool
  pure keep_blocks(loaded:bool, next:[PageBlock], current:[PageBlock]) -> [PageBlock]
  pure apply_page_text(blocks:[PageBlock], delta:PagesDelta) -> [PageBlock]
  pure apply_page_title(title:str, delta:PagesDelta, active_page:str) -> str
  pure apply_page_rename(pages:[PageItem], delta:PagesDelta) -> [PageItem]
  pure pages_delta_folds(delta:PagesDelta) -> bool
  pure keep_folded_page_titles(fold_outran_reply:bool, next:[PageItem], current:[PageItem]) -> [PageItem]
  pure keep_folded_block_texts(fold_outran_reply:bool, next:[PageBlock], current:[PageBlock]) -> [PageBlock]
  pure plane_live_hit(kind:LiveKind, module:str, want:str) -> bool
  pure agents_plane_hit(kind:LiveKind, module:str) -> bool
  pure tab_reads_plane(tab:ShellTab, plane:str) -> bool
  pure keep_str(loaded:bool, next:str, current:str) -> str
  pure keep_bool(loaded:bool, next:bool, current:bool) -> bool
  pure keep_i64(loaded:bool, next:i64, current:i64) -> i64
  pure keep_strs(loaded:bool, next:[str], current:[str]) -> [str]
  pure commented_targets_of(threads:[PageCommentThread], page_id:str) -> [str]
  pure thread_is_resolved(threads:[PageCommentThread], id:str) -> bool
  pure keep_forge_repos(loaded:bool, next:[ForgeRepo], current:[ForgeRepo]) -> [ForgeRepo]
  pure keep_branches(loaded:bool, next:[str], current:[str]) -> [str]
  pure keep_forge_items(loaded:bool, next:[ForgeItem], current:[ForgeItem]) -> [ForgeItem]
  pure keep_forge_reviews(loaded:bool, next:[ForgeReview], current:[ForgeReview]) -> [ForgeReview]
  pure initial_channel_reads(channels:[ChatChannel], existing:[ChannelRead]) -> [ChannelRead]
  pure frozen_unread_boundary(reads:[ChannelRead], channels:[ChatChannel], current_channel:str, next_channel:str, current_boundary:i64) -> i64
  pure first_unread_seq(messages:[ChatMessage], boundary:i64) -> i64
  pure thread_generation_after_refresh(generation:i64, current_channel:str, next_channel:str, previous_root:i64, next_root:i64) -> i64
  pure thread_loading_after_refresh(loading:bool, current_channel:str, next_channel:str, previous_root:i64, next_root:i64) -> bool
  pure retain_thread_messages(messages:[ChatMessage], root_seq:i64) -> [ChatMessage]
  pure thread_root_seed(messages:[ChatMessage], thread:[ChatMessage], seq:i64) -> [ChatMessage]
  pure remember_orphaned_comment_drafts(drafts:[str], blocks:[PageBlock], selected_id:str, current:str) -> [str]
  pure remove_recovered_draft(drafts:[str], recovered:str) -> [str]
  pure retain_selected_string(value:str, selected_id:str) -> str
  pure retain_selected_i64(value:i64, selected_id:str) -> i64
  pure retain_selected_comment_threads(threads:[PageCommentThread], selected_id:str) -> [PageCommentThread]
  pure retain_selected_comments(comments:[PageComment], selected_id:str) -> [PageComment]
  pure scope_key(scope:str, id:str) -> str
  pure reaction_palette() -> [str]
  pure keep_participants(loaded:bool, next:[HuddleParticipant], current:[HuddleParticipant]) -> [HuddleParticipant]
  pure huddle_recipient_nodes(roster:[HuddleParticipant]) -> [str]
  // ! HydrationError, not ! AppError: the three room-switch loaders below fail
  // with the generation of the switch they belong to, so `chat_load_failed` can
  // drop a failure the reader has already clicked past. `committed` is what
  // `AppError` adds and a switch has nothing to commit.
  load_channel_window(rpc:str, channel_id:str, generation:i64) -> ChatData ! HydrationError
  load_chat_hit(rpc:str, channel_id:str, root_seq:i64, target_seq:i64, generation:i64) -> ChatData ! HydrationError
  create_channel(rpc:str, password:str, name:str, members_only:bool, generation:i64) -> ChatData ! AppError
  rename_channel(rpc:str, password:str, channel_id:str, name:str) -> bool ! AppError
  archive_channel(rpc:str, password:str, channel_id:str) -> bool ! AppError
  unarchive_channel(rpc:str, password:str, channel_id:str) -> bool ! AppError
  add_channel_member(rpc:str, password:str, channel_id:str, member_key:str) -> bool ! AppError
  remove_channel_member(rpc:str, password:str, channel_id:str, member_key:str) -> bool ! AppError
  join_huddle(rpc:str, password:str, channel_id:str) -> bool ! AppError
  leave_huddle(rpc:str, password:str, channel_id:str) -> bool ! AppError
  pure huddle_self(roster:[HuddleParticipant]) -> bool
  DmPeer(key:str, name:str, initials:str, is_agent:bool, channel_id:str)
  DmPeersData(generation:i64, peers:[DmPeer])
  load_dm_peers(rpc:str, generation:i64) -> DmPeersData ! HydrationError
  pure dm_channel_id(a:str, b:str) -> str
  pure dm_peer_of_channel(peer:str, me:str, channel:str) -> str
  pure dm_peer_named(peers:[DmPeer], key:str) -> DmPeer
  pure no_dm_peer() -> DmPeer
  open_dm(rpc:str, password:str, peer_key:str, generation:i64) -> ChatData ! HydrationError
  pure post_gate(archived:bool, members_only:bool, members:[ChatMember], me:str) -> str
  pure reaction_refusal(archived:bool, banner:str) -> str
  send_message(rpc:str, password:str, channel_id:str, message_id:str, body:str, members:[ChatMember]) -> SendReceipt ! OptimisticMutationError
  load_thread(rpc:str, channel_id:str, root_seq:i64, target_seq:i64, generation:i64) -> ThreadLoadData ! HydrationError
  load_thread_page(rpc:str, channel_id:str, root_seq:i64, after_reply_seq:i64, generation:i64) -> ThreadPageData ! HydrationError
  refresh_live_thread(rpc:str, channel_id:str, root_seq:i64) -> LiveThreadData ! AppError
  send_reply(rpc:str, password:str, channel_id:str, root_seq:i64, message_id:str, body:str, members:[ChatMember]) -> SendReceipt ! OptimisticMutationError
  edit_message(rpc:str, password:str, channel_id:str, seq:i64, base_rev:i64, body:str, members:[ChatMember]) -> bool ! AppError
  delete_message(rpc:str, password:str, channel_id:str, seq:i64) -> bool ! AppError
  add_reaction(rpc:str, password:str, channel_id:str, seq:i64, emoji:str) -> bool ! AppError
  remove_reaction(rpc:str, password:str, channel_id:str, seq:i64, emoji:str) -> bool ! AppError
  search_chat(rpc:str, channel_id:str, text:str) -> ChatSearchData ! AppError
  load_page(rpc:str, page_id:str) -> PagesData ! AppError
  load_page_threads(rpc:str, page_id:str, generation:i64) -> BlockThreadListData ! HydrationError
  load_block_comment_page(rpc:str, target:str, thread_id:str, from:i64, generation:i64) -> BlockCommentData ! HydrationError
  post_block_comment(rpc:str, password:str, target:str, thread_id:str, text:str, generation:i64) -> BlockCommentData ! AppError
  resolve_comment_thread(rpc:str, password:str, thread_id:str, resolved:bool) -> bool ! AppError
  open_external_url(url:str) -> bool ! AppError
  create_page(rpc:str, password:str, title:str) -> PagesData ! AppError
  delete_page(rpc:str, password:str, page_id:str) -> PagesData ! AppError
  // THE PAGE'S ONE WRITE PATH. The edited buffer in, the module's own ops
  // out — see backend/document.rs for the ordering rule and the refusal.
  save_page_document(rpc:str, password:str, page_id:str, text:str, saved:str) -> DocumentSaveResult ! AppError
  // The buffer a page opens on: its TITLE as line 0, its blocks under it.
  pure page_document_text(title:str, blocks:[PageBlock]) -> str
  pure subpage_blocks(blocks:[PageBlock]) -> [PageBlock]
  pure count_label(count:i64) -> str
  // A live resync replaces the buffer ONLY when it is clean and the node's
  // text differs; both read the same decision so buffer and baseline move
  // together.
  sync refreshed_page_editor(document:editor, title:str, blocks:[PageBlock], saved:str) -> editor
  pure refreshed_page_saved(document:editor, title:str, blocks:[PageBlock], saved:str) -> str
  pure saved_baseline(written:bool, canonical:str, submitted:str) -> str
  pure baseline_at_submitted_title(canonical:str, submitted:str) -> str
  pure install_decision(document:editor, current_page:str, next_page:str, saved:str, canonical:str) -> bool
  sync installed_page_editor(document:editor, install:bool, canonical:str) -> editor
  sync rolled_back_editor(document:editor, untouched:bool, canonical:str) -> editor
  pure remember_orphaned_page_comment(drafts:[str], pages:[PageItem], target:str, draft:str) -> [str]
  search_pages(rpc:str, page_id:str, text:str) -> PageSearchData ! AppError
  palette_search(rpc:str, text:str) -> PaletteSearchData ! AppError
