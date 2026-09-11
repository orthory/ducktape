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
  // moves once per block a module view was told about (`view_live_hit`):
  // the state change is what draws the view that took the item
  views_live_serial:i64 = 0
  // Is ⌘ down right now? Set from the modifier stream and read by exactly one
  // subscription gate: it arms the command-chord key route, so ordinary typing
  // never pays for a key-press subscription (`lifecycle.ice`). Nothing renders
  // it.
  cmd_held = false
  // Is ⇧ down right now? Same stream, same reason: a shift-click extends the
  // chat's copy range, and a press carries no modifiers of its own — so the
  // press handler reads this instead of the app growing a second key route.
  shift_held = false
  // The window the OS last gave focus to. ⌘W closes THIS one — a chord that
  // guessed instead (the console, say) would close a window nobody was looking
  // at. Nothing renders it either.
  focused_win:window-id? = none
  block_height:i64 = -1
  hydration_generation:i64 = 0
  connect_generation:i64 = 0
  // The public key SEATED for signing right now, hex, and "" while the seat is
  // locked. Nothing draws it: it is an identity a subscription keys on.
  //
  // A connection is named by endpoint + chain + connect attempt, and NONE of
  // those moves when a seat does — Settings unlocks and locks in place
  // (`handlers/node.ice` SettingsIntent.unlock/.lock), and `unlock_user_key`
  // opens whichever wallet is ACTIVE. So a lane whose entitlement depends on
  // the seated key must key on this too, or a locked→unlocked device never
  // recovers and a key SWITCH leaves the previous key's reading on screen.
  signer_key:str = ""
  hydration_retry_attempt:i64 = 0
  mutation_phase:MutationPhase = MutationPhase.idle
  error = ""
  // A `duck://` URL the OS handed this process on the command line (the
  // `x-scheme-handler/duck` desktop entry's `%u`). Parked until the first
  // status names the connected chain — the earliest moment the open plane can
  // tell this network's addresses from another's — then spent exactly once.
  startup_duck_link:str = startup_duck_url()
