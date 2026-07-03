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
mod workspaces;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            daemon::daemon_spawn,
            workspaces::workspace_list,
            workspaces::workspace_active,
            workspaces::workspace_create,
            workspaces::workspace_join,
            workspaces::workspace_invite_blob,
            workspaces::workspace_admit,
            workspaces::workspace_select,
            workspaces::workspace_phase,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
