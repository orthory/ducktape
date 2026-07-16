//! The sim lane: transaction round-trips against an EMBEDDED deterministic
//! simnode (`simnode::boot`) — the iced twin of the TS `app/src/test/sim/`
//! suites, foundry-style: plain `cargo test` needs no external binaries.
//! Design: docs/superpowers/specs/2026-07-16-iced-sim-lane-design.md (v2).

// In-process frame signing (dev-deps only → test builds only).
mod signing;

// The proof scenarios — chat round-trip + duplicate-channel rejection.
mod chat;
mod files;
mod governance;
mod members;
mod pages;

use std::collections::VecDeque;
use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use futures_util::StreamExt as _;
use iced_agent_plugin::Role;
use iced_agent_plugin::selector::by;
use iced_winit::runtime::{Action, task};

use super::*;
use crate::backend::{Backend, Workspace, WorkspacePorts};

pub(super) struct SimShell {
    state: Shell,
    id: window::Id,
    rt: tokio::runtime::Runtime,
    sim: simnode::SimHandle,
    queue: VecDeque<Task<Message>>,
    _storage: tempfile::TempDir,
    _identity_root: tempfile::TempDir,
}

impl SimShell {
    /// Self-contained boot: embedded auto-mode sim + in-process signing.
    /// No external binaries, no skip path — failure is a test failure.
    pub(super) fn boot() -> Self {
        Self::boot_with_opts(simnode::SimOpts {
            auto: true,
            ..Default::default()
        })
    }

    /// Governance-enabled boot whose sole validator is the fixture signer.
    pub(super) fn boot_with_valset() -> Self {
        let pubkey = signing::author_pubkey_hex();
        let mut shell = Self::boot_with_opts(simnode::SimOpts {
            auto: true,
            valset_keys: vec![fixture_pubkey_bytes()],
            node_key: Some(pubkey.clone()),
            ..Default::default()
        });
        shell.state.active_workspace = Some(Workspace {
            id: "sim".into(),
            name: "Sim".into(),
            chain_id: String::new(),
            pubkey,
            founder: true,
            member: true,
            ports: WorkspacePorts {
                listen: 0,
                http: shell.sim.addr().port(),
                rpc: 0,
                wireguard: None,
                invite: None,
            },
        });
        shell
    }

    fn boot_with_opts(opts: simnode::SimOpts) -> Self {
        signing::install();
        let storage = tempfile::tempdir().expect("sim storage");
        let sim = simnode::boot(storage.path(), "127.0.0.1:0".parse().expect("addr"), opts)
            .expect("boot embedded sim");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        // Shell.backend must exist for the signed-frame write path; with the
        // verb override installed it never spawns a subprocess.
        let identity_root = tempfile::tempdir().expect("identity root");
        let backend = rt
            .block_on(Backend::at_root(identity_root.path().to_path_buf()))
            .expect("backend fixture");

        let (mut state, _boot) = preset::ui_demo();
        state.node_client = Some(NodeClient::local(sim.addr().port()).expect("sim node client"));
        state.backend = Some(backend);
        let id = state.desktop.main.expect("preset opens a main window");
        Self {
            state,
            id,
            rt,
            sim,
            queue: VecDeque::new(),
            _storage: storage,
            _identity_root: identity_root,
        }
    }

    pub(super) fn click(&mut self, role: Role, name: &str) {
        let messages: Vec<Message> = {
            let mut ui = iced_test::simulator(view::view(&self.state, self.id));
            ui.click(by::role(role, name.to_owned()))
                .unwrap_or_else(|error| panic!("click {role:?} \"{name}\": {error:?}"));
            ui.into_messages().collect()
        };
        self.dispatch(messages);
    }

    /// Feed a message straight into `update()` — for widgets without a Sem
    /// wrapper and for timer ticks (nothing is asynchronous in this lane).
    pub(super) fn inject(&mut self, message: Message) {
        self.dispatch(vec![message]);
    }

    fn dispatch(&mut self, messages: Vec<Message>) {
        {
            // update() may tokio::spawn directly (e.g. the pages presence
            // handle); give it the lane's runtime as ambient context.
            let _enter = self.rt.enter();
            for message in messages {
                let task = update(&mut self.state, message);
                self.queue.push_back(task);
            }
        }
        self.pump();
    }

    /// The lane's one new mechanism: execute queued `update()` Tasks on the
    /// private runtime, feeding every `Action::Output` back through
    /// `update()`, until quiescent. Other actions (window/widget/font) have
    /// no runtime to serve them and are dropped.
    fn pump(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while let Some(queued) = self.queue.pop_front() {
            let Some(mut stream) = task::into_stream(queued) else {
                continue;
            };
            loop {
                assert!(
                    Instant::now() < deadline,
                    "pump deadline exceeded with {} task(s) still queued",
                    self.queue.len() + 1
                );
                let next = self.rt.block_on(async {
                    tokio::time::timeout(Duration::from_secs(10), stream.next()).await
                });
                match next {
                    Err(_elapsed) => panic!(
                        "pump: a task action stalled >10s ({} task(s) queued)",
                        self.queue.len()
                    ),
                    Ok(None) => break,
                    Ok(Some(Action::Output(message))) => {
                        let follow_up = {
                            let _enter = self.rt.enter();
                            update(&mut self.state, message)
                        };
                        self.queue.push_back(follow_up);
                    }
                    Ok(Some(_)) => {}
                }
            }
        }
    }

    pub(super) fn has(&self, role: Role, name: &str) -> bool {
        let mut ui = iced_test::simulator(view::view(&self.state, self.id));
        ui.find(by::role(role, name.to_owned())).is_ok()
    }

    pub(super) fn sees_text(&self, text: &str) -> bool {
        let mut ui = iced_test::simulator(view::view(&self.state, self.id));
        ui.find(text).is_ok()
    }

    pub(super) fn shell(&self) -> &Shell {
        &self.state
    }

    /// Chain-side read over the embedded listener (raw HTTP — any plain
    /// client must be a full wire citizen, same doctrine as the harnesses).
    pub(super) fn node_query(&self, target: &str, query: serde_json::Value) -> serde_json::Value {
        let body = serde_json::json!({ "target": target, "query": query }).to_string();
        let mut stream = TcpStream::connect(self.sim.addr()).expect("sim reachable");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("timeout");
        let request = format!(
            "POST /v1/query HTTP/1.1\r\nHost: sim\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).expect("write");
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read");
        let text = String::from_utf8_lossy(&raw);
        let payload = text.split("\r\n\r\n").nth(1).unwrap_or("").trim();
        assert!(
            text.starts_with("HTTP/1.1 200"),
            "query {target} failed: {text}"
        );
        serde_json::from_str(payload).unwrap_or(serde_json::Value::Null)
    }
}

fn fixture_pubkey_bytes() -> Vec<u8> {
    let hex = signing::author_pubkey_hex();
    assert_eq!(hex.len(), 64, "fixture signer must be a 32-byte key");
    (0..hex.len())
        .step_by(2)
        .map(|start| u8::from_str_radix(&hex[start..start + 2], 16).expect("fixture key hex"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::user::Resource;

    #[test]
    fn boot_navigate_chat_loads_over_the_wire() {
        let mut ui = SimShell::boot();
        ui.inject(Message::Navigate(Screen::Chat));
        assert!(
            matches!(ui.shell().user_screens.chat.data, Resource::Empty),
            "LoadChat round-trip completed against the embedded sim"
        );
    }
}
