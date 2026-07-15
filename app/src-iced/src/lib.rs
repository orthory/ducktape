mod account_service;
mod adapters;
mod backend;
#[cfg(feature = "cef-browser")]
mod browser;
mod browser_chrome;
mod community_service;
mod desktop;
mod duck_url;
mod external_url;
mod forge_agents_service;
mod huddle_media;
mod huddle_session;
mod huddle_ui;
mod icons;
mod mac_tray;
mod module_host;
mod network_content;
mod notifications;
mod onboarding;
mod operator_service;
mod page_presence;
mod profile_service;
mod screen_service;
mod screens;
mod search;
mod shell;
mod terminal_contract;
mod terminal_service;
mod theme;
mod transport;
mod user_content_service;
mod view_api;
mod workspace_service;

/// Run the native shell from the normal executable on macOS/Linux.
///
/// On Windows the CEF bootstrap export below installs the sandbox context
/// before entering this function. A directly launched Rust executable is
/// rejected by `dispatch_helper_processes` because it has no such context.
pub fn run() -> iced::Result {
    // Credential refusal must precede CEF loading/helper dispatch, tracing,
    // native window creation, and every filesystem or node-control action.
    refuse_root_process();

    // Sandboxed macOS CEF helpers must enter seatbelt before loading the CEF
    // framework or initializing tracing. The browser process returns here and
    // continues into the native iced shell below.
    #[cfg(feature = "cef-browser")]
    browser::dispatch_helper_processes();

    // CEF requires the principal macOS application to implement
    // CefAppProtocol. Register the class before iced/Winit creates NSApp.
    #[cfg(all(feature = "cef-browser", target_os = "macos"))]
    browser::prepare_macos_application();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ducktape=info,ducktape_iced=info".into()),
        )
        .try_init()
        .ok();

    #[cfg(all(feature = "cef-browser", target_os = "linux"))]
    force_x11();
    #[cfg(all(feature = "cef-browser", target_os = "linux"))]
    assert_pinned_libcef();
    shell::run()
}

#[cfg(unix)]
fn refuse_root_process() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid returns the process credential and retains no pointers.
    if unsafe { geteuid() } == 0 {
        tracing::error!(
            target: "ducktape::desktop",
            reason = "root_process",
            "Ducktape is a rootless desktop app and will not run as root"
        );
        std::process::exit(77);
    }
}

#[cfg(windows)]
fn refuse_root_process() {
    // SAFETY: IsUserAnAdmin reads the current process token and retains no pointers.
    if unsafe { windows::Win32::UI::Shell::IsUserAnAdmin() }.as_bool() {
        tracing::error!(
            target: "ducktape::desktop",
            reason = "elevated_process",
            "Ducktape is a rootless desktop app and will not run as administrator"
        );
        std::process::exit(77);
    }
}

#[cfg(not(any(unix, windows)))]
fn refuse_root_process() {}

#[cfg(all(feature = "cef-browser", target_os = "linux"))]
fn assert_pinned_libcef() {
    unsafe extern "C" {
        fn cef_version_info(entry: std::os::raw::c_int) -> std::os::raw::c_int;
    }
    // SAFETY: CEF exposes immutable version integers through this entry point.
    let loaded = unsafe {
        (
            cef_version_info(0),
            cef_version_info(1),
            cef_version_info(2),
        )
    };
    let pinned = (
        cef::sys::CEF_VERSION_MAJOR,
        cef::sys::CEF_VERSION_MINOR,
        cef::sys::CEF_VERSION_PATCH,
    );
    if loaded != pinned {
        tracing::error!(
            target: "ducktape::browser",
            event = "cef_version_mismatch",
            loaded_major = loaded.0,
            loaded_minor = loaded.1,
            loaded_patch = loaded.2,
            pinned_major = pinned.0,
            pinned_minor = pinned.1,
            pinned_patch = pinned.2,
            "refusing a mismatched CEF runtime"
        );
        std::process::exit(70);
    }
}

#[cfg(all(feature = "cef-browser", target_os = "linux"))]
fn force_x11() {
    // SAFETY: this runs before iced, CEF, or their worker threads exist.
    unsafe {
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("WAYLAND_SOCKET");
    }
}

#[cfg(all(target_os = "windows", feature = "cef-browser"))]
#[allow(non_camel_case_types)]
pub type LPTSTR = *mut u16;

/// CEF M146+ bootstrap version record from `cef_version_info.h`.
#[cfg(all(target_os = "windows", feature = "cef-browser"))]
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct cef_version_info_t {
    size: usize,
    cef_version_major: i32,
    cef_version_minor: i32,
    cef_version_patch: i32,
    cef_commit_number: i32,
    chrome_version_major: i32,
    chrome_version_minor: i32,
    chrome_version_build: i32,
    chrome_version_patch: i32,
    sandbox_compat_hash: [std::ffi::c_char; 17],
}

#[cfg(all(target_os = "windows", feature = "cef-browser"))]
impl cef_version_info_t {
    fn matches_pinned_cef(&self) -> bool {
        // cef-dll-sys intentionally exposes the semantic CEF/Chromium
        // constants but not CEF_COMMIT_NUMBER. This number is part of the
        // exact Cargo pin `147.0.10+gd58e84d+chromium-147.0.7727.118` and must
        // move with that pin.
        const PINNED_CEF_COMMIT_NUMBER: i32 = 3512;
        const PINNED_SANDBOX_COMPAT_HASH: &[u8; 17] = b"2c7f1000da15f67f\0";
        self.size >= std::mem::size_of::<Self>()
            && self.cef_version_major == cef::sys::CEF_VERSION_MAJOR
            && self.cef_version_minor == cef::sys::CEF_VERSION_MINOR
            && self.cef_version_patch == cef::sys::CEF_VERSION_PATCH
            && self.cef_commit_number == PINNED_CEF_COMMIT_NUMBER
            && self.chrome_version_major == cef::sys::CHROME_VERSION_MAJOR
            && self.chrome_version_minor == cef::sys::CHROME_VERSION_MINOR
            && self.chrome_version_build == cef::sys::CHROME_VERSION_BUILD
            && self.chrome_version_patch == cef::sys::CHROME_VERSION_PATCH
            && self
                .sandbox_compat_hash
                .iter()
                .map(|byte| *byte as u8)
                .eq(PINNED_SANDBOX_COMPAT_HASH.iter().copied())
    }
}

/// Entrypoint loaded by CEF's M138+ `bootstrap.exe` on Windows.
///
/// The C ABI and argument order intentionally match `cef_sandbox_win.h`.
/// `bootstrap.exe` owns both pointer arguments for this blocking call.
#[cfg(all(target_os = "windows", feature = "cef-browser"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RunWinMain(
    hinstance: cef::sys::HINSTANCE,
    _command_line: LPTSTR,
    _show_command: i32,
    sandbox_info: *mut std::ffi::c_void,
    version_info: *mut cef_version_info_t,
) -> i32 {
    refuse_root_process();
    if sandbox_info.is_null() || version_info.is_null() {
        return 70;
    }
    // SAFETY: bootstrap owns a live cef_version_info_t for this blocking call.
    if !unsafe { &*version_info }.matches_pinned_cef() {
        return 70;
    }
    if browser::install_windows_bootstrap(hinstance, sandbox_info).is_err() {
        return 70;
    }

    match std::panic::catch_unwind(run) {
        Ok(Ok(())) => 0,
        Ok(Err(_)) => 1,
        Err(_) => 101,
    }
}
