//! the ducktape desktop shell: a pure ui over the node daemon.
//!
//! the node no longer lives in this process. the shell's jobs beyond the
//! webview are the `~/.ducktape` WORKSPACE REGISTRY (see [`workspaces`]) — found
//! or join networks, allocate ports, drive the node's onboarding verbs, and
//! spawn the selected workspace's node DETACHED (its own process group, stdio
//! to `daemon.log`) so it survives this app exiting — and the legacy single-node
//! `daemon_spawn` fallback it is retiring. everything else is the webview's: it
//! probes /v1/status, streams blocks over /v1/ws, and retires the node with
//! POST /v1/shutdown. no pid is tracked; the port is the node's identity.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod daemon;
mod forge_git;
mod huddle;
mod tray;
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
            workspaces::workspace_observer_remove,
            workspaces::workspace_demote,
            workspaces::workspace_request_leave,
            workspaces::workspace_forget,
            workspaces::workspace_select,
            workspaces::workspace_phase,
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

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
