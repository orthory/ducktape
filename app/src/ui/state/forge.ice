state
  // THE ONLY FORGE STATE LEFT IN THE APP. The screen belongs to the `forge`
  // view (crates/views/forge), which reads and writes it all itself; what
  // the app still holds is the host composer's in-flight note — the
  // operation id of the discussion note crossing the wire — and the
  // `duck://forge/...` its open plane last routed to the view.
  forge_note_pending = ""
  forge_link = ""
  // moves once per routed link, so the same address twice still lands
  forge_link_tick:i64 = 0
