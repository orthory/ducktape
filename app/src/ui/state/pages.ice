state
  pages:[PageItem] = []
  doc_tabs:[str] = []
  blocks:[PageBlock] = []
  active_page = ""
  active_page_title = ""
  active_page_parent = ""
  page_draft = ""
  page_create_open = false
  pending_page = ""

  block_comments_open = false
  block_comments_target = ""
  block_comments_generation:i64 = 0
  block_comment_threads:[PageCommentThread] = []
  block_comment_rows:[PageCommentThreadRow] = []
  block_comment_thread_total:i64 = 0
  block_comment_threads_next_from:i64 = 0
  block_comment_threads_has_more = false
  block_comment_threads_loading = false
  active_block_comment_thread = ""
  block_thread_comments:[PageComment] = []
  block_thread_comments_next_from:i64 = 0
  block_thread_comments_has_more = false
  block_thread_comments_loading = false
  block_comment_draft = ""
  pending_block_comment = ""

  // The document is one editor buffer. Drift from the last saved text is the
  // dirty signal; `buffer_page` names what that buffer actually contains.
  page_editor:editor = ""
  page_saved_text = ""
  buffer_page = ""
  commented_block_hits:[str] = []
  caret_comment_target = ""
  active_thread_target = ""
  active_thread_anchor = ""
  page_inflight_text = ""
  page_refusal = ""
  block_autosave_status:AutosaveStatus = AutosaveStatus.idle
  orphaned_comment_drafts:[str] = []
  page_delete_armed = false
  // Domain revision for text folds, independent of structural hydration.
  pages_fold_serial:i64 = 0
  page_search_draft = ""
  page_search_hits:[PageSearchHit] = []
  page_searching = false
  // THE STRING THE ZERO-HIT PLATE IS SPEAKING FOR — the query a search was
  // actually SENT for, `""` when no answer is standing (an empty value also
  // gates both reply handlers: no search standing, the reply is dropped).
  // Not a boolean — the enter-to-submit rationale lives on the plate arm in
  // `screens/pages.ice`. Written by `search_pages_submit`; cleared by
  // `page_search_failed` and every handler that drops the hits. (The palette
  // needs no captured string: `palette_changed` runs on every keystroke —
  // see `palette_search_phase`.)
  page_search_query = ""
