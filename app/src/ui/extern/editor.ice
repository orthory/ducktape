extern crate::pages
  pure has_unclosed_fence(text:str) -> bool
  pure block_at_line_target(blocks:[PageBlock], line:i64) -> str
  PageCommentThreadRow(thread:PageCommentThread, anchor:str)
  pure page_comment_thread_rows(blocks:[PageBlock], threads:[PageCommentThread], page_id:str) -> [PageCommentThreadRow]
  pure comment_thread_for_target(threads:[PageCommentThread], target:str, selected:str) -> str
  pure comment_rows_for_target(rows:[PageCommentThreadRow], target:str) -> [PageCommentThreadRow]
  pure comment_anchor_label(blocks:[PageBlock], target:str, page_id:str) -> str
  pure comment_compose_hint(blocks:&[PageBlock], target:&str, page_id:&str) -> str

extern crate::module_view::pages_document
  AcceptedDocument(comment_draft:str, accepted:bool, text:str, cursor_line:i64, comment_line:i64, link:str)
  sync accept_page_document(event:ModuleViewEvent, network:str, page:str) -> AcceptedDocument
  CurrentDocument(ready:bool, text:str)
  sync current_page_document(network:str, page:str, fallback:str) -> CurrentDocument
