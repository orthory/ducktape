state
  forge_list_phase:ForgePhase = ForgePhase.idle
  forge_repos:[ForgeRepo] = []
  forge_repo = ""
  // A forge deep link's second step, consumed by `forge_repo_loaded`.
  forge_focus_number:i64 = 0
  forge_focus_path = ""
  forge_focus_rev = ""
  // A forge item deep link's `#seq`: the Discussion note to land on, consumed
  // by `forge_discussion_loaded` into the note below — picked ONCE there, so
  // the view never hands the discussion list to anything.
  forge_focus_seq:i64 = 0
  forge_linked_note:ChatMessage? = none
  forge_repo_phase:ForgePhase = ForgePhase.idle
  forge_branches:[str] = []
  forge_items:[ForgeItem] = []
  forge_item_number:i64 = 0
  forge_item_phase:ForgePhase = ForgePhase.idle
  forge_item_title = ""
  forge_item_state = ""
  forge_item_kind = ""
  forge_item_body = ""
  forge_item_author = ""
  forge_item_branches = ""
  forge_item_channel = ""
  forge_item_source_branch = ""
  forge_item_source_oid = ""
  forge_item_target_oid = ""
  forge_item_merge_oid = ""
  forge_item_diff = ""
  forge_item_diff_truncated:bool = false
  forge_item_files_changed:i64 = 0
  forge_item_additions:i64 = 0
  forge_item_deletions:i64 = 0
  forge_item_reviews:[ForgeReview] = []
  forge_item_approvals:i64 = 0
  forge_item_change_requests:i64 = 0
  forge_review_verdict:ForgeReviewVerdict = ForgeReviewVerdict.comment
  forge_review_draft = ""
  forge_review_busy:bool = false
  forge_comment_path = ""
  forge_comment_line = ""
  forge_comment_side = ""
  forge_comment_draft = ""
  forge_comment_staged:[ForgeDraftComment] = []
  forge_merge_busy:bool = false
  forge_merge_conflicts:[str] = []
  forge_discussion:[ChatMessage] = []
  forge_discussion_members:[ChatMember] = []
  forge_discussion_editor:editor = ""
  forge_discussion_pending = ""
  forge_generation:i64 = 0

  forge_tab:ForgeTab = ForgeTab.code
  forge_repo_menu = false
