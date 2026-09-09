app PagesEditorFixture
  title "Pages editor binding"
  id "dev.ducktape.pages-editor-binding"

extern crate::editor_binding
  HistoryState(snapshot:bytes)
  pure initial_history() -> HistoryState
  editor-binding keys(history:HistoryState) -> HistoryState

extern crate::document_ingress
  DocumentSource(reference:bytes)
  DocumentItem(source:bytes, text:str, error:str)
  pure empty_source() -> DocumentSource
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
  document:editor = editor("- 한글")
  history:HistoryState = initial_history()
  source:DocumentSource = empty_source()
  installed_source:bytes = bytes()
  load_error = ""

subscribe
  document_source(source) when !empty(source.reference) && source.reference != installed_source -> document_arrived _

on load
  source = large_source()

on document_arrived(item)
  return if item.source != source.reference
  load_error = item.error
  return if !empty(item.error)
  document = editor(item.text)
  installed_source = item.source

on committed(next)
  history = next

view
  col w=640.0 gap=8.0
    editor #document <-> document -> committed _
      with
        key-binding=keys(history)
        w=640.0
        min-h=240.0
        max-h=240.0
    if 64000 > len(editor_text(document))
      text editor_text(document) #echo
    text load_error #load-error
    button "Load document" -> load
