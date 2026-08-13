state
  // Block/event inspector.
  explorer_blocks:[ExplorerBlock] = []
  explorer_ops:[ExplorerOp] = []
  explorer_generation:i64 = 0
  explorer_loading = false
  explorer_selected:i64 = 0

  // Cross-module workspace search.
  explorer_query = ""
  explorer_kind = "all"
  explorer_hits:[ExplorerHit] = []
  explorer_kinds:[KindCount] = []
  explorer_partial = ""
  explorer_searching = false
