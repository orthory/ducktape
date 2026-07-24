extern crate::backend
  ChatChannel(id:str, name:str, archived:bool, members_only:bool, huddle_count:i64, head_seq:i64)
  ChatReaction(emoji:str, count:i64, reacted_by_me:bool)
  ChatMember(key:str, label:str)
  ChannelRead(channel:str, seq:i64)
  ChatSpan(text:str, bold:bool, italic:bool, highlight:bool, link:str)
  ChatBlock(kind:str, text:str, lang:str, rich:bool, spans:[ChatSpan])
  ChatMessage(id:str, seq:i64, author:str, meta:str, body:str, blocks:[ChatBlock], pending:bool, rev:i64, edited:bool, deleted:bool, reply_count:i64, thread_seq:i64, show_author:bool, initial:str, avatar_r:f64, avatar_g:f64, avatar_b:f64, reactions:[ChatReaction])
  ChatData(channels:[ChatChannel], messages:[ChatMessage], active_channel:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, active_channel_huddle_count:i64, channel_members:[ChatMember], selected_message_seq:i64, selected_message_rev:i64, selected_message_body:str, active_thread_seq:i64, thread_target_seq:i64, thread_messages:[ChatMessage], thread_next_reply_offset:i64, thread_has_more:bool)
  SendReceipt(operation_id:str, channel_id:str)
  ChatDelta(kind:str, channel_id:str, seq:i64, root_seq:i64, message:ChatMessage, channel:ChatChannel, name:str, archived:bool, emoji:str, added:bool, reactor:str, by_me:bool, member:ChatMember)
  PagesDelta(kind:str, comments:bool)
  LiveRefresh(generation:i64, chat_loaded:bool, channels:[ChatChannel], messages:[ChatMessage], active_channel:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, active_channel_huddle_count:i64, channel_members:[ChatMember], pages_loaded:bool, pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str)
  ThreadLoadData(generation:i64, root_seq:i64, target_seq:i64, messages:[ChatMessage], next_reply_offset:i64, has_more:bool)
  ThreadPageData(generation:i64, messages:[ChatMessage], next_reply_offset:i64, has_more:bool)
  LiveThreadData(generation:i64, channel_id:str, root_seq:i64, target_seq:i64, messages:[ChatMessage], next_reply_offset:i64, has_more:bool)
  HistoryPageData(generation:i64, messages:[ChatMessage])
  ChatSearchHit(channel_id:str, seq:i64, root_seq:i64, author:str, text:str, meta:str)
  ChatSearchData(generation:i64, hits:[ChatSearchHit])
  PageItem(id:str, title:str, parent:str, prefix:str, child_count:i64)
  PageBlock(key:i64, id:str, parent:str, kind:str, text:str, pending:bool, checked:bool, prefix:str, child_count:i64, mark_count:i64)
  PagesData(pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str, selected_block_id:str, selected_block_kind:str, selected_block_text:str, selected_block_checked:bool, page_title_selected:bool)
  BlockInsertResult(data:PagesData, operation_id:str, page_id:str)
  PageCommentThread(id:str, author:str, meta:str, resolved:bool, comment_count:i64)
  PageComment(id:str, ordinal:i64, author:str, meta:str, text:str)
  BlockThreadListData(generation:i64, target:str, from:i64, threads:[PageCommentThread], total:i64, next_from:i64, has_more:bool)
  BlockCommentData(generation:i64, target:str, thread_id:str, from:i64, comments:[PageComment], next_from:i64, has_more:bool)
  BlockCommentsRefreshData(generation:i64, target:str, threads:[PageCommentThread], total:i64, threads_next_from:i64, threads_has_more:bool, thread_id:str, comments:[PageComment], comments_next_from:i64, comments_has_more:bool)
  PageSearchHit(page_id:str, block_id:str, kind:str, text:str)
  PageSearchData(generation:i64, hits:[PageSearchHit])
  AutosaveResult(generation:i64, written:bool)
  WorkspaceData(generation:i64, rpc:str, status:str, height:i64, channels:[ChatChannel], messages:[ChatMessage], active_channel:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, active_channel_huddle_count:i64, channel_members:[ChatMember], pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str)
  BellItem(seq:i64, kind:str, body:str, source:str, height:i64, read:bool)
  BellDelta(kind:str, item:BellItem, up_to_seq:i64)
  BellData(generation:i64, unread:i64, items:[BellItem])
  sync apply_bell(items:[BellItem], delta:BellDelta) -> [BellItem]
  sync bell_unread_after(unread:i64, items:[BellItem], delta:BellDelta) -> i64
  sync bell_head(items:[BellItem]) -> i64
  load_bell(rpc:str, generation:i64) -> BellData ! HydrationError
  mark_bell_read(rpc:str, password:str, up_to_seq:i64) -> bool ! AppError
  LiveUpdate(kind:str, status:str, height:i64, module:str, load_chat:bool, load_pages:bool, debounce:bool, chat:ChatDelta, pages:PagesDelta, bell:BellDelta)
  ComposerCmd()
  AppError(message:str, committed:bool)
  OptimisticMutationError(message:str, committed:bool, operation_id:str, scope_id:str, body:str)
  HydrationError(generation:i64, message:str)
  container-style avatar_style(r:f64, g:f64, b:f64)
  editor-binding composer_keys() -> ComposerCmd
  connect(rpc:str) -> WorkspaceData ! AppError
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
  sync append_thread_page(messages:[ChatMessage], next:[ChatMessage]) -> [ChatMessage]
  sync merge_thread_reply(messages:[ChatMessage], reply:ChatMessage) -> [ChatMessage]
  sync history_has_older(messages:[ChatMessage]) -> bool
  sync oldest_message_seq(messages:[ChatMessage]) -> i64
  sync prepend_history(messages:[ChatMessage], older:[ChatMessage]) -> [ChatMessage]
  sync thread_offset_after_reply(offset:i64, has_more:bool, committed:bool) -> i64
  sync optimistic_block(blocks:[PageBlock], after_id:str, kind:str, text:str, id:str) -> [PageBlock]
  sync merge_pending_blocks(canonical:[PageBlock], current:[PageBlock], current_page:str, next_page:str, settled_id:str) -> [PageBlock]
  sync merge_block_insert_result(canonical:[PageBlock], current:[PageBlock], current_page:str, next_page:str, settled_id:str) -> [PageBlock]
  sync rollback_pending_block(blocks:[PageBlock], pending_id:str, committed:bool) -> [PageBlock]
  sync remember_failed_block(drafts:[str], current:str, pending:str, committed:bool) -> [str]
  sync rollback_blocks(blocks:[PageBlock], keep_pending:bool) -> [PageBlock]
  sync append_page_comment_threads(threads:[PageCommentThread], next:[PageCommentThread]) -> [PageCommentThread]
  sync append_page_comments(comments:[PageComment], next:[PageComment]) -> [PageComment]
  sync restore_draft(current:str, pending:str, keep_pending:bool) -> str
  sync remember_failed_draft(existing:str, current:str, pending:str, committed:bool) -> str
  sync canonical_endpoint(input:str) -> str
  sync connection_degraded(status:str) -> bool
  sync palette_key_action(physical:physical-key, modifiers:key-modifiers, open:bool) -> str
  NavItem(id:str, title:str, icon:str, active:bool)
  FsEntry(path:str, name:str, kind:str, size:i64)
  FsSnapshot(id:str, short_id:str, author:str, height:i64, message:str)
  FsListing(generation:i64, path:str, entries:[FsEntry])
  FsPreview(generation:i64, path:str, text:str, truncated:bool, binary:bool)
  FsHistory(generation:i64, snapshots:[FsSnapshot])
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
  sync shell_nav(tab:str) -> [NavItem]
  SettingsFacts(generation:i64, endpoint:str, node_key:str, height:i64, key_path:str, key_state:str, open_tabs:i64)
  load_settings_facts(rpc:str, generation:i64) -> SettingsFacts ! HydrationError
  clear_doc_tabs(rpc:str) -> bool
  ProposalRow(id:str, action:str, proposer:str, status:str, deadline:i64, approvals:i64, rejections:i64, electorate:i64, open:bool)
  GovernanceData(generation:i64, proposals:[ProposalRow])
  load_governance(rpc:str, generation:i64) -> GovernanceData ! HydrationError
  governance_vote(rpc:str, password:str, proposal_id:str, approve:bool) -> bool ! AppError
  governance_execute(rpc:str, password:str, proposal_id:str) -> bool ! AppError
  MemberRow(key:str, label:str, role:str, is_this_node:bool)
  MembersData(generation:i64, validators:i64, residents:i64, members:[MemberRow])
  load_members(rpc:str, generation:i64) -> MembersData ! HydrationError
  ExplorerBlock(height:i64, hash:str, commit:str, op_count:i64)
  ExplorerOp(height:i64, proposer:str, target:str, disposition:str, op_hash:str, payload:str, trace:str)
  ExplorerData(generation:i64, blocks:[ExplorerBlock], ops:[ExplorerOp])
  sync explorer_ops_at(ops:[ExplorerOp], height:i64) -> [ExplorerOp]
  load_explorer(rpc:str, generation:i64) -> ExplorerData ! HydrationError
  sync slash_kind_matches(draft:str, kinds:[str]) -> [str]
  sync doc_tabs_with(tabs:[str], page_id:str) -> [str]
  sync doc_tabs_without(tabs:[str], page_id:str) -> [str]
  DocTab(id:str, title:str, active:bool)
  sync retain_doc_tabs(tabs:[str], pages:[PageItem]) -> [str]
  sync doc_tab_rows(tabs:[str], pages:[PageItem], active:str) -> [DocTab]
  sync next_doc_tab(tabs:[str], closed:str, active:str) -> str
  load_doc_tabs(rpc:str) -> [str]
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
  sync channel_flag_archived(channels:[ChatChannel], channel:str, current:bool) -> bool
  sync channel_flag_members_only(channels:[ChatChannel], channel:str, current:bool) -> bool
  sync channel_live_huddle_count(channels:[ChatChannel], channel:str, current:i64) -> i64
  sync keep_channels(loaded:bool, next:[ChatChannel], current:[ChatChannel]) -> [ChatChannel]
  sync keep_messages(loaded:bool, next:[ChatMessage], current:[ChatMessage]) -> [ChatMessage]
  sync keep_members(loaded:bool, next:[ChatMember], current:[ChatMember]) -> [ChatMember]
  sync keep_pages(loaded:bool, next:[PageItem], current:[PageItem]) -> [PageItem]
  sync keep_blocks(loaded:bool, next:[PageBlock], current:[PageBlock]) -> [PageBlock]
  sync keep_str(loaded:bool, next:str, current:str) -> str
  sync keep_bool(loaded:bool, next:bool, current:bool) -> bool
  sync keep_i64(loaded:bool, next:i64, current:i64) -> i64
  sync initial_channel_reads(channels:[ChatChannel], existing:[ChannelRead]) -> [ChannelRead]
  sync frozen_unread_boundary(reads:[ChannelRead], channels:[ChatChannel], current_channel:str, next_channel:str, current_boundary:i64) -> i64
  sync first_unread_seq(messages:[ChatMessage], boundary:i64) -> i64
  sync thread_generation_after_refresh(generation:i64, current_channel:str, next_channel:str, previous_root:i64, next_root:i64) -> i64
  sync thread_loading_after_refresh(loading:bool, current_channel:str, next_channel:str, previous_root:i64, next_root:i64) -> bool
  sync retain_thread_messages(messages:[ChatMessage], root_seq:i64) -> [ChatMessage]
  sync cancel_autosaves(rpc:str, generation:i64) -> i64
  sync refreshed_block_draft(blocks:[PageBlock], selected_id:str, current:str, autosave_status:str) -> str
  sync remember_orphaned_block_drafts(drafts:[str], blocks:[PageBlock], selected_id:str, current:str, autosave_status:str) -> [str]
  sync remember_orphaned_comment_drafts(drafts:[str], blocks:[PageBlock], selected_id:str, current:str) -> [str]
  sync remove_recovered_draft(drafts:[str], recovered:str) -> [str]
  sync retain_drafts_for_endpoint(drafts:[str], current:str, next:str) -> [str]
  sync refreshed_selected_block(blocks:[PageBlock], selected_id:str) -> str
  sync retain_selected_string(value:str, selected_id:str) -> str
  sync retain_selected_i64(value:i64, selected_id:str) -> i64
  sync retain_selected_comment_threads(threads:[PageCommentThread], selected_id:str) -> [PageCommentThread]
  sync retain_selected_comments(comments:[PageComment], selected_id:str) -> [PageComment]
  sync cancel_missing_block_autosave(rpc:str, generation:i64, blocks:[PageBlock], selected_id:str) -> i64
  sync scope_key(scope:str, id:str) -> str
  sync block_action_menu_y(pointer_y:f64, viewport_height:f64) -> f64
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
  load_page(rpc:str, page_id:str, selected_block_id:str) -> PagesData ! AppError
  load_block_threads(rpc:str, target:str, from:i64, generation:i64) -> BlockThreadListData ! HydrationError
  load_block_comment_page(rpc:str, target:str, thread_id:str, from:i64, generation:i64) -> BlockCommentData ! HydrationError
  refresh_block_comments(rpc:str, target:str, thread_id:str, generation:i64) -> BlockCommentsRefreshData ! HydrationError
  post_block_comment(rpc:str, password:str, target:str, thread_id:str, text:str, generation:i64) -> BlockCommentData ! AppError
  create_page(rpc:str, password:str, title:str) -> PagesData ! AppError
  autosave_page_title(rpc:str, password:str, page_id:str, title:str) -> bool ! AppError
  delete_page(rpc:str, password:str, page_id:str) -> PagesData ! AppError
  add_block(rpc:str, password:str, page_id:str, after_id:str, kind:str, block_id:str, text:str) -> BlockInsertResult ! OptimisticMutationError
  autosave_block_text(rpc:str, password:str, block_id:str, kind:str, text:str, generation:i64) -> AutosaveResult ! HydrationError
  save_block(rpc:str, password:str, page_id:str, block_id:str, kind:str, text:str) -> PagesData ! AppError
  set_block_checked(rpc:str, password:str, page_id:str, block_id:str, checked:bool) -> PagesData ! AppError
  move_block(rpc:str, password:str, page_id:str, block_id:str, direction:str) -> PagesData ! AppError
  remove_block(rpc:str, password:str, page_id:str, block_id:str) -> PagesData ! AppError
  search_pages(rpc:str, page_id:str, text:str, generation:i64) -> PageSearchData ! HydrationError
