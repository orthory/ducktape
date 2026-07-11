//! Capability-free surface for executable gateway content.
//!
//! Publisher content never shares the privileged `main` webview: it renders
//! inline in the Browser pane as a multiwebview child. On the CEF runtime the
//! surface is its own renderer process, and navigation stays pinned to one
//! random, short-lived gateway-session origin.
//!
//! The surface records the `.duck` route it shows with [`crate::permissions`]
//! before it opens: the session origin is a random loopback token, so the
//! route is the only honest name a permission prompt can put in front of the
//! user.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::webview::NewWindowResponse;
use tauri::{Manager as _, WebviewUrl};

fn validate_session_url(value: &str) -> Result<(tauri::Url, String), String> {
    let url: tauri::Url = value
        .parse()
        .map_err(|error| format!("invalid gateway session URL: {error}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| "gateway session URL has no host".to_string())?;
    let token = host
        .strip_suffix(".localhost")
        .filter(|token| {
            token.len() == 32
                && token
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or_else(|| "gateway session URL has an invalid capability host".to_string())?
        .to_string();
    if url.scheme() != "http"
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("gateway session URL is not a bounded localhost HTTP origin".into());
    }
    Ok((url, token))
}

/// The `.duck` route a surface was opened for, as the UI addressed it. It names
/// the content in permission prompts, so it is bounded like any other input.
fn validate_site(title: &str) -> Result<(), String> {
    if title.is_empty() || title.len() > 128 || !title.is_ascii() {
        return Err("gateway site name must be bounded ASCII".into());
    }
    Ok(())
}

/// Browser-pane rect in main-window logical px, reported by the UI.
#[derive(Clone, Copy, serde::Deserialize)]
pub struct InlineRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

const INLINE_PREFIX: &str = "gateway-inline-";

#[derive(Clone)]
struct InlineState {
    url: tauri::Url,
    rect: InlineRect,
    ready: bool,
    wanted_visible: bool,
}

impl InlineState {
    fn new(url: tauri::Url, rect: InlineRect) -> Self {
        Self {
            url,
            rect,
            ready: false,
            wanted_visible: true,
        }
    }

    /// Returns whether the child must navigate. Re-selecting a loaded tab only
    /// reveals it; it must not reload or discard publisher page state.
    fn open(&mut self, url: tauri::Url, rect: InlineRect) -> bool {
        let changed = self.url != url;
        self.url = url;
        self.rect = rect;
        self.wanted_visible = true;
        if changed {
            self.ready = false;
        }
        changed
    }

    fn finish_loading(&mut self) -> Option<InlineRect> {
        self.ready = true;
        self.wanted_visible.then_some(self.rect)
    }
}

fn inline_states() -> &'static Mutex<HashMap<String, InlineState>> {
    static STATES: OnceLock<Mutex<HashMap<String, InlineState>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn inline_label(tab_id: &str) -> Result<String, String> {
    if tab_id.is_empty()
        || tab_id.len() > 32
        || !tab_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("gateway tab id must be bounded ASCII".into());
    }
    Ok(format!("{INLINE_PREFIX}{tab_id}"))
}

fn hide_other_inline_webviews(
    app: &crate::rt::AppHandle,
    keep: Option<&str>,
) -> Result<(), String> {
    {
        let mut states = inline_states()
            .lock()
            .expect("inline gateway state registry poisoned");
        for (label, state) in states.iter_mut() {
            state.wanted_visible = keep == Some(label.as_str());
        }
    }
    for (label, webview) in app.webviews() {
        if label.starts_with(INLINE_PREFIX) && keep != Some(label.as_str()) {
            webview
                .hide()
                .map_err(|error| format!("hide inactive gateway tab: {error}"))?;
        }
    }
    Ok(())
}

/// Open (or re-navigate) the inline gateway child webview at `rect`. The
/// `gateway-inline` label matches no capability, so the embedded child is
/// fully isolated from the app's command surface.
#[tauri::command]
pub async fn gateway_open_inline(
    app: crate::rt::AppHandle,
    url: String,
    title: String,
    tab_id: String,
    rect: InlineRect,
) -> Result<(), String> {
    use tauri::{LogicalPosition, LogicalSize, Manager as _};

    let (url, _token) = validate_session_url(&url)?;
    validate_site(&title)?;
    let label = inline_label(&tab_id)?;
    crate::permissions::note_gateway_site(&label, &title);

    hide_other_inline_webviews(&app, Some(&label))?;
    let position = LogicalPosition::new(rect.x, rect.y);
    let size = LogicalSize::new(rect.width, rect.height);
    if let Some(existing) = app.get_webview(&label) {
        let should_navigate = {
            let mut states = inline_states()
                .lock()
                .expect("inline gateway state registry poisoned");
            match states.entry(label.clone()) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().open(url.clone(), rect)
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(InlineState::new(url.clone(), rect));
                    true // a retired physical slot currently shows about:blank
                }
            }
        };
        place_inline_webview(&existing, position, size)?;
        if should_navigate {
            existing
                .hide()
                .map_err(|error| format!("hide reloading gateway view: {error}"))?;
            existing
                .navigate(url)
                .map_err(|error| format!("navigate inline gateway view: {error}"))?;
        } else {
            existing
                .show()
                .map_err(|error| format!("show inline gateway view: {error}"))?;
        }
        return Ok(());
    }

    let allowed_host = url.host_str().expect("validated host").to_string();
    let allowed_port = url.port().expect("validated port");
    inline_states()
        .lock()
        .expect("inline gateway state registry poisoned")
        .insert(label.clone(), InlineState::new(url.clone(), rect));
    let state_label = label.clone();
    let builder = tauri::webview::WebviewBuilder::new(label.clone(), WebviewUrl::External(url))
        .incognito(true)
        .devtools(false)
        // CEF paints its default surface before publisher CSS arrives. Match
        // the Browser pane while the child is kept offscreen until Finished.
        .background_color(tauri::webview::Color(252, 252, 252, 255))
        .on_page_load(move |webview, payload| {
            if payload.event() != tauri::webview::PageLoadEvent::Finished {
                return;
            }
            let visible_rect = {
                let mut states = inline_states()
                    .lock()
                    .expect("inline gateway state registry poisoned");
                states
                    .get_mut(&state_label)
                    .and_then(InlineState::finish_loading)
            };
            if let Some(rect) = visible_rect {
                let _ = place_inline_webview(
                    &webview,
                    tauri::LogicalPosition::new(rect.x, rect.y),
                    tauri::LogicalSize::new(rect.width, rect.height),
                );
                let _ = webview.show();
            }
        })
        .on_navigation(move |candidate| {
            candidate.scheme() == "http"
                && candidate.host_str() == Some(allowed_host.as_str())
                && candidate.port() == Some(allowed_port)
                && candidate.username().is_empty()
                && candidate.password().is_none()
        })
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false);
    let window = app
        .get_window("main")
        .ok_or_else(|| "main console window is unavailable".to_string())?;
    // A newly-created CEF child is visible immediately. Keep its pre-load
    // surface offscreen; the Finished callback moves it into the pane.
    if let Err(error) = window.add_child(
        builder,
        LogicalPosition::new(-10_000.0, -10_000.0),
        LogicalSize::new(1.0, 1.0),
    ) {
        inline_states()
            .lock()
            .expect("inline gateway state registry poisoned")
            .remove(&label);
        return Err(format!("open inline gateway view: {error}"));
    }
    Ok(())
}

/// Track the Browser pane as it resizes (ResizeObserver on the UI side).
#[tauri::command]
pub async fn gateway_inline_place(
    app: crate::rt::AppHandle,
    tab_id: String,
    rect: InlineRect,
) -> Result<(), String> {
    use tauri::{LogicalPosition, LogicalSize, Manager as _};

    let label = inline_label(&tab_id)?;
    let Some(existing) = app.get_webview(&label) else {
        return Ok(()); // already closed — a late resize is not an error
    };
    let ready = {
        let mut states = inline_states()
            .lock()
            .expect("inline gateway state registry poisoned");
        let Some(state) = states.get_mut(&label) else {
            return Ok(());
        };
        state.rect = rect;
        state.ready
    };
    if !ready {
        return Ok(());
    }
    place_inline_webview(
        &existing,
        LogicalPosition::new(rect.x, rect.y),
        LogicalSize::new(rect.width, rect.height),
    )
}

/// Close the inline gateway view (idempotent) — navigation away or view switch.
#[tauri::command]
pub async fn gateway_inline_close(app: crate::rt::AppHandle, tab_id: String) -> Result<(), String> {
    use tauri::Manager as _;
    let label = inline_label(&tab_id)?;
    inline_states()
        .lock()
        .expect("inline gateway state registry poisoned")
        .remove(&label);
    if let Some(existing) = app.get_webview(&label) {
        existing
            .close()
            .map_err(|error| format!("close inline gateway view: {error}"))?;
    }
    // The surface is gone: so are the permissions the user granted it.
    crate::permissions::forget_webview(&label);
    Ok(())
}

#[tauri::command]
pub async fn gateway_inline_hide_all(app: crate::rt::AppHandle) -> Result<(), String> {
    hide_other_inline_webviews(&app, None)
}

fn place_inline_webview(
    webview: &tauri::Webview<crate::rt::Cef>,
    position: tauri::LogicalPosition<f64>,
    size: tauri::LogicalSize<f64>,
) -> Result<(), String> {
    webview
        .set_bounds(tauri::Rect {
            position: position.into(),
            size: size.into(),
        })
        .map_err(|error| format!("place inline gateway view: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_window_accepts_only_exact_capability_origin_shape() {
        let (url, token) = validate_session_url(
            "http://0123456789abcdef0123456789abcdef.localhost:49152/path?q=1",
        )
        .unwrap();
        assert_eq!(url.port(), Some(49152));
        assert_eq!(token, "0123456789abcdef0123456789abcdef");
        for unsafe_url in [
            "https://0123456789abcdef0123456789abcdef.localhost:49152/",
            "http://localhost:49152/",
            "http://0123456789abcdef0123456789abcdef.localhost/",
            "http://0123456789abcdef0123456789abcdef.localhost:49152/#leak",
            "http://0123456789abcdef0123456789abcdeg.localhost:49152/",
            "http://0123456789abcdef0123456789abcdef.localhost.evil:49152/",
        ] {
            assert!(
                validate_session_url(unsafe_url).is_err(),
                "accepted {unsafe_url}"
            );
        }
    }

    #[test]
    fn inline_labels_are_scoped_to_bounded_tab_ids() {
        assert_eq!(inline_label("tab-12").unwrap(), "gateway-inline-tab-12");
        for invalid in [
            "",
            "../main",
            "tab space",
            "012345678901234567890123456789012",
        ] {
            assert!(inline_label(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn inline_state_reveals_same_page_without_reload_and_gates_new_loads() {
        let first: tauri::Url = "http://0123456789abcdef0123456789abcdef.localhost:49152/"
            .parse()
            .unwrap();
        let second: tauri::Url = "http://fedcba9876543210fedcba9876543210.localhost:49153/"
            .parse()
            .unwrap();
        let rect = InlineRect {
            x: 10.0,
            y: 20.0,
            width: 800.0,
            height: 600.0,
        };
        let mut state = InlineState::new(first.clone(), rect);

        assert!(state.finish_loading().is_some());
        state.wanted_visible = false;
        assert!(!state.open(first, rect), "tab selection must not reload");
        assert!(state.ready);

        assert!(state.open(second, rect), "a new session URL must navigate");
        assert!(!state.ready, "new navigation stays hidden until Finished");
        state.wanted_visible = false;
        assert!(
            state.finish_loading().is_none(),
            "a late load cannot reshow a hidden tab"
        );
    }
}
