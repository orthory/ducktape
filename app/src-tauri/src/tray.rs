//! Menu-bar (system tray) icon + the popover window it toggles — macOS only.
//!
//! Left-clicking the icon shows the frameless `tray` window (rendered by the
//! frontend at `?view=tray`), anchored under the icon; clicking away (focus
//! lost) hides it. The native menu (right-click) offers Open + Quit. Closing the
//! main window hides it to the tray instead of quitting, so the app keeps living
//! in the menu bar.
//!
//! The whole feature is gated to macOS: on other platforms `init` is a no-op —
//! no tray icon, no popover window is created — and the two commands become
//! harmless no-ops (nothing ever invokes them, since only the macOS-only tray
//! window renders the popover that calls them).

use tauri::{AppHandle, Manager, Runtime};

// ── macOS implementation ────────────────────────────────
#[cfg(target_os = "macos")]
mod imp {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use tauri::image::Image;
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::utils::config::WindowEffectsConfig;
    use tauri::utils::{WindowEffect, WindowEffectState};
    use tauri::{
        AppHandle, Manager, PhysicalPosition, Position, Runtime, Theme, WebviewUrl,
        WebviewWindowBuilder, WindowEvent,
    };

    const POPOVER_W: f64 = 430.0;
    const POPOVER_H: f64 = 460.0;

    // When the popover was last hidden (ms since epoch). A tray-icon click while
    // the popover is open ALSO fires a focus-loss that hides it; recording the
    // hide lets the click's own toggle see "just hidden" and stay dismissed
    // instead of immediately reopening (the classic status-item toggle race).
    static LAST_HIDE_MS: AtomicU64 = AtomicU64::new(0);

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
    fn mark_hidden() {
        LAST_HIDE_MS.store(now_ms(), Ordering::Relaxed);
    }

    /// Build the popover window, tray icon, and wire behavior. Call once from setup.
    pub fn init<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
        // The popover webview (frameless, hidden until the icon is clicked).
        // Created here rather than in tauri.conf.json so non-macOS builds never
        // get it — the whole feature stays macOS-only.
        if app.get_webview_window("tray").is_none() {
            WebviewWindowBuilder::new(app, "tray", WebviewUrl::App("index.html?view=tray".into()))
                .title("Ducktape")
                .inner_size(POPOVER_W, POPOVER_H)
                .resizable(false)
                .decorations(false)
                // Native macOS vibrancy (dark glass) instead of an opaque panel:
                // transparent is required by `.effects(...)`, which paints the
                // real NSVisualEffectView "popover" material behind the webview.
                .transparent(true)
                .shadow(true)
                .theme(Some(Theme::Dark))
                .effects(WindowEffectsConfig {
                    effects: vec![WindowEffect::Popover],
                    state: Some(WindowEffectState::FollowsWindowActiveState),
                    radius: Some(12.0),
                    color: None,
                })
                .always_on_top(true)
                .skip_taskbar(true)
                .visible(false)
                .build()?;
        }

        let open = MenuItem::with_id(app, "tray_open", "Open Ducktape", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "tray_quit", "Quit Ducktape", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;

        // Click-away to dismiss: hide the popover when it loses focus.
        if let Some(popover) = app.get_webview_window("tray") {
            let w = popover.clone();
            popover.on_window_event(move |event| {
                if let WindowEvent::Focused(false) = event {
                    mark_hidden();
                    let _ = w.hide();
                }
            });
        }

        // Close-to-tray: closing the console hides it instead of quitting the app,
        // so it keeps living in the menu bar.
        if let Some(main) = app.get_webview_window("main") {
            let w = main.clone();
            main.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = w.hide();
                }
            });
        }

        // Dedicated full-bleed tray asset (icons/tray.png), NOT the dock/bundle
        // icon: the bundle icon's Apple-grid padding would shrink the duck inside
        // the fixed-height menu bar.
        let tray_icon =
            Image::from_bytes(include_bytes!("../icons/tray.png")).expect("decode tray icon png");
        TrayIconBuilder::with_id("ducktape")
            .icon(tray_icon)
            // Not `.icon_as_template(true)`: the icon is colored, and template mode
            // would render it as a solid black silhouette in the menu bar.
            .tooltip("Ducktape")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "tray_open" => super::show_main(app),
                "tray_quit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    position,
                    ..
                } = event
                {
                    toggle_popover(tray.app_handle(), position);
                }
            })
            .build(app)?;

        Ok(())
    }

    /// Toggle the popover, anchoring it just under the clicked tray icon.
    fn toggle_popover<R: Runtime>(app: &AppHandle<R>, icon: PhysicalPosition<f64>) {
        let Some(win) = app.get_webview_window("tray") else {
            return;
        };
        if win.is_visible().unwrap_or(false) {
            mark_hidden();
            let _ = win.hide();
            return;
        }
        // If the popover was hidden a moment ago, this same click is the one whose
        // focus-loss hid it — treat it as a dismiss rather than reopening.
        if now_ms().saturating_sub(LAST_HIDE_MS.load(Ordering::Relaxed)) < 250 {
            return;
        }

        let size = win.outer_size().ok();
        let width = size.map(|s| s.width as f64).unwrap_or(POPOVER_W);
        let height = size.map(|s| s.height as f64).unwrap_or(POPOVER_H);
        let mut x = icon.x - width / 2.0;
        let mut y = icon.y + 6.0;
        // Clamp into the monitor under the icon so the popover never runs off the
        // right/bottom edge.
        let monitor = win
            .available_monitors()
            .ok()
            .and_then(|ms| {
                ms.into_iter().find(|m| {
                    let p = m.position();
                    let s = m.size();
                    let (mx, my) = (p.x as f64, p.y as f64);
                    icon.x >= mx
                        && icon.x < mx + s.width as f64
                        && icon.y >= my
                        && icon.y < my + s.height as f64
                })
            })
            .or_else(|| win.primary_monitor().ok().flatten());
        if let Some(m) = monitor {
            let mp = m.position();
            let ms = m.size();
            let left = mp.x as f64 + 8.0;
            let right = (mp.x as f64 + ms.width as f64 - width - 8.0).max(left);
            x = x.clamp(left, right);
            let bottom = mp.y as f64 + ms.height as f64 - 8.0;
            if y + height > bottom {
                y = (bottom - height).max(mp.y as f64 + 8.0);
            }
        } else {
            x = x.max(8.0);
        }
        let _ = win.set_position(Position::Physical(PhysicalPosition {
            x: x as i32,
            y: y as i32,
        }));
        let _ = win.show();
        let _ = win.set_focus();
    }
}

// ── Cross-platform surface ──────────────────────────────

/// Build the tray icon + popover (macOS only; a no-op elsewhere). Call from setup.
#[cfg(target_os = "macos")]
pub fn init<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    imp::init(app)
}

#[cfg(not(target_os = "macos"))]
pub fn init<R: Runtime>(_app: &AppHandle<R>) -> tauri::Result<()> {
    Ok(())
}

pub(crate) fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Payload for [`tray_open_console`].
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenConsole {
    pub screen: Option<String>,
    /// Structured deep-link (screen + optional channel/thread/forge item),
    /// forwarded VERBATIM as the `ducktape://navigate` payload; the webview
    /// listener parses it. Takes precedence over the plain `screen` string.
    pub target: Option<serde_json::Value>,
}

/// Show the console (optionally navigating to a screen) and hide the popover.
/// Invoked by the popover's rows.
#[tauri::command]
pub fn tray_open_console<R: Runtime>(
    app: AppHandle<R>,
    request: Option<OpenConsole>,
) -> Result<(), String> {
    show_main(&app);
    if let Some(popover) = app.get_webview_window("tray") {
        let _ = popover.hide();
    }
    if let Some(request) = request {
        use tauri::Emitter;
        if let Some(target) = request.target {
            let _ = app.emit("ducktape://navigate", target);
        } else if let Some(screen) = request.screen {
            let _ = app.emit("ducktape://navigate", screen);
        }
    }
    Ok(())
}

/// Quit the whole app from the popover / tray menu.
#[tauri::command]
pub fn tray_quit<R: Runtime>(app: AppHandle<R>) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::OpenConsole;

    #[test]
    fn open_console_deserializes_the_empty_payload() {
        let request: OpenConsole = serde_json::from_str("{}").expect("empty object");
        assert!(request.screen.is_none());
        assert!(request.target.is_none());
    }

    #[test]
    fn open_console_deserializes_the_plain_screen_payload() {
        let request: OpenConsole =
            serde_json::from_str(r#"{"screen":"chat"}"#).expect("plain screen");
        assert_eq!(request.screen.as_deref(), Some("chat"));
        assert!(request.target.is_none());
    }

    #[test]
    fn open_console_deserializes_the_structured_target_payload() {
        let request: OpenConsole =
            serde_json::from_str(r#"{"target":{"screen":"chat","channelId":"general"}}"#)
                .expect("structured target");
        assert!(request.screen.is_none());
        assert_eq!(
            request.target,
            Some(serde_json::json!({"screen": "chat", "channelId": "general"})),
            "the structured target is carried verbatim"
        );
    }
}
