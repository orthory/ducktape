// THE WINDOW LAYERS — the bell, the command palette and the block explorer.
// `global_key_pressed` lives here because the palette is what it opens.

on refresh_explorer
  return if !connected || explorer_loading
  explorer_generation = explorer_generation + 1
  explorer_loading = true
  run load_explorer(connected_rpc, explorer_generation) -> explorer_loaded _ | explorer_failed _

on explorer_loaded(next)
  return if next.generation != explorer_generation
  explorer_loading = false
  explorer_blocks = next.blocks
  explorer_ops = next.ops

on explorer_failed(cause)
  return if cause.generation != explorer_generation
  explorer_loading = false
  error = cause.message

on select_explorer_block(height)
  explorer_selected = height

on close_palette
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
  run mark_bell_read(connected_rpc, password, bell_head(bell_items)) -> bell_marked _ | mutation_failed _

on bell_loaded(next)
  return if next.generation != bell_generation
  bell_unread = next.unread
  bell_items = next.items

on bell_failed(cause)
  return if cause.generation != bell_generation

on bell_marked(_result)
  error = error

on global_key_pressed(event)
  // THE ESCAPE LADDER — one decide-fn names the single topmost TRANSIENT
  // layer this key dismisses (the z-order lives in `escape_target`), and
  // every closable flag self-selects against the verdict: the handler
  // grammar has no branches, so the keepers ARE the routing. Sits before
  // the palette block, whose open path must end in its focus task.
  escape_key = escape_target(event.key, palette_open, bell_open, channel_create_open, thread_message_action, message_action, channel_settings_open, forge_repo_menu)
  bell_open = bell_open && escape_key != "bell"
  channel_create_open = channel_create_open && escape_key != "channel_create"
  thread_selected_seq = keep_i64(escape_key == "thread_menu", 0, thread_selected_seq)
  thread_selected_rev = keep_i64(escape_key == "thread_menu", 0, thread_selected_rev)
  thread_message_action = keep_str(escape_key == "thread_menu", "toolbar", thread_message_action)
  thread_edit_draft = keep_str(escape_key == "thread_menu", "", thread_edit_draft)
  selected_message_seq = keep_i64(escape_key == "message_menu", 0, selected_message_seq)
  selected_message_rev = keep_i64(escape_key == "message_menu", 0, selected_message_rev)
  message_action = keep_str(escape_key == "message_menu", "toolbar", message_action)
  message_edit_draft = keep_str(escape_key == "message_menu", "", message_edit_draft)
  channel_settings_open = channel_settings_open && escape_key != "channel_settings"
  forge_repo_menu = forge_repo_menu && escape_key != "repo_menu"
  // The composer's formatting chords (Cmd/Ctrl+B/I, +Shift+C, +Shift+9). The
  // editor lets command-letter presses bubble on purpose, so its focus is
  // still on the draft when the mark lands; an empty verdict is a no-op.
  //
  // ONE chord, TWO composers: the subscription is global and sees no widget
  // focus, so `composer_focus` (stamped by whichever composer last took an
  // event) picks the target and the other arm's gate goes false — the two
  // lines are exclusive, never both. Without the split, Cmd+B pressed with the
  // caret in a thread reply silently bolded the CHANNEL draft instead.
  let reply_chord = active_thread_seq > 0 && composer_focus == "reply"
  message_editor = composer_toggle_mark(message_editor, composer_mark_shortcut(event.key, event.physical_key, event.modifiers, (connected && shell_tab == "chat" && !palette_open && !reply_chord)))
  reply_editor = composer_toggle_mark(reply_editor, composer_mark_shortcut(event.key, event.physical_key, event.modifiers, (connected && shell_tab == "chat" && !palette_open && reply_chord)))
  // The page document's undo/redo (Cmd/Ctrl+Z, +Shift+Z) — the editor
  // bubbles command-letter chords on purpose; an off-pages press is the
  // identity.
  page_editor = page_history_key(page_editor, event.key, event.physical_key, event.modifiers, (connected && shell_tab == "pages" && !palette_open))
  palette_key = palette_key_action(event.key, event.physical_key, event.modifiers, palette_open)
  return if palette_key == "none"
  return if palette_key == "open" && !connected
  palette_open = palette_key == "open"
  palette_key = ""
  palette_draft = ""
  palette_chat_hits = []
  palette_page_hits = []
  palette_generation = palette_generation + 1
  palette_searching = false
  return if !palette_open
  task widget focus #workspace-tabs/overlays/palette-input

// THE CONTENT PANE'S KEYBOARD SCROLL. iced's scrollable has no focus and no
// key handling, so Page Down over Settings moved nothing — and neither did
// Home, End or the arrows, on any screen. One decide-fn turns the press into
// a pixel delta and every full-pane content scroll takes the same delta: the
// shell mounts exactly ONE of these at a time (`match tab` in WorkspaceTabs),
// and `scroll-by` against a pane that is not on screen is a no-op, so naming
// them all IS the routing — the same shape as the escape ladder above, where
// the keepers do the dispatch.
//
// The multi-pane screens (chat, pages, files, forge, the explorer) are absent
// on purpose: they show two or three scrolls side by side and nothing here can
// say which one the reader means. Giving them a keyboard scroll is a focus
// design, not a bug fix, and guessing a pane would move the wrong one.
on content_scroll_key(event)
  content_scroll = content_scroll_step(event.key, event.modifiers)
  return if content_scroll == 0.0
  parallel
    task widget scroll-by #workspace-tabs/content/settings/settings-body 0.0 content_scroll
    task widget scroll-by #workspace-tabs/content/governance/approvals-body 0.0 content_scroll
    task widget scroll-by #workspace-tabs/content/members/members-body 0.0 content_scroll
    task widget scroll-by #workspace-tabs/content/agents/agents-body 0.0 content_scroll

on palette_changed(next)
  palette_draft = next
  palette_generation = palette_generation + 1
  palette_searching = !empty(trim(palette_draft))
  return if empty(trim(palette_draft))
  run palette_search(connected_rpc, trim(palette_draft), palette_generation) -> palette_results _ | palette_search_failed _

on palette_results(next)
  return if next.generation != palette_generation
  palette_chat_hits = next.chat_hits
  palette_page_hits = next.page_hits
  palette_searching = false

on palette_search_failed(cause)
  return if cause.generation != palette_generation
  palette_searching = false

// Held here beside the loaders that fill them.
state
  // The escape ladder's verdict for the keepers above — state, not `let`:
  // the checker cannot type a subscription payload's field inside a let.
  escape_key = ""
  // Same reason as `escape_key`: a subscription payload's field cannot be
  // typed inside a `let`.
  content_scroll = 0.0
  explorer_hits:[ExplorerHit] = []
  explorer_kinds:[KindCount] = []
  explorer_searching = false
  explorer_search_generation:i64 = 0

on explorer_search_submit
  return if !connected || explorer_searching || empty(trim(explorer_query))
  explorer_search_generation = explorer_search_generation + 1
  explorer_searching = true
  explorer_hits = []
  explorer_kinds = []
  explorer_kind = "all"
  error = ""
  run search_workspace(connected_rpc, trim(explorer_query), explorer_search_generation) -> explorer_results_loaded _ | explorer_search_failed _

on explorer_results_loaded(next)
  return if next.generation != explorer_search_generation
  explorer_hits = next.hits
  explorer_kinds = next.kinds
  explorer_searching = false
  error = ""

on explorer_search_failed(cause)
  return if cause.generation != explorer_search_generation
  explorer_searching = false
  error = cause.message

on clear_explorer_search
  explorer_search_generation = explorer_search_generation + 1
  explorer_query = ""
  explorer_hits = []
  explorer_kinds = []
  explorer_kind = "all"
  explorer_searching = false

// The kind strip's only act. `explorer_kind` has been in state.ice since wave 1
// with nothing writing it and nothing reading it.
on pick_explorer_kind(kind)
  explorer_kind = kind
