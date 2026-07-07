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
            forge_git::forge_head,
            forge_git::forge_log,
            forge_git::forge_tree,
            forge_git::forge_read_file,
            forge_git::forge_diff,
            tray::tray_open_console,
            tray::tray_quit,
            huddle::huddle_pop_out,
            huddle::huddle_pop_in,
        ])
        // Menu-bar icon + popover (macOS only; a no-op on other platforms).
        .setup(|app| {
            tray::init(app.handle())?;
            Ok(())
        });

    // dev-only debug bridge (tauri-plugin-mcp): opens a local unix socket so a
    // helper can screenshot the window, run JS in the webview, and drive input —
    // the way to see/verify the real native UI on a headless box. gated to
    // debug + desktop; a release runtime never opens it. socket path overridable
    // via DUCKTAPE_TAURI_MCP_SOCKET (default /tmp/ducktape-tauri-mcp.sock — a
    // ducktape-specific name so a second Tauri app on the same box doesn't fight
    // over the generic /tmp/tauri-mcp.sock).
    #[cfg(all(debug_assertions, desktop))]
    {
        let socket_path = std::env::var_os("DUCKTAPE_TAURI_MCP_SOCKET")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/ducktape-tauri-mcp.sock"));
        builder = builder.plugin(tauri_plugin_mcp::init_with_config(
            tauri_plugin_mcp::PluginConfig::new("ducktape".to_string())
                .start_socket_server(true)
                .socket_path(socket_path),
        ));
    }

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            if let Err(err) = workspaces::stop_active_workspace_node(app) {
                eprintln!("app exit: could not stop active workspace node: {err}");
            }
        }
    });
}
