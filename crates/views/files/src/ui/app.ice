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
  SaveReply(namespace:str, context:str, request:i64, success:bool, message:str)
  SaveHistory(replies:[SaveReply], overflow:str)
  pure save_answer(history:&SaveHistory, context:&str, namespace:&str, request:i64) -> SaveReply
  FilesProps(save_namespace:str, network_scope:str, context:str, preview_base:str, save_reply:SaveHistory, path:str, listed:bool, entries:[FsEntry], directories:[FsEntry], connected:bool, loading:bool, preview_path:str, preview_entry:FsEntry, delete_target:str, diff_from:str, diff:[FsDiffEntry], history:[FsSnapshot], preview_truncated:bool, preview_binary:bool, preview_picture:bool, preview_width:i64, preview_height:i64, preview_text:str, dark:bool, write_refusal:str, writes:i64, display_omitted:i64, display_shortened:bool, display_unavailable:bool, preview_display_text:str, preview_display_clipped:bool)
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
  pure save(namespace:&str, context:&str, path:&str, base:&str, request:i64, text:&str) -> bool
  pure edit_token(context:&str, path:&str, base:&str, draft:i64) -> str
  pure open_link(url:&str) -> bool
  pure icon(name:&str) -> bytes
  pure no_fs_entry() -> FsEntry
  pure keep_draft(consumed:bool, draft:&str) -> str
  pure keep_str(take:bool, next:&str, previous:&str) -> str
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
  network_scope = ""
  context = ""
  preview_base = ""
  draft_network = ""
  draft_path = ""
  draft_base = ""
  draft_id:i64 = 0
  save_request:i64 = 0
  save_context = ""
  save_namespace = ""
  pending_namespace = ""
  reply_overflow = ""
  pending_overflow = ""
  save_pending = false
  draft_error = ""
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

derived
  draft_here = editing && draft_path == preview_path && draft_network == network_scope
  draft_parked = editing && !draft_here
  edit_context = edit_token(context, preview_path, preview_base, draft_id)

// The facts are the host's: one subscription, one item per change. A
// subscription, not a mount task, so a replacement restored from this
// view's state asks for the facts again on its own.
subscribe
  props() -> props_arrived _

on props_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  let next = item.next
  let reply = save_answer(next.save_reply, save_context, pending_namespace, save_request)
  let answered = save_pending && reply.request == save_request
  let lost = save_pending && !answered && next.save_reply.overflow != pending_overflow
  editing = editing && !(answered && reply.success)
  draft_error = keep_str(lost, "Save confirmation is no longer available. Your edits are still here; check the file before saving again.", keep_str(answered, reply.message, draft_error))
  save_pending = save_pending && !answered && !lost && next.connected && next.context == context
  reply_overflow = next.save_reply.overflow
  save_namespace = next.save_namespace
  network_scope = next.network_scope
  context = next.context
  preview_base = next.preview_base
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

on begin_edit(token)
  return if token != edit_context || editing || loading || !connected || empty(network_scope) || empty(preview_base) || preview_binary || preview_truncated || empty(preview_path)
  editing = true
  draft_id = draft_id + 1
  draft_network = network_scope
  draft_path = preview_path
  draft_base = preview_base
  draft_error = ""
  draft = editor(preview_text)

on cancel_edit(token)
  return if token != edit_context
  editing = false
  save_pending = false
  draft_error = ""
  draft = editor("")

on discard_draft(id)
  return if id != draft_id
  editing = false
  save_pending = false
  draft_error = ""
  draft = editor("")

// Keep the original bytes and base until this exact request is committed.
on save_edit(token)
  return if token != edit_context || !draft_here || loading || save_pending || !connected || empty(preview_base)
  save_request = save_request + 1
  save_context = context
  pending_namespace = save_namespace
  pending_overflow = reply_overflow
  save_pending = true
  draft_error = ""
  sent = save(save_namespace, context, draft_path, draft_base, save_request, editor_text(draft))

on open_link_at(url)
  sent = open_link(url)

view
  col w=fill h=fill
    if draft_parked
      col w=fill gap=4.0
        text "Unsaved changes to:" size=13.0
        text draft_path size=13.0
        text "Return to this file to continue editing." size=13.0
        button "Discard unsaved changes" -> discard_draft(draft_id)
    if !empty(draft_error)
      text draft_error size=13.0
    if display_unavailable
      text "Too much display data. Open a smaller directory or item." size=13.0
    if !display_unavailable
      col w=fill h=fill
        if display_omitted > 0
          row gap=4.0
            text display_omitted #display-omitted size=12.5
            text "rows are not shown." size=12.5
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
              loading=(loading || save_pending || (draft_here && empty(preview_base)))
              preview_path
              preview_entry
              delete_target
              diff_from
              diff
              history
              preview_truncated
              preview_binary
              editing=draft_here
              edit_context
              edit_blocked=(draft_parked || empty(preview_base) || empty(network_scope))
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
              fs_begin_edit -> begin_edit _
              fs_cancel_edit -> cancel_edit _
              fs_save_edit -> save_edit _
