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
  forge_item_blocks:[ChatBlock] = []
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
  forge_review_busy:bool = false
  forge_comment_staged:[ForgeDraftComment] = []
  // The review body and the line comment being written are the Forge
  // view's. A committed op tells it which drafts it consumed: the count
  // moves once per op, the scope names them (`item`, `review`, `comment`).
  forge_drafts_cleared:i64 = 0
  forge_drafts_scope = ""
  // A forge item deep link's `#seq` landing, counted for the view's scroll:
  // the tick moves once per landing, the seq is the row it lands on.
  forge_landed_seq:i64 = 0
  forge_landed_tick:i64 = 0
  forge_merge_busy:bool = false
  forge_merge_conflicts:[str] = []
  forge_discussion:[ChatMessage] = []
  forge_discussion_members:[ChatMember] = []
  forge_discussion_pending = ""
  forge_generation:i64 = 0
  // THE CODE BROWSE: the open directory's listing, pinned to the commit the
  // root listing answered with, and the file opened under it. The view
  // draws the file only while the tree still stands where it was opened
  // (`forge_file_header`).
  forge_tree_path = ""
  forge_tree_rev = ""
  forge_tree_entries:[TreeEntry] = []
  forge_tree_born:bool = false
  forge_tree_truncated:bool = false
  forge_tree_phase:ForgeTreePhase = ForgeTreePhase.loading
  forge_file_path = ""
  forge_file_text = ""
  forge_file_binary:bool = false
  forge_file_truncated:bool = false
  forge_file_picture:bool = false
  forge_file_width:i64 = 0
  forge_file_height:i64 = 0
  forge_file_note = ""
  forge_opened_dir = ""
  forge_opened_rev = ""
  forge_file_phase:ForgeFilePhase = ForgeFilePhase.idle

  forge_tab:ForgeTab = ForgeTab.code
  forge_repo_menu = false
