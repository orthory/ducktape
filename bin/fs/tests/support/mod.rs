//! an in-process duckfs node for the CLI e2e: a REAL `host::Host::genesis` with
//! ONLY the files module on a dedicated thread pumping `NodeCommand`s (the shape
//! of `bin/noded/src/main.rs`'s actor loop, minus the oracle/index/metrics
//! machinery a files-only node has no use for), fronted by `noded::router` on a
//! local tokio listener. the CLI subprocess (`env!(CARGO_BIN_EXE_ducktape-fs)`)
//! drives it over http exactly as it would a real daemon.
//!
//! shut down cleanly on drop (POST /v1/shutdown → serve returns → the handle
//! drops → the actor's command channel closes → both threads join) so the
//! tempdir is deleted only once the host's qmdb handles are closed.
#![allow(dead_code)]

use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::process::Command;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use futures::channel::mpsc;
use host::{BlockContext, Host};
use noded::{BlockSummary, ModuleCategory, ModuleStatus, NodeCommand, NodeHandle, NodeStatus};
use sdk::{Msg, Origin};

/// a running in-process node plus the CLI-under-test's path.
pub struct Harness {
    port: u16,
    dir: Option<tempfile::TempDir>,
    server: Option<JoinHandle<()>>,
    actor: Option<JoinHandle<()>>,
}

impl Harness {
    /// stand up the node: genesis the files module on the actor thread, serve the
    /// router on the server thread, and block until `/v1/status` answers.
    pub fn start() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("ducktape-fs-e2e")
            .tempdir()
            .expect("harness tempdir");
        let duckfs_dir = dir.path().join("duckfs");
        let port = free_port();

        let (handle, cmd_rx, _events) = NodeHandle::channel();

        // the actor thread: owns the host, drains commands one block per submit.
        let actor = std::thread::Builder::new()
            .name("fs-e2e-actor".into())
            .spawn(move || run_actor(duckfs_dir, cmd_rx))
            .expect("spawn actor");

        // the server thread: serves the client surface on its own tokio runtime.
        let server = std::thread::Builder::new()
            .name("fs-e2e-server".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("server runtime");
                rt.block_on(async move {
                    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
                        .await
                        .expect("bind harness listener");
                    let _ = noded::serve(listener, handle).await;
                });
            })
            .expect("spawn server");

        let harness = Harness {
            port,
            dir: Some(dir),
            server: Some(server),
            actor: Some(actor),
        };
        harness.await_ready();
        harness
    }

    /// the http base the CLI's `--node` flag takes.
    pub fn node_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// a `ducktape-fs` invocation pre-pointed at this node via `--node`.
    pub fn cli(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape-fs"));
        cmd.args(args).arg("--node").arg(self.node_url());
        cmd
    }

    /// a bare `ducktape-fs` invocation (no `--node`) — for the resolution-error
    /// and stub-verb cases.
    pub fn cli_bare(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape-fs"));
        cmd.args(args);
        cmd
    }

    fn await_ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if http_status(self.port, "GET", "/v1/status") == Some(200) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "harness node never answered /v1/status on port {}",
                self.port
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        // graceful shutdown: the serve future returns, the router (and its handle)
        // drop, the command channel closes, both threads end — so the tempdir is
        // removed only after qmdb is closed.
        let _ = http_status(self.port, "POST", "/v1/shutdown");
        if let Some(s) = self.server.take() {
            let _ = s.join();
        }
        if let Some(a) = self.actor.take() {
            let _ = a.join();
        }
        drop(self.dir.take());
    }
}

/// the files-only actor loop: one block per submit, queries answered inline.
fn run_actor(duckfs_dir: std::path::PathBuf, mut cmd_rx: mpsc::Receiver<NodeCommand>) {
    let files = files::Files::open("files", duckfs_dir).expect("open files");
    let mut host = Host::genesis(vec![Box::new(files)]).expect("genesis");
    let mut height: u64 = 0;
    futures::executor::block_on(async move {
        while let Some(cmd) = cmd_rx.next().await {
            match cmd {
                NodeCommand::Submit {
                    target,
                    payload,
                    origin,
                    reply,
                } => {
                    // a rejected op is NOT a block: the height only advances on a
                    // clean commit (mirrors the noded actor).
                    let next = height + 1;
                    let ctx = BlockContext {
                        protocol_version: 0,
                        height: next,
                        consensus_time: next,
                        origin: Origin::External(origin),
                    };
                    let result = match host.submit_at(ctx, Msg { target, payload }).await {
                        Ok(out) => {
                            height = next;
                            Ok(BlockSummary {
                                height,
                                app_hash: noded::hex_root(&out.app_hash),
                            })
                        }
                        Err(err) => Err(err.to_string()),
                    };
                    let _ = reply.send(result);
                }
                // the duckfs CLI submits frameless (the daemon's trusted-client
                // lane); this files-only actor has no agent, no session key, and
                // nothing that signs — so the frame lane is refused rather than
                // faked.
                NodeCommand::SubmitFrame { reply, .. } => {
                    let _ = reply.send(Err(
                        "the files test actor serves no signed-frame lane".into()
                    ));
                }
                NodeCommand::Query { target, req, reply } => {
                    let result = host.query(&target, &req).await.map_err(|e| e.to_string());
                    let _ = reply.send(result);
                }
                NodeCommand::Status { reply } => {
                    let _ = reply.send(NodeStatus {
                        version: env!("CARGO_PKG_VERSION").into(),
                        app_hash: noded::hex_root(&host.app_hash()),
                        height,
                        modules: vec![ModuleStatus {
                            id: "files".into(),
                            root: host
                                .module_root("files")
                                .map(|r| noded::hex_root(&r))
                                .unwrap_or_default(),
                            category: ModuleCategory::of("files"),
                        }],
                        public_key: String::new(),
                    });
                }
                NodeCommand::Metrics { reply } => {
                    let _ = reply.send(String::new());
                }
            }
        }
    });
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind port probe")
        .local_addr()
        .expect("probe addr")
        .port()
}

/// a minimal raw-http request that returns just the status code (or `None` if
/// the node is not up yet) — enough to poll readiness and post the shutdown.
fn http_status(port: u16, method: &str, path: &str) -> Option<u16> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let req = format!(
        "{method} {path} HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw);
    text.split_whitespace().nth(1).and_then(|s| s.parse().ok())
}
