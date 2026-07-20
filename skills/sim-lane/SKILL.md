---
name: sim-lane
description: Use when testing transaction flows of the iced desktop app deterministically (submit → commit → re-render, module rejections in UI state), when adding tests under app/src-iced/src/shell/sim/, or when any Rust #[test] in this workspace needs a deterministic in-process Ducktape node (no child processes, no fleet). Also covers the chat wire shapes and Simulator traps those tests hit.
---

# Sim lane — deterministic transaction tests

Two halves, one embedded node:
- **The iced lane** (`app/src-iced/src/shell/sim/`) — UI-driven transaction
  round-trips: real `update()` loop + `iced_test::Simulator`, with an embedded
  `simnode` answering the app's real HTTP. `cargo test -p ducktape-iced
  shell::sim` — self-contained, ~0.3s, no external binaries, no env vars.
- **The embeddable node** (`simnode::boot`, `bin/simnode/src/lib.rs`) — any
  crate's `#[test]` can boot a deterministic node in-process.

Which lane a test belongs in: see the lane-doctrine table in `skills/qa`
(node/module semantics without UI → `bin/simnode/tests`; TS store → `app/src/test/sim`).

## Where things live

| Thing | Path |
|---|---|
| Harness (`SimShell`, task pump) | `app/src-iced/src/shell/sim/mod.rs` |
| In-process signing override | `app/src-iced/src/shell/sim/signing.rs` (seam: `run_verb_inner`, `backend/node_control.rs`) |
| Chat proof tests (the exemplars) | `app/src-iced/src/shell/sim/chat.rs` |
| Embeddable node lib + doc comment | `bin/simnode/src/lib.rs` |
| Cross-crate embedder example | `bin/simnode/tests/embed.rs` |

The lane is a `shell` child on purpose: `Shell`/`Message`/`update` are
module-private. New surface files go under `shell/sim/`, declared in `mod.rs`.

## SimShell quick reference

`SimShell::boot()` → embedded auto-mode sim + `NodeClient` + `Backend` +
signing override installed. Returns `Self`; failures panic. All `pub(super)`:

| Call | Does |
|---|---|
| `click(role, name)` | Simulator click by Sem role+name, then pumps |
| `inject(message)` | Feed a `Message` straight into `update()`, then pumps — for widgets with no Sem wrapper, composer text, timer ticks |
| `has(role, name)` / `sees_text(t)` | Widget-tree probes (see traps) |
| `shell()` | `&Shell` — assert render models directly (in-crate) |
| `node_query(target, query)` | Raw `/v1/query` against the embedded node — the chain-side assert |

The pump runs each `update()` Task on a private runtime and feeds
`Action::Output` back through `update()` until quiescent (15s deadline, 10s
per-action stall — both panic loudly). **No sleeps, no polling ever**: if a
test seems to need a wait, the flow is broken, not slow.

## Simulator traps (each cost a debug round once)

- **`typewrite` does not reach a `text_editor`** (blank post, Send disabled).
  Inject composer text as a Paste edit:
  `inject(Message::UserScreen(user_screens::Message::Chat(ChatMessageEvent::Composer{ thread: false, message: chat_composer::Message::Edit(text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new(body.into())))) })))`
  — `thread` is a `bool`. There is deliberately no `click_and_type`.
- **`sees_text` can NEVER match a message body**: bodies render as
  `rich_text`, which has no `operate`/text hook (only plain `text` feeds the
  finder). Assert bodies via the render model
  (`shell().user_screens.chat.data` — node-sourced only, no local-echo
  writer) and confirm the row materialized via a plain-text sibling (the
  author label).
- **Bare inputs have no Sem wrapper** (e.g. the channel-name field): drive
  them with `inject(...Changed(value))`, keep buttons as real clicks.
- **`ChatLoaded` auto-selects** the first non-archived channel; a rail click
  is only needed to exercise `SelectChannel → LoadChannel` itself.
- **`chat.error` gates the auto-refresh**: on submit Ok the reducer chains
  `LoadChat` (that re-query is what makes committed state render); on Err it
  sets `chat.error` and chains nothing. Assert `error.is_none()` FIRST when
  a rail entry is missing. Rejection strings look like
  `op rejected: Module(channel already exists: dup)`.

## Chat wire facts (safe to rely on, verified in-tree)

- Replies are externally tagged: `{"channels":[..]}`, `{"messages":[..]}`.
- `messages_latest` returns messages **ascending by `seq`**
  (`crates/apps/chat/src/lib.rs`), and the app maps the reply 1:1 with no
  reversal (`screen_service/chat.rs`) — order assertions are sound.
- The composer clears on Submit (`chat_composer::update`), so consecutive
  posts don't concatenate.
- Frames are signed in-process by a fixed-seed test key
  (`signing::author_pubkey_hex()`); the override answers only
  `user-key status` and `user-sign-frame` — any other verb errors loudly.
  It is process-global and authoritative; never expect a real
  `ducktape` subprocess inside `cargo test -p ducktape-iced`.

## Embedding the node in any crate's test

```toml
[dev-dependencies]
simnode = { path = "../../bin/simnode" }   # adjust depth
```
```rust
let storage = tempfile::tempdir()?;        // FRESH dir per test — determinism
let sim = simnode::boot(
    storage.path(),
    "127.0.0.1:0".parse()?,                // ephemeral; real port via sim.addr()
    simnode::SimOpts { auto: true, ..Default::default() },
)?;
// HTTP: full noded /v1 surface at sim.addr(). Control: sync handle —
// step()/set_auto()/peer_block()/state()/shutdown() (Drop also tears down).
```

`SimOpts` for embedders: keep `install_log: false` (true stacks a
process-global tracing subscriber + panic hook — binary only). `auto: true`
= submits commit inline; `false` = held mode, commits happen on `step()`
(races as scripts). Governance scenarios: `valset_keys` (raw 32-byte ed25519
pubkeys) + `invite_binding`; `node_key` fabricates `status.publicKey`;
`persona` picks daemon (`opHash` receipts) vs validator (height-only) shape.
After a host fatal the control surface fails closed (every call errs with
the reason); the *triggering* call may still return Ok — check the next one.

## Adding a new surface beyond chat

Map three things before writing the test (grep, don't guess):
1. Widget handles: `Sem(Role::…, "name")` wrappers in `screens/<surface>.rs`
   (`sem(`/`filled(`/`outline(` helpers) — unwrapped widgets need `inject`.
2. The message chain: widget → `ChatMessageEvent`-style reducer →
   `Command::…` → `screen_service/<surface>.rs` (the node wire + reply shape).
3. The refresh contract: which `ServiceEvent` writes the render model, and
   whether success auto-chains a reload (chat does; a surface that only
   refreshes via notifications-ws push needs an explicit `inject` tick —
   subscriptions never run in this lane).
