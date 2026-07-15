# Shared Sessions: consensus-ordered command lane — Design

Date: 2026-07-15
Status: approved (idea decided in discussion), phased delivery. This spec = the
full model; **PR 1 implements the command-lane seam (phase 1)**.

## The idea (from the design discussion)

Today's interactive terminal session (shipped in dev, `77e2d67cd6`) is
node-local, single-member, ephemeral: a pty in a podman container, output on a
node-local ws topic, raw-keystroke input over the same ws. This design turns it
into a **shared session** by splitting it across two planes *by the nature of
each half*:

- **Output = non-deterministic** (the LLM + the container do real I/O). Validators
  can never agree on it → it **broadcasts over the data-plane** (huddle-style:
  members across nodes join and receive the stream). Never through consensus.
- **Input = needs a single agreed ordering** (multiple members driving one
  session must agree on the command sequence) → **consensus totally-orders the
  commands**. The commands are deterministic text, so consensus *can* agree on
  them.

This mirrors ducktape's core principle (`consensus = isolation boundary`): the
non-deterministic execution stays OFF consensus, exactly like agent runs — but
the **ordered, attributed input** goes ON it. It is "agent-runs, made
interactive + multi-member + live-streamed."

What falls out for free:
- **Input arbitration** — consensus IS the arbiter; no keystroke race.
- **Per-turn authorship** — each command is origin-attributed (already the
  ducktape model everywhere).
- **The shared conversation object** — the ordered command log IS that object:
  durable, ordered, attributed. `--resume`'s JSONL is the off-chain "answers".
- **Cross-node** — output fans out over the data-plane; input is consensus, so
  already cross-node.
- **Audit/replay** — replay the ordered command log (the answers differ — the
  model is non-deterministic — but the *asks* are an exact, attributed log).

### The necessary consequence: grain = COMMAND, not keystroke

Per-keystroke consensus is absurd (block-time latency per character). So what is
ordered/shared is a **submitted command** (a prompt / line), not raw keys. The
shared session is a *consensus-ordered command queue to a live CLI, with
huddle-streamed output* — which is the right model for multi-member anyway (raw
keystroke sharing is chaos). Raw-keystroke `TermInput` stays for the SOLO case.

## Architecture: the two seams

```
                 ┌─ CommandSource (ORDERED, ATTRIBUTED) ──────────┐
  member(s) ───► │  phase 1: ws TermCommand   phase 2: consensus  │
                 └───────────────────────┬─────────────────────────┘
                                         │  serial consumer (one host node)
                                         ▼   feeds text + Enter, in seq order
                              pty ── podman container ── codex/claude
                                         │  output (non-deterministic)
                 ┌───────────────────────┴─────────────────────────┐
                 │  phase 1: node-local ws topic   phase 3: data-plane fan-out │
  member(s) ◄──── │  (term:<id> chunks + term-cmd:<id> command log)  │
                 └─────────────────────────────────────────────────┘
```

- **`CommandSource`** — the input seam. It hands the session an ordered,
  origin-attributed stream of commands. Phase 1's source is the ws
  (`TermCommand`); phase 2 swaps in consensus. The downstream is identical.
- **Serial consumer** — one task per session drains the lane in order, assigns a
  monotonic `seq` (the total order), records the command to the ordered log, and
  feeds `text` + `\r` (the command grain) to the pty. One node hosts the pty.
- **Output plane** — unchanged in phase 1 (node-local ws). Phase 3 fans it out
  over the data-plane (the run-output peer-forwarding pattern, already in dev).

## Phasing

- **PR 1 (this branch): the command-lane seam.** `TermCommand` op; a per-session
  ordered command lane (mpsc) with a serial consumer feeding the pty at command
  grain; a monotonic `seq`; an ordered, origin-attributed **command log**
  broadcast on a new `term-cmd:<session>` topic (the shared-conversation-object
  seed). Raw `TermInput` kept for solo. This is the linchpin — "input = ordered
  commands" — verifiable node-local, with `CommandSource` as the consensus swap
  point. NOT yet consensus, NOT yet data-plane fan-out.
- **PR 2: consensus command source.** A session-commands consensus module (like
  chat/agent-runs): `SubmitCommand` is an on-chain op, totally-ordered,
  origin-signed; the host node is the assignee (dispatch/oracle pattern) and
  drains the ordered commands into the same lane. Origin becomes the signed
  member. Failover = replay the on-chain log + `--resume` on a new host.
- **PR 3: data-plane output fan-out.** Forward `term:<id>` chunks (+ the command
  log) to peer nodes, mirroring the run-output forwarder, so members on other
  nodes join like a huddle. Cross-node.
- **Later: spend accounting** (multiple members burn one subscription → per-member
  caps on top of the broker's byte/request caps), host failover, session ACL.

## PR 1 detail

- **`ClientMsg::TermCommand { session, text, origin }`** (`stream.rs`) — `origin`
  is the caller-supplied attribution (the app passes a member label; empty =
  `"local"`). Phase 2 replaces it with the signed consensus origin. Distinct from
  `TermInput` (raw keystrokes, solo).
- **`TerminalSessions`**: each `Live` session gains an `mpsc::UnboundedSender<Command>`
  where `Command { origin, text }`. `enqueue_command(session, origin, text)` is
  the `CommandSource` entry point (ws calls it now; consensus later).
- **Serial consumer task** (one per session, spawned at create): receives
  commands FIFO, assigns a monotonic per-session `seq`, appends `(seq, origin,
  text)` to the command log + broadcasts a `ServerFrame` on `term-cmd:<session>`,
  then writes `text` + `\r` to the pty. Serial = the total order; one host feeds
  the pty.
- **Command log**: a bounded per-session ring of `(seq, origin, text)` on the
  StreamHub (a focused twin of `TermRing`), with catch-up on subscribe — the
  ordered, attributed conversation view. This is the shared object seed.
- Entitlement: a `TermCommand` is gated exactly like `TermInput` today (the
  connection must be subscribed to the session's topic — M6). Consensus (PR 2)
  makes the origin cryptographic.
- Logging: `tracing`, `target: "ducktape::term"`, per-command at `debug` with the
  `seq` (never the command text — it can carry secrets), refusals `warn`.

## Verification (PR 1)

- Rust: `cargo clippy -p noded --tests --no-deps` clean; `cargo test -p noded --lib`
  (ordering — seq monotonic under concurrent enqueues; command-log catch-up;
  serial-consumer feeds text+Enter).
- Live on real podman: a `TermCommand` submitted to a real `codex` session runs
  the prompt and the model reply streams back on `term:<id>` — the command grain
  drives an actual CLI end-to-end (as the shipped model-turn tests already do,
  now via the command lane).

## Review findings (fold in as sharing lands)

These surfaced re-reviewing the spec. #1/#2/#5 are real defects that become
reachable once sessions are actually SHARED (PR 2/3), so PR 1 flags them and the
implementation lands with the sharing:

1. **Raw `TermInput` bypasses the ordered lane.** A command-lane session must
   REJECT raw keystroke input — otherwise it hits the pty without a `seq`, log,
   or attribution and the total-order guarantee is false. Raw input and the
   command lane are **mutually exclusive per session** (raw = solo only).
2. **The command lane needs a bound / backpressure.** An unbounded lane lets any
   subscribed member flood commands, each burning model tokens — a spend +
   memory DoS in a shared session. Bound it (+ per-member rate) when sharing
   lands; the broker's coarse lifetime cap is only a backstop.
5. **PR 1 `origin` is caller-supplied = SPOOFABLE.** The command log's
   attribution is untrusted until PR 2 signs it via consensus. Nothing may trust
   PR 1 origins.

Framing (honesty, not defects):
- The command-log ring in PR 1 is an in-memory, bounded, VOLATILE broadcast ring
  — the durable shared-conversation object is the PR 2 on-chain log, not this.
- Consensus here is an **ordering + attribution + durability** service, NOT
  state replication: the execution and conversation state stay on the host node,
  non-deterministically. Validators agree on the *asks*, never the *answers*.
- **Command grain is clean for prompt-submit only.** A rich TUI (approval
  dialogs, modes, multi-line) does not cleanly decompose into "a command";
  `text + \r` submits a line. That is a known rough edge, not a general remote
  terminal.

## Non-goals (PR 1)

- Consensus (PR 2), data-plane fan-out (PR 3), host failover, spend accounting,
  session ACL. Raw-keystroke `TermInput` stays unchanged for the solo case.
- **PR 1 ships no user-visible shared session** — it is the command-lane SEAM
  (a new op + `term-cmd:` topic + ordered consumer). Without data-plane fan-out
  (PR 3) other-node members can't join; without consensus (PR 2) the order is
  node-local arrival order, not trustless. It is an architectural linchpin, and
  the app is not yet wired to use `TermCommand` — latent until PR 2/3.
