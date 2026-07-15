# Interactive Terminal Sessions — Design

Date: 2026-07-15
Status: approved (approach B), phased delivery

## Goal

Let a Ducktape node host **interactive terminal sessions** that a network member
attaches to from the app and drives a `claude` / `codex` CLI directly (its native
TUI), while:

- the CLI runs **subscription-authenticated** (not an API key), and
- the **credential never enters the container** the member can type into.

The member is treated as **adversarial**: a terminal is arbitrary code execution
by definition, and the CLIs' own Bash/exec tools mean "no shell" is a product
choice, not a security boundary. **The container is the boundary; nothing relies
on the CLI's permission mode.**

## Threat model (decided)

- **Who attaches:** any authorized network member, to a session on the operator's
  node, burning the operator's subscription.
- **Blocked from whom:** the member in the container. Credentials, the node's
  key/data dir, host loopback services, and unbounded model spend are all out of
  reach from inside a session.
- **ToS reality (operator's accepted risk, flagged twice, not re-litigated):**
  routing a Pro/Max subscription on behalf of other users is account-sharing and
  prohibited by Anthropic/OpenAI; enforcement is server-side. The broker's
  upstream credential is isolated to **one swappable knob** (subscription OAuth ↔
  Console API key) so the compliant path is a config change, not a rewrite. The
  broker relays the **real CLI's own requests byte-faithfully** — it does not
  spoof the harness.

## What already exists (reused, not rebuilt)

| Piece | Location | Reused for |
|---|---|---|
| CLI spawn seam (argv/env/backend/broker/teardown) | `crates/system/capability-host` `CliProvider::prepared_command` | interactive spawn shares argv + backend construction |
| codex broker (host holds token, child gets opaque bearer + loopback URL, empty `CODEX_HOME`) | `capability-host/src/broker.rs` | **codex is already credential-safe today** |
| Podman backend (mount ns, fresh HOME, cpu/mem caps) | `capability-host/src/sandbox.rs` | container boundary |
| Per-run ring + WS topic + catch-up + broadcast | `bin/noded/src/stream.rs` (`RunOutputRegistry`, `run-output:<id>`) | terminal scrollback + streaming, cloned as `term:<session>` |
| WS subscribe/catch-up/heartbeat machinery, JSON text frames | `bin/noded/src/stream.rs` (`ClientMsg`/`ServerFrame`) | terminal bytes ride as base64 in the existing frame types |
| Session-key / capability grant (who may act on the node) | `agent-session-keys` plane | **authorization for session creation** |
| Run output pane (per-run subscribe + render) | `app/src/console/views/agent/RunsTimeline.tsx` | pattern for the xterm view |

Genuinely new: (1) pty spawn, (2) an **input** direction (today's stream is
one-way), (3) an interactive session lifecycle distinct from `invoke()`'s
headless read-loop, (4) the claude broker (Phase 2), (5) xterm.js in the app.

## Architecture (approach B: node-local off-chain session)

A terminal session is an **ephemeral, node-local process** — not a consensus run.
It does not commit on-chain. (The `run-output:` topic already proves the house
pattern: runs commit, bytes stream off-chain.) Authorization rides the existing
grant; the session itself lives entirely in `noded`.

```
member (app)
  │  1. create session  ── authenticated RPC (grant check) ──▶ noded
  │  ◀── { sessionId, topic: "term:<id>" }
  │  2. ws subscribe "term:<id>"  (trusted-local surface)
  │  3. ws ClientMsg::TermInput{session,data(b64)}  / TermResize{session,cols,rows}
  │  ◀── ServerFrame::Event{ topic:"term:<id>", item: b64 bytes }  (+ catch-up ring)
  ▼
noded TermSessionManager
  ├─ authorize (existing grant), enforce per-node session cap
  ├─ capability_host::InteractiveSession::spawn(spec, broker)   ← NEW
  │     openpty ─ podman run -it … (broker argv) ─ child holds container tty
  │     host holds pty master
  ├─ pump: master → TermRing(ring) → broadcast(topic)      (reuse ring pattern)
  │        TermInput → master.write ; TermResize → TIOCSWINSZ(master)
  └─ teardown: close → SIGTERM→SIGKILL process group + podman cleanup (reuse)
```

### Component 1 — `capability_host::InteractiveSession` (Rust, `capability-host`)

New module beside `invoke()`; **shares** `broker_argv` + backend argv/env
construction, **replaces** the stdio+read-loop.

- **Podman backend only** for day 1. Direct is excluded on purpose: no mount ns,
  no fresh HOME → wrong for an adversarial member. (`InteractiveSession::spawn`
  returns an error for `Direct`/`Tart`.)
- pty via **`nix::pty::openpty`** (already in the tree — no new crate). The slave
  fd becomes `podman run -it`'s stdio; the host keeps the master. `podman -t`
  allocates the container-side tty and relays SIGWINCH, so **no host-side
  `TIOCSCTTY` is needed** — the interactive program is the CLI inside the
  container, not podman.
- master wrapped for async I/O (tokio `AsyncFd`); expose `read`, `write(bytes)`,
  `resize(cols,rows)` (`TIOCSWINSZ`), `kill()`.
- broker started exactly as `invoke()` does (`start_broker`); the child gets the
  opaque bearer + loopback base URL. For codex this is the **existing** path.
- **No idle-timeout kill.** A terminal is idle by nature; lifecycle is bounded by
  explicit close + a per-session wall-clock ceiling + the broker's spend cap, not
  by output silence.

Self-check: openpty + spawn `podman run -it <img> cat`, write bytes, read them
back; assert echo. (Gated so it skips when podman is absent, like existing
podman-dependent tests.)

### Component 2 — `TermSessionManager` (Rust, `bin/noded`)

- `create(caller_grant, agent: "codex", opts) -> {sessionId, topic}` — **grant
  check here** (reuse the session-key/capability check that dispatch already
  applies); refuse if the node is over its session cap (small constant, e.g. 4).
- Owns a `TermRing` per session (clone of `RunOutputRegistry`'s ring: bounded
  bytes, monotonic seq, `broadcast`, LRU across sessions). Catch-up replays the
  ring on (re)subscribe — same code path as run-output.
- Wires `ClientMsg::TermInput` → `session.write`, `ClientMsg::TermResize` →
  `session.resize`. Both carry `session` id; the manager rejects input to a
  session the connection did not create/attach.
- Teardown on last-detach-after-close or explicit close.

### Component 3 — WS protocol delta (`bin/noded/src/stream.rs`)

Additive to the existing enums (no wire break — new variants only):

- `ClientMsg::TermInput { session: String, data: String /* base64 */ }`
- `ClientMsg::TermResize { session: String, cols: u16, rows: u16 }`
- Output reuses `ServerFrame::Event { topic: "term:<session>", item }` where
  `item` is a base64 chunk. Modest throughput (human typing + TUI redraws); base64
  over the existing JSON text-frame path avoids a new binary channel. `ponytail:`
  base64-over-text; switch to ws binary frames only if a redraw-heavy TUI shows
  measurable overhead.

### Component 4 — app Terminal view (TS, `app/src`)

- Add `xterm.js` (+ `@xterm/addon-fit`) to `app/package.json` (bun; **no
  package-lock.json**).
- A Terminal surface: create session via the app's RPC client, subscribe to
  `term:<session>`, decode base64 → `term.write`, `term.onData` → `TermInput`,
  `fitAddon` + `term.onResize` → `TermResize`. Mirrors `RunOutputPane`'s
  per-topic subscribe.
- Where it lands in the module UI (rail entry vs. inside Agent view) is a UI
  call, resolved during the app slice; the transport contract above is fixed.

### Isolation posture (day-1, honest)

Day-1 inherits the **existing** podman posture (`--network=host`, no
`--cap-drop`/`--userns`/seccomp) — same as every codex run ships with today, so
**it regresses nothing**. Credential blocking (the stated hard requirement) is
already met for codex by the broker + empty `CODEX_HOME`. The lateral-reach
weakness of `--network=host` is real and is **Phase 3**, and it improves all runs,
not just terminals.

## Phasing

- **PR 1 (this branch): codex interactive terminal, podman backend.** Components
  1–4, codex only. Credential-safe today. Demoable end-to-end. Zero new legal
  exposure. Proves ~90% of the code.
- **PR 2: claude broker + claude terminal.** Extend `broker.rs` to serve
  `POST /v1/messages` (path-match, ignore query) with **SSE passthrough**, forward
  `anthropic-version`/`anthropic-beta` verbatim, relay upstream error bodies
  unmodified, tolerate `HEAD /`. Set `CLAUDE_CONFIG_DIR`→empty dir (the
  `CODEX_HOME` equivalent), `ANTHROPIC_BASE_URL`→broker, `ANTHROPIC_AUTH_TOKEN`→
  opaque bearer; env `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1`,
  `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, `DISABLE_AUTOUPDATER=1`, setting
  `skipWebFetchPreflight: true`. Upstream credential = the swappable knob. This is
  where the ToS decision bites, isolated to one module.
- **PR 3 (SHIPPED, safe part): opt-in private netns + host-gateway.** A private
  container netns replaces `--network=host`, so the container gets its OWN
  loopback and can no longer scan the host's for other runs' brokers / the node
  RPC — the primary lateral-reach weakness. The broker becomes reachable via
  `host.containers.internal` (a new `broker::Reachability::HostGateway`, shared
  with Tart), and this touches the **existing codex broker reachability** too.
  **Opt-in** via `DUCKTAPE_SANDBOX_PRIVATE_NET` (off by default → `--network=host`
  unchanged, nothing regresses) precisely because the podman networking specifics
  can't be validated without a podman host. **DEFERRED, needs a podman host:**
  whether the gateway forwards to host-loopback services (pasta `--map-gw`) or the
  broker must bind the gateway address; and a full OUTBOUND egress allowlist (block
  the internet, allow only broker + node RPC). Do **not** use the
  reference-devcontainer `NET_ADMIN` in-container firewall — an adversarial
  container must not hold `NET_ADMIN`; enforce egress from outside.
- **PR 4: full-shell mode — DROPPED** (user call: "full shell은 별로"). A member
  drives the CLI TUI, not a bare shell.

## Tart (macOS host) — added, UNVERIFIED

Both sandbox backends host a session now (`Direct` is still refused — no fence).
Tart reuses the headless VM lifecycle (`tart_setup` clone/boot/ip + `TartGuard`
teardown) and the same broker (`start_for_tart` binds the host-gateway the guest
reaches as `ducktape-host`); interactive differs only in the guest script
(`exec`s the TUI, no rsync-back) and the ssh flag (`-tt` forces a remote pty vs
headless `-T`). The session holds the `TartGuard`, so ending it stops/deletes the
VM. noded selects the backend via `DUCKTAPE_SANDBOX_BACKEND` (`podman` default,
`tart` on macOS) + `DUCKTAPE_SANDBOX_IMAGE`.

**Status: compiles + lints clean + argv-asserted (`-tt`, interactive guest
script), but NOT runtime-verified** — needs an Apple-Silicon Tart host (macmini-duke)
with a **codex-in-guest** VM image. Caveats to validate there: a Tart interactive
session holds a `tart_semaphore` permit (cap 2) for its whole lifetime, so it
starves headless Tart runs; and `TartGuard::Drop` stops the VM synchronously on
the thread that drops the last session `Arc`.

## Non-goals

- Consensus-committing sessions, session persistence across node restart,
  multi-member shared sessions.

## Verification (PR 1)

- Rust gates: `cargo clippy -p capability-host --tests --no-deps`,
  `cargo clippy -p ducktape-noded --tests --no-deps`, `cargo check`.
- `InteractiveSession` self-check (podman echo round-trip).
- Fleet QA: launch an instance, open the Terminal surface, drive a live `codex`
  TUI (type a prompt, see the TUI render), confirm from inside the session that
  `env | grep -i openai` and `cat ~/.codex/auth.json` find nothing.
