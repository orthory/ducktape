// The rich composer boundary. `crate::editor` wraps
// `ui_lang_runtime::RichTextEditor` — the cached-line rich layout and the IME
// hardening live in that widget, not in the stock Ice `editor` — and
// classifies each interaction where the shift state is known: plain Enter is
// `Submit`, everything else is `Apply`. `apply_composer_event` is a no-op on
// `Submit`, so a flat handler applies first and then guards on
// `composer_submits`; handlers have no `if` blocks and never need one here.
extern crate::editor
  ComposerEvent()
  component rich_composer(document:&editor, hint:str, disabled:bool, shift:bool, min_h:f64, max_h:f64, pad:f64) -> ComposerEvent
  sync apply_composer_event(document:editor, event:ComposerEvent) -> editor
  sync composer_submits(event:ComposerEvent) -> bool
  sync composer_submit_event() -> ComposerEvent
  sync composer_toggle_mark(document:editor, kind:str) -> editor
  sync composer_mark_shortcut(logical:key, physical:physical-key, modifiers:key-modifiers, chat_ready:bool) -> str

// THE PAGE DOCUMENT — one editor over the whole page, not one per block.
// Every key is a pure buffer edit (`crate::pages`); nothing here writes to the
// node. The dirty-gated tick in handlers/pages.ice is the only write path.
extern crate::pages
  PageEvent()
  component page_document(document:&editor, dark:bool, disabled:bool, commented:[i64]) -> PageEvent
  sync apply_page_event(document:editor, event:PageEvent) -> editor
  sync page_link_of(event:PageEvent) -> str
  sync page_opens_comments(event:PageEvent) -> bool
  sync page_history_key(document:editor, logical:key, physical:physical-key, modifiers:key-modifiers, ready:bool) -> editor
  sync page_text(document:editor) -> str
  sync has_unclosed_fence(text:str) -> bool
  sync block_at_line_target(blocks:[PageBlock], line:i64) -> str
  sync commented_lines(blocks:[PageBlock], targets:[str]) -> [i64]
  sync comment_anchor_label(blocks:[PageBlock], target:str, page_id:str) -> str
  sync comment_compose_hint(blocks:[PageBlock], target:str, page_id:str) -> str
