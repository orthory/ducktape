state
  // THE ONE PAGES FACT THE APP STILL HOLDS: the page a `duck://page/…`
  // address asked to open. Everything else — the list, the document, its
  // comments and every write — belongs to the `pages` view, which reads and
  // signs through the kernel. The serial moves once per ask, so following the
  // same link twice opens the page twice.
  page_route = ""
  page_route_serial:i64 = 0
