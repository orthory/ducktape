state
  // Block/event inspector.
  explorer_blocks:[ExplorerBlock] = []
  explorer_ops:[ExplorerOp] = []
  explorer_generation:i64 = 0
  explorer_loading = false
  // The answer to the last workspace search the Explorer view asked for,
  // and the query it stands for ("" while none does). The draft, the kind
  // filter and the selected block are the view's own.
  explorer_hits:[ExplorerHit] = []
  explorer_kinds:[KindCount] = []
  explorer_partial = ""
  explorer_searching = false
  explorer_sent_query = ""
