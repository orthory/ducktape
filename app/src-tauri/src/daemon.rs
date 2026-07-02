//! spawn the node daemon as a detached orphan.
//!
//! binary resolution, in order: the `DUCKTAPE_NODED_BIN` env override, then
//! `ducktape-noded` next to this executable — which covers BOTH builds: in dev
//! the workspace target dir holds both binaries side by side (run
//! `cargo build -p noded` once), and in the bundle tauri's externalBin places
//! the sidecar next to the app executable.
//!
//! detaching: on unix the child gets its own process group, so a terminal
//! Ctrl-C to `tauri dev` (or the app quitting) never signals it; on windows
//! DETACHED_PROCESS + CREATE_NEW_PROCESS_GROUP is the equivalent. stdio goes
//! to a log file under app-data. the tauri shell plugin's sidecar API is NOT
//! used on purpose — it kills children when the app exits.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use tauri::Manager as _;

/// launch `ducktape-noded --listen <listen>` detached, storage + log under the
/// OS app-data dir. returns the daemon's log path. the caller (webview) is
/// responsible for polling /v1/status until the daemon answers.
#[tauri::command]
pub fn daemon_spawn(app: tauri::AppHandle, listen: String) -> Result<String, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("no app-data dir: {err}"))?;
    let node_dir = data_dir.join("node");
    fs::create_dir_all(&node_dir).map_err(|err| format!("create {node_dir:?}: {err}"))?;

    let log_path = node_dir.join("daemon.log");
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|err| format!("open {log_path:?}: {err}"))?;
    let log_err = log.try_clone().map_err(|err| err.to_string())?;

    let binary = noded_binary()?;
    let mut cmd = Command::new(&binary);
    cmd.arg("--listen")
        .arg(&listen)
        .arg("--storage")
        .arg(node_dir.join("storage"))
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err);
    detach(&mut cmd);
    cmd.spawn()
        .map_err(|err| format!("spawn {binary:?}: {err}"))?;

    Ok(log_path.display().to_string())
}

/// find the daemon binary (see module docs for the resolution order).
fn noded_binary() -> Result<PathBuf, String> {
    if let Ok(explicit) = std::env::var("DUCKTAPE_NODED_BIN") {
        return Ok(PathBuf::from(explicit));
    }
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "app executable has no parent dir".to_string())?;
    let sibling = dir.join(format!("ducktape-noded{}", std::env::consts::EXE_SUFFIX));
    if sibling.exists() {
        return Ok(sibling);
    }
    Err(format!(
        "ducktape-noded not found at {} — build it with `cargo build -p noded` \
         or set DUCKTAPE_NODED_BIN",
        sibling.display()
    ))
}

#[cfg(unix)]
fn detach(cmd: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    // own process group: terminal signals aimed at the app never reach it
    cmd.process_group(0);
}

#[cfg(windows)]
fn detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}
