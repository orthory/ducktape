// FORGE, as a module-owned view on the kernel contract. The app pushes
// session facts only (`session()` — connected, dark, the network's name and
// chain id, the endpoint, and the `duck://` link its open plane routed
// here). Everything on this screen is read HERE: the repo namespace, one
// repo's branches and tracker, one item with its patch, reviews and
// discussion, and the code browse's listing and file all come off
// `rpc.query` / `rpc.view`, re-read on every forge block. A review and a
// merge leave as `op.submit`, signed by the kernel with the seated key.
//
// Four things stay the host's because they are host capabilities: the
// picture viewer, the Markdown document, the highlighted code reader — and
// the discussion note composer, docked under this view over the item's own
// channel, since the editor it edits cannot cross the wire.
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
  ForgeBranch(name:str, head:str)
  ForgeItem(number:i64, kind:str, state:str, title:str, author:str, author_name:str)
  ChatSpan(mention:str, mention_link:str, link_text:str, link:str, bold_italic:str, bold:str, italic:str, plain:str)
  ChatBlock(kind:str, text:str, lang:str, rich:bool, spans:[ChatSpan])
  ChatMessage(seq:i64, author:str, meta:str, blocks:[ChatBlock], initial:str, avatar_kind:str, render_rev:i64)
  ChatMember(label:str, key:str)
  ForgeReviewComment(anchor:str, body:str, blocks:[ChatBlock])
  ForgeReview(author:str, author_name:str, verdict:str, body:str, blocks:[ChatBlock], commit:str, outdated:bool, created_at:i64, comments:[ForgeReviewComment])
  ForgeDraftComment(anchor:str, path:str, line:str, side:str, body:str)
  TreeEntry(name:str, path:str, kind:str)
  DiffLine(key:i64, kind:str, old_no:str, new_no:str, sign:str, text:str, path:str, side:str)
  ForgeLink(repo:str, number:i64, seq:i64, path:str, rev:str)
  // the session, pushed by the kernel: nothing here is forge's
  Session(connected:bool, dark:bool, org:str, about:str, tier:str, network_chain_id:str, connected_rpc:str, link:str, link_tick:i64)
  SessionItem(next:Session, error:str)
  // everything below is read by this view, through the kernel
  RepoListItem(repos:[ForgeRepo], error:str)
  RepoItem(repo:str, branches:[ForgeBranch], items:[ForgeItem], error:str)
  ItemItem(repo:str, number:i64, kind:str, title:str, state:str, author:str, branches:str, body:str, blocks:[ChatBlock], channel_id:str, source_branch:str, source_oid:str, target_oid:str, merge_oid:str, diff_rows:[DiffLine], diff_truncated:bool, files_changed:i64, additions:i64, deletions:i64, reviews:[ForgeReview], approvals:i64, change_requests:i64, error:str)
  DiscussionItem(channel_id:str, messages:[ChatMessage], members:[ChatMember], clipped:bool, error:str)
  TreeItem(repo:str, rev:str, path:str, born:bool, entries:[TreeEntry], truncated:bool, error:str)
  BlobItem(repo:str, path:str, text:str, truncated:bool, binary:bool, picture:bool, width:i64, height:i64, note:str, error:str)
  ActItem(kind:str, merge_oid:str, conflicts:[str], error:str)
  subscription session() -> SessionItem
  subscription repos(connection:i64) -> RepoListItem
  subscription repo(connection:i64, repo:str) -> RepoItem
  subscription item(connection:i64, repo:str, number:i64) -> ItemItem
  subscription discussion(connection:i64, channel_id:str) -> DiscussionItem
  subscription tree(connection:i64, repo:str, rev:str, path:str) -> TreeItem
  subscription blob(connection:i64, repo:str, rev:str, path:str, net:str) -> BlobItem
  // every write's outcome, as the kernel answers it
  subscription acts() -> ActItem
  pure connection_serial_after(was_connected:bool, connected:bool, serial:i64) -> i64
  pure forge_link(url:&str) -> ForgeLink
  pure routed_link(fresh:bool, url:&str) -> str
  pure note_at_seq(discussion:&[ChatMessage], seq:i64) -> [ChatMessage]
  pure phase_of(error:&str) -> str
  pure act_of(kind:&str) -> Act
  pure keep_staged(dropped:bool, staged:[ForgeDraftComment]) -> [ForgeDraftComment]
  pure keep_focus(focused:str, current:&str) -> str
  sync seat_roster(scope:&str, members:&[ChatMember]) -> bool
  sync review_submit(repo:str, number:i64, verdict:str, body:str, commit_oid:str, comments:[ForgeDraftComment]) -> bool
  sync merge(repo:str, number:i64, source_branch:str, expected_source_oid:str, prev_target_oid:str) -> bool
  sync open_link(url:&str) -> bool
  sync copy(text:&str, label:&str) -> bool
  pure icon(name:&str) -> bytes
  pure plural(count:i64, one:&str, many:&str) -> str
  pure filter_forge_items(items:&[ForgeItem], tab:&str) -> [ForgeItem]
  pure forge_open_count(items:&[ForgeItem], kind:&str) -> i64
  pure kind_tab(kind:&str) -> str
  pure forge_merge_note(merge_oid:&str, branches:&str) -> str
  pure verdict_label(verdict:&str) -> str
  pure verdict_pick_label(current:&str, key:&str, label:&str) -> str
  pure markdown_path(path:&str) -> bool
  pure picture_caption(width:i64, height:i64) -> str
  pure binary_note(text:&str) -> str
  pure commit_label(rev:&str) -> str
  pure repo_names(repos:&[ForgeRepo]) -> [str]
  pure branch_names(branches:&[ForgeBranch]) -> [str]
  pure pinned_branch(tree_branch:&str) -> str?
  pure forge_branch_head(branches:&[ForgeBranch], name:&str) -> str
  pure forge_tree_branch(branches:&[ForgeBranch], picked:&str, rev:&str) -> str
  pure forge_parent(path:&str) -> str
  pure forge_file_header(opened_dir:&str, opened_rev:&str, dir:&str, rev:&str, path:&str) -> str
  pure forge_stats(files:i64, additions:i64, deletions:i64) -> str
  pure duck_forge_item_link(repo:&str, number:i64, chain_id:&str) -> str
  pure forge_push_command(rpc:&str) -> str
  pure forge_comment_target(path:&str, line:&str, side:&str) -> str
  pure stage_forge_comment(staged:[ForgeDraftComment], path:str, line:str, side:str, body:str) -> [ForgeDraftComment]
  pure drop_forge_comment(staged:[ForgeDraftComment], anchor:&str) -> [ForgeDraftComment]
  pure forge_comment_cap_reached(staged:&[ForgeDraftComment]) -> bool
  pure forge_branch_moved(next_oid:&str, current_oid:&str) -> bool
  pure staged_comment_drop_note(dropped:bool) -> str
  pure keep_draft(consumed:bool, draft:&str) -> str
  pure tree_width_after_delta(width:f64, delta:f64, viewport:f64) -> f64
  pure landed_seq_of(fresh:bool, seq:i64) -> i64
  pure composer_scope(endpoint:&str, channel_id:&str) -> str
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

// Which write the kernel just answered for.
enum Act
  review
  merge

state
  active_palette:palette[AppTheme] = AppTheme.app
  // ---- the session the kernel pushes ----
  connected = false
  dark = false
  org = ""
  about = ""
  tier = ""
  network_chain_id = ""
  connected_rpc = ""
  // moves when the session comes up: every read starts afresh
  connection_serial:i64 = 0
  // the last `duck://forge/...` the app routed here, counted so the same
  // address twice still lands
  link_tick:i64 = 0
  // ---- the repo namespace ----
  repos:[ForgeRepo] = []
  list_phase = "idle"
  open_repo = ""
  repo_phase = "idle"
  branches:[ForgeBranch] = []
  items:[ForgeItem] = []
  tab = "code"
  // ---- the open item ----
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
  forge_item_source_branch = ""
  forge_item_source_oid = ""
  forge_item_target_oid = ""
  forge_item_channel = ""
  forge_item_reviews:[ForgeReview] = []
  forge_item_approvals:i64 = 0
  forge_item_change_requests:i64 = 0
  // ---- the discussion ----
  discussion:[ChatMessage] = []
  discussion_clipped = false
  linked_note:[ChatMessage] = []
  roster_set = false
  // a deep link's `#seq`, consumed once the discussion holding it lands
  focus_seq:i64 = 0
  landed_tick:i64 = 0
  // a deep link's item number, consumed once its repo's slice lands
  focus_number:i64 = 0
  // ---- the merge box and the review ----
  merge_conflicts:[str] = []
  merge_busy = false
  review_verdict = "comment"
  review_busy = false
  staged_comments:[ForgeDraftComment] = []
  // ---- the code browse ----
  // THE BRANCH SELECTOR: the branch the reader picked ("" until a pick) —
  // the browse itself stays pinned to a commit (`tree_rev`); the pick
  // re-roots it at that branch's head.
  tree_pick = ""
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
  file_phase = "idle"
  // the directory and commit the open file was picked under: the reader
  // retires the preview when either moves
  opened_dir = ""
  opened_rev = ""
  // a deep link's file, opened once the tree it names has landed, and the
  // `@rev` it pinned that tree to
  focus_path = ""
  focus_rev = ""
  // ---- the reader's own drafts ----
  review_draft = ""
  comment_draft = ""
  comment_path = ""
  comment_line = ""
  comment_side = ""
  host_error = ""
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false
  viewport_width = 1280.0
  tree_width = 258.0

// Subscriptions, not mount tasks, so a replacement restored from this
// view's state asks for everything again on its own. Each is keyed by what
// it names, so a repo, item, directory or file that moves re-reads and
// nothing else does.
subscribe
  session() -> session_arrived _
  repos(connection_serial) when connected -> repos_arrived _
  repo(connection_serial, open_repo) when connected -> repo_arrived _
  item(connection_serial, open_repo, forge_item_number) when connected -> item_arrived _
  discussion(connection_serial, forge_item_channel) when connected -> discussion_arrived _
  tree(connection_serial, open_repo, tree_rev, tree_path) when connected -> tree_arrived _
  blob(connection_serial, open_repo, tree_rev, file_path, network_chain_id) when connected -> blob_arrived _
  acts() -> act_done _

on session_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  let next = item.next
  connection_serial = connection_serial_after(connected, next.connected, connection_serial)
  connected = next.connected
  dark = next.dark
  org = next.org
  about = next.about
  tier = next.tier
  network_chain_id = next.network_chain_id
  connected_rpc = next.connected_rpc
  let routed = next.link_tick != link_tick
  link_tick = next.link_tick
  // The palette switch and a routed link both have to close the handler,
  // so the link rides in each appearance arm.
  match appearance_of(next.dark)
    Appearance.light
      active_palette = AppTheme.app
      flow
        from done routed_link(routed, next.link)
        done -> forge_land_link _
    Appearance.dark
      active_palette = AppTheme.app_dark
      flow
        from done routed_link(routed, next.link)
        done -> forge_land_link _

// A `duck://forge/...` the app's open plane routed here: the repo opens
// first, and its second step — an item with its `#seq`, or a file with its
// `@rev` — is parked for the read that lands it.
on forge_land_link(url)
  return if empty(url)
  let link = forge_link(url)
  return if empty(link.repo)
  focus_number = link.number
  focus_seq = link.seq
  focus_path = link.path
  focus_rev = link.rev
  flow
    from done link.repo
    done -> forge_open_repo _

on repos_arrived(next)
  host_error = next.error
  return if !empty(next.error)
  repos = next.repos
  list_phase = "ready"

on repo_arrived(next)
  return if next.repo != open_repo
  host_error = next.error
  return if !empty(next.error)
  branches = next.branches
  items = next.items
  repo_phase = "ready"
  // A DEEP LINK'S SECOND STEP, once and only once: the repo is open, now
  // its item — or, for a link to a FILE, the browse moves to that file's
  // directory (pinned to the `@rev` the link names, else wherever the root
  // listing landed) and `tree_arrived` opens the file under it.
  let parked = focus_number > 0 || !empty(focus_path)
  return if !parked
  tree_rev = keep_focus(focus_rev, tree_rev)
  tree_path = keep_focus(forge_parent(focus_path), tree_path)
  tree_entries = []
  tree_truncated = false
  tree_phase = "loading"
  focus_rev = ""
  return if focus_number <= 0
  let number = focus_number
  focus_number = 0
  flow
    from done number
    done -> forge_open_item _

on item_arrived(next)
  return if next.repo != open_repo || next.number != forge_item_number
  host_error = next.error
  return if !empty(next.error)
  // A staged comment anchors into ONE patch: the anchor is (path, line,
  // side) into a specific diff, and a review is submitted pinning the head
  // on screen. A source head that moved under the open item takes what was
  // written against the old diff with it, rather than posting it against a
  // patch it was never written about.
  let branch_moved = forge_branch_moved(next.source_oid, forge_item_source_oid)
  host_error = staged_comment_drop_note(branch_moved && !empty(staged_comments))
  staged_comments = keep_staged(branch_moved, staged_comments)
  comment_draft = keep_draft(branch_moved, comment_draft)
  comment_path = keep_draft(branch_moved, comment_path)
  comment_line = keep_draft(branch_moved, comment_line)
  comment_side = keep_draft(branch_moved, comment_side)
  item_phase = "ready"
  forge_item_kind = next.kind
  // The tab bar stays up over an open item, so the lit tab is the item's
  // own seat — a duck:// link can land a pull request from the Code seat.
  tab = kind_tab(next.kind)
  forge_item_title = next.title
  forge_item_state = next.state
  forge_item_author = next.author
  forge_item_branches = next.branches
  forge_item_body = next.body
  forge_item_blocks = next.blocks
  forge_item_channel = next.channel_id
  forge_item_source_branch = next.source_branch
  forge_item_source_oid = next.source_oid
  forge_item_target_oid = next.target_oid
  forge_item_merge_oid = next.merge_oid
  diff_rows = next.diff_rows
  forge_item_diff_truncated = next.diff_truncated
  forge_item_files_changed = next.files_changed
  forge_item_additions = next.additions
  forge_item_deletions = next.deletions
  forge_item_reviews = next.reviews
  forge_item_approvals = next.approvals
  forge_item_change_requests = next.change_requests

on discussion_arrived(next)
  return if next.channel_id != forge_item_channel
  host_error = next.error
  return if !empty(next.error)
  discussion = next.messages
  discussion_clipped = next.clipped
  // The composer is the host's, so its mention menu is too.
  roster_set = seat_roster(composer_scope(connected_rpc, forge_item_channel), next.members)
  // A DEEP LINK'S `#seq` LANDS ONCE: the note is picked out of the list and
  // the item page scrolls to that row by its key in the discussion's keyed
  // column (a seq the list does not hold scrolls nowhere; the linked-note
  // card above the list still shows it).
  return if focus_seq == 0
  let landed = focus_seq
  focus_seq = 0
  linked_note = note_at_seq(next.messages, landed)
  landed_tick = landed_tick + 1
  task widget scroll-to-key #forge/item-detail landed

// A listing answers for one repo and one directory; once the root has
// pinned the commit it answers for that too.
on tree_arrived(next)
  return if next.repo != open_repo || next.path != tree_path
  return if !empty(tree_rev) && next.rev != tree_rev
  host_error = next.error
  tree_phase = phase_of(next.error)
  return if !empty(next.error)
  tree_rev = next.rev
  tree_born = next.born
  tree_entries = next.entries
  tree_truncated = next.truncated
  // A deep link's file opens under the tree it moved to, and only then.
  return if empty(focus_path)
  let path = focus_path
  focus_path = ""
  flow
    from done path
    done -> forge_open_file _

// A blob answers for ONE file: a completion naming another path is a
// superseded read landing late, and painting it would put one file's source
// under another file's header.
on blob_arrived(next)
  return if next.repo != open_repo || next.path != file_path
  file_note = next.note
  file_phase = phase_of(next.error)
  return if !empty(next.error)
  file_text = next.text
  file_binary = next.binary
  file_truncated = next.truncated
  file_picture = next.picture
  file_width = next.width
  file_height = next.height

// A COMMITTED WRITE lands here with what it produced. The review's drafts
// went out INSIDE it, so they are cleared with the body; a failure keeps
// them — the whole submit is one transaction, and losing a page of written
// comments to a transient error is not recoverable.
on act_done(next)
  host_error = next.error
  match act_of(next.kind)
    Act.review
      review_busy = false
      return if !empty(next.error)
      review_verdict = "comment"
      staged_comments = []
      review_draft = ""
      comment_draft = ""
      comment_path = ""
      comment_line = ""
      comment_side = ""
    Act.merge
      merge_busy = false
      merge_conflicts = next.conflicts

// ---- the acts the screen offers ----

// Picking a repo starts the code browse at the ROOT: the root listing
// answers with the exact commit every nested read is then pinned to. The
// previous repo's item, listing and file go with it.
on forge_open_repo(name)
  return if !connected
  open_repo = name
  host_error = ""
  repo_phase = "loading"
  branches = []
  items = []
  tab = "code"
  tree_pick = ""
  forge_item_number = 0
  item_phase = "idle"
  forge_item_channel = ""
  linked_note = []
  diff_rows = []
  discussion = []
  discussion_clipped = false
  merge_conflicts = []
  staged_comments = []
  tree_path = ""
  tree_rev = ""
  tree_entries = []
  tree_born = false
  tree_truncated = false
  tree_phase = "loading"
  file_path = ""
  file_text = ""
  file_note = ""
  file_phase = "idle"
  opened_dir = ""
  opened_rev = ""

// The breadcrumb home. Nothing else clears `open_repo`, so without this the
// repo grid is unreachable for the rest of the session once a repo is open.
on forge_close_repo
  open_repo = ""
  repo_phase = "idle"
  branches = []
  items = []
  tree_pick = ""
  forge_item_number = 0
  item_phase = "idle"
  forge_item_channel = ""
  linked_note = []
  focus_seq = 0
  diff_rows = []
  discussion = []
  discussion_clipped = false
  merge_conflicts = []
  staged_comments = []
  tree_path = ""
  tree_rev = ""
  tree_entries = []
  tree_born = false
  tree_truncated = false
  tree_phase = "loading"
  file_path = ""
  file_text = ""
  file_note = ""
  file_phase = "idle"
  opened_dir = ""
  opened_rev = ""

// A BRANCH PICK: the browse re-roots at that branch's head — the commit the
// repo slice last read it at — and every nested read is then pinned there.
// The listing starts at the root because the picked branch need not hold
// the open directory. A name the slice no longer holds picks nothing; the
// open file retires through `forge_file_header` as it does on any move.
on forge_pick_branch(name)
  return if !connected || empty(open_repo)
  let head = forge_branch_head(branches, name)
  return if empty(head)
  tree_pick = name
  tree_path = ""
  tree_entries = []
  tree_truncated = false
  tree_phase = "loading"
  tree_rev = head

// A DIRECTORY ROW: the listing moves, pinned to the tree's commit. The file
// opened in the previous directory is retired by the move itself —
// `forge_file_header` names it only while the tree stands where it was
// opened.
on forge_open_dir(path)
  return if !connected || empty(open_repo)
  tree_path = path
  tree_entries = []
  tree_truncated = false
  tree_phase = "loading"

// A FILE ROW. The read is pinned to the tree's commit and remembers the
// directory and revision it was opened under, so the reader can retire the
// preview when either moves. The previous file's flags must not describe
// the one in flight: a stale `binary` would brand the next blob "not text"
// until its load settles.
on forge_open_file(path)
  return if !connected || empty(open_repo)
  opened_dir = tree_path
  opened_rev = tree_rev
  file_path = path
  file_text = ""
  file_binary = false
  file_truncated = false
  file_picture = false
  file_note = ""
  file_phase = "loading"

on forge_open_item(number)
  return if !connected || empty(open_repo)
  forge_item_number = number
  // The highlight belongs to the item it was landed on; the one-shot
  // `focus_seq` survives this open — it is THIS open's landing.
  linked_note = []
  host_error = ""
  item_phase = "loading"
  // THE PREVIOUS ITEM'S CHANNEL RETIRES NOW, not when the next item lands:
  // a note submitted for it that is still crossing the wire must find no
  // box on screen to post into, and go back to its own.
  forge_item_channel = ""
  review_verdict = "comment"
  // A staged comment anchors to THIS item's diff. Carrying one across items
  // would post it against a patch it was never written about.
  staged_comments = []
  review_draft = ""
  comment_draft = ""
  comment_path = ""
  comment_line = ""
  comment_side = ""
  merge_conflicts = []
  diff_rows = []
  discussion = []
  discussion_clipped = false

on forge_close_item
  forge_item_number = 0
  item_phase = "idle"
  forge_item_channel = ""
  linked_note = []
  focus_seq = 0
  diff_rows = []
  discussion = []
  discussion_clipped = false
  merge_conflicts = []
  staged_comments = []

on select_forge_tab(next)
  tab = next
  // The tab bar stays up over an open item, so a tab press is also the way
  // out of the item: it leaves the detail and shows the seat. With no item
  // open there is nothing to leave.
  return if forge_item_number <= 0
  flow
    from done true
    done -> forge_close_item()

on forge_review_pick(verdict)
  review_verdict = verdict

// The module refuses a review that is empty on BOTH halves, so the guard
// refuses it first rather than spending a round trip to be told.
on forge_review_submit(body)
  return if review_busy || !connected || empty(forge_item_source_oid)
  return if empty(body) && empty(staged_comments)
  review_busy = true
  sent = review_submit(open_repo, forge_item_number, review_verdict, body, forge_item_source_oid, staged_comments)

on forge_merge_submit
  return if !connected || merge_busy || empty(open_repo) || forge_item_number <= 0
  merge_busy = true
  merge_conflicts = []
  sent = merge(open_repo, forge_item_number, forge_item_source_branch, forge_item_source_oid, forge_item_target_oid)

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

// `stage_forge_comment` is the authority on what is stageable and on
// replacing the comment already at this anchor; the guard here keeps the
// obviously empty case off it.
on forge_comment_stage(body)
  return if empty(comment_path) || empty(body) || forge_comment_cap_reached(staged_comments)
  staged_comments = stage_forge_comment(staged_comments, comment_path, comment_line, comment_side, body)
  comment_path = ""
  comment_line = ""
  comment_side = ""
  comment_draft = ""

on forge_comment_drop(anchor)
  staged_comments = drop_forge_comment(staged_comments, anchor)

on open_message_link(url)
  sent = open_link(url)

on copy_to_clipboard(text, label)
  sent = copy(text, label)

on tree_resized(dx, _dy)
  tree_width = tree_width_after_delta(tree_width, dx, viewport_width)

on viewport_changed(width, _height)
  viewport_width = width
  tree_width = tree_width_after_delta(tree_width, 0.0, width)

view
  col w=fill h=fill
    sensor show=viewport_changed resize=viewport_changed
      space w=fill h=0.0
    if !empty(host_error) && empty(repos)
      box w=fill h=fill p=22.0
        EmptyState
          with
            title="Unable to read Forge"
            // the kernel's own words: a refusal a reader cannot read is a
            // refusal nobody can act on
            description=host_error
    if empty(host_error) || !empty(repos)
      col w=fill h=fill
        ForgeScreen review_draft<->review_draft comment_draft<->comment_draft #forge
          with
            tree_width
            org
            about
            tier
            network_chain_id
            connected_rpc
            repos
            list_phase
            open_repo
            repo_phase
            branches
            tree_branch=forge_tree_branch(branches, tree_pick, tree_rev)
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
            has_merge_conflicts=!empty(merge_conflicts)
            merge_busy
            review_verdict
            review_busy
            comment_target=forge_comment_target(comment_path, comment_line, comment_side)
            staged_comments
            has_staged_comments=!empty(staged_comments)
            comment_cap_reached=forge_comment_cap_reached(staged_comments)
            discussion
            linked_note
            discussion_clipped
            note_scope=composer_scope(connected_rpc, forge_item_channel)
            note_blocked=(!connected || empty(forge_item_channel) || item_phase != "ready")
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
            file_header=forge_file_header(opened_dir, opened_rev, tree_path, tree_rev, file_path)
            file_phase
            connected
            dark
          events
            forge_open_repo -> forge_open_repo _
            forge_close_repo -> forge_close_repo
            forge_pick_branch -> forge_pick_branch _
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
            resize_tree -> tree_resized _ _
