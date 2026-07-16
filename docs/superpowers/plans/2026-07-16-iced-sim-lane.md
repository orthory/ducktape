# Iced Sim Lane Implementation Plan (v2 — in-process)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** UI-driven transaction round-trips for the iced app against an EMBEDDED deterministic simnode — click → in-process signed submit → commit → committed state renders back; plain `cargo test -p ducktape-iced` is self-contained (foundry-style, no external binaries).

**Architecture:** A `shell/sim/` test module boots `preset::ui_demo()`, embeds the chain via `simnode::boot(storage, 127.0.0.1:0, SimOpts{auto:true})` (phase A lib, merged to dev), points the app's real `NodeClient` at the in-process listener, installs a test-only signing override at the verb choke-point so chat's signed frames are produced by `node::encode_frame` with a generated ed25519 key, and drains every `Task<Message>` from `update()` through `iced_winit::runtime::task::into_stream` on a private tokio runtime. Spec: `docs/superpowers/specs/2026-07-16-iced-sim-lane-design.md` (v2 header).

**Tech Stack:** Rust, iced 0.14, `iced_test`, tokio (all present); NEW dev-deps: `simnode` (path `bin/simnode`), `node` + `sdk` (kernel crates), the workspace ed25519 crate (`commonware-cryptography`).

## Global Constraints

- Worktree: `/home/eddy/dev/ducktape/.worktree/iced-sim-lane`, branch `feat/iced-sim-lane`, PR target **`feat/iced-app`**.
- Phase A (`simnode` lib) must be MERGED into `dev` and `origin/dev` merged into this branch before Task 2 — Task 1 does that merge.
- `CARGO_INCREMENTAL=0` on every cargo command; SIGSEGV recovery: `sccache --stop-server; export RUSTC_WRAPPER=""; export RUST_MIN_STACK=2147483648`.
- Lint gate: `cargo clippy -p ducktape-iced --tests --no-deps`. Never `cargo fmt --all`.
- The sim module is a child of `shell` (`Shell`/`Message`/`update` are module-private), gated `#[cfg(all(feature = "agent", debug_assertions, test))]` — already declared on this branch.
- Production behavior must be untouched: the signing override and its statics are `#[cfg(test)]` only; `run_verb_inner`'s subprocess path is the compiled behavior in every non-test build.
- No `println!` in app code. Exact import paths compiler-guided; item/variant names below are verified.
- NEVER set env vars in tests (edition 2024 `set_var` is unsafe and v2 needs none — that was v1 machinery).

---

### Task 1: Converge the branch — revert child-process harness, merge dev, add dev-deps

**Files:**
- Revert: commit `069e714dc9` (the v1 child-process `SimNode`; removes `shell/sim/*` and the `mod sim;` declaration)
- Merge: `origin/dev` (brings the `simnode` lib)
- Modify: `app/src-iced/Cargo.toml` (`[dev-dependencies]`)
- Re-add: `app/src-iced/src/shell.rs` `mod sim;` declaration + empty `app/src-iced/src/shell/sim/mod.rs` skeleton

**Interfaces:**
- Produces: a tree where `simnode::boot` is importable from `ducktape-iced` dev-deps and `shell::sim` exists as an empty gated module. Later tasks rely on dev-dep names `simnode`, `node`, `sdk`, and the ed25519 signer type from the workspace crypto crate.

- [ ] **Step 1: Revert the v1 harness commit**

```bash
cd /home/eddy/dev/ducktape/.worktree/iced-sim-lane
git revert --no-edit 069e714dc9
```

- [ ] **Step 2: Merge dev (phase A must already be merged there)**

```bash
git fetch origin dev
git merge --no-edit origin/dev
ls bin/simnode/src/lib.rs   # must exist after the merge
```
Expected: clean or trivially-resolvable merge (`bin/simnode` had zero divergence on this branch). If conflicts appear outside docs, stop and report BLOCKED with the conflict list.

- [ ] **Step 3: Add dev-dependencies**

In `app/src-iced/Cargo.toml` `[dev-dependencies]` (below `iced_test`):

```toml
# The embedded sim lane (shell/sim): in-process deterministic node + frame signing.
simnode = { path = "../../bin/simnode" }
node.workspace = true
sdk.workspace = true
```
Check the root `Cargo.toml` `[workspace.dependencies]` for the exact ed25519 crate name the kernel uses (`commonware-cryptography`); add it the same way (`commonware-cryptography.workspace = true` — adjust to the actual key). If `node`/`sdk` are not in workspace.dependencies, use `{ path = "../../crates/kernel/node" }` / the sdk crate's actual path.

- [ ] **Step 4: Re-add the gated module skeleton**

`app/src-iced/src/shell.rs` (after `mod qa;`):
```rust
#[cfg(all(feature = "agent", debug_assertions, test))]
mod sim;
```
`app/src-iced/src/shell/sim/mod.rs`:
```rust
//! The sim lane: transaction round-trips against an EMBEDDED deterministic
//! simnode (`simnode::boot`) — the iced twin of the TS `app/src/test/sim/`
//! suites, foundry-style: plain `cargo test` needs no external binaries.
//! Design: docs/superpowers/specs/2026-07-16-iced-sim-lane-design.md (v2).
```

- [ ] **Step 5: Compile check + commit**

```bash
CARGO_INCREMENTAL=0 cargo check -p ducktape-iced --tests
git add -A
git commit -m "chore(sim-lane): revert child-process harness, merge dev's simnode lib, wire dev-deps

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```
(The revert + merge already created their own commits; this commit carries the Cargo.toml + skeleton.)

---

### Task 2: In-process signing override at the verb choke-point

**Files:**
- Modify: `app/src-iced/src/backend/node_control.rs` (the `#[cfg(test)]` override + check in `run_verb_inner`, ~line 302)
- Modify: `app/src-iced/src/backend/mod.rs` (re-export the installer `pub(crate)`)
- Create: `app/src-iced/src/shell/sim/signing.rs` (the override implementation + installer)
- Modify: `app/src-iced/src/shell/sim/mod.rs` (`mod signing;`)

**Interfaces:**
- Consumes: Task 1's dev-deps (`node`, `sdk`, the ed25519 crate).
- Produces (Task 3 relies on): `pub(super) fn install() ` in `shell::sim::signing` — idempotent, process-global; after calling it, `Backend`'s `sign_content_frame` and `signing_secrets` work with NO subprocess and NO `user.key` file. Also `pub(super) fn author_pubkey_hex() -> String` for assertions that need the author.

- [ ] **Step 1: The choke-point seam in `node_control.rs`**

At module scope (near the other statics/helpers):

```rust
/// Test-only in-process verb override. The sim lane installs a closure that
/// answers the `user-*` signing verbs with `node::encode_frame` over a
/// generated key, so `cargo test` needs no `ducktape-node` binary. An
/// installed override is authoritative: verbs it does not recognize are
/// errors, never a silent fall-through to a subprocess.
#[cfg(test)]
pub(crate) type VerbOverride =
    Box<dyn Fn(&[&str], &[&str]) -> Result<String, String> + Send + Sync>;
#[cfg(test)]
static VERB_OVERRIDE: std::sync::OnceLock<VerbOverride> = std::sync::OnceLock::new();
#[cfg(test)]
pub(crate) fn install_verb_override(handler: VerbOverride) {
    let _ = VERB_OVERRIDE.set(handler);
}
```

At the very top of `run_verb_inner` (before `resolve_node_bin`):

```rust
#[cfg(test)]
if let Some(handler) = VERB_OVERRIDE.get() {
    return handler(args, stdin_lines);
}
```
(Match the real parameter names/types of `run_verb_inner` — `args: &[&str]`, secret lines slice — read the function head first.)

In `backend/mod.rs`, alongside the existing `pub(crate)`/`pub(super)` re-exports:
```rust
#[cfg(test)]
pub(crate) use node_control::{VerbOverride, install_verb_override};
```

- [ ] **Step 2: The override implementation — `shell/sim/signing.rs`**

```rust
//! In-process frame signing for the sim lane: answers the two verbs the
//! chat write path uses (`user-key status`, `user-sign-frame`) with
//! `node::encode_frame` over one generated ed25519 key — no user.key file,
//! no subprocess. Installed once per process; the key is shared by every
//! test in the binary (like a fixture account).

use std::sync::OnceLock;

// exact signer type: whatever `node::encode_frame`'s first parameter is —
// commonware_cryptography::ed25519::PrivateKey (see crates/kernel/node/src/lib.rs:203)

fn signer() -> &'static Ed25519PrivateKey {
    static KEY: OnceLock<Ed25519PrivateKey> = OnceLock::new();
    KEY.get_or_init(|| /* generate: use the crate's documented constructor —
        e.g. from a fixed 32-byte seed so authorship is stable across runs */)
}

pub(super) fn author_pubkey_hex() -> String {
    // hex of signer().public_key() — read the crypto crate's accessor
}

pub(super) fn install() {
    crate::backend::install_verb_override(Box::new(|args, stdin| {
        match args {
            // signing_secrets() + identity_state() run: user-key status --key <path>
            ["user-key", "status", ..] => Ok(/* the status stdout whose
                last_line parse_key_status() reads as a PLAINTEXT key with
                our pubkey — read app/src-iced/src/backend/identity.rs
                parse_key_status for the exact line format and reproduce it */),
            // sign_frame() runs: user-sign-frame --key <p> --target <t> --seq <n>
            ["user-sign-frame", rest @ ..] => {
                let target = flag_value(rest, "--target")?;
                let seq: u64 = flag_value(rest, "--seq")?.parse().map_err(|e| format!("seq: {e}"))?;
                let payload_hex = stdin.last().ok_or("user-sign-frame: missing payload line")?;
                let payload = hex_decode(payload_hex)?; // small local helper or the node crate's
                let frame = node::encode_frame(signer(), seq, &sdk::Msg {
                    target: target.to_string(),
                    payload,
                });
                Ok(hex_encode(&frame))
            }
            other => Err(format!("verb override: unhandled verb {:?}", other.first())),
        }
    }));
}
```
This sketch is intent — bind it to the real signatures: read `node::encode_frame` (crates/kernel/node/src/lib.rs:203) for the exact `Msg` shape and signer type, `backend/identity.rs` `parse_key_status`/`last_line` for the status format, and `backend/signing.rs` `sign_frame` for the argv/stdin order it sends (payload hex is the LAST stdin line, after the — for plaintext keys empty — secrets). `flag_value`/`hex_decode`/`hex_encode` are ~15 lines of local helpers (or reuse the node crate's hex helpers if public).

- [ ] **Step 3: Unit test at the bottom of `signing.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The override produces a frame the kernel verifies: encode via the
    /// override path (as Backend would), decode via node's verify path.
    #[test]
    fn override_signs_verifiable_frames() {
        install();
        // call the override through the same entrypoint Backend uses:
        let payload = serde_json::to_vec(&serde_json::json!({"noop": {}})).expect("payload");
        let hex_payload = /* hex of payload */;
        let out = /* invoke crate::backend's run_verb_with_stdin-equivalent path,
                     or call the installed closure directly via a small
                     test-only accessor — simplest: reproduce the argv
                     ["user-sign-frame","--key","/dev/null","--target","chat","--seq","7"]
                     against install()'s closure logic by calling
                     crate::backend::… — if no clean path exists, factor
                     signing.rs's match body into a testable fn and call THAT */;
        let frame = /* hex-decode out */;
        // node's decode/verify (crates/kernel/node/src/lib.rs:219) must accept it:
        let decoded = node::decode_frame(&frame).expect("frame verifies");
        assert_eq!(decoded_target, "chat");
    }
}
```
Bind to the real decode API (name/shape at `crates/kernel/node/src/lib.rs:219`); the assertion that matters is decode/verify succeeds and the target round-trips. If decode is private or shaped differently, assert instead via a live simnode: `POST /v1/submit/frame` of the produced frame returns 200 (boot one with `simnode::boot` — Task 3's harness makes this trivial; in that case move this test to Task 3 and say so in the report).

- [ ] **Step 4: Run + commit**

```bash
CARGO_INCREMENTAL=0 cargo test -p ducktape-iced shell::sim::signing -- --nocapture
git add app/src-iced/src
git commit -m "test(sim-lane): in-process frame signing — verb override at the node_control choke-point

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: SimShell — embedded boot + task pump

**Files:**
- Modify: `app/src-iced/src/shell/sim/mod.rs` (the harness)

**Interfaces:**
- Consumes: `simnode::{boot, SimOpts, SimHandle}` (dev-dep), `signing::install()` from Task 2.
- Produces (Task 4 relies on, all `pub(super)`):
  - `struct SimShell`; `fn boot() -> SimShell` (NOT Option — self-contained, panics on real failure)
  - `fn click(&mut self, role: Role, name: &str)`
  - `fn click_and_type(&mut self, role: Role, name: &str, text: &str)`
  - `fn inject(&mut self, message: Message)` — also the tick affordance
  - `fn has(&self, role: Role, name: &str) -> bool`, `fn sees_text(&self, text: &str) -> bool`
  - `fn shell(&self) -> &Shell`
  - `fn node_query(&self, target: &str, query: serde_json::Value) -> serde_json::Value` — chain-side asserts over the in-process listener

- [ ] **Step 1: Write the harness (replaces the module doc-only body)**

```rust
mod signing;

use std::collections::VecDeque;
use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use futures_util::StreamExt as _;
use iced_agent_plugin::Role;
use iced_agent_plugin::selector::by;
use iced_winit::runtime::{Action, task};

use super::*;
use crate::backend::Backend;

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
        signing::install();
        let storage = tempfile::tempdir().expect("sim storage");
        let sim = simnode::boot(
            storage.path(),
            "127.0.0.1:0".parse().expect("addr"),
            simnode::SimOpts { auto: true, ..Default::default() },
        )
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
        state.node_client =
            Some(NodeClient::local(sim.addr().port()).expect("sim node client"));
        state.backend = Some(backend);
        let id = state.desktop.main.expect("preset opens a main window");
        Self { state, id, rt, sim, queue: VecDeque::new(), _storage: storage, _identity_root: identity_root }
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
    /// wrapper and for timer ticks (nothing is asynchronous in this lane).
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
            let Some(mut stream) = task::into_stream(queued) else { continue };
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
```

- [ ] **Step 2: Boot smoke test at the bottom of `mod.rs`**

The Navigate→LoadChat round-trip proves the pump executed a real HTTP query against the embedded node: a fresh sim has zero channels, and only a completed `ChatLoaded` turns `chat.data` from `Resource::Loading` into `Resource::Empty`.

```rust
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
```
(`Resource` import path: `chat.data` is `Resource<ChatData>` per `screens/chat.rs:161`; if `Resource` lives elsewhere the compiler error names it — fix the `use`, not the assertion.)

- [ ] **Step 3: Run**

```bash
CARGO_INCREMENTAL=0 cargo test -p ducktape-iced shell::sim -- --nocapture
```
Expected: signing test + boot test pass. Debug notes:
- Backend fixture panic → the override isn't reached: check `run_verb_inner`'s override check sits BEFORE `resolve_node_bin` (a "no trusted executable ducktape-node" error means it didn't).
- `chat.data` stays `Loading` → the pump never fed `ChatLoaded` back; check `Message::UserScreen(user_screens::Message::Service(ServiceEvent::ChatLoaded(..)))` arrives (load task built at `shell.rs:2352`).
- `Resource::Error` → the query failed; the error string is in the variant.

- [ ] **Step 4: Commit**

```bash
git add app/src-iced/src/shell/sim/
git commit -m "test(sim-lane): SimShell harness — embedded sim boot + task pump

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Proof scenarios — chat round-trip + duplicate-channel rejection

**Files:**
- Create: `app/src-iced/src/shell/sim/chat.rs`
- Modify: `app/src-iced/src/shell/sim/mod.rs` (add `mod chat;`)

**Interfaces:**
- Consumes: the full `SimShell` API from Task 3.
- Produces: nothing downstream — these are the lane's proof.

**Flow facts (verified against the tree, exact strings):**
- Nav button: `by::role(Role::Button, "Chat")` → `Message::Navigate(Screen::Chat)` → chained `LoadChat` queries `chat`/`"channels"`.
- Create form: `Role::Button, "+"` toggles it (traced in the EMPTY chat shell, `screens/chat.rs:540`); the channel-name input is a BARE `TextInput` (placeholder `"channel name"`, NO Sem wrapper — `field()` at `screens/user.rs:976`), so the draft is set by injecting `ChatMessageEvent::ChannelNameChanged`; `Role::Button, "Create channel"` submits (disabled until non-empty draft).
- Create wire: signed frame, target `chat`, `{"create_channel":{"channel_id":<slug>,"name":…,"post_policy":"open"}}`. On Ok the reducer auto-chains `LoadChat` (`screens/user.rs:758-767`) — the re-render is node data, not local echo.
- Composer: `Role::TextInput, "Message composer"` (Sem-wrapped `text_editor`), send via `Role::Button, "Send"`; wire `{"post_message":{…}}`; same auto-chained `LoadChat`.
- Channel rail entry: `Role::ListItem, <channel name>`.
- Rejection: duplicate `slug(name)` → chat module `Error::Module("channel already exists: <id>")` (`crates/apps/chat/src/lib.rs:440`) → lands in `Shell.user_screens.chat.error` (`screens/user.rs:762`), auto-refresh suppressed, NOT rendered by `chat::view` (verified: no view read of `state.error`) — assert state + unchanged rail, not a widget.

- [ ] **Step 1: Write `sim/chat.rs`**

```rust
//! Chat transaction round-trips — the sim lane's proof scenarios.

use super::super::*;
use super::SimShell;
use crate::screens::chat::ChatMessageEvent;
use crate::screens::user::Resource;
use iced_agent_plugin::Role;

/// Create a channel through the UI. The name input is a bare TextInput
/// (no Sem wrapper), so the draft change message is injected; the toggle
/// and submit are real widget interactions.
fn create_channel(ui: &mut SimShell, name: &str) {
    ui.click(Role::Button, "+");
    ui.inject(Message::UserScreen(user_screens::Message::Chat(
        ChatMessageEvent::ChannelNameChanged(name.into()),
    )));
    ui.click(Role::Button, "Create channel");
}

#[test]
fn create_channel_and_post_message_round_trip() {
    let mut ui = SimShell::boot();
    ui.inject(Message::Navigate(Screen::Chat));
    assert!(
        matches!(ui.shell().user_screens.chat.data, Resource::Empty),
        "fresh sim has no channels"
    );

    create_channel(&mut ui, "qa-lane");

    assert!(
        ui.shell().user_screens.chat.error.is_none(),
        "create failed: {:?}",
        ui.shell().user_screens.chat.error
    );
    assert!(ui.has(Role::ListItem, "qa-lane"), "committed channel renders in the rail");
    let channels = ui.node_query("chat", serde_json::json!("channels"));
    assert!(
        channels.to_string().contains("qa-lane"),
        "channel is committed node-side: {channels}"
    );

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

#[test]
fn duplicate_channel_rejection_lands_in_error_and_chains_no_refresh() {
    let mut ui = SimShell::boot();
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
    // The rejected submit chained no refresh and corrupted nothing.
    assert!(ui.has(Role::ListItem, "dup"));
    let channels = ui.node_query("chat", serde_json::json!("channels"));
    let listed = channels.to_string().matches("dup").count();
    assert!(listed >= 1, "committed list unchanged: {channels}");
}
```

Fallbacks with exact code, keep whichever passes and delete the other:
- If `click(Role::Button, "+")` fails in the NON-empty view (second create): grep `ToggleNewChannel` in `screens/chat.rs` for the label the non-empty rail uses and adjust the selector — the message chain is identical.
- If `click_and_type` doesn't reach the text_editor (Send disabled / blank post): inject the edit, keep Send as a real click:
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
  (Exact `Composer` field names: `screens/chat.rs:275`.)

- [ ] **Step 2: Run the lane**

```bash
CARGO_INCREMENTAL=0 cargo test -p ducktape-iced shell::sim -- --nocapture
```
Expected: 4 passed (signing, boot, round-trip, rejection).

- [ ] **Step 3: Commit**

```bash
git add app/src-iced/src/shell/sim/
git commit -m "test(sim-lane): chat round-trip + duplicate-channel rejection against the embedded sim

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Wire into `make ui-qa`, doctrine docs, gates, PR

**Files:**
- Modify: `Makefile` (`ui-qa` target, ~line 431)
- Modify: `skills/qa/SKILL.md` (lane doctrine)
- Modify: `app/src-iced/src/test/mod.rs` (doc pointer)

- [ ] **Step 1: Makefile** — add ONE line at the top of `ui-qa` (no prebuild, no env — the lane is self-contained):

```make
ui-qa:
	CARGO_INCREMENTAL=0 $(CARGO) test -p ducktape-iced shell::sim
	CARGO_INCREMENTAL=0 $(CARGO) test -p ducktape-iced qa_recipes
	$(CARGO) build -p ducktape-iced --bin ducktape-iced
	ops/iced-fleet up 2 --preset ui-demo
	ops/iced-fleet run qa/recipes/*.json; status=$$?; ops/iced-fleet down; exit $$status
```

- [ ] **Step 2: `skills/qa/SKILL.md`** — add the sim lane to the doctrine (match the file's existing format; extend, don't restructure):

```markdown
- **Sim lane (`shell::sim`)** — transaction round-trips and node-visible
  flows: the in-process shell (Simulator + a task pump executing `update()`
  Tasks on tokio) against an EMBEDDED `simnode::boot` node — self-contained,
  `cargo test -p ducktape-iced shell::sim`, no external binaries.
  Deterministic: no timers, no subscriptions; timer refreshes are injected
  messages. What belongs here: submit → commit → re-render flows, module
  rejections surfacing in UI state. What does NOT: anything needing a real
  window (`lane: fleet`), pure view variants (`src/test/`), recipe-provable
  navigation (`lane: both`).
```

- [ ] **Step 3: `app/src-iced/src/test/mod.rs`** — extend the "What does NOT belong here" doc:

```rust
//! Transaction round-trips against a deterministic embedded node belong in
//! the sim lane (`src/shell/sim/`, `cargo test -p ducktape-iced shell::sim`).
```

- [ ] **Step 4: Gates**

```bash
find app/src-iced/src/shell/sim app/src-iced/src/backend/node_control.rs -name '*.rs' -exec touch {} +
CARGO_INCREMENTAL=0 cargo clippy -p ducktape-iced --tests --no-deps
CARGO_INCREMENTAL=0 cargo test -p ducktape-iced
```
Expected: clippy clean on touched files; full crate green (sim lane + qa_recipes + screen tests + shell unit tests).

- [ ] **Step 5: Commit, push, PR against `feat/iced-app`**

```bash
git add Makefile skills/qa/SKILL.md app/src-iced/src/test/mod.rs
git commit -m "test(sim-lane): wire shell::sim into make ui-qa + lane doctrine

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push -u origin feat/iced-sim-lane
gh pr create --base feat/iced-app --title "test(sim-lane): embedded-simnode transaction round-trips for the iced app" --body "$(cat <<'EOF'
## Summary
- New sim lane (`app/src-iced/src/shell/sim/`): the iced twin of the TS `app/src/test/sim/` suites, foundry-style — `SimShell::boot()` EMBEDS a deterministic node via `simnode::boot` (dev's new lib), points the app's real `NodeClient` at the in-process listener, and executes `update()` Tasks via `iced_winit::runtime::task::into_stream` on a private runtime. Plain `cargo test -p ducktape-iced` is self-contained: no prebuilt binaries.
- In-process frame signing: a `#[cfg(test)]` verb override at `run_verb_inner` answers `user-key status`/`user-sign-frame` with `node::encode_frame` over a generated key — production subprocess path untouched.
- Proof scenarios: chat create+post round-trip (committed data re-renders; also asserted node-side via `/v1/query`), duplicate-channel rejection landing in `chat.error` with no chained refresh.
- `make ui-qa` runs the lane first; lane doctrine updated in `skills/qa/SKILL.md`.

Design: `docs/superpowers/specs/2026-07-16-iced-sim-lane-design.md` (v2); phase A = simnode lib (merged to dev).

## Verification
- `CARGO_INCREMENTAL=0 cargo test -p ducktape-iced` — state actual counts.
- `cargo clippy -p ducktape-iced --tests --no-deps` — clean on touched files.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 6: Report** — counts, gates, deviations (which fallbacks were taken).

---

## Self-Review (authoring time)

- **Spec coverage (v2):** embedded boot (T3), signing override (T2), pump/tick (T3), proofs (T4), self-containment — no binaries/env (T1 deps + T3 boot), Make/doctrine (T5). v1 leftovers (child spawn, loud-skip, DUCKTAPE_* env) are reverted in T1.
- **Placeholders:** the deliberately compiler-bound spots (Resource path, ed25519 constructor, status-line format, decode API) each name the exact file:line to read — no "handle appropriately".
- **Type consistency:** `SimShell::boot() -> Self` (no Option) consistent across T3/T4; `node_query` name consistent; `signing::install()` idempotent per OnceLock.
