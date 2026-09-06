// Cheap scalar projections belong to the compiler-owned derived graph, not
// mirrored booleans that every writer must remember to update.
derived
  dark = appearance == Appearance.dark
  app_background = keep_str(appearance == Appearance.dark, "#1b1a16", "#fdfdfb")
  app_text = keep_str(appearance == Appearance.dark, "#e8e6df", "#2c2b27")
  has_error = !empty(error)
  mutation_busy = mutation_phase != MutationPhase.idle
