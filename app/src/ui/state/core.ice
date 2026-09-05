state
  app_palette:palette[AppTheme] = AppTheme.app
  appearance:Appearance = Appearance.system
  // Native banners for a mention or a DM. Default ON, persisted in
  // app-prefs.json beside `appearance` — a device preference, not a
  // workspace one.
  desktop_notifications = true
  wall_now:i64 = current_wall_seconds()
  rpc = ""
  connected_rpc = ""
  password = ""
  status = "Connecting…"
  connected = false
  loading = false
  // Is ⌘ down right now? Set from the modifier stream and read by exactly one
  // subscription gate: it arms the command-chord key route, so ordinary typing
  // never pays for a key-press subscription (`lifecycle.ice`). Nothing renders
  // it.
  cmd_held = false
  // The window the OS last gave focus to. ⌘W closes THIS one — a chord that
  // guessed instead (the console, say) would close a window nobody was looking
  // at. Nothing renders it either.
  focused_win:window-id? = none
  block_height:i64 = -1
  hydration_generation:i64 = 0
  connect_generation:i64 = 0
  hydration_retry_attempt:i64 = 0
  mutation_phase:MutationPhase = MutationPhase.idle
  error = ""
  // A `duck://` URL the OS handed this process on the command line (the
  // `x-scheme-handler/duck` desktop entry's `%u`). Parked until the first
  // status names the connected chain — the earliest moment the open plane can
  // tell this network's addresses from another's — then spent exactly once.
  startup_duck_link:str = startup_duck_url()
