extern crate::backend
  ChatChannel(id:str, name:str, archived:bool, members_only:bool, huddle_count:i64, head_seq:i64)
  ChatReaction(emoji:str, count:i64)
  ChatMember(key:str, label:str)
  ChatMessage(id:str, seq:i64, author:str, meta:str, body:str, pending:bool, rev:i64, edited:bool, deleted:bool, reply_count:i64, thread_seq:i64, reactions:[ChatReaction])
  ChatData(channels:[ChatChannel], messages:[ChatMessage], active_channel:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, active_channel_huddle_count:i64, channel_members:[ChatMember])
  ThreadData(root_seq:i64, messages:[ChatMessage])
  ChatSearchHit(channel_id:str, seq:i64, author:str, text:str, meta:str)
  ChatSearchData(hits:[ChatSearchHit])
  PageItem(id:str, title:str, parent:str, prefix:str, child_count:i64)
  PageBlock(id:str, parent:str, kind:str, text:str, pending:bool, checked:bool, prefix:str, child_count:i64, mark_count:i64)
  PagesData(pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str)
  PageSearchHit(page_id:str, block_id:str, kind:str, text:str)
  PageSearchData(hits:[PageSearchHit])
  WorkspaceData(generation:i64, rpc:str, status:str, height:i64, channels:[ChatChannel], messages:[ChatMessage], active_channel:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, active_channel_huddle_count:i64, channel_members:[ChatMember], pages:[PageItem], blocks:[PageBlock], active_page:str, active_page_title:str, active_page_parent:str)
  LiveUpdate(kind:str, status:str, height:i64)
  AppError(message:str)
  HydrationError(generation:i64, message:str)
  connect(rpc:str) -> WorkspaceData ! AppError
  stream live_events(rpc:str) -> LiveUpdate
  refresh(rpc:str, channel_id:str, page_id:str, generation:i64) -> WorkspaceData ! HydrationError
  retry_refresh(rpc:str, channel_id:str, page_id:str, generation:i64, attempt:i64) -> WorkspaceData ! HydrationError
  sync optimistic_message(messages:[ChatMessage], body:str) -> [ChatMessage]
  sync rollback_messages(messages:[ChatMessage]) -> [ChatMessage]
  sync optimistic_block(blocks:[PageBlock], kind:str, text:str) -> [PageBlock]
  sync rollback_blocks(blocks:[PageBlock]) -> [PageBlock]
  sync restore_draft(current:str, pending:str) -> str
  load_chat(rpc:str, channel_id:str) -> ChatData ! AppError
  create_channel(rpc:str, password:str, name:str) -> ChatData ! AppError
  rename_channel(rpc:str, password:str, channel_id:str, name:str) -> ChatData ! AppError
  archive_channel(rpc:str, password:str, channel_id:str) -> ChatData ! AppError
  unarchive_channel(rpc:str, password:str, channel_id:str) -> ChatData ! AppError
  add_channel_member(rpc:str, password:str, channel_id:str, member_key:str) -> ChatData ! AppError
  remove_channel_member(rpc:str, password:str, channel_id:str, member_key:str) -> ChatData ! AppError
  join_huddle(rpc:str, password:str, channel_id:str) -> ChatData ! AppError
  leave_huddle(rpc:str, password:str, channel_id:str) -> ChatData ! AppError
  send_message(rpc:str, password:str, channel_id:str, body:str) -> ChatData ! AppError
  load_thread(rpc:str, channel_id:str, root_seq:i64) -> ThreadData ! AppError
  send_reply(rpc:str, password:str, channel_id:str, root_seq:i64, body:str) -> ThreadData ! AppError
  edit_message(rpc:str, password:str, channel_id:str, seq:i64, base_rev:i64, body:str) -> ChatData ! AppError
  delete_message(rpc:str, password:str, channel_id:str, seq:i64) -> ChatData ! AppError
  add_reaction(rpc:str, password:str, channel_id:str, seq:i64, emoji:str) -> ChatData ! AppError
  search_chat(rpc:str, channel_id:str, text:str) -> ChatSearchData ! AppError
  load_page(rpc:str, page_id:str) -> PagesData ! AppError
  create_page(rpc:str, password:str, title:str) -> PagesData ! AppError
  autosave_page_title(rpc:str, password:str, page_id:str, title:str) -> bool ! AppError
  delete_page(rpc:str, password:str, page_id:str) -> PagesData ! AppError
  add_block(rpc:str, password:str, page_id:str, after_id:str, kind:str, text:str) -> PagesData ! AppError
  autosave_block_text(rpc:str, password:str, block_id:str, kind:str, text:str) -> bool ! AppError
  save_block(rpc:str, password:str, page_id:str, block_id:str, kind:str, text:str) -> PagesData ! AppError
  set_block_checked(rpc:str, password:str, page_id:str, block_id:str, checked:bool) -> PagesData ! AppError
  move_block(rpc:str, password:str, page_id:str, block_id:str, direction:str) -> PagesData ! AppError
  remove_block(rpc:str, password:str, page_id:str, block_id:str) -> PagesData ! AppError
  search_pages(rpc:str, page_id:str, text:str) -> PageSearchData ! AppError
