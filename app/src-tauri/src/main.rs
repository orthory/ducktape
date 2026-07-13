//! the ducktape desktop shell: a pure ui over the node daemon.
//!
//! the node no longer lives in this process. the shell's jobs beyond the
//! webview are the `~/.ducktape` WORKSPACE REGISTRY (see [`workspaces`]) — found
//! or join networks and allocate ports. A bounded, dedicated node-control actor
//! drives onboarding/custody verbs and spawns the selected workspace's node
//! DETACHED (its own process group, stdio to `daemon.log`), so Tauri runtime
//! workers never execute or wait on the node binary. On macOS, closing the
//! console window only hides to the menu-bar app instead of killing the
//! network (close-to-hide is wired in the mac-gated tray init); on
//! Linux/Windows there is no tray to hide into, so close quits. The managed
//! node is deliberately detached and outlives every shell exit; only explicit
//! workspace Stop/Forget actions may tear it down. A later shell adopts the
//! listener through the workspace registry.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod daemon;
mod duck_scheme;
mod gateway_window;
mod enroll;
mod forge_git;
mod huddle;
mod lan_http;
mod link_relay;
mod menu;
mod notify;
mod permissions;
mod rt;
mod sandbox;
mod touchid;
mod tray;
mod user_identity;
mod workspaces;

/// Refuse to run against a libcef other than the pinned distribution.
///
/// CEF's API-versioning lets a binary built for one CEF boot on another major
/// version, with whole features silently absent instead of an error — a
/// system cef-vaapi-bin 150 loads fine but has no GTK input-method
/// integration, so fcitx/IME just goes dead. The DT_RPATH set in build.rs
/// makes the binary prefer the runtime staged beside it, but if those files
/// are missing ld.so falls back to LD_LIBRARY_PATH; turn that silent
/// degradation into an actionable abort, in the browser process and every
/// re-exec'd CEF subprocess alike.
#[cfg(all(target_os = "linux", any(feature = "cef", feature = "dev-cef")))]
fn assert_pinned_libcef() {
    unsafe extern "C" {
        // cef-dll-sys does not bind cef_version_info; libcef.so is already a
        // link-time dependency, so declare the one entry point directly.
        fn cef_version_info(entry: std::os::raw::c_int) -> std::os::raw::c_int;
    }
    // SAFETY: cef_version_info reads static version data; no preconditions.
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
        eprintln!(
            "ducktape-desktop: loaded libcef.so is CEF {}.{}.{}, but this binary is built \
             for CEF {}.{}.{}. A mismatched CEF boots but silently loses features (IME, \
             input methods). Run the binary installed by `make install-app` (which stages \
             the pinned CEF runtime beside it), and do not point LD_LIBRARY_PATH at a \
             system CEF such as /usr/lib/cef.",
            loaded.0, loaded.1, loaded.2, pinned.0, pinned.1, pinned.2,
        );
        std::process::exit(70);
    }
}

/// Point the `duck://` scheme handler at the active node's browser-gateway
/// base (from `/v1/gateway/browser`). The frontend calls this when a workspace
/// becomes active; `None` clears it.
#[tauri::command]
fn duck_set_gateway_base(base: Option<String>) {
    duck_scheme::set_gateway_base(base);
}

/// CEF switches. Production needs only the keychain opt-outs; a headless QA run
/// appends `DUCKTAPE_CEF_EXTRA_ARGS` (e.g.
/// `--single-process --disable-gpu --use-gl=angle --use-angle=swiftshader
/// --enable-unsafe-swiftshader --no-sandbox`), which a real desktop never sets.
fn cef_command_line_args() -> Vec<(String, Option<String>)> {
    let mut args = vec![
        ("--use-mock-keychain".to_string(), None),
        ("password-store".to_string(), Some("basic".to_string())),
    ];
    if let Ok(extra) = std::env::var("DUCKTAPE_CEF_EXTRA_ARGS") {
        for arg in extra.split_whitespace() {
            match arg.split_once('=') {
                Some((key, value)) => args.push((key.to_string(), Some(value.to_string()))),
                None => args.push((arg.to_string(), None)),
            }
        }
    }
    args
}

fn main() {
    #[cfg(all(target_os = "linux", any(feature = "cef", feature = "dev-cef")))]
    assert_pinned_libcef();

    // Must precede the helper dispatch below AND tauri boot: CEF subprocesses
    // re-exec this binary and need the same custom-scheme registration, and
    // the browser process reads the switches/cache identifier at runtime init.
    tauri_runtime_cef::configure(tauri_runtime_cef::CefConfig {
        identifier: "com.ducktape.app".into(),
        // Keep Chromium's local-data encryption OFF the OS keychain: os_crypt
        // otherwise prompts macOS for "Chromium Safe Storage" Keychain access
        // (and would hit kwallet/gnome-keyring on Linux). The key only guards
        // Chromium's own cookie/localStorage store — worthless here: the
        // console is a localhost UI whose real data lives in the node, and
        // gateway sessions are incognito. Deliberate: web-content storage is
        // not keychain-protected.
        command_line_args: cef_command_line_args(),
        // `duck` renders gateway routes at stable origins (spec §1). It is a
        // standard+secure+CORS+fetch scheme with a real cookie jar (the jar
        // needs `cookieable_schemes` on both the global settings and every
        // per-webview request context — the crate wires both).
        custom_schemes: vec![
            "tauri".into(),
            "ipc".into(),
            "asset".into(),
            "duck".into(),
        ],
        cookieable_schemes: vec!["duck".into()],
        ..Default::default()
    });

    // Deny-by-default Chromium permission policy (camera, microphone, screen,
    // clipboard, geolocation, …), installed before the runtime can field a
    // single request. The rules live in `permissions`.
    permissions::install_policy();

    // CEF non-browser processes (renderer/GPU/plugin) re-exec this same
    // binary; dispatch them to the CEF helper before any app setup.
    if std::env::args().any(|arg| arg.starts_with("--type=")) {
        tauri_runtime_cef::run_cef_helper_process();
        return;
    }

    // The streaming `duck://` handler (process-global). Registered before the
    // builder so the first gateway navigation is served.
    duck_scheme::register();

    let node_control = daemon::NodeControl::new().expect("start desktop node-control actor");
    let mut builder = tauri::Builder::<rt::Cef>::new()
        .manage(node_control)
        // The streaming handler above owns every duck:// request; this buffered
        // registration only exists so the runtime installs the scheme factory
        // and per-webview registry entry (its handler is never called).
        .register_uri_scheme_protocol("duck", |_ctx, _request| {
            tauri::http::Response::builder()
                .status(500)
                .body(std::borrow::Cow::Borrowed(&b"duck streaming handler unreachable"[..]))
                .unwrap()
        })
        .invoke_handler(tauri::generate_handler![
            workspaces::workspace_list,
            workspaces::workspace_active,
            workspaces::gateway_route_bind,
            workspaces::gateway_route_unbind,
            workspaces::gateway_route_list,
            gateway_window::gateway_open_inline,
            gateway_window::gateway_inline_place,
            gateway_window::gateway_inline_close,
            gateway_window::gateway_inline_hide_all,
            duck_set_gateway_base,
            workspaces::workspace_create,
            workspaces::workspace_join,
            workspaces::workspace_invite_blob,
            workspaces::workspace_join_requests,
            workspaces::workspace_admit,
            workspaces::workspace_promote,
            workspaces::workspace_resident_remove,
            workspaces::workspace_demote,
            workspaces::workspace_request_leave,
            workspaces::workspace_forget,
            workspaces::workspace_select,
            workspaces::workspace_sandbox_apply,
            workspaces::workspace_phase,
            workspaces::workspace_log_tail,
            workspaces::workspace_runtime_facts,
            workspaces::user_identity_confirm_mnemonic,
            user_identity::user_identity_state,
            user_identity::user_identity_create,
            user_identity::user_identity_restore,
            user_identity::user_identity_unlock,
            user_identity::user_identity_reveal,
            user_identity::user_identity_encrypt,
            user_identity::user_identity_lock,
            user_identity::user_sign_bind,
            user_identity::user_sign_unbind,
            user_identity::user_sign_gateway_route,
            user_identity::user_sign_possession,
            user_identity::user_sign_add_member,
            user_identity::user_sign_remove_member,
            user_identity::user_sign_files_frame,
            touchid::touchid_available,
            touchid::touchid_enroll,
            touchid::touchid_enrolled,
            touchid::touchid_unlock,
            touchid::touchid_disable,
            enroll::enroll_start,
            enroll::enroll_poll,
            enroll::enroll_cancel,
            link_relay::link_relay_start,
            link_relay::link_relay_poll,
            link_relay::link_relay_cancel,
            link_relay::link_fetch_challenge,
            link_relay::link_send_response,
            forge_git::forge_list_repos,
            forge_git::forge_list_branches,
            forge_git::forge_head,
            forge_git::forge_log,
            forge_git::forge_tree,
            forge_git::forge_read_file,
            forge_git::forge_read_file_page,
            forge_git::forge_diff,
            forge_git::forge_compare,
            forge_git::forge_build_merge,
            tray::tray_open_console,
            tray::tray_quit,
            huddle::huddle_pop_out,
            huddle::huddle_pop_in,
            notify::notify_configure,
            notify::notify_mark_seen,
            notify::notify_recent,
            permissions::permission_prompt_state,
            permissions::permission_prompt_decide,
            sandbox::sandbox_preflight,
        ]);
    builder = builder.plugin(tauri_plugin_notification::init());
    builder = builder.plugin(tauri_plugin_opener::init());
    builder = builder
        // Menu-bar icon + popover, and an app menu with no Cmd+W Close
        // Window so the webview owns the key (macOS only; no-ops elsewhere).
        .setup(|app| {
            tray::init(app.handle())?;
            // After tray::init: both stack window-event handlers on "main".
            notify::init(app.handle())?;
            menu::install(app)?;
            // The permission policy is already live; give it the handle it needs
            // to raise the native consent window when gateway content asks for
            // a device.
            permissions::attach(app.handle());
            let main = tauri::Manager::get_webview_window(app, "main")
                .ok_or("main window missing (tauri.conf.json windows)")?;
            // macOS overlays its native traffic lights on the in-app title
            // bar (titleBarStyle Overlay). Other desktops get the same
            // single-bar chrome by dropping native decorations; the title bar
            // hosts in-app window controls and the frame edges drive resize
            // (see WindowChrome.tsx).
            // ponytail: set at setup = one decorated first paint on launch;
            // move to per-platform tauri conf files if the flash matters.
            #[cfg(not(target_os = "macos"))]
            main.set_decorations(false)?;
            Ok(())
        });

    // dev-only debug bridge (tauri-agent-plugin): registers our agent debugger
    // so the `tauri-agent` CLI / MCP server can drive the real native UI —
    // semantic tree, input, DOM-SVG screenshots, logs — over an app-scoped
    // endpoint registry. Gated to debug + desktop; a release runtime never
    // registers it (and the inline server refuses to bind without the
    // allowReleaseSocket opt-in, which we never set). Inline-server config lives
    // in tauri.conf.json under `plugins.agent`. The endpoint publishes to
    // ${XDG_RUNTIME_DIR|TMPDIR|TMP}/tauri-agent/com.ducktape.app/endpoint.json;
    // set XDG_RUNTIME_DIR per instance to isolate parallel worktree apps.
    #[cfg(all(debug_assertions, desktop))]
    {
        builder = builder.plugin(tauri_agent_plugin::init());
    }

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    app.run(|app, event| match event {
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => {
            // Closing the console intentionally hides it into the menu bar.
            // Finder/Dock activation of the already-running app arrives here;
            // without this, `open -a Ducktape` leaves the process alive with
            // no visible window and reads as another launch failure.
            tray::show_main(app);
        }
        tauri::RunEvent::ExitRequested { .. } => {
            // The notify stream task is detach-on-drop; a real quit is the
            // one place that asks its loop to exit.
            if let Some(notify) = tauri::Manager::try_state::<notify::NotifyHandles>(app) {
                notify.stream.shutdown();
            }
            // The workspace node is a durable execution host, not a Tauri
            // child: it is deliberately left running so the next shell adopts
            // it, and so a dev rebuild or an ordinary quit never kills a live
            // agent provider. Verified teardown belongs to Stop/Forget alone.
        }
        _ => {}
    });
}
