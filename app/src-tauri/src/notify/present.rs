use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;

use super::{engine::Sink, matchers::Notification};

/// The production notification sink backed by the desktop application.
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
