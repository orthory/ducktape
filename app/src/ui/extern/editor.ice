// The rich composer boundary. `crate::editor` wraps
// `ui_lang_runtime::RichTextEditor` — the cached-line rich layout and the IME
// hardening live in that widget, not in the stock Ice `editor` — and
// classifies each interaction at the widget's own key binding, where the
// press's live modifiers are known (ducktape-ui#601): plain Enter is
// `Submit`, everything else is `Apply`. `apply_composer_event` is a no-op on
// `Submit`, so a flat handler applies first and then guards on
// `composer_submits`; handlers have no `if` blocks and never need one here.
extern crate::editor
  ComposerEvent()
  component rich_composer(document:&editor, hint:str, disabled:bool, min_h:f64, max_h:f64, pad:f64) -> ComposerEvent
  sync apply_composer_event(document:editor, event:ComposerEvent) -> editor
  pure composer_submits(event:ComposerEvent) -> bool
  pure composer_submit_event() -> ComposerEvent
  sync composer_toggle_mark(document:editor, kind:str) -> editor

// THE PAGE DOCUMENT — one editor over the whole page, not one per block.
// Every key is a pure buffer edit (`crate::pages`); nothing here writes to the
// node. The dirty-gated tick in handlers/pages.ice is the only write path.
extern crate::pages
  PageEvent()
  component page_document(document:&editor, dark:bool, disabled:bool, blocks:[PageBlock], hits:[str]) -> PageEvent
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
