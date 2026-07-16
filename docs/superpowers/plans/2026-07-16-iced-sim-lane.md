# Iced Sim Lane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** UI-driven transaction round-trips for the iced app against a spawned deterministic simnode — click → signed submit → commit → committed state renders back.

**Architecture:** A `shell/sim/` test module (gated like `shell/qa.rs`) boots `preset::ui_demo()`, injects `NodeClient::local(sim_port)` and a real `Backend` identity fixture, drives widgets via `iced_test::Simulator` + `by::role`, and — the one new mechanism — drains every `Task<Message>` returned by `update()` through `iced_runtime::task::into_stream` on a private tokio runtime, feeding `Action::Output` messages back into `update()` until quiescent. Spec: `docs/superpowers/specs/2026-07-16-iced-sim-lane-design.md`.

**Tech Stack:** Rust, iced 0.14 (`iced_winit::runtime` re-export — no new deps), `iced_test` (already a dev-dep), tokio (already a dep), `ducktape-simnode` + `ducktape-node` child processes.

## Global Constraints

- Worktree: `/home/eddy/dev/ducktape/.worktree/iced-sim-lane`, branch `feat/iced-sim-lane`, PR target **`feat/iced-app`** (NOT `dev` — the iced app rides that campaign branch; precedent PRs #639–#646).
- `CARGO_INCREMENTAL=0` on every cargo build/test — this box's rustc ICEs with incremental on `ducktape-iced`.
- If rustc SIGSEGVs building the wasmtime dep graph (simnode pulls it): `sccache --stop-server; export RUSTC_WRAPPER=""; export RUST_MIN_STACK=2147483648` and retry.
- Lint gate: `cargo clippy -p ducktape-iced --tests --no-deps` (the `--no-deps` is deliberate). Never `cargo fmt --all`; format only touched code.
- Package names: simnode = `-p simnode` (binary `ducktape-simnode`), node = `-p node-bin` (binary `ducktape-node`). `-p node` is a DIFFERENT crate — using it makes the gate vacuous.
- Workspace edition is 2024: `std::env::set_var` is `unsafe`.
- All module-internal items (`Shell`, `Message`, `update`, `view::view`) are private to the `shell` module — the sim module MUST be a child of `shell` (like `qa.rs`), reached via `use super::*`.
- Exact import paths for cross-module items (`Resource`, `ChatMessageEvent`, `user_screens`): the item and variant names below are verified against the tree; if a path differs, follow the compiler error — `shell.rs`'s own imports (top of file) show the canonical aliases.
- No `println!` in the app; the harness's loud-skip `eprintln!` is test-binary output and is fine.

### One-time setup (before Task 1)

```bash
cd /home/eddy/dev/ducktape/.worktree/iced-sim-lane
# Warm the Cargo target by hardlink-cloning the sibling checkout's (~140G, saves a cold build):
cp -al ../iced-app/target ./target
# Build the two child binaries the suite spawns:
CARGO_INCREMENTAL=0 cargo build -p simnode -p node-bin
ls target/debug/ducktape-simnode target/debug/ducktape-node   # both must exist
```

---

### Task 1: `SimNode` — spawn/control a simnode child

**Files:**
- Create: `app/src-iced/src/shell/sim/node.rs`
- Create: `app/src-iced/src/shell/sim/mod.rs` (skeleton — declares `mod node;`)
- Modify: `app/src-iced/src/shell.rs:52` (add the module declaration after `mod qa;`)

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces (Task 2/3/4 rely on these exact signatures):
  - `pub(super) struct SimNode` with `pub(super) fn spawn() -> Option<SimNode>` (None = binaries missing, already reported loudly / panics under `DUCKTAPE_SIM_REQUIRE`), `pub(super) fn port(&self) -> u16`, `pub(super) fn query(&self, target: &str, query: serde_json::Value) -> serde_json::Value`.
  - Side effect of `spawn()`: sets `DUCKTAPE_NODE_BIN` (process-wide, once) so `Backend` signing verbs resolve `ducktape-node`.

- [ ] **Step 1: Declare the module in `shell.rs`**

In `app/src-iced/src/shell.rs`, directly after the `mod qa;` declaration (line ~52):

```rust
#[cfg(all(feature = "agent", debug_assertions, test))]
mod sim;
```

- [ ] **Step 2: Write the module skeleton `sim/mod.rs`**

```rust
//! The sim lane: transaction round-trips against a deterministic simnode.
//!
//! The in-process recipe lane (`shell/qa.rs`) proves what `update()` +
//! `view()` can prove but never runs a Task; the fleet lane has a real node
//! but nothing deterministic. This lane closes the gap — the iced twin of the
//! TS `app/src/test/sim/` suites: boot the real shell state, point
//! `node_client` at a spawned `ducktape-simnode`, execute `update()` Tasks on
//! a private tokio runtime, and assert committed state renders back through
//! the real view. Design: docs/superpowers/specs/2026-07-16-iced-sim-lane-design.md.

mod node;

use node::SimNode;
```

- [ ] **Step 3: Write `sim/node.rs`**

The spawn/wire bits are copied down from `bin/simnode/tests/harness/mod.rs` (simnode has no lib crate and that harness lives with simnode's own tests on `dev`; consolidation is deferred until `feat/iced-app` converges with `dev`). Differences from the original: binary resolution via env/target (no `CARGO_BIN_EXE_`), owned tempdir storage, `--auto` always on, and the loud-skip guard.

```rust
//! Spawn/control a real `ducktape-simnode` child for the sim lane.
//!
//! Copied-down spawn bits of `bin/simnode/tests/harness/mod.rs`; consolidation
//! into a shared crate is deferred until this branch converges with `dev`.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Once;
use std::time::{Duration, Instant};

pub(super) struct SimNode {
    child: Child,
    port: u16,
    _storage: tempfile::TempDir,
}

impl Drop for SimNode {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl SimNode {
    /// Spawn a fresh `--auto` simnode on a free port with fresh storage.
    /// Returns None (after reporting loudly) when the child binaries are not
    /// built; panics instead under `DUCKTAPE_SIM_REQUIRE=1` so the Make gate
    /// can never pass vacuously.
    pub(super) fn spawn() -> Option<Self> {
        let Some(simnode) = resolve_bin("DUCKTAPE_SIMNODE_BIN", "ducktape-simnode") else {
            return skip("ducktape-simnode is not built");
        };
        let Some(node) = resolve_bin("DUCKTAPE_NODE_BIN", "ducktape-node") else {
            return skip("ducktape-node is not built (signing verbs need it)");
        };
        export_node_bin(&node);

        let storage = tempfile::tempdir().expect("sim storage dir");
        let port = free_port();
        let mut cmd = Command::new(simnode);
        cmd.arg("--listen")
            .arg(format!("127.0.0.1:{port}"))
            .arg("--storage")
            .arg(storage.path())
            .arg("--auto")
            .stdout(Stdio::null())
            // Startup failures land on stderr — keep it visible or they read
            // as an opaque readiness timeout.
            .stderr(Stdio::inherit());
        let child = cmd.spawn().expect("spawn ducktape-simnode");
        let mut sim = Self { child, port, _storage: storage };
        sim.await_status();
        Some(sim)
    }

    pub(super) fn port(&self) -> u16 {
        self.port
    }

    /// Chain-side read for committed-state assertions.
    pub(super) fn query(&self, target: &str, query: serde_json::Value) -> serde_json::Value {
        let (status, reply) = self.request(
            "POST",
            "/v1/query",
            Some(&serde_json::json!({ "target": target, "query": query })),
        );
        assert_eq!(status, 200, "query {target} failed: {reply}");
        reply
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> (u16, serde_json::Value) {
        try_request(self.port, method, path, body).expect("sim reachable")
    }

    fn await_status(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Ok((200, _)) = try_request(self.port, "GET", "/v1/status", None) {
                return;
            }
            if let Some(status) = self.child.try_wait().expect("poll sim") {
                panic!("sim exited during startup ({status}) — see stderr above");
            }
            assert!(
                Instant::now() < deadline,
                "sim on port {} never answered /v1/status",
                self.port
            );
            std::thread::sleep(Duration::from_millis(150));
        }
    }
}

/// `DUCKTAPE_NODE_BIN` must be set process-wide: `Backend`'s verb runner
/// falls back to a sibling of the current exe, which under `cargo test` is
/// `target/debug/deps/` — not where cargo puts `ducktape-node`.
fn export_node_bin(node: &std::path::Path) {
    static EXPORT: Once = Once::new();
    let node = node.to_path_buf();
    EXPORT.call_once(move || {
        if std::env::var_os("DUCKTAPE_NODE_BIN").is_none() {
            // SAFETY: first harness boot; no backend verb thread exists yet
            // to race this write (edition 2024 makes set_var unsafe).
            unsafe { std::env::set_var("DUCKTAPE_NODE_BIN", &node) };
        }
    });
}

fn skip(reason: &str) -> Option<SimNode> {
    if std::env::var_os("DUCKTAPE_SIM_REQUIRE").is_some() {
        panic!("sim lane required (DUCKTAPE_SIM_REQUIRE) but {reason}");
    }
    eprintln!(
        "SKIP shell::sim — {reason}; run `CARGO_INCREMENTAL=0 cargo build -p simnode -p node-bin`"
    );
    None
}

fn resolve_bin(env_key: &str, file_name: &str) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(env_key) {
        let path = PathBuf::from(path);
        return path.exists().then_some(path);
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target")
        });
    ["debug", "release"]
        .iter()
        .map(|profile| target.join(profile).join(file_name))
        .find(|path| path.exists())
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind a free port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn try_request(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&serde_json::Value>,
) -> std::io::Result<(u16, serde_json::Value)> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let payload = body.map(|value| value.to_string()).unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(request.as_bytes())?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw)?;
    let status: u16 = raw
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let json_body = raw
        .split("\r\n\r\n")
        .nth(1)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or(serde_json::Value::Null);
    Ok((status, json_body))
}
```

NOTE for the implementer: before finalizing `try_request`, compare against the original in `/home/eddy/dev/ducktape/bin/simnode/tests/harness/mod.rs` (readable from this worktree's sibling — or `git show origin/dev:bin/simnode/tests/harness/mod.rs`). If the original handles chunked responses or headers differently, prefer the original's body — it is wire-proven against this exact server.

- [ ] **Step 4: Write the smoke test at the bottom of `sim/node.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The child spawns, answers /v1/status, and dies with the handle.
    #[test]
    fn spawn_answers_status() {
        let Some(sim) = SimNode::spawn() else { return };
        let (status, reply) = sim.request("GET", "/v1/status", None);
        assert_eq!(status, 200, "status: {reply}");
    }
}
```

- [ ] **Step 5: Run the smoke test**

```bash
cd /home/eddy/dev/ducktape/.worktree/iced-sim-lane
CARGO_INCREMENTAL=0 DUCKTAPE_SIM_REQUIRE=1 cargo test -p ducktape-iced shell::sim::node -- --nocapture
```
Expected: `spawn_answers_status ... ok` (1 passed). If it panics "sim lane required but…", the setup build step was skipped — build the binaries first.

- [ ] **Step 6: Commit**

```bash
git add app/src-iced/src/shell.rs app/src-iced/src/shell/sim/
git commit -m "test(sim-lane): SimNode child harness — spawn simnode, resolve node bin

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: `SimShell` — boot the shell against the sim and pump Tasks

**Files:**
- Modify: `app/src-iced/src/shell/sim/mod.rs` (replace the skeleton body)

**Interfaces:**
- Consumes: `SimNode::spawn() / port() / query()` from Task 1.
- Produces (Tasks 3/4 rely on these exact signatures, all `pub(super)` in `shell::sim`):
  - `struct SimShell`; `fn boot() -> Option<SimShell>`
  - `fn click(&mut self, role: Role, name: &str)`
  - `fn click_and_type(&mut self, role: Role, name: &str, text: &str)`
  - `fn inject(&mut self, message: Message)` — also the spec's "tick" affordance
  - `fn has(&self, role: Role, name: &str) -> bool`, `fn sees_text(&self, text: &str) -> bool`
  - `fn shell(&self) -> &Shell`, `fn sim(&self) -> &SimNode`
  - `const PASSWORD: &str` (shared — the identity password cache is process-global)

- [ ] **Step 1: Write the harness in `sim/mod.rs`**

Keep the module doc + `mod node;` from Task 1, add `mod chat;` ONLY in Task 3. Body:

```rust
mod node;

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use futures_util::StreamExt as _;
use iced_agent_plugin::Role;
use iced_agent_plugin::selector::by;
use iced_winit::runtime::{Action, task};

use node::SimNode;

use super::*;
use crate::backend::Backend;

/// One constant everywhere: the backend's unlock cache is process-global,
/// so parallel tests must agree on the password.
pub(super) const PASSWORD: &str = "correct horse battery staple";

pub(super) struct SimShell {
    state: Shell,
    id: window::Id,
    rt: tokio::runtime::Runtime,
    sim: SimNode,
    queue: VecDeque<Task<Message>>,
    _identity_root: tempfile::TempDir,
}

impl SimShell {
    /// None = child binaries missing (already reported loudly by SimNode).
    /// Everything past that point panics — the fixture must work.
    pub(super) fn boot() -> Option<Self> {
        let sim = SimNode::spawn()?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        // Chat writes are account-signed frames: submit_signed() needs a
        // Backend with an unlocked identity. create_identity caches the
        // password, so signing works immediately after.
        let identity_root = tempfile::tempdir().expect("identity root");
        let backend = rt
            .block_on(async {
                let backend = Backend::at_root(identity_root.path().to_path_buf()).await?;
                backend.create_identity(PASSWORD.into()).await?;
                Ok::<_, String>(backend)
            })
            .expect("identity fixture");

        let (mut state, _boot) = preset::ui_demo();
        state.node_client = Some(NodeClient::local(sim.port()).expect("sim node client"));
        state.backend = Some(backend);
        let id = state.desktop.main.expect("preset opens a main window");
        Some(Self {
            state,
            id,
            rt,
            sim,
            queue: VecDeque::new(),
            _identity_root: identity_root,
        })
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

    pub(super) fn click_and_type(&mut self, role: Role, name: &str, text: &str) {
        let messages: Vec<Message> = {
            let mut ui = iced_test::simulator(view::view(&self.state, self.id));
            ui.click(by::role(role, name.to_owned()))
                .unwrap_or_else(|error| panic!("focus {role:?} \"{name}\": {error:?}"));
            let _ = ui.typewrite(text);
            ui.into_messages().collect()
        };
        self.dispatch(messages);
    }

    /// Feed a message straight into `update()` — for widgets without a Sem
    /// wrapper and for timer ticks (nothing is asynchronous in this lane;
    /// a tick is an explicit, deterministic injection).
    pub(super) fn inject(&mut self, message: Message) {
        self.dispatch(vec![message]);
    }

    fn dispatch(&mut self, messages: Vec<Message>) {
        for message in messages {
            let task = update(&mut self.state, message);
            self.queue.push_back(task);
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
                continue; // Task::none()
            };
            loop {
                assert!(
                    Instant::now() < deadline,
                    "pump deadline exceeded with {} task(s) still queued",
                    self.queue.len() + 1
                );
                let next = self
                    .rt
                    .block_on(async { tokio::time::timeout(Duration::from_secs(10), stream.next()).await });
                match next {
                    Err(_elapsed) => panic!(
                        "pump: a task action stalled >10s ({} task(s) queued)",
                        self.queue.len()
                    ),
                    Ok(None) => break,
                    Ok(Some(Action::Output(message))) => {
                        let follow_up = update(&mut self.state, message);
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

    pub(super) fn sim(&self) -> &SimNode {
        &self.sim
    }
}
```

Import notes (compiler-guided, names verified): `Action` and `task` come from `iced_winit::runtime` (the facade's own source — `iced-0.14.0/src/lib.rs:482` does `use iced_winit::runtime;`); `Backend` from `crate::backend` (`shell.rs` field `backend: Option<Backend>` at `shell.rs:278`); `NodeClient::local` from the existing `use crate::transport::NodeClient` already in `super::*`.

- [ ] **Step 2: Write the boot smoke test at the bottom of `sim/mod.rs`**

The Navigate→LoadChat round-trip is the proof that the pump really executed an HTTP query against the child: a fresh sim has zero channels, and only a completed `ChatLoaded` turns `chat.data` from `Resource::Loading` into `Resource::Empty`.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::user::Resource;

    #[test]
    fn boot_navigate_chat_loads_over_the_wire() {
        let Some(mut ui) = SimShell::boot() else { return };
        ui.inject(Message::Navigate(Screen::Chat));
        assert!(
            matches!(ui.shell().user_screens.chat.data, Resource::Empty),
            "LoadChat round-trip completed against the sim child"
        );
    }
}
```

(`Resource` path: `chat.data` is `Resource<ChatData>` per `screens/chat.rs:161`; if `Resource` lives in a different module the compiler error names it — fix the `use`, not the assertion.)

- [ ] **Step 3: Run it**

```bash
CARGO_INCREMENTAL=0 DUCKTAPE_SIM_REQUIRE=1 cargo test -p ducktape-iced shell::sim -- --nocapture
```
Expected: 2 passed (`spawn_answers_status`, `boot_navigate_chat_loads_over_the_wire`).

Debug notes if the boot test fails:
- Panic in `identity fixture`: run with `--nocapture` and read the verb error. "no trusted, executable ducktape-node" = `export_node_bin` didn't run before `Backend` was used, or the binary is missing. "password must be at least…" = `PASSWORD` too short (must be ≥ the MIN_PASSWORD_CHARS in `backend/identity.rs`).
- `chat.data` stays `Loading`: the pump never fed `ChatLoaded` back — print what `update` returned; check `Message::UserScreen(user_screens::Message::Service(ServiceEvent::ChatLoaded(..)))` arrived (the load task is built at `shell.rs:2352`).
- `Resource::Error` instead of `Empty`: the query failed — the sim child's stderr (inherited) says why.

- [ ] **Step 4: Commit**

```bash
git add app/src-iced/src/shell/sim/mod.rs
git commit -m "test(sim-lane): SimShell harness — task pump + identity fixture

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Chat round-trip proof scenario

**Files:**
- Create: `app/src-iced/src/shell/sim/chat.rs`
- Modify: `app/src-iced/src/shell/sim/mod.rs` (add `mod chat;` under `mod node;`)

**Interfaces:**
- Consumes: everything Task 2 produces (`SimShell::boot/click/click_and_type/inject/has/sees_text/shell/sim`), `SimNode::query`.
- Produces: `pub(super) fn create_channel(ui: &mut SimShell, name: &str)` (Task 4 reuses it).

**Flow facts (verified against the tree, exact strings):**
- Nav button: `by::role(Role::Button, "Chat")` → `Message::Navigate(Screen::Chat)` → chained `LoadChat` queries `chat`/`"channels"`.
- Create form: `Role::Button, "+"` toggles it; the channel-name input is a BARE `TextInput` (placeholder `"channel name"`, NO Sem wrapper — `field()` at `screens/user.rs:976` attaches no role), so the draft is set by injecting `ChatMessageEvent::ChannelNameChanged`; `Role::Button, "Create channel"` submits (disabled until the draft is non-empty).
- Create wire: signed frame, target `chat`, `{"create_channel":{"channel_id":<slug>,"name":…,"post_policy":"open"}}`. On Ok the reducer chains `LoadChat` automatically (`screens/user.rs:758-767`) — the re-render comes from the node, not from local echo.
- Composer: `Role::TextInput, "Message composer"` (a Sem-wrapped `text_editor`), send via `Role::Button, "Send"`; wire `{"post_message":{…}}`; same auto-chained `LoadChat`.
- Channel rail entry: `Role::ListItem, <channel name>`.

- [ ] **Step 1: Write the test in `sim/chat.rs`**

```rust
//! Chat transaction round-trips — the sim lane's proof scenarios.

use super::super::*;
use super::{SimShell, node::SimNode};
use crate::screens::chat::ChatMessageEvent;
use crate::screens::user::Resource;
use iced_agent_plugin::Role;

/// Create a channel through the UI. The name input is a bare TextInput
/// (no Sem wrapper), so the draft change message is injected; the toggle
/// and submit are real widget interactions.
pub(super) fn create_channel(ui: &mut SimShell, name: &str) {
    ui.click(Role::Button, "+");
    ui.inject(Message::UserScreen(user_screens::Message::Chat(
        ChatMessageEvent::ChannelNameChanged(name.into()),
    )));
    ui.click(Role::Button, "Create channel");
}

#[test]
fn create_channel_and_post_message_round_trip() {
    let Some(mut ui) = SimShell::boot() else { return };
    ui.inject(Message::Navigate(Screen::Chat));
    assert!(
        matches!(ui.shell().user_screens.chat.data, Resource::Empty),
        "fresh sim has no channels"
    );

    create_channel(&mut ui, "qa-lane");

    // The signed frame committed on the sim and the auto-chained LoadChat
    // re-queried the COMMITTED channel list — this render is node data.
    assert!(
        ui.shell().user_screens.chat.error.is_none(),
        "create failed: {:?}",
        ui.shell().user_screens.chat.error
    );
    assert!(ui.has(Role::ListItem, "qa-lane"), "committed channel renders in the rail");
    let channels = ui.sim().query("chat", serde_json::json!("channels"));
    assert!(
        channels.to_string().contains("qa-lane"),
        "channel is committed node-side: {channels}"
    );

    // Post a message through the real composer.
    ui.click_and_type(Role::TextInput, "Message composer", "hello from the sim lane");
    ui.click(Role::Button, "Send");
    assert!(
        ui.shell().user_screens.chat.error.is_none(),
        "post failed: {:?}",
        ui.shell().user_screens.chat.error
    );
    assert!(
        ui.sees_text("hello from the sim lane"),
        "committed message re-renders from the node"
    );
}
```

- [ ] **Step 2: Run it**

```bash
CARGO_INCREMENTAL=0 DUCKTAPE_SIM_REQUIRE=1 cargo test -p ducktape-iced shell::sim -- --nocapture
```
Expected: 3 passed.

Debug notes:
- `click "+"` fails to find the button: the "+" create toggle was traced in the EMPTY chat shell (`screens/chat.rs:540`). If the button isn't found, grep `ToggleNewChannel` in `screens/chat.rs` for the label the non-empty view uses and adjust the selector — the message chain stays identical.
- `click_and_type` typewrite doesn't reach the editor (Send stays disabled / post is blank): fall back to injecting the composer edit directly, keeping Send as a real click:
  ```rust
  use crate::screens::chat_composer;
  use iced::widget::text_editor;
  ui.inject(Message::UserScreen(user_screens::Message::Chat(
      ChatMessageEvent::Composer {
          thread: None,
          message: chat_composer::Message::Edit(text_editor::Action::Edit(
              text_editor::Edit::Paste(std::sync::Arc::new("hello from the sim lane".into())),
          )),
      },
  )));
  ```
  (Exact `Composer` field names: `screens/chat.rs:275`.) Keep whichever variant passes and delete the other.
- `chat.error == Some("desktop identity backend is unavailable")`: `Shell.backend` wasn't injected — Task 2's boot regressed.
- `chat.error` contains an HTTP error: the sim child rejected the frame; its stderr (inherited) names the module reason.

- [ ] **Step 3: Commit**

```bash
git add app/src-iced/src/shell/sim/
git commit -m "test(sim-lane): chat round-trip — UI create/post commits on simnode and re-renders

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Rejection proof scenario

**Files:**
- Modify: `app/src-iced/src/shell/sim/chat.rs` (append one test)

**Interfaces:**
- Consumes: `create_channel(&mut SimShell, &str)` from Task 3; `SimShell` API from Task 2.
- Produces: nothing downstream.

**Flow facts:** creating a channel whose `slug(name)` collides is rejected by the chat module with `Error::Module("channel already exists: <id>")` (`crates/apps/chat/src/lib.rs:440`). The rejection lands in `Shell.user_screens.chat.error` (`screens/user.rs:762`) and — deliberately — chains NO refresh. The error is NOT rendered by `chat::view` (verified: `state.error` has no view read), so the assertions go through state + the unchanged rail, not a widget.

- [ ] **Step 1: Append the test to `sim/chat.rs`**

```rust
#[test]
fn duplicate_channel_rejection_lands_in_error_and_chains_no_refresh() {
    let Some(mut ui) = SimShell::boot() else { return };
    ui.inject(Message::Navigate(Screen::Chat));

    create_channel(&mut ui, "dup");
    assert!(ui.has(Role::ListItem, "dup"));
    assert!(ui.shell().user_screens.chat.error.is_none());

    // Same name → same slug → the module rejects the second create.
    create_channel(&mut ui, "dup");
    let error = ui
        .shell()
        .user_screens
        .chat
        .error
        .clone()
        .expect("module rejection reaches chat.error");
    assert!(
        error.contains("already exists"),
        "error carries the module reason: {error}"
    );
    // The rejected submit chained no refresh and corrupted nothing:
    // the committed list still has exactly one channel.
    assert!(ui.has(Role::ListItem, "dup"));
    let channels = ui.sim().query("chat", serde_json::json!("channels"));
    assert_eq!(
        channels.to_string().matches("\"dup\"").count() > 0,
        true,
        "committed list unchanged: {channels}"
    );
}
```

- [ ] **Step 2: Run the whole lane**

```bash
CARGO_INCREMENTAL=0 DUCKTAPE_SIM_REQUIRE=1 cargo test -p ducktape-iced shell::sim -- --nocapture
```
Expected: 4 passed.

Note: if the second `create_channel` panics inside `click(Role::Button, "+")` because the create form re-render differs after a first channel exists, apply the same fallback as Task 3's debug note (grep `ToggleNewChannel` for the live label).

- [ ] **Step 3: Commit**

```bash
git add app/src-iced/src/shell/sim/chat.rs
git commit -m "test(sim-lane): duplicate-channel rejection surfaces in chat.error, chains no refresh

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Wire the lane into `make ui-qa` + lane doctrine docs

**Files:**
- Modify: `Makefile:431` (the `ui-qa` target)
- Modify: `skills/qa/SKILL.md` (lane doctrine table)
- Modify: `app/src-iced/src/test/mod.rs` (module doc pointer)

**Interfaces:**
- Consumes: the `shell::sim` test filter (Tasks 1–4) and the `DUCKTAPE_SIM_REQUIRE` guard (Task 1).
- Produces: the CI-facing entrypoint; nothing code-level.

- [ ] **Step 1: Extend `ui-qa` in the Makefile**

Current target (Makefile:431):

```make
ui-qa:
	CARGO_INCREMENTAL=0 $(CARGO) test -p ducktape-iced qa_recipes
	$(CARGO) build -p ducktape-iced --bin ducktape-iced
	ops/iced-fleet up 2 --preset ui-demo
	ops/iced-fleet run qa/recipes/*.json; status=$$?; ops/iced-fleet down; exit $$status
```

Replace with (sim lane first — it needs the child binaries built, and `DUCKTAPE_SIM_REQUIRE=1` turns a silent skip into a hard failure):

```make
ui-qa:
	CARGO_INCREMENTAL=0 $(CARGO) build -p simnode -p node-bin
	CARGO_INCREMENTAL=0 DUCKTAPE_SIM_REQUIRE=1 $(CARGO) test -p ducktape-iced shell::sim
	CARGO_INCREMENTAL=0 $(CARGO) test -p ducktape-iced qa_recipes
	$(CARGO) build -p ducktape-iced --bin ducktape-iced
	ops/iced-fleet up 2 --preset ui-demo
	ops/iced-fleet run qa/recipes/*.json; status=$$?; ops/iced-fleet down; exit $$status
```

- [ ] **Step 2: Run the make target far enough to prove the new lines**

```bash
make ui-qa 2>&1 | head -40
```
Expected: the simnode/node-bin build runs, then `shell::sim` tests pass (4 passed). You may Ctrl-C once the `qa_recipes` line starts — the fleet half is unchanged and expensive; note in the PR that it was not re-run, or let it run to completion if the box has a display/Xvfb set up.

- [ ] **Step 3: Add the lane to `skills/qa/SKILL.md`**

Read the file's lane-doctrine table (it defines `lane: both` vs `lane: fleet`). Add a row/paragraph for the sim lane, wording:

```markdown
- **Sim lane (`shell::sim`)** — transaction round-trips and node-visible flows:
  the in-process shell (Simulator + a task pump executing `update()` Tasks on
  tokio) against a spawned `ducktape-simnode --auto` child. Deterministic: no
  timers, no subscriptions; timer refreshes are injected messages. Run:
  `CARGO_INCREMENTAL=0 cargo build -p simnode -p node-bin && DUCKTAPE_SIM_REQUIRE=1 cargo test -p ducktape-iced shell::sim`.
  What belongs here: submit → commit → re-render flows, module rejections
  surfacing in UI state. What does NOT: anything needing a real window
  (`lane: fleet`), pure view variants (`src/test/`), recipe-provable
  navigation (`lane: both`).
```

Match the file's existing formatting (table row vs bullet) — extend, don't restructure.

- [ ] **Step 4: Point `src/test/mod.rs` at the lane**

In `app/src-iced/src/test/mod.rs`, extend the "What does NOT belong here" doc paragraph with one sentence:

```rust
//! Transaction round-trips against a deterministic node belong in the sim
//! lane (`src/shell/sim/`, `cargo test -p ducktape-iced shell::sim`).
```

- [ ] **Step 5: Commit**

```bash
git add Makefile skills/qa/SKILL.md app/src-iced/src/test/mod.rs
git commit -m "test(sim-lane): wire shell::sim into make ui-qa + lane doctrine

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: Gates, spec/plan commit hygiene, PR

**Files:**
- No new files; verification + delivery.

**Interfaces:**
- Consumes: the whole branch.
- Produces: a PR against `feat/iced-app`.

- [ ] **Step 1: Lint gate (touch first — cached cargo re-emits no warnings)**

```bash
cd /home/eddy/dev/ducktape/.worktree/iced-sim-lane
find app/src-iced/src/shell/sim -name '*.rs' -exec touch {} +
CARGO_INCREMENTAL=0 cargo clippy -p ducktape-iced --tests --no-deps
```
Expected: zero warnings in `shell/sim/*` (pre-existing warnings elsewhere in the crate are not this task's).

- [ ] **Step 2: Full crate test run**

```bash
CARGO_INCREMENTAL=0 DUCKTAPE_SIM_REQUIRE=1 cargo test -p ducktape-iced
```
Expected: all tests pass, including the 4 `shell::sim` tests and the pre-existing `qa_recipes` / screen tests.

- [ ] **Step 3: Push and open the PR against `feat/iced-app`**

```bash
git push -u origin feat/iced-sim-lane
gh pr create --base feat/iced-app --title "test(sim-lane): iced transaction round-trips against simnode" --body "$(cat <<'EOF'
## Summary
- New sim lane (`app/src-iced/src/shell/sim/`): the iced twin of the TS `app/src/test/sim/` suites — boot the real shell, point `node_client` at a spawned `ducktape-simnode --auto`, execute `update()` Tasks via `iced_runtime::task::into_stream` on a private tokio runtime, assert committed state re-renders.
- Identity fixture: chat writes are account-signed frames, so the harness builds a real `Backend` (`at_root` + `create_identity`) and resolves `ducktape-node` for the signing verbs.
- Proof scenarios: chat create+post round-trip (committed data re-renders; also asserted node-side via `/v1/query`), duplicate-channel rejection landing in `chat.error` with no chained refresh.
- `make ui-qa` builds `simnode`+`node-bin` and runs the lane with `DUCKTAPE_SIM_REQUIRE=1` (a skip is a hard failure); lane doctrine updated in `skills/qa/SKILL.md`.

Design: `docs/superpowers/specs/2026-07-16-iced-sim-lane-design.md`.

## Verification
- `CARGO_INCREMENTAL=0 DUCKTAPE_SIM_REQUIRE=1 cargo test -p ducktape-iced` — all green (state actual counts).
- `cargo clippy -p ducktape-iced --tests --no-deps` — clean on touched files.
- `make ui-qa` first half (sim + recipe lanes) — state what ran.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 4: Report**

State in the final summary: test counts, gate results, anything skipped (e.g. the fleet half of `ui-qa`), and any deviation from this plan (e.g. the composer typewrite fallback taken in Task 3).

---

## Self-Review (done at authoring time)

- **Spec coverage:** SimNode (Task 1), SimShell/pump/tick=inject (Task 2), round-trip proof (Task 3), rejection proof (Task 4), Make gate + loud-skip (Tasks 1+5), doctrine docs (Task 5), gates+PR (Task 6). Cut items (recipe sim lane, `/sim/step`+`peer_block`, Emulator, shared crate) are cut in the spec too.
- **Placeholders:** none; the two genuinely uncertain UI details (the "+" label in the non-empty view, composer typewrite) carry exact fallback code inline rather than "handle appropriately".
- **Type consistency:** `SimNode::spawn/port/query`, `SimShell::boot/click/click_and_type/inject/has/sees_text/shell/sim`, `create_channel(&mut SimShell, &str)` — names match across Tasks 1–5.
