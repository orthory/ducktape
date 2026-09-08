// FILES, as a module-owned view: the duckfs browser — the crumb bar, the
// write bar, the 206px tree, the object table, the preview and the 306px
// object panel — drawn from the facts the desktop app pushes. The screen body
// is the app's own (screens/storage.ice before the port). The two drafts are
// the view's: the new entry's name and the editor over the previewed file. The
// app hears a name only when the reader submits it, and the edited body only
// on Save; a committed write says so through `writes`, and the view clears
// the name it consumed. The picture viewer, the highlighted reader and the
// Markdown document are the app's surfaces, painted into the slots left here.
app FilesView
  title "Files"
  palette active_palette
  id "dev.ducktape.view.files"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"
use "../../../../../app/src/ui/ducktape-ui/recipes.ice"
use "../../../../../app/src/ui/components/icon.ice"
use "browser.ice"
use "files.ice"
use "kit.ice"

extern crate::host
  FsEntry(key:i64, path:str, name:str, kind:str, size:i64, object:str)
  FsSnapshot(id:str, short_id:str, author:str, height:i64, message:str)
  FsDiffEntry(path:str, kind:str)
  FilesProps(display_omitted:i64, display_shortened:bool, display_unavailable:bool, path:str, listed:bool, entries:[FsEntry], directories:[FsEntry], connected:bool, loading:bool, preview_path:str, preview_entry:FsEntry, delete_target:str, diff_from:str, diff:[FsDiffEntry], history:[FsSnapshot], preview_truncated:bool, preview_binary:bool, preview_picture:bool, preview_width:i64, preview_height:i64, preview_text:str, preview_display_text:str, preview_display_clipped:bool, dark:bool, write_refusal:str, writes:i64)
  PropsItem(next:FilesProps, error:str)
  subscription props() -> PropsItem
  pure open_dir(path:&str) -> bool
  pure open_file(path:&str) -> bool
  pure open_parent() -> bool
  pure make_dir(name:&str) -> bool
  pure make_file(name:&str) -> bool
  pure arm_delete(path:&str) -> bool
  pure disarm_delete() -> bool
  pure delete_object() -> bool
  pure close_diff() -> bool
  pure show_diff(id:&str) -> bool
  pure save(path:&str, text:&str) -> bool
  pure open_link(url:&str) -> bool
  pure icon(name:&str) -> bytes
  pure no_fs_entry() -> FsEntry
  pure keep_draft(consumed:bool, draft:&str) -> str
  pure fs_counts_summary(connected:bool, listed:bool, entries:&[FsEntry]) -> str
  pure size_label(bytes:i64) -> str
  pure height_label(height:i64) -> str
  pure picture_caption(width:i64, height:i64) -> str
  pure markdown_path(path:&str) -> bool
  // the app's own surfaces, painted into the slots the preview leaves
  component picture(surface:str, path:str) -> unit
  component forge_code(source:str, path:str, dark:bool) -> unit
  component agent_markdown(source:str, dark:bool) -> str

state
  display_omitted:i64 = 0
  display_shortened = false
  display_unavailable = false
  active_palette:palette[AppTheme] = AppTheme.app
  path = "/shared"
  listed = false
  entries:[FsEntry] = []
  directories:[FsEntry] = []
  connected = false
  loading = false
  preview_path = ""
  preview_entry:FsEntry = no_fs_entry()
  delete_target = ""
  diff_from = ""
  diff:[FsDiffEntry] = []
  history:[FsSnapshot] = []
  preview_truncated = false
  preview_binary = false
  preview_picture = false
  preview_width:i64 = 0
  preview_height:i64 = 0
  preview_text = ""
  preview_display_text = ""
  preview_display_clipped = false
  dark = false
  write_refusal = ""
  // the last write the app reported: a count that moves once per commit
  writes:i64 = 0
  // the reader's own: the new entry's name, and the editor over the preview
  new_name = ""
  draft:editor = ""
  editing = false
  host_error = ""
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false

// The facts are the host's: one subscription, one item per change. A
// subscription, not a mount task, so a replacement restored from this
// view's state asks for the facts again on its own.
subscribe
  props() -> props_arrived _

on props_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  let next = item.next
  display_omitted = next.display_omitted
  display_shortened = next.display_shortened
  display_unavailable = next.display_unavailable
  path = next.path
  listed = next.listed
  entries = next.entries
  directories = next.directories
  connected = next.connected
  loading = next.loading
  preview_path = next.preview_path
  preview_entry = next.preview_entry
  delete_target = next.delete_target
  diff_from = next.diff_from
  diff = next.diff
  history = next.history
  preview_truncated = next.preview_truncated
  preview_binary = next.preview_binary
  preview_picture = next.preview_picture
  preview_width = next.preview_width
  preview_height = next.preview_height
  preview_text = next.preview_text
  preview_display_text = next.preview_display_text
  preview_display_clipped = next.preview_display_clipped
  dark = next.dark
  write_refusal = next.write_refusal
  // A COMMITTED WRITE CONSUMES THE NAME IT READ: the count says one landed
  // since the last props, and the field it came from clears.
  let consumed = next.writes != writes
  writes = next.writes
  new_name = keep_draft(consumed, new_name)
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

on open_dir_at(target)
  sent = open_dir(target)

on open_file_at(target)
  sent = open_file(target)

on go_parent
  sent = open_parent()

on mkdir_submit
  return if loading || empty(trim(new_name)) || !empty(write_refusal)
  sent = make_dir(trim(new_name))

on new_file_submit
  return if loading || empty(trim(new_name)) || !empty(write_refusal)
  sent = make_file(trim(new_name))

on arm_delete_at(target)
  sent = arm_delete(target)

on disarm_delete_now
  sent = disarm_delete()

on delete_submit
  sent = delete_object()

on close_diff_now
  sent = close_diff()

on show_diff_of(id)
  sent = show_diff(id)

on begin_edit
  return if preview_binary || empty(preview_path)
  editing = true
  draft = editor(preview_text)

on cancel_edit
  editing = false

// The body leaves on Save and the pane drops back to the reader at once; the
// app shows the saved text under the path until the re-read lands.
on save_edit
  return if loading || !editing || empty(preview_path)
  editing = false
  sent = save(preview_path, editor_text(draft))

on open_link_at(url)
  sent = open_link(url)

view
  col w=fill h=fill
    if display_unavailable
      text "Too much display data. Open a smaller directory or item." size=13.0
    if !display_unavailable
      col w=fill h=fill
        if display_omitted > 0
          text "{display_omitted} rows are not shown." size=12.5
        if display_shortened
          text "Some content is shortened for display." size=12.5
        box #root
          with
            w=fill
            h=fill
            bg=bg
          FilesScreen new_name<->new_name draft<->draft
            with
              display_omitted
              path
              listed
              entries
              directories
              connected
              loading
              preview_path
              preview_entry
              delete_target
              diff_from
              diff
              history
              preview_truncated
              preview_binary
              editing
              preview_text=preview_display_text
              preview_display_clipped
              dark
              preview_picture
              preview_width
              preview_height
              write_refusal
            events
              open_message_link -> open_link_at _
              fs_open_dir -> open_dir_at _
              fs_open_file -> open_file_at _
              fs_open_parent -> go_parent
              fs_mkdir_submit -> mkdir_submit
              fs_new_file_submit -> new_file_submit
              fs_arm_delete -> arm_delete_at _
              fs_disarm_delete -> disarm_delete_now
              fs_delete_submit -> delete_submit
              fs_close_diff -> close_diff_now
              fs_show_diff -> show_diff_of _
              fs_begin_edit -> begin_edit
              fs_cancel_edit -> cancel_edit
              fs_save_edit -> save_edit
