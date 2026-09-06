// Cheap scalar projections belong to the compiler-owned derived graph, not
// mirrored booleans that every writer must remember to update.
derived
  dark = appearance == Appearance.dark
  app_background = keep_str(appearance == Appearance.dark, "#1b1a16", "#fdfdfb")
  app_text = keep_str(appearance == Appearance.dark, "#e8e6df", "#2c2b27")
  has_error = !empty(error)
  huddle_popped = huddle_win != none
  // THE TWO SHAPES THE IN-WINDOW HUDDLE TAKES, derived once because three
  // surfaces read them: the two arms of the huddle layer, and the chat
  // timeline's inset, which has to lift its last rows by exactly what the
  // layer above them covers.
  mutation_busy = mutation_phase != MutationPhase.idle
