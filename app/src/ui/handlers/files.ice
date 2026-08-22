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

on fs_open_parent
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

on fs_new_name_changed(next)
  fs_new_name = next

on fs_mkdir_submit
  return if fs_loading || !connected || empty(trim(fs_new_name))
  fs_loading = true
  error = ""
  run every files_mkdir(connected_rpc, fs_child(fs_path, trim(fs_new_name))) -> fs_wrote _ | fs_write_failed _

on fs_new_file_submit
  return if fs_loading || !connected || empty(trim(fs_new_name))
  fs_loading = true
  error = ""
  run every files_write_text(connected_rpc, fs_child(fs_path, trim(fs_new_name)), "") -> fs_wrote _ | fs_write_failed _

on fs_arm_delete(path)
  fs_delete_target = path

on fs_disarm_delete
  fs_delete_target = ""

on fs_delete_submit
  return if fs_loading || !connected || empty(fs_delete_target)
  fs_loading = true
  error = ""
  run every files_remove(connected_rpc, fs_delete_target) -> fs_wrote _ | fs_write_failed _

on fs_begin_edit
  return if fs_preview_binary || empty(fs_preview_path)
  fs_editing = true
  fs_editor = editor(fs_preview_text)

on fs_cancel_edit
  fs_editing = false

on fs_save_edit
  return if fs_loading || !connected || !fs_editing || empty(fs_preview_path)
  fs_loading = true
  fs_editing = false
  fs_preview_text = editor_text(fs_editor)
  error = ""
  run every files_write_text(connected_rpc, fs_preview_path, editor_text(fs_editor)) -> fs_wrote _ | fs_write_failed _

on fs_wrote(_result)
  fs_new_name = ""
  fs_delete_target = ""
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
  fs_loading = true
  error = ""
  run every files_upload(connected_rpc, fs_path, path) -> fs_wrote _ | fs_write_failed _

on fs_show_diff(from)
  return if fs_loading || !connected
  fs_diff_from = from
  fs_generation = fs_generation + 1
  run replace lane=files_diff files_diff(connected_rpc, fs_diff_from, fs_generation) -> fs_diffed _ | fs_failed _

on fs_close_diff
  invalidate lane=files_diff
  fs_diff_from = ""
  fs_diff = []

on fs_diffed(next)
  return if next.generation != fs_generation
  fs_diff = next.entries
