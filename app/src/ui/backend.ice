extern crate::backend
  ChatChannel(id:str, name:str, archived:bool, members_only:bool, huddle_count:i64, head_seq:i64)
  ChatReaction(emoji:str, count:i64, reacted_by_me:bool)
  ChatMember(key:str, label:str)
  ChatSpan(text:str, bold:bool, italic:bool, highlight:bool, link:str)
  ChatBlock(kind:str, text:str, lang:str, rich:bool, spans:[ChatSpan])
  ChatMessage(id:str, seq:i64, author:str, meta:str, body:str, blocks:[ChatBlock], pending:bool, rev:i64, edited:bool, deleted:bool, reply_count:i64, thread_seq:i64, show_author:bool, initial:str, avatar_r:f64, avatar_g:f64, avatar_b:f64, reactions:[ChatReaction])
  ChatData(channels:[ChatChannel], messages:[ChatMessage], active_channel:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, active_channel_huddle_count:i64, channel_members:[ChatMember], selected_message_seq:i64, selected_message_rev:i64, selected_message_body:str, active_thread_seq:i64, thread_target_seq:i64, thread_messages:[ChatMessage], thread_next_reply_offset:i64, thread_has_more:bool)
  ChatSendResult(data:ChatData, operation_id:str, channel_id:str)
  ThreadLoadData(generation:i64, root_seq:i64, target_seq:i64, messages:[ChatMessage], next_reply_offset:i64, has_more:bool)
  ThreadPageData(generation:i64, messages:[ChatMessage], next_reply_offset:i64, has_more:bool)
  LiveThreadData(generation:i64, channel_id:str, root_seq:i64, target_seq:i64, messages:[ChatMessage], next_reply_offset:i64, has_more:bool)
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
  LiveUpdate(kind:str, status:str, height:i64)
  AppError(message:str, committed:bool)
  OptimisticMutationError(message:str, committed:bool, operation_id:str, scope_id:str, body:str)
  HydrationError(generation:i64, message:str)
  container-style avatar_style(r:f64, g:f64, b:f64)
  connect(rpc:str) -> WorkspaceData ! AppError
  stream live_events(rpc:str) -> LiveUpdate
  refresh(rpc:str, channel_id:str, page_id:str, generation:i64) -> WorkspaceData ! HydrationError
  retry_refresh(rpc:str, channel_id:str, page_id:str, generation:i64, attempt:i64) -> WorkspaceData ! HydrationError
  sync fresh_operation_id(prefix:str) -> str
  sync optimistic_message(messages:[ChatMessage], body:str, message_id:str) -> [ChatMessage]
  sync merge_pending_messages(canonical:[ChatMessage], current:[ChatMessage], current_channel:str, next_channel:str, settled_id:str) -> [ChatMessage]
  sync merge_message_send_result(canonical:[ChatMessage], current:[ChatMessage], current_channel:str, next_channel:str, settled_id:str) -> [ChatMessage]
  sync rollback_pending_message(messages:[ChatMessage], pending_id:str, committed:bool) -> [ChatMessage]
  sync contains_pending_message(messages:[ChatMessage], pending_id:str) -> bool
  sync append_thread_page(messages:[ChatMessage], next:[ChatMessage]) -> [ChatMessage]
  sync merge_thread_reply(messages:[ChatMessage], reply:ChatMessage) -> [ChatMessage]
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
  sync retain_for_endpoint(value:str, current:str, next:str) -> str
  sync mutation_failure_phase(committed:bool) -> str
  sync message_seq_after_failure(current:i64, phase:str, committed:bool) -> i64
  sync message_text_after_failure(current:str, phase:str, committed:bool) -> str
  sync message_action_after_failure(current:str, phase:str, committed:bool) -> str
  sync refreshed_required_message_seq(messages:[ChatMessage], current_channel:str, next_channel:str, value:i64) -> i64
  sync refreshed_known_message_seq(messages:[ChatMessage], current_channel:str, next_channel:str, value:i64) -> i64
  sync refreshed_channel_value(current_channel:str, next_channel:str, value:i64) -> i64
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
  create_channel(rpc:str, password:str, name:str) -> ChatData ! AppError
  rename_channel(rpc:str, password:str, channel_id:str, name:str) -> ChatData ! AppError
  archive_channel(rpc:str, password:str, channel_id:str) -> ChatData ! AppError
  unarchive_channel(rpc:str, password:str, channel_id:str) -> ChatData ! AppError
  add_channel_member(rpc:str, password:str, channel_id:str, member_key:str) -> ChatData ! AppError
  remove_channel_member(rpc:str, password:str, channel_id:str, member_key:str) -> ChatData ! AppError
  join_huddle(rpc:str, password:str, channel_id:str) -> ChatData ! AppError
  leave_huddle(rpc:str, password:str, channel_id:str) -> ChatData ! AppError
  send_message(rpc:str, password:str, channel_id:str, message_id:str, body:str) -> ChatSendResult ! OptimisticMutationError
  load_thread(rpc:str, channel_id:str, root_seq:i64, target_seq:i64, through_reply_offset:i64, generation:i64) -> ThreadLoadData ! HydrationError
  load_thread_page(rpc:str, channel_id:str, root_seq:i64, from:i64, generation:i64) -> ThreadPageData ! HydrationError
  refresh_live_thread(rpc:str, channel_id:str, root_seq:i64, target_seq:i64, through_reply_offset:i64, generation:i64) -> LiveThreadData ! HydrationError
  send_reply(rpc:str, password:str, channel_id:str, root_seq:i64, message_id:str, body:str) -> ChatMessage ! OptimisticMutationError
  edit_message(rpc:str, password:str, channel_id:str, seq:i64, base_rev:i64, body:str) -> ChatData ! AppError
  delete_message(rpc:str, password:str, channel_id:str, seq:i64) -> ChatData ! AppError
  add_reaction(rpc:str, password:str, channel_id:str, seq:i64, emoji:str) -> ChatData ! AppError
  remove_reaction(rpc:str, password:str, channel_id:str, seq:i64, emoji:str) -> ChatData ! AppError
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
