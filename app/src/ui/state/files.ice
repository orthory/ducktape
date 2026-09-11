state
  // Where a file DROPPED on the window lands: the directory the files view
  // last said it was standing in. The browser's own path, listing, preview,
  // history and drafts are the view's — the app holds none of them.
  fs_drop_dir = "/shared"
  fs_dropping = false
  // WHERE A duck:// LINK SENT THE READER. The shell's link plane resolves the
  // address and moves the tab, so the path it named is a session fact the view
  // is told — the one navigation the browser cannot decide for itself. The
  // serial counts the pushes: the same path twice must navigate twice.
  fs_route = ""
  fs_route_serial:i64 = 0
