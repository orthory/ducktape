# iced-agent-plugin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Full tauri-agent parity for the native iced shell — semantic tree, `@ref` find, input drive, screenshots, logs, state, CLI + stdio MCP — plus real OS AccessKit fed from the same tree, via a vendored 3-seam `iced_winit` fork.

**Architecture:** Vendored `iced_winit 0.14.0` fork (`third_party/iced-winit`, applied via `[patch.crates-io]`) carries three `// AGENT SEAM` blocks: an `accesskit_winit::Adapter` per window, a `set_tree` push, and synthetic iced-core event injection into the real input path. A new `iced-agent-plugin` crate provides the `sem()` tagging widget, the Operation-based tree collector, a loopback JSON-lines bridge publishing `endpoint.json`, and the tool handlers. A bun CLI and a dependency-free stdio MCP server speak the bridge protocol. All agent code compiles only under `debug_assertions`.

**Tech Stack:** Rust (iced 0.14.0 pinned, winit 0.30.13, accesskit_winit =0.33.2 / accesskit =0.24.1, tokio, serde, image), bun/TypeScript for CLI+MCP, AT-SPI (busctl) for a11y verification.

## Global Constraints

- iced stays pinned `=0.14.0`; the fork keeps version `0.14.0` so `[patch.crates-io]` resolves graph-wide.
- Every fork modification is bracketed by `// AGENT SEAM` comments and gated `#[cfg(feature = "agent")]`; app-side agent wiring is additionally gated `#[cfg(debug_assertions)]`. A release binary compiles none of it.
- App Tauri-era discovery id stays: `com.ducktape.app`. Endpoint path: `${XDG_RUNTIME_DIR|TMPDIR|TMP}/iced-agent/com.ducktape.app/endpoint.json`.
- `@ref` handles are valid only until the next `tree`/`find` snapshot (tauri-agent convention).
- Bridge is loopback-only; curated intents and curated state projections only; no eval analog; never serialize key material, capability URLs, or secrets into tree/state/logs.
- Use `tracing`, never `println!` (CLI/MCP TS bins may print — they are program output).
- Lint gate per touched crate: `cargo clippy -p <crate> --tests --no-deps`. No `cargo fmt --all`.
- Workspace uses bun, never npm (`package-lock.json` must never exist).
- Run all commands from the worktree root (`.worktree/iced-agent-plugin`).
- Linux dev run: `DUCKTAPE_NODE_BIN="$(pwd)/target/debug/ducktape-node" cargo run -p ducktape-iced` under Xvfb with `WEBKIT_DISABLE_DMABUF_RENDERER=1`-style env not needed (no WebKit); CEF needs X11 (`force_x11` already in `lib.rs`).

---

### Task 1: Vendor the iced_winit fork and patch it in

**Files:**
- Create: `third_party/iced-winit/` (vendored source of crates.io `iced_winit-0.14.0`)
- Create: `third_party/iced-winit/AGENT-FORK.md`
- Modify: `Cargo.toml` (workspace root: `[patch.crates-io]`)

**Interfaces:**
- Produces: a path-patched `iced_winit` compiled into `ducktape-iced` with zero behavior change; later tasks add seams to it.

- [ ] **Step 1: Vendor the crate source**

```bash
cd "$(git rev-parse --show-toplevel)"
curl -sL https://static.crates.io/crates/iced_winit/iced_winit-0.14.0.crate -o /tmp/iced_winit.crate
mkdir -p third_party
tar xzf /tmp/iced_winit.crate -C third_party
mv third_party/iced_winit-0.14.0 third_party/iced-winit
rm -f third_party/iced-winit/Cargo.toml.orig third_party/iced-winit/.cargo_vcs_info.json
```

- [ ] **Step 2: Write the fork provenance note**

Create `third_party/iced-winit/AGENT-FORK.md`:

```markdown
# iced_winit agent fork

Vendored from crates.io `iced_winit 0.14.0` (unmodified base).
Applied graph-wide via `[patch.crates-io]` in the workspace root.

Local modifications are exactly the blocks marked `// AGENT SEAM`, all gated
behind `feature = "agent"`:
1. AccessKit adapter per window (`src/agent.rs`, hooks in `src/lib.rs`).
2. `agent::set_tree(window::Id, TreeUpdate)` push into the adapter.
3. Synthetic iced-core event injection into the runtime event path.

Upstream-shaped on purpose: candidate for an iced a11y PR. When bumping iced,
re-vendor the new iced_winit and re-apply the seams (diff against this base).
```

- [ ] **Step 3: Patch the workspace**

In root `Cargo.toml`, append (bottom of file):

```toml
[patch.crates-io]
iced_winit = { path = "third_party/iced-winit" }
```

- [ ] **Step 4: Verify the patch resolves and nothing changed**

```bash
cargo tree -p ducktape-iced -i iced_winit | head -3
```
Expected: `iced_winit v0.14.0 (<worktree>/third_party/iced-winit)` — path source, not registry.

```bash
cargo check -p ducktape-iced --lib
```
Expected: clean check (first run recompiles the iced stack).

- [ ] **Step 5: Commit**

```bash
git add third_party/iced-winit Cargo.toml Cargo.lock
git commit -m "chore(agent): vendor iced_winit 0.14.0 as the agent fork base"
```

---

### Task 2: Fork seam — `agent` module (registry, adapter, injection queue)

**Files:**
- Create: `third_party/iced-winit/src/agent.rs`
- Modify: `third_party/iced-winit/src/lib.rs` (module decl + 3 hook sites)
- Modify: `third_party/iced-winit/Cargo.toml` (feature + deps)

**Interfaces:**
- Produces (all `pub`, consumed by the plugin crate in Tasks 4–7):
  - `iced_winit::agent::set_tree(id: iced_core::window::Id, update: accesskit::TreeUpdate)`
  - `iced_winit::agent::take_action_rx() -> Option<std::sync::mpsc::Receiver<(iced_core::window::Id, accesskit::ActionRequest)>>`
  - `iced_winit::agent::inject(id: iced_core::window::Id, event: iced_core::Event)`
  - `iced_winit::agent::window_ids() -> Vec<iced_core::window::Id>`
- The fork re-exports `accesskit` so plugin and fork share one version: `pub use accesskit;` under the feature.

- [ ] **Step 1: Add feature + dependencies to the fork manifest**

In `third_party/iced-winit/Cargo.toml`:

```toml
[features]
# ...existing features stay...
agent = ["dep:accesskit", "dep:accesskit_winit"]
```

and under dependencies:

```toml
[dependencies.accesskit]
version = "=0.24.1"
optional = true

[dependencies.accesskit_winit]
version = "=0.33.2"
optional = true
```

- [ ] **Step 2: Write `src/agent.rs`**

```rust
//! AGENT SEAM: dev-only agent instrumentation for the iced event loop.
//!
//! Three seams: (1) an AccessKit adapter attached to every window at creation,
//! (2) `set_tree` pushing semantic tree updates into that adapter, and
//! (3) `inject` feeding synthetic iced-core events into the same runtime path
//! real input takes. Everything is process-global because the event loop owns
//! the windows and the app-side plugin only holds `window::Id`s.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};

use crate::core::window::Id;

/// Per-window adapter + the winit window it belongs to.
struct Slot {
    adapter: accesskit_winit::Adapter,
    window: Arc<winit::window::Window>,
    /// Last tree pushed, replayed on activation and dumpable for QA.
    last_tree: Arc<Mutex<Option<accesskit::TreeUpdate>>>,
}

struct Globals {
    slots: Mutex<HashMap<Id, Slot>>,
    winit_to_iced: Mutex<HashMap<winit::window::WindowId, Id>>,
    injected: Mutex<Vec<(Id, crate::core::Event)>>,
    injected_flag: AtomicBool,
    actions_tx: Sender<(Id, accesskit::ActionRequest)>,
    actions_rx: Mutex<Option<Receiver<(Id, accesskit::ActionRequest)>>>,
}

fn globals() -> &'static Globals {
    static G: OnceLock<Globals> = OnceLock::new();
    G.get_or_init(|| {
        let (tx, rx) = channel();
        Globals {
            slots: Mutex::new(HashMap::new()),
            winit_to_iced: Mutex::new(HashMap::new()),
            injected: Mutex::new(Vec::new()),
            injected_flag: AtomicBool::new(false),
            actions_tx: tx,
            actions_rx: Mutex::new(Some(rx)),
        }
    })
}

/// Replays the last pushed tree when the platform activates accessibility.
struct Activation {
    last_tree: Arc<Mutex<Option<accesskit::TreeUpdate>>>,
}

impl accesskit::ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        self.last_tree.lock().unwrap().clone()
    }
}

struct Actions {
    id: Id,
    tx: Sender<(Id, accesskit::ActionRequest)>,
}

impl accesskit::ActionHandler for Actions {
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        let _ = self.tx.send((self.id, request));
    }
}

struct Deactivation;
impl accesskit::DeactivationHandler for Deactivation {
    fn deactivate_accessibility(&mut self) {}
}

/// Seam 1: called by the event loop right after `create_window`, while the
/// window is still invisible (the adapter requires pre-visibility creation).
pub(crate) fn attach(
    event_loop: &winit::event_loop::ActiveEventLoop,
    id: Id,
    window: &Arc<winit::window::Window>,
) {
    let g = globals();
    let last_tree = Arc::new(Mutex::new(None));
    let adapter = accesskit_winit::Adapter::with_direct_handlers(
        event_loop,
        window,
        Activation { last_tree: Arc::clone(&last_tree) },
        Actions { id, tx: g.actions_tx.clone() },
        Deactivation,
    );
    g.winit_to_iced.lock().unwrap().insert(window.id(), id);
    g.slots.lock().unwrap().insert(
        id,
        Slot { adapter, window: Arc::clone(window), last_tree },
    );
}

/// Seam 1: forward every winit window event to the window's adapter.
pub(crate) fn process_event(
    winit_id: winit::window::WindowId,
    event: &winit::event::WindowEvent,
) {
    let g = globals();
    let Some(id) = g.winit_to_iced.lock().unwrap().get(&winit_id).copied() else {
        return;
    };
    if let Some(slot) = g.slots.lock().unwrap().get_mut(&id) {
        slot.adapter.process_event(&slot.window, event);
    }
    if matches!(event, winit::event::WindowEvent::Destroyed) {
        g.slots.lock().unwrap().remove(&id);
        g.winit_to_iced.lock().unwrap().remove(&winit_id);
    }
}

/// Seam 2: push a semantic tree for a window (app-side plugin calls this).
pub fn set_tree(id: Id, update: accesskit::TreeUpdate) {
    let g = globals();
    if let Some(slot) = g.slots.lock().unwrap().get_mut(&id) {
        *slot.last_tree.lock().unwrap() = Some(update.clone());
        slot.adapter.update_if_active(|| update);
    }
}

/// Seam 2: the plugin takes the (single) ActionRequest receiver at boot.
pub fn take_action_rx() -> Option<Receiver<(Id, accesskit::ActionRequest)>> {
    globals().actions_rx.lock().unwrap().take()
}

/// Dump the last tree pushed for a window (the `iced_a11y` tool).
pub fn last_tree(id: Id) -> Option<accesskit::TreeUpdate> {
    globals()
        .slots
        .lock()
        .unwrap()
        .get(&id)
        .and_then(|slot| slot.last_tree.lock().unwrap().clone())
}

/// Seam 3: queue a synthetic iced-core event; the loop drains it into the
/// same per-window event vector real input lands in, then redraws.
pub fn inject(id: Id, event: crate::core::Event) {
    let g = globals();
    g.injected.lock().unwrap().push((id, event));
    g.injected_flag.store(true, Ordering::Release);
    if let Some(slot) = g.slots.lock().unwrap().get(&id) {
        slot.window.request_redraw();
    }
}

/// Seam 3: drained by the event loop each cycle.
pub(crate) fn drain_injected() -> Vec<(Id, crate::core::Event)> {
    let g = globals();
    if !g.injected_flag.swap(false, Ordering::AcqRel) {
        return Vec::new();
    }
    std::mem::take(&mut *g.injected.lock().unwrap())
}

/// Windows currently alive (the `iced_windows` tool).
pub fn window_ids() -> Vec<Id> {
    globals().slots.lock().unwrap().keys().copied().collect()
}
```

- [ ] **Step 3: Declare the module + re-export in `src/lib.rs`**

Near the other `mod` declarations at the top of `third_party/iced-winit/src/lib.rs` (around line 33, `mod proxy;`):

```rust
// AGENT SEAM: dev-only agent instrumentation (adapter, tree push, injection).
#[cfg(feature = "agent")]
pub mod agent;
#[cfg(feature = "agent")]
pub use accesskit;
```

- [ ] **Step 4: Hook window creation**

In `src/lib.rs`, find the window-creation site (~line 349): `let window = event_loop.create_window(window_attributes).expect("Create window");`. The surrounding block handles `Control::CreateWindow { id, .. }` and later wraps the window in `Arc` for `Event::WindowCreated`. Locate where the `Arc<winit::window::Window>` is constructed (the `window` sent in `Event::WindowCreated { window, .. }`) and insert immediately after the Arc exists, before any `set_visible(true)`:

```rust
// AGENT SEAM: attach the AccessKit adapter while the window is invisible.
#[cfg(feature = "agent")]
crate::agent::attach(event_loop, id, &window);
```

(If the Arc is created inline in the `Event::WindowCreated` constructor, bind it to a local first: `let window = Arc::new(window);` then pass the local.)

- [ ] **Step 5: Hook window-event forwarding**

In the `ApplicationHandler::window_event` impl (~line 200), before the event is wrapped into `Event::EventLoopAwakened(...)`:

```rust
// AGENT SEAM: adapters see every window event (activation, focus, bounds).
#[cfg(feature = "agent")]
crate::agent::process_event(window_id, &event);
```

- [ ] **Step 6: Hook synthetic event drain**

In `run_instance`, find where accumulated window events are drained into UI updates: `for (id, event) in events.drain(..)` (~line 1204) — the `events: Vec<(window::Id, core::Event)>` vector filled at ~line 1118 (`events.push((id, event));`). Immediately **before** the drain loop:

```rust
// AGENT SEAM: synthetic events enter the same path real input takes.
#[cfg(feature = "agent")]
events.extend(crate::agent::drain_injected());
```

- [ ] **Step 7: Verify both feature states compile**

```bash
cargo check -p iced_winit
cargo check -p iced_winit --features agent
```
Expected: both clean. (First confirms zero-impact default; second compiles the seams. Fix exact local variable names at the three hook sites as needed — the anchors above are from the vendored base.)

- [ ] **Step 8: Commit**

```bash
git add third_party/iced-winit
git commit -m "feat(agent): iced_winit fork seams — AccessKit adapter, tree push, event injection"
```

---

### Task 3: Enable the agent feature through the app + prove AccessKit end-to-end (P0 spike gate)

**Files:**
- Modify: `app/src-iced/Cargo.toml` (feature plumb)
- Modify: `app/src-iced/src/shell.rs` (dev-only stub tree push after boot)
- Test: manual AT-SPI probe under Xvfb (this task IS the risk gate)

**Interfaces:**
- Consumes: `iced_winit::agent::{set_tree, inject}` from Task 2.
- Produces: proof that (a) AT-SPI exposes our pushed nodes, (b) an injected synthetic click flips real app state. Later tasks replace the stub with the real tree.

- [ ] **Step 1: Plumb the feature**

In `app/src-iced/Cargo.toml`:

```toml
[features]
default = ["cef-browser", "agent"]
cef-browser = ["dep:cef", "dep:cef-dll-sys", "dep:x11-dl"]
agent = ["iced_winit/agent"]

[dependencies]
# add alongside iced:
iced_winit = { version = "=0.14.0", default-features = false }
```

(`iced_winit` must be a direct dep to name its feature and call `agent::*`; the patch makes it the fork. `default-features = false` — the iced facade already picks x11/wayland.)

- [ ] **Step 2: Push a stub tree once the main window opens**

In `app/src-iced/src/shell.rs`, in the `Message::MainOpened(id)` arm of `update` (find `MainOpened`), add at its start:

```rust
#[cfg(all(feature = "agent", debug_assertions))]
{
    use iced_winit::accesskit as ak;
    let mut root = ak::Node::new(ak::Role::Window);
    root.set_label("Ducktape");
    let mut probe = ak::Node::new(ak::Role::Button);
    probe.set_label("agent-probe");
    root.set_children(vec![ak::NodeId(1)]);
    iced_winit::agent::set_tree(
        id,
        ak::TreeUpdate {
            nodes: vec![(ak::NodeId(0), root), (ak::NodeId(1), probe)],
            tree: Some(ak::Tree::new(ak::NodeId(0))),
            focus: ak::NodeId(0),
        },
    );
}
```

(Exact accesskit 0.24 builder method names may differ slightly — `set_label` vs `set_name`; fix to compile, the shape is what matters.)

- [ ] **Step 3: Build and boot headless**

```bash
cargo build -p node-bin -p ducktape-iced
Xvfb :99 -screen 0 1400x900x24 -nolisten tcp &
DISPLAY=:99 DUCKTAPE_NODE_BIN="$(pwd)/target/debug/ducktape-node" \
  dbus-run-session -- cargo run -p ducktape-iced &
sleep 20
```
Expected: app boots on :99 (watch stderr for the iced window log).

- [ ] **Step 4: Prove AT-SPI sees the stub**

Within the same dbus session (run the probe under the same `dbus-run-session`, e.g. start a shell in it, or launch the app with a fixed `DBUS_SESSION_BUS_ADDRESS`):

```bash
busctl --user list | grep -i a11y
busctl --user call org.a11y.Bus /org/a11y/bus org.a11y.Bus GetAddress
```
Expected: an `org.a11y.atspi` registration from the app's pid exists. If `busctl` proves awkward, use a 10-line Python `pyatspi`/`dogtail`-free probe over the a11y bus, or the `atspi` Rust crate in a scratch bin — acceptance = a node named `agent-probe` reachable from the app's accessible root.

- [ ] **Step 5: Prove synthetic injection clicks a real widget**

Temporary probe (delete after): in the same `MainOpened` arm, schedule an injected click on the theme toggle's known screen position — instead, cleaner probe that avoids coordinates: inject a keyboard event and observe the global shortcut handler (`shell.rs:1310` `iced::event::listen_with(global_shortcut)`) fire:

```rust
#[cfg(all(feature = "agent", debug_assertions))]
{
    use iced_winit::agent;
    use iced::keyboard;
    // Synthetic Cmd/Ctrl+K opens search via the existing global shortcut.
    agent::inject(
        id,
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Character("k".into()),
            modified_key: keyboard::Key::Character("k".into()),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyK),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::COMMAND,
            text: None,
        }),
    );
}
```

Rebuild, rerun, and verify in logs/screenshot that the search overlay opened (`search::State` activates). Acceptance: injected event flowed through iced's real event path and mutated app state.

- [ ] **Step 6: Remove the injection probe (keep the stub tree until Task 6 replaces it), commit**

```bash
git add app/src-iced/Cargo.toml app/src-iced/src/shell.rs Cargo.lock
git commit -m "feat(agent): app agent feature + AccessKit/injection spike proof"
```

**GATE:** If Step 4 or 5 fails and cannot be fixed by adjusting the seam placement, STOP — re-read the spec's fallback (per-platform accesskit adapters inside the same fork seam) and consult before proceeding.

---

### Task 4: `iced-agent-plugin` crate — protocol types + semantic node model

**Files:**
- Create: `app/iced-agent-plugin/Cargo.toml`
- Create: `app/iced-agent-plugin/src/lib.rs`
- Create: `app/iced-agent-plugin/src/protocol.rs`
- Modify: `Cargo.toml` (workspace members: add `"app/iced-agent-plugin"` next to `"app/src-iced"`)
- Test: unit tests inside `protocol.rs`

**Interfaces:**
- Produces (consumed by Tasks 5–8):
  - `protocol::Request { id: u64, cmd: Cmd }`, `protocol::Response { id: u64, ok: bool, result: serde_json::Value, error: Option<String> }`
  - `protocol::Cmd` — serde-tagged enum: `Tree{window}`, `Find{window, role, name, text}`, `Click{target}`, `Type{text}`, `Press{key, modifiers}`, `Hover{target}`, `Scroll{target, dx, dy}`, `Drag{from, to}`, `State{path}`, `Intent{intent}`, `Shot{window}`, `Logs{clear}`, `Wait{cond, timeout_ms}`, `Expect{cond}`, `Windows`, `A11y{window}`
  - `protocol::SemNode { r#ref: String, role: Role, name: String, value: Option<String>, bounds: Rect, disabled: bool, focused: bool, children: Vec<SemNode> }`
  - `protocol::Role` — `Window, Button, Link, TextInput, Checkbox, Tab, List, ListItem, Heading, Label, Image, Group, Region` (`snake_case` serde), `fn to_accesskit(self) -> accesskit::Role`
  - `protocol::Intent` — `Section{name}`, `Navigate{url}`, `ToggleTheme`, `Search{query}`
  - `protocol::Cond` — `Node{role, name, exists}`, `StatePath{path, equals}`

- [ ] **Step 1: Crate skeleton**

`app/iced-agent-plugin/Cargo.toml`:

```toml
[package]
name = "iced-agent-plugin"
edition.workspace = true
version.workspace = true

[dependencies]
iced = { version = "=0.14.0", default-features = false, features = ["advanced", "tokio"] }
iced_winit = { version = "=0.14.0", default-features = false, features = ["agent"] }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { version = "1", features = ["rt", "net", "io-util", "sync", "time", "macros"] }
tracing = { workspace = true }
image = { version = "0.25", default-features = false, features = ["png"] }
base64 = { workspace = true }
```

Add `"app/iced-agent-plugin"` to workspace `members` in root `Cargo.toml`.

- [ ] **Step 2: Write `protocol.rs` with the types above + round-trip tests**

Write the enums/structs exactly as in Interfaces, `#[derive(Debug, Clone, Serialize, Deserialize)]`, `#[serde(tag = "cmd", rename_all = "snake_case")]` on `Cmd`, `#[serde(rename_all = "snake_case")]` on `Role`/`Intent`/`Cond`. `Target` is `{ r#ref: Option<String>, x: Option<f32>, y: Option<f32> }`. Include tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trips() {
        let line = r#"{"id":1,"cmd":{"cmd":"find","window":"main","role":"button","name":"Forge","text":null}}"#;
        let req: Request = serde_json::from_str(line).unwrap();
        assert!(matches!(req.cmd, Cmd::Find { .. }));
        let back = serde_json::to_string(&req).unwrap();
        let again: Request = serde_json::from_str(&back).unwrap();
        assert_eq!(req.id, again.id);
    }

    #[test]
    fn role_maps_to_accesskit() {
        assert_eq!(Role::Button.to_accesskit(), iced_winit::accesskit::Role::Button);
        assert_eq!(Role::TextInput.to_accesskit(), iced_winit::accesskit::Role::TextInput);
    }
}
```

- [ ] **Step 3: Test, lint, commit**

```bash
cargo test -p iced-agent-plugin
cargo clippy -p iced-agent-plugin --tests --no-deps
git add Cargo.toml app/iced-agent-plugin
git commit -m "feat(agent): iced-agent-plugin crate — protocol + semantic node model"
```

---

### Task 5: `sem()` tagging widget + Operation tree collector

**Files:**
- Create: `app/iced-agent-plugin/src/sem.rs` (wrapper widget)
- Create: `app/iced-agent-plugin/src/collect.rs` (Operation + snapshot store)
- Modify: `app/iced-agent-plugin/src/lib.rs`
- Test: `collect.rs` unit tests on the stack builder

**Interfaces:**
- Consumes: `protocol::{Role, SemNode}`.
- Produces:
  - `sem<'a, M: 'a>(role: Role, name: impl Into<String>, content: impl Into<iced::Element<'a, M>>) -> iced::Element<'a, M>` with builder `Sem::value()/disabled()`
  - `collect::Collector` — `iced::advanced::widget::Operation<()>` impl; `Collector::new(shared: SnapshotSlot)`
  - `collect::SnapshotSlot = Arc<Mutex<Vec<WindowSnapshot>>>`; `WindowSnapshot { window_name: String, nodes: SemNode, flat: Vec<FlatNode> }`, `FlatNode { r#ref: String, role: Role, name: String, bounds: Rect }`
  - `collect::to_accesskit(&WindowSnapshot) -> accesskit::TreeUpdate` (ref `@N` ↔ `NodeId(N)`, same numbering)

- [ ] **Step 1: Write the `Sem` wrapper widget**

`sem.rs` — a transparent wrapper `Widget` impl that delegates every method (`size`, `layout`, `draw`, `update`, `mouse_interaction`, `overlay`, `children`/`diff` via a one-child `Tree`) to the inner element, except `operate`, which brackets the child so the collector can build hierarchy:

```rust
/// Marker passed through `Operation::custom` on entry/exit of a `sem` node.
pub enum SemProbe {
    Enter { role: Role, name: String, value: Option<String>, disabled: bool },
    Exit,
}

fn operate(
    &mut self,
    tree: &mut Tree,
    layout: Layout<'_>,
    renderer: &Renderer,
    operation: &mut dyn Operation,
) {
    let mut enter = SemProbe::Enter {
        role: self.role,
        name: self.name.clone(),
        value: self.value.clone(),
        disabled: self.disabled,
    };
    operation.custom(None, layout.bounds(), &mut enter);
    self.content.as_widget_mut().operate(
        &mut tree.children[0],
        layout.children().next().unwrap(),
        renderer,
        operation,
    );
    let mut exit = SemProbe::Exit;
    operation.custom(None, layout.bounds(), &mut exit);
}
```

Follow the delegation pattern of any thin wrapper in `iced_widget` (e.g. `container` minus styling): `size/size_hint` from content, `layout` = content layout in a padded-zero node, `children()` = `vec![Tree::new(&self.content)]`, `diff` recurses. Full `Widget` impl, no shortcuts — this is the plugin's core widget.

- [ ] **Step 2: Write the collector**

`collect.rs`:

```rust
pub struct Collector {
    stack: Vec<SemNode>,
    counter: u64,
    roots: Vec<SemNode>,
    focused_bounds: Option<Rect>,
}

impl iced::advanced::widget::Operation<()> for Collector {
    fn container(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        _bounds: iced::Rectangle,
        operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation<()>),
    ) {
        operate(self);
    }

    fn custom(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        bounds: iced::Rectangle,
        state: &mut dyn std::any::Any,
    ) {
        let Some(probe) = state.downcast_mut::<crate::sem::SemProbe>() else { return };
        match probe {
            crate::sem::SemProbe::Enter { role, name, value, disabled } => {
                self.counter += 1;
                self.stack.push(SemNode {
                    r#ref: format!("@{}", self.counter),
                    role: *role,
                    name: name.clone(),
                    value: value.clone(),
                    bounds: bounds.into(),
                    disabled: *disabled,
                    focused: false,
                    children: Vec::new(),
                });
            }
            crate::sem::SemProbe::Exit => {
                let node = self.stack.pop().expect("balanced sem brackets");
                match self.stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => self.roots.push(node),
                }
            }
        }
    }

    fn focusable(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        bounds: iced::Rectangle,
        state: &mut dyn iced::advanced::widget::operation::Focusable,
    ) {
        if state.is_focused() {
            if let Some(top) = self.stack.last_mut() {
                top.focused = true;
            }
            self.focused_bounds = Some(bounds.into());
        }
    }
    // text_input(): copy the current value into the enclosing sem node's value.
}
```

(Check the exact `Operation` method signatures against `iced_core-0.14.0/src/widget/operation.rs` — `container` includes the recursion closure in 0.14 via `traverse`/`container`; mirror what built-in operations like `focusable::find_focused` do.)

`to_accesskit`: walk `SemNode` depth-first; `@N` → `NodeId(N)`; role via `Role::to_accesskit`; set label/bounds (`accesskit::Rect` from our `Rect`); root = `Node::new(Role::Window)` id 0 with window name; `Tree::new(NodeId(0))`.

Unit tests (pure, no iced runtime):

```rust
#[test]
fn bracket_stream_builds_hierarchy() {
    let mut c = Collector::default();
    let b = iced::Rectangle { x: 0.0, y: 0.0, width: 100.0, height: 20.0 };
    let mut enter_root = SemProbe::Enter { role: Role::Window, name: "main".into(), value: None, disabled: false };
    let mut enter_btn = SemProbe::Enter { role: Role::Button, name: "Forge".into(), value: None, disabled: false };
    let mut exit = SemProbe::Exit;
    use iced::advanced::widget::Operation;
    c.custom(None, b, &mut enter_root);
    c.custom(None, b, &mut enter_btn);
    c.custom(None, b, &mut exit);
    let mut exit2 = SemProbe::Exit;
    c.custom(None, b, &mut exit2);
    assert_eq!(c.roots.len(), 1);
    assert_eq!(c.roots[0].children[0].name, "Forge");
    assert_eq!(c.roots[0].children[0].r#ref, "@2");
}

#[test]
fn accesskit_ids_match_refs() {
    // build as above, then:
    // let update = to_accesskit(&snapshot);
    // assert node with NodeId(2) has label "Forge".
}
```

- [ ] **Step 3: Test, lint, commit**

```bash
cargo test -p iced-agent-plugin
cargo clippy -p iced-agent-plugin --tests --no-deps
git add app/iced-agent-plugin
git commit -m "feat(agent): sem() tagging widget + Operation tree collector"
```

---

### Task 6: Bridge server, endpoint registry, tool handlers

**Files:**
- Create: `app/iced-agent-plugin/src/bridge.rs` (TCP server + endpoint.json)
- Create: `app/iced-agent-plugin/src/tools.rs` (command execution)
- Create: `app/iced-agent-plugin/src/logs.rs` (tracing ring layer)
- Modify: `app/iced-agent-plugin/src/lib.rs`
- Test: tokio integration test `app/iced-agent-plugin/tests/bridge.rs`

**Interfaces:**
- Consumes: Tasks 4–5 types; `iced_winit::agent::{inject, set_tree, last_tree, window_ids}`.
- Produces (consumed by app wiring in Task 7):
  - `AgentHandle::boot(app_id: &str) -> AgentHandle` — spawns the tokio server thread, writes endpoint.json, installs nothing iced-side.
  - `AgentHandle::snapshot_slot() -> SnapshotSlot` — shared store the app's collector task fills.
  - `AgentHandle::state_slot() -> Arc<Mutex<serde_json::Value>>` — curated app-state projection, app refreshes it.
  - `AgentHandle::drain_ui(&self) -> Vec<UiCommand>` — commands the bridge cannot satisfy itself and hands to the app inside `update()`: `UiCommand::Intent(Intent)`, `UiCommand::Shot { window, reply: tokio::sync::oneshot::Sender<Vec<u8>> }`
  - `pub fn ring_layer() -> (impl tracing_subscriber::Layer<Registry>, LogsHandle)` from `logs.rs`
  - `AgentHandle::window_map() -> Arc<Mutex<HashMap<String, iced::window::Id>>>` — app registers "main"/"huddle"/"tray".
- Endpoint file: `${XDG_RUNTIME_DIR|TMPDIR|TMP}/iced-agent/<app_id>/endpoint.json` = `{"transport":"tcp","host":"127.0.0.1","port":N,"pid":P,"cdp":null}`; dir `chmod 700`; file removed on drop/exit.

- [ ] **Step 1: `logs.rs`** — a `tracing_subscriber::Layer` pushing `(level, target, message)` into `Arc<Mutex<VecDeque<LogLine>>>` capped at 4096; `LogsHandle::snapshot()/clear()`. Unit test: emit via `tracing::info!` under a scoped subscriber, assert captured.

- [ ] **Step 2: `bridge.rs`** — `std::thread` running a tokio current-thread runtime; `TcpListener::bind("127.0.0.1:0")`; per-connection loop: read line → `serde_json::from_str::<Request>` → `tools::execute(...)` → write `Response` line. Write endpoint.json after bind (create dir with `std::os::unix::fs::PermissionsExt` 0o700; best-effort on non-unix). Remove endpoint.json in `Drop`.

- [ ] **Step 3: `tools.rs`** — `execute(cmd, ctx) -> Result<serde_json::Value, String>` with `ctx` holding snapshot slot, state slot, window map, logs handle, UI-command queue. Implement:
  - `Tree/Find`: read latest `WindowSnapshot`; `Find` filters flat nodes by role/name-substring/text; refresh happens app-side each tick, so serve the stored snapshot.
  - `Click/Hover/Scroll/Drag/Type/Press`: resolve `Target` (`@ref` → flat node bounds center; or raw x/y) → `iced_winit::agent::inject(window_id, …)` sequences:
    - click: `Mouse(CursorMoved{position})`, `Mouse(ButtonPressed(Left))`, `Mouse(ButtonReleased(Left))`
    - hover: `CursorMoved` only; scroll: `CursorMoved` + `WheelScrolled{delta: ScrollDelta::Pixels{x: dx, y: dy}}`
    - drag: `CursorMoved(from)`, `ButtonPressed`, N interpolated `CursorMoved`, `ButtonReleased` at `to`
    - type: per char `Keyboard(KeyPressed{key: Character(c), text: Some(c), ..})` + `KeyReleased`
    - press: named-key table (enter/tab/escape/backspace/delete/arrows/home/end/pageup/pagedown) → `Key::Named(..)`, modifiers parsed from `["ctrl","shift","alt","cmd"]`
  - `State{path}`: dot-path walk into the state slot JSON.
  - `Intent`: push `UiCommand::Intent`, return ack.
  - `Shot`: push `UiCommand::Shot` with oneshot; await ≤5 s; return `{png_base64}`.
  - `Logs{clear}`: from `LogsHandle`.
  - `Wait/Expect`: poll snapshot/state every 50 ms until cond or timeout (default 5000 ms).
  - `Windows`: window map + snapshot bounds. `A11y{window}`: `iced_winit::agent::last_tree(id)` serialized via `serde_json::to_value(&update.nodes.iter().map(|(id, n)| (id.0, format!("{:?}", n))).collect::<Vec<_>>())` (Debug-format nodes; accesskit Node isn't Serialize).

- [ ] **Step 4: Integration test `tests/bridge.rs`** — boot `AgentHandle` with a fabricated snapshot (one window, one button `Forge`), connect TCP, send `tree`, `find{role:button,name:Forge}`, bad JSON, unknown ref click; assert: tree returns the node, find returns `@`-ref, bad JSON → `ok:false`, unknown ref → error mentioning the ref. (Injection itself needs the live loop — covered in Task 8 e2e.)

- [ ] **Step 5: Test, lint, commit**

```bash
cargo test -p iced-agent-plugin
cargo clippy -p iced-agent-plugin --tests --no-deps
git add app/iced-agent-plugin
git commit -m "feat(agent): loopback bridge, endpoint registry, tool handlers, log ring"
```

---

### Task 7: App wiring — boot, snapshot loop, intents, first instrumentation

**Files:**
- Modify: `app/src-iced/Cargo.toml` (dep on plugin)
- Modify: `app/src-iced/src/lib.rs` (ring layer install)
- Modify: `app/src-iced/src/shell.rs` (boot handle, tick, drain, window map, remove Task 3 stub)
- Modify: `app/src-iced/src/shell/view.rs` (root `sem` wrap + chrome tagging)
- Modify: `app/src-iced/src/screens/home.rs`, `app/src-iced/src/screens/settings.rs` (first two instrumented screens)

**Interfaces:**
- Consumes: `iced_agent_plugin::{AgentHandle, sem, Role, Collector, UiCommand, ring_layer}`.
- Produces: a live app whose real tree is served and driven; the pattern every later screen copies.

- [ ] **Step 1: Dependency**

`app/src-iced/Cargo.toml`: `agent = ["iced_winit/agent", "dep:iced-agent-plugin"]`, `iced-agent-plugin = { path = "../iced-agent-plugin", optional = true }`.

- [ ] **Step 2: Install the ring layer** in `lib.rs` `run()`: replace the `fmt()` builder with `tracing_subscriber::registry()` + fmt layer + env filter; under `#[cfg(all(feature = "agent", debug_assertions))]` also `.with(ring)` where `(ring, logs_handle)` comes from the plugin; stash `logs_handle` in a `OnceLock` the shell reads at boot.

- [ ] **Step 3: Shell wiring** (`shell.rs`, all under `#[cfg(all(feature = "agent", debug_assertions))]`):
  - `Shell::boot`: `AgentHandle::boot("com.ducktape.app")`, store in `Shell`.
  - `MainOpened(id)` / `HuddleOpened(id)` / tray open: `window_map` insert; remove the Task 3 stub push.
  - New `Message::AgentTick`: subscription `iced::time::every(Duration::from_millis(150))`. Handler chains: `iced::advanced::widget::operate(Collector::new(slot))` Task, then on completion: read roots → per named window (root `sem` node name = window kind, Step 4) build `WindowSnapshot` + `to_accesskit` → `iced_winit::agent::set_tree(window_id, update)`; refresh `state_slot` (see Step 5); then `handle.drain_ui()` → `UiCommand::Intent` mapped to real Messages (`Intent::Section{name}` → `Message::Section(..)` by matching the existing `Section` enum's names, `Intent::Navigate{url}` → the existing duck-url open path, `Intent::ToggleTheme` → `Message::ToggleTheme`, `Intent::Search{query}` → open search + set query) and `UiCommand::Shot{window, reply}` → `iced::window::screenshot(id).map(...)` Task, encode PNG via `image`, send through the oneshot.
  - Also drain `iced_winit::agent::take_action_rx()` once at boot into a std thread that forwards ActionRequests as `UiCommand`-equivalent clicks: `Action::Click` on `NodeId(n)` → same path as bridge `click @n` (route through the bridge's injector so both paths share code).
- [ ] **Step 4: Root tagging in `view.rs`**: wrap each window's returned content: main → `sem(Role::Window, "main", content)`, huddle → `"huddle"`, tray → `"tray"`. Then tag the chrome in `view.rs`: nav sections (`sem(Role::Button, section_label, button)`), back/forward, search box (`Role::TextInput`, name "search"), theme toggle, notifications bell.
- [ ] **Step 5: State projection**: build `serde_json::json!({ "screen": ..., "section": ..., "history_len": ..., "workspace": active_workspace.map(id), "onboarding": ..., "unread": ..., "search_open": ... })` from `&Shell` in the tick handler — reviewed fields only, no secrets.
- [ ] **Step 6: Instrument `home.rs` + `settings.rs`**: every interactive widget gets `sem` (buttons, list items as `Role::ListItem`, headings as `Role::Heading`, inputs as `Role::TextInput` with `.value(...)`).
- [ ] **Step 7: Verify live**

```bash
cargo build -p ducktape-iced
# boot under Xvfb as in Task 3
find "${XDG_RUNTIME_DIR:-/tmp}/iced-agent" -name endpoint.json -exec cat {} \;
printf '{"id":1,"cmd":{"cmd":"tree","window":"main"}}\n' | nc 127.0.0.1 $PORT
printf '{"id":2,"cmd":{"cmd":"find","window":"main","role":"button","name":null,"text":null}}\n' | nc 127.0.0.1 $PORT
```
Expected: endpoint exists; tree shows window root + chrome + home nodes; find returns buttons with `@refs`.

- [ ] **Step 8: Gates + commit**

```bash
cargo clippy -p ducktape-iced --tests --no-deps
cargo test -p ducktape-iced
git add -A app/src-iced app/iced-agent-plugin
git commit -m "feat(agent): app wiring — snapshot loop, intents, chrome+home+settings tagging"
```

---

### Task 8: Live e2e — drive the real app through the bridge (acceptance for the core)

**Files:** none new (fixups only). Script the probes in `/tmp` or scratch.

- [ ] **Step 1: Boot headless** (Task 3 recipe). Read endpoint.json for `$PORT`.
- [ ] **Step 2: `find` a section button, `click` its `@ref`, re-`tree`** — assert the `state.section` changed (`state{path:"section"}`).
- [ ] **Step 3: `click` the search input, `type` "forge", `state{path:"search_open"}`** → true; `press{key:"escape"}` closes it.
- [ ] **Step 4: `shot`** → decode base64, `file out.png` → PNG, nonzero size.
- [ ] **Step 5: `logs`** → contains `ducktape` events; `logs{clear}` empties.
- [ ] **Step 6: `a11y{window:"main"}`** → nodes match the served tree refs; AT-SPI probe from Task 3 Step 4 now shows real labels (e.g. a section name), not `agent-probe`.
- [ ] **Step 7: `wait{cond:{role:"button",name:"Settings",exists:true}}`** returns before timeout.
- [ ] **Step 8: Commit fixups**

```bash
git add -A && git commit -m "fix(agent): live-drive fixups from bridge e2e" # only if needed
```

---

### Task 9: CLI + stdio MCP server + registration

**Files:**
- Create: `app/iced-agent-plugin/bin/iced-agent.ts` (CLI)
- Create: `app/iced-agent-plugin/bin/iced-agent-mcp.ts` (MCP stdio, zero deps)
- Create: `ops/iced-agent` (bash shim, executable)
- Modify: `.mcp.json`

**Interfaces:**
- Consumes: the bridge protocol (Task 4 JSON-lines) + endpoint discovery.
- Produces: `ops/iced-agent <cmd> ...` and MCP tools `iced_tree, iced_find, iced_click, iced_type, iced_press, iced_hover, iced_scroll, iced_drag, iced_state, iced_intent, iced_shot, iced_logs, iced_wait, iced_expect, iced_windows, iced_a11y`.

- [ ] **Step 1: `iced-agent.ts`** — resolve endpoint (`--app com.ducktape.app` default; base dir `XDG_RUNTIME_DIR || TMPDIR || /tmp`), open TCP socket, one request/response, print result JSON (or write `--out file.png` for `shot`, decoding base64). Commands mirror `Cmd` 1:1: `tree|find|click|type|press|hover|scroll|drag|state|intent|shot|logs|wait|expect|windows|a11y` with flags (`--role`, `--name`, `--window`, `--path`, `--key`, `--timeout`, positional `@ref`/text).
- [ ] **Step 2: `iced-agent-mcp.ts`** — hand-rolled MCP stdio server (~150 lines, no deps): handle `initialize` (protocolVersion `2024-11-05`), `tools/list` (the 16 tools with JSON-schema inputs), `tools/call` → same TCP round-trip → text content result. No npm deps; bun runs it directly.
- [ ] **Step 3: `ops/iced-agent` shim**

```bash
#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec bun "$repo_root/app/iced-agent-plugin/bin/iced-agent.ts" "$@"
```

- [ ] **Step 4: `.mcp.json`**

```json
{
  "mcpServers": {
    "iced-agent": {
      "command": "bun",
      "args": ["app/iced-agent-plugin/bin/iced-agent-mcp.ts"]
    }
  }
}
```

- [ ] **Step 5: Probe test (no live app needed for the handshake)**

```bash
printf '%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"probe","version":"0"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  | timeout 10 bun app/iced-agent-plugin/bin/iced-agent-mcp.ts | grep -q iced_tree && echo OK
```
Expected: `OK`. With the live app up: `ops/iced-agent tree | head` prints the tree.

- [ ] **Step 6: Commit**

```bash
git add app/iced-agent-plugin/bin ops/iced-agent .mcp.json
git commit -m "feat(agent): iced-agent CLI + stdio MCP server + registration"
```

---

### Task 10: CEF CDP exposure

**Files:**
- Modify: `app/src-iced/src/browser/mod.rs` (`on_before_command_line_processing`, ~line 465)
- Modify: `app/iced-agent-plugin/src/bridge.rs` (endpoint.json `cdp` field)

**Interfaces:**
- Produces: dev builds run CEF with `--remote-debugging-port=<free port>`; the port lands in `endpoint.json` as `"cdp": "http://127.0.0.1:<port>"`.

- [ ] **Step 1:** In the browser-process branch of `on_before_command_line_processing` (where existing switches like `use-mock-keychain` are appended), add under `#[cfg(debug_assertions)]`: pick a free port at startup (`std::net::TcpListener::bind(("127.0.0.1", 0))`, read port, drop), store in a `OnceLock<u16>`, and `command_line.append_switch_with_value(Some(&CefString::from("remote-debugging-port")), Some(&CefString::from(port.to_string().as_str())))`.
- [ ] **Step 2:** Plumb the port into `AgentHandle::boot` (an `Option<u16>` param) → `cdp` field in endpoint.json.
- [ ] **Step 3:** Verify: boot app, open Browser pane, `curl http://127.0.0.1:$CDP/json/version` returns Chromium metadata.
- [ ] **Step 4:** Gates + commit

```bash
cargo clippy -p ducktape-iced --tests --no-deps
git add app/src-iced/src/browser/mod.rs app/iced-agent-plugin/src/bridge.rs
git commit -m "feat(agent): CEF CDP endpoint in dev, published via endpoint.json"
```

---

### Task 11: Instrumentation sweep — every screen

**Files (all Modify, same `sem()` pattern as Task 7 Step 6):**
- `app/src-iced/src/screens/chat.rs`, `chat_composer.rs`, `pages.rs` + `pages/`, `explorer.rs`, `file_browser.rs`, `forge.rs` + `forge/`, `agents.rs` + `agents/`, `governance.rs`, `members.rs`, `operator.rs` + `operator/`, `terminal.rs`, `user.rs`, `workspace.rs`, `mod.rs`
- `app/src-iced/src/onboarding.rs`, `search.rs`, `notifications.rs` (view fns), `huddle_ui.rs`, `browser_chrome.rs`, `network_content.rs`

**Interfaces:** consumes `sem`/`Role` only.

- [ ] **Step 1–N (one commit per file group):** For each file: wrap interactive widgets — buttons/links (`Role::Button`/`Link`, name = visible label), inputs (`Role::TextInput` + `.value()`), list rows (`Role::ListItem`, name = primary text), tabs (`Role::Tab`), headings (`Role::Heading`), panes (`Role::Region`, name = pane title). Terminal grid → single `Role::Region` named "terminal" (content via `iced_state` later if needed); CEF pane → `Role::Region` "browser" (driven via CDP). After each group: `cargo check -p ducktape-iced` + spot `ops/iced-agent find --role button` against the live app for one screen of the group.
- [ ] **Final step: Gates + commit per group**

```bash
cargo clippy -p ducktape-iced --tests --no-deps && cargo test -p ducktape-iced
git add <group files> && git commit -m "feat(agent): sem instrumentation — <group>"
```

---

### Task 12: QA skill rewrite + docs pointer

**Files:**
- Modify: `skills/qa/SKILL.md`
- Modify: `docs/superpowers/specs/2026-07-16-iced-agent-plugin-design.md` (status → shipped, deltas if any)

- [ ] **Step 1:** Rewrite `skills/qa/SKILL.md` around the agent flow while keeping the package/lifecycle checklist: what's wired (fork feature, plugin crate, `ops/iced-agent`, `.mcp.json` server `iced-agent`, endpoint path, CDP field); drive recipes (`tree/find/click/type/state/shot/wait`); headless Xvfb bring-up; per-instance isolation via `XDG_RUNTIME_DIR`; a11y verification (`iced_a11y` + AT-SPI probe); dev-only stance; process-safety rules unchanged (no `pkill -f`).
- [ ] **Step 2:** Commit

```bash
git add skills/qa/SKILL.md docs/superpowers/specs/2026-07-16-iced-agent-plugin-design.md
git commit -m "docs(qa): agent-driven QA flow for the iced shell"
```

---

### Task 13: Full gates, release-safety audit, PR

- [ ] **Step 1: Gates**

```bash
cargo clippy -p iced-agent-plugin --tests --no-deps
cargo clippy -p ducktape-iced --tests --no-deps
cargo test -p iced-agent-plugin
cargo test -p ducktape-iced
cargo check -p files --no-default-features
```
All green (files-crate gate untouched but standing).

- [ ] **Step 2: Release-safety audit**

```bash
cargo tree -p ducktape-iced --release -e features | grep -i accesskit && echo "LEAK" || echo "clean-features"
grep -rn "debug_assertions" app/iced-agent-plugin/src app/src-iced/src/shell.rs | head
```
Note: the `agent` cargo feature is on by default, so accesskit *links* in release; the seams' `debug_assertions` gate means no adapter attaches, no server binds, no endpoint publishes. Verify: release build (`cargo build --release -p ducktape-iced`), run briefly, confirm no `iced-agent` dir appears and no listener binds.

- [ ] **Step 3: Full live e2e re-run** (Task 8 sequence + one CDP check + one huddle-window `tree --window huddle`).

- [ ] **Step 4: PR**

```bash
git push -u origin feat/iced-agent-plugin
gh pr create --base feat/iced-app --title "feat(agent): iced-agent-plugin — native agent driver with real AccessKit" --body "..."
```
PR body: spec+plan links, e2e evidence (tree/find/click/state/shot/a11y outputs), release-safety audit results, fork-maintenance note.
🤖 Generated with [Claude Code](https://claude.com/claude-code)

---

## Self-Review

**Spec coverage:** fork 3 seams → Tasks 1–2; P0 gate (AT-SPI + injection) → Task 3; sem/collector/AccessKit-native tree → Task 5; bridge/endpoint/tools → Task 6; multi-window, intents, state, logs, shot → Tasks 6–7; drive parity table → Tasks 6/8; CLI+MCP → Task 9; CDP → Task 10; sweep → Task 11; qa skill → Task 12; gates/release audit/PR → Task 13. ✓

**Placeholder scan:** Task 5 Step 1 delegates the boilerplate `Widget` delegation to the `container` pattern by reference (named source, complete `operate` shown) — acceptable as the non-novel part; Task 6 Steps are named behaviors with exact protocols and sequences; no TBDs. ✓

**Type consistency:** `Cmd`/`SemNode`/`Role`/`Intent`/`Cond` names match across Tasks 4/6/9; `@N` ↔ `NodeId(N)` invariant stated in Tasks 5/8; `AgentHandle` API consistent across Tasks 6/7/10. Fork API (`set_tree/take_action_rx/inject/window_ids/last_tree`) consistent across Tasks 2/3/6/7. ✓

**Known adjust-on-contact points (named, not placeholders):** exact local bindings at the three fork hook sites; accesskit 0.24 builder method names; `Operation` trait method exact signatures in 0.14. Each has a compile-check step immediately after.
