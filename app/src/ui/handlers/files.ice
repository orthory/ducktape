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
  fs_preview_text = ""
  run replace lane=files_list files_ls(connected_rpc, fs_path, fs_generation) -> fs_listed _ | fs_failed _

on fs_open_parent
  return if fs_loading || !connected || empty(fs_path)
  invalidate lane=files_preview
  invalidate lane=files_diff
  fs_path = fs_parent(fs_path)
  fs_generation = fs_generation + 1
  fs_loading = true
  fs_preview_path = ""
  fs_preview_text = ""
  run replace lane=files_list files_ls(connected_rpc, fs_path, fs_generation) -> fs_listed _ | fs_failed _

on fs_open_file(path)
  return if fs_loading || !connected
  fs_preview_path = path
  fs_generation = fs_generation + 1
  run replace lane=files_preview files_preview(connected_rpc, fs_preview_path, fs_generation) -> fs_previewed _ | fs_failed _

on fs_toggle_history
  fs_history_open = !fs_history_open

on fs_listed(next)
  return if next.generation != fs_generation
  fs_loading = false
  fs_path = next.path
  fs_listed_path = next.path
  fs_entries = next.entries

on fs_previewed(next)
  return if next.generation != fs_generation
  fs_preview_path = next.path
  fs_preview_text = next.text
  fs_preview_truncated = next.truncated
  fs_preview_binary = next.binary

on fs_history_loaded(next)
  return if next.generation != fs_generation
  fs_history = next.snapshots

// The whole path list behind the tree sidebar — a prefix walk, not a listing.
on fs_tree_loaded(next)
  return if next.generation != fs_generation
  files_tree = next.entries

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
    run replace lane=files_tree files_find(connected_rpc, "", fs_generation) -> fs_tree_loaded _ | fs_failed _
    run replace lane=files_history files_history(connected_rpc, fs_generation) -> fs_history_loaded _ | fs_failed _

on fs_write_failed(cause)
  fs_loading = false
  error = cause.message

on fs_file_dropped(path)
  return if shell_tab != "files" || fs_loading || !connected
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
