//! The loopback JSON-lines bridge: a std thread running a current-thread tokio
//! runtime that binds `127.0.0.1:0`, publishes an `endpoint.json` in the
//! user-private runtime dir, and serves one [`Request`] per line via
//! [`tools::execute`](crate::tools::execute).
//!
//! Loopback only, dev-only. The endpoint file is removed on drop.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use crate::collect::SnapshotSlot;
use crate::logs::LogsHandle;
use crate::protocol::{Intent, Request, Response};
use crate::tools;

/// A command the bridge cannot satisfy itself and hands to the app inside its
/// `update()` loop (drained via [`AgentHandle::drain_ui`]).
pub enum UiCommand {
    /// Inject a curated semantic intent.
    Intent(Intent),
    /// Take a screenshot of `window`; the app fills `reply` with PNG bytes.
    Shot {
        window: String,
        reply: tokio::sync::oneshot::Sender<Vec<u8>>,
    },
}

/// Everything the tool handlers read/write, shared between the server thread
/// and the app.
pub struct Shared {
    pub snapshot: SnapshotSlot,
    pub state: Arc<Mutex<serde_json::Value>>,
    pub window_map: Arc<Mutex<HashMap<String, iced::window::Id>>>,
    pub logs: LogsHandle,
    pub ui_queue: Mutex<VecDeque<UiCommand>>,
}

impl Shared {
    fn new(logs: LogsHandle) -> Self {
        Self {
            snapshot: SnapshotSlot::default(),
            state: Arc::new(Mutex::new(serde_json::Value::Null)),
            window_map: Arc::new(Mutex::new(HashMap::new())),
            logs,
            ui_queue: Mutex::new(VecDeque::new()),
        }
    }

    /// Queues a UI command for the app to drain.
    pub fn push_ui(&self, cmd: UiCommand) {
        if let Ok(mut q) = self.ui_queue.lock() {
            q.push_back(cmd);
        }
    }
}

/// Handle the app holds for the process lifetime. Owns the endpoint file
/// (removed on drop); the server thread is a daemon reaped at process exit.
pub struct AgentHandle {
    shared: Arc<Shared>,
    endpoint_path: PathBuf,
    addr: SocketAddr,
}

impl AgentHandle {
    /// Boots the bridge: binds a loopback port, writes `endpoint.json`, and
    /// starts serving. `logs` is the reader half of the ring the app installed
    /// in its subscriber (see [`ring_layer`](crate::logs::ring_layer)).
    pub fn boot(app_id: &str, logs: LogsHandle) -> AgentHandle {
        Self::boot_with_cdp(app_id, logs, None)
    }

    /// Like [`AgentHandle::boot`], also publishing a CDP URL for embedded
    /// Chromium content (the Browser pane) in `endpoint.json`.
    pub fn boot_with_cdp(app_id: &str, logs: LogsHandle, cdp: Option<String>) -> AgentHandle {
        let shared = Arc::new(Shared::new(logs));
        let (tx, rx) = std::sync::mpsc::channel();
        let server_shared = Arc::clone(&shared);

        std::thread::Builder::new()
            .name("iced-agent-bridge".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build agent bridge runtime");
                rt.block_on(async move {
                    let listener = match TcpListener::bind("127.0.0.1:0").await {
                        Ok(l) => l,
                        Err(e) => {
                            let _ = tx.send(Err(e.to_string()));
                            return;
                        }
                    };
                    let addr = listener.local_addr().expect("bridge local_addr");
                    if tx.send(Ok(addr)).is_err() {
                        return;
                    }
                    serve(listener, server_shared).await;
                });
            })
            .expect("spawn agent bridge thread");

        let addr = rx
            .recv()
            .expect("agent bridge thread reported bind result")
            .expect("agent bridge bound loopback port");

        let endpoint_path = write_endpoint(app_id, addr.port(), cdp.as_deref());
        tracing::info!(target: "iced::agent", port = addr.port(), "agent bridge listening");

        AgentHandle {
            shared,
            endpoint_path,
            addr,
        }
    }

    /// Shared snapshot store the app's collector task fills each tick.
    pub fn snapshot_slot(&self) -> SnapshotSlot {
        Arc::clone(&self.shared.snapshot)
    }

    /// Curated app-state projection the app refreshes each tick.
    pub fn state_slot(&self) -> Arc<Mutex<serde_json::Value>> {
        Arc::clone(&self.shared.state)
    }

    /// The `"main"`/`"huddle"`/`"tray"` → window id map the app registers into.
    pub fn window_map(&self) -> Arc<Mutex<HashMap<String, iced::window::Id>>> {
        Arc::clone(&self.shared.window_map)
    }

    /// Drains queued UI commands for the app to apply inside `update()`.
    pub fn drain_ui(&self) -> Vec<UiCommand> {
        self.shared
            .ui_queue
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    /// The bound loopback address (for tests and diagnostics).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for AgentHandle {
    fn drop(&mut self) {
        // ponytail: leave the daemon thread to process exit; only the endpoint
        // file needs active cleanup so a stale endpoint never points nowhere.
        let _ = std::fs::remove_file(&self.endpoint_path);
        if let Some(dir) = self.endpoint_path.parent() {
            let _ = std::fs::remove_dir(dir);
        }
    }
}

async fn serve(listener: TcpListener, shared: Arc<Shared>) {
    loop {
        match listener.accept().await {
            Ok((stream, _peer)) => {
                let conn_shared = Arc::clone(&shared);
                tokio::spawn(handle_conn(stream, conn_shared));
            }
            Err(e) => {
                tracing::warn!(target: "iced::agent", reason = "accept_failed", error = %e);
                return;
            }
        }
    }
}

async fn handle_conn(stream: tokio::net::TcpStream, shared: Arc<Shared>) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(req) => {
                let id = req.id;
                match tools::execute(req.cmd, &shared).await {
                    Ok(result) => Response::ok(id, result),
                    Err(error) => Response::err(id, error),
                }
            }
            Err(e) => Response::err(0, format!("bad json: {e}")),
        };
        let mut out = serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"id":0,"ok":false,"result":null,"error":"serialize failed"}"#.into()
        });
        out.push('\n');
        if writer.write_all(out.as_bytes()).await.is_err() {
            break;
        }
    }
}

/// `${XDG_RUNTIME_DIR|TMPDIR|TMP}`, falling back to `/tmp`.
fn base_dir() -> PathBuf {
    for var in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP"] {
        if let Ok(value) = std::env::var(var)
            && !value.is_empty()
        {
            return PathBuf::from(value);
        }
    }
    PathBuf::from("/tmp")
}

/// Writes the discovery file and returns its path.
fn write_endpoint(app_id: &str, port: u16, cdp: Option<&str>) -> PathBuf {
    let dir = base_dir().join("iced-agent").join(app_id);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(target: "iced::agent", reason = "endpoint_dir_failed", error = %e);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    let path = dir.join("endpoint.json");
    let doc = serde_json::json!({
        "transport": "tcp",
        "host": "127.0.0.1",
        "port": port,
        "pid": std::process::id(),
        "cdp": cdp,
    });
    if let Err(e) = std::fs::write(&path, serde_json::to_vec_pretty(&doc).unwrap_or_default()) {
        tracing::warn!(target: "iced::agent", reason = "endpoint_write_failed", error = %e);
    }
    path
}
