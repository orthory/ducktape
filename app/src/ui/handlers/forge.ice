// FORGE — repos, the tracker, one item with its reviews, its merge and its
// discussion. The list/repo/item loaders key on `forge_generation`; the
// replace-lane discussion and code reads carry their semantic scope instead.

// The committed namespace is the whole repo-card answer: name and head. Code
// Browsing queries only the requested listing/blob; merge fetches the selected
// repo only when that explicit act needs a client-computed commit.
on forge_loaded(next)
  return if next.generation != forge_generation
  forge_list_phase = ForgePhase.ready
  forge_repos = next.repos
  error = ""

on forge_list_failed(cause)
  return if cause.generation != forge_generation
  forge_list_phase = ForgePhase.failed
  error = cause.message

on forge_live_failed(cause)
  return if cause.generation != forge_generation
  error = cause.message

// Picking a repo also DISMISSES the switcher. Nothing else clears it on this
// route, so the popover stayed pinned over the first rows of the tracker list
// the user just navigated to, with the crumb as the only way out.
on forge_open_repo(name)
  return if !connected
  invalidate lane=forge_item
  invalidate lane=forge_discussion
  forge_repo_menu = false
  forge_repo = name
  error = ""
  forge_repo_phase = ForgePhase.loading
  forge_branches = []
  forge_items = []
  forge_item_number = 0
  forge_item_phase = ForgePhase.idle
  forge_item_diff = ""
  forge_generation = forge_generation + 1
  // THE CODE TAB LOADS ITS OWN CONTENT NOW. Opening a repo landed on Code with
  // an empty file tree and a line reading "Pick Code again to browse this
  // repository" — the screen asking to be clicked on the tab it was already
  // showing, because only `select_forge_tab` ever fired the tree read. Same
  // reset block as that handler, and the two loads go out together.
  forge_tab = ForgeTab.code
  forge_tree_path = ""
  forge_tree_rev = ""
  forge_tree_entries = []
  forge_tree_truncated = false
  forge_file_path = ""
  forge_file_text = ""
  forge_file_binary = false
  forge_file_truncated = false
  forge_tree_born = false
  forge_code_phase = ForgeCodePhase.tree_loading
  parallel
    run replace lane=forge_repo load_forge_repo(connected_rpc, forge_repo, forge_generation) -> forge_repo_loaded _ | forge_repo_failed _
    run replace lane=forge_code forge_tree(connected_rpc, forge_repo, "", "") -> forge_tree_loaded _ | forge_tree_failed _

on forge_repo_loaded(next)
  return if next.generation != forge_generation
  forge_repo = next.repo
  forge_repo_phase = ForgePhase.ready
  forge_branches = next.branches
  forge_items = next.items

on forge_repo_failed(cause)
  return if cause.generation != forge_generation
  forge_repo_phase = ForgePhase.failed
  error = cause.message

on forge_open_item(number)
  return if !connected || empty(forge_repo)
  invalidate lane=forge_code
  invalidate lane=forge_discussion
  forge_item_number = number
  error = ""
  forge_item_phase = ForgePhase.loading
  forge_review_verdict = ForgeReviewVerdict.comment
  forge_review_draft = ""
  // A staged comment anchors to THIS item's diff. Carrying one across items
  // would post it against a patch it was never written about.
  forge_comment_staged = []
  forge_comment_path = ""
  forge_comment_line = ""
  forge_comment_side = ""
  forge_comment_draft = ""
  forge_merge_conflicts = []
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
  forge_discussion_editor = editor("")
  forge_generation = forge_generation + 1
  run replace lane=forge_item load_forge_item(connected_rpc, forge_repo, forge_item_number, forge_generation) -> forge_item_loaded _ | forge_item_failed _

on forge_item_loaded(next)
  return if next.generation != forge_generation
  forge_item_number = next.number
  forge_item_phase = ForgePhase.ready
  forge_item_title = next.title
  forge_item_state = next.state
  forge_item_kind = next.kind
  forge_item_body = next.body
  forge_item_author = next.author_name
  forge_item_branches = next.branches
  forge_item_channel = next.channel_id
  forge_item_source_branch = next.source_branch
  forge_item_source_oid = next.source_oid
  forge_item_target_oid = next.target_oid
  forge_item_merge_oid = next.merge_oid
  forge_item_diff = next.diff
  forge_item_diff_truncated = next.diff_truncated
  forge_item_files_changed = next.files_changed
  forge_item_additions = next.additions
  forge_item_deletions = next.deletions
  forge_item_reviews = next.reviews
  forge_item_approvals = next.approvals
  forge_item_change_requests = next.change_requests
  error = ""
  return if empty(forge_item_channel)
  run replace lane=forge_discussion load_forge_discussion(connected_rpc, forge_item_channel) -> forge_discussion_loaded _ | forge_discussion_failed _

on forge_item_failed(cause)
  return if cause.generation != forge_generation
  forge_item_phase = ForgePhase.failed
  error = cause.message

on forge_discussion_loaded(next)
  return if next.channel_id != forge_item_channel
  forge_discussion = next.messages
  forge_discussion_members = next.members

on forge_discussion_failed(cause)
  error = cause.message

on forge_review_pick(verdict)
  forge_review_verdict = verdict

// Clicking a diff gutter PICKS the line — it does not stage anything yet. The
// picked anchor is what the composer writes against, and picking a second line
// simply moves it, so a mis-click costs nothing.
on forge_comment_open(path, line, side)
  return if empty(path)
  forge_comment_path = path
  forge_comment_line = line
  forge_comment_side = side

on forge_comment_cancel
  forge_comment_path = ""
  forge_comment_line = ""
  forge_comment_side = ""
  forge_comment_draft = ""

// `stage_forge_comment` is the authority on what is stageable and on replacing
// the comment already at this anchor; the guard here only keeps the obviously
// empty case off the call.
on forge_comment_stage
  return if empty(forge_comment_path) || empty(forge_comment_draft)
  forge_comment_staged = stage_forge_comment(forge_comment_staged, forge_comment_path, forge_comment_line, forge_comment_side, forge_comment_draft)
  forge_comment_path = ""
  forge_comment_line = ""
  forge_comment_side = ""
  forge_comment_draft = ""

on forge_comment_drop(anchor)
  forge_comment_staged = drop_forge_comment(forge_comment_staged, anchor)

on forge_review_submit
  return if !connected || forge_review_busy || empty(forge_repo) || forge_item_number <= 0
  forge_review_busy = true
  run every submit_forge_review(connected_rpc, password, forge_repo, forge_item_number, forge_review_verdict, forge_review_draft, forge_item_source_oid, forge_comment_staged) -> forge_review_submitted(connected_rpc, forge_repo, forge_item_number, _) | forge_review_failed(connected_rpc, forge_repo, forge_item_number, _)

// The staged comments went out INSIDE this review, so they are cleared with the
// body. A failure keeps them — the whole submit is one transaction, and losing
// a page of written comments to a transient RPC error is not recoverable.
on forge_review_submitted(started_rpc, started_repo, started_number, _result)
  forge_review_busy = false
  return if started_rpc != connected_rpc || started_repo != forge_repo || started_number != forge_item_number
  forge_review_draft = ""
  forge_review_verdict = ForgeReviewVerdict.comment
  forge_comment_staged = []
  forge_comment_path = ""
  forge_comment_line = ""
  forge_comment_side = ""
  forge_comment_draft = ""
  error = ""

on forge_review_failed(started_rpc, started_repo, started_number, cause)
  forge_review_busy = false
  return if started_rpc != connected_rpc || started_repo != forge_repo || started_number != forge_item_number
  error = cause.message

on forge_merge_submit
  return if !connected || forge_merge_busy || empty(forge_repo) || forge_item_number <= 0
  forge_merge_busy = true
  forge_merge_conflicts = []
  run every merge_forge_pr(connected_rpc, password, forge_repo, forge_item_number, forge_item_source_branch, forge_item_source_oid, forge_item_target_oid) -> forge_merged(connected_rpc, forge_repo, forge_item_number, _) | forge_merge_failed(connected_rpc, forge_repo, forge_item_number, _)

on forge_merged(started_rpc, started_repo, started_number, next)
  // RELEASED ABOVE THE IDENTITY CHECK, the same shape as `history_loaded`.
  // The launch route snapshots endpoint+repo+number; closing an item or
  // switching networks rejects the body while this reply still lowers the one
  // session-wide busy flag.
  forge_merge_busy = false
  return if started_rpc != connected_rpc || started_repo != forge_repo || started_number != forge_item_number
  forge_merge_conflicts = next.conflicts
  error = ""

on forge_merge_failed(started_rpc, started_repo, started_number, cause)
  forge_merge_busy = false
  return if started_rpc != connected_rpc || started_repo != forge_repo || started_number != forge_item_number
  error = cause.message

on forge_composer_event(event)
  forge_discussion_editor = apply_composer_event(forge_discussion_editor, event)
  return if !composer_submits(event)
  return if loading || !connected || empty(forge_item_channel) || !empty(forge_discussion_pending) || empty(trim(editor_text(forge_discussion_editor)))
  forge_discussion_pending = fresh_operation_id("forge-note")
  run every send_message(connected_rpc, password, forge_item_channel, forge_discussion_pending, trim(editor_text(forge_discussion_editor)), forge_discussion_members) -> forge_note_sent _ | forge_note_failed _

on forge_note_sent(next)
  return if next.channel_id != forge_item_channel
  forge_discussion_pending = ""
  forge_discussion_editor = editor("")
  error = ""

on forge_note_failed(cause)
  return if cause.scope_id != forge_item_channel
  forge_discussion_pending = ""
  error = cause.message

on forge_refreshed(next)
  return if next.generation != forge_generation
  forge_repos = keep_forge_repos(next.repos_loaded, next.repos, forge_repos)
  forge_list_phase = keep_forge_phase(next.repos_loaded, ForgePhase.ready, forge_list_phase)
  forge_branches = keep_branches(next.repo_loaded, next.branches, forge_branches)
  forge_items = keep_forge_items(next.repo_loaded, next.items, forge_items)
  forge_repo_phase = keep_forge_phase(next.repo_loaded, ForgePhase.ready, forge_repo_phase)
  forge_item_title = keep_str(next.item_loaded, next.item.title, forge_item_title)
  forge_item_state = keep_str(next.item_loaded, next.item.state, forge_item_state)
  forge_item_kind = keep_str(next.item_loaded, next.item.kind, forge_item_kind)
  forge_item_body = keep_str(next.item_loaded, next.item.body, forge_item_body)
  forge_item_author = keep_str(next.item_loaded, next.item.author_name, forge_item_author)
  forge_item_branches = keep_str(next.item_loaded, next.item.branches, forge_item_branches)
  forge_item_channel = keep_str(next.item_loaded, next.item.channel_id, forge_item_channel)
  forge_item_source_branch = keep_str(next.item_loaded, next.item.source_branch, forge_item_source_branch)
  // A staged comment anchors into ONE patch. These read the source head BEFORE
  // it is reassigned below, so a branch that moved under an open composer takes
  // its comments with it instead of being submitted as if they were written
  // against the new diff.
  error = staged_comment_drop_note(next.item_loaded, next.item.source_oid, forge_item_source_oid, forge_comment_staged, error)
  forge_comment_staged = keep_staged_comments(next.item_loaded, next.item.source_oid, forge_item_source_oid, forge_comment_staged)
  forge_comment_path = keep_comment_text(next.item_loaded, next.item.source_oid, forge_item_source_oid, forge_comment_path)
  forge_comment_draft = keep_comment_text(next.item_loaded, next.item.source_oid, forge_item_source_oid, forge_comment_draft)
  forge_item_source_oid = keep_str(next.item_loaded, next.item.source_oid, forge_item_source_oid)
  forge_item_target_oid = keep_str(next.item_loaded, next.item.target_oid, forge_item_target_oid)
  forge_item_merge_oid = keep_str(next.item_loaded, next.item.merge_oid, forge_item_merge_oid)
  forge_item_diff = keep_str(next.item_loaded, next.item.diff, forge_item_diff)
  forge_item_diff_truncated = keep_bool(next.item_loaded, next.item.diff_truncated, forge_item_diff_truncated)
  forge_item_files_changed = keep_i64(next.item_loaded, next.item.files_changed, forge_item_files_changed)
  forge_item_additions = keep_i64(next.item_loaded, next.item.additions, forge_item_additions)
  forge_item_deletions = keep_i64(next.item_loaded, next.item.deletions, forge_item_deletions)
  forge_item_reviews = keep_forge_reviews(next.item_loaded, next.item.reviews, forge_item_reviews)
  forge_item_approvals = keep_i64(next.item_loaded, next.item.approvals, forge_item_approvals)
  forge_item_change_requests = keep_i64(next.item_loaded, next.item.change_requests, forge_item_change_requests)
  forge_item_phase = keep_forge_phase(next.item_loaded, ForgePhase.ready, forge_item_phase)

// The breadcrumb home. Nothing else clears `forge_repo`, so without this the
// repo grid is unreachable for the rest of the session once a repo is opened.
on forge_close_repo
  // Closing retires each scoped request immediately; the generation bump is
  // the matching state guard if an already-delivered completion is queued.
  invalidate lane=forge_repo
  invalidate lane=forge_item
  invalidate lane=forge_discussion
  invalidate lane=forge_code
  forge_generation = forge_generation + 1
  forge_repo = ""
  forge_repo_phase = ForgePhase.idle
  forge_branches = []
  forge_items = []
  forge_repo_menu = false
  forge_item_number = 0
  forge_item_phase = ForgePhase.idle
  forge_item_diff = ""
  forge_item_channel = ""
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
  forge_merge_conflicts = []

on forge_toggle_repo_menu
  forge_repo_menu = !forge_repo_menu

on forge_close_item
  // Same retirement as the close above: cancel the scoped work and bump the
  // state guard before clearing the item it could otherwise reopen.
  invalidate lane=forge_item
  invalidate lane=forge_discussion
  forge_generation = forge_generation + 1
  forge_item_number = 0
  forge_item_phase = ForgePhase.idle
  forge_item_diff = ""
  forge_item_channel = ""
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
  forge_merge_conflicts = []

// Picking Code (re)reads the repo root: `forge_tree` lists ONE
// directory, so the browse always starts from a listing that belongs to the
// repo currently open, and a tree left over from the previous repo can never
// paint.
on select_forge_tab(tab)
  invalidate lane=forge_code
  forge_tab = tab
  return if tab != ForgeTab.code || !connected || empty(forge_repo)
  forge_tree_path = ""
  forge_tree_rev = ""
  forge_tree_entries = []
  forge_tree_truncated = false
  forge_file_path = ""
  forge_file_text = ""
  forge_file_binary = false
  forge_file_truncated = false
  forge_tree_born = false
  forge_code_phase = ForgeCodePhase.tree_loading
  run replace lane=forge_code forge_tree(connected_rpc, forge_repo, "", "") -> forge_tree_loaded _ | forge_tree_failed _

// A directory row NAVIGATES. `forge_tree` answers for one path, so there is no
// whole-tree read to expand in place against — the same shape the duckfs tree
// on the Files screen already has, and the reason every row sits at depth 0.
on forge_open_dir(path)
  return if !connected || empty(forge_repo)
  forge_tree_path = path
  forge_tree_entries = []
  forge_tree_truncated = false
  forge_file_path = ""
  forge_file_text = ""
  forge_file_binary = false
  forge_file_truncated = false
  forge_code_phase = ForgeCodePhase.tree_loading
  run replace lane=forge_code forge_tree(connected_rpc, forge_repo, forge_tree_rev, path) -> forge_tree_loaded _ | forge_tree_failed _

on forge_tree_loaded(next)
  let same_repo = next.repo == forge_repo
  let same_path = next.path == forge_tree_path
  let same_rev = empty(forge_tree_rev) || next.rev == forge_tree_rev
  return if !same_repo || !same_path || !same_rev
  forge_tree_repo = next.repo
  forge_tree_rev = next.rev
  forge_tree_path = next.path
  forge_tree_born = next.born
  forge_tree_entries = next.entries
  forge_tree_truncated = next.truncated
  forge_code_phase = ForgeCodePhase.ready
  error = ""

on forge_open_file(path)
  return if !connected || empty(forge_repo)
  forge_file_path = path
  forge_file_text = ""
  forge_code_phase = ForgeCodePhase.file_loading
  run replace lane=forge_code forge_blob(connected_rpc, forge_repo, forge_tree_rev, path) -> forge_blob_loaded _ | forge_file_failed _

on forge_blob_loaded(next)
  let same_repo = next.repo == forge_repo
  let same_rev = next.rev == forge_tree_rev
  let same_path = next.path == forge_file_path
  return if !same_repo || !same_rev || !same_path
  forge_file_path = next.path
  forge_file_text = next.text
  forge_file_binary = next.binary
  forge_file_truncated = next.truncated
  forge_code_phase = ForgeCodePhase.ready
  error = ""

on forge_tree_failed(cause)
  forge_code_phase = ForgeCodePhase.tree_failed
  error = cause.message

on forge_file_failed(cause)
  forge_code_phase = ForgeCodePhase.file_failed
  error = cause.message
