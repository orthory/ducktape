use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;

use super::{engine::Sink, matchers::Notification};

/// The production notification sink backed by the desktop application.
///
/// Toasts are title + body only. Click-navigation (a deep-link target per
/// notification) was deliberately NOT built: tauri-plugin-notification exposes
/// no toast click/action events on our platforms, so a target would feed
/// nothing. Don't re-grow that machinery until OS toast actions are wired;
/// the tray popover has its own navigate path (`tray_open_console`).
pub struct AppSink<R: Runtime>(pub AppHandle<R>);

impl<R: Runtime> Sink for AppSink<R> {
    fn present(&self, notification: &Notification) {
        if let Err(err) = self
            .0
            .notification()
            .builder()
            .title(&notification.title)
            .body(&notification.body)
            .show()
        {
            eprintln!("notify: could not show native notification: {err}");
        }
    }

    fn badge(&self, unread: u32) {
        if let Some(main) = self.0.get_webview_window("main") {
            let count = if unread == 0 {
                None
            } else {
                Some(unread as i64)
            };
            // WebKitGTK/Linux does not support badge counts, so its error is expected.
            let _ = main.set_badge_count(count);
            set_title_badge(&main, unread);
        }

        set_tray_title(&self.0, unread);
        // The sink cannot recover event delivery; log the frontend-contract failure and continue.
        if let Err(err) = self.0.emit(
            "ducktape://notify-unread",
            serde_json::json!({ "unread": unread }),
        ) {
            eprintln!("notify: could not emit unread update: {err}");
        }
    }
}

#[cfg(target_os = "macos")]
fn set_tray_title<R: Runtime>(app: &AppHandle<R>, unread: u32) {
    if let Some(tray) = app.tray_by_id("ducktape") {
        if unread == 0 {
            // Clearing the cosmetic tray title is best-effort.
            let _ = tray.set_title(None::<&str>);
        } else {
            // Updating the cosmetic tray title is best-effort.
            let _ = tray.set_title(Some(unread.to_string()));
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn set_tray_title<R: Runtime>(_app: &AppHandle<R>, _unread: u32) {}

/// The window title MUST match `tauri.conf.json`'s `app.windows[main].title` —
/// this restores it when the count clears.
#[cfg(target_os = "linux")]
const BASE_TITLE: &str = "Ducktape";

/// Linux's unread surface. It has neither of the other two: `set_badge_count`
/// is a no-op under WebKitGTK/CEF and the tray title is macOS-only, so without
/// this a backgrounded Linux app shows NO trace of an unread notification once
/// its transient toast is gone. The window title is the platform's badge — the
/// taskbar and window list render it (the convention Slack/Discord/browsers
/// follow), and it survives the app being hidden behind another window, which
/// is the only time unread is ever non-zero (focus marks everything seen).
#[cfg(target_os = "linux")]
fn set_title_badge<R: Runtime>(main: &tauri::WebviewWindow<R>, unread: u32) {
    // A cosmetic title is best-effort.
    let _ = main.set_title(&title_for(unread));
}

/// The titled badge itself — `set_title_badge`'s only branch, split out so it
/// is testable without a live window.
#[cfg(target_os = "linux")]
fn title_for(unread: u32) -> String {
    if unread == 0 {
        BASE_TITLE.to_owned()
    } else {
        format!("({unread}) {BASE_TITLE}")
    }
}

#[cfg(not(target_os = "linux"))]
fn set_title_badge<R: Runtime>(_main: &tauri::WebviewWindow<R>, _unread: u32) {}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::title_for;

    #[test]
    fn title_badges_unread_and_restores_the_base_title() {
        assert_eq!(title_for(0), "Ducktape");
        assert_eq!(title_for(3), "(3) Ducktape");
    }
}
