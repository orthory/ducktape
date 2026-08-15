// FORGE — the repo overview, one repo's Code/Pull requests/Issues seats, and
// the item detail with its merge box, reviews and discussion. See
// `screens/roster.ice` for the screen contract.
//
// The `forge_item_*` props keep their app names on purpose: the detail half is
// one family, and the guards in main.rs name several of its members verbatim.
// Everything outside that family drops the redundant `forge_` prefix.

enum ForgeFilePhase
  idle
  loading
  ready
  failed

component ForgeScreen(org:str, about:str, tier:str, connected_rpc:str, repos:[ForgeRepo], list_phase:ForgePhase, open_repo:str, repo_menu:bool, repo_phase:ForgePhase, branches:[str], tab:ForgeTab, items:[ForgeItem], tree_repo:str, tree_rev:str, tree_path:str, tree_born:bool, tree_entries:[TreeEntry], tree_truncated:bool, code_phase:ForgeCodePhase, forge_item_number:i64, item_phase:ForgePhase, forge_item_kind:str, forge_item_title:str, forge_item_state:str, forge_item_author:str, forge_item_branches:str, forge_item_body:str, forge_item_files_changed:i64, forge_item_additions:i64, forge_item_deletions:i64, forge_item_diff:str, forge_item_diff_truncated:bool, forge_item_merge_oid:str, forge_item_source_oid:str, forge_item_channel:str, forge_item_approvals:i64, forge_item_change_requests:i64, forge_item_reviews:[ForgeReview], merge_conflicts:[str], merge_busy:bool, review_verdict:ForgeReviewVerdict, bind review_draft:str, review_busy:bool, comment_target:str, bind comment_draft:str, staged_comments:[ForgeDraftComment], discussion:[ChatMessage], bind discussion_editor:editor, discussion_pending:str, connected:bool, loading:bool, dark:bool)
  emits
    forge_open_repo(str)
    forge_close_repo()
    forge_toggle_repo_menu()
    select_forge_tab(ForgeTab)
    forge_open_dir(str)
    forge_open_item(i64)
    forge_close_item()
    forge_merge_submit()
    forge_review_pick(ForgeReviewVerdict)
    forge_review_submit()
    forge_comment_open(str, str, str)
    forge_comment_stage()
    forge_comment_cancel()
    forge_comment_drop(str)
    note_composer_event(ComposerEvent)
    open_message_link(str)
  col w=fill h=fill
    // NOT CONNECTED IS NOT EMPTY, and the arm sits ABOVE both seats because
    // both of them read. `connected` already disabled every act here, while the
    // overview went on plating "No repos yet" — handing out a push command — off
    // a forge nobody queried, and an open repo went on showing empty Code / Pull
    // requests / Issues lists. Same words Chat and Pages use.
    if !connected
      box
        with
          w=fill
          h=fill
          p=22.0
        EmptyState
          with
            title="Not connected"
            description="Click the network name in the titlebar to pick or reconnect a network."
    // THE REPO OVERVIEW. Reachable again: `forge_close_repo` clears the open
    // repo, which nothing did before — once a repo was opened the grid was
    // gone for the rest of the session.
    if connected && empty(open_repo)
      scroll
        with
          dir=vertical
          w=fill
          h=fill
        col
          with
            w=fill
            p=16.0
            gap=14.0
          ForgeOrgHeader
            with
              org
              about
              repos=len(repos)
              tier
              answered=(list_phase == ForgePhase.ready)
          if empty(repos) && list_phase == ForgePhase.loading
            box w=fill p=30.0 align-x=center
              text "Loading repositories…" size=13.0 @text-meta
          if empty(repos) && list_phase == ForgePhase.failed
            box w=fill p=30.0 align-x=center
              text "Could not load repositories. Reopen Forge to try again." size=13.0 @text-meta
          // NOT `EmptyPlate`: this screen has no "new repository" button and
          // never will, because forge IS a git remote — a repo comes into
          // existence when a push lands on it. Saying only that a repo
          // "appears here once it is created" and naming no way to create one
          // is a dead end, so the plate carries the command with this
          // workspace's own endpoint already in it.
          if empty(repos) && list_phase == ForgePhase.ready
            box
              with
                w=fill
                p=30.0
                align-x=center
                bg=transparent
                border=border
                border-w=1.0
                r=12.0
              col
                with
                  gap=10.0
                  align=center
                text "No repos yet. Forge is a git remote — a repo appears when a push lands on it."
                  with
                    size=13.0
                    @text-meta
                box
                  with
                    px=12.0
                    py=8.0
                    bg=muted_bg
                    border=border
                    border-w=1.0
                    r=8.0
                  text forge_push_command(connected_rpc)
                    with
                      size=12.0
                      wrap=word-or-glyph
                      font=code
                      @text-accent_fg
          if !empty(repos)
            grid min-cell=320.0 gap=10.0
              for repo in repos
                RepoCard repo=repo
                  forward
                    forge_open_repo
    if connected && !empty(open_repo)
      col w=fill h=fill
        box
          with
            w=fill
            pl=16.0
            pr=16.0
            pt=8.0
            pb=8.0
          stack w=fill
            row
              with
                w=fill
                gap=9.0
                align=center
              box
                with
                  w=fill
                  clip=true
                button -> emit(forge_toggle_repo_menu)
                  with
                    label="Switch repository"
                    expanded=repo_menu
                    w=fill
                    p=0.0
                    @ghost_action
                  RepoCrumb
                    with
                      org
                      repo=open_repo
                      branch=""
                      open=repo_menu
                  active bg=transparent text=fg border=transparent border-w=1.0 r=9.0
                  hovered bg=row_hover text=fg
                  pressed bg=elevated text=fg
              // Detail navigation belongs in the persistent repo bar. Keeping
              // it in the scrolling body spent a whole row on a control that
              // should remain available while a long diff is being read.
              if forge_item_number > 0 && item_phase == ForgePhase.ready
                BackToList kind=forge_item_kind
                  forward
                    forge_close_item
              if forge_item_number > 0 && item_phase != ForgePhase.ready
                button "Back to tracker" -> emit(forge_close_item)
                  with
                    h=28.0
                    p=6.0
                    @secondary_action
              button "All repos" -> emit(forge_close_repo)
                with
                  h=28.0
                  p=6.0
                  @secondary_action
            if repo_menu
              pin x=0.0 y=36.0
                Popover width=290.0
                  col w=fill gap=1.0
                    for repo in repos
                      RepoMenuRow repo=repo active=(repo.name == open_repo)
                        forward
                          forge_open_repo
        box
          with
            w=fill
            h=1.0
            bg=separator
          space w=1.0 h=1.0
        if forge_item_number <= 0
          col w=fill h=fill
            // THE TAB BAR THE TRACKER NEVER GOT. `forge_tab` has sat in
            // state/forge.ice and `filter_forge_items`/`forge_open_count` in
            // backend.ice since wave 1 with no call site at all, so the
            // screen piled merged PRs and closed issues into one flat
            // list. The counts are OPEN work — a PR counts until it
            // merges, an issue until it closes.
            box
              with
                w=fill
                pl=16.0
                pr=16.0
              row
                with
                  w=fill
                  gap=18.0
                  align=center
                button -> emit(select_forge_tab, ForgeTab.code)
                  with
                    label="Browse the code"
                    checked=(tab == ForgeTab.code)
                    p=0.0
                    @ghost_action
                  TabLabel
                    with
                      label="Code"
                      count=0
                      active=(tab == ForgeTab.code)
                  active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
                  hovered bg=transparent text=fg
                  pressed bg=transparent text=fg
                button -> emit(select_forge_tab, ForgeTab.pulls)
                  with
                    label="Show pull requests"
                    checked=(tab == ForgeTab.pulls)
                    p=0.0
                    @ghost_action
                  TabLabel
                    with
                      label="Pull requests"
                      count=forge_open_count(items, "pr")
                      active=(tab == ForgeTab.pulls)
                  active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
                  hovered bg=transparent text=fg
                  pressed bg=transparent text=fg
                button -> emit(select_forge_tab, ForgeTab.issues)
                  with
                    label="Show issues"
                    checked=(tab == ForgeTab.issues)
                    p=0.0
                    @ghost_action
                  TabLabel
                    with
                      label="Issues"
                      count=forge_open_count(items, "issue")
                      active=(tab == ForgeTab.issues)
                  active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
                  hovered bg=transparent text=fg
                  pressed bg=transparent text=fg
                // Branches are context for every repo seat, not a separate
                // destination. Let them spend the remaining tab-bar width and
                // scroll horizontally instead of charging the content a row.
                if !empty(branches)
                  box
                    with
                      w=1.0
                      h=18.0
                      bg=separator
                    space w=1.0 h=1.0
                  scroll
                    with
                      dir=horizontal
                      w=fill
                      h=22.0
                      bar=hidden
                    row
                      with
                        h=fill
                        gap=4.0
                        align=center
                      for branch in branches
                        box
                          with
                            h=20.0
                            pl=7.0
                            pr=7.0
                            align-y=center
                            bg=surface
                            border=border
                            border-w=1.0
                            r=10.0
                          text branch
                            with
                              size=9.0
                              wrap=none
                              font=code_semibold
                              @text-meta
                if empty(branches)
                  space w=fill
            box
              with
                w=fill
                h=1.0
                bg=separator
              space w=1.0 h=1.0
            // One discriminant, one match: Code browses the tree, the
            // other two seats are the same tracker list under different
            // filters.
            match tab
              ForgeTab.code
                ForgeCodeBrowser #code(open_repo)
                  with
                    connected_rpc
                    connected
                    repo=open_repo
                    tree_repo
                    tree_rev
                    tree_path
                    tree_born
                    tree_entries
                    tree_truncated
                    code_phase
                    dark
                  forward
                    forge_open_dir
                    open_message_link
              ForgeTab.issues
                ForgeTrackerList
                  with
                    phase=repo_phase
                    items=filter_forge_items(items, ForgeTab.issues)
                    empty_message="No issues — this app reads the tracker but cannot open one yet."
                  forward
                    forge_open_item
              ForgeTab.pulls
                ForgeTrackerList
                  with
                    phase=repo_phase
                    items=filter_forge_items(items, ForgeTab.pulls)
                    empty_message="No pull requests — an agent run opens one when it delivers its work."
                  forward
                    forge_open_item
            // NO GATE NOTE HERE. `ForgeGateNote` told a resident the
            // node refuses their merge; `ForgeMsg::MergePr` authorizes
            // on `author_from_origin` alone, so the write succeeds and
            // the plate described an enforcement that does not exist.
            // The one true sentence about it lives beside the merge
            // button, where the decision is made.
        if forge_item_number > 0 && item_phase == ForgePhase.loading
          box w=fill h=fill p=16.0
            EmptyPlate message="Loading tracker item…"
        if forge_item_number > 0 && item_phase == ForgePhase.failed
          box w=fill h=fill p=16.0
            EmptyPlate message="Could not load this item. Go back and open it again to retry."
        if forge_item_number > 0 && item_phase == ForgePhase.ready
          scroll
            with
              dir=vertical
              w=fill
              h=fill
            col
              with
                w=fill
                pl=18.0
                pr=18.0
                pt=14.0
                pb=18.0
                gap=12.0
              row
                with
                  w=fill
                  gap=9.0
                  align=center
                box
                  with
                    w=fill
                    clip=true
                  text forge_item_title
                    with
                      size=16.0
                      wrap=none
                      font=display
                      @text-primary
                if forge_item_kind == "pr"
                  PrStatePill state=forge_item_state
                if forge_item_kind != "pr"
                  StatusBadge label=forge_item_state
              row
                with
                  w=fill
                  gap=10.0
                  align=center
                if !empty(forge_item_author)
                  box
                    with
                      max-w=160.0
                      clip=true
                    text forge_item_author
                      with
                        size=11.0
                        wrap=none
                        font=code_medium
                        @text-meta
                if !empty(forge_item_branches)
                  box
                    with
                      w=fill
                      clip=true
                    text forge_item_branches
                      with
                        size=12.0
                        wrap=none
                        font=code
                        @text-meta
                if empty(forge_item_branches)
                  space w=fill
                if forge_item_files_changed > 0
                  DiffCount
                    with
                      additions=forge_item_additions
                      deletions=forge_item_deletions
                      files=forge_item_files_changed
              if !empty(forge_item_body)
                IssueBodyCard author=forge_item_author body=forge_item_body
              if !empty(forge_item_diff)
                col w=fill gap=6.0
                  if forge_item_diff_truncated
                    text "Patch truncated — the counts above cover the whole diff."
                      with
                        size=12.5
                        @text-caption
                  // The pane's header names the patch's own branch pair:
                  // `forge_item_diff` is the WHOLE unified patch, and its
                  // per-file headers ride inside it as `file` diff rows.
                  DiffPane
                    with
                      file=forge_item_branches
                      additions=forge_item_additions
                      deletions=forge_item_deletions
                      lines=diff_lines(forge_item_diff)
                    forward
                      forge_comment_open
              if forge_item_kind == "pr"
                col w=fill gap=9.0
                  GroupLabel label="MERGE"
                  if forge_item_state == "merged"
                    MergedBanner note=forge_merge_note(forge_item_merge_oid, forge_item_branches)
                  if forge_item_state == "closed"
                    text "Closed without merging." size=12.5 @text-caption
                  if forge_item_state == "open"
                    col w=fill gap=9.0
                      if !empty(merge_conflicts)
                        col w=fill gap=3.0
                          text "Merge conflicts — resolve on the branch and push again:"
                            with
                              size=12.5
                              @text-caption
                          for conflict_path in merge_conflicts
                            text conflict_path
                              with
                                size=12.0
                                font=code
                                @text-fg
                      MergeAdvisory change_requests=forge_item_change_requests
                      row
                        with
                          w=fill
                          gap=10.0
                          align=center
                        MergeButton
                          with
                            busy=merge_busy
                            disabled=(!connected || empty(forge_item_source_oid))
                          forward
                            forge_merge_submit
                        // The tally belongs where the decision is made,
                        // and it is loaded already. The sentence beside
                        // it is the whole permission model this module
                        // has: none. Approvals never block a merge.
                        text forge_item_approvals
                          with
                            size=12.0
                            wrap=none
                            font=code_medium
                            @text-meta
                        text "approvals"
                          with
                            size=12.5
                            wrap=none
                            @text-caption
                        // The blanket sentence is the NO-ADVISORY half only.
                        // MergeAdvisory above already says "a reviewer
                        // requested changes — merge not recommended", and
                        // printing both left the box asserting that nothing
                        // gates the merge directly under a line recommending
                        // against it.
                        if forge_item_change_requests <= 0
                          text "Approvals are advisory — merging is never gated."
                            with
                              w=fill
                              size=12.5
                              @text-caption
              if forge_item_kind == "pr"
                col w=fill gap=9.0
                  GroupLabel label="REVIEWS"
                  if empty(forge_item_reviews)
                    text "No reviews yet." size=12.5 @text-caption
                  for review in forge_item_reviews
                    ReviewCard review=review
                  row
                    with
                      w=fill
                      gap=6.0
                      align=center
                    button -> emit(forge_review_pick, ForgeReviewVerdict.comment)
                      with
                        label="Pick comment verdict"
                        checked=(review_verdict == ForgeReviewVerdict.comment)
                        h=24.0
                        p=5.0
                        @ghost_action
                      text verdict_pick_label(review_verdict, ForgeReviewVerdict.comment, "Comment") size=13.0
                      active bg=surface text=fg border=card_line border-w=1.0 r=7.0
                      hovered bg=elevated text=fg
                      pressed bg=subtle text=fg
                    button -> emit(forge_review_pick, ForgeReviewVerdict.approve)
                      with
                        label="Pick approve verdict"
                        checked=(review_verdict == ForgeReviewVerdict.approve)
                        h=24.0
                        p=5.0
                        @ghost_action
                      text verdict_pick_label(review_verdict, ForgeReviewVerdict.approve, "Approve") size=13.0
                      active bg=final_bg text=fg border=final_line border-w=1.0 r=7.0
                      hovered bg=success_bg text=fg
                      pressed bg=success_bg text=fg
                    button -> emit(forge_review_pick, ForgeReviewVerdict.request_changes)
                      with
                        label="Pick request-changes verdict"
                        checked=(review_verdict == ForgeReviewVerdict.request_changes)
                        h=24.0
                        p=5.0
                        @ghost_action
                      text verdict_pick_label(review_verdict, ForgeReviewVerdict.request_changes, "Request changes")
                        with
                          size=13.0
                      active bg=alert_bg text=fg border=alert_line border-w=1.0 r=7.0
                      hovered bg=danger_zone_bg text=fg
                      pressed bg=danger_zone_bg text=fg
                    space w=fill
                  // THE LINE-COMMENT COMPOSER, open only while a diff gutter has
                  // picked a line. It lives beside the review body rather than
                  // inline in the patch because the two go out in ONE
                  // transaction — a line comment is part of a review here, not
                  // a standalone post — and the writer should see both drafts
                  // at once before submitting.
                  if !empty(comment_target)
                    col w=fill gap=6.0
                      row
                        with
                          w=fill
                          gap=7.0
                          align=center
                        text comment_target
                          with
                            w=fill
                            size=11.0
                            wrap=none
                            font=code_medium
                            @text-brand
                        button "Cancel" -> emit(forge_comment_cancel)
                          with
                            h=24.0
                            p=5.0
                            @secondary_action
                      row
                        with
                          w=fill
                          gap=6.0
                          align=center
                        input "" #forge-comment-body <-> comment_draft
                          with
                            label="Line comment"
                            hint="Comment on this line…"
                            disabled=(review_busy || !connected)
                            submit=emit(forge_comment_stage)
                            w=fill
                            p=6.2
                            text-size=13.0
                            line-h=1.2
                            @control
                          active bg=surface value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                          hovered bg=muted_bg border=control_line
                          disabled bg=muted_bg/54 value=muted
                        button "Add comment" -> emit(forge_comment_stage)
                          with
                            disabled=(review_busy || !connected || empty(comment_draft) || forge_comment_cap_reached(staged_comments))
                            h=28.0
                            p=6.0
                            @secondary_action
                  if forge_comment_cap_reached(staged_comments)
                    text "Comment limit reached for one review — submit this review, then start another."
                      with
                        size=12.5
                        @text-caption
                  // Staged and not yet sent. These ride out INSIDE the review
                  // below, so they are shown as part of its draft, not as
                  // posted comments.
                  if !empty(staged_comments)
                    col w=fill gap=4.0
                      for staged in staged_comments
                        row
                          with
                            w=fill
                            gap=7.0
                            align=center
                          box
                            with
                              w=fill
                              pl=11.0
                              pr=11.0
                              pt=9.0
                              pb=9.0
                              bg=brand_wash
                              border=brand_line
                              border-w=1.0
                              r=9.0
                            col w=fill gap=4.0
                              row
                                with
                                  w=fill
                                  gap=7.0
                                  align=center
                                text staged.anchor
                                  with
                                    w=fill
                                    size=11.0
                                    wrap=none
                                    font=code_medium
                                    @text-brand
                                text "not sent yet"
                                  with
                                    size=10.0
                                    wrap=none
                                    font=code_semibold
                                    @text-label
                              text staged.body
                                with
                                  w=fill
                                  size=12.0
                                  line-h=1.55
                                  @text-accent_fg
                          button "Remove" -> emit(forge_comment_drop, staged.anchor)
                            with
                              label="Remove staged comment"
                              h=24.0
                              p=5.0
                              @secondary_action
                  row
                    with
                      w=fill
                      gap=6.0
                      align=center
                    input "" #forge-review-body <-> review_draft
                      with
                        label="Review body"
                        hint="Leave a review…"
                        disabled=(review_busy || !connected)
                        submit=emit(forge_review_submit)
                        w=fill
                        p=6.2
                        text-size=13.0
                        line-h=1.2
                        @control
                      active bg=surface value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                      hovered bg=muted_bg border=control_line
                      disabled bg=muted_bg/54 value=muted
                    // The module refuses a review that is empty on BOTH halves,
                    // so the button refuses it first rather than spending a
                    // round trip to be told.
                    button "Submit review" -> emit(forge_review_submit)
                      with
                        disabled=(review_busy || !connected || empty(forge_item_source_oid) || (empty(review_draft) && empty(staged_comments)))
                        h=28.0
                        p=6.0
                        @primary_action
              col w=fill gap=9.0
                GroupLabel label="DISCUSSION"
                if empty(discussion)
                  text "No discussion yet." size=12.5 @text-caption
                keyed message in discussion by=message.seq virtual-row=44.0 w=fill gap=9.0
                  // A note is a pure function of its message, so the whole row
                  // caches under the same (seq, render_rev) key the chat
                  // stream's quiet arm uses — the live delta fold
                  // (`fold_live_chat` in lifecycle.ice) bumps `render_rev`
                  // on every in-place mutation, and a resync's replacement rows
                  // arrive content-seeded.
                  lazy message by message.seq, message.render_rev as cached_note
                    row
                      with
                        w=fill
                        gap=9.0
                        align=start
                      MessageAvatar initials=cached_note.initial kind=cached_note.avatar_kind
                      col w=fill gap=2.0
                        row
                          with
                            w=fill
                            gap=7.0
                            align=center
                          text cached_note.author
                            with
                              size=13.0
                              wrap=none
                              font=display
                              @text-fg
                          text cached_note.meta
                            with
                              size=11.0
                              wrap=none
                              font=code_medium
                              @text-meta
                          space w=fill
                        MessageBody message=cached_note
                          forward
                            open_message_link
                flex
                  with
                    w=fill
                    gap=8.0
                    items=end
                  box
                    with
                      w=fill
                      bg=surface
                      border=card_line
                      border-w=1.0
                      r=8.0
                      clip=true
                    extern rich_composer(discussion_editor, "Write a note…", (loading || !connected || empty(forge_item_channel)), 38.0, 120.0, 6.0) #forge-note -> emit(note_composer_event, _)
                  button "Send" -> emit(note_composer_event, composer_submit_event())
                    with
                      disabled=(loading || !connected || empty(forge_item_channel) || !empty(discussion_pending) || empty(trim(editor_text(discussion_editor))))
                      w=60.0
                      h=28.0
                      p=6.0
                      @primary_action

// THE FILE READER OWNS ITS OWN CYCLE. A file click routes to a local
// handler, the blob lands in component state, and no app handler clears
// any of it. The call site keys the instance by repository, and mounted
// lifetime prunes it — state, in-flight lane and all — when the reader
// leaves the Code tab or the repo: exactly the retirement
// `select_forge_tab` used to perform by hand. Within a stay, the preview
// is gated on the directory AND revision it was opened under
// (`forge_file_header`), so navigating away or a tree that reloaded at a
// newer commit retires it by moving the ground under it, and returning to
// the same directory at the same revision honestly resurfaces it.
component ForgeCodeBrowser(connected_rpc:str, connected:bool, repo:str, tree_repo:str, tree_rev:str, tree_path:str, tree_born:bool, tree_entries:[TreeEntry], tree_truncated:bool, code_phase:ForgeCodePhase, dark:bool)
  emits
    forge_open_dir(str)
    open_message_link(str)
  lifetime mounted
  state
    file_path = ""
    file_text = ""
    file_binary = false
    file_truncated = false
    failed_note = ""
    opened_dir = ""
    opened_rev = ""
    phase:ForgeFilePhase = ForgeFilePhase.idle
  on open_file(rpc, online, repo_now, rev, dir, path)
    return if !online || empty(repo_now)
    opened_dir = dir
    opened_rev = rev
    file_path = path
    file_text = ""
    // the previous file's flags must not describe the one in flight: a stale
    // `binary` would brand the next blob "not text" until its load settles.
    file_binary = false
    file_truncated = false
    failed_note = ""
    phase = ForgeFilePhase.loading
    run replace lane=blob forge_blob(rpc, repo_now, rev, path) -> file_loaded _ | file_failed _
  on file_loaded(next)
    return if next.path != file_path
    file_text = next.text
    file_binary = next.binary
    file_truncated = next.truncated
    phase = ForgeFilePhase.ready
  on file_failed(cause)
    phase = ForgeFilePhase.failed
    failed_note = cause.message
  // The header carries the path from the repo root and
  // NOTHING ELSE. The artifact prefixes the repo name;
  // the breadcrumb directly above already says it, and
  // Ice has no string concatenation to join the two.
  // `message` / `author` / `stamp` are the last commit
  // under this path — a future server log query could answer
  // that, and until it does the three slots stay
  // empty rather than printing a middot run around values
  // nobody read. ForgeCodeHeader drops each empty slot by
  // construction.
  ForgeCodeTab
    with
      path=forge_file_header(opened_dir, opened_rev, tree_path, tree_rev, file_path)
      message=""
      author=""
      stamp=""
    files:
      // Loading, failure and a truly empty tree are different
      // facts. In particular, an in-flight tree query must never
      // paint "nothing committed" before its answer arrives.
      col w=fill
        if code_phase == ForgeCodePhase.tree_loading
          box
            with
              w=fill
              pl=16.0
              pr=16.0
              pt=8.0
            text "Loading repository files…"
              with
                w=fill
                size=11.5
                line-h=1.5
                @text-label
        if code_phase == ForgeCodePhase.tree_failed
          box w=fill pl=16.0 pr=16.0 pt=8.0
            text "Could not load code. Pick Code to try again."
              with
                w=fill
                size=11.5
                line-h=1.5
                @text-label
        if tree_repo == repo && code_phase != ForgeCodePhase.tree_loading && code_phase != ForgeCodePhase.tree_failed
          col w=fill
            if empty(tree_entries) && !tree_born
              box w=fill pl=16.0 pr=16.0 pt=8.0
                text "Nothing committed on this repository yet."
                  with
                    w=fill
                    size=11.5
                    line-h=1.5
                    @text-label
            if empty(tree_entries) && tree_born && !tree_truncated
              box w=fill pl=16.0 pr=16.0 pt=8.0
                text "No files in this commit."
                  with
                    w=fill
                    size=11.5
                    line-h=1.5
                    @text-label
            if empty(tree_entries) && tree_born && tree_truncated
              box w=fill pl=16.0 pr=16.0 pt=8.0
                text "This directory has entries that cannot be shown."
                  with
                    w=fill
                    size=11.5
                    line-h=1.5
                    @text-label
            if !empty(tree_path)
              button -> emit(forge_open_dir, "")
                with
                  label="Back to the repository root"
                  w=fill
                  p=0.0
                  @icon_action
                ForgeTreeDirRow
                  with
                    name="/"
                    depth=0.0
                    open=true
                active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
                hovered bg=rail_hover text=fg
                pressed bg=elevated text=fg
            for entry in tree_entries
              col w=fill
                if entry.kind == "dir"
                  button -> emit(forge_open_dir, entry.path)
                    with
                      label="Open directory"
                      description=entry.path
                      w=fill
                      p=0.0
                      @icon_action
                    ForgeTreeDirRow
                      with
                        name=entry.name
                        depth=0.0
                        open=false
                    active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
                    hovered bg=rail_hover text=fg
                    pressed bg=elevated text=fg
                if entry.kind != "dir"
                  button -> open_file(connected_rpc, connected, repo, tree_rev, tree_path, entry.path)
                    with
                      label="Open file"
                      description=entry.path
                      w=fill
                      p=0.0
                      @icon_action
                    ForgeTreeFileRow
                      with
                        name=entry.name
                        depth=0.0
                        selected=(entry.path == file_path && opened_dir == tree_path && opened_rev == tree_rev)
                    active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
                    hovered bg=rail_hover text=fg
                    pressed bg=elevated text=fg
            if tree_truncated
              box w=fill p=12.0
                text "Some entries are not shown."
                  with
                    w=fill
                    size=11.5
                    line-h=1.5
                    @text-label
    source:
      // The reader's states, each with its own true reason.
      col w=fill
        if code_phase == ForgeCodePhase.tree_loading
          ForgeCodeEmpty name="" note="Loading repository files…"
        if phase == ForgeFilePhase.loading && !empty(forge_file_header(opened_dir, opened_rev, tree_path, tree_rev, file_path))
          ForgeCodeEmpty name=file_path note="Loading file…"
        if code_phase == ForgeCodePhase.tree_failed
          ForgeCodeEmpty name="" note="Could not load code. Pick Code to try again."
        if phase == ForgeFilePhase.failed && !empty(forge_file_header(opened_dir, opened_rev, tree_path, tree_rev, file_path))
          ForgeCodeEmpty name=file_path note=failed_note
        if code_phase == ForgeCodePhase.ready && empty(forge_file_header(opened_dir, opened_rev, tree_path, tree_rev, file_path)) && empty(tree_entries) && !tree_born
          ForgeCodeEmpty name="" note="Nothing is committed on this repository yet, so there is no file to read."
        if code_phase == ForgeCodePhase.ready && empty(forge_file_header(opened_dir, opened_rev, tree_path, tree_rev, file_path)) && empty(tree_entries) && tree_born && !tree_truncated
          ForgeCodeEmpty name="" note="This commit has no files to read."
        if code_phase == ForgeCodePhase.ready && empty(forge_file_header(opened_dir, opened_rev, tree_path, tree_rev, file_path)) && empty(tree_entries) && tree_born && tree_truncated
          ForgeCodeEmpty name="" note="This directory has entries outside the browser's display limits."
        if code_phase == ForgeCodePhase.ready && empty(forge_file_header(opened_dir, opened_rev, tree_path, tree_rev, file_path)) && !empty(tree_entries)
          ForgeCodeEmpty name="" note="Pick a file from the tree to read it."
        if phase == ForgeFilePhase.ready && !empty(forge_file_header(opened_dir, opened_rev, tree_path, tree_rev, file_path)) && file_binary
          ForgeCodeEmpty
            with
              name=file_path
              note="This is not text — the reader shows no preview for it."
        // A MARKDOWN BLOB READS AS A DOCUMENT, not a line
        // listing: a README is the first file every repo page
        // opens, and the shell's agent answers already ship
        // the full iced-markdown adapter (`agent_markdown`) —
        // this reuses it verbatim. Links route through the
        // same `open_message_link` seam the discussion's
        // messages already use. Markdown-vs-code is the
        // path's call (`markdown_path`) because the wire only
        // says binary-or-text.
        if phase == ForgeFilePhase.ready && !empty(forge_file_header(opened_dir, opened_rev, tree_path, tree_rev, file_path)) && !file_binary && markdown_path(file_path)
          col
            with
              w=fill
              px=16.0
              pt=13.0
              pb=13.0
              gap=9.0
            extern agent_markdown(file_text, dark) #forge-markdown -> emit(open_message_link, _)
            if file_truncated
              text "This file is larger than the 64 KiB preview limit."
                with
                  size=11.5
                  wrap=none
                  @text-label
        if phase == ForgeFilePhase.ready && !empty(forge_file_header(opened_dir, opened_rev, tree_path, tree_rev, file_path)) && !file_binary && !markdown_path(file_path)
          col
            with
              w=fill
              pt=13.0
              pb=13.0
              gap=9.0
            // HIGHLIGHTED, NUMBERED LINES. Token colour needs
            // per-span inks, which Ice's named-token text
            // nodes cannot carry, so the whole surface — the
            // gutter and the syntect-coloured code — renders
            // in the backend extern (the `agent_markdown`
            // idiom). The language is the path's call
            // (`code_token`); an unknown extension degrades
            // to plain text in one ink, the reading this
            // viewer shipped with. Metrics stay pinned to
            // DiffRow's by the shape lint in app/src/tests.rs.
            //
            // THE MEMO BOUNDARY IS THIS `lazy`, not a widget
            // inside the extern: the blob IS the key, so the
            // tokenize + row build reruns only when the text,
            // path, or appearance moves — the same projection
            // idiom as every cached surface, instead of the
            // app's one raw iced Lazy, which shipped a pane
            // that drew nothing.
            lazy file_text by file_text, file_path, dark as cached_source
              extern forge_code(cached_source, file_path, dark) #forge-code
            if file_truncated
              text "This file is larger than the 64 KiB preview limit."
                with
                  size=11.5
                  wrap=none
                  @text-label
