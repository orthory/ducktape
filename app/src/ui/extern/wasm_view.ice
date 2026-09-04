// A MODULE'S VIEW, RUN AS WASM. A module that ships `<id>.view.wasm` beside
// its component draws its own screen inside a fuel and memory budget; the app
// answers what the guest asks for (its theme, a data generation, a query to
// the node) and replays the frame it draws. See `backend/wasm_view.rs`.
//
// `load_wasm_view` answers `none` when the module ships no view, and the
// screen stays the app's own. `serve_wasm_view` is the one task the widget's
// events run: it carries the guest's queries to the node, or reloads a
// trapped module into the same handle — the handler needs no branch, the
// event says which. The surface it answers with is the same handle; hearing
// it is what redraws the view and delivers the guest's inbox.
extern crate::backend::wasm_view
  WasmSurface()
  WasmViewEvent()
  WasmViewError(message:str)
  load_wasm_view(id:str) -> WasmSurface? ! WasmViewError
  serve_wasm_view(surface:WasmSurface?, event:WasmViewEvent, rpc:str) -> WasmSurface? ! WasmViewError
  component wasm_view(surface:&WasmSurface, dark:bool, generation:i64, wake:i64) -> WasmViewEvent
