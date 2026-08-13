// Cheap scalar projections belong to the compiler-owned derived graph, not
// mirrored booleans that every writer must remember to update.
derived
  dark = appearance == "dark"
  has_error = !empty(error)
  huddle_popped = huddle_win != none
  mutation_busy = mutation_phase != MutationPhase.idle
