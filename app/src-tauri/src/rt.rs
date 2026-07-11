//! Concrete runtime aliases for the CEF-backed shell.
//!
//! Published tauri only defaults its generic types (`AppHandle<R = Wry>`,
//! `WebviewWindow<R = Wry>`, …) when the `wry` feature is on; this build runs
//! on the standalone `tauri-runtime-cef` crate with wry compiled out, so the
//! bare names have no default. Import these aliases instead of the tauri
//! types in command signatures and window plumbing.

/// The CEF runtime specialized to tauri's event-loop message type — the `R`
/// in every generic tauri type this app touches.
pub type Cef = tauri_runtime_cef::CefRuntime<tauri::EventLoopMessage>;

pub type App = tauri::App<Cef>;
pub type AppHandle = tauri::AppHandle<Cef>;
pub type WebviewWindow = tauri::WebviewWindow<Cef>;
