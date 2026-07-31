// FORGE — the repo overview, one repo's Code/Pull requests/Issues seats, and
// the item detail with its merge box, reviews and discussion. See
// `screens/roster.ice` for the screen contract.
//
// The `forge_item_*` props keep their app names on purpose: the detail half is
// one family, and the guards in main.rs name several of its members verbatim.
// Everything outside that family drops the redundant `forge_` prefix.

component ForgeScreen(org:str, about:str, tier:str, repos:[ForgeRepo], open_repo:str, repo_menu:bool, branches:[str], tab:str, items:[ForgeItem], tree_repo:str, tree_path:str, tree_entries:[TreeEntry], file_path:str, file_text:str, file_binary:bool, file_truncated:bool, forge_item_number:i64, forge_item_kind:str, forge_item_title:str, forge_item_state:str, forge_item_author:str, forge_item_branches:str, forge_item_body:str, forge_item_files_changed:i64, forge_item_additions:i64, forge_item_deletions:i64, forge_item_diff:str, forge_item_diff_truncated:bool, forge_item_merge_oid:str, forge_item_source_oid:str, forge_item_channel:str, forge_item_approvals:i64, forge_item_change_requests:i64, forge_item_reviews:[ForgeReview], merge_conflicts:[str], merge_busy:bool, review_verdict:str, bind review_draft:str, review_busy:bool, comment_target:str, bind comment_draft:str, staged_comments:[ForgeDraftComment], discussion:[ChatMessage], bind discussion_editor:editor, discussion_pending:str, connected:bool, loading:bool, shift_held:bool)
  emits
    forge_open_repo(str)
    forge_close_repo()
    forge_toggle_repo_menu()
    select_forge_tab(str)
    forge_open_dir(str)
    forge_open_file(str)
    forge_open_item(i64)
    forge_close_item()
    forge_merge_submit()
    forge_review_pick(str)
    forge_review_submit()
    forge_comment_open(str, str, str)
    forge_comment_stage()
    forge_comment_cancel()
    forge_comment_drop(str)
    note_composer_event(ComposerEvent)
  col w=fill h=fill
    // THE REPO OVERVIEW. Reachable again: `forge_close_repo` clears the open
    // repo, which nothing did before — once a repo was opened the grid was
    // gone for the rest of the session.
    if empty(open_repo)
      scroll dir=vertical w=fill h=fill
        col w=fill p=22.0 gap=18.0
          ForgeOrgHeader org=org about=about repos=len(repos) tier=tier
          if empty(repos)
            EmptyPlate message="No repos — a consensus-backed repo appears here once it is created."
          if !empty(repos)
            grid min-cell=380.0 gap=13.0
              for repo in repos
                RepoCard repo=repo
                  forward
                    forge_open_repo
    if !empty(open_repo)
      col w=fill h=fill
        box w=fill pl=22.0 pr=22.0 pt=14.0 pb=12.0
          stack w=fill
            row w=fill gap=9.0 align=center
              button label="Switch repository" w=fill p=0.0 @ghost_action -> emit(forge_toggle_repo_menu)
                RepoCrumb org=org repo=open_repo branch="" open=repo_menu
                active bg=transparent text=fg border=transparent border-w=1.0 r=9.0
                hovered bg=row_hover text=fg
                pressed bg=elevated text=fg
              button "All repos" h=28.0 p=6.0 @secondary_action -> emit(forge_close_repo)
            if repo_menu
              pin x=0.0 y=38.0
                Popover width=290.0
                  col w=fill gap=1.0
                    for repo in repos
                      RepoMenuRow repo=repo active=(repo.name == open_repo)
                        forward
                          forge_open_repo
        box w=fill h=1.0 bg=separator
          space w=1.0 h=1.0
        if forge_item_number <= 0
          col w=fill h=fill
            if !empty(branches)
              box w=fill pl=22.0 pr=22.0 pt=10.0 pb=10.0
                scroll dir=horizontal w=fill h=22.0 bar=hidden
                  row h=fill gap=4.0 align=center
                    for branch in branches
                      box h=20.0 pl=7.0 pr=7.0 align-y=center bg=surface border=border border-w=1.0 r=10.0
                        text branch size=9.0 wrap=none font=code_semibold @text-meta
            // THE TAB BAR THE TRACKER NEVER GOT. `forge_tab` has sat in
            // state.ice and `filter_forge_items`/`forge_open_count` in
            // backend.ice since wave 1 with no call site at all, so the
            // screen piled merged PRs and closed issues into one flat
            // list. The counts are OPEN work — a PR counts until it
            // merges, an issue until it closes.
            box w=fill pl=22.0 pr=22.0
              row w=fill gap=22.0 align=center
                button label="Browse the code" p=0.0 @ghost_action -> emit(select_forge_tab, "code")
                  TabLabel label="Code" count=0 active=(tab == "code")
                  active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
                  hovered bg=transparent text=fg
                  pressed bg=transparent text=fg
                button label="Show pull requests" p=0.0 @ghost_action -> emit(select_forge_tab, "pulls")
                  TabLabel label="Pull requests" count=forge_open_count(items, "pr") active=(tab == "pulls")
                  active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
                  hovered bg=transparent text=fg
                  pressed bg=transparent text=fg
                button label="Show issues" p=0.0 @ghost_action -> emit(select_forge_tab, "issues")
                  TabLabel label="Issues" count=forge_open_count(items, "issue") active=(tab == "issues")
                  active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
                  hovered bg=transparent text=fg
                  pressed bg=transparent text=fg
                space w=fill
            box w=fill h=1.0 bg=separator
              space w=1.0 h=1.0
            // One discriminant, one match: Code browses the tree, the
            // other two seats are the same tracker list under different
            // filters.
            match tab
              "code"
                // The header carries the path from the repo root and
                // NOTHING ELSE. The artifact prefixes the repo name;
                // the breadcrumb directly above already says it, and
                // Ice has no string concatenation to join the two.
                // `message` / `author` / `stamp` are the last commit
                // under this path — the mirror could answer that with a
                // revwalk, and until a loader does the three slots stay
                // empty rather than printing a middot run around values
                // nobody read. ForgeCodeHeader drops each empty slot by
                // construction.
                ForgeCodeTab path=file_path message="" author="" stamp=""
                  files:
                    // ONE GUARD, AT THE TOP OF THE PANE. The listing
                    // paints only for the repo it was read from —
                    // `forge_open_repo` lives in a handler file this one
                    // does not own and clears none of this, and another
                    // project's files under a new breadcrumb is a wrong
                    // reading, not a stale one. Picking Code re-reads
                    // the root, so the tab itself is the recovery.
                    col w=fill
                      if tree_repo != open_repo
                        box w=fill pl=16.0 pr=16.0 pt=8.0
                          text "Pick Code again to browse this repository." w=fill size=11.5 line-h=1.5 @text-label
                      if tree_repo == open_repo
                        col w=fill
                          if !empty(tree_path)
                            button label="Back to the repository root" w=fill p=0.0 @icon_action -> emit(forge_open_dir, "")
                              ForgeTreeDirRow name="/" depth=0.0 open=true
                              active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
                              hovered bg=rail_hover text=fg
                              pressed bg=elevated text=fg
                          for entry in tree_entries
                            col w=fill
                              if entry.kind == "dir"
                                button label="Open directory" description=entry.path w=fill p=0.0 @icon_action -> emit(forge_open_dir, entry.path)
                                  ForgeTreeDirRow name=entry.name depth=0.0 open=false
                                  active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
                                  hovered bg=rail_hover text=fg
                                  pressed bg=elevated text=fg
                              if entry.kind != "dir"
                                button label="Open file" description=entry.path w=fill p=0.0 @icon_action -> emit(forge_open_file, entry.path)
                                  ForgeTreeFileRow name=entry.name depth=0.0 selected=(entry.path == file_path)
                                  active bg=transparent text=fg border=transparent border-w=1.0 r=0.0
                                  hovered bg=rail_hover text=fg
                                  pressed bg=elevated text=fg
                  source:
                    // The reader's three states, each with its own true
                    // reason. Nothing here claims a file is empty.
                    col w=fill
                      if empty(file_path)
                        ForgeCodeEmpty name="" note="Pick a file from the tree to read it."
                      if !empty(file_path) && file_binary
                        ForgeCodeEmpty name=file_path note="This is not text — the reader shows no preview for it."
                      if !empty(file_path) && !file_binary
                        col w=fill pt=13.0 pb=13.0 gap=9.0
                          // NUMBERED LINES. `source_lines` is the exact
                          // counterpart `diff_lines` already is for a
                          // patch: `forge_blob` hands back ONE string
                          // and Ice has no string ops, so the split
                          // happens in backend.rs and the gutter counts
                          // the rows it actually produced rather than
                          // guessing where the file breaks.
                          col w=fill
                            for line in source_lines(file_text)
                              ForgeCodeLine number=line.number code=line.text
                          if file_truncated
                            text "Truncated at the reader's 64 KiB window — the file on the node is whole." size=11.5 wrap=none @text-label
              "issues"
                ForgeTrackerList items=filter_forge_items(items, "issue") empty_message="No issues — an issue opened against this repo appears here."
                  forward
                    forge_open_item
              _
                ForgeTrackerList items=filter_forge_items(items, "pr") empty_message="No pull requests — a PR pushed to this repo appears here."
                  forward
                    forge_open_item
            // NO GATE NOTE HERE. `ForgeGateNote` told a resident the
            // node refuses their merge; `ForgeMsg::MergePr` authorizes
            // on `author_from_origin` alone, so the write succeeds and
            // the plate described an enforcement that does not exist.
            // The one true sentence about it lives beside the merge
            // button, where the decision is made.
        if forge_item_number > 0
          scroll dir=vertical w=fill h=fill
            col w=fill p=22.0 gap=14.0
              BackToList kind=forge_item_kind
                forward
                  forge_close_item
              row w=fill gap=9.0 align=center
                text forge_item_title w=fill size=16.0 wrap=none font=display @text-primary
                if forge_item_kind == "pr"
                  PrStatePill state=forge_item_state
                if forge_item_kind != "pr"
                  StatusBadge label=forge_item_state
              row w=fill gap=10.0 align=center
                if !empty(forge_item_author)
                  text forge_item_author size=11.0 wrap=none font=code_medium @text-meta
                if !empty(forge_item_branches)
                  text forge_item_branches size=12.0 wrap=none font=code @text-meta
                if forge_item_files_changed > 0
                  DiffCount additions=forge_item_additions deletions=forge_item_deletions files=forge_item_files_changed
                space w=fill
              if !empty(forge_item_body)
                IssueBodyCard author=forge_item_author body=forge_item_body
              if !empty(forge_item_diff)
                col w=fill gap=6.0
                  if forge_item_diff_truncated
                    text "Patch truncated — the counts above cover the whole diff." size=12.5 @text-caption
                  // The pane's header names the patch's own branch pair:
                  // `forge_item_diff` is the WHOLE unified patch, and its
                  // per-file headers ride inside it as `file` diff rows.
                  DiffPane file=forge_item_branches additions=forge_item_additions deletions=forge_item_deletions lines=diff_lines(forge_item_diff)
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
                          text "Merge conflicts — resolve on the branch and push again:" size=12.5 @text-caption
                          for conflict_path in merge_conflicts
                            text conflict_path size=12.0 font=code @text-fg
                      MergeAdvisory change_requests=forge_item_change_requests
                      row w=fill gap=10.0 align=center
                        MergeButton busy=merge_busy disabled=(!connected || empty(forge_item_source_oid))
                          forward
                            forge_merge_submit
                        // The tally belongs where the decision is made,
                        // and it is loaded already. The sentence beside
                        // it is the whole permission model this module
                        // has: none. Approvals never block a merge.
                        text forge_item_approvals size=12.0 wrap=none font=code_medium @text-meta
                        text "approvals" size=12.5 wrap=none @text-caption
                        // The blanket sentence is the NO-ADVISORY half only.
                        // MergeAdvisory above already says "a reviewer
                        // requested changes — merge not recommended", and
                        // printing both left the box asserting that nothing
                        // gates the merge directly under a line recommending
                        // against it.
                        if forge_item_change_requests <= 0
                          text "Approvals are advisory — merging is never gated." w=fill size=12.5 @text-caption
              if forge_item_kind == "pr"
                col w=fill gap=9.0
                  GroupLabel label="REVIEWS"
                  if empty(forge_item_reviews)
                    text "No reviews yet." size=12.5 @text-caption
                  for review in forge_item_reviews
                    ReviewCard review=review
                  row w=fill gap=6.0 align=center
                    button label="Pick comment verdict" h=24.0 p=5.0 @ghost_action -> emit(forge_review_pick, "comment")
                      text verdict_pick_label(review_verdict, "comment", "Comment") size=13.0
                      active bg=surface text=fg border=card_line border-w=1.0 r=7.0
                      hovered bg=elevated text=fg
                      pressed bg=subtle text=fg
                    button label="Pick approve verdict" h=24.0 p=5.0 @ghost_action -> emit(forge_review_pick, "approve")
                      text verdict_pick_label(review_verdict, "approve", "Approve") size=13.0
                      active bg=final_bg text=fg border=final_line border-w=1.0 r=7.0
                      hovered bg=success_bg text=fg
                      pressed bg=success_bg text=fg
                    button label="Pick request-changes verdict" h=24.0 p=5.0 @ghost_action -> emit(forge_review_pick, "request_changes")
                      text verdict_pick_label(review_verdict, "request_changes", "Request changes") size=13.0
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
                      row w=fill gap=7.0 align=center
                        text comment_target w=fill size=11.0 wrap=none font=code_medium @text-brand
                        button "Cancel" h=24.0 p=5.0 @secondary_action -> emit(forge_comment_cancel)
                      row w=fill gap=6.0 align=center
                        input "" #forge-comment-body label="Line comment" <-> comment_draft hint="Comment on this line…" disabled=(review_busy || !connected) submit=emit(forge_comment_stage) w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                          active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                          hovered bg=muted_bg border=control_line
                          disabled bg=muted_bg/54 value=muted
                        button "Add comment" disabled=(review_busy || !connected || empty(comment_draft) || forge_comment_cap_reached(staged_comments)) h=28.0 p=6.0 @secondary_action -> emit(forge_comment_stage)
                  if forge_comment_cap_reached(staged_comments)
                    text "Comment limit reached for one review — submit this review, then start another." size=12.5 @text-caption
                  // Staged and not yet sent. These ride out INSIDE the review
                  // below, so they are shown as part of its draft, not as
                  // posted comments.
                  if !empty(staged_comments)
                    col w=fill gap=4.0
                      for staged in staged_comments
                        row w=fill gap=7.0 align=center
                          box w=fill pl=11.0 pr=11.0 pt=9.0 pb=9.0 bg=brand_wash border=brand_line border-w=1.0 r=9.0
                            col w=fill gap=4.0
                              row w=fill gap=7.0 align=center
                                text staged.anchor w=fill size=11.0 wrap=none font=code_medium @text-brand
                                text "not sent yet" size=10.0 wrap=none font=code_semibold @text-label
                              text staged.body w=fill size=12.0 line-h=1.55 @text-panel_tile
                          button "Remove" label="Remove staged comment" h=24.0 p=5.0 @secondary_action -> emit(forge_comment_drop, staged.anchor)
                  row w=fill gap=6.0 align=center
                    input "" #forge-review-body label="Review body" <-> review_draft hint="Leave a review…" disabled=(review_busy || !connected) submit=emit(forge_review_submit) w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                      active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                      hovered bg=muted_bg border=control_line
                      disabled bg=muted_bg/54 value=muted
                    // The module refuses a review that is empty on BOTH halves,
                    // so the button refuses it first rather than spending a
                    // round trip to be told.
                    button "Submit review" disabled=(review_busy || !connected || empty(forge_item_source_oid) || (empty(review_draft) && empty(staged_comments))) h=28.0 p=6.0 @primary_action -> emit(forge_review_submit)
              col w=fill gap=9.0
                GroupLabel label="DISCUSSION"
                if empty(discussion)
                  text "No discussion yet." size=12.5 @text-caption
                for message in discussion
                  row w=fill gap=9.0 align=start
                    MessageAvatar initials=message.initial kind=message.avatar_kind
                    col w=fill gap=2.0
                      row w=fill gap=7.0 align=center
                        text message.author size=13.0 wrap=none font=display @text-fg
                        text message.meta size=11.0 wrap=none font=code_medium @text-meta
                        space w=fill
                      MessageBody message=message
                flex w=fill gap=8.0 items=end
                  box w=fill bg=surface border=card_line border-w=1.0 r=8.0 clip=true
                    extern rich_composer(discussion_editor, "Write a note…", (loading || !connected || empty(forge_item_channel)), shift_held, 38.0, 120.0, 6.0) #forge-note -> emit(note_composer_event, _)
                  button "Send" disabled=(loading || !connected || empty(forge_item_channel) || !empty(discussion_pending) || empty(trim(editor_text(discussion_editor)))) w=60.0 h=28.0 p=6.0 @primary_action -> emit(note_composer_event, composer_submit_event())
