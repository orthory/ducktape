app PagesView
  title "Pages"
  palette active_palette
  id "dev.ducktape.view.pages"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"
use "../../../../../app/src/ui/ducktape-ui/recipes.ice"
use "../../../../../app/src/ui/components/icon.ice"
use "pages.ice"
use "rows.ice"
use "kit.ice"

extern crate::host
  PageItem(id:str, title:str, parent:str, prefix:str, child_count:i64)
  DocTab(id:str, title:str, active:bool)
  Subpage(id:str, title:str)
  PageSearchHit(page_id:str, page_title:str, block_id:str, kind:str, text:str)
  PageCommentThread(id:str, target:str, author:str, meta:str, resolved:bool, comment_count:i64)
  PageCommentThreadRow(thread:PageCommentThread, anchor:str)
  PageComment(id:str, ordinal:i64, author:str, meta:str, text:str)
  PagesProps(document_source:bytes, document_error:str, commented_lines:[i64], dark:bool, connected:bool, loading:bool, busy:bool, page_link:str, pages:[PageItem], page_create_open:bool, active_page:str, active_page_title:str, active_page_parent:str, page_searching:bool, page_search_hits:[PageSearchHit], page_search_query:str, page_delete_armed:bool, autosave:str, page_refusal:str, doc_tabs:[DocTab], subpages:[Subpage], orphaned_comment_drafts:[str], block_comments_open:bool, thread_total:i64, comment_rows:[PageCommentThreadRow], threads_loading:bool, threads_has_more:bool, active_thread:str, thread_resolved:bool, active_thread_anchor:str, comments:[PageComment], comments_loading:bool, comments_has_more:bool, compose_hint:str, seed_rev:i64, page_seed:str, comment_seed:str)
  PropsItem(next:PagesProps, error:str)
  subscription props() -> PropsItem
  pure edited(source:bytes, reference:bytes, navigation:bytes) -> bool
  pure toggle_create() -> bool
  pure create(title:&str, comment_draft:&str) -> bool
  pure choose(id:&str, comment_draft:&str) -> bool
  pure search(query:&str) -> bool
  pure clear_search() -> bool
  pure arm_delete() -> bool
  pure disarm_delete() -> bool
  pure delete(comment_draft:&str) -> bool
  pure close_tab(id:&str) -> bool
  pure open_hit(page_id:&str, block_id:&str, comment_draft:&str) -> bool
  pure use_draft(draft:&str, comment_draft:&str) -> bool
  pure discard_draft(draft:&str) -> bool
  pure toggle_comments(comment_draft:&str) -> bool
  pure close_comments(comment_draft:&str) -> bool
  pure open_thread(id:&str, target:&str) -> bool
  pure resolve(resolved:bool) -> bool
  pure more_threads() -> bool
  pure close_thread() -> bool
  pure more_comments() -> bool
  pure post(text:&str) -> bool
  pure copy(text:&str, label:&str) -> bool
  pure icon(name:&str) -> bytes
  pure count_label(count:i64) -> str
  pure keep_str(keep:bool, next:&str, current:&str) -> str
  pure search_answer_stands(query:&str, draft:&str, searching:bool) -> bool
  pure initials_of(name:&str) -> str
  pure seeded(moved:bool, seed:&str, draft:&str) -> str

extern crate::editor_binding
  HistoryState(snapshot:bytes)
  MenuState(snapshot:bytes)
  EditorUpdate(notice:str, history:HistoryState, menu:MenuState, reference:bytes, interaction:bytes)
  pure initial_history() -> HistoryState
  pure initial_menu() -> MenuState
  editor-binding keys(history:HistoryState, menu:MenuState) -> EditorUpdate

extern crate::presentation
  editor-highlighter paint(menu:MenuState, dark:bool, commented:[i64])

extern crate::document_ingress
  DocumentSource(reference:bytes)
  DocumentItem(notice:str, source:bytes, text:str, cursor:bytes, error:str)
  pure empty_source() -> DocumentSource
  pure document_editor(text:str, cursor:bytes) -> editor
  subscription document_source(source:DocumentSource) -> DocumentItem

state
  formatting_notice = ""
  document:editor = ""
  document_history:HistoryState = initial_history()
  document_menu:MenuState = initial_menu()
  document_source_ref:DocumentSource = empty_source()
  document_installed:bytes = bytes()
  document_error = ""
  document_source_error = ""
  document_dark = false
  document_commented:[i64] = []
  active_palette:palette[AppTheme] = AppTheme.app
  connected = false
  loading = false
  busy = false
  page_link = ""
  pages:[PageItem] = []
  page_create_open = false
  active_page = ""
  active_page_title = ""
  active_page_parent = ""
  page_searching = false
  page_search_hits:[PageSearchHit] = []
  page_search_query = ""
  page_delete_armed = false
  autosave = "idle"
  page_refusal = ""
  doc_tabs:[DocTab] = []
  subpages:[Subpage] = []
  orphaned_comment_drafts:[str] = []
  block_comments_open = false
  thread_total:i64 = 0
  comment_rows:[PageCommentThreadRow] = []
  threads_loading = false
  threads_has_more = false
  active_thread = ""
  thread_resolved = false
  active_thread_anchor = ""
  comments:[PageComment] = []
  comments_loading = false
  comments_has_more = false
  compose_hint = ""
  // the last seed the app pushed: the count moves once per hand-back
  seed_rev:i64 = 0
  // the reader's own: the three drafts the screen edits
  page_draft = ""
  page_search_draft = ""
  block_comment_draft = ""
  host_error = ""
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false

// The facts are the host's: one subscription, one item per change. A
// subscription, not a mount task, so a replacement restored from this
// view's state asks for the facts again on its own.
subscribe
  document_source(document_source_ref) when !empty(document_source_ref.reference) && document_source_ref.reference != document_installed -> document_arrived _
  props() -> props_arrived _

on props_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  let next = item.next
  connected = next.connected
  loading = next.loading
  busy = next.busy
  page_link = next.page_link
  pages = next.pages
  page_create_open = next.page_create_open
  active_page = next.active_page
  active_page_title = next.active_page_title
  active_page_parent = next.active_page_parent
  page_searching = next.page_searching
  page_search_hits = next.page_search_hits
  page_search_query = next.page_search_query
  page_delete_armed = next.page_delete_armed
  autosave = next.autosave
  page_refusal = next.page_refusal
  doc_tabs = next.doc_tabs
  subpages = next.subpages
  orphaned_comment_drafts = next.orphaned_comment_drafts
  block_comments_open = next.block_comments_open
  thread_total = next.thread_total
  comment_rows = next.comment_rows
  threads_loading = next.threads_loading
  threads_has_more = next.threads_has_more
  active_thread = next.active_thread
  thread_resolved = next.thread_resolved
  active_thread_anchor = next.active_thread_anchor
  comments = next.comments
  comments_loading = next.comments_loading
  comments_has_more = next.comments_has_more
  compose_hint = next.compose_hint
  // THE APP HANDS A DRAFT BACK ONLY WHEN THE SEED MOVED — a recovered
  // comment taken up, a failed post returned, a failed page create
  // returned. Every other push leaves the reader's fields alone.
  let moved = next.seed_rev != seed_rev
  seed_rev = next.seed_rev
  page_draft = seeded(moved, next.page_seed, page_draft)
  block_comment_draft = seeded(moved, next.comment_seed, block_comment_draft)
  document_source_ref = DocumentSource(next.document_source)
  document_source_error = next.document_error
  document_dark = next.dark
  document_commented = next.commented_lines
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

on toggle_page_create
  sent = toggle_create()

// The title leaves with the act; the field clears here, as the app's
// handler used to clear it — a refused create hands it back as a seed.
on create_page_submit
  return if loading || busy || !connected || empty(trim(page_draft))
  sent = create(trim(page_draft), block_comment_draft)
  page_draft = ""

// A PICK ABANDONS THE RAIL'S DRAFT: it leaves with the act, for the app to
// keep as a recovered draft on the page it belonged to.
on choose_page(id)
  return if loading || busy
  sent = choose(id, block_comment_draft)
  block_comment_draft = ""

on search_pages_submit
  return if page_searching || empty(trim(page_search_draft))
  sent = search(trim(page_search_draft))

on clear_page_search
  sent = clear_search()
  page_search_draft = ""

on arm_page_delete
  sent = arm_delete()

on disarm_page_delete
  sent = disarm_delete()

on delete_page_submit
  sent = delete(block_comment_draft)

on close_doc_tab(id)
  sent = close_tab(id)

on open_page_search_hit(page_id, block_id)
  return if loading || busy
  sent = open_hit(page_id, block_id, block_comment_draft)
  block_comment_draft = ""

on use_orphaned_comment_draft(draft)
  return if loading || busy || !empty(trim(block_comment_draft))
  sent = use_draft(draft, block_comment_draft)

on discard_orphaned_comment_draft(draft)
  sent = discard_draft(draft)

on toggle_block_comments
  return if loading || busy || empty(active_page)
  sent = toggle_comments(block_comment_draft)
  block_comment_draft = ""

on close_block_comments
  sent = close_comments(block_comment_draft)
  block_comment_draft = ""

on open_block_comment_thread(id, target)
  sent = open_thread(id, target)

on resolve_thread_submit(resolved)
  sent = resolve(resolved)

on load_more_block_threads
  sent = more_threads()

on close_block_comment_thread
  sent = close_thread()

on load_more_block_comments
  sent = more_comments()

// The comment leaves with the act and the field clears, as the app's
// handler used to clear it; a post the node refused hands it back as a seed.
on post_block_comment_submit
  return if busy || threads_loading || comments_loading || empty(trim(block_comment_draft))
  sent = post(trim(block_comment_draft))
  block_comment_draft = ""

on copy_to_clipboard(text, label)
  sent = copy(text, label)

on document_arrived(item)
  return if item.source != document_source_ref.reference
  document_error = item.error
  return if !empty(item.error)
  formatting_notice = item.notice
  document = document_editor(item.text, item.cursor)
  document_installed = item.source
  document_menu = initial_menu()

on document_committed(next)
  formatting_notice = next.notice
  document_history = next.history
  document_menu = next.menu
  sent = edited(document_installed, next.reference, next.interaction)

view
  box #root
    with
      w=fill
      h=fill
      bg=bg
    PagesScreen page_draft<->page_draft page_search_draft<->page_search_draft block_comment_draft<->block_comment_draft #pages
      with
        page_link
        pages
        page_create_open
        loading
        busy
        connected
        active_page
        active_page_title
        active_page_parent
        page_searching
        page_search_hits
        page_search_query
        page_delete_armed
        autosave
        page_refusal
        doc_tabs
        subpages
        orphaned_comment_drafts
        block_comments_open
        thread_total
        comment_rows
        threads_loading
        threads_has_more
        active_thread
        thread_resolved
        active_thread_anchor
        comments
        comments_loading
        comments_has_more
        compose_hint
      events
        toggle_page_create -> toggle_page_create
        create_page_submit -> create_page_submit
        choose_page -> choose_page _
        search_pages_submit -> search_pages_submit
        clear_page_search -> clear_page_search
        arm_page_delete -> arm_page_delete
        disarm_page_delete -> disarm_page_delete
        delete_page_submit -> delete_page_submit
        close_doc_tab -> close_doc_tab _
        open_page_search_hit -> open_page_search_hit _ _
        use_orphaned_comment_draft -> use_orphaned_comment_draft _
        discard_orphaned_comment_draft -> discard_orphaned_comment_draft _
        toggle_block_comments -> toggle_block_comments
        close_block_comments -> close_block_comments
        open_block_comment_thread -> open_block_comment_thread _ _
        resolve_thread_submit -> resolve_thread_submit _
        load_more_block_threads -> load_more_block_threads
        close_block_comment_thread -> close_block_comment_thread
        load_more_block_comments -> load_more_block_comments
        post_block_comment_submit -> post_block_comment_submit
        copy_to_clipboard -> copy_to_clipboard _ _
      document:
        col w=fill h=fill
          if !empty(formatting_notice)
            text formatting_notice @text-muted
          if !empty(document_source_error)
            text document_source_error @text-danger
          if !empty(document_error)
            text document_error @text-danger
          editor #document <-> document -> document_committed _
            with
              key-binding=keys(document_history, document_menu)
              highlighter=paint(document_menu, document_dark, document_commented)
              w=fill
              h=fill
              size=14.0
              line-h=1.65
              wrap=word
              font=display
              disabled=(loading || !connected || empty(document_source_ref.reference) || document_installed != document_source_ref.reference)
