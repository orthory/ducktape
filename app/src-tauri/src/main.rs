//! the ducktape desktop shell: a pure ui over the node daemon.
//!
//! the node no longer lives in this process. the shell's jobs beyond the
//! webview are the `~/.ducktape` WORKSPACE REGISTRY (see [`workspaces`]) — found
//! or join networks, allocate ports, drive the node's onboarding verbs, and
//! spawn the selected workspace's node DETACHED (its own process group, stdio
//! to `daemon.log`) so closing the console window only hides to the menu-bar
//! app instead of killing the network. a real app quit (tray Quit / Cmd-Q /
//! OS exit) stops the active managed node through the workspace pidfile before
//! the shell exits.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod daemon;
mod forge_git;
mod huddle;
mod menu;
mod tray;
mod user_identity;
mod workspaces;

fn main() {
    let mut builder = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            daemon::daemon_spawn,
            workspaces::workspace_list,
            workspaces::workspace_active,
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
        ])
        // Menu-bar icon + popover, and an app menu with no Cmd+W Close
        // Window so the webview owns the key (macOS only; no-ops elsewhere).
        .setup(|app| {
            tray::init(app.handle())?;
            menu::install(app)?;
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
    #[cfg(all(debug_assertions, desktop))]
    {
        builder = builder.plugin(tauri_plugin_agent::init());
    }

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event
            && let Err(err) = workspaces::stop_active_workspace_node(app)
        {
            eprintln!("app exit: could not stop active workspace node: {err}");
        }
    });
}
