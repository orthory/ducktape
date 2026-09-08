// FORGE, as a module-owned view: the repo overview, one repo's Code / Pull
// requests / Issues seats, and the item detail with its merge box, reviews
// and discussion, drawn from the facts the desktop app pushes. The screen
// body is the app's own (screens/forge.ice and components/forge.ice before
// the port). The review body and the line comment being written are the
// view's: the app hears them only when the reader submits, and tells the
// view which drafts an op consumed through `drafts_cleared`. The code
// browse and the discussion note composer are the app's: a pick leaves as
// an intent and the listing or blob comes back as props, and the composer
// is docked under this view by the app, since the editor it edits cannot
// cross the wire.
app ForgeView
  title "Forge"
  palette active_palette
  id "dev.ducktape.view.forge"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"
use "../../../../../app/src/ui/ducktape-ui/recipes.ice"
use "../../../../../app/src/ui/components/icon.ice"
use "kit.ice"
use "components.ice"
use "forge.ice"

extern crate::host
  ForgeRepo(name:str, head:str)
  ForgeItem(number:i64, kind:str, state:str, title:str, author:str, author_name:str)
  ChatSpan(mention:str, link_text:str, link:str, bold_italic:str, bold:str, italic:str, plain:str)
  ChatBlock(kind:str, text:str, lang:str, rich:bool, spans:[ChatSpan])
  ChatMessage(seq:i64, author:str, meta:str, blocks:[ChatBlock], initial:str, avatar_kind:str, render_rev:i64)
  ForgeReviewComment(anchor:str, body:str, blocks:[ChatBlock])
  ForgeReview(author:str, author_name:str, verdict:str, body:str, blocks:[ChatBlock], commit:str, outdated:bool, created_at:i64, comments:[ForgeReviewComment])
  ForgeDraftComment(anchor:str, path:str, line:str, side:str, body:str)
  TreeEntry(name:str, path:str, kind:str)
  DiffLine(key:i64, kind:str, old_no:str, new_no:str, sign:str, text:str, path:str, side:str)
  ForgeProps(display_omitted:i64, display_shortened:bool, display_unavailable:bool, dark:bool, connected:bool, org:str, about:str, tier:str, network_chain_id:str, connected_rpc:str, repos:[ForgeRepo], list_phase:str, open_repo:str, repo_menu:bool, repo_phase:str, branches:[str], tab:str, items:[ForgeItem], forge_item_number:i64, item_phase:str, forge_item_kind:str, forge_item_title:str, forge_item_state:str, forge_item_author:str, forge_item_branches:str, forge_item_body:str, forge_item_blocks:[ChatBlock], forge_item_files_changed:i64, forge_item_additions:i64, forge_item_deletions:i64, diff_rows:[DiffLine], forge_item_diff_truncated:bool, forge_item_merge_oid:str, forge_item_source_oid:str, forge_item_approvals:i64, forge_item_change_requests:i64, forge_item_reviews:[ForgeReview], merge_conflicts:[str], has_merge_conflicts:bool, merge_busy:bool, review_verdict:str, review_busy:bool, staged_comments:[ForgeDraftComment], has_staged_comments:bool, comment_cap_reached:bool, discussion:[ChatMessage], linked_note:[ChatMessage], discussion_clipped:bool, landed_seq:i64, landed_tick:i64, tree_path:str, tree_rev:str, tree_entries:[TreeEntry], tree_born:bool, tree_truncated:bool, tree_phase:str, file_path:str, file_text:str, file_binary:bool, file_truncated:bool, file_picture:bool, file_width:i64, file_height:i64, file_note:str, file_header:str, file_phase:str, drafts_cleared:i64, drafts_scope:str, note_scope:str, note_blocked:bool)
  PropsItem(next:ForgeProps, error:str)
  subscription props() -> PropsItem
  pure open_repo(name:&str) -> bool
  pure close_repo() -> bool
  pure toggle_repo_menu() -> bool
  pure pick_tab(tab:&str) -> bool
  pure open_item(number:i64) -> bool
  pure close_item() -> bool
  pure merge() -> bool
  pure review_pick(verdict:&str) -> bool
  pure review_submit(body:&str) -> bool
  pure comment_stage(path:&str, line:&str, side:&str, body:&str) -> bool
  pure comment_drop(anchor:&str) -> bool
  pure open_dir(path:&str) -> bool
  pure open_file(path:&str) -> bool
  pure open_link(url:&str) -> bool
  pure copy(text:&str, label:&str) -> bool
  pure icon(name:&str) -> bytes
  pure plural(count:i64, one:&str, many:&str) -> str
  pure filter_forge_items(items:&[ForgeItem], tab:&str) -> [ForgeItem]
  pure forge_open_count(items:&[ForgeItem], kind:&str) -> i64
  pure forge_merge_note(merge_oid:&str, branches:&str) -> str
  pure verdict_label(verdict:&str) -> str
  pure verdict_pick_label(current:&str, key:&str, label:&str) -> str
  pure markdown_path(path:&str) -> bool
  pure picture_caption(width:i64, height:i64) -> str
  pure binary_note(text:&str) -> str
  pure duck_forge_item_link(repo:&str, number:i64, chain_id:&str) -> str
  pure duck_forge_repo_link(repo:&str, chain_id:&str) -> str
  pure forge_push_command(rpc:&str) -> str
  pure forge_comment_target(path:&str, line:&str, side:&str) -> str
  pure drafts_cleared_by(scope:&str, draft:&str) -> bool
  pure keep_draft(consumed:bool, draft:&str) -> str
  pure landed_seq_of(fresh:bool, seq:i64) -> i64
  pure appearance_of(dark:bool) -> Appearance
  // HOST SURFACES — drawn by the desktop app in the slot this view leaves:
  // the decoded picture parked under the forge surface, the document-aware
  // Markdown reader (its links come back here), the highlighted code reader.
  component picture(surface:str, path:str) -> unit
  component forge_markdown(source:str, doc:str, dark:bool) -> str
  component forge_code(source:str, path:str, dark:bool) -> unit
  // the discussion note composer: the app's rich composer over a document
  // it keeps per scope; a submit crosses as the `composer` intent
  component forge_composer(scope:str, kind:str, compact:bool, hint:str, blocked:bool, restore_blocked:bool, failed_note:str) -> unit

// The appearance as a word the handler can match on: a handler branches on
// an enum, and the palette switch has to sit above the landing flow.
enum Appearance
  light
  dark

state
  display_omitted:i64 = 0
  display_shortened = false
  display_unavailable = false
  active_palette:palette[AppTheme] = AppTheme.app
  connected = false
  dark = false
  org = ""
  about = ""
  tier = ""
  network_chain_id = ""
  connected_rpc = ""
  repos:[ForgeRepo] = []
  list_phase = "idle"
  open_repo = ""
  repo_menu = false
  repo_phase = "idle"
  branches:[str] = []
  tab = "code"
  items:[ForgeItem] = []
  forge_item_number:i64 = 0
  item_phase = "idle"
  forge_item_kind = ""
  forge_item_title = ""
  forge_item_state = ""
  forge_item_author = ""
  forge_item_branches = ""
  forge_item_body = ""
  forge_item_blocks:[ChatBlock] = []
  forge_item_files_changed:i64 = 0
  forge_item_additions:i64 = 0
  forge_item_deletions:i64 = 0
  diff_rows:[DiffLine] = []
  forge_item_diff_truncated = false
  forge_item_merge_oid = ""
  forge_item_source_oid = ""
  forge_item_approvals:i64 = 0
  forge_item_change_requests:i64 = 0
  forge_item_reviews:[ForgeReview] = []
  merge_conflicts:[str] = []
  merge_busy = false
  review_verdict = "comment"
  review_busy = false
  staged_comments:[ForgeDraftComment] = []
  comment_cap_reached = false
  discussion:[ChatMessage] = []
  linked_note:[ChatMessage] = []
  has_merge_conflicts = false
  has_staged_comments = false
  discussion_clipped = false
  note_scope = ""
  note_blocked = true
  // the last landing the app reported: a count that moves once per deep
  // link, and the discussion seq it landed on
  landed_tick:i64 = 0
  tree_path = ""
  tree_rev = ""
  tree_entries:[TreeEntry] = []
  tree_born = false
  tree_truncated = false
  tree_phase = "loading"
  file_path = ""
  file_text = ""
  file_binary = false
  file_truncated = false
  file_picture = false
  file_width:i64 = 0
  file_height:i64 = 0
  file_note = ""
  file_header = ""
  file_phase = "idle"
  // the last consumption the app reported: a count that moves once per
  // committed op, and which drafts it took (`item`, `review`, `comment`)
  drafts_cleared:i64 = 0
  // the reader's own: the review body, and the line comment with the diff
  // line it is written against
  review_draft = ""
  comment_draft = ""
  comment_path = ""
  comment_line = ""
  comment_side = ""
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
  connected = next.connected
  dark = next.dark
  org = next.org
  about = next.about
  tier = next.tier
  network_chain_id = next.network_chain_id
  connected_rpc = next.connected_rpc
  repos = next.repos
  list_phase = next.list_phase
  open_repo = next.open_repo
  repo_menu = next.repo_menu
  repo_phase = next.repo_phase
  branches = next.branches
  tab = next.tab
  items = next.items
  forge_item_number = next.forge_item_number
  item_phase = next.item_phase
  forge_item_kind = next.forge_item_kind
  forge_item_title = next.forge_item_title
  forge_item_state = next.forge_item_state
  forge_item_author = next.forge_item_author
  forge_item_branches = next.forge_item_branches
  forge_item_body = next.forge_item_body
  forge_item_blocks = next.forge_item_blocks
  forge_item_files_changed = next.forge_item_files_changed
  forge_item_additions = next.forge_item_additions
  forge_item_deletions = next.forge_item_deletions
  diff_rows = next.diff_rows
  forge_item_diff_truncated = next.forge_item_diff_truncated
  forge_item_merge_oid = next.forge_item_merge_oid
  forge_item_source_oid = next.forge_item_source_oid
  forge_item_approvals = next.forge_item_approvals
  forge_item_change_requests = next.forge_item_change_requests
  forge_item_reviews = next.forge_item_reviews
  merge_conflicts = next.merge_conflicts
  has_merge_conflicts = next.has_merge_conflicts
  merge_busy = next.merge_busy
  review_verdict = next.review_verdict
  review_busy = next.review_busy
  staged_comments = next.staged_comments
  has_staged_comments = next.has_staged_comments
  comment_cap_reached = next.comment_cap_reached
  discussion = next.discussion
  linked_note = next.linked_note
  discussion_clipped = next.discussion_clipped
  note_scope = next.note_scope
  note_blocked = next.note_blocked
  tree_path = next.tree_path
  tree_rev = next.tree_rev
  tree_entries = next.tree_entries
  tree_born = next.tree_born
  tree_truncated = next.tree_truncated
  tree_phase = next.tree_phase
  file_path = next.file_path
  file_text = next.file_text
  file_binary = next.file_binary
  file_truncated = next.file_truncated
  file_picture = next.file_picture
  file_width = next.file_width
  file_height = next.file_height
  file_note = next.file_note
  file_header = next.file_header
  file_phase = next.file_phase
  // A COMMITTED OP CONSUMES THE DRAFTS IT READ, and only those: a landed
  // review takes its body and the line comment, a branch that moved under
  // the composer takes the line comment alone, an opened item takes all.
  let consumed = next.drafts_cleared != drafts_cleared
  drafts_cleared = next.drafts_cleared
  let review_consumed = consumed && drafts_cleared_by(next.drafts_scope, "review")
  let comment_consumed = consumed && drafts_cleared_by(next.drafts_scope, "comment")
  review_draft = keep_draft(review_consumed, review_draft)
  comment_draft = keep_draft(comment_consumed, comment_draft)
  comment_path = keep_draft(comment_consumed, comment_path)
  comment_line = keep_draft(comment_consumed, comment_line)
  comment_side = keep_draft(comment_consumed, comment_side)
  // A DEEP LINK'S `#seq` LANDS ONCE: the app counts each landing, and the
  // item page scrolls to that row by its key in the discussion's keyed
  // column (a seq the list does not hold scrolls nowhere; the linked-note
  // card above the list still shows it).
  let fresh_landing = next.landed_tick != landed_tick
  landed_tick = next.landed_tick
  // The palette switch and the landing both have to close the handler, so
  // the landing rides in each appearance arm.
  match appearance_of(next.dark)
    Appearance.light
      active_palette = AppTheme.app
      flow
        from done landed_seq_of(fresh_landing, next.landed_seq)
        done -> land_note _
    Appearance.dark
      active_palette = AppTheme.app_dark
      flow
        from done landed_seq_of(fresh_landing, next.landed_seq)
        done -> land_note _

on land_note(seq)
  return if seq <= 0
  task widget scroll-to-key #forge/item-detail seq

on forge_open_repo(name)
  sent = open_repo(name)

on forge_close_repo
  sent = close_repo()

on forge_toggle_repo_menu
  sent = toggle_repo_menu()

on select_forge_tab(next)
  sent = pick_tab(next)

on forge_open_item(number)
  sent = open_item(number)

on forge_close_item
  sent = close_item()

on forge_merge_submit
  sent = merge()

on forge_review_pick(verdict)
  sent = review_pick(verdict)

// The module refuses a review that is empty on BOTH halves, so the guard
// refuses it first rather than spending a round trip to be told.
on forge_review_submit(body)
  return if review_busy || !connected || empty(forge_item_source_oid) || (empty(body) && !has_staged_comments)
  sent = review_submit(body)

// Clicking a diff gutter PICKS the line — it does not stage anything yet.
// The picked anchor is what the composer writes against, and picking a
// second line simply moves it, so a mis-click costs nothing.
on forge_comment_open(path, line, side)
  return if empty(path)
  comment_path = path
  comment_line = line
  comment_side = side

on forge_comment_cancel
  comment_path = ""
  comment_line = ""
  comment_side = ""
  comment_draft = ""

// The app is the authority on what is stageable and on replacing the comment
// already at this anchor; the guard here keeps the obviously empty case off
// the wire. The composer empties on the send: the staged row comes back as
// props, and a refused one is the app's to report.
on forge_comment_stage(body)
  return if empty(comment_path) || empty(body) || comment_cap_reached
  sent = comment_stage(comment_path, comment_line, comment_side, body)
  comment_path = ""
  comment_line = ""
  comment_side = ""
  comment_draft = ""

on forge_comment_drop(anchor)
  sent = comment_drop(anchor)

on forge_open_dir(path)
  sent = open_dir(path)

on forge_open_file(path)
  sent = open_file(path)

on open_message_link(url)
  sent = open_link(url)

on copy_to_clipboard(text, label)
  sent = copy(text, label)

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
        ForgeScreen review_draft<->review_draft comment_draft<->comment_draft #forge
          with
            display_omitted
            org
            about
            tier
            network_chain_id
            connected_rpc
            repos
            list_phase
            open_repo
            repo_menu
            repo_phase
            branches
            tab
            items
            forge_item_number
            item_phase
            forge_item_kind
            forge_item_title
            forge_item_state
            forge_item_author
            forge_item_branches
            forge_item_body
            forge_item_blocks
            forge_item_files_changed
            forge_item_additions
            forge_item_deletions
            diff_rows
            forge_item_diff_truncated
            forge_item_merge_oid
            forge_item_source_oid
            forge_item_approvals
            forge_item_change_requests
            forge_item_reviews
            merge_conflicts
            has_merge_conflicts
            merge_busy
            review_verdict
            review_busy
            comment_target=forge_comment_target(comment_path, comment_line, comment_side)
            staged_comments
            has_staged_comments
            comment_cap_reached
            discussion
            linked_note
            discussion_clipped
            note_scope
            note_blocked
            tree_path
            tree_rev
            tree_entries
            tree_born
            tree_truncated
            tree_phase
            file_path
            file_text
            file_binary
            file_truncated
            file_picture
            file_width
            file_height
            file_note
            file_header
            file_phase
            connected
            dark
          events
            forge_open_repo -> forge_open_repo _
            forge_close_repo -> forge_close_repo
            forge_toggle_repo_menu -> forge_toggle_repo_menu
            select_forge_tab -> select_forge_tab _
            forge_open_item -> forge_open_item _
            forge_close_item -> forge_close_item
            forge_merge_submit -> forge_merge_submit
            forge_review_pick -> forge_review_pick _
            forge_review_submit -> forge_review_submit _
            forge_comment_open -> forge_comment_open _ _ _
            forge_comment_stage -> forge_comment_stage _
            forge_comment_cancel -> forge_comment_cancel
            forge_comment_drop -> forge_comment_drop _
            forge_open_dir -> forge_open_dir _
            forge_open_file -> forge_open_file _
            open_message_link -> open_message_link _
            copy_to_clipboard -> copy_to_clipboard _ _
