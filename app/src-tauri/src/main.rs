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
//! Linux/Windows there is no tray to hide into, so close quits. A real app
//! quit (tray Quit / Cmd-Q / OS exit / non-mac window close) stops the active
//! managed node through the workspace pidfile before the shell exits.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod daemon;
mod gateway_window;
mod enroll;
mod forge_git;
mod huddle;
mod menu;
mod notify;
mod tray;
mod user_identity;
mod workspaces;

// On CEF, non-browser processes (renderer/GPU/plugin) re-exec this same binary;
// the entry-point macro dispatches them to run_cef_helper_process and returns.
#[cfg(feature = "cef")]
type ShellRuntime = tauri::Cef;
#[cfg(not(feature = "cef"))]
type ShellRuntime = tauri::Wry;

#[cfg_attr(feature = "cef", tauri::cef_entry_point)]
fn main() {
    let node_control = daemon::NodeControl::new().expect("start desktop node-control actor");
    let mut builder = tauri::Builder::<ShellRuntime>::default()
        .manage(node_control)
        .invoke_handler(tauri::generate_handler![
            workspaces::workspace_list,
            workspaces::workspace_active,
            workspaces::gateway_route_bind,
            workspaces::gateway_route_unbind,
            workspaces::gateway_route_list,
            gateway_window::gateway_open_window,
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
            workspaces::workspace_phase,
            workspaces::workspace_log_tail,
            workspaces::workspace_runtime_facts,
            workspaces::user_identity_confirm_mnemonic,
            user_identity::user_identity_status,
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
            enroll::enroll_start,
            enroll::enroll_poll,
            enroll::enroll_cancel,
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
        ]);
    builder = builder.plugin(tauri_plugin_notification::init());
    builder = builder
        // Menu-bar icon + popover, and an app menu with no Cmd+W Close
        // Window so the webview owns the key (macOS only; no-ops elsewhere).
        .setup(|app| {
            tray::init(app.handle())?;
            // After tray::init: both stack window-event handlers on "main".
            notify::init(app.handle())?;
            menu::install(app)?;
            // The in-app huddle dock captures from THIS window, so the main
            // webview needs the same mic/camera grant as the pop-out
            // (Linux only; no-ops elsewhere — see huddle::allow_user_media).
            let main = tauri::Manager::get_webview_window(app, "main")
                .ok_or("main window missing (tauri.conf.json windows)")?;
            huddle::allow_user_media(&main)?;
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

    // dev-only debug bridge (tauri-plugin-agent): registers our agent debugger
    // so the `tauri-agent` CLI / MCP server can drive the real native UI —
    // semantic tree, input, DOM-SVG screenshots, logs — over an app-scoped
    // endpoint registry. Gated to debug + desktop; a release runtime never
    // registers it (and the inline server refuses to bind without the
    // allowReleaseSocket opt-in, which we never set). Inline-server config lives
    // in tauri.conf.json under `plugins.agent`. The endpoint publishes to
    // ${XDG_RUNTIME_DIR|TMPDIR|TMP}/tauri-agent/com.ducktape.app/endpoint.json;
    // set XDG_RUNTIME_DIR per instance to isolate parallel worktree apps.
    // tauri-plugin-agent drives the wry webview; skip it on the CEF probe until
    // its runtime compat is verified separately.
    #[cfg(all(debug_assertions, desktop, not(feature = "cef")))]
    {
        builder = builder.plugin(tauri_plugin_agent::init());
    }

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            // The notify stream task is detach-on-drop; a real quit is the
            // one place that asks its loop to exit.
            if let Some(notify) = tauri::Manager::try_state::<notify::NotifyHandles>(app) {
                notify.stream.shutdown();
            }
            if let Err(err) = workspaces::stop_active_workspace_node(app) {
                eprintln!("app exit: could not stop active workspace node: {err}");
            }
        }
    });
}
