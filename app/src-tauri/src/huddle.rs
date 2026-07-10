//! The pop-out huddle window — a small companion window rendering the frontend
//! at `?view=huddle`. It is a pure event mirror of the main window's voice
//! session (protocol in app/src/console/store/huddle-window.ts); this module
//! owns the window lifecycle plus the native media-permission wiring every
//! huddle-capable webview needs (`allow_user_media`). Whatever way the window
//! dies — native close button, the pop-in control, or `huddle_pop_in` — the
//! Destroyed hook tells the main window so its in-app card comes back.

use tauri::{
    AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

const LABEL: &str = "huddle";
const CLOSED_EVENT: &str = "ducktape://huddle-closed";
// Sized for a real video surface (the window runs its own session now), not the
// old audio pill — but still compact and floating.
const WIDTH: f64 = 380.0;
const HEIGHT: f64 = 300.0;
const MIN_WIDTH: f64 = 300.0;
const MIN_HEIGHT: f64 = 220.0;

/// Grant the webview's mic/camera permission requests (Linux; no-op elsewhere).
///
/// WebKitGTK has no OS permission prompt: `getUserMedia` raises a
/// `permission-request` signal for the EMBEDDER to decide, and an unhandled
/// request is denied — so without this hook every huddle fails `NotAllowedError`
/// ("mic-denied") no matter what the user does. User-media requests only ever
/// originate from our own bundled console, where joining a huddle / enabling the
/// camera is itself the consent, so they are allowed; every other request kind
/// (geolocation, notifications, …) is left to WebKit's default deny. macOS and
/// Windows are untouched: WKWebView and WebView2 already run a real OS prompt.
pub fn allow_user_media<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    // WebKitGTK-only hook: CEF handles getUserMedia through its own permission
    // path, so under the `cef` feature this is a no-op (the wry webview handle
    // this closure needs doesn't exist on the CEF runtime).
    #[cfg(all(target_os = "linux", not(feature = "cef")))]
    return window.with_webview(|webview| {
        use webkit2gtk::glib::Cast;
        use webkit2gtk::{PermissionRequestExt, UserMediaPermissionRequest, WebViewExt};
        webview.inner().connect_permission_request(|_, request| {
            match request.downcast_ref::<UserMediaPermissionRequest>() {
                Some(media) => {
                    media.allow();
                    true
                }
                None => false,
            }
        });
    });
    #[cfg(any(not(target_os = "linux"), feature = "cef"))]
    {
        let _ = window;
        Ok(())
    }
}

/// Create (or re-show) the huddle window.
#[tauri::command]
pub fn huddle_pop_out<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(LABEL) {
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }
    let win = WebviewWindowBuilder::new(
        &app,
        LABEL,
        WebviewUrl::App("index.html?view=huddle".into()),
    )
    .title("Huddle")
    .inner_size(WIDTH, HEIGHT)
    .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
    .resizable(true)
    .maximizable(false)
    .minimizable(false)
    // A huddle is glanceable state you keep in view while working, so the popped
    // card floats above other windows and stays out of the taskbar/dock — a
    // proper always-on-top pill rather than another window to hunt for.
    .always_on_top(true)
    .skip_taskbar(true)
    .build()
    .map_err(|e| e.to_string())?;
    allow_user_media(&win).map_err(|e| e.to_string())?;

    let handle = app.clone();
    win.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            let _ = handle.emit_to("main", CLOSED_EVENT, ());
        }
    });
    Ok(())
}

/// Close the huddle window if it exists (idempotent).
#[tauri::command]
pub fn huddle_pop_in<R: Runtime>(app: AppHandle<R>) {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.close();
    }
}
