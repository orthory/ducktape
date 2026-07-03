//! the ducktape desktop shell: a pure ui over the node daemon.
//!
//! the node no longer lives in this process. the shell's one job beyond the
//! webview is `daemon_spawn`: launch `ducktape-noded` DETACHED — its own
//! process group, stdio to a log file — so it survives this app exiting (an
//! orphan). everything else is the webview's: it probes /v1/status to adopt a
//! daemon that is already running, polls after a spawn, streams blocks over
//! /v1/ws, and retires the daemon with POST /v1/shutdown. no pid is tracked;
//! the port is the daemon's identity.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod daemon;

fn main() {
    let mut builder = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![daemon::daemon_spawn]);

    // dev-only debug bridge (tauri-plugin-mcp): opens a local unix socket so a
    // helper can screenshot the window, run JS in the webview, and drive input —
    // the way to see/verify the real native UI on a headless box. gated to
    // debug + desktop; a release runtime never opens it. socket path overridable
    // via DUCKTAPE_TAURI_MCP_SOCKET (default /tmp/tauri-mcp.sock).
    #[cfg(all(debug_assertions, desktop))]
    {
        let socket_path = std::env::var_os("DUCKTAPE_TAURI_MCP_SOCKET")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/tauri-mcp.sock"));
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
