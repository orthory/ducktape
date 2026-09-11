// FILES, as a module-owned view on the KERNEL CONTRACT: the duckfs browser —
// the crumb bar, the write bar, the 206px tree, the object table, the preview
// and the 306px object panel. The kernel pushes session facts only
// (`session()`: connected, dark, and the chain a draft belongs to); the
// listing, the snapshot history, the preview and a snapshot diff are read HERE
// through `files.get`, re-read on every files block (`rpc.live`), and a mkdir /
// new file / delete / save leaves as `op.submit` carrying the duckfs commit the
// kernel signs. The two drafts are the view's: the new entry's name and the
// editor over the previewed file. The picture viewer, the highlighted reader
// and the Markdown document are the app's surfaces, painted into the slots left
// here; a link the Markdown reader offers and a file dropped on the window are
// the app's doors, so those two alone stay intents.
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

// which palette the app's `dark` names — a handler branches on an enum only,
// and the session handler has work to do after the branch
enum Tone
  light
  dark

extern crate::host
  FsEntry(key:i64, path:str, name:str, kind:str, size:i64, object:str)
  FsSnapshot(id:str, short_id:str, author:str, height:i64, message:str)
  FsDiffEntry(path:str, kind:str)
  Session(connected:bool, dark:bool, chain:str, route:str, route_serial:i64)
  SessionItem(next:Session, error:str)
  ListingItem(entries:[FsEntry], directories:[FsEntry], history:[FsSnapshot], omitted:i64, error:str)
  PreviewItem(path:str, base:str, text:str, display_text:str, clipped:bool, truncated:bool, binary:bool, picture:bool, width:i64, height:i64, error:str)
  DiffItem(entries:[FsDiffEntry], omitted:i64, error:str)
  ActItem(kind:str, error:str)
  subscription session() -> SessionItem
  // this directory and the snapshot rail beside it: read once per generation,
  // then again on every files block
  subscription listing(generation:i64, path:str) -> ListingItem
  subscription preview(generation:i64, path:str) -> PreviewItem
  subscription diff(generation:i64, from:str) -> DiffItem
  // every write's outcome, as the kernel answers it
  subscription acts() -> ActItem
  pure generation_after(was_connected:bool, connected:bool, generation:i64) -> i64
  pure tone_of(dark:bool) -> Tone
  sync make_dir(dir:&str, name:&str) -> bool
  sync make_file(dir:&str, name:&str) -> bool
  sync delete_object(path:&str) -> bool
  sync save(path:&str, base:&str, text:&str) -> bool
  // the app's own doors: the shell's link plane, and where a dropped file lands
  sync open_link(url:&str) -> bool
  sync at(path:&str) -> bool
  pure write_refusal(dir:&str) -> str
  pure fs_parent(path:&str) -> str
  pure entry_named(entries:&[FsEntry], path:&str) -> FsEntry
  pure edit_token(chain:&str, path:&str, base:&str, draft:i64) -> str
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
  active_palette:palette[AppTheme] = AppTheme.app
  connected = false
  dark = false
  // the network a draft belongs to, across reconnections to it
  chain = ""
  // moves when the session comes up and after every write: the reads restart
  generation:i64 = 0
  // the last `duck://files/...` push this view has landed on
  route_serial:i64 = 0
  path = "/shared"
  // `listed` says the rows on hand describe `path`
  listed = false
  entries:[FsEntry] = []
  directories:[FsEntry] = []
  history:[FsSnapshot] = []
  omitted:i64 = 0
  diff_omitted:i64 = 0
  preview_path = ""
  preview_entry:FsEntry = no_fs_entry()
  preview_base = ""
  // the complete read: the editor's seed and the save's source
  preview_text = ""
  // a bounded display-only projection; never the editor seed
  preview_display_text = ""
  preview_clipped = false
  preview_truncated = false
  preview_binary = false
  preview_picture = false
  preview_width:i64 = 0
  preview_height:i64 = 0
  delete_target = ""
  diff_from = ""
  diff:[FsDiffEntry] = []
  // a write is out, or a save is
  acting = false
  saving = false
  // the last refusal or failure the view has to say
  notice = ""
  // the reader's own: the new entry's name, and the editor over the preview
  new_name = ""
  draft:editor = ""
  editing = false
  draft_chain = ""
  draft_path = ""
  draft_base = ""
  draft_id:i64 = 0
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false

derived
  refusal = write_refusal(path)
  loading = acting || saving || (connected && !listed)
  draft_here = editing && draft_path == preview_path && draft_chain == chain
  draft_parked = editing && !draft_here
  edit_context = edit_token(chain, preview_path, preview_base, draft_id)

// Subscriptions, not mount tasks, so a replacement restored from this view's
// state asks for the session and its reads again on its own.
subscribe
  session() -> session_arrived _
  listing(generation, path) when connected -> listing_arrived _
  preview(generation, preview_path) when connected && !empty(preview_path) -> preview_arrived _
  diff(generation, diff_from) when connected && !empty(diff_from) -> diff_arrived _
  acts() -> act_done _

on session_arrived(item)
  notice = keep_str(!empty(item.error), item.error, notice)
  return if !empty(item.error)
  let next = item.next
  generation = generation_after(connected, next.connected, generation)
  // rows read over a connection that dropped are no longer this path's
  listed = listed && next.connected
  connected = next.connected
  chain = next.chain
  dark = next.dark
  // A duck:// LINK LANDS ON THE FILE. The shell resolved the address and
  // moved the tab; the path itself is a session fact, and the SERIAL — not
  // the path — says a push happened, so the same file twice opens twice.
  // It rides the palette match because a handler branches nowhere else.
  let routed = next.route_serial != route_serial && !empty(next.route)
  route_serial = next.route_serial
  let landing = keep_str(routed, next.route, "")
  match tone_of(next.dark)
    Tone.light
      active_palette = AppTheme.app
      flow
        from done landing
        done -> route_to _
    Tone.dark
      active_palette = AppTheme.app_dark
      flow
        from done landing
        done -> route_to _

// WHERE THE LINK SENT THE READER. The address names a FILE: its directory is
// what the browser lists, the file itself is what the preview reads, and
// everything the old path had on screen goes with it.
on route_to(target)
  return if empty(target)
  notice = ""
  // the same address twice is the same two keys, so the generation is what
  // makes the second push read again instead of sitting on cleared state
  generation = generation + 1
  path = fs_parent(target)
  listed = false
  entries = []
  directories = []
  omitted = 0
  diff_from = ""
  diff = []
  diff_omitted = 0
  preview_path = target
  preview_entry = no_fs_entry()
  preview_base = ""
  preview_text = ""
  preview_display_text = ""
  preview_clipped = false
  preview_truncated = false
  preview_binary = false
  preview_picture = false
  preview_width = 0
  preview_height = 0
  sent = at(path)

on listing_arrived(item)
  notice = keep_str(!empty(item.error), item.error, notice)
  return if !empty(item.error)
  listed = true
  entries = item.entries
  directories = item.directories
  history = item.history
  omitted = item.omitted
  preview_entry = entry_named(item.entries, preview_path)

on preview_arrived(item)
  return if item.path != preview_path
  notice = keep_str(!empty(item.error), item.error, notice)
  return if !empty(item.error)
  preview_base = item.base
  preview_text = item.text
  preview_display_text = item.display_text
  preview_clipped = item.clipped
  preview_truncated = item.truncated
  preview_binary = item.binary
  preview_picture = item.picture
  preview_width = item.width
  preview_height = item.height

on diff_arrived(item)
  notice = keep_str(!empty(item.error), item.error, notice)
  return if !empty(item.error)
  diff = item.entries
  diff_omitted = item.omitted

// EVERY WRITE'S OUTCOME IS THIS VIEW'S OWN. A committed write consumes the
// name it read, frees the bar, and moves the generation so the directory,
// the history and the open preview are read again.
on act_done(item)
  notice = item.error
  let ok = empty(item.error)
  let saved = item.kind == "save"
  let named = item.kind == "mkdir" || item.kind == "new_file"
  acting = acting && saved
  saving = saving && !saved
  editing = editing && !(saved && ok)
  delete_target = keep_str(item.kind == "delete" && ok, "", delete_target)
  new_name = keep_draft(named && ok, new_name)
  generation = generation + 1

on open_dir_at(target)
  return if loading || !connected || target == path
  notice = ""
  path = target
  listed = false
  entries = []
  directories = []
  omitted = 0
  diff_from = ""
  diff = []
  diff_omitted = 0
  preview_path = ""
  preview_entry = no_fs_entry()
  preview_base = ""
  preview_text = ""
  preview_display_text = ""
  preview_picture = false
  preview_binary = false
  preview_truncated = false
  sent = at(path)

on open_file_at(target)
  return if loading || !connected || target == preview_path
  notice = ""
  preview_path = target
  preview_entry = entry_named(entries, target)
  // The old body must not sit under the new path while the read is in flight
  // — the pane would show A's text (and A's Edit button) under B.
  preview_base = ""
  preview_text = ""
  preview_display_text = ""
  preview_clipped = false
  preview_truncated = false
  preview_binary = false
  preview_picture = false
  preview_width = 0
  preview_height = 0

on mkdir_submit
  return if loading || !connected || empty(trim(new_name)) || !empty(refusal)
  notice = ""
  acting = true
  sent = make_dir(path, trim(new_name))

on new_file_submit
  return if loading || !connected || empty(trim(new_name)) || !empty(refusal)
  notice = ""
  acting = true
  sent = make_file(path, trim(new_name))

on arm_delete_at(target)
  delete_target = target

on disarm_delete_now
  delete_target = ""

on delete_submit
  return if loading || !connected || empty(delete_target)
  notice = write_refusal(fs_parent(delete_target))
  return if !empty(notice)
  acting = true
  sent = delete_object(delete_target)

on close_diff_now
  diff_from = ""
  diff = []
  diff_omitted = 0

on show_diff_of(id)
  return if loading || !connected
  notice = ""
  diff = []
  diff_omitted = 0
  diff_from = id

on begin_edit(token)
  return if token != edit_context || editing || loading || !connected || empty(chain) || empty(preview_base) || preview_binary || preview_picture || preview_truncated || empty(preview_path)
  editing = true
  draft_id = draft_id + 1
  draft_chain = chain
  draft_path = preview_path
  draft_base = preview_base
  notice = ""
  draft = editor(preview_text)

on cancel_edit(token)
  return if token != edit_context
  editing = false
  saving = false
  notice = ""
  draft = editor("")

on discard_draft(id)
  return if id != draft_id
  editing = false
  saving = false
  notice = ""
  draft = editor("")

// Keep the original bytes and base until this exact draft is committed.
on save_edit(token)
  return if token != edit_context || !draft_here || loading || !connected || empty(draft_base)
  notice = ""
  saving = true
  sent = save(draft_path, draft_base, editor_text(draft))

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
    if !empty(notice)
      text notice size=13.0
    if omitted > 0
      row gap=4.0
        text omitted #display-omitted size=12.5
        text "rows are not shown." size=12.5
    box #root
      with
        w=fill
        h=fill
        bg=bg
      FilesScreen new_name<->new_name draft<->draft
        with
          omitted
          diff_omitted
          path
          parent=fs_parent(path)
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
          editing=draft_here
          edit_context
          edit_blocked=(draft_parked || empty(preview_base) || empty(chain))
          preview_text=preview_display_text
          preview_display_clipped=preview_clipped
          dark
          preview_picture
          preview_width
          preview_height
          write_refusal=refusal
        events
          open_message_link -> open_link_at _
          fs_open_dir -> open_dir_at _
          fs_open_file -> open_file_at _
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
