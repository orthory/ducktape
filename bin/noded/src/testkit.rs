//! An in-process node for e2e harnesses: a REAL [`host::Host`] on a dedicated
//! actor thread (the host is deliberately non-Send), fronted by [`crate::serve`]
//! on a loopback listener — the exact shape `bin/noded/src/main.rs`'s daemon
//! runs, minus the oracle/index/metrics machinery a focused test node has no
//! use for. A harness hands it a genesis closure and the module ids to surface
//! in `/v1/status`; the CLI/tool subprocess under test drives it over http like
//! any real daemon.
//!
//! This is the ONE definition of the minimal `NodeCommand` actor. The fs/mcp
//! CLI harnesses each used to hand-mirror this arm-for-arm (their own comments
//! admitted it), which drifts from the daemon it claims to stand in for. Now
//! they build a host and call [`InProcDaemon::start`].
//!
//! Gated behind the `testkit` feature so it never compiles into a shipping
//! node — consumers enable it as a dev-dependency feature only.

use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use futures::channel::mpsc;
use host::{BlockContext, Host};
use sdk::{Msg, Origin};

use crate::{
    BlockSummary, ModuleCategory, ModuleStatus, NodeCommand, NodeHandle, NodeStatus, hex_root,
};

/// a running in-process node: an actor thread owning the host and a server
/// thread serving the client surface, torn down cleanly on drop.
pub struct InProcDaemon {
    port: u16,
    /// this harness's admin credential. The harness OWNS the node in-process, so
    /// it is the operator — there is no workspace to write the token into, and
    /// nothing outside this struct ever learns it.
    operator_token: String,
    server: Option<JoinHandle<()>>,
    actor: Option<JoinHandle<()>>,
}

impl InProcDaemon {
    /// stand the node up: build the host on the actor thread via `build_host`
    /// (genesis holds non-Send qmdb handles, so it runs THERE, not on the
    /// caller), serve the router on the server thread, and block until
    /// `/v1/status` answers. `status_modules` are the ids reported in the
    /// status projection (each module's root read straight from the host).
    pub fn start(
        build_host: impl FnOnce() -> Host + Send + 'static,
        status_modules: Vec<String>,
    ) -> Self {
        let port = nettest::free_port();
        let (handle, cmd_rx, _events) = NodeHandle::channel();
        let status = handle.status_cell();
        // the testkit has no mesh and no registry: an empty exposition
        // parses to the honest empty peers sample and an empty scrape.
        status.wire_exposition(String::new);
        // the admin namespace is operator-gated, and this harness serves on a
        // REAL loopback port — so it mints a real per-instance credential rather
        // than a shared literal any other local process could guess.
        let operator_token = crate::admin::new_operator_token();
        let handle = handle.with_admin(crate::AdminConfig {
            operator_token: Some(operator_token.clone()),
            ..Default::default()
        });

        // the readiness event, same contract as `bin/noded`'s daemon: genesis
        // runs on the actor thread and publishes the boot snapshot, and only
        // THEN does the listener bind. Serving first would let `await_ready`
        // (and any harness probing `/v1/status` for "up") take a 200 carrying
        // `NodeStatus::default()` — version "", no modules, height 0 — as the
        // node's real state, while genesis is still running behind it.
        let (booted_tx, booted_rx) = std::sync::mpsc::sync_channel::<()>(1);
        let actor = std::thread::Builder::new()
            .name("inproc-actor".into())
            .spawn(move || run_actor(build_host(), status_modules, cmd_rx, status, booted_tx))
            .expect("spawn actor");
        // a dropped sender means genesis panicked; say that rather than block.
        booted_rx.recv().expect("in-proc actor died during genesis");

        let server = std::thread::Builder::new()
            .name("inproc-server".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("server runtime");
                rt.block_on(async move {
                    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
                        .await
                        .expect("bind harness listener");
                    let _ = crate::serve(listener, handle).await;
                });
            })
            .expect("spawn server");

        let daemon = Self {
            port,
            operator_token,
            server: Some(server),
            actor: Some(actor),
        };
        daemon.await_ready();
        daemon
    }

    /// the port the client surface listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// the http base a CLI's `--node` flag (or `DUCKTAPE_NODE`) takes.
    pub fn node_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn await_ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if nettest::http_status(self.port, "GET", "/v1/status") == Some(200) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "in-proc node never answered /v1/status on port {}",
                self.port
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for InProcDaemon {
    fn drop(&mut self) {
        // graceful shutdown over the wire (NOT a held handle clone — that would
        // keep the command channel's Sender alive and wedge the actor's drain
        // loop open, hanging the join below). the POST makes serve() return →
        // its sole handle drops → the command channel closes → both threads end,
        // so a caller's tempdir (dropped AFTER this) is removed only once the
        // host's qmdb handles are closed.
        let _ = nettest::http_status_with(
            self.port,
            "POST",
            "/v1/admin/shutdown",
            &[(crate::admin::ADMIN_TOKEN_HEADER, &self.operator_token)],
        );
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
        if let Some(actor) = self.actor.take() {
            let _ = actor.join();
        }
    }
}

/// the minimal node actor: one block per submit, queries answered inline — the
/// SAME [`NodeCommand`] contract `bin/noded`'s daemon serves, so a harness
/// proves the real submit/query/frame path rather than a hand-mirrored stub.
pub fn run_actor(
    mut host: Host,
    status_modules: Vec<String>,
    mut cmd_rx: mpsc::Receiver<NodeCommand>,
    status: crate::StatusCell,
    booted: std::sync::mpsc::SyncSender<()>,
) {
    let mut height: u64 = 0;
    futures::executor::block_on(async move {
        // the boot snapshot, then one publish per committed block — the SAME
        // publish-into-the-cell contract the real daemons serve. the signal
        // releases the caller to bind: the first status a client can reach is
        // this one, never the cell's empty default.
        publish_status(&status, &host, &status_modules, height);
        let _ = booted.send(());
        while let Some(cmd) = cmd_rx.next().await {
            match cmd {
                NodeCommand::Submit {
                    target,
                    payload,
                    origin,
                    reply,
                } => {
                    let result =
                        commit(&mut host, &mut height, Origin::External(origin), Msg { target, payload })
                            .await;
                    publish_status(&status, &host, &status_modules, height);
                    let _ = reply.send(result);
                }
                // the signed-frame lane, FAITHFUL to the real daemon: the origin
                // is the frame's VERIFIED signer, never a caller claim. a
                // forged/tampered frame is a rejection, not a block.
                NodeCommand::SubmitFrame { frame, reply } => {
                    let result = match node::decode_frame(&frame) {
                        Ok((origin, msg, _cont)) => {
                            commit(&mut host, &mut height, origin, msg).await
                        }
                        Err(err) => Err(err.to_string()),
                    };
                    publish_status(&status, &host, &status_modules, height);
                    let _ = reply.send(result);
                }
                NodeCommand::Query { target, req, reply } => {
                    let result = host.query(&target, &req).await.map_err(|e| e.to_string());
                    let _ = reply.send(result);
                }
            }
        }
    });
}

/// assemble + publish the harness's `/v1/status` snapshot (each module's root
/// read straight from the host) into the shared cell.
fn publish_status(
    status: &crate::StatusCell,
    host: &Host,
    status_modules: &[String],
    height: u64,
) {
    let modules = status_modules
        .iter()
        .map(|id| ModuleStatus {
            id: id.clone(),
            root: host.module_root(id).map(|r| hex_root(&r)).unwrap_or_default(),
            category: ModuleCategory::of(id),
        })
        .collect();
    status.publish(NodeStatus {
        version: env!("CARGO_PKG_VERSION").into(),
        root_hash: hex_root(&host.root_hash()),
        height,
        modules,
        public_key: String::new(),
        operations: Default::default(),
    });
    // no mesh: height only, no roles or epoch — same as the embedded daemon.
    status.publish_peers(crate::PeersStanding {
        height,
        ..Default::default()
    });
}

/// apply one op as its own block, advancing `height` only on a clean commit — a
/// rejected op was never a block, so the height (and the reply) reports its
/// consensus fate faithfully.
async fn commit(
    host: &mut Host,
    height: &mut u64,
    origin: Origin,
    msg: Msg,
) -> Result<BlockSummary, String> {
    let next = *height + 1;
    let ctx = BlockContext {
        height: next,
        consensus_time: next,
        origin,
    };
    match host.submit_at(ctx, msg).await {
        Ok(out) => {
            *height = next;
            Ok(BlockSummary {
                height: *height,
                root_hash: hex_root(&out.root_hash),
            })
        }
        Err(err) => Err(err.to_string()),
    }
}
