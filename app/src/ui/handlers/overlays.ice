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

on palette_changed(next)
  palette_draft = next
  palette_generation = palette_generation + 1
  palette_searching = !empty(trim(palette_draft))
  return if empty(trim(palette_draft))
  parallel
    run search_chat(connected_rpc, "", trim(palette_draft), palette_generation) -> palette_chat_loaded _ | palette_search_failed _
    run search_pages(connected_rpc, "", trim(palette_draft), palette_generation) -> palette_page_loaded _ | palette_search_failed _

on palette_chat_loaded(next)
  return if next.generation != palette_generation
  palette_chat_hits = next.hits
  palette_searching = false

on palette_page_loaded(next)
  return if next.generation != palette_generation
  palette_page_hits = next.hits
  palette_searching = false

on palette_search_failed(cause)
  return if cause.generation != palette_generation
  palette_searching = false

// Held here beside the loaders that fill them.
state
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
