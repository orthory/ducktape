state
  // `fs_path` is requested immediately; `fs_listed_path` names the rows that
  // have actually landed, so navigation never presents stale rows as current.
  fs_path = "/shared"
  fs_listed_path = ""
  fs_entries:[FsEntry] = []
  fs_generation:i64 = 0
  fs_loading = false
  fs_preview_path = ""
  fs_preview_entry:FsEntry = no_fs_entry()
  fs_preview_text = ""
  fs_preview_truncated = false
  fs_preview_binary = false
  fs_preview_picture = false
  fs_preview_width:i64 = 0
  fs_preview_height:i64 = 0
  fs_history:[FsSnapshot] = []
  fs_new_name = ""
  fs_delete_target = ""
  fs_editor:editor = ""
  fs_editing = false
  fs_diff_from = ""
  fs_diff:[FsDiffEntry] = []
