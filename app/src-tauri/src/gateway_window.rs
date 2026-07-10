//! Capability-free native window for executable gateway content.
//!
//! Tauri cannot distinguish iframe IPC from main-frame IPC on Linux, so
//! publisher content must never share the privileged `main` WebView. This
//! window label matches no capability and navigation stays pinned to one
//! random, short-lived gateway-session origin.

use tauri::webview::{NewWindowResponse, WebviewWindowBuilder};
use tauri::{Manager as _, WebviewUrl};

const WINDOW_PREFIX: &str = "gateway-";

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

#[tauri::command]
pub fn gateway_open_window(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    url: String,
    title: String,
) -> Result<(), String> {
    crate::daemon::require_main_window(&window)?;
    let (url, token) = validate_session_url(&url)?;
    if title.is_empty() || title.len() > 128 || !title.is_ascii() {
        return Err("gateway window title must be bounded ASCII".into());
    }
    let label = format!("{WINDOW_PREFIX}{token}");
    if let Some(existing) = app.get_webview_window(&label) {
        existing
            .navigate(url)
            .map_err(|error| format!("reopen gateway session: {error}"))?;
        existing
            .show()
            .and_then(|_| existing.set_focus())
            .map_err(|error| format!("focus gateway window: {error}"))?;
        return Ok(());
    }

    // Keep at most one executable publisher window. Closing the previous view
    // drops its incognito storage and prevents stale sessions accumulating.
    for (label, existing) in app.webview_windows() {
        if label.starts_with(WINDOW_PREFIX) {
            existing
                .close()
                .map_err(|error| format!("close old gateway window: {error}"))?;
        }
    }

    let allowed_host = url.host_str().expect("validated host").to_string();
    let allowed_port = url.port().expect("validated port");
    WebviewWindowBuilder::new(&app, label, WebviewUrl::External(url))
        .title(title)
        .inner_size(1100.0, 760.0)
        .min_inner_size(720.0, 480.0)
        .resizable(true)
        .incognito(true)
        .devtools(false)
        .on_navigation(move |candidate| {
            candidate.scheme() == "http"
                && candidate.host_str() == Some(allowed_host.as_str())
                && candidate.port() == Some(allowed_port)
                && candidate.username().is_empty()
                && candidate.password().is_none()
        })
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false)
        .build()
        .map_err(|error| format!("open isolated gateway window: {error}"))?;
    Ok(())
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
}
