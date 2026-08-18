// THE WINDOW LAYERS — the bell, the command palette and the block explorer.
// `global_key_pressed` lives here because the palette is what it opens.

on refresh_explorer
  return if !connected || explorer_loading
  explorer_generation = explorer_generation + 1
  explorer_loading = true
  run replace lane=explorer_load load_explorer(connected_rpc, explorer_generation) -> explorer_loaded _ | explorer_failed _

on explorer_loaded(next)
  return if next.generation != explorer_generation
  explorer_loading = false
  explorer_blocks = next.blocks
  explorer_ops = next.ops

on explorer_failed(cause)
  return if cause.generation != explorer_generation
  explorer_loading = false
  error = cause.message

on close_palette
  invalidate lane=palette_search
  // The invalidate dropped the reply that would have moved the phase — park
  // it idle, or the next open path inherits a permanent "Searching…".
  palette_search_phase = SearchPhase.idle
  palette_open = false

// Opening the bell only opens it. Marking read is the Mark-all-read button's
// job — doing it here cleared the badge and every unread row before the list
// painted, and left that button with nothing to do.
on toggle_bell
  bell_open = !bell_open

on close_bell
  bell_open = false

on mark_bell_read_submit
  return if bell_unread <= 0
  run every mark_bell_read(connected_rpc, password, bell_head(bell_items)) -> bell_marked _ | mutation_failed _

on bell_loaded(next)
  bell_unread = next.unread
  bell_items = next.items

on bell_failed(_cause)

on bell_marked(_result)

on global_key_pressed(event)
  // EVERY VERDICT THIS HANDLER CAN ACT ON, RESOLVED FIRST — then the press
  // that means none of them leaves before it can cost anything.
  //
  // This subscription sees EVERY key, so an ordinary letter typed into the
  // composer used to walk the whole body: the escape ladder's dozen keepers,
  // and then THREE `editor` self-assignments. Each of those lowers to
  // `mem::take(&mut self.<editor>)`, which leaves a `Content::default()`
  // behind — a fresh cosmic-text buffer built under a WRITE lock on the
  // process-global font system — so a chord that fires once in a thousand
  // presses charged three of them per keystroke, serialized against whatever
  // the renderer was shaping. The guard calls the SAME four decide-fns its
  // statements do, so it cannot drift from what it guards — and it repeats them
  // rather than naming them, because a subscription payload's fields do not
  // type inside a `let` (E151, the same reason `plane_live_hit` is an extern).
  // They are pure key classifications; the cost this is here for is the take.
  //
  // THE ESCAPE LADDER — one decide-fn names the single topmost TRANSIENT
  // layer this key dismisses (the z-order lives in `escape_target`), and
  // every closable flag self-selects against the verdict: the handler
  // grammar has no branches, so the keepers ARE the routing. Sits before
  // the palette block, whose open path must end in its focus task.
  let escape_key = escape_target(event.key, shell_tab, palette_open, bell_open, channel_create_open, thread_message_action, message_action, channel_settings_open, forge_repo_menu)
  // THE COMPOSER'S FORMATTING CHORDS ARE NOT HERE ANY MORE. They land at the
  // widget that has the caret — `RichTextEditor::on_chord` (ducktape-ui#711)
  // is offered exactly the presses the bubble contract releases — so the
  // composer instance marks its OWN content and this subscription never
  // needed a `composer_focus` discriminant to guess which one was focused.
  // The page document's undo/redo (Cmd/Ctrl+Z, +Shift+Z) — the editor
  // bubbles command-letter chords on purpose; an off-pages press names no move.
  let pages_ready = connected && shell_tab == ShellTab.pages && !palette_open
  let palette_key = palette_key_action(event.key, event.physical_key, event.modifiers, palette_open)
  return if empty(escape_key) && palette_key == "none" && empty(page_history_shortcut(event.key, event.physical_key, event.modifiers, pages_ready))
  bell_open = bell_open && escape_key != "bell"
  channel_create_open = channel_create_open && escape_key != "channel_create"
  thread_selected_seq = keep_i64(escape_key == "thread_menu", 0, thread_selected_seq)
  thread_selected_rev = keep_i64(escape_key == "thread_menu", 0, thread_selected_rev)
  thread_message_action = close_message_action(escape_key == "thread_menu", thread_message_action)
  thread_edit_draft = keep_str(escape_key == "thread_menu", "", thread_edit_draft)
  selected_message_seq = keep_i64(escape_key == "message_menu", 0, selected_message_seq)
  selected_message_rev = keep_i64(escape_key == "message_menu", 0, selected_message_rev)
  message_action = close_message_action(escape_key == "message_menu", message_action)
  message_edit_draft = keep_str(escape_key == "message_menu", "", message_edit_draft)
  channel_settings_open = channel_settings_open && escape_key != "channel_settings"
  forge_repo_menu = forge_repo_menu && escape_key != "repo_menu"
  page_editor = page_history_key(page_editor, page_history_shortcut(event.key, event.physical_key, event.modifiers, pages_ready))
  return if palette_key == "none"
  return if palette_key == "open" && !connected
  invalidate lane=palette_search
  palette_open = palette_key == "open"
  palette_draft = ""
  palette_chat_hits = []
  palette_page_hits = []
  palette_search_phase = SearchPhase.idle
  return if !palette_open
  task widget focus #workspace-tabs/overlays/palette-input window=window_target(console_win)

// THE CONTENT PANE'S KEYBOARD SCROLL. iced's scrollable has no focus and no
// key handling, so Page Down over Settings moved nothing — and neither did
// Home or End, on any screen. One decide-fn turns the press into a pixel delta
// and every full-pane content scroll takes the same delta: the shell mounts
// exactly ONE of these at a time (`match tab` in WorkspaceTabs), and
// `scroll-by` against a pane that is not on screen is a no-op, so naming them
// all IS the routing — the same shape as the escape ladder above, where the
// keepers do the dispatch.
//
// `topmost_overlay` is the SAME reading the escape ladder takes, not a second
// derivation of it: with the palette or the bell up, the pane the reader can
// see is not the one a `scroll-by` would move, and `content_scroll_step`
// answers 0.0 for every key while any layer is over the content.
//
// The multi-pane screens (chat, pages, files, forge, the explorer) are absent
// on purpose: they show two or three scrolls side by side and nothing here can
// say which one the reader means. Giving them a keyboard scroll is a focus
// design, not a bug fix, and guessing a pane would move the wrong one.
on content_scroll_key(event)
  let content_scroll = content_scroll_step(event.key, event.modifiers, topmost_overlay(shell_tab, palette_open, bell_open, channel_create_open, thread_message_action, message_action, channel_settings_open, forge_repo_menu))
  return if content_scroll == 0.0
  parallel
    task widget scroll-by #workspace-tabs/content/settings/settings-body 0.0 content_scroll window=window_target(console_win)
    task widget scroll-by #workspace-tabs/content/node/node-body 0.0 content_scroll window=window_target(console_win)
    task widget scroll-by #workspace-tabs/content/governance/approvals-body 0.0 content_scroll window=window_target(console_win)
    task widget scroll-by #workspace-tabs/content/members/members-body 0.0 content_scroll window=window_target(console_win)
    task widget scroll-by #workspace-tabs/content/agents/agents-body 0.0 content_scroll window=window_target(console_win)

on palette_changed(next)
  invalidate lane=palette_search
  palette_draft = next
  palette_search_phase = SearchPhase.idle
  // THE ROWS BELONG TO THE OLD QUERY, AND THE RESULTS ARM IS KEYED ON THE HITS
  // ALONE (`!empty(chat_hits) || !empty(page_hits)`), so anything left here
  // renders as the answer for what is in the box now. Cleared ABOVE the early
  // return, which is the path that matters most: backspacing to empty runs
  // no search at all, and the previous query's hits were left listed under a
  // blank field with nothing coming to replace them.
  palette_chat_hits = []
  palette_page_hits = []
  return if empty(trim(palette_draft))
  palette_search_phase = SearchPhase.searching
  run replace lane=palette_search palette_search(connected_rpc, trim(palette_draft)) -> palette_results _ | palette_search_failed _

on palette_results(next)
  palette_chat_hits = next.chat_hits
  palette_page_hits = next.page_hits
  // The only place `done` is written — an answer, empty or not, has landed.
  palette_search_phase = SearchPhase.done

on palette_search_failed(cause)
  // BACK TO "idle", NOT "done" — a search that never ran did not find nothing.
  // Idle under a live draft is reachable no other way, and the panel's FAILURE
  // arm reads exactly that pair; the rationale for why no `error` assignment
  // could speak here lives on that arm in `screens/overlays.ice`.
  palette_search_phase = SearchPhase.idle
  // AND THE HITS GO WITH IT. The results arm is keyed on the hits alone, so a
  // failure that kept them rendered "Search failed." directly above the
  // previous query's live, clickable rows — read as results for the query
  // that just failed.
  palette_chat_hits = []
  palette_page_hits = []
