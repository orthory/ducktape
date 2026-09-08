// DUCKFS — one directory at a time, its preview, its history and its writes.
// Every loader keys on `fs_generation`.

on fs_open_dir(path)
  return if fs_loading || !connected
  invalidate lane=files_preview
  invalidate lane=files_diff
  fs_path = path
  fs_generation = fs_generation + 1
  fs_loading = true
  fs_preview_path = ""
  fs_preview_entry = no_fs_entry()
  fs_preview_text = ""
  run replace lane=files_list files_ls(connected_rpc, fs_path, fs_generation) -> fs_listed _ | fs_failed _

on fs_open_file(path)
  return if fs_loading || !connected
  fs_preview_path = path
  fs_preview_entry = fs_entry_named(fs_entries, fs_preview_path)
  // The old body must not sit under the new path while the read is in
  // flight — the pane would show A's text (and A's Edit button) under B.
  fs_preview_text = ""
  fs_preview_truncated = false
  fs_preview_binary = false
  fs_preview_picture = false
  fs_generation = fs_generation + 1
  run replace lane=files_preview files_preview(connected_rpc, fs_preview_path, fs_generation) -> fs_previewed _ | fs_failed _

on fs_listed(next)
  return if next.generation != fs_generation
  fs_loading = false
  fs_path = next.path
  fs_listed_path = next.path
  fs_entries = next.entries
  fs_preview_entry = fs_entry_named(fs_entries, fs_preview_path)
  // A deep link's second step: the directory is listed, now its file.
  return if empty(fs_focus_path)
  let focus = fs_focus_path
  fs_focus_path = ""
  run every duck_echo_str(focus) -> fs_open_file _ | external_url_failed _

on fs_previewed(next)
  return if next.generation != fs_generation
  fs_preview_text = next.text
  fs_preview_truncated = next.truncated
  fs_preview_binary = next.binary
  fs_preview_picture = next.picture
  fs_preview_width = next.width
  fs_preview_height = next.height

on fs_history_loaded(next)
  return if next.generation != fs_generation
  fs_history = next.snapshots

on fs_failed(cause)
  return if cause.generation != fs_generation
  fs_loading = false
  error = cause.message

// THE VIEW'S INTENTS. The screen is the `files` module view: every act the
// reader takes in it comes back here as one intent, and the write it names
// goes through the same handler that signed it before the port. Navigation
// the rest of the console also drives (a deep link into a directory, the
// listing's second step into a file) is reached by a `flow` so its body
// stays in one place.
//
// EVERY WRITE ASKS THE MODULE'S RULE FIRST (`files_write_gate`): a root or
// another member's home refuses here, in the module's words, instead of
// after a signed round trip. The drafts are the view's: an intent carries
// the name or the body the reader typed, and a committed write says so
// through `fs_writes`.
on files_view_event(event)
  match files_intent(event)
    FilesIntent.open_dir
      flow
        from done event_text(event, "path")
        done -> fs_open_dir _
    FilesIntent.open_file
      flow
        from done event_text(event, "path")
        done -> fs_open_file _
    FilesIntent.open_parent
      return if fs_loading || !connected || fs_path == "/"
      invalidate lane=files_preview
      invalidate lane=files_diff
      fs_path = fs_parent(fs_path)
      fs_generation = fs_generation + 1
      fs_loading = true
      fs_preview_path = ""
      fs_preview_entry = no_fs_entry()
      fs_preview_text = ""
      run replace lane=files_list files_ls(connected_rpc, fs_path, fs_generation) -> fs_listed _ | fs_failed _
    FilesIntent.mkdir
      return if fs_loading || !connected || empty(trim(event_text(event, "name")))
      error = files_write_gate(fs_path, settings_user_key)
      return if !empty(error)
      fs_loading = true
      run every files_mkdir(connected_rpc, password, fs_child(fs_path, trim(event_text(event, "name")))) -> fs_wrote _ | fs_write_failed _
    FilesIntent.new_file
      return if fs_loading || !connected || empty(trim(event_text(event, "name")))
      error = files_write_gate(fs_path, settings_user_key)
      return if !empty(error)
      fs_loading = true
      run every files_write_text(connected_rpc, password, fs_child(fs_path, trim(event_text(event, "name"))), "") -> fs_wrote _ | fs_write_failed _
    FilesIntent.arm_delete
      fs_delete_target = event_text(event, "path")
    FilesIntent.disarm_delete
      fs_delete_target = ""
    FilesIntent.delete
      return if fs_loading || !connected || empty(fs_delete_target)
      error = files_write_gate(fs_parent(fs_delete_target), settings_user_key)
      return if !empty(error)
      fs_loading = true
      run every files_remove(connected_rpc, password, fs_delete_target) -> fs_wrote _ | fs_write_failed _
    // The edited body: shown under the path at once, written back, re-read.
    FilesIntent.save
      return if fs_loading || !connected || empty(fs_preview_path) || event_text(event, "path") != fs_preview_path
      error = files_write_gate(fs_parent(fs_preview_path), settings_user_key)
      return if !empty(error)
      fs_loading = true
      fs_preview_text = event_text(event, "text")
      run every files_write_text(connected_rpc, password, fs_preview_path, event_text(event, "text")) -> fs_wrote _ | fs_write_failed _
    FilesIntent.show_diff
      return if fs_loading || !connected
      fs_diff_from = event_text(event, "id")
      fs_generation = fs_generation + 1
      run replace lane=files_diff files_diff(connected_rpc, fs_diff_from, fs_generation) -> fs_diffed _ | fs_failed _
    FilesIntent.close_diff
      invalidate lane=files_diff
      fs_diff_from = ""
      fs_diff = []
    // a link the Markdown preview offered, through the shell's link seam
    FilesIntent.open_link
      flow
        from done event_text(event, "url")
        done -> open_message_link _

on fs_wrote(_result)
  fs_delete_target = ""
  fs_writes = fs_writes + 1
  fs_generation = fs_generation + 1
  fs_loading = true
  parallel
    run replace lane=files_list files_ls(connected_rpc, fs_path, fs_generation) -> fs_listed _ | fs_failed _
    run replace lane=files_history files_history(connected_rpc, fs_generation) -> fs_history_loaded _ | fs_failed _

on fs_write_failed(cause)
  fs_loading = false
  error = cause.message

on fs_file_dropped(path)
  return if shell_tab != ShellTab.files || fs_loading || !connected
  error = files_write_gate(fs_path, settings_user_key)
  return if !empty(error)
  fs_loading = true
  run every files_upload(connected_rpc, password, fs_path, path) -> fs_wrote _ | fs_write_failed _

on fs_diffed(next)
  return if next.generation != fs_generation
  fs_diff = next.entries
