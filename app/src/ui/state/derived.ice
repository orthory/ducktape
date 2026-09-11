// Cheap scalar projections belong to the compiler-owned derived graph, not
// mirrored booleans that every writer must remember to update.
derived
  dark = appearance == Appearance.dark
  app_background = keep_str(appearance == Appearance.dark, "#1b1a16", "#fdfdfb")
  app_text = keep_str(appearance == Appearance.dark, "#e8e6df", "#2c2b27")
  has_error = !empty(error)
  mutation_busy = mutation_phase != MutationPhase.idle
  // The launch window's doors are held while a mutation or a door's connect
  // is in flight; a refused connect (its message in `onboarding_error`) hands
  // them back while its retry goes on.
  hub_busy = mutation_busy || (console_entry == ConsoleEntry.entering && empty(onboarding_error))
