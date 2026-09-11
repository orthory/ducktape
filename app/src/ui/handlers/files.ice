// DUCKFS — the browser itself is the `files` VIEW's: it lists, previews,
// diffs and writes the module through the kernel. Two doors are the app's and
// stay here: a link the Markdown reader activated, and a file DROPPED on the
// window, which lands in the directory the view says it is standing in.

on files_view_event(event)
  fs_drop_dir = keep_str(event.kind == "at", event_text(event, "path"), fs_drop_dir)
  return if event.kind != "open_link"
  flow
    from done event_text(event, "url")
    done -> open_message_link _

// THE WINDOW'S OWN DROP. The dropped path never leaves this device — only
// bytes do — and the module's rule for the target directory is asked first,
// in the module's words, instead of after a signed round trip.
on fs_file_dropped(path)
  return if shell_tab != ShellTab.files || fs_dropping || !connected
  error = files_write_gate(fs_drop_dir, settings_user_key)
  return if !empty(error)
  fs_dropping = true
  run every files_upload(connected_rpc, password, fs_drop_dir, path) -> fs_dropped _ | fs_drop_failed _

on fs_dropped(_result)
  fs_dropping = false

on fs_drop_failed(cause)
  fs_dropping = false
  error = cause.message
