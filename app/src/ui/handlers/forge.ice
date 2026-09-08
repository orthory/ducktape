// FORGE — repos, the tracker, one item with its reviews, its merge and its
// discussion. The list/repo/item loaders key on `forge_generation`; the
// replace-lane discussion and code reads carry their semantic scope instead.
//
// The screen is the `forge` MODULE VIEW (crates/views/forge): everything
// below is reached from `forge_view_event`, one intent per act, and the
// facts go back as props (view.ice). The review body and the line comment
// being written are the view's; the app hears them on submit and reports
// which drafts an op consumed through `forge_drafts_cleared`.

// EVERY INTENT THE VIEW SENDS lands here, and the handler that signs the
// act stays where it was: a shared handler is reached through a flow, an
// act with nothing shared is signed in its arm.
on forge_view_event(event)
  match forge_intent(event)
    ForgeIntent.open_repo
      flow
        from done event_text(event, "name")
        done -> forge_open_repo _
    ForgeIntent.close_repo
      flow
        from done true
        done -> forge_close_repo()
    ForgeIntent.toggle_repo_menu
      forge_repo_menu = !forge_repo_menu
    ForgeIntent.tab
      flow
        from done forge_event_tab(event)
        done -> select_forge_tab _
    ForgeIntent.open_item
      flow
        from done event_number(event, "number")
        done -> forge_open_item _
    ForgeIntent.close_item
      flow
        from done true
        done -> forge_close_item()
    ForgeIntent.merge
      flow
        from done true
        done -> forge_merge_submit()
    ForgeIntent.review_pick
      forge_review_verdict = forge_event_verdict(event)
    ForgeIntent.review_submit
      flow
        from done event_text(event, "body")
        done -> forge_review_submit _
    // `stage_forge_comment` is the authority on what is stageable and on
    // replacing the comment already at this anchor; the view keeps the
    // obviously empty case off the wire.
    ForgeIntent.comment_stage
      forge_comment_staged = stage_forge_comment(forge_comment_staged, event_text(event, "path"), event_text(event, "line"), event_text(event, "side"), event_text(event, "body"))
    ForgeIntent.comment_drop
      forge_comment_staged = drop_forge_comment(forge_comment_staged, event_text(event, "anchor"))
    ForgeIntent.tree
      flow
        from done event_text(event, "path")
        done -> forge_open_dir _
    ForgeIntent.blob
      flow
        from done event_text(event, "path")
        done -> forge_open_blob _
    ForgeIntent.open_link
      flow
        from done event_text(event, "url")
        done -> open_message_link _
    ForgeIntent.composer
      let scope = event_text(event, "scope")
      run every duck_echo_str(event_text(event, "body")) -> forge_composer_event(scope, _) | external_url_failed _
    ForgeIntent.copy
      toast = event_text(event, "label")
      toast_age = 0
      task clipboard write event_text(event, "text")

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
  // THE PREVIOUS ITEM'S CHANNEL RETIRES HERE, not when the next item lands:
  // a note submitted for it that is still crossing the wire must find no
  // box on screen to post into, and go back to its own.
  forge_item_channel = ""
  forge_item_diff = ""
  forge_generation = forge_generation + 1
  forge_tab = ForgeTab.code
  // THE CODE BROWSE STARTS AT THE ROOT: the root listing answers with the
  // exact commit every nested read is then pinned to. The previous repo's
  // listing and file go before the read is issued.
  invalidate lane=forge_blob
  forge_tree_path = ""
  forge_tree_rev = ""
  forge_tree_entries = []
  forge_tree_born = false
  forge_tree_truncated = false
  forge_tree_phase = ForgeTreePhase.loading
  forge_file_path = ""
  forge_file_text = ""
  forge_file_note = ""
  forge_opened_dir = ""
  forge_opened_rev = ""
  forge_file_phase = ForgeFilePhase.idle
  parallel
    run replace lane=forge_repo load_forge_repo(connected_rpc, forge_repo, forge_generation) -> forge_repo_loaded _ | forge_repo_failed _
    run replace lane=forge_tree forge_tree(connected_rpc, forge_repo, "", "") -> forge_tree_loaded _ | forge_tree_failed _

on forge_repo_loaded(next)
  return if next.generation != forge_generation
  forge_repo = next.repo
  forge_repo_phase = ForgePhase.ready
  forge_branches = next.branches
  forge_items = next.items
  // A deep link's second step: the repo is open, now its item or its file.
  // A `duck://forge/<repo>/blob/<path>[@<rev>]` link first moves the tree
  // to the file's directory — pinned to `@rev` when the link names one,
  // else wherever the root listing lands — and `forge_tree_loaded` opens
  // the parked file under that tree's revision. One path whether or not a
  // tree had landed yet.
  match forge_focus_kind(forge_focus_number, forge_focus_path)
    ForgeFocus.idle
      return if true
    ForgeFocus.item
      let number = forge_focus_number
      forge_focus_number = 0
      run every duck_echo_i64(number) -> forge_open_item _ | external_url_failed _
    ForgeFocus.blob
      let rev = forge_focus_rev
      forge_focus_rev = ""
      forge_tree_path = forge_parent(forge_focus_path)
      forge_tree_rev = keep_str(!empty(rev), rev, forge_tree_rev)
      forge_tree_entries = []
      forge_tree_truncated = false
      forge_tree_phase = ForgeTreePhase.loading
      run replace lane=forge_tree forge_tree(connected_rpc, forge_repo, forge_tree_rev, forge_tree_path) -> forge_tree_loaded _ | forge_tree_failed _

on forge_repo_failed(cause)
  return if cause.generation != forge_generation
  forge_repo_phase = ForgePhase.failed
  error = cause.message

// A DIRECTORY ROW: the listing moves, pinned to the tree's commit. The
// file opened in the previous directory is retired by the move itself —
// `forge_file_header` names it only while the tree stands where it was
// opened.
on forge_open_dir(path)
  return if !connected || empty(forge_repo)
  forge_tree_path = path
  forge_tree_entries = []
  forge_tree_truncated = false
  forge_tree_phase = ForgeTreePhase.loading
  run replace lane=forge_tree forge_tree(connected_rpc, forge_repo, forge_tree_rev, path) -> forge_tree_loaded _ | forge_tree_failed _

// A listing answers for one repo, one directory and — once the root has
// pinned it — one commit; a superseded read landing late paints nothing.
on forge_tree_loaded(next)
  return if next.repo != forge_repo || next.path != forge_tree_path
  return if !empty(forge_tree_rev) && next.rev != forge_tree_rev
  forge_tree_rev = next.rev
  forge_tree_born = next.born
  forge_tree_entries = next.entries
  forge_tree_truncated = next.truncated
  forge_tree_phase = ForgeTreePhase.ready
  // A deep link's file opens under the tree it moved to, and only then.
  return if empty(forge_focus_path)
  let path = forge_focus_path
  forge_focus_path = ""
  flow
    from done path
    done -> forge_open_blob _

on forge_tree_failed(cause)
  forge_tree_phase = ForgeTreePhase.failed

// A FILE ROW. The read is pinned to the tree's commit and remembers the
// directory and revision it was opened under, so the view can retire the
// preview when either moves. The previous file's flags must not describe
// the one in flight: a stale `binary` would brand the next blob "not text"
// until its load settles. `network_chain_id` reaches the blob's reader: a
// Markdown blob's inline pictures are duck:// addresses too, and one naming
// another network must not draw THIS network's blob of the same name.
on forge_open_blob(path)
  return if !connected || empty(forge_repo)
  forge_opened_dir = forge_tree_path
  forge_opened_rev = forge_tree_rev
  forge_file_path = path
  forge_file_text = ""
  forge_file_binary = false
  forge_file_truncated = false
  forge_file_picture = false
  forge_file_note = ""
  forge_file_phase = ForgeFilePhase.loading
  run replace lane=forge_blob forge_blob(connected_rpc, forge_repo, forge_tree_rev, path, network_chain_id) -> forge_blob_loaded _ | forge_blob_failed _

// A BLOB ANSWERS FOR ONE FILE: a completion naming another path is a
// superseded read landing late, and painting it would put one file's
// source under another file's header.
on forge_blob_loaded(next)
  return if next.repo != forge_repo || next.path != forge_file_path
  forge_file_text = next.text
  forge_file_binary = next.binary
  forge_file_truncated = next.truncated
  forge_file_picture = next.picture
  forge_file_width = next.width
  forge_file_height = next.height
  forge_file_phase = ForgeFilePhase.ready

on forge_blob_failed(cause)
  forge_file_phase = ForgeFilePhase.failed
  forge_file_note = cause.message

on forge_open_item(number)
  return if !connected || empty(forge_repo)
  invalidate lane=forge_discussion
  forge_item_number = number
  // The highlight belongs to the item it was landed on; the one-shot
  // `forge_focus_seq` survives this open — it is THIS open's landing.
  forge_linked_note = none
  error = ""
  forge_item_phase = ForgePhase.loading
  // the previous item's channel retires now (see forge_open_repo)
  forge_item_channel = ""
  forge_review_verdict = ForgeReviewVerdict.comment
  // A staged comment anchors to THIS item's diff. Carrying one across items
  // would post it against a patch it was never written about — and the
  // view's review body and line comment go with it.
  forge_comment_staged = []
  forge_drafts_cleared = forge_drafts_cleared + 1
  forge_drafts_scope = "item"
  forge_merge_conflicts = []
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
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
  forge_item_blocks = next.blocks
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
  // A deep link's `#seq` lands here, once: the note is picked out of the list and
  // the view scrolls the item's page to that row — by its key in the
  // Discussion's keyed column, on the landing the tick counts (a seq the
  // list does not hold scrolls nowhere; the linked-note card above the list
  // still shows it).
  return if forge_focus_seq == 0
  let landed = forge_focus_seq
  forge_linked_note = linked_note(forge_discussion, landed)
  forge_focus_seq = 0
  forge_landed_seq = landed
  forge_landed_tick = forge_landed_tick + 1

on forge_discussion_failed(cause)
  error = cause.message

// The body is what the reader typed in the view; the staged comments ride
// with it, and the verdict is the one picked.
on forge_review_submit(body)
  return if !connected || forge_review_busy || empty(forge_repo) || forge_item_number <= 0
  forge_review_busy = true
  run every submit_forge_review(connected_rpc, password, forge_repo, forge_item_number, forge_review_verdict, body, forge_item_source_oid, forge_comment_staged) -> forge_review_submitted(connected_rpc, forge_repo, forge_item_number, _) | forge_review_failed(connected_rpc, forge_repo, forge_item_number, _)

// The staged comments went out INSIDE this review, so they are cleared with the
// body: the view is told its review draft and line comment were consumed. A
// failure keeps them — the whole submit is one transaction, and losing a page
// of written comments to a transient RPC error is not recoverable.
on forge_review_submitted(started_rpc, started_repo, started_number, _result)
  forge_review_busy = false
  return if started_rpc != connected_rpc || started_repo != forge_repo || started_number != forge_item_number
  forge_review_verdict = ForgeReviewVerdict.comment
  forge_comment_staged = []
  forge_drafts_cleared = forge_drafts_cleared + 1
  forge_drafts_scope = "review"
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

// The note's words live in the host's composer (`forge_composer`, the chat
// composer over the item's channel on this network as its scope); a send
// arrives here as the trimmed body and the scope it was written in. The
// composer cleared itself before it emitted, so a body the gate refuses —
// or one written for an item or a network the reader has since left — goes
// back to THAT box, never to the item on screen, and a failed send too.
//
// THE SEND CARRIES ITS OWN IDENTITY on both routes — the box it left from
// and its operation id — because the state fields move under it: opening
// another item clears `forge_discussion_pending` and a newer note may be
// in flight by the time this one answers. So a failure restores into the
// scope IT captured, and only the completion whose id is still the pending
// one clears the flag; a stale answer never clears a newer note's.
on forge_composer_event(scope, body)
  match submit_verdict(loading, connected, forge_item_channel, forge_discussion_pending, forge_item_number > 0 && forge_item_phase == ForgePhase.ready, scope, composer_scope(connected_rpc, forge_item_channel))
    SubmitVerdict.refused
      composer_stashed = chat_composer_unsent(scope, body, false)
    SubmitVerdict.admitted
      let op = fresh_operation_id("forge-note")
      forge_discussion_pending = op
      run every send_message(connected_rpc, password, forge_item_channel, op, trim(body), forge_discussion_members) -> forge_note_sent(op, _) | forge_note_failed(scope, op, _)

on forge_note_sent(op, next)
  return if op != forge_discussion_pending
  forge_discussion_pending = ""
  error = ""

on forge_note_failed(scope, op, cause)
  composer_stashed = chat_composer_unsent(scope, cause.body, cause.committed)
  return if op != forge_discussion_pending
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
  forge_item_blocks = keep_chat_blocks(next.item_loaded, next.item.blocks, forge_item_blocks)
  forge_item_author = keep_str(next.item_loaded, next.item.author_name, forge_item_author)
  forge_item_branches = keep_str(next.item_loaded, next.item.branches, forge_item_branches)
  forge_item_channel = keep_str(next.item_loaded, next.item.channel_id, forge_item_channel)
  forge_item_source_branch = keep_str(next.item_loaded, next.item.source_branch, forge_item_source_branch)
  // A staged comment anchors into ONE patch. These read the source head BEFORE
  // it is reassigned below, so a branch that moved under an open composer takes
  // its comments with it instead of being submitted as if they were written
  // against the new diff — and the line comment being written in the view
  // goes the same way.
  let branch_moved = forge_branch_moved(next.item_loaded, next.item.source_oid, forge_item_source_oid)
  error = staged_comment_drop_note(next.item_loaded, next.item.source_oid, forge_item_source_oid, forge_comment_staged, error)
  forge_comment_staged = keep_staged_comments(next.item_loaded, next.item.source_oid, forge_item_source_oid, forge_comment_staged)
  forge_drafts_cleared = keep_i64(branch_moved, forge_drafts_cleared + 1, forge_drafts_cleared)
  forge_drafts_scope = keep_str(branch_moved, "comment", forge_drafts_scope)
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
  invalidate lane=forge_tree
  invalidate lane=forge_blob
  forge_generation = forge_generation + 1
  forge_repo = ""
  forge_tree_path = ""
  forge_tree_rev = ""
  forge_tree_entries = []
  forge_tree_born = false
  forge_tree_truncated = false
  forge_tree_phase = ForgeTreePhase.loading
  forge_file_path = ""
  forge_file_text = ""
  forge_file_note = ""
  forge_opened_dir = ""
  forge_opened_rev = ""
  forge_file_phase = ForgeFilePhase.idle
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

on forge_close_item
  // Same retirement as the close above: cancel the scoped work and bump the
  // state guard before clearing the item it could otherwise reopen.
  invalidate lane=forge_item
  invalidate lane=forge_discussion
  forge_generation = forge_generation + 1
  forge_item_number = 0
  forge_linked_note = none
  forge_focus_seq = 0
  forge_item_phase = ForgePhase.idle
  forge_item_diff = ""
  forge_item_channel = ""
  forge_discussion = []
  forge_discussion_members = []
  forge_discussion_pending = ""
  forge_merge_conflicts = []

on select_forge_tab(tab)
  forge_tab = tab
