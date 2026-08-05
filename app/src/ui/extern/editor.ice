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
  // The pages block editor's seams on the stock Ice `editor` widget: the
  // structural keys become one checked event (classified in Rust where the
  // modifiers are known), and the highlighter is the composers' own inline
  // markdown table, so a block lights up exactly like a message draft.
  BlockKeyEvent(action:str)
  editor-binding block_key_press(kind:str, empty:bool, has_children:bool) -> BlockKeyEvent
  editor-highlighter page_inline_marks()
