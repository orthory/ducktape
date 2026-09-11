// THE PAGES VIEW'S TWO OS DOORS. The screen, the workspace reads, every
// write and all of its own state live in the `pages` view
// (crates/views/pages), which speaks the kernel contract: `rpc.view` for the
// workspace and `op.submit` for the pages module's ops. What cannot cross
// that contract is what the OS owns — the clipboard, and the ONE open plane
// (`open_message_link`) a `duck://` address in a document goes through: only
// that plane knows the module table and the network scope.
on pages_view_event(event)
  match pages_intent(event)
    PagesIntent.open_link
      flow
        from done event_text(event, "link")
        done -> open_message_link _
    PagesIntent.copy
      toast = event_text(event, "label")
      toast_age = 0
      task clipboard write event_text(event, "text")

// A page named by a `duck://page/…` address, a search hit or a bell row: the
// Pages tab takes the window and the view is told which page to open. The
// serial moves once per ask, so the same page twice really opens twice.
on open_page_search_hit(page_id, _block_id)
  return if empty(page_id)
  palette_open = false
  shell_tab = ShellTab.pages
  page_route = page_id
  page_route_serial = page_route_serial + 1

on external_url_opened(_opened)

on external_url_failed(cause)
  error = cause.message
