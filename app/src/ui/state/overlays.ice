state
  palette_open = false
  bell_open = false
  bell_unread:i64 = 0
  bell_items:[BellItem] = []
  palette_draft = ""
  palette_key = ""
  palette_searching = false
  palette_chat_hits:[ChatSearchHit] = []
  palette_page_hits:[PageSearchHit] = []

  // Subscription payload fields cannot be typed inside a handler-local let.
  escape_key = ""
  content_scroll = 0.0
  toast = ""
  toast_age:i64 = 0
