//! Open ordinary web links outside the privileged desktop process.

use reqwest::Url;

pub(crate) async fn open(raw: String) -> Result<(), String> {
    let url = validate(&raw)?;
    open_system_browser(&url)
}

fn validate(raw: &str) -> Result<String, String> {
    if raw.is_empty()
        || raw.len() > 2 * 1024
        || raw
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err("External link is not a valid web address.".into());
    }
    let url = Url::parse(raw).map_err(|_| "External link is not a valid web address.")?;
    let authority = raw
        .split_once("://")
        .map(|(_, rest)| rest.split(['/', '?', '#']).next().unwrap_or_default())
        .unwrap_or_default();
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || authority.is_empty()
        || authority.contains('@')
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Only HTTP and HTTPS links can open in the system browser.".into());
    }
    Ok(url.into())
}

#[cfg(target_os = "macos")]
fn open_system_browser(url: &str) -> Result<(), String> {
    spawn_and_reap("/usr/bin/open", url)
}

#[cfg(target_os = "linux")]
fn open_system_browser(url: &str) -> Result<(), String> {
    spawn_and_reap("/usr/bin/xdg-open", url)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn spawn_and_reap(program: &str, url: &str) -> Result<(), String> {
    let (send, receive) = std::sync::mpsc::sync_channel::<std::process::Child>(1);
    std::thread::Builder::new()
        .name("duck-link-reaper".into())
        .spawn(move || {
            if let Ok(mut child) = receive.recv() {
                let _ = child.wait();
            }
        })
        .map_err(|_| "Could not start the system-browser process reaper.".to_string())?;
    let child = std::process::Command::new(program)
        .arg(url)
        .spawn()
        .map_err(|_| "Could not open the link in the system browser.".to_string())?;
    if let Err(mut child) = send.send(child) {
        let _ = child.0.wait();
        return Err("Could not reap the system-browser process.".into());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_system_browser(url: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt as _;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{PCWSTR, w};

    let url = std::ffi::OsStr::new(url)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: all pointers are either null or live, NUL-terminated UTF-16 for
    // the duration of the call. ShellExecuteW retains none of them.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(url.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err("Could not open the link in the system browser.".into())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn open_system_browser(_url: &str) -> Result<(), String> {
    Err("Opening system browser links is unsupported on this platform.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_unambiguous_http_urls() {
        assert_eq!(
            validate("https://example.com/a?q=1#part").unwrap(),
            "https://example.com/a?q=1#part"
        );
        assert!(validate("http://example.com").is_ok());
        for invalid in [
            "duck://demo.duck",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://user:secret@example.com/",
            "https://example.com/has space",
            "https://example.com/has\nnewline",
            "https:///missing-host",
        ] {
            assert!(validate(invalid).is_err(), "accepted {invalid}");
        }
    }
}
