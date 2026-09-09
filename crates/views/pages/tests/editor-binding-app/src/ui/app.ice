app PagesEditorFixture
  title "Pages editor binding"
  id "dev.ducktape.pages-editor-binding"

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

extern crate::fixture
  pure large_source() -> DocumentSource

theme contract AppTheme
  bg
  fg
  primary
  danger
palette app for AppTheme
  bg #ffffff
  fg #000000
  primary #ff0000
  danger #ff00ff

state
  formatting_notice = ""
  document:editor = editor("- 한글")
  history:HistoryState = initial_history()
  menu:MenuState = initial_menu()
  source:DocumentSource = empty_source()
  installed_source:bytes = bytes()
  load_error = ""
  paint_dark = false
  commented:[i64] = []

subscribe
  document_source(source) when !empty(source.reference) && source.reference != installed_source -> document_arrived _

on load
  source = large_source()

on document_arrived(item)
  return if item.source != source.reference
  load_error = item.error
  return if !empty(item.error)
  formatting_notice = item.notice
  document = document_editor(item.text, item.cursor)
  installed_source = item.source
  menu = initial_menu()

on committed(next)
  formatting_notice = next.notice
  history = next.history
  menu = next.menu

view
  col w=640.0 gap=8.0
    editor #document <-> document -> committed _
      with
        key-binding=keys(history, menu)
        highlighter=paint(menu, paint_dark, commented)
        w=640.0
        min-h=240.0
        max-h=240.0
    if 64000 > len(editor_text(document))
      text editor_text(document) #echo
    text formatting_notice #formatting-notice
    text load_error #load-error
    button "Load document" -> load
