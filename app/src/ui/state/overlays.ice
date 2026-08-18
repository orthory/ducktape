state
  palette_open = false
  bell_open = false
  bell_unread:i64 = 0
  bell_items:[BellItem] = []
  palette_draft = ""
  // The chat float's discriminant, honest here for the same reason a captured
  // query is unnecessary: `palette_changed` runs on EVERY keystroke and moves
  // this, so no phase can outlive the draft that earned it (see
  // `page_search_query` for the class the pages search needed a string for).
  // `done` is an answer; a failure returns to `idle`, and `idle` under a live
  // draft is the panel's FAILURE arm — see `screens/overlays.ice`.
  palette_search_phase:SearchPhase = SearchPhase.idle
  palette_chat_hits:[ChatSearchHit] = []
  palette_page_hits:[PageSearchHit] = []

  toast = ""
  toast_age:i64 = 0
