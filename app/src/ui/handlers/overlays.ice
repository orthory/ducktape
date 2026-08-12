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
  escape_key = escape_target(event.key, palette_open, bell_open, channel_create_open, thread_message_action, message_action, channel_settings_open, forge_repo_menu)
  // The composer's formatting chords (Cmd/Ctrl+B/I, +Shift+C, +Shift+9). The
  // editor lets command-letter presses bubble on purpose, so its focus is
  // still on the draft when the mark lands; an empty verdict is a no-op.
  //
  // ONE chord, TWO composers, and BOTH arms self-select on a POSITIVE match of
  // `composer_focus` — the same keeper shape as the escape ladder below. The
  // negation this replaced (`!reply_chord`) made the stream's composer the
  // fallback for every state, so the moment the discriminant said "the caret
  // left" the chord went right back to marking a draft nobody was in.
  let chord_ready = connected && shell_tab == "chat"
  let message_chord = chord_ready && composer_focus == "message"
  // A closed rail can never be the target, however stale the discriminant is —
  // and one route leaves it stale ON PURPOSE. Every rail teardown a USER asks
  // for retires (they all write a literal `active_thread_seq = 0`, which is a
  // linted rule), but `live_resynced` closes the rail when the thread root is
  // deleted under you, and that same handler runs on every ordinary resync
  // while you keep typing in the rail — so it cannot retire unconditionally.
  // This term is that route's only cover; the lint pins it by driving it.
  let reply_chord = chord_ready && composer_focus == "reply" && active_thread_seq > 0
  // The page document's undo/redo (Cmd/Ctrl+Z, +Shift+Z) — the editor
  // bubbles command-letter chords on purpose; an off-pages press names no move.
  let pages_ready = connected && shell_tab == "pages" && !palette_open
  palette_key = palette_key_action(event.key, event.physical_key, event.modifiers, palette_open)
  return if empty(escape_key) && palette_key == "none" && empty(composer_mark_shortcut(event.key, event.physical_key, event.modifiers, message_chord || reply_chord)) && empty(page_history_shortcut(event.key, event.physical_key, event.modifiers, pages_ready))
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
  message_editor = composer_toggle_mark(message_editor, composer_mark_shortcut(event.key, event.physical_key, event.modifiers, message_chord))
  reply_editor = composer_toggle_mark(reply_editor, composer_mark_shortcut(event.key, event.physical_key, event.modifiers, reply_chord))
  page_editor = page_history_key(page_editor, page_history_shortcut(event.key, event.physical_key, event.modifiers, pages_ready))
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
  // THE PALETTE TAKES THE CARET, and closing it does not hand it back — so the
  // retire is here, at the open, not on a `!palette_open` term in the chord's
  // own gate. That term could only mute the chord while the palette was up;
  // Escape then handed a stale "reply" straight back to it.
  composer_focus = "none"
  task widget focus #workspace-tabs/overlays/palette-input

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
  content_scroll = content_scroll_step(event.key, event.modifiers, topmost_overlay(palette_open, bell_open, channel_create_open, thread_message_action, message_action, channel_settings_open, forge_repo_menu))
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
  // Names the sources that did not answer this search, empty when they all
  // did. A timed-out source contributes no hits and no chip, so without this
  // line the screen presents whatever survived as the whole truth.
  explorer_partial = ""
  explorer_searching = false
  explorer_search_generation:i64 = 0

on explorer_search_submit
  return if !connected || explorer_searching || empty(trim(explorer_query))
  explorer_search_generation = explorer_search_generation + 1
  explorer_searching = true
  explorer_hits = []
  explorer_kinds = []
  explorer_partial = ""
  explorer_kind = "all"
  error = ""
  run search_workspace(connected_rpc, trim(explorer_query), explorer_search_generation) -> explorer_results_loaded _ | explorer_search_failed _

on explorer_results_loaded(next)
  return if next.generation != explorer_search_generation
  explorer_hits = next.hits
  explorer_kinds = next.kinds
  explorer_partial = next.partial
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
  explorer_partial = ""
  explorer_kind = "all"
  explorer_searching = false

// The kind strip's only act. `explorer_kind` has been in state.ice since wave 1
// with nothing writing it and nothing reading it.
on pick_explorer_kind(kind)
  explorer_kind = kind
