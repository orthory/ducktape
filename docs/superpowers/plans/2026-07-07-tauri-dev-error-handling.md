# Honest Error Handling for `tauri dev` — Implementation Plan

> **For agentic workers:** Implement task-by-task. Steps use checkbox (`- [ ]`) syntax. TDD, DRY, YAGNI, frequent commits. Spec: `docs/superpowers/specs/2026-07-07-tauri-dev-error-handling-design.md`.

**Goal:** Make the `tauri dev` loop honest — a node that fails to start or dies says why, fast, on screen, with a way to retry and a way to read the log; never a green success line, a blank screen, or an eternal spinner.

**Architecture:** Seven composable mechanisms (M1–M7) across three independently-shippable phases. P0 kills the false-success/blank-screen through-line; P1 removes the remaining hangs and adds a node-side fatal contract + transport classification; P2 is steady-state polish.

**Tech Stack:** Rust (Tauri v2 shell: `app/src-tauri`), Bash (`ops/dev.sh`), React 19 + TypeScript + Vite + Vitest (`app/src`), `cargo test`.

## Delivery status (2026-07-07)

**Shipped** (all with tests green — 21 Rust unit, ~308 frontend vitest, dev.sh honesty tests):
- **P0 (complete):** M1 `spawn_verified` + env-path guard · M2 `workspace_log_tail` + reason surfacing · M3a broadened `classify()` + process-death fatality · M4 honest `ops/dev.sh` restart + worktree isolation · M6-min `NodeFailed` body (reason + idempotent Retry + log) + boot-failure→gate · M7 `ErrorBoundary` + global handlers.
- **P1 (core):** registry corrupt-recovery + atomic write (T9) · `run_verb` 30s timeout (T10) · node-side `FATAL:` marker on run-path boot failure (M3b) · transport classification + bounded fetches + ws backoff (M5). *Both critical non-startup cases (corrupt-registry brick, run_verb hang) fixed.*
- **P2 (high-value UX):** mid-session reconnecting banner + heartbeat identity re-check.

**Deferred as polish** (non-load-bearing; the single `error` string + the surfaces above cover the critical/high cases): P1 QoL — solo/inactive `workspace_forget` short-circuit, batched teardown grace, time-bounded `public_ip`. P2 — full `ConnectionPhase` machine (kept the boolean + `connectionDown`), error *queue* vs single string, hoisted copyable presenter, in-app confirm modal, JoinProgress copy fixes, daemon.log rotation, retire the dead `daemon_spawn`. The honest headless GUI e2e (§7) is the recommended manual verification.

## Global Constraints

- Rust commands return `Result<T, String>` (the `Err` string is human-readable, shown in the UI). Keep that contract.
- The node is spawned **detached** (own process group / `DETACHED_PROCESS`); never use the tauri shell sidecar API. Preserve `detach()`.
- Binary resolution order stays: `DUCKTAPE_NODE_BIN` env → sibling `ducktape-node` → (legacy) noded. Add `usable()` to the env path; do not reorder.
- The `classify()` marker strings are a contract asserted by `bin/node/tests/invite_e2e.rs` — extend, do not remove or rename existing markers.
- Frontend: no new toast/design-system library — reuse the huddle-card vocabulary (`HuddleCard.tsx`/`HuddleWindow.tsx`). One shared error concept.
- Node source (`bin/node/`) is consensus-adjacent and moves fast — re-read it immediately before the P1 M3b task (it changed on `origin/dev` after this plan was written).
- Every task ends green: `cargo build -p ducktape-desktop` (or `-p node-bin`) and, for FE tasks, `cd app && bun run typecheck && bun run test` (vitest).

---

## Shared Interface Contracts

Defined once; every task uses these exact names/types.

**Rust — `app/src-tauri/src/daemon.rs`**
```rust
/// A verified-spawn failure: the node forked but did not survive the grace
/// window (insta-crash), with the reason and the tail of daemon.log.
pub struct SpawnFailure {
    pub reason: String,   // e.g. "node exited on start (exit status: 1)"
    pub log_tail: String, // last ~N lines of daemon.log (may be empty)
}

/// Spawn `cmd` detached, then verify the child did not die within a short
/// grace window. `ready_port` (when Some) is polled as a fast success signal
/// (member/founder http); a joiner passes None (it serves no http while
/// parked) and success = "still alive after grace". On insta-death, reads the
/// tail of `log_path` and returns Err. Retries spawn() a few times on
/// ETXTBSY/ENOEXEC (hot-reload binary-rewrite race).
pub fn spawn_verified(
    cmd: std::process::Command,
    log_path: &std::path::Path,
    ready_port: Option<u16>,
) -> Result<std::process::Child, SpawnFailure>;

/// Present-and-non-empty-and-executable AND trims empty env as unset.
fn usable(path: &std::path::Path) -> bool; // (existing; extend callers)
```

**Rust — `app/src-tauri/src/workspaces.rs`**
```rust
#[derive(serde::Serialize)]
pub struct LogTail { pub path: String, pub tail: String }

#[tauri::command]
pub fn workspace_log_tail(app: tauri::AppHandle, id: String) -> Result<LogTail, String>;
```

**TypeScript — `app/src/domain/workspace-client.ts`**
```ts
export interface LogTail { path: string; tail: string }
export const workspaceLogTail = (id: string): Promise<LogTail> =>
  invoke<LogTail>("workspace_log_tail", { id });
```

**TypeScript — `app/src/console/store/state.ts`** (P0-min; P2 promotes to a phase machine)
```ts
/** A managed node failed to start / connect. Drives the NodeFailed body.
 *  null when there is no boot failure. */
export interface BootError {
  workspaceId: string | null; // for idempotent Retry against the same workspace
  reason: string;             // the human message (folds in the daemon.log tail)
  logPath: string | null;     // for "Open daemon.log"
}
// add to ConsoleState: bootError: BootError | null   (initial: null)
```

**TypeScript — P2 connection phase (added in P2, referenced here so P0/P1 code names it consistently when it lands)**
```ts
export type ConnectionPhase = "connecting" | "live" | "reconnecting" | "down" | "degraded";
```

---

## PHASE 0 — Honest core

Kills false-success + blank-screen. Ships as one PR-worth of work; each task is independently testable.

### Task 1: `spawn_verified` helper (M1 core)

**Files:**
- Modify: `app/src-tauri/src/daemon.rs` (add `SpawnFailure`, `spawn_verified`; extend `usable`/env-path guard)
- Test: `app/src-tauri/src/daemon.rs` `#[cfg(test)]`

**Interfaces:** Produces `SpawnFailure`, `spawn_verified` (see contracts). Consumes existing `port_listening` (move a shared copy to daemon.rs or reuse workspaces’ — keep one; put a `pub(crate) fn port_listening(port: u16) -> bool` in daemon.rs and have workspaces re-export/use it).

- [ ] **Step 1 — failing test: insta-death is caught.** Test spawns `/bin/sh -c "exit 3"` (unix) as the cmd with a temp log file pre-seeded with `"boom\n"`, `ready_port=None`; assert `Err(SpawnFailure)` whose `reason` contains `"exit"` and `log_tail` contains `"boom"`.
- [ ] **Step 2 — run, expect FAIL** (`cargo test -p ducktape-desktop spawn_verified` → symbol missing).
- [ ] **Step 3 — implement `spawn_verified`:** keep the `Child`; loop up to ~1.5s at 50ms: `child.try_wait()` → `Ok(Some(status))` ⇒ read `read_tail(log_path, 8KB)`, `Err(SpawnFailure{reason: format!("node exited on start ({status})"), log_tail})`; else if `ready_port.map_or(false, port_listening)` ⇒ `Ok(child)`; else sleep. After grace with a live child ⇒ `Ok(child)`. Wrap the initial `cmd.spawn()` in a 3× retry on `io::ErrorKind` for ETXTBSY (raw os error 26) / ENOEXEC (8) with 100ms backoff; other spawn errors ⇒ `SpawnFailure{reason, log_tail:""}`.
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — test: live node returns Ok.** Spawn `/bin/sh -c "sleep 5"`, `ready_port=None` ⇒ `Ok`; kill the child in teardown.
- [ ] **Step 6 — test: `usable` rejects empty env + 0-byte.** `usable(Path::new(""))` false; a 0-byte temp file false; a 0755 non-empty file true.
- [ ] **Step 7 — extend env-path guard:** in `node_binary`/`resolve_node_bin`, `std::env::var("DUCKTAPE_NODE_BIN")` → trim; empty ⇒ treat as unset (fall through); non-empty ⇒ return only if `usable(&path)`, else `Err(format!("DUCKTAPE_NODE_BIN={} is empty or not executable — run `cargo build -p node-bin` or unset it", path))`.
- [ ] **Step 8 — run all daemon.rs tests, expect PASS. Commit** `fix(app): spawn_verified — catch a node that dies on start, guard the env-path binary`.

### Task 2: wire `spawn_verified` into the live spawn paths (M1 integration)

**Files:**
- Modify: `app/src-tauri/src/workspaces.rs` (`workspace_select` :839-866; the adopt gate :832; `active` commit :822-825)
- Modify: `app/src-tauri/src/daemon.rs` (`daemon_spawn` :48-69)

**Interfaces:** Consumes `spawn_verified`, `SpawnFailure`, `workspace_log_tail`’s `read_tail`.

- [ ] **Step 1 — `workspace_select`:** build `cmd` as today, then `let child = spawn_verified(cmd, &log_path, Some(ws.ports.http)).map_err(|f| format!("the node for \"{}\" exited on start: {}\n{}", ws.name, f.reason, f.log_tail))?;`. Keep the pidfile write. **Move the `reg.active` commit (:822-825) to AFTER a successful spawn/adopt** so a failed select never poisons `active`.
- [ ] **Step 2 — adopt gate (:832):** keep probing `ws.ports.listen` for idempotent re-select, but before returning `Ok`, for the **member** path also confirm the http surface (`port_listening(ws.ports.http)`) — if listen is up but http is dead after a short grace, fall through to the failure message rather than returning a dead `http_url`. (Joiner/parked has no http; gate this on `ws` membership — reuse the existing member/joiner predicate in the file.)
- [ ] **Step 3 — `daemon_spawn`:** replace the bare `cmd.spawn()` with `spawn_verified(cmd, &log_path, /* solo http */ parse_port(&listen))` and map the `SpawnFailure` into the returned `Err` string.
- [ ] **Step 4 — build + existing tests:** `cargo build -p ducktape-desktop && cargo test -p ducktape-desktop` → PASS.
- [ ] **Step 5 — Commit** `fix(app): verify the node survived spawn before reporting success (select + daemon_spawn)`.

### Task 3: `workspace_log_tail` command + registration (M2 Rust)

**Files:**
- Modify: `app/src-tauri/src/workspaces.rs` (add `LogTail`, `workspace_log_tail`)
- Modify: `app/src-tauri/src/main.rs` (register in `generate_handler!`)

- [ ] **Step 1 — test:** `workspace_log_tail`-shaped unit over `read_tail`: write a temp `daemon.log` with 200 lines, assert the returned tail is ≤64KB and ends with the last line.
- [ ] **Step 2 — implement:** `workspace_log_tail` loads the registry, finds `id`, builds `<ws dir>/daemon.log`, returns `LogTail{ path: log_path.display().to_string(), tail: read_tail(&log_path, 64*1024)? }`.
- [ ] **Step 3 — register** `workspaces::workspace_log_tail` in main.rs.
- [ ] **Step 4 — build + test, PASS. Commit** `feat(app): workspace_log_tail command — daemon.log path + tail for the UI`.

### Task 4: broaden `classify()` + process-death fatality (M3a)

**Files:**
- Modify: `app/src-tauri/src/workspaces.rs` (`classify` :888-923; `workspace_phase` :872-879; add `pid_alive`)
- Test: `workspaces.rs` `#[cfg(test)]` (there are existing `classify_*` tests)

- [ ] **Step 1 — test: a raw panic classifies fatal.** `classify("thread 'main' panicked at src/x.rs:1:1:\nboom")` ⇒ `phase == "fatal"`, detail contains `panicked`.
- [ ] **Step 2 — test: a content line merely *containing* a marker word does NOT flip phase** (anchor markers to the node’s `] ` log prefix where the existing markers already sit — match on the message part, not arbitrary substring). Keep existing `classify_ranks_latest_phase` / `classify_parked_holds_until_admitted` green.
- [ ] **Step 3 — implement:** add markers `("fatal", "panicked at")` and a start-of-message `error:` check to `MARKERS`; keep last-match semantics.
- [ ] **Step 4 — test: process-death fatality.** New `pid_alive(dir)` reads `node.pid`, returns whether that pid is live (`kill -0` via `libc`/`/proc`). In `workspace_phase`, if `classify` returns `"starting"` **and** the pidfile pid is dead **and** neither port is held, return `PhaseReport{ phase:"fatal", detail: Some(last non-empty log line or "the node exited before it came up") }`.
- [ ] **Step 5 — run, PASS. Commit** `fix(app): classify a dead-on-boot node as fatal (panic marker + process-death), not eternal "starting"`.

### Task 5: dev.sh honest restart + isolation (M4 core)

**Files:**
- Modify: `ops/dev.sh` (`restart_node`, `spawn_node`, startup sweep, watch predicate, preflight, env exports)
- Test: `ops/dev.test.sh` (new; sourced-function tests) + `shellcheck`

- [ ] **Step 1 — refactor for testability:** guard the run-body with `[ "${DEV_SH_LIB:-}" = 1 ] && return 0` after the function defs so the file can be `source`d in a test without launching tauri.
- [ ] **Step 2 — failing test:** `ops/dev.test.sh` sources with `DEV_SH_LIB=1`, stubs a fake node that exits immediately, calls `restart_node`, asserts the output contains `✗` and NOT `✓ node back`.
- [ ] **Step 3 — implement honest restart:** after the TERM wait loop, `if kill -0 "$pid"; then kill -9 "$pid"; wait; fi`; poll until the listen port is free; `spawn_node`; capture the child pid; sleep ~300ms; `if ! kill -0 "$child" || ! port_probe; then log "✗ rebuilt node exited — last log lines:"; tail -n 20 "$logpath"; return; fi`; only then `log "✓ node back on :$port"`. Add a `port_probe()` (bash `/dev/tcp` or `nc`).
- [ ] **Step 4 — isolation:** scope the startup sweep to `$NODE_BIN` (absolute) matched literally (`pgrep -f -- "$NODE_BIN"` with the path fixed, or filter `ps` with `grep -F`); preflight-probe :1430 before the sweep and abort naming the pid if taken (kill nothing); stage/point `DUCKTAPE_NODE_BIN` at a copy outside `target/` (mirror the fleet approach) so `build.rs` can’t zero it; respawn appends to the same `daemon.log` the app reads (or echo the path).
- [ ] **Step 5 — hash-gate the restart:** only `restart_node` when the built artifact’s mtime/hash advanced (avoid gratuitous bounces on test/doc edits).
- [ ] **Step 6 — `shellcheck ops/dev.sh` clean; `ops/dev.test.sh` PASS. Commit** `fix(dev): honest node restart + worktree isolation in ops/dev.sh`.

### Task 6: `bootError` state + NodeFailed surface + Retry/Open-log (M6-min)

**Files:**
- Modify: `app/src/console/store/state.ts` (add `BootError`, `bootError`), `reducer.ts` (patch passthrough already generic — confirm), `actions.ts` (`connectActive` catch; `retryConnect`; boot-resolution catch), `DucktapeConsole.tsx` (route), `workspace-client.ts` (`workspaceLogTail`)
- Create: `app/src/console/views/onboarding/NodeFailed.tsx`
- Test: `app/src/console/views/onboarding/NodeFailed.test.tsx`, extend `actions`/store tests

**Interfaces:** Consumes `workspaceLogTail`, `BootError`. Produces `retryConnect(workspaceId)` action (idempotent — re-runs `connectActive` against the existing workspace, never `create`).

- [ ] **Step 1 — test (vitest):** `NodeFailed` renders `reason`, a Retry button (calls `onRetry`), an "Open daemon.log" button (calls `onOpenLog`), and shows a collapsible tail. Assert copy is present and the buttons fire.
- [ ] **Step 2 — implement `NodeFailed.tsx`** (reuse huddle-card visual vocabulary; wrapping/selectable/copyable reason).
- [ ] **Step 3 — state + route:** add `bootError` to state/init; in `DucktapeConsole` routing, `bootError` (and no live node) ⇒ render `<NodeFailed .../>` ahead of the empty-shell fallback.
- [ ] **Step 4 — set it:** in `connectActive`’s catch, when `managed`, call `workspaceLogTail(id)` (best-effort), set `bootError={workspaceId:id, reason:String(err), logPath:tail.path}`; on boot-list/active failure force `needsOnboarding:true` (land on the gate, not an empty shell); on a **joiner** hard failure set `onboardingPhase={phase:'fatal',detail:String(err)}` so JoinProgress shows it.
- [ ] **Step 5 — Retry:** `retryConnect(id)` clears `bootError`/`error`, re-runs `connectActive(ws)` for the existing workspace; wire Retry+Open-log (Open-log = call a small reveal, or expand the inline tail) in `NodeFailed`. Guard `createWorkspace` retries against duplicating a same-name failed workspace.
- [ ] **Step 6 — `bun run typecheck && bun run test` PASS. Commit** `feat(app): a "Node failed to start" surface with the real reason, Retry, and the log`.

### Task 7: React ErrorBoundary + global handlers (M7-min)

**Files:**
- Create: `app/src/console/layout/ErrorBoundary.tsx`
- Modify: `app/src/main.tsx` (wrap `DucktapeConsole`), `app/src/console/layout/WindowFrame.tsx` (host the boundary inside the frame to keep the titlebar)
- Test: `app/src/console/layout/ErrorBoundary.test.tsx`

- [ ] **Step 1 — test:** a child that throws renders the fallback ("Something crashed" + a Reload button + copyable message); `getDerivedStateFromError` path.
- [ ] **Step 2 — implement `ErrorBoundary`** (class component, `getDerivedStateFromError` + `componentDidCatch`); fallback keeps the window chrome, shows message+stack, Reload calls `location.reload()`.
- [ ] **Step 3 — mount** inside `WindowFrame` around the body branches; add `window.addEventListener('error'|'unhandledrejection')` in `main.tsx` funneling into a module-level surface (a simple `bootError`-style overlay or the boundary’s state via a store dispatch).
- [ ] **Step 4 — typecheck + test PASS. Commit** `feat(app): top-level ErrorBoundary + global error handlers — no more blank white window`.

### Task 8: P0 verification (build/test/e2e)

- [ ] **Step 1** — `cargo build -p ducktape-desktop -p node-bin && cargo test -p ducktape-desktop`.
- [ ] **Step 2** — `cd app && bun run typecheck && bun run test`.
- [ ] **Step 3** — `shellcheck ops/dev.sh`.
- [ ] **Step 4 — honest e2e (headless, via the `tauri-debug`/`qa` skill):** (a) occupy the workspace http port → select → assert `address already in use` surfaces in ~2s with Retry+Open-log, not a spinner; (b) truncate `DUCKTAPE_NODE_BIN` → assert the actionable rebuild message; (c) inject a panic in a scratch node config / feed a bad `node.toml` → assert JoinProgress/NodeFailed shows the reason, not eternal "starting"; (d) `ops/dev.sh` restart with the port occupied → assert `✗` + tail, never `✓ node back`.
- [ ] **Step 5 — Commit** any test/fixup; tag P0 complete.

---

## PHASE 1 — Correctness & no-more-hangs

### Task 9: registry atomic-write + flock + corrupt recovery
- `save_registry`: write to `registry.json.tmp`, `fsync`, atomic `rename`. `load_registry`: on parse error, rename the bad file to `registry.json.bak`, start empty, set a one-time notice surfaced to the UI. Add a cross-process advisory `flock` held across load→mutate→save. TDD: mid-write kill leaves the real file intact; corrupt file ⇒ `.bak` + empty; two writers under flock don’t lose entries. Commit.

### Task 10: `run_verb` wall-clock timeout + forget/teardown hardening
- `run_verb`: spawn + `wait_timeout` (add the `wait-timeout` crate or a manual thread+recv), kill on expiry; timeout ⇒ a clean `Err`/Unconfirmed (force-overridable). Solo/inactive `workspace_forget` short-circuits Safe from registry-recorded membership without a live probe. Batch teardown: TERM the whole pid set, one shared grace, KILL survivors. Time-bound `public_ip` on a deadline (skip for local). TDD (a socket that accepts-but-never-replies returns within the deadline). Commit.

### Task 11: node-side FATAL marker + non-zero exit (M3b)
- **Re-read current `bin/node/src/main.rs` + `bin/node/src/config.rs` first** (they changed on dev). On every terminal boot failure (bind EADDRINUSE on http + mesh, config parse, storage/wire incompat, boot panic) emit a stable `FATAL <cause>` line on stderr and exit non-zero. Keep it minimal (a log line + exit code, not a status stream). Update the `classify()` marker contract test (`bin/node/tests/invite_e2e.rs`). TDD. Commit.

### Task 12: M5 transport classification + bounded fetches
- `transport.ts`: `status()` and the shared parse path reject with a typed `{kind: 'refused'|'httpError'|'badBody'|'timeout'|'csp', status?, detail?}`; wrap every fetch in an `AbortController` deadline; validate the `NodeStatus` shape and compare `version` against an app wire-compat range (hard-refuse managed, warn remote); ws reconnect gains exp-backoff+cap+jitter, a liveness/staleness timer, try/catch’d `JSON.parse`, and an identity re-check on reconnect. `waitUntilUp` retries only on `refused`/`timeout`. `node-bootstrap.ts`: `normalizeNodeUrl` via `URL()` → origin, reject bad schemes. TDD (mock 500/404/empty/refused/abort). Commit.

### Task 13: P1 verification — build/test/typecheck + the M5/registry/verb e2e cases from the spec §7. Commit.

---

## PHASE 2 — Steady-state polish

### Task 14: connection phase machine (M6-full)
- Replace `connected: boolean` with `connection: ConnectionPhase` (`connecting|live|reconnecting|down|degraded`); keep a derived `connected` for existing consumers or migrate them. Heartbeat drives the transitions; `AbortController` timeout + in-flight guard on the beat. Null-node boot-retry loop auto-adopts a late node. Down→up edge re-verifies `status().publicKey` before `refresh()`. TDD. Commit.

### Task 15: persistent banners + staleness scrim + saved-remote fallback
- A persistent labeled reconnecting/degraded banner (Restart node + Open daemon.log) split from transient dismissible op-errors; a staleness scrim over the shell body while down (submits paused); saved-remote failure falls back to local with a "forget remote" action; probe a remote before persisting it. TDD. Commit.

### Task 16: error queue + hoisted presenter + confirm modal + misc polish
- Error *queue* with timestamps instead of one clobbered string; one hoisted copyable presenter in `WindowFrame` over every body; an in-app confirm modal (replace `window.confirm`, which is awkward over VNC); `normalizeError` helper (never `[object Object]`); DEV-only degraded-module badge; JoinProgress copy fixes (step count, fatal bar, `starting` vs `parked`); daemon.log rotation/size-cap; atomic sidecar staging (`install -m 0755`); retire the dead `daemon_spawn`/`ensure_solo_config`. TDD where a unit exists; typecheck + vitest otherwise. Commit.

### Task 17: P2 verification + full local gate. Commit.

---

## Finalization

- [ ] Full local gate (`make test` or `cargo test --workspace` + `cd app && bun run typecheck && bun run test`).
- [ ] Push `feat/tauri-dev-error-handling`; open a PR **based on `dev`** summarizing the honest-error-handling work (P0–P2), linking the spec + this plan.

## Self-review notes

- Spec coverage: every layer of the §2 matrix maps to a task — dev.sh→T5; build/sidecar→T5(step4)+T16; daemon.rs→T1/T2; workspaces.rs→T2/T3/T4/T9/T10; node→T11; fe-transport→T12; fe-boot→T6/T14; fe-ux→T6/T7/T15/T16.
- The two `critical` non-startup cases (corrupt-registry brick, `run_verb` hang) are T9/T10 (P1).
- Interface names are shared via the Contracts section: `SpawnFailure`/`spawn_verified` (T1→T2), `LogTail`/`workspaceLogTail` (T3→T6), `BootError`/`retryConnect` (T6→T7/T15), `ConnectionPhase` (named here, lands T14).
