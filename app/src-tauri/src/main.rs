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
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![daemon::daemon_spawn])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
