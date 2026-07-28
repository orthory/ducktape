// FORGE — repos, the tracker, one item with its reviews, its merge and its
// discussion. Every loader keys on `forge_generation`.

// Every row arrives carrying its own about line, language and `updated_at`.
// `updated_at` is UNIX SECONDS off the head commit's git committer time, so it
// renders with `relative_time(...)` — NOT with `height_label_short()` like the
// rest of this app. Every other record time here is a consensus stamp (the
// validator sets consensus_time = height), which is why heights print
// everywhere else; a git client wrote this one against a real wall clock.
on forge_loaded(next)
  return if next.generation != forge_generation
  forge_repos = next.repos

on forge_failed(cause)
  return if cause.generation != forge_generation

// Picking a repo also DISMISSES the switcher. Nothing else clears it on this
// route, so the popover stayed pinned over the first rows of the tracker list
// the user just navigated to, with the crumb as the only way out.
on forge_open_repo(name)
  return if !connected
  forge_repo_menu = false
  forge_repo = name
  forge_item_number = 0
  forge_item_diff = ""
  forge_generation = forge_generation + 1
  run load_forge_repo(connected_rpc, forge_repo, forge_generation) -> forge_repo_loaded _ | forge_failed _

on forge_repo_loaded(next)
  return if next.generation != forge_generation
  forge_repo = next.repo
  forge_branches = next.branches
  forge_items = next.items

on forge_open_item(number)
  return if !connected || empty(forge_repo)
  forge_item_number = number
  forge_review_verdict = "comment"
  forge_review_draft = ""
  forge_merge_conflicts = []
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
  forge_discussion_editor = editor("")
  forge_generation = forge_generation + 1
  run load_forge_item(connected_rpc, forge_repo, forge_item_number, forge_generation) -> forge_item_loaded _ | forge_failed _

on forge_item_loaded(next)
  return if next.generation != forge_generation
  forge_item_number = next.number
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
  forge_discussion_generation = forge_discussion_generation + 1
  return if empty(forge_item_channel)
  run load_forge_discussion(connected_rpc, forge_item_channel, forge_discussion_generation) -> forge_discussion_loaded _ | forge_discussion_failed _

on forge_discussion_loaded(next)
  return if next.generation != forge_discussion_generation || next.channel_id != forge_item_channel
  forge_discussion = next.messages
  forge_discussion_members = next.members

on forge_discussion_failed(cause)
  return if cause.generation != forge_discussion_generation
  error = cause.message

on forge_review_pick(verdict)
  forge_review_verdict = verdict

on forge_review_submit
  return if !connected || forge_review_busy || empty(forge_repo) || forge_item_number <= 0
  forge_review_busy = true
  run submit_forge_review(connected_rpc, password, forge_repo, forge_item_number, forge_review_verdict, forge_review_draft, forge_item_source_oid) -> forge_review_submitted _ | forge_review_failed _

on forge_review_submitted(_result)
  forge_review_busy = false
  forge_review_draft = ""
  forge_review_verdict = "comment"
  error = ""

on forge_review_failed(cause)
  forge_review_busy = false
  error = cause.message

on forge_merge_submit
  return if !connected || forge_merge_busy || empty(forge_repo) || forge_item_number <= 0
  forge_merge_busy = true
  forge_merge_conflicts = []
  run merge_forge_pr(connected_rpc, password, forge_repo, forge_item_number, forge_item_source_branch, forge_item_source_oid, forge_item_target_oid) -> forge_merged _ | forge_merge_failed _

on forge_merged(next)
  return if next.repo != forge_repo || next.number != forge_item_number
  forge_merge_busy = false
  forge_merge_conflicts = next.conflicts
  error = ""

on forge_merge_failed(cause)
  forge_merge_busy = false
  error = cause.message

on forge_note_submit
  return if loading || !connected || empty(forge_item_channel) || !empty(forge_discussion_pending) || empty(trim(editor_text(forge_discussion_editor)))
  forge_discussion_pending = fresh_operation_id("forge-note")
  run send_message(connected_rpc, password, forge_item_channel, forge_discussion_pending, trim(editor_text(forge_discussion_editor)), forge_discussion_members) -> forge_note_sent _ | forge_note_failed _

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
  forge_branches = keep_branches(next.repo_loaded, next.branches, forge_branches)
  forge_items = keep_forge_items(next.repo_loaded, next.items, forge_items)
  forge_item_title = keep_str(next.item_loaded, next.item.title, forge_item_title)
  forge_item_state = keep_str(next.item_loaded, next.item.state, forge_item_state)
  forge_item_kind = keep_str(next.item_loaded, next.item.kind, forge_item_kind)
  forge_item_body = keep_str(next.item_loaded, next.item.body, forge_item_body)
  forge_item_author = keep_str(next.item_loaded, next.item.author_name, forge_item_author)
  forge_item_branches = keep_str(next.item_loaded, next.item.branches, forge_item_branches)
  forge_item_channel = keep_str(next.item_loaded, next.item.channel_id, forge_item_channel)
  forge_item_source_branch = keep_str(next.item_loaded, next.item.source_branch, forge_item_source_branch)
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

// The breadcrumb home. Nothing else clears `forge_repo`, so without this the
// repo grid is unreachable for the rest of the session once a repo is opened.
on forge_close_repo
  forge_repo = ""
  forge_branches = []
  forge_items = []
  forge_repo_menu = false
  forge_item_number = 0
  forge_item_diff = ""
  forge_item_channel = ""
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
  forge_merge_conflicts = []

on forge_toggle_repo_menu
  forge_repo_menu = !forge_repo_menu

on forge_close_item
  forge_item_number = 0
  forge_item_diff = ""
  forge_item_channel = ""
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
  forge_merge_conflicts = []

// Held here beside the loaders that fill them.
state
  // The per-kind counts `search_workspace` already returns and the app threw
  // away. They are what the chip strip is drawn FROM, so the strip can only
  // ever name a kind the search actually ran — including Tasks, which has a
  // loader again.
  // THE FORGE CODE BROWSE. Same relocation note as the explorer block above:
  // these belong in state.ice and handlers/lifecycle.ice, and they sit here
  // because the components (`ForgeCodeTab` and the tree rows) landed with their
  // click targets named and nothing declaring them — a Code tab with no route
  // into it is the failure this pass exists to close.
  forge_tree_path = ""
  // WHICH REPO THE LISTING BELONGS TO. `forge_open_repo` lives in
  // handlers/lifecycle.ice and clears none of this, so without the stamp a
  // repo switch would leave another project's files painted under the new
  // breadcrumb — a wrong reading, not just a stale one.
  forge_tree_repo = ""
  forge_tree_entries:[TreeEntry] = []
  forge_file_path = ""
  forge_file_text = ""
  forge_file_binary = false
  forge_file_truncated = false
  forge_code_generation:i64 = 0
  // THE REGISTERED MODULE SET, same relocation note again. `load_modules` reads
  // /v1/status plus the lifecycle projection; it is only ever wanted while the
  // Modules tab is open, so the tab's own button is what pulls it.

// The forge tab bar's only act, same story: `forge_tab` was declared and never
// touched. Picking Code (re)reads the repo root: `forge_tree` lists ONE
// directory, so the browse always starts from a listing that belongs to the
// repo currently open, and a tree left over from the previous repo can never
// paint.
on select_forge_tab(tab)
  forge_tab = tab
  return if tab != "code" || !connected || empty(forge_repo)
  forge_code_generation = forge_code_generation + 1
  forge_tree_path = ""
  forge_tree_entries = []
  forge_file_path = ""
  forge_file_text = ""
  forge_file_binary = false
  forge_file_truncated = false
  run forge_tree(connected_rpc, forge_repo, "", "", forge_code_generation) -> forge_tree_loaded _ | forge_code_failed _

// A directory row NAVIGATES. `forge_tree` answers for one path, so there is no
// whole-tree read to expand in place against — the same shape the duckfs tree
// on the Files screen already has, and the reason every row sits at depth 0.
on forge_open_dir(path)
  return if !connected || empty(forge_repo)
  forge_code_generation = forge_code_generation + 1
  forge_tree_path = path
  forge_tree_entries = []
  run forge_tree(connected_rpc, forge_repo, "", path, forge_code_generation) -> forge_tree_loaded _ | forge_code_failed _

on forge_tree_loaded(next)
  return if next.generation != forge_code_generation
  forge_tree_repo = next.repo
  forge_tree_path = next.path
  forge_tree_entries = next.entries
  error = ""

on forge_open_file(path)
  return if !connected || empty(forge_repo)
  forge_code_generation = forge_code_generation + 1
  forge_file_path = path
  forge_file_text = ""
  run forge_blob(connected_rpc, forge_repo, "", path, forge_code_generation) -> forge_blob_loaded _ | forge_code_failed _

on forge_blob_loaded(next)
  return if next.generation != forge_code_generation
  forge_file_path = next.path
  forge_file_text = next.text
  forge_file_binary = next.binary
  forge_file_truncated = next.truncated
  error = ""

on forge_code_failed(cause)
  return if cause.generation != forge_code_generation
  error = cause.message
