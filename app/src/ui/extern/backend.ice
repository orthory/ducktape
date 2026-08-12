extern crate::backend
  ChatChannel(id:str, name:str, archived:bool, members_only:bool, huddle_count:i64, head_seq:i64)
  ChatReaction(emoji:str, count:i64, reacted_by_me:bool)
  ChatMember(key:str, label:str)
  ChannelRead(channel:str, seq:i64)
  ChatSpan(text:str, bold:bool, italic:bool, highlight:bool, link:str)
  ChatBlock(kind:str, text:str, lang:str, rich:bool, spans:[ChatSpan])
  ChatMessage(id:str, seq:i64, author:str, meta:str, body:str, blocks:[ChatBlock], pending:bool, rev:i64, edited:bool, deleted:bool, reply_count:i64, thread_seq:i64, show_author:bool, initial:str, avatar_kind:str, mine:bool, height:i64, time:i64, reactions:[ChatReaction])
  HuddleParticipant(key:str, label:str, initials:str, is_agent:bool, is_you:bool, joined_at:i64, node:str)
  ChatData(channels:[ChatChannel], messages:[ChatMessage], active_channel:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, active_channel_huddle_count:i64, huddle_roster:[HuddleParticipant], channel_members:[ChatMember], selected_message_seq:i64, selected_message_rev:i64, selected_message_body:str, active_thread_seq:i64, thread_target_seq:i64, thread_messages:[ChatMessage], thread_next_reply_offset:i64, thread_has_more:bool)
  SendReceipt(operation_id:str, channel_id:str)
  ChatDelta(kind:str, channel_id:str, seq:i64, root_seq:i64, message:ChatMessage, channel:ChatChannel, name:str, archived:bool, emoji:str, added:bool, reactor:str, by_me:bool, member:ChatMember)
  PagesDelta(kind:str, block_id:str, text:str)
  LiveRefresh(generation:i64, chat_loaded:bool, channels:[ChatChannel], messages:[ChatMessage], active_channel:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, active_channel_huddle_count:i64, huddle_roster:[HuddleParticipant], channel_members:[ChatMember], pages_loaded:bool, pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str, comment_thread_total:i64, commented_block_hits:[str])
  ThreadLoadData(generation:i64, root_seq:i64, target_seq:i64, messages:[ChatMessage], next_reply_offset:i64, has_more:bool)
  ThreadPageData(generation:i64, messages:[ChatMessage], next_reply_offset:i64, has_more:bool)
  LiveThreadData(generation:i64, channel_id:str, root_seq:i64, target_seq:i64, messages:[ChatMessage], next_reply_offset:i64, has_more:bool)
  HistoryPageData(generation:i64, channel_id:str, messages:[ChatMessage])
  ChatSearchHit(channel_id:str, seq:i64, root_seq:i64, author:str, text:str, meta:str)
  ChatSearchData(generation:i64, hits:[ChatSearchHit])
  PageItem(id:str, title:str, parent:str, prefix:str, child_count:i64)
  PageBlock(key:i64, id:str, parent:str, kind:str, text:str, pending:bool, checked:bool, prefix:str, child_count:i64)
  PagesData(pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str, comment_thread_total:i64, commented_block_hits:[str])
  PageCommentThread(id:str, target:str, author:str, meta:str, resolved:bool, comment_count:i64)
  PageComment(id:str, ordinal:i64, author:str, meta:str, text:str)
  BlockThreadListData(generation:i64, target:str, from:i64, threads:[PageCommentThread], total:i64, next_from:i64, has_more:bool)
  BlockCommentData(generation:i64, target:str, thread_id:str, from:i64, comments:[PageComment], next_from:i64, has_more:bool)
  PageSearchHit(page_id:str, page_title:str, block_id:str, kind:str, text:str)
  PageSearchData(generation:i64, hits:[PageSearchHit])
  PaletteSearchData(generation:i64, chat_hits:[ChatSearchHit], page_hits:[PageSearchHit])
  // `refusal` is not a failure: the write was NOT attempted because carrying it
  // out would have destroyed records. `document` is the canonical text either
  // way — the buffer takes it, which is what rolls an illegal edit back.
  DocumentSaveResult(generation:i64, written:bool, refusal:str, data:PagesData, document:str)
  WorkspaceData(generation:i64, rpc:str, status:str, height:i64, channels:[ChatChannel], messages:[ChatMessage], active_channel:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, active_channel_huddle_count:i64, huddle_roster:[HuddleParticipant], channel_members:[ChatMember], pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str, comment_thread_total:i64, commented_block_hits:[str])
  BellItem(seq:i64, kind:str, body:str, source:str, height:i64, read:bool)
  BellDelta(kind:str, item:BellItem, up_to_seq:i64)
  BellData(generation:i64, unread:i64, items:[BellItem])
  sync apply_bell(items:[BellItem], delta:BellDelta) -> [BellItem]
  sync bell_unread_after(unread:i64, items:[BellItem], delta:BellDelta) -> i64
  sync bell_head(items:[BellItem]) -> i64
  sync bell_severity(kind:str) -> str
  sync bell_title(kind:str) -> str
  sync bell_worst_severity(items:[BellItem]) -> str
  load_bell(rpc:str, generation:i64) -> BellData ! HydrationError
  mark_bell_read(rpc:str, password:str, up_to_seq:i64) -> bool ! AppError
  ForgeRefresh(repo:str, number:i64, refs_moved:bool)
  LiveUpdate(kind:str, status:str, height:i64, module:str, load_chat:bool, load_pages:bool, debounce:bool, chat:ChatDelta, pages:PagesDelta, bell:BellDelta, forge:ForgeRefresh)
  AppError(message:str, committed:bool)
  OptimisticMutationError(message:str, committed:bool, operation_id:str, scope_id:str, body:str)
  HydrationError(generation:i64, message:str)
  box-style card_style()
  box-style raised_style()
  svg-style icon_tint(tone:str)
  sync icon(name:str) -> str
  connect(rpc:str, attempt:i64, generation:i64) -> WorkspaceData ! HydrationError
  stream live_events(rpc:str) -> LiveUpdate
  sync resync_planes(load_chat:bool, load_pages:bool) -> str
  live_resync_load(rpc:str, channel_id:str, page_id:str, planes:str, debounce:bool, generation:i64, attempt:i64) -> LiveRefresh ! HydrationError
  load_older_messages(rpc:str, channel_id:str, before_seq:i64, generation:i64) -> HistoryPageData ! HydrationError
  sync fresh_operation_id(prefix:str) -> str
  sync optimistic_message(messages:[ChatMessage], body:str, message_id:str) -> [ChatMessage]
  sync merge_pending_messages(canonical:[ChatMessage], current:[ChatMessage], current_channel:str, next_channel:str, settled_id:str) -> [ChatMessage]
  sync merge_message_send_result(canonical:[ChatMessage], current:[ChatMessage], current_channel:str, next_channel:str, settled_id:str) -> [ChatMessage]
  sync rollback_pending_message(messages:[ChatMessage], pending_id:str, committed:bool) -> [ChatMessage]
  sync contains_pending_message(messages:[ChatMessage], pending_id:str) -> bool
  sync reaction_applied(messages:[ChatMessage], seq:i64, emoji:str, added:bool) -> [ChatMessage]
  sync send_settled_by(messages:[ChatMessage], delta:ChatDelta, active_channel:str) -> bool
  sync settled_send_id(messages:[ChatMessage], delta:ChatDelta, active_channel:str, current:str) -> str
  sync reply_settled_by(thread:[ChatMessage], delta:ChatDelta, active_channel:str) -> bool
  sync settled_reply_id(thread:[ChatMessage], delta:ChatDelta, active_channel:str, current:str) -> str
  sync append_thread_page(messages:[ChatMessage], next:[ChatMessage]) -> [ChatMessage]
  sync merge_thread_reply(messages:[ChatMessage], reply:ChatMessage) -> [ChatMessage]
  sync history_has_older(messages:[ChatMessage]) -> bool
  sync oldest_message_seq(messages:[ChatMessage]) -> i64
  sync prepend_history(messages:[ChatMessage], older:[ChatMessage]) -> [ChatMessage]
  sync thread_offset_after_reply(offset:i64, has_more:bool, committed:bool) -> i64
  sync merge_pending_blocks(canonical:[PageBlock], current:[PageBlock], current_page:str, next_page:str, settled_id:str) -> [PageBlock]
  sync restore_draft(current:str, pending:str, keep_pending:bool) -> str
  // Chat's message/thread menus still place themselves this way; the name is
  // the pages block menu it was written for, which no longer exists.
  sync block_action_menu_y(pointer_y:f64, viewport_height:f64) -> f64
  sync rollback_blocks(blocks:[PageBlock], keep_pending:bool) -> [PageBlock]
  sync append_page_comment_threads(threads:[PageCommentThread], next:[PageCommentThread]) -> [PageCommentThread]
  sync append_page_comments(comments:[PageComment], next:[PageComment]) -> [PageComment]
  sync remember_failed_draft(existing:str, current:str, pending:str, committed:bool) -> str
  sync canonical_endpoint(input:str) -> str
  sync network_slug(name:str) -> str
  WorkspaceInit(chain_id:str, workspace:str, rpc:str)
  join_network(blob:str) -> WorkspaceInit ! AppError
  mint_invite(workspace:str, role:str, ttl_days:i64) -> str ! AppError
  ProvisionStep(index:i64, label:str, state:str, settled:bool)
  stream provision_progress(workspace:str, rpc:str) -> ProvisionStep
  HubNetwork(id:str, chain_id:str, name:str, endpoint:str, kind:str, last_used:i64, probed:bool, live:bool, height:i64)
  HubProbe(generation:i64, id:str, live:bool, height:i64)
  HubState(key_state:str, networks:[HubNetwork], preselect:str, hidden:i64)
  KeyCreated(words:str, pubkey:str)
  hub_state() -> HubState
  stream probe_known_networks(generation:i64) -> HubProbe
  sync apply_network_probe(networks:[HubNetwork], probe:HubProbe) -> [HubNetwork]
  sync network_run_hint(row:HubNetwork) -> str
  sync hub_entry_step(key_state:str) -> str
  sync selected_network_endpoint(networks:[HubNetwork], id:str) -> str
  sync refreshed_hub_selection(networks:[HubNetwork], current:str, preselect:str) -> str
  sync password_problem(password:str, confirm:str) -> str
  sync without_window(current:window-id?, closed:window-id) -> window-id?
  sync window_target(current:window-id?) -> window-id
  sync window_target_unless(keep:bool, current:window-id?) -> window-id
  create_user_key(password:str) -> KeyCreated ! AppError
  restore_user_key(words:str, password:str) -> str ! AppError
  unlock_user_key(password:str) -> str ! AppError
  remember_network(rpc:str) -> bool
  forget_network(id:str, kind:str) -> bool
  restore_hidden_networks() -> bool
  sync connection_degraded(status:str) -> bool
  sync titlebar_inset() -> f64
  sync palette_key_action(logical:key, physical:physical-key, modifiers:key-modifiers, open:bool) -> str
  sync topmost_overlay(palette_open:bool, bell_open:bool, channel_create_open:bool, thread_message_action:str, message_action:str, channel_settings_open:bool, forge_repo_menu:bool) -> str
  sync escape_target(logical:key, palette_open:bool, bell_open:bool, channel_create_open:bool, thread_message_action:str, message_action:str, channel_settings_open:bool, forge_repo_menu:bool) -> str
  sync content_scroll_step(logical:key, modifiers:key-modifiers, overlay:str) -> f64
  NavItem(id:str, title:str, icon:str, badge:i64, active:bool, live:bool)
  FsEntry(path:str, name:str, kind:str, size:i64, object:str)
  FsSnapshot(id:str, short_id:str, author:str, height:i64, message:str)
  FsListing(generation:i64, path:str, entries:[FsEntry])
  FsPreview(generation:i64, path:str, text:str, truncated:bool, binary:bool)
  FsHistory(generation:i64, snapshots:[FsSnapshot])
  sync fs_dir_count(entries:[FsEntry]) -> i64
  sync fs_file_count(entries:[FsEntry]) -> i64
  sync fs_counts_summary(connected:bool, listed:bool, entries:[FsEntry]) -> str
  sync fs_parent(path:str) -> str
  sync fs_child(path:str, name:str) -> str
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
  files_find(rpc:str, prefix:str, generation:i64) -> FsListing ! HydrationError
  ChangeStamp(generation:i64, path:str, author:str, height:i64)
  last_changed_at_path(rpc:str, path:str, generation:i64) -> ChangeStamp ! HydrationError
  sync size_label(bytes:i64) -> str
  sync shell_nav(tab:str, approvals:i64, agent_live:bool) -> [NavItem]
  sync open_proposals(rows:[ProposalRow]) -> i64
  sync plural(count:i64, one:str, many:str) -> str
  sync members_summary(connected:bool, rows:[MemberRow]) -> str
  sync agents_summary(connected:bool, rows:[AgentRow]) -> str
  sync proposals_summary(connected:bool, rows:[ProposalRow]) -> str
  QuorumSeat(filled:bool)
  sync quorum_dots(approvals:i64, required:i64) -> [QuorumSeat]
  sync tally_label(approvals:i64, required:i64) -> str
  sync reading_pair(left:str, right:str) -> str
  sync tally_tone(approvals:i64, required:i64) -> str
  sync tally_note(approvals:i64, required:i64) -> str
  sync approve_label(approvals:i64, required:i64) -> str
  sync proposal_kind_tone(action:str) -> str
  sync settled_proposals(rows:[ProposalRow]) -> [ProposalRow]
  sync pending_label(rows:[ProposalRow]) -> str
  sync expires_in_blocks(deadline_height:i64, height:i64) -> str
  sync relative_time(unix_seconds:i64) -> str
  sync mmss(seconds:i64) -> str
  sync network_label(account_name:str, rpc:str) -> str
  sync height_label(height:i64) -> str
  sync height_label_short(height:i64) -> str
  sync height_ago(then_height:i64, now_height:i64) -> str
  sync doc_tabs_pruned(tabs:[str], pages:[PageItem]) -> [str]
  sync initial_of(name:str) -> str
  sync initials_of(name:str) -> str
  NodeLogLine(cursor:str, line:str)
  LogParts(time:str, level:str, message:str)
  sync split_log_line(line:str) -> LogParts
  NodeFacts(generation:i64, version:str, root_hash:str, view:i64?, quorum:i64?, reachable_validators:i64?, last_finalized_at:i64, checkpoint_height:i64, height:i64, phase:str, phase_since:i64, sync_target:i64, sync_applied:i64, sync_retries:i64, sync_failures:i64, sync_last_error:str)
  load_node_facts(rpc:str, generation:i64) -> NodeFacts ! HydrationError
  sync optional_number(value:i64?) -> str
  PeerRow(key:str, role:str, live:bool)
  PeersData(generation:i64, peers:[PeerRow])
  sync push_log_line(lines:[NodeLogLine], line:NodeLogLine) -> [NodeLogLine]
  sync filter_log_lines(lines:[NodeLogLine], filter:str) -> [NodeLogLine]
  stream node_logs(rpc:str) -> NodeLogLine
  sync sync_label(phase:str, applied:i64, target:i64) -> str
  stream node_status_live(rpc:str) -> NodeFacts
  stream node_peers_live(rpc:str) -> PeersData
  load_peers(rpc:str, generation:i64) -> PeersData ! HydrationError
  ModuleRow(id:str, category:str, root:str, code_hash:str, pending_hash:str, activation_height:i64, readiness:i64, ready:bool)
  ModulesData(generation:i64, rows:[ModuleRow])
  load_modules(rpc:str, generation:i64) -> ModulesData ! HydrationError
  AccountData(generation:i64, bound:bool, account_id:str, display_name:str, bio:str, members:i64, nodes:i64)
  load_account(rpc:str, generation:i64) -> AccountData ! HydrationError
  set_account_name(rpc:str, password:str, display_name:str) -> bool ! AppError
  SettingsFacts(generation:i64, endpoint:str, node_key:str, key_path:str, key_state:str, data_dir:str, open_tabs:i64, user_key:str)
  load_settings_facts(rpc:str, generation:i64) -> SettingsFacts ! HydrationError
  clear_doc_tabs(rpc:str) -> bool
  forget_workspace(rpc:str) -> bool ! AppError
  ForgeRepo(name:str, head:str, about:str, language:str, updated_at:i64)
  ForgeItem(number:i64, kind:str, state:str, title:str, author:str, author_name:str)
  ForgeData(generation:i64, repos:[ForgeRepo])
  ForgeRepoData(generation:i64, repo:str, branches:[str], items:[ForgeItem])
  ForgeReviewComment(anchor:str, body:str)
  ForgeReview(author:str, author_name:str, verdict:str, body:str, commit:str, outdated:bool, created_at:i64, comments:[ForgeReviewComment])
  ForgeItemData(generation:i64, repo:str, number:i64, title:str, state:str, kind:str, body:str, author_name:str, branches:str, channel_id:str, source_branch:str, source_oid:str, target_oid:str, merge_oid:str, diff:str, diff_truncated:bool, files_changed:i64, additions:i64, deletions:i64, reviews:[ForgeReview], approvals:i64, change_requests:i64)
  ForgeDiscussionData(generation:i64, channel_id:str, messages:[ChatMessage], members:[ChatMember])
  ForgeMergeOutcome(repo:str, number:i64, merged:bool, merge_oid:str, conflicts:[str])
  ForgeLiveData(generation:i64, repos_loaded:bool, repos:[ForgeRepo], repo_loaded:bool, branches:[str], items:[ForgeItem], item_loaded:bool, item:ForgeItemData)
  load_forge(rpc:str, generation:i64) -> ForgeData ! HydrationError
  load_forge_repo(rpc:str, repo:str, generation:i64) -> ForgeRepoData ! HydrationError
  load_forge_item(rpc:str, repo:str, number:i64, generation:i64) -> ForgeItemData ! HydrationError
  load_forge_discussion(rpc:str, channel_id:str, generation:i64) -> ForgeDiscussionData ! HydrationError
  TreeEntry(name:str, path:str, kind:str, size:i64)
  ForgeTreeData(generation:i64, repo:str, rev:str, path:str, entries:[TreeEntry])
  BlobView(generation:i64, repo:str, rev:str, path:str, text:str, truncated:bool, binary:bool, lines:i64)
  forge_tree(rpc:str, repo:str, rev:str, path:str, generation:i64) -> ForgeTreeData ! HydrationError
  forge_blob(rpc:str, repo:str, rev:str, path:str, generation:i64) -> BlobView ! HydrationError
  ForgeDraftComment(anchor:str, path:str, line:str, side:str, body:str)
  sync stage_forge_comment(staged:[ForgeDraftComment], path:str, line:str, side:str, body:str) -> [ForgeDraftComment]
  sync drop_forge_comment(staged:[ForgeDraftComment], anchor:str) -> [ForgeDraftComment]
  sync forge_comment_cap_reached(staged:[ForgeDraftComment]) -> bool
  sync keep_staged_comments(loaded:bool, next_oid:str, current_oid:str, staged:[ForgeDraftComment]) -> [ForgeDraftComment]
  sync keep_comment_text(loaded:bool, next_oid:str, current_oid:str, value:str) -> str
  sync staged_comment_drop_note(loaded:bool, next_oid:str, current_oid:str, staged:[ForgeDraftComment], error:str) -> str
  sync forge_comment_target(path:str, line:str, side:str) -> str
  submit_forge_review(rpc:str, password:str, repo:str, number:i64, verdict:str, body:str, commit_oid:str, comments:[ForgeDraftComment]) -> bool ! AppError
  merge_forge_pr(rpc:str, password:str, repo:str, number:i64, source_branch:str, expected_source_oid:str, prev_target_oid:str) -> ForgeMergeOutcome ! AppError
  forge_live_refresh(rpc:str, open_repo:str, open_item:i64, kind:str, module:str, scope:ForgeRefresh, forge_open:bool, generation:i64) -> ForgeLiveData ! HydrationError
  sync forge_live_hit(kind:str, module:str) -> bool
  sync forge_repo_row(repos:[ForgeRepo], name:str) -> ForgeRepo
  sync forge_stats(files:i64, additions:i64, deletions:i64) -> str
  DiffLine(kind:str, old_no:str, new_no:str, sign:str, text:str, path:str, side:str)
  sync forge_push_command(rpc:str) -> str
  sync diff_lines(diff:str) -> [DiffLine]
  SourceLine(number:str, text:str)
  sync source_lines(text:str) -> [SourceLine]
  sync filter_forge_items(items:[ForgeItem], kind:str) -> [ForgeItem]
  sync forge_open_count(items:[ForgeItem], kind:str) -> i64
  sync forge_merge_note(merge_oid:str, branches:str) -> str
  sync verdict_label(verdict:str) -> str
  sync verdict_pick_label(current:str, key:str, label:str) -> str
  AgentSkill(name:str, always:bool)
  AgentCap(label:str, arg:str)
  AgentRow(id:str, name:str, initials:str, capability:str, status:str, owner_key:str, owner_handle:str, created_at:i64, is_mine:bool, live:bool, tools:i64, secrets:i64, subagent_budget:i64, allowed_actions:[str], skills:[AgentSkill], caps:[AgentCap])
  RunRow(run_id:str, agent_id:str, outcome:str, running:bool, created_at:i64, summary:str)
  AgentRunsData(generation:i64, runs:[RunRow])
  AgentsData(generation:i64, agents:[AgentRow])
  load_agents(rpc:str, generation:i64) -> AgentsData ! HydrationError
  sync any_agent_active(rows:[AgentRow]) -> bool
  load_agent_runs(rpc:str, agent_id:str, generation:i64) -> AgentRunsData ! HydrationError
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
  sync members_is_admin(rows:[MemberRow]) -> bool
  sync member_tier(rows:[MemberRow]) -> str
  sync filter_members(rows:[MemberRow], filter:str) -> [MemberRow]
  ExplorerBlock(height:i64, hash:str, commit:str, op_count:i64)
  ExplorerOp(height:i64, proposer:str, target:str, disposition:str, op_hash:str, payload:str, trace:str)
  ExplorerData(generation:i64, blocks:[ExplorerBlock], ops:[ExplorerOp])
  sync explorer_ops_at(ops:[ExplorerOp], height:i64) -> [ExplorerOp]
  load_explorer(rpc:str, generation:i64) -> ExplorerData ! HydrationError
  ExplorerHit(kind:str, code:str, title:str, snippet:str, meta:str, target:str)
  KindCount(kind:str, label:str, count:i64)
  ExplorerResults(generation:i64, hits:[ExplorerHit], kinds:[KindCount], partial:str)
  search_workspace(rpc:str, text:str, generation:i64) -> ExplorerResults ! HydrationError
  sync doc_tabs_with(tabs:[str], page_id:str) -> [str]
  sync doc_tabs_without(tabs:[str], page_id:str) -> [str]
  DocTab(id:str, title:str, active:bool)
  sync retain_doc_tabs(tabs:[str], pages:[PageItem]) -> [str]
  sync doc_tab_rows(tabs:[str], pages:[PageItem], active:str) -> [DocTab]
  sync next_doc_tab(tabs:[str], closed:str, active:str) -> str
  load_doc_tabs(rpc:str) -> [str]
  load_appearance() -> str
  save_appearance(mode:str) -> bool
  save_doc_tabs(rpc:str, tabs:[str]) -> bool
  sync retain_for_endpoint(value:str, current:str, next:str) -> str
  sync mutation_failure_phase(committed:bool) -> str
  sync message_seq_after_failure(current:i64, phase:str, committed:bool) -> i64
  sync message_text_after_failure(current:str, phase:str, committed:bool) -> str
  sync message_action_after_failure(current:str, phase:str, committed:bool) -> str
  sync refreshed_required_message_seq(messages:[ChatMessage], current_channel:str, next_channel:str, value:i64) -> i64
  sync refreshed_known_message_seq(messages:[ChatMessage], current_channel:str, next_channel:str, value:i64) -> i64
  sync refreshed_channel_value(current_channel:str, next_channel:str, value:i64) -> i64
  sync channel_last_read(reads:[ChannelRead], channel:str) -> i64
  sync channel_head_seq(channels:[ChatChannel], channel:str) -> i64
  sync mark_channel_read(reads:[ChannelRead], channel:str, seq:i64) -> [ChannelRead]
  sync channel_is_unread(reads:[ChannelRead], channel:str, head_seq:i64) -> bool
  sync apply_chat_channels(channels:[ChatChannel], delta:ChatDelta) -> [ChatChannel]
  sync apply_chat_messages(messages:[ChatMessage], delta:ChatDelta, active_channel:str) -> [ChatMessage]
  sync apply_chat_thread(thread:[ChatMessage], delta:ChatDelta, active_channel:str, root:i64) -> [ChatMessage]
  sync apply_chat_members(members:[ChatMember], delta:ChatDelta, active_channel:str) -> [ChatMember]
  sync thread_offset_after_live(offset:i64, has_more:bool, delta:ChatDelta, active_channel:str, root:i64) -> i64
  sync channel_display_name(channels:[ChatChannel], channel:str, current:str) -> str
  // The pages twin of `channel_display_name`: the header title of a page that
  // has only just been clicked, read from the list already in hand.
  sync page_display_title(pages:[PageItem], page:str, current:str) -> str
  sync channel_flag_archived(channels:[ChatChannel], channel:str, current:bool) -> bool
  sync channel_flag_members_only(channels:[ChatChannel], channel:str, current:bool) -> bool
  sync channel_live_huddle_count(channels:[ChatChannel], channel:str, current:i64) -> i64
  sync keep_channels(loaded:bool, next:[ChatChannel], current:[ChatChannel]) -> [ChatChannel]
  sync keep_messages(loaded:bool, next:[ChatMessage], current:[ChatMessage]) -> [ChatMessage]
  sync keep_members(loaded:bool, next:[ChatMember], current:[ChatMember]) -> [ChatMember]
  sync keep_roster(joined:bool, next:[HuddleParticipant]) -> [HuddleParticipant]
  sync keep_peers(loaded:bool, next:[PeerRow], current:[PeerRow]) -> [PeerRow]
  sync keep_pages(loaded:bool, next:[PageItem], current:[PageItem]) -> [PageItem]
  sync pages_reply_answers_current(pages:[PageItem], replied:str, current:str) -> bool
  sync keep_blocks(loaded:bool, next:[PageBlock], current:[PageBlock]) -> [PageBlock]
  sync apply_page_text(blocks:[PageBlock], delta:PagesDelta) -> [PageBlock]
  sync apply_page_title(title:str, delta:PagesDelta, active_page:str) -> str
  sync apply_page_rename(pages:[PageItem], delta:PagesDelta) -> [PageItem]
  sync plane_live_hit(kind:str, module:str, want:str) -> bool
  sync tab_reads_plane(tab:str, plane:str) -> bool
  sync keep_str(loaded:bool, next:str, current:str) -> str
  sync keep_bool(loaded:bool, next:bool, current:bool) -> bool
  sync keep_i64(loaded:bool, next:i64, current:i64) -> i64
  sync keep_strs(loaded:bool, next:[str], current:[str]) -> [str]
  sync commented_targets_of(threads:[PageCommentThread], page_id:str) -> [str]
  sync thread_is_resolved(threads:[PageCommentThread], id:str) -> bool
  sync keep_forge_repos(loaded:bool, next:[ForgeRepo], current:[ForgeRepo]) -> [ForgeRepo]
  sync keep_branches(loaded:bool, next:[str], current:[str]) -> [str]
  sync keep_forge_items(loaded:bool, next:[ForgeItem], current:[ForgeItem]) -> [ForgeItem]
  sync keep_forge_reviews(loaded:bool, next:[ForgeReview], current:[ForgeReview]) -> [ForgeReview]
  sync initial_channel_reads(channels:[ChatChannel], existing:[ChannelRead]) -> [ChannelRead]
  sync frozen_unread_boundary(reads:[ChannelRead], channels:[ChatChannel], current_channel:str, next_channel:str, current_boundary:i64) -> i64
  sync first_unread_seq(messages:[ChatMessage], boundary:i64) -> i64
  sync thread_generation_after_refresh(generation:i64, current_channel:str, next_channel:str, previous_root:i64, next_root:i64) -> i64
  sync thread_loading_after_refresh(loading:bool, current_channel:str, next_channel:str, previous_root:i64, next_root:i64) -> bool
  sync retain_thread_messages(messages:[ChatMessage], root_seq:i64) -> [ChatMessage]
  sync cancel_autosaves(rpc:str, generation:i64) -> i64
  sync remember_orphaned_comment_drafts(drafts:[str], blocks:[PageBlock], selected_id:str, current:str) -> [str]
  sync remove_recovered_draft(drafts:[str], recovered:str) -> [str]
  sync retain_selected_string(value:str, selected_id:str) -> str
  sync retain_selected_i64(value:i64, selected_id:str) -> i64
  sync retain_selected_comment_threads(threads:[PageCommentThread], selected_id:str) -> [PageCommentThread]
  sync retain_selected_comments(comments:[PageComment], selected_id:str) -> [PageComment]
  sync scope_key(scope:str, id:str) -> str
  sync reaction_palette() -> [str]
  sync keep_participants(loaded:bool, next:[HuddleParticipant], current:[HuddleParticipant]) -> [HuddleParticipant]
  sync huddle_recipient_nodes(roster:[HuddleParticipant]) -> [str]
  sync huddle_refresh_hits(delta:ChatDelta, active_channel:str) -> bool
  load_chat(rpc:str, channel_id:str) -> ChatData ! AppError
  load_chat_hit(rpc:str, channel_id:str, root_seq:i64, target_seq:i64) -> ChatData ! AppError
  create_channel(rpc:str, password:str, name:str, members_only:bool) -> ChatData ! AppError
  rename_channel(rpc:str, password:str, channel_id:str, name:str) -> bool ! AppError
  archive_channel(rpc:str, password:str, channel_id:str) -> bool ! AppError
  unarchive_channel(rpc:str, password:str, channel_id:str) -> bool ! AppError
  add_channel_member(rpc:str, password:str, channel_id:str, member_key:str) -> bool ! AppError
  remove_channel_member(rpc:str, password:str, channel_id:str, member_key:str) -> bool ! AppError
  join_huddle(rpc:str, password:str, channel_id:str) -> bool ! AppError
  leave_huddle(rpc:str, password:str, channel_id:str) -> bool ! AppError
  sync huddle_self(roster:[HuddleParticipant]) -> bool
  DmPeer(key:str, name:str, initials:str, is_agent:bool)
  DmPeersData(generation:i64, peers:[DmPeer])
  load_dm_peers(rpc:str, generation:i64) -> DmPeersData ! HydrationError
  sync dm_channel_id(a:str, b:str) -> str
  sync rooms_only(channels:[ChatChannel], peers:[DmPeer], me:str) -> [ChatChannel]
  open_dm(rpc:str, password:str, peer_key:str) -> ChatData ! AppError
  sync post_gate(archived:bool, members_only:bool, members:[ChatMember], me:str) -> str
  send_message(rpc:str, password:str, channel_id:str, message_id:str, body:str, members:[ChatMember]) -> SendReceipt ! OptimisticMutationError
  load_thread(rpc:str, channel_id:str, root_seq:i64, target_seq:i64, through_reply_offset:i64, generation:i64) -> ThreadLoadData ! HydrationError
  load_thread_page(rpc:str, channel_id:str, root_seq:i64, from:i64, generation:i64) -> ThreadPageData ! HydrationError
  refresh_live_thread(rpc:str, channel_id:str, root_seq:i64, target_seq:i64, through_reply_offset:i64, generation:i64) -> LiveThreadData ! HydrationError
  send_reply(rpc:str, password:str, channel_id:str, root_seq:i64, message_id:str, body:str, members:[ChatMember]) -> SendReceipt ! OptimisticMutationError
  edit_message(rpc:str, password:str, channel_id:str, seq:i64, base_rev:i64, body:str, members:[ChatMember]) -> bool ! AppError
  delete_message(rpc:str, password:str, channel_id:str, seq:i64) -> bool ! AppError
  add_reaction(rpc:str, password:str, channel_id:str, seq:i64, emoji:str) -> bool ! AppError
  remove_reaction(rpc:str, password:str, channel_id:str, seq:i64, emoji:str) -> bool ! AppError
  search_chat(rpc:str, channel_id:str, text:str, generation:i64) -> ChatSearchData ! HydrationError
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
  save_page_document(rpc:str, password:str, page_id:str, text:str, saved:str, generation:i64) -> DocumentSaveResult ! HydrationError
  // The buffer a page opens on: its TITLE as line 0, its blocks under it.
  sync page_document_text(title:str, blocks:[PageBlock]) -> str
  sync subpage_blocks(blocks:[PageBlock]) -> [PageBlock]
  sync count_label(count:i64) -> str
  // A live resync replaces the buffer ONLY when it is clean and the node's
  // text differs; both read the same decision so buffer and baseline move
  // together.
  sync refreshed_page_editor(document:editor, title:str, blocks:[PageBlock], saved:str) -> editor
  sync refreshed_page_saved(document:editor, title:str, blocks:[PageBlock], saved:str) -> str
  sync saved_baseline(written:bool, canonical:str, submitted:str) -> str
  sync baseline_at_submitted_title(canonical:str, submitted:str) -> str
  sync install_decision(document:editor, current_page:str, next_page:str, saved:str, canonical:str) -> bool
  sync installed_page_editor(document:editor, install:bool, canonical:str) -> editor
  sync rolled_back_editor(document:editor, untouched:bool, canonical:str) -> editor
  sync remember_orphaned_page_comment(drafts:[str], pages:[PageItem], target:str, draft:str) -> [str]
  search_pages(rpc:str, page_id:str, text:str, generation:i64) -> PageSearchData ! HydrationError
  palette_search(rpc:str, text:str, generation:i64) -> PaletteSearchData ! HydrationError
