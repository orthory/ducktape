// THE WINDOW LAYERS — the bell, the command palette and the block explorer.
// `global_key_pressed` lives here because the palette is what it opens.

// THE EXPLORER SPEAKS THE KERNEL CONTRACT: it reads the block window and
// runs its workspace search itself, so the only thing it asks the app for is
// the clipboard — an OS door no view holds.
on explorer_view_event(event)
  return if event.kind != "copy"
  toast = event_text(event, "label")
  toast_age = 0
  task clipboard write event_text(event, "text")

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
  return if !bell_open
  bell_error = ""
  run replace lane=bell_load load_bell(connected_rpc, account_number) -> bell_loaded connect_generation account_number _ | bell_failed connect_generation account_number _

on reload_bell
  bell_error = ""
  run replace lane=bell_load load_bell(connected_rpc, account_number) -> bell_loaded connect_generation account_number _ | bell_failed connect_generation account_number _

on close_bell
  bell_open = false

on mark_bell_read_submit
  return if bell_unread <= 0 || bell_marking
  bell_marking = true
  bell_error = ""
  run replace lane=bell_mark mark_bell_read(connected_rpc, password, account_number, bell_head(bell_items)) -> bell_marked connect_generation account_number _ | bell_mark_failed connect_generation account_number _

on bell_loaded(generation, account, next)
  return if generation != connect_generation || account != account_number
  bell_error = ""
  bell_items = merge_bell_loaded(bell_items, next.items, bell_read_through, bell_clear_through)
  bell_presentations = merge_bell_presentations(bell_visible_items(bell_items, account_number, settings_user_key), bell_presentations, next.presentations)
  bell_unread = bell_unread_count(bell_items, account_number, settings_user_key)

on bell_context_loaded(generation, account, next)
  return if generation != connect_generation || account != account_number
  bell_presentations = merge_bell_presentations(bell_visible_items(bell_items, account_number, settings_user_key), bell_presentations, next)
  bell_error = ""

on bell_failed(generation, account, cause)
  return if generation != connect_generation || account != account_number
  bell_error = cause.message

on bell_marked(generation, account, delta)
  return if generation != connect_generation || account != account_number
  bell_marking = false
  bell_read_through = keep_i64(delta.up_to_seq > bell_read_through, delta.up_to_seq, bell_read_through)
  bell_items = apply_bell(bell_items, delta)
  bell_unread = bell_unread_count(bell_items, account_number, settings_user_key)

on bell_mark_failed(generation, account, cause)
  return if generation != connect_generation || account != account_number
  bell_marking = false
  bell_error = cause.message

on bell_open_item(generation, account, context)
  return if generation != connect_generation || account != account_number
  return if context.target == BellTarget.unavailable || empty(context.object)
  bell_open = false
  match context.target
    BellTarget.run
      flow
        from done context.object
        done -> open_run_panel _
    BellTarget.page
      run replace lane=bell_navigation duck_echo_str(context.object) -> open_page_search_hit(_, context.anchor) | external_url_failed _
    BellTarget.message
      flow
        from done bell_link(context, network_chain_id)
        done -> open_message_link _
    BellTarget.forge
      flow
        from done bell_link(context, network_chain_id)
        done -> open_message_link _
    BellTarget.repo
      flow
        from done bell_link(context, network_chain_id)
        done -> open_message_link _
    BellTarget.unavailable
      return if true

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
  let escape_key = escape_target(event.key, shell_tab, palette_open, bell_open, channel_create_open, thread_message_action, message_action, channel_settings_open, fs_delete_target)
  // THE COMPOSER'S FORMATTING CHORDS ARE NOT HERE ANY MORE. They land at the
  // widget that has the caret — `RichTextEditor::on_chord` (ducktape-ui#711)
  // is offered exactly the presses the bubble contract releases — so the
  // composer instance marks its OWN content and this subscription never
  // needed a `composer_focus` discriminant to guess which one was focused.
  // The page document's undo/redo (Cmd/Ctrl+Z, +Shift+Z) — the editor
  // bubbles command-letter chords on purpose; an off-pages press names no move.
  let palette_key = palette_key_action(event.key, event.physical_key, event.modifiers, palette_open)
  return if empty(escape_key) && palette_key == "none"
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
  fs_delete_target = keep_str(escape_key == "fs_delete", "", fs_delete_target)
  return if palette_key == "none"
  return if palette_key == "open" && !connected
  invalidate lane=palette_search
  palette_open = palette_key == "open"
  palette_draft = ""
  palette_chat_hits = []
  palette_page_hits = []
  palette_search_phase = SearchPhase.idle
  return if !palette_open
  task widget focus #workspace-tabs/overlays/palette-input

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
