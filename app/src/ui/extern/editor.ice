// THE PAGE DOCUMENT — one editor over the whole page, not one per block.
// Every key is a pure buffer edit (`crate::pages`); nothing here writes to the
// node. The dirty-gated tick in handlers/pages.ice is the only write path.
extern crate::pages
  PageEvent()
  // the oldest event the host-painted document queued (`crate::pages::surface`),
  // one per `edited` intent the pages view relays
  sync page_document_take() -> PageEvent
  sync apply_page_event(document:editor, event:PageEvent) -> editor
  pure page_link_of(event:PageEvent) -> str
  pure page_opens_comments(event:PageEvent) -> bool
  pure page_history_shortcut(logical:key, physical:physical-key, modifiers:key-modifiers, ready:bool) -> str
  sync page_history_key(document:editor, action:str) -> editor
  pure has_unclosed_fence(text:str) -> bool
  pure block_at_line_target(blocks:[PageBlock], line:i64) -> str
  PageCommentThreadRow(thread:PageCommentThread, anchor:str)
  pure page_comment_thread_rows(blocks:[PageBlock], threads:[PageCommentThread], page_id:str) -> [PageCommentThreadRow]
  pure comment_anchor_label(blocks:[PageBlock], target:str, page_id:str) -> str
  pure comment_compose_hint(blocks:&[PageBlock], target:&str, page_id:&str) -> str
