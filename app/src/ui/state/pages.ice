state
  pages:[PageItem] = []
  blocks:[PageBlock] = []
  active_page = ""
  active_page_title = ""
  active_page_parent = ""
  page_draft = ""
  page_create_open = false
  // Moves once per draft the app hands BACK to the pages view (a recovered
  // comment taken up, a refused post or create returned): the view keeps
  // its own fields and adopts `page_draft`/`block_comment_draft` only then.
  pages_seed_rev:i64 = 0
  pending_page = ""

  block_comments_open = false
  // The page the standing thread load answers for — a reply naming another
  // page is a load the reader has already clicked past.
  block_comments_target = ""
  // THE CARD'S SCOPE: the block whose conversation it is showing, or "" for
  // the whole page. A margin badge opens a block; the header chip opens the
  // page and a group header narrows to a block from there.
  inline_comment_target = ""
  // A BADGE-OPENED CARD IS THE BLOCK'S, whole: it offers no way back out to
  // the page, because the page was never what the reader asked for.
  block_comments_pinned = false
  block_comments_generation:i64 = 0
  block_comment_threads:[PageCommentThread] = []
  // The scope's threads with their anchors resolved — a mirror, so the view
  // never clones the block list once per thread (see tests/stream.rs).
  block_comment_rows:[PageCommentThreadRow] = []
  block_comment_thread_total:i64 = 0
  block_comment_threads_loading = false
  block_comment_draft = ""
  pending_block_comment = ""

  // The document is one editor buffer. Drift from the last saved text is the
  // dirty signal; `buffer_page` names what that buffer actually contains.
  page_text = ""
  page_cursor_line:i64 = 0
  page_saved_text = ""
  buffer_page = ""
  commented_block_hits:[str] = []
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
