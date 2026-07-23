# Ducktape Logging Plan

**Status:** Phase 1 is startable today.

---

## 1. The finding

The log sink, the stream topic, and the UI **already exist, wired end to end.** They are starved of input.

- `bin/node/src/main.rs:215 init_tracing` installs a `tracing_subscriber` registry with two layers: a **stderr layer** (→ `<workspace>/daemon.log`) and a **ring layer** (→ `noded::LogRing`, 4096 lines, `stream.rs:31`).
- The ring is streamed over the websocket **`logs` topic** (`ServerFrame::Tail { TailItem::Log }`) into the desktop app's Logs tab.
- **Not one `info!`/`warn!`/`error!` exists anywhere in the workspace.** The only thing feeding that pipe is third-party commonware. Our code writes 492+ `println!`/`eprintln!` — invisible in the app, unfilterable, no fields, no ids.

Three facts, verified in the code, that set the whole design:

| # | Fact | Where | Consequence |
|---|---|---|---|
| 1 | The **stderr** layer is `EnvFilter::from_default_env()` — with `RUST_LOG` unset that is **ERROR-only**. The **ring** layer already falls back to `EnvFilter::new("info")`. | `main.rs:216-229` | `daemon.log` already carries `error!` today. `info!`/`warn!` land nowhere. **The foundation is a one-line default-directive change.** |
| 2 | `RUST_LOG` is read **once, at boot**, and there is no reload handle. The desktop spawns the node with **only `PATH`** (`daemon.rs:692 prepare_node_command_env`). | `main.rs:219`, `daemon.rs:692` | Every `debug!` we write is unreachable on a live node. Getting one means restarting — destroying the wedged state you were trying to look at. **A reload handle is not a nice-to-have; without it `debug` is dead code.** |
| 3 | `bin/noded` installs **no stderr layer at all** (`main.rs:178`, ring only). `simnode`, `coordinator`, `mcp`, `fs` install **nothing**. | 6 × `main.rs` | Tracing added to any shared crate is silently dropped in four binaries. |

This is not a "build logging" project. It is **connect the emitter to the pipe that is already there, and delete the scaffolding in the same hunk.**

---

## 2. The doctrine

### 2.1 Facade: `tracing`. Already vendored (0.1.44, transitive via commonware); `tracing-subscriber` is already a direct dep of `bin/node` + `bin/noded`. Adding it to a crate is a Cargo.toml line.

Add `tracing = { workspace = true }` to: `bin/{node,noded,simnode,coordinator,mcp,fs}`, `crates/kernel/{host,node,consensus,statesync,recovery,indexer,wasm-host}`, `crates/system/{overlay-net,wireguard,nat-traversal,reachability,data-plane,blobstore,dispatch-oracle,capability-host}`, `crates/apps/forge`, `crates/duckfs/{client,disk}`, `app/src-tauri`.

**Never — this is a gate, not a preference:**

| Blocked | Why |
|---|---|
| `crates/kernel/sdk` | Every wasm guest builds it for `wasm32-unknown-unknown`. Its own manifest: *"NO domain deps on purpose."* |
| The 9 wasm app crates (`agent`, `automations`, `chat`, `inbox`, `jobs`, `pages`, `runs`, `tasks`, `vaults`) | They compile to guests. The WIT world exposes **no log import**. Their only outbound diagnostic channel is `ctx.emit_event` — see Phase 1's drain fix. |
| `crates/duckfs/core` | Pure, sdk-free, in the wasm32 graph (`runs → duckfs-core`). Diagnostics leave as **return values**, logged by native callers. |
| `crates/apps/files` | Optional dep under the existing `native` feature only. Gate: `cargo check -p files --no-default-features`. |
| The 12 other consensus modules (`kv`, `valset`, `governance`, …) | They already return every rejection as `Result<_, String>`. Nothing is swallowed. Log the String at the **host boundary** (Phase 3) — one line covers all 13. |

### 2.2 Targets: explicit `ducktape::<plane>` strings

The crate graph does not match the debugging planes in either direction: `bin/node` alone is six planes, and the **join** plane spans `bin/node/first_contact_join.rs` + `crates/system/reachability` + `crates/system/nat-traversal`. `RUST_LOG=ducktape::join=debug` must light all three and nothing else. A crate-path target cannot express that.

```
ducktape::node          boot identity, process lifecycle, panics, fail-stop
ducktape::consensus     block apply, epoch cutover
ducktape::submit        op accept / reject — "did my write land?"
ducktape::statesync     serve + client, RangePruned, backfill
ducktape::recovery      journal replay, checkpoint, prune, schema preflight
ducktape::join          invite, first contact, gate, admission
ducktape::reachability  epoch mesh, tunnels, WG handshake
ducktape::voice         hub bind, call session, media plane
ducktape::dataplane     the substrate under voice/gateway/forge
ducktape::gateway       duck:// publish + browse
ducktape::http          the noded HTTP surface
ducktape::modules       unclaimed module events — THE wasm channel
ducktape::saga          stuck sagas
ducktape::shell         the Tauri desktop shell
```

Anything not on this list uses the **default** (crate-path) target. Audit for typos with one command — put it in the PR checklist:

```bash
grep -rho 'target: "[^"]*"' --include=*.rs bin crates app/src-tauri | sort | uniq -c
```

### 2.3 Levels — **`info` is a budget, not a level**

The ring is **4096 lines** at an `info` floor and `DRAIN_TICK = 100 ms`. One `info!` per drain tick evicts the entire ring every **6.8 minutes**. One `info!` per block (BLOCK_TIME = 1 s) holds **68 minutes**. That is the budget.

> **The rule: if it can fire more than once per block, it is not `info`.**

| Level | The rule | Examples |
|---|---|---|
| `error` | A subsystem has **stopped and will not self-heal**. A human must go fix something. | drain FATAL; index poisoned; data-plane `halted`; non-deterministic module replay; schema preflight mismatch; mesh not restored (`AlreadyCreated`); a panicking task |
| `warn` | We **refused or dropped** something for a nameable reason, or a retry loop began. **Somebody is not getting what they asked for.** | op rejected in consensus; statesync refused (not in committed standing); `RangePruned`; code push refused/corrupt; blob corrupt; HTTP 5xx |
| `info` | A **lifecycle** fact, at most once per {boot, block, epoch, session, connection, checkpoint, swap}. | boot identity; block committed; epoch cutover; tunnel up/down; call session open/close; checkpoint written |
| `debug` | **Per-op / per-request** detail. The thing you turn on for **one plane** during a hunt. | op applied; HTTP request refused (4xx); per-candidate join attempt |
| `trace` | Per-frame / per-datagram / per-tick. Never on by default. | media frame; underlay datagram; statesync chunk served |

**Three corollaries, not optional:**

1. **Retry loops need log-side backoff, independent of retry-side backoff.** `device.rs:184` (underlay recv), `data-plane/host.rs:66` (bind), `reachability/orchestrator.rs:602` (rendezvous) all retry forever. A naive `warn!` in any of them is a **log bomb that evicts the whole ring in seconds** — strictly worse than silence, because it destroys the surrounding context. Pattern: **log attempt 1, then every Nth, carrying an `attempts` counter. The counter IS the diagnosis.**
2. **Every persistent-failure event needs a matching resolution event.** A log that only prints refusals cannot distinguish "still wedged" from "healed 40 s ago". Log the transition, both ways.
3. **Adversary-drivable paths get a counter + a first occurrence, never an unconditional line.** Unknown-peer WG handshakes, coordinator auth rejects, and — critically — every 4xx from `error_response()`, which the **untrusted duck:// browse proxy shares** (`gateway_http.rs:28`).

### 2.4 Fields — and the correlation id we already mint and throw away

**`node::FrameId` is already the correlation id.** Minted at submit, held in `pending_submits`, matched in the drain, returned to the HTTP caller. It is the only thing tying a request to the block that settled or rejected it. **Nobody logs it.** That is the whole correlation story; nothing else needs threading.

```
frame       hex   per-submit. FrameId. Already minted, already matched, never logged.
height      u64   per-block.
epoch       u64   per-mesh-generation.
nonce       hex[..8]  per-invite. the ONLY id on BOTH sides of a join
                      (lobby::IntroAck.nonce == first_contact_join's token_nonce).
run_id      hex   per-agent-run.
channel_id  hex   per-call / per-presence session.
peer        hex[..4]  truncated PUBLIC key.
attempts    u64   on every retry-loop event.
reason      &str  a stable snake_case token, NOT prose: "not_in_committed_standing",
                  not "the requester was not found in the committed standing set".
                  tokens are greppable and countable; prose is neither.
```

**Two hard secrecy rules.** The ring is streamed over a ws topic documented as **unauthenticated** (`stream.rs:21`), and the shell will open that topic to remote-client users.

- **Never log a URI path or query string.** `gateway_http.rs:370` routes `/.duck/ws/{token}` — a WebSocket **capability token carried in the path**. Log axum's `MatchedPath` if you ever need a route name. (Following this rule, `error_response` below logs no path at all, and the leak is structurally impossible.)
- **Never log key material or session keys.** The invite `nonce` is safe: `governance/src/invite.rs:8` — *"the ONE key the invite admits (no bearer invites)"*, redemption needs the target's own PoP signature. Log it truncated to 8 hex anyway, matching the `peer` convention.

### 2.5 Spans: **none.**

There are **four independent tokio runtimes** — the node runtime, the reachability plane's own thread+runtime (`reachability_plane.rs:178`), the voice hub's own thread+runtime (`voice.rs:107`), and the overlay virtual-stack thread. A span opened on one is not the parent of anything in the others; a spans-first design silently produces orphaned events. **Field-keyed, not parent-keyed.** Every id above is cheap enough to carry explicitly.

If you ever add a span: never hold a `.entered()` guard across `me.execute(&mut ctx, &msg).await` (`host/src/lib.rs:1450`) — the host futures are `#[async_trait(?Send)]`. Use `.instrument()`.

### 2.6 Determinism, in one paragraph

Logging around module execution is safe **because a module cannot observe it**: `emit_msg`/`emit_event` are the only channels out of a guest, a log has no return value, and the WIT world exposes no log import — a wasm module **cannot in principle branch on whether a log fired.** The precedent and its exact wording already exist: `DrainedFrame.reason` (`node/src/lib.rs:568`) is *"node-local, NON-CONSENSUS … NEVER enters the seal, the WAL, or any hashed root."* Logging inherits that contract verbatim. Three rules:

1. Never gate control flow on a log. **`if tracing::enabled!()` is forbidden.**
2. Never emit between `BlockSink::pre_apply` and `seal` (`sync/catchup.rs:110-125`) — the WAL critical section.
3. A log cannot change consensus state, but it **can** change block *cadence*: the ring write takes a `Mutex` and pays a `format!` per event, on the emitting thread. That is why the level doctrine exists and why every in-drain event below is **bounded per block**.

---

## 3. The sinks — decided

| Sink | Fed by | Survives a crash | Read by |
|---|---|---|---|
| `<workspace>/daemon.log` | node stdout+stderr → **stderr layer @ `info` (Phase 1)** + CLI prints | ✅ (stderr is unbuffered) | Fleet, QA, `workspace_log_tail`, the LogsTab backfill |
| `LogRing` → ws `logs` topic | the same filter, ring layer | ❌ by design — it is the live tail | the app's LogsTab (and remote-client users, after S2) |
| `<app_log_dir>/Ducktape.log` | shell + panic hook (S2) | ✅ | the user, "Reveal in Finder" |

**The node needs no new file sink.** `daemon.log` *is* it — the app (`daemon.rs:505-523`) and the fleet (`ops/dev.sh:124`) already pipe stdout+stderr into it. It carries `error!` today. It carries nothing else **only because the stderr layer's default directive is ERROR.** Adding `tracing-appender` under `<storage>/logs/` would give one process two file logs, one of which nothing reads. Two real gaps, both cheap, both in Phase 1: **noded has no stderr layer**, and **`daemon.log` never rotates** (`create(true).append(true)`, forever) while only its last 64 KiB is ever read — so a long-lived node's boot sequence, where every schema/bind/reachability failure is decided, scrolls out of reach.

**The shell is the one place with genuinely no sink.** Its 19 `eprintln!`s go to `/dev/null` in every configuration a real user runs: Windows release has `windows_subsystem = "windows"` (no console); macOS Launch Services hands a GUI process `/dev/null`; a Linux `.desktop` launch inherits the session leader's stderr. Only `bun run tauri dev` and Fleet QA ever see them. **That is why this has gone unnoticed.** Fix: a `Mutex<File>` writer (`impl MakeWriter for Mutex<W>` exists — tracing-subscriber 0.3.23 `fmt/writer.rs:808`) plus the same rename-if-big. No new dependency, one rotation mechanism in the codebase instead of two, and no background writer thread to lose the last lines on `exit()`.

---

## 4. The sequence

Six PRs against `dev`, each independently shippable. **Delete the print in the same hunk that adds its event** — except the ten marker strings below, which are a live contract until S3.

> **The marker allowlist** (`app/src-tauri/src/workspaces/phase.rs:34-48`, asserted by `bin/node/tests/invite_e2e.rs`). Do not delete or reword these prints before S3: `"joiner mode:"`, `"joining:"`, `"ADMITTED at height"`, `"admitted at epoch"`, `"resident: standing granted"`, `"synced root_hash="`, `"resident: pre-synced boundary"`, `"promoted:"`, `"FATAL"`, `"panicked at"`. Converting them to `tracing` is safe — both readers use `contains()` and the detail extractor falls back with `.unwrap_or(line)` — the message text survives in the rendered line.

---

### Phase 1 — The pipe, and its first real content

**Files:** `bin/noded/src/log.rs` *(new, ~55 lines)*, `bin/noded/src/{main.rs,lib.rs,stream.rs}`, `bin/node/src/{main.rs,boot/env.rs,validator/run/drain.rs}`, `bin/{simnode,coordinator,mcp,fs}/src/main.rs`, `app/src-tauri/src/daemon.rs`, `ops/dev.sh`, 8 × Cargo.toml.
**Size:** ~14 files, ~230 lines. **Risk: LOW.**

**a. One subscriber, one filter, reloadable.** Today there are two `EnvFilter`s per process (one of them ERROR-by-default). This replaces them with **one** — a net simplification — and makes it retunable on a live node.

```rust
// bin/noded/src/log.rs — the noded LIB, because bin/node already depends on it
// (noded::LogRing) and both bins serve noded::router(). The other four bins inline
// their six lines: bin/coordinator/Cargo.toml:18 says "no node-crate dependency",
// and a shared constructor is not worth inverting that.
use std::sync::OnceLock;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, reload, util::SubscriberInitExt as _};

/// `info` floor with RUST_LOG unset: the desktop spawns the node with NO environment
/// (daemon.rs::prepare_node_command_env sets only PATH), so anything that needs an env
/// var to be visible is, in practice, invisible.
/// If commonware's `info` proves chatty, pin it here — it is a string:
/// "info,commonware_p2p=warn". Don't guess: the dropped-lines marker (stream.rs, below)
/// tells you whether it actually evicts anything.
const DEFAULT_FILTER: &str = "info";

/// RUST_LOG *adds to* the default rather than replacing it. `EnvFilter::from_default_env`
/// REPLACES, and its no-directive default is ERROR — so a bare `RUST_LOG=one::target=debug`
/// silently drops every other event to ERROR. Turning logging UP must not turn the rest OFF.
/// `EnvFilter::new` already parses the comma list and skips bad directives with its own
/// message; there is nothing to hand-roll.
pub fn filter() -> EnvFilter {
    let env = std::env::var("RUST_LOG").unwrap_or_default();
    EnvFilter::new(if env.is_empty() { DEFAULT_FILTER.into() } else { format!("{DEFAULT_FILTER},{env}") })
}

static RELOAD: OnceLock<Box<dyn Fn(&str) -> Result<(), String> + Send + Sync>> = OnceLock::new();

/// retune a LIVE node (POST /v1/log-filter). Without this, every debug! in this plan is
/// unreachable without a restart — and a restart destroys the wedged state you were
/// trying to observe.
pub fn set_filter(directives: &str) -> Result<(), String> {
    (RELOAD.get().ok_or("no subscriber installed")?)(directives)
}

/// call ONCE from main. `ring: None` for bins with no stream.
pub fn init(ring: Option<crate::LogRing>) {
    let (layer, handle) = reload::Layer::new(filter());
    let _ = RELOAD.set(Box::new(move |d: &str| handle.reload(EnvFilter::new(d)).map_err(|e| e.to_string())));
    let _ = tracing_subscriber::registry()
        .with(layer) // ONE global filter, gating BOTH sinks
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(ring.map(|r| tracing_subscriber::fmt::layer().with_ansi(false).with_writer(r)))
        .try_init();

    // a panic in a spawned task kills THAT TASK ONLY: the node stays "up" and one plane
    // goes dark forever. the reachability plane, the voice hub and the overlay stack each
    // own a thread. chain, don't replace — the default hook keeps the backtrace on stderr.
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(target: "ducktape::node",
                        thread = std::thread::current().name().unwrap_or("?"), "PANIC: {info}");
        default(info);
    }));
}
```

```rust
// bin/noded/src/lib.rs — one route, beside /v1/shutdown (same router, same trust boundary;
// bin/node serves this router too, so ONE route covers both bins).
.route("/v1/log-filter", post(log_filter))

async fn log_filter(body: String) -> Response {
    match crate::log::set_filter(body.trim()) {
        Ok(()) => (StatusCode::OK, body).into_response(),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e),
    }
}
// curl -XPOST localhost:$PORT/v1/log-filter -d 'info,ducktape::join=debug'
```

The other four bins get six lines each — **`mcp` must use stderr** (stdout is the JSON-RPC wire; `fmt()` defaults to stdout), **`simnode` must use `.without_time()`** (deterministic output):

```rust
let env = std::env::var("RUST_LOG").unwrap_or_default();
let dirs = if env.is_empty() { "info".to_string() } else { format!("info,{env}") };
tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::new(dirs))
    .with_writer(std::io::stderr)
    .init();
```

**b. The boot identity line — one line that ends a whole incident class.**

```rust
// bin/node/src/boot/env.rs — the first line of every node's life.
// NO build.rs: a git sha baked by a build script goes STALE (cargo will not re-run it on
// a commit, and `.git` is a FILE in every worktree here, so the usual rerun-if-changed fix
// is fragile) — it would lie during exactly the stale-binary incident it exists to prevent.
// exe path + mtime is the mechanical equivalent of the `strings <bin> | grep <symbol>`
// workaround the team was already doing by hand, and it cannot go stale.
let exe = std::env::current_exe().unwrap_or_default();
let built = std::fs::metadata(&exe).and_then(|m| m.modified()).ok()
    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
tracing::info!(target: "ducktape::node",
    version = env!("CARGO_PKG_VERSION"),
    profile = if cfg!(debug_assertions) { "debug" } else { "release" },
    binary = %exe.display(), built_unix = built, pid = std::process::id(),
    listen = %cfg.listen, namespace = %cfg.namespace, storage = %cfg.storage.display(),
    "node boot");
```

**c. Cap the tape in the same PR that turns the volume up.** `daemon.log` is opened `create(true).append(true)` forever, by both spawners:

```rust
// app/src-tauri/src/daemon.rs, in spawn_node immediately before OpenOptions — 3 lines.
if fs::metadata(log_path).is_ok_and(|m| m.len() > 32 << 20) {
    let _ = fs::rename(log_path, log_path.with_extension("log.1"));
}
```
```sh
# ops/dev.sh, before the nohup at :124 — one generation, not logrotate.
[ -s "$dir/daemon.log" ] && [ "$(stat -c%s "$dir/daemon.log")" -gt 33554432 ] && mv "$dir/daemon.log" "$dir/daemon.log.1"
```

**d. Stop destroying evidence silently.** `LogRingInner` already tracks `floor_seq` (`stream.rs:242`), so ring eviction is **computable and unsurfaced**. In the `logs` topic's catch-up, when a client's resume cursor is below `floor_seq`, emit one synthetic line: `--- N lines dropped (ring full) ---`. ~5 lines. This is also how you decide empirically whether commonware needs pinning, instead of guessing.

**e. Drain the unclaimed module events — the highest leverage:effort ratio in the plan.**

`crates/apps/runs` has **42 live `note()` calls** — its own doc calls them *"an observability breadcrumb"* — including **`run {id} failed: {reason}`**, the single most important line in the agent path. They are emitted as `sdk::Event`s, collected into `BlockOutcome.events`, offered to workers, and then dropped on the floor. The comment at the drop site says it out loud, verbatim in **both** bins (`drain.rs:777`, `noded/main.rs:677`):

> *"an unclaimed event is normally plain observability; one that DECODES as a worker request means a saga is stuck Pending."*

— and then it discards it. **The only unclaimed event that ever produces output is the one that isn't observability at all.**

This is not optional cleverness: 9 of the 11 app crates compile to wasm guests and the WIT world exposes **no log import**. `emit_event` is their entire outbound diagnostic surface, forever. **This 20-line site *is* the logging for nine modules.** It also sidesteps replay amplification — the host drains once per live block, not once per replayed one.

```rust
// bin/node/src/validator/run/drain.rs:777 and bin/noded/src/main.rs:677 (verbatim twins).
// `take_events()` hands back everything accumulated since the last tick, and a drain can
// apply MANY blocks (catch-up, post-reboot suffix) — uncapped, one tick could evict the
// whole 4096-line ring at exactly the moment an operator is watching a join.
const NOTE_BUDGET: usize = 16;
let (mut noted, mut suppressed) = (0usize, 0usize);

for eff in node.take_events() {
    // ... existing worker offer loop, unchanged ...
    if !claimed {
        if saga::decode_worker_request(&eff.payload).is_ok() {
            // a stuck saga does not clear itself: this fires EVERY BLOCK, forever.
            // latch it — an error level in a permanent loop stops meaning anything.
            *stuck += 1;
            if *stuck == 1 || *stuck % 600 == 0 {
                tracing::error!(target: "ducktape::saga", height, source = %eff.source,
                                occurrences = *stuck,
                                "WorkerRequest with no worker — saga is stuck Pending");
            }
        } else if noted < NOTE_BUDGET {
            noted += 1;
            tracing::info!(target: "ducktape::modules", height, source = %eff.source,
                           note = %note(&eff.payload));
        } else {
            suppressed += 1;
        }
    }
}
if suppressed > 0 {
    tracing::info!(target: "ducktape::modules", suppressed, "module notes suppressed this drain");
}

/// a module payload is arbitrary bytes from a wasm guest, and runs::note() embeds
/// free-form provider/LLM text. cap it and strip control chars before it reaches a
/// terminal — and the webview, which the ring streams to.
fn note(payload: &[u8]) -> String {
    String::from_utf8_lossy(payload).chars().filter(|c| !c.is_control()).take(256).collect()
}
```

**Gate:**
```bash
cargo clippy -p node -p noded -p simnode -p coordinator --tests --no-deps
cargo check -p files --no-default-features                       # must stay green
env -u RUST_LOG cargo run -p node -- --config <cfg> 2>&1 | head -1   # boot line on stderr
curl -XPOST localhost:$PORT/v1/log-filter -d 'info,ducktape::modules=debug'   # 200, takes effect live
# fleet up → NODE rail → Status → Logs: the SAME boot line appears in the ring.
```
Plus a simnode scenario that suspends an agent and asserts the breadcrumb appears — the first thing that proves simnode's new subscriber works.

---

### Phase 2 — Fail-stop: make a dying node explicable

**Files:** `bin/node/src/{main.rs, validator/boot.rs, validator/run/drain.rs, sync/serve.rs, replica/park.rs, reachability_plane.rs}`, `bin/noded/src/main.rs`.
**Size:** ~8 files, ~40 sites, ~120 lines. **Risk: LOW** — `error!` already reaches `daemon.log` at today's filter, and both readers of the markers use `contains()`.

~35 `eprintln!("FATAL: …"); exit(1)` sites are the **one class of event an operator most needs** — *the node exited, here is why* — and they are invisible in the only surface most users have. `main.rs:135` wraps terminal errors in the `FATAL:` string **precisely so the app can string-match a death it cannot otherwise see.**

Sites: `validator/boot.rs` (~21), `drain.rs:76` + `:614`, `sync/serve.rs` (5), `replica/park.rs` (11), `noded/main.rs:486` + `:530`.

```rust
// bin/node/src/validator/run/drain.rs:76
tracing::error!(target: "ducktape::consensus",
                error = %e, height = node.finalized(), epoch, root_hash = %hex(&node.root_hash()),
                "FATAL: block-boundary fault — halting");  // "FATAL" preserved (marker allowlist)
// then FLUSH before exit(1): process::exit skips every Drop, including LogRingWriter's.
```

Same class, same PR:
- **Index apply failed** (`drain.rs:142`) — `error!`. Consensus stays healthy while the entire app UI silently stops updating.
- **Schema preflight mismatch** — `error!` with the per-module fingerprint **diff**. Today this surfaces as a bogus `snapshot truncated`.
- **Mesh not restored / `AlreadyCreated`** (`reachability_plane.rs:291`) — `error!(effect="AlreadyCreated", consequence="restart reconnect is dead for this boot")`. The tell exists today as an unlevelled println that reads like startup chatter — *which is exactly why it sat there being ignored.*

**Gate:** `cargo test -p node` (the e2e suites still find their markers). Kill a node mid-block; the reason lands in `daemon.log`. *Do not gate CI on the ring:* `exit(1)` gives the ring's watch-channel subscriber no chance to be scheduled, so the tab is best-effort here by design. `daemon.log` is the durable sink.

---

### Phase 3 — The block spine and the submit correlation

**Files:** `crates/kernel/node/src/lib.rs`, `crates/kernel/host/src/lib.rs`, `bin/node/src/validator/run/drain.rs`, `bin/noded/src/lib.rs`.
**Size:** ~4 files, ~60 lines. **Risk: LOW.** No wire change, no kernel command change.

**(a) Nothing in this repo ever says "height H produced root-hash X."** Fork triage, upgrade verification, and *"is my node keeping up"* all start here. Every field is already in scope.

```rust
// crates/kernel/node/src/lib.rs:1318, beside `let batch_hash = outcome.root_hash;`
// gate on `applied`: an idle chain heartbeats a nop block every second and would fill
// the ring with nothing.
if any_applied {
    tracing::info!(target: "ducktape::consensus", height, view, epoch,
                   root_hash = %hex(&batch_hash), members, applied, rejected,
                   "block committed");
} else {
    tracing::debug!(target: "ducktape::consensus", height, "idle block");
}
```

**(b) The submit-Ok ≠ committed trap, closed.** An op rejected in consensus (files CAS conflict, chat empty author, governance non-member) produces **no record anywhere.** The module's verbatim reason, the target, the origin and the height are all in hand at `drain.rs:254` — and thrown away. *The submitter's log says SUCCESS while the state machine says NO.*

```rust
// bin/node/src/validator/run/drain.rs — emit on EVERY drained Rejected frame,
// BEFORE the pending_submits lookup. Internal submits (oracle results, capability
// announces, upgrade readiness, code-ready signals) are fire-and-forget and never enter
// that map — which is exactly why a rejected announce wedges its latch forever
// (announce.rs:168), silently leaving the node out of every rendezvous pool, with the
// upgrade stuck at R<n, and nothing anywhere saying why.
if matches!(d.disposition, node::Disposition::Rejected) {
    tracing::warn!(target: "ducktape::submit",
        frame  = %hex_bytes(&d.id),
        height = d.height,
        module = d.op.as_ref().map_or("system", |o| o.target.as_str()),
        origin = d.op.as_ref().map_or("-", |o| o.origin.as_str()),
        reason = %d.reason.as_deref().unwrap_or("deterministic_no_op"),
        "op rejected in consensus");
}
```

**(c) The one HTTP funnel.** `error_response()` is where **every** rejection flows through — 403 origin-guard, 413 body cap, 409 conflict, 503 no-mesh, every module 400. Three lines light up the whole surface.

```rust
// bin/noded/src/lib.rs:319
// 4xx is DEBUG on purpose: gateway_http.rs:28 shares this funnel and its fallback proxies
// UNTRUSTED duck:// pages' fetches — an unconditional warn here is a log-ring DoS any page
// could drive. Turn it on live when you care:
//   curl -XPOST localhost:$PORT/v1/log-filter -d 'info,ducktape::http=debug'
// NEVER log the URI: /.duck/ws/{token} (gateway_http.rs:370) carries a capability token in
// the path, and the ring is streamed to the webview.
fn error_response(status: StatusCode, message: &str) -> Response {
    if status.is_server_error() {
        tracing::warn!(target: "ducktape::http", status = status.as_u16(), message, "request failed");
    } else {
        tracing::debug!(target: "ducktape::http", status = status.as_u16(), message, "request refused");
    }
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}
```

**(d) Non-deterministic module replay** (`host/src/lib.rs:1185`) — `error!` inside the `map_err` closure. The kernel's **only** in-band detector of module non-determinism, the most fork-relevant event that can occur, is currently wrapped into a `FatalError` mislabelled as an Abort-phase boundary fault and returned **in silence**.

**Gate:** `cargo clippy -p node -p host -p noded --tests --no-deps`; `cargo check -p files --no-default-features`; submit a knowingly-invalid op → the 400 body's reason and the node's `warn!` carry **the same `frame` id**.

---

### Phase 4 — The refusals: statesync, recovery, code

**Files:** `crates/kernel/{statesync,recovery}/src/lib.rs`, `bin/node/src/{validator/wiring.rs, sync/serve.rs, code_plane.rs, replica/park.rs, util.rs}`.
**Size:** ~7 files, ~150 lines added, ~60 deleted. **Risk: LOW.**

**The statesync serve loop silently DROPS every refused request** — five bare `continue`s at `wiring.rs:277/318/326/354/357`, including the **fail-closed committed-standing gate** (`:356`). *"Why is this joiner never syncing?"* is unanswerable **from the serving side**: standing-refused, proof-invalid and malformed look identical (silence) to both parties. `sync_monitor.record` fires only *after* all five drops, so metrics do not cover it either. Each gets a `warn!` with a `reason` token — and each gets its **resolution** twin (`catch-up complete, converged at height H`), because a log that only prints refusals cannot tell "still wedged" from "healed 40 s ago".

**The `RangePruned` wedge, logged by neither side.** `checkpoint_blocks` defaults to **32**, prune trails one checkpoint, BLOCK_TIME is 1 s → the retention window is **32–64 seconds**. A slow bridge or a laptop wake is outrun, and **no node anywhere records what the retention floor was or who got refused.**

```rust
// bin/node/src/sync/serve.rs:481 — the server side of the known wedge.
tracing::warn!(target: "ducktape::statesync", peer = %hex(&peer[..4]),
               requested_after, retained_from, gap_blocks = retained_from - requested_after,
               checkpoint_blocks, "frame range refused: pruned below the retention floor");
```
That is the line that answers *"was my follower too slow, or was the source pruning too aggressively"* — the exact question that ate the 07-14 live-join session. On the client side, the same wedge prints **the same impossible range on every certificate**, indistinguishable from healthy catch-up: log it with **`attempts`** and `permanent = true`. **The counter IS the diagnosis.**

**The module-code receive path is 100% dark** — `code_plane.rs:206/209/215/223/229`, five distinct refusal reasons all calling the same silent `refuse()`. A member stuck refusing every push never signals code-ready, the upgrade never arms at R=n, and nothing says why. `RESULT_CORRUPT` on hash mismatch (`:255`) — a peer sending bytes that do not hash to the committed digest, a **security-relevant** event — is detected and discarded with no local record.

**Delete in this PR, do not migrate:** `bin/node/src/util.rs:47 diag_log` and its **13 call sites**. It is a hand-rolled logger — env-var gate + println + append-to-file — off by default, so the promotion/catch-up/boundary diagnostics *that someone thought valuable enough to hand-build a logging system for* are invisible in every normal run. `RUST_LOG` + the reload route + app delivery subsume both its jobs and do the one thing it never could. **No `DUCKTAPE_DIAG_LOG` shim** (repo rule). *That someone built this by hand is the strongest possible evidence for where these events belong.*

**Gate:** `cargo clippy -p statesync -p recovery -p node --tests --no-deps`; a fleet join against a node with `checkpoint_blocks=8` — the refusal must name the floor.

---

### Phase 5 — The connection planes: join, voice, reachability

**Files:** `bin/node/src/{voice.rs, voice_plane.rs, reachability_plane.rs, first_contact_join.rs}`, `crates/system/reachability/src/orchestrator.rs`, `crates/system/overlay-net/src/userspace/device.rs`, `crates/system/data-plane/src/{plane.rs, real.rs}`.
**Size:** ~8 files, ~230 lines. **Risk: MEDIUM.** Last among the node PRs — it benefits most from the filter machinery having soaked.

**This is where "Voice connection failed." dies.** Three lifecycles, field-keyed:

```
ducktape::reachability  key: epoch + peer   plane_started → retarget → mesh_ready →
                                            tunnels_applied → ★peer_handshake_complete →
                                            peer_dark / epoch_failed
ducktape::join          key: nonce          candidates_planned → candidate_attempt(via) →
                                            [inviter, SAME nonce] intro_received(outcome) →
                                            first_contact_won / terminal
ducktape::voice         key: channel_id     hub.bind_waiting → hub.bound → session_opened →
                                            [1 Hz call.stats] → session_closed(reason)
```

**★ The single highest-value addition in this area.** `TunnelsApplied` proves only that the effect **accepted a config** — nothing today proves a WireGuard handshake ever **completed**. `WgDevice::time_since_last_handshake()` (`device.rs:452`) **already exists and is used only by tests.** Sample it on the existing 4 Hz timer and emit **only on transition**: tunnel-up/tunnel-down for free, at zero per-packet cost. It splits *"overlay never came up"* from *"overlay up, peer dark"* — the two failures that both surface as one string today. Log at the sample site; do not add an enum variant that only the logger consumes.

**How the three voice bugs become three distinct reads** (filter by `channel_id`):

| Bug | The log now says |
|---|---|
| overlay never came up | repeating `hub.bind_waiting{attempts=27, elapsed_s=81}` → `hub.join_refused{reason=overlay_down}`. **No `hub.bound`.** |
| overlay up, peer dark | `hub.bound` + `session_opened` + `tunnels_applied{peers=2}` but **no `peer_handshake_complete{peer=X}`**, then `peer_dark{peer=X}`. `call.stats{frames_sent=N, frames_received=0}`. |
| roster never arrived | everything green — **plus `call.no_recipients{elapsed_s=3, frames_discarded=150}`**. The failure is client-side and the log says so. |

That third one is **100% invisible node-side today**: `voice.rs` is 1867 lines with **zero prints of any kind**, and `if peers.is_empty() { continue; }` (`:687`) silently discards every captured frame.

**Cost discipline — this is the hottest area in the repo.** `call.stats` rides the **existing 1 Hz `ctl_tick`** (`voice.rs:804`); every field is already computed there and thrown away. **Nothing logs per frame or per datagram, at any level.** The retry loops (`device.rs:184`, `data-plane/host.rs:66`, `orchestrator.rs:602`) get attempt-1-then-every-Nth with an `attempts` counter — a naive `warn!` in any of them evicts the whole ring in seconds.

Two lines the code already wrote for you: `first_contact_join.rs:445` is literally `let _ = &label; // reserved for future per-attempt tracing`. And `orchestrator.rs:628` binds its failure as **`Err(_unreachable)`** — someone named the variable, knew exactly what was being discarded, and discarded it anyway. That is the one-word summary of this whole area.

**Gate:** `cargo clippy -p reachability -p overlay-net -p data-plane -p node --tests --no-deps`; a live two-node huddle over the fleet; kill the overlay mid-call and confirm `plane.halted` fires and `peer_dark` follows.

---

### Shell track — runs in parallel with Phases 2–5

**S1 — Reap the `Child`. (This is a bug fix, not a logging feature. Ship it first, on its own.)**
`spawn_verified` watches the node for ~1500 ms, returns the `Child`, and `workspace_select_blocking` **drops it.** Nothing ever calls `wait()` again. **The shell holds the only handle that can report the node's exit status and throws it away** — so a node that OOMs after an hour is reported as *"the node exited before it came up."* Keep the handle, watch it, log `node pid=1234 exited code=101 after 3612s`. This also kills the zombie that `lifecycle.rs:137-147` exists purely to work around, and it makes `fatal` a **process fact**, which is what unblocks S3.
*Files:* `app/src-tauri/src/daemon.rs`, `lifecycle.rs`. ~60 lines. **Risk: LOW.**

**S2 — The shell's own sink.**
`main.rs:154 .expect("start desktop node-control actor")` and `:287 .expect("error while building tauri application")` panic into a stderr **that does not exist**. The app vanishes with **zero bytes on disk.**
- `fmt::layer().with_writer(Mutex::new(File))` into `app.path().app_log_dir()` (the sibling of the `app_data_dir()` already used at `notify/mod.rs:57`) + the same 32 MB rename. macOS: `~/Library/Logs/com.ducktape.app/Ducktape.log` — the OS convention; you can tell a user *"open Console"*. No `tracing-appender`, no background writer thread to lose the tail.
- **⚠ Install the subscriber AFTER the CEF helper-process dispatch** (`main.rs:144-148`). CEF **re-execs this same binary** for its renderer/GPU/utility subprocesses — install before the dispatch and 4–6 helpers each open and append to the same file.
- A panic hook (same chain-don't-replace pattern as Phase 1).
- **`permissions.rs::audit()` → `info!`.** Its own doc comment claims *"the desktop launcher tees it into the workspace log."* **Nothing tees it.** The one record deliberately designed as a **security audit log** — which untrusted `.duck` publisher got your microphone — is destroyed at the instant it is written.
- **Plumb `RUST_LOG` into the spawned node** (`daemon.rs:692` sets only `PATH`): one `cmd.env(...)` from a workspace setting. (With the `/v1/log-filter` route from Phase 1, this is now the *cold* path — you can retune a live node without it.)
- **One-line fix:** `LogsTab` is gated on `nodeControlAvailable = workspace !== null && state.managed`, so a **remote-client user has no log surface at all** — even though their transport already carries the `logs` topic. *The user best placed to debug a remote connection is the one user forbidden from seeing the logs.*

*Files:* `app/src-tauri/src/{main.rs, daemon.rs, permissions.rs}`, `app/src/console/.../LogsTab.tsx`. ~90 lines. **Risk: LOW.**

**S3 — Delete the log scraper.**
The node **already serves an authoritative `join-state` RPC** that the shell prefers (`workspaces/mod.rs:1096`) for 4 of the 6 phases. With `fatal` now a process fact (S1), the string-scraping has nothing left to do. **Delete `phase.rs::MARKERS`** — and with it, the last reader of the ten marker prints, which now die in `bin/node` too.
*Files:* `app/src-tauri/src/workspaces/{phase.rs, mod.rs}`, `bin/node/src/*`. **Risk: MEDIUM** (touches the join UI — land only after S1 has soaked). **Gate:** `cargo test -p node` (the e2e suites still pass), plus a live fleet join.

---

## 5. The incident payoff

Every one of these is in the operator's own incident record.

| Past incident | Cost | The line that collapses it | Phase |
|---|---|---|---|
| **Stale uplifted binary faked 3 consecutive "regressions."** The documented workaround was `strings <bin> \| grep <a symbol you just added>`. Binaries were being **dated by which bug they exhibited.** | Hours, **repeatedly, ≥4 incidents** | `info!(target:"ducktape::node", version, binary, built_unix, pid, "node boot")` | **1** |
| **Agent run fails; run history says "Failed"; no reason anywhere.** | Ongoing | **Already emitted and dropped.** `run {id} failed: {reason}` is one of 42 breadcrumbs the host throws in the bin. | **1** |
| **State-schema flag day (#447).** Surfaced as a bogus `snapshot truncated`; the user-visible symptom (*"my duckdns input is blocked"*) was **a dead node nobody thought to check.** | A flag day + a re-investigation | `error!(target:"ducktape::recovery", mismatched_modules="saga: 3→4", …)` | **2** |
| **Mesh restore `AlreadyCreated` (#471).** The tell existed as an unlevelled println that *"read like startup chatter, so it sat there being ignored."* | Days | `error!(target:"ducktape::reachability", effect="AlreadyCreated", consequence="restart reconnect is dead for this boot")` | **2** |
| **submit-Ok ≠ committed.** *"An announcer that latches on submit-Ok wedges FOREVER — silently out of every rendezvous pool, **with a success log**."* | Recurring class | `warn!(target:"ducktape::submit", frame, height, module, reason, "op rejected in consensus")` — fired on **every** drained rejection, internal submits included | **3** |
| **macOS "missing blocks": follower wedged FOREVER on RangePruned (#493).** It printed **the same impossible range on every certificate** — indistinguishable from healthy catch-up. Filed in the memory as *"the remaining known boot noise."* | Days | `error!(target:"ducktape::statesync", after_view, retained_from, attempts=3000, permanent=true)` — **the `attempts` field IS the diagnosis** | **4** |
| **Live join 07-14: statesync never converged.** `checkpoint_blocks=32` pruned faster than a 180 ms bridge could sync. | A full test session | `warn!(target:"ducktape::statesync", checkpoint_blocks, retained_from, gap_blocks, "pruned below the retention floor")` | **4** |
| **"Voice connection failed." = 3 bugs behind 1 string (#473).** | **Days** | `hub.bind_waiting{attempts,elapsed_s}` / `peer_handshake_complete` absent / `call.no_recipients{frames_discarded}` — **three distinct reads** | **5** |
| **Joiner statesync dark right after "standing granted" (#487).** The invite-layer merge destroyed a working, concrete endpoint; the symptom appeared 2 minutes later **in a different subsystem.** | Days | `warn!(target:"ducktape::reachability", peer, endpoints_cleared=1, "invite layer merged away a concrete endpoint")` | **5** |
| **Auto-bind deadlock (#481).** Reported as *"cannot assign duckdns"* — a symptom **four layers from the cause.** | A debugging session | The node was dead; **S1** reports its exit code, and Phase 2 reports why it died. | **2 / S1** |
| **The desktop app dies on launch.** | Unsupportable | A panic hook and a file that exists. | **S2** |

**The shape repeats: one string for N different failures, where the code KNEW the specific reason at the point of failure and threw it away.** "Voice connection failed." (3 causes). "cannot assign duckdns" (a dead node). "snapshot truncated" (a schema flag day). "No local forge repositories."

So the organizing principle is not *"add logging to subsystem X."* It is:

> **At every point where an error is collapsed into a user-facing string, emit the uncollapsed reason with its fields first.**

---

## 6. Explicitly not doing

| Killed | Why |
|---|---|
| **OpenTelemetry / OTLP / any collector** | Single-operator networks, ≤10 nodes, no collector to run and nobody to run it. Cross-process correlation is solved by the `frame` / `nonce` fields — already minted, currently discarded. Adding a collector to correlate ids we do not yet log is backwards. |
| **A rolling file sink for the node** (`<storage>/logs/`) | `daemon.log` **already is** the node's file sink. This is a default-directive change plus a 3-line rename, not a subsystem. |
| **`tracing-appender`** (node or shell) | A `Mutex<File>` writer plus the same rename-if-big is fewer lines, one rotation mechanism instead of two, and no background writer to lose the last lines on `exit()` — which is precisely when they matter. |
| **`tauri-plugin-log`** | A `log`-crate façade ⇒ a second filter system + a `log`→`tracing` bridge, in a workspace standardising on tracing. Its value-add is a webview JS sink; `app/src` has **4 non-test `console.*` calls**. It is not even in `Cargo.lock`; `tracing-subscriber` is. |
| **A frontend `log_write` Tauri command + `app/src/domain/log.ts`** | A new command ⇒ `build.rs` registration ⇒ a `trusted.toml` ACL entry ⇒ a hand-written reentrancy guard against a ring→ws→webview→ring amplification loop **that only exists because you built the bridge** — to serve three call sites. Two of the three are *bug reports about discarded errors* (`auto-bind`'s `.catch(() => "failed")` flattens five distinct causes into one word the caller then throws away). Fix them by **not discarding the error.** Revisit at twenty sites. |
| **A `req: u64` threaded through `NodeCommand`, and the HTTP request span** | `FrameId` is already the correlation id, already minted, already returned to the HTTP caller. `req` correlates nothing `frame` does not, and it is the only thing that would have made Phase 3 a kernel-wire change. |
| **Any spans at all** | Four independent tokio runtimes; a span does not cross them, so a spans-first design silently orphans events. Every id we need is a field. |
| **A `build.rs` git sha** | Cargo will not re-run the script on a commit, so the sha bakes in and **goes stale** — lying during exactly the stale-binary incident it exists to prevent. `.git` is a *file* in every worktree here, so the usual `rerun-if-changed` fix is fragile too. `current_exe()` + mtime cannot go stale. |
| **A standalone Phase-8 "cull" of ~700 prints** | The end-of-plan 40-file sweep that never lands, colliding with every in-flight branch (CLAUDE.md bans this shape for `fmt` for exactly this reason), with a vanity acceptance metric. **Delete each print in the same hunk that adds its event.** Every PR stays roughly net-neutral. |
| **Converting CLI stdout** | `bin/node/cli.rs` (66 prints), `userkey_cli.rs` (19), `bin/demo`, `bin/fs`. A user running `ducktape-node join-state` **expects** that on stdout. **Program output is not logging. Never touch it.** ~85 prints out of scope by definition. |
| **JSON-structured logs** | The reader is a **human**, in a 4096-line ring, with `grep`. If a machine ever needs it, `.json()` on the fmt layer is one line. |
| **A `/v1/logs` HTTP route** | The ws `logs` topic exists and is already subscribed. (`/v1/log-filter` is a different thing and it is the one route that pays for itself: it is what makes `debug` reachable at all.) |
| **A guest-side wasm log import** | (a) The memoized-replay loop re-runs the guest up to 4096× per dispatch — every guest log line would fire once per round. (b) A log call is **code**, so adding a debug line changes the component's code hash and **requires a governance code-swap to deploy**, whereas host-side logging is turned up with one HTTP POST on one node, with zero consensus events. That asymmetry alone settles it. (c) The WIT world is explicitly *"determinism by omission."* (d) Modules already have two channels: the verbatim reject string (Phase 3) and `emit_event` (Phase 1). |
| **Metrics** | `/metrics` already exists. This is about **logs**. The two subsystems best served by metrics (nat-traversal's `CoordinatorMetrics`, data-plane's atomics) are precisely the **darkest**, because a counter is structurally incapable of saying *why* — the `Err` holding the answer is destructured away one line before the counter is incremented. *The fix is not more counters; it is to stop discarding the `Err` next to the counter that already exists.* **One exception, file separately:** `data-plane/real.rs:286`'s untracked-source drop has no counter at all — invisible even to `/metrics`. That is a metrics hole, not a logging one. |

**The one line to carry into all 40 future PRs — put it in CLAUDE.md:**

> **If it can fire more than once per block, it is not `info`.**