# Remote Agent Sessions Phase 2 — Directed Session Control + Peer Input Lane

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A guest node directs its host peer to spawn a Podman sandbox running the guest's chosen provider with the guest's (or a granted) credential, streams the pty over the existing peer mesh, and forwards the guest's keystrokes/resizes to the host's pty — creator-only. The credential resolves to the owner's co-hosted gateway (Phase 1), so the host's broker holds no secret. This delivers the DATA + CONTROL planes of `ducktape agent pty --node <host> --cred <name>`; the CLI attach client that drives them is a separate builder (see the final Interfaces section).

**Architecture:** One transport — the existing `Service::TermSession` overlay stream plane (`bin/node/src/term_plane.rs`), which already binds the service and fans a session's OUTPUT out to peers. Phase 2 grows that ONE bound plane with two new intents on the same accept loop: a `CONTROL` intent (guest→host create/close request→reply, mirroring `gateway_plane.rs`'s request/response exactly) and an `INPUT` intent (guest→host forwarded keystrokes/resizes, creator-gated host-side). A guest-side client half drains a `SessionJob` channel (mirroring `GatewayJob`/`GatewayLane`) fed by the daemon's existing `/v1/term/*` HTTP routes and ws `TermInput`/`TermResize` handlers. Credential wiring rides Phase 1's `AirlockConfig::self_host(&ResolvedCredential)` threaded onto `RunContext` into the interactive spawn's broker.

**Tech Stack:** Rust; tokio; serde_json (length-prefixed JSON frames, reusing `term_plane`'s existing `write_frame`/`read_frame`); libc (already a workspace dep — `tcgetattr`/`tcsetattr` for the excluded CLI's raw mode); existing crates only — no new dependencies.

**Spec:** `docs/superpowers/specs/2026-07-23-remote-agent-sessions-design.md` (data plane, control plane, CLI surface).
**Consumes from Phase 1:** `docs/superpowers/plans/2026-07-23-remote-agent-sessions-phase1.md` Task 5 — `capability_host::ResolvedCredential { name, kind, authority, via, seal_pk }`, `AirlockConfig::self_host(resolved: &ResolvedCredential) -> AirlockConfig` (pub), `AirlockTrust::PinnedSealPk`; and Task 1 — `gateway::{CredentialRecord, CredentialKind, GatewayQuery::Credential, GatewayReply::Credential, credential_use_allowed}`.

## Global Constraints

- Work in a worktree at `<primary>/.worktree/remote-agent-sessions-phase2`, branch `feat/remote-agent-sessions-phase2` off `origin/dev`; deliver as PR(s) against `dev`. Create it with the superpowers:using-git-worktrees skill before Task 1. **Phase 2 builds on Phase 1's merged tree** — do not start until Phase 1 Tasks 1–5 are on `dev` (the `gateway::Credential*` types and `capability_host::{ResolvedCredential, AirlockConfig::self_host}` must exist).
- Lint gate per touched crate: `cargo clippy -p <crate> --tests --no-deps`. Format only code you touched; never `cargo fmt --all`.
- `tracing` only in node/daemon code — never `println!`/`eprintln!`. Never log pty bytes, keystroke `data`, credential names, seal keys, or route handles; every `reason`/`event` is a snake_case token. Terminal output is per-frame → `trace!`; a create/close is a per-session lifecycle fact → at most one `info!`; a refused create or a rejected non-creator input is a `warn!` with a nameable `reason`.
- No versioned names anywhere: no `v2` in types, intents, routes, or fields (repo mandate). The existing `term_flow()` derives `b"ducktape:term-session:v1"` — the new intents ride that SAME flow, no new flow domain.
- Tests synchronize on events (channel recv, stream frame arrival, pty output on the ring's `watch`) — never sleep/spin (house rule). The e2e drives a scripted child (`cat`) and waits on the echoed chunk landing on the guest ring, not on a timer.
- State-machine + explicit-control-flow house rules apply: the control handler is ONE `match` on the decoded request variant, one arm per variant (no `_` wildcard), each a single delegation; admission checks are named predicates (`is_owner_or_granted`, `provider_matches_kind`), decided before any spawn effect.
- **`Service::TermSession` is bound exactly ONCE** (`term_plane::spawn`). Do NOT add a second plane on the same service/port — the new intents are handled inside the existing accept loop. Intent constants must not collide: `CHUNK_INTENT=1`, `COMMAND_INTENT=2` are taken; Phase 2 uses `3` and `4`.
- Commits end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: Wire — control + input intents and frame types (data plane)

**Files:**
- Modify: `bin/node/src/term_plane.rs` (intents `:43-44`, `accept_loop` `:155`, `write_frame`/`read_frame` `:331`/`:348` are reused verbatim)
- Create: `bin/noded/src/term_remote.rs` (the `SessionJob` channel type + the remote-session binding map — daemon side, data-plane-free)
- Modify: `bin/noded/src/lib.rs` (re-export `SessionJob`, `SessionLane`), `bin/noded/src/handle.rs` (`NodeHandle` gains the lane + remote map, mirroring `with_gateway` `:226`)
- Test: `term_plane.rs` unit tests (frame round-trip) + `term_remote.rs` unit tests (binding map)

**Interfaces:**
- Produces (later tasks + the excluded CLI depend on these EXACT names):
  - In `term_plane.rs`:
    ```rust
    /// guest→host directed create/close, one request → one reply (mirrors gateway PROXY_INTENT).
    const CONTROL_INTENT: u8 = 3;
    /// guest→host forwarded keystrokes/resizes, one persistent stream, creator-gated host-side.
    const INPUT_INTENT: u8 = 4;
    ```
  - The three wire structs (length-prefixed JSON via the EXISTING `write_frame`/`read_frame`; snake_case; the request carries NO creator field — the host derives the creator from the mesh-authenticated peer node):
    ```rust
    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(tag = "op", rename_all = "snake_case")]
    pub enum SessionControlRequest {
        Create { provider: String, cred: String, cpu: Option<u64>, mem_gb: Option<u64> },
        Close { session: String },
    }

    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(tag = "result", rename_all = "snake_case")]
    pub enum SessionControlReply {
        Created { session: String, topic: String },
        Closed,
        Refused { reason: String, detail: String },
    }

    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub enum SessionInputEvent {
        Input { session: String, data_b64: String },
        Resize { session: String, cols: u16, rows: u16 },
    }
    ```
  - In `bin/noded/src/term_remote.rs` (no data-plane deps — plain data + a oneshot reply the client half resolves):
    ```rust
    /// a unit of remote-session work the guest node hands its overlay client half.
    pub enum SessionJob {
        Create {
            host: [u8; 32],
            provider: String,
            cred: String,
            cpu: Option<u64>,
            mem_gb: Option<u64>,
            reply: tokio::sync::oneshot::Sender<Result<CreatedSession, String>>,
        },
        Close { host: [u8; 32], session: String },
        Input { host: [u8; 32], event: SessionInputWire },
    }

    /// the daemon-side twin of term_plane's SessionInputEvent (kept here so noded
    /// carries no data-plane dep; term_plane maps one to the other 1:1).
    pub enum SessionInputWire {
        Input { session: String, data_b64: String },
        Resize { session: String, cols: u16, rows: u16 },
    }

    pub type SessionLane = tokio::sync::mpsc::Sender<SessionJob>;

    /// guest-side registry: session id → the host node that owns its pty. Set when
    /// a remote create returns; read by the ws input handler to pick the forward
    /// lane over the (absent) local session. Arc<Mutex<..>> like the gateway's
    /// ws-token store.
    #[derive(Clone, Default)]
    pub struct RemoteSessions(std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, [u8; 32]>>>);
    impl RemoteSessions {
        pub fn remember(&self, session: String, host: [u8; 32]) { /* insert */ }
        pub fn host_of(&self, session: &str) -> Option<[u8; 32]> { /* get */ }
        pub fn forget(&self, session: &str) { /* remove on close */ }
    }
    ```
    (`CreatedSession` is the existing `crate::term::CreatedSession`.)
  - `NodeHandle::with_session_lane(lane: SessionLane) -> Self` + `pub(crate) fn session_lane(&self) -> Option<&SessionLane>`; `NodeHandle` also holds a `RemoteSessions` (always present, `Default`), exposed `pub(crate) fn remote_sessions(&self) -> &RemoteSessions`.
- Consumes: nothing (pure wire + wiring types).

- [ ] **Step 1: Read the exemplars end-to-end.** `gateway_plane.rs` client half (`:87-195`) + `proxy_remote` (`:273`) for the request/response shape; `gateway_http.rs` `GatewayJob` (`:34`) + `GatewayLane` (`:102`) + `with_gateway` (`:226`) for the channel wiring; `term_plane.rs` `accept_loop` (`:155`) + `write_frame`/`read_frame` (`:331`/`:348`) for the frame codec you REUSE. The new frames ride the identical length-prefixed-JSON codec — do not add a second codec.

- [ ] **Step 2: Write failing wire tests** in `term_plane.rs` `mod tests`, mirroring `frame_round_trips_both_event_types` (`:385`):

```rust
#[tokio::test]
async fn control_and_input_frames_round_trip() {
    let (mut a, mut b) = tokio::io::duplex(64 * 1024);
    let req = SessionControlRequest::Create {
        provider: "claude".into(), cred: "jess-fable-1".into(), cpu: Some(1), mem_gb: Some(2),
    };
    write_frame(&mut a, &req).await.unwrap();
    let got: SessionControlRequest = read_frame(&mut b).await.unwrap().unwrap();
    assert_eq!(got, req);

    let ev = SessionInputEvent::Resize { session: "00000000deadbeef".into(), cols: 120, rows: 40 };
    write_frame(&mut a, &ev).await.unwrap();
    let got: SessionInputEvent = read_frame(&mut b).await.unwrap().unwrap();
    assert_eq!(got, ev);
}

#[test]
fn intents_do_not_collide() {
    for i in [CHUNK_INTENT, COMMAND_INTENT, CONTROL_INTENT, INPUT_INTENT] {
        assert_eq!([CHUNK_INTENT, COMMAND_INTENT, CONTROL_INTENT, INPUT_INTENT]
            .iter().filter(|x| **x == i).count(), 1, "intent {i} is unique");
    }
}
```

And a `term_remote.rs` test that `RemoteSessions` remembers/forgets a binding.

- [ ] **Step 3: Run to verify failure.** `cargo test -p node control_and_input && cargo test -p noded remote_sessions` → FAIL (types absent).

- [ ] **Step 4: Implement.** Add the two intent consts + three structs to `term_plane.rs`; create `term_remote.rs` with `SessionJob`/`SessionInputWire`/`SessionLane`/`RemoteSessions`; add the module to `bin/noded/src/lib.rs` and re-export; extend `NodeHandle` with the lane field (`Option<SessionLane>`, `None` by default like `gateway`) + a `RemoteSessions` field (`Default`) + `with_session_lane`/`session_lane`/`remote_sessions`. Do NOT yet touch the accept loop or the HTTP routes — those are Tasks 4–5.

- [ ] **Step 5: Run + lint + commit.**
```bash
cargo test -p node control_and_input && cargo test -p noded remote_sessions
cargo clippy -p node --tests --no-deps && cargo clippy -p noded --tests --no-deps
git add bin/node/src/term_plane.rs bin/noded/src/term_remote.rs bin/noded/src/lib.rs bin/noded/src/handle.rs
git commit -m "feat(term): control + input intents, SessionJob lane, remote-session map"
```

---

### Task 2: Capability-host — resolved credential onto the interactive spawn's broker

**Files:**
- Modify: `crates/modules/system/capability-host/src/lib.rs` (`RunContext` `:232`, `start_broker` `:1031`, the headless caller `:3534`)
- Modify: `crates/modules/system/capability-host/src/interactive.rs` (`spawn_interactive_session` `:237` — the `start_broker` call)
- Modify: `crates/modules/system/capability-host/src/broker.rs` (`RunBroker::start_anthropic*` constructors — accept the per-run `AirlockConfig` override Phase 1 taught `resolve_anthropic_upstream` to prefer)
- Test: broker unit tests beside Phase 1's airlock tests

**Interfaces:**
- Consumes: Phase 1 Task 5's `AirlockConfig` (pub), `AirlockConfig::self_host`, and `resolve_anthropic_upstream`'s explicit-override precedence.
- Produces (Tasks 3–4 depend on these):
  - `RunContext.airlock: Option<broker::AirlockConfig>` (pub field; default `None`). When set, the interactive spawn's broker resolves the upstream to THIS config instead of `AirlockConfig::from_env()` — the per-run credential replaces the boundary env.
  - `RunBroker::start_anthropic_with(airlock: Option<AirlockConfig>)`, `..._for_podman_private_with`, `..._for_tart_with` — the existing zero-arg twins delegate with `None` (no behavior change for headless/env callers).
  - `CliProvider::start_broker` gains a param: `async fn start_broker(&self, airlock: Option<&AirlockConfig>)` — the `AnthropicMessages` arm forwards it to the `_with` constructor; the `CodexResponses` arm ignores it (codex airlock is Phase 1 Task 8's lane, out of scope here).

- [ ] **Step 1: Write a failing test** that an explicit `RunContext.airlock` reaches the interactive broker:

```rust
#[tokio::test]
async fn interactive_broker_uses_the_run_context_airlock_not_env() {
    // a testkit self-host gateway (Phase 1) with a distinct bearer; a RunContext
    // carrying AirlockConfig::self_host(&resolved) pinned to that gateway. Starting
    // the anthropic broker via start_broker(ctx.airlock.as_ref()) and issuing one
    // sealed round-trip must hit the RESOLVED gateway even with DUCKTAPE_AIRLOCK_* unset.
}
```

(Build it on Phase 1's `airlock_broker_uses_the_gateway_as_credential_source` fixture at `broker.rs:2472` — same mock upstream + testkit gateway, just supplied through `RunContext.airlock` rather than env.)

- [ ] **Step 2: Run to verify failure**, then implement. Add the `RunContext.airlock` field. In `interactive.rs:237` change `self.start_broker().await?` to `self.start_broker(ctx.airlock.as_ref()).await?`. In `lib.rs` `start_broker`, thread the param: the `AnthropicMessages` arm picks the `_with` constructor variant (tart / podman-private / plain) passing `airlock.cloned()`; the `CodexResponses` arm keeps today's zero-arg calls. In `broker.rs`, add the `_with` constructors that carry the override into the `RunBroker` so its per-request `resolve_anthropic_upstream` prefers it (Phase 1's precedence seam). Update the headless `start_broker` caller at `lib.rs:3534` to pass `ctx.airlock.as_ref()` (unifies both paths; `None` for every existing run).

- [ ] **Step 3: Run.** `cargo test -p capability-host --features <the airlock test features>` → PASS (new + Phase 1's).

- [ ] **Step 4: Lint + commit.**
```bash
cargo clippy -p capability-host --tests --no-deps
git add crates/modules/system/capability-host
git commit -m "feat(capability-host): per-run RunContext.airlock into the interactive broker"
```

---

### Task 3: Session manager — peer-attached create (forwarded output, creator binding, limits, airlock)

**Files:**
- Modify: `bin/noded/src/term.rs` (`Live` `:493`, `Inner` `:500`, `create`/`spawn` `:591`/`:618`, `spawn_pump` `:687`, and new accessors)
- Test: `term.rs` `mod tests` (creator binding + forward flag, without a live pty)

**Interfaces:**
- Consumes: Task 2's `RunContext.airlock` + `capability_host::AirlockConfig`.
- Produces (Task 4 depends on these):
  - `pub struct PeerAttach { pub creator_node: [u8; 32], pub airlock: capability_host::AirlockConfig, pub limits: std::collections::BTreeMap<String, u64> }`.
  - `pub async fn TerminalSessions::create_for_peer(&self, provider: &str, attach: PeerAttach) -> Result<CreatedSession, TermError>` — spawns the FULL solo TUI (`restricted = false`, raw-keystroke pty) but with output FORWARDING enabled (so `term_plane` fans it to the guest node) and the creator node recorded for the input gate. Reuses `create`'s slot-reservation discipline (`:591`).
  - `pub fn TerminalSessions::creator_node(&self, id: &str) -> Option<[u8; 32]>` — the host-side input gate reads this; `None` for a local (non-attached) session, so a forwarded input frame for a local session is refused.
  - `pub fn TerminalSessions::write_input(&self, id, bytes)` / `resize(&self, id, cols, rows)` are NOT new — the input lane calls the existing `session(id)` → `write_all`/`resize` (`term.rs:830`, `interactive.rs:131`/`:156`). No manager change for the write path itself.

- [ ] **Step 1: Write failing tests** (no live pty needed — assert the plumbing decisions):

```rust
#[test]
fn creator_node_is_recorded_for_a_peer_attach_and_absent_for_local() {
    // a manager with no providers can't spawn, but the binding accessors are pure:
    // assert creator_node("unknown") is None; and unit-check that PeerAttach carries
    // the fields (construction compiles) — the live spawn is covered by Task 6 e2e.
    let terminals = TerminalSessions::new(None, "node".into(),
        PathBuf::from("term-sessions"), TermRing::default(), TermCommandRing::default());
    assert!(terminals.creator_node("nope").is_none());
}
```

Plus a `spawn_pump` unit that a session created with `forward = true` publishes to the peer-forwarder feed (extend `output_ring_publishes_local_appends_but_stays_silent_on_remote` at `:1089` — the manager decides `forward`, the ring already distinguishes `append` vs `append_local_only`).

- [ ] **Step 2: Run to verify failure**, then implement:
  - `Live` gains `creator_node: Option<[u8; 32]>` and `forward: bool`. Local `create` (`:591`) sets both to their local defaults (`None`, `forward = mode == Shared`), unchanged behavior.
  - `create_for_peer`: reserve the slot like `create`; resolve the provider; build the `RunContext` exactly as `spawn` (`:629`) does but add `limits: attach.limits`, `airlock: Some(attach.airlock)`, and keep `portable: true`; spawn interactive with `restricted = false`; insert a `Live` with `creator_node = Some(attach.creator_node)`, `forward = true`; `spawn_pump` with `forward = true` (so output rings via `append`, not `append_local_only`).
  - `spawn_pump` (`:687`): drive its `forward` from the stored `Live.forward` rather than only `mode == Shared`. Extract the branch into a named predicate at the call site; no boolean threaded through the pump loop.
  - Add `creator_node(id)` accessor mirroring `mode(id)` (`:820`).

- [ ] **Step 3: Run.** `cargo test -p noded term` → PASS.

- [ ] **Step 4: Lint + commit.**
```bash
cargo clippy -p noded --tests --no-deps
git add bin/noded/src/term.rs
git commit -m "feat(noded): peer-attached terminal session — forwarded output, creator binding, airlock"
```

> `ponytail:` a peer-attached session's OUTPUT rides the existing all-peer `term_plane` fan-out (every mesh peer that subscribes `term:<id>` sees it), not guest-only. The security-critical direction (INPUT) is creator-gated in Task 4; per-guest output scoping is a follow-up. Session ids are 16 random hex, not enumerable, and the host reading the pty is a stated design non-goal — so this is a bounded, documented ceiling, not an open hole.

---

### Task 4: Host side — control handler (admission + spawn) and creator-gated input lane

**Files:**
- Modify: `bin/node/src/term_plane.rs` (`accept_loop` `:155-185` — route `CONTROL_INTENT`/`INPUT_INTENT`; new `serve_control`, `serve_create`, `serve_close`, `receive_input`; the plane gains `terminals: Option<TerminalSessions>` + `commands: mpsc::Sender<NodeCommand>` + the host's local gateway `via` URL, threaded through `spawn`)
- Modify: `bin/node/src/validator/mod.rs` (`term_plane::spawn` call `:244`) and `bin/node/src/replica/park.rs` (`:166`) — pass the new args
- Test: `term_plane.rs` unit tests (admission decision + creator gate, pure — no live pty)

**Interfaces:**
- Consumes: Task 1 wire types; Task 3 `create_for_peer`/`creator_node`; Phase 1 `gateway::{CredentialRecord, GatewayQuery::Credential, credential_use_allowed}` + `capability_host::{ResolvedCredential, AirlockConfig::self_host}`; the `account_of_node` identity-query pattern (`gateway_plane.rs:492`).
- Produces (Task 6 e2e depends on the refusal reasons):
  - Admission is a pure decision function so it is unit-testable without a pty:
    ```rust
    /// the host's create decision, given committed state already fetched. Returns
    /// Ok(ResolvedCredential + limits) to spawn, or a Refused reason.
    fn admit_create(
        provider: &str,
        creator_account: &[u8],
        record: Option<&gateway::CredentialRecord>,
        cpu: Option<u64>,
        mem_gb: Option<u64>,
        sandbox_present: bool,
    ) -> Result<AdmitOk, (&'static str, String)>;
    ```
    with the refusal reasons (snake_case tokens carried in `SessionControlReply::Refused.reason`): `no_sandbox`, `unknown_credential`, `credential_not_granted`, `provider_kind_mismatch`. (`at_capacity` and `unknown_provider` surface from `create_for_peer`'s existing `TermError` mapping — reason `at_capacity` / `unknown_provider`.)
- Note: the create request carries NO creator field. The host derives `creator_account` from the mesh-authenticated requesting peer node via `identity::IdentityQuery::OfNode` (the exact `account_of_node` pattern at `gateway_plane.rs:492`). This makes the creator cryptographic in every case where the creator runs on their own node (the lending case), per the spec.

- [ ] **Step 1: Write failing admission + gate tests** (pure, no pty):

```rust
fn rec(name: &str, owner: &[u8], grants: &[&[u8]], kind: gateway::CredentialKind) -> gateway::CredentialRecord { /* build */ }

#[test]
fn admit_gates_on_sandbox_credential_grant_and_kind() {
    let owner = b"owner-acct".to_vec();
    let grantee = b"grantee-acct".to_vec();
    let stranger = b"stranger".to_vec();
    let claude = rec("c1", &owner, &[&grantee], gateway::CredentialKind::Claude);

    // no sandbox → refused before any lookup
    assert_eq!(admit_create("claude", &owner, Some(&claude), None, None, false).unwrap_err().0, "no_sandbox");
    // unknown credential
    assert_eq!(admit_create("claude", &owner, None, None, None, true).unwrap_err().0, "unknown_credential");
    // owner is allowed; grantee is allowed; stranger is refused
    assert!(admit_create("claude", &owner, Some(&claude), None, None, true).is_ok());
    assert!(admit_create("claude", &grantee, Some(&claude), None, None, true).is_ok());
    assert_eq!(admit_create("claude", &stranger, Some(&claude), None, None, true).unwrap_err().0, "credential_not_granted");
    // explicit provider contradicting the cred's kind is refused
    assert_eq!(admit_create("codex", &owner, Some(&claude), None, None, true).unwrap_err().0, "provider_kind_mismatch");
}

#[test]
fn input_frame_is_accepted_only_from_the_creator_node() {
    // creator gate is pure: given a session→creator_node map and an arriving peer,
    // a frame from the creator is written, a frame from anyone else is dropped.
    assert!(input_permitted(Some([7u8; 32]), PeerId([7u8; 32])));
    assert!(!input_permitted(Some([7u8; 32]), PeerId([9u8; 32])));
    assert!(!input_permitted(None, PeerId([7u8; 32]))); // not an attached session
}
```

- [ ] **Step 2: Run to verify failure**, then implement:
  - **Thread state into the plane.** `term_plane::spawn` gains `terminals: Option<TerminalSessions>`, `commands: mpsc::Sender<NodeCommand>`, and `local_gateway_via: String` (the host's own gateway door base URL — the `via` for `ResolvedCredential`). Build the `TerminalSessions` in `boot/surfaces.rs` (as today, `:264`) and pass a clone into `run_validator`, which forwards it to `term_plane::spawn`; the `via` URL is the host's configured gateway base (reuse whatever `airlock_serve`/`gateway_routes` already knows as the loopback gateway base — the same value `DUCKTAPE_AIRLOCK_VIA` would name). A sync-only / joiner node passes `None` terminals → the control handler refuses with `no_sandbox`.
  - **Route the new intents.** In `accept_loop` (`:164`), the existing `matches!(intent, CHUNK_INTENT | COMMAND_INTENT)` guard extends to the two new intents. Keep the one dispatch shape: `match intent { CHUNK_INTENT => receive_chunks, COMMAND_INTENT => receive_commands, CONTROL_INTENT => serve_control, INPUT_INTENT => receive_input, _ => continue }`. `CONTROL` is NOT deduped per-peer (each create/close is its own short stream); `INPUT` is deduped per-peer like the existing feeds.
  - **`serve_control`** (mirror `gateway_plane.rs`'s `serve_proxy_stream` `:315`): read ONE `SessionControlRequest` frame, `match` on it — `Create` → `serve_create`, `Close` → `serve_close` — write ONE `SessionControlReply`, done. No loop.
  - **`serve_create`:** derive `creator_account` from `peer` via `account_of_node(&commands, &peer.0)` (copy `gateway_plane.rs:492`); query `gateway::GatewayQuery::Credential { name: cred }` via the `query(&commands, "gateway", ..)` helper (copy `gateway_plane.rs:973`); `admit_create(..)` → on `Err((reason, detail))` reply `Refused`; on `Ok` build the `ResolvedCredential` from the record + `local_gateway_via` (see the mapping below), `AirlockConfig::self_host(&resolved)`, a `limits` map from `cpu`/`mem_gb` (keys `cores`/`mem_gb`, matching `sandbox.rs:157-163`), then `terminals.create_for_peer(provider, PeerAttach { creator_node: peer.0, airlock, limits })`. Map `create_for_peer`'s `TermError` to the matching `Refused.reason`, or `Created { session, topic }` on success. `info!` `event = "session_created"` once, no secrets.
  - **The record → `ResolvedCredential` mapping** (bin/node owns it — capability-host must not depend on the gateway crate, per Phase 1 Task 5): `name = record.name`; `kind` maps `gateway::CredentialKind → capability_host::CredentialKind` (a 2-arm `match`); `seal_pk = record.seal_pk`; `via = local_gateway_via`; `authority` addresses the owner's `RouteName::named("airlock")` route under `record.owner_account` — the SAME coordinates `gateway_plane.rs` resolves a route by (`account_id`, `RouteName`), which is how the owner's co-hosted airlock (Phase 1 `surfaces.rs:122`) is reachable. If Phase 1 exposes a non-test `resolved_credential_from(record, via)` helper, reuse it; otherwise write this mapping in `term_plane.rs`.
  - **`serve_close`:** `terminals.close(session)` (existing, idempotent `:781`), reply `Closed`. (Only the host owns teardown; a close from a non-creator peer is harmless — it names a random 16-hex id it would have to already know, and the existing wall-clock + kill-on-drop backstops hold. Creator-binding the close is a named follow-up, not v1.)
  - **`receive_input`** (mirror `receive_chunks` `:187`): loop reading `SessionInputEvent` frames; for each, the creator gate `input_permitted(terminals.creator_node(&session), peer)` — mismatch/absent → `warn!` `reason = "input_not_creator"`, drop the frame, keep the stream; on pass, `Input` → `terminals.session(&session)?.write_all(&b64_decode(data_b64))`, `Resize` → `terminals.session(&session)?.resize(cols, rows)`. Never log `data_b64`.
  - Update `validator/mod.rs:244` and `replica/park.rs:166` call sites with the three new args.

- [ ] **Step 3: Run.** `cargo test -p node admit && cargo test -p node input_frame` → PASS.

- [ ] **Step 4: Lint + commit.**
```bash
cargo clippy -p node --tests --no-deps
git add bin/node/src/term_plane.rs bin/node/src/validator/mod.rs bin/node/src/replica/park.rs bin/node/src/boot/surfaces.rs
git commit -m "feat(node): host-side directed session create/close + creator-gated input lane"
```

---

### Task 5: Guest side — HTTP surface, client half, and input forwarding

**Files:**
- Modify: `bin/noded/src/term.rs` (`CreateSessionBody` `:935`, `create_session` `:947`, `close_session` `:986`)
- Modify: `bin/noded/src/stream.rs` (`handle_term_input` `:837`, `handle_term_resize` `:872` — forward remote sessions)
- Modify: `bin/node/src/term_plane.rs` (client half draining `SessionJob`, mirroring `gateway_plane.rs`'s job loop `:87-195`)
- Modify: `bin/node/src/boot/surfaces.rs` (create the `SessionLane` channel like the gateway lane `:145`, `http_handle.with_session_lane`), `bin/node/src/validator/mod.rs` (thread the receiver into `term_plane::spawn`)
- Test: `term.rs` create-route unit (remote vs local routing) + `stream.rs` forward-decision unit

**Interfaces:**
- Consumes: Task 1 `SessionJob`/`SessionLane`/`RemoteSessions`; Task 4's host-side control handler (the other end of the mesh stream).
- Produces (the excluded CLI attach client depends on this HTTP shape — see the final section):
  - `CreateSessionBody` extended (all new fields optional; today's `{agent, mode}` local path is untouched when they are absent):
    ```rust
    pub struct CreateSessionBody {
        pub agent: String,             // provider tag (claude|codex); required
        #[serde(default)] pub mode: SessionMode,
        #[serde(default)] pub node: Option<String>,   // hex host node key; None = this node (local path)
        #[serde(default)] pub cred: Option<String>,   // credential name; required when node is set
        #[serde(default)] pub cpu: Option<u64>,
        #[serde(default)] pub mem_gb: Option<u64>,
    }
    ```
  - `POST /v1/term/sessions` reply is unchanged (`CreatedSession { session_id, topic }`) for both local and remote. A remote create with no `cred` → `400 { "error": "a cross-node session requires --cred" }`. A `node` that is not 64 hex → `400 { "error": "node must be a 32-byte hex node key" }`. A host refusal surfaces the reason: `502 { "error": "host refused: <reason>: <detail>" }` (reasons from Task 4).

- [ ] **Step 1: Write failing tests:**
```rust
#[test]
fn create_body_routes_local_when_node_absent_and_remote_when_present() {
    // parse decision is pure: node=None → Local; node=Some(64hex)+cred=Some → Remote{host};
    // node=Some+cred=None → Err("requires --cred"); node="zz" → Err("32-byte hex").
}
```
Plus a `stream.rs` test that `forward_target(remote_sessions, session)` returns `Some(host)` for a remembered remote session and `None` for a local one.

- [ ] **Step 2: Run to verify failure**, then implement:
  - **`create_session` (`:947`):** parse `node`. Absent → today's path, but pass `agent` + `cred`/`cpu`/`mem_gb` into a LOCAL attach when `cred` is set (resolve the cred on THIS node — the transport-trivial own-node case — via the same host-side `serve_create` admission, reachable because a local create also goes through the `SessionLane` with `host = own_node`; the client half loopback-short-circuits like `gateway_plane.rs:113`). Absent `node` AND absent `cred` → the existing `terminals.create(agent, mode)`. Present `node` → require `cred` (else 400), decode the 64-hex host key (else 400), send `SessionJob::Create { host, provider: agent, cred, cpu, mem_gb, reply }` on `handle.session_lane()` (503 if unwired), await the oneshot; on `Ok(created)` call `handle.remote_sessions().remember(created.session_id.clone(), host)` and return it; on `Err(msg)` → 502.
  - **`handle_term_input`/`handle_term_resize` (`:837`/`:872`):** after the entitlement gate, check `handle.remote_sessions().host_of(session)`. `Some(host)` → send `SessionJob::Input { host, event: SessionInputWire::{Input|Resize} }` on the lane (the guest forwards; no local pty here). `None` → today's local path (`terminals.session` write/resize), unchanged. Never log `data`.
  - **`close_session` (`:986`):** if `handle.remote_sessions().host_of(id)` is `Some(host)`, send `SessionJob::Close { host, session: id }` and `forget(id)`; else today's local close.
  - **Client half in `term_plane.rs`** (mirror `gateway_plane.rs:87-195`): `term_plane::spawn` gains `mut jobs: tokio::sync::mpsc::Receiver<SessionJob>`. A task drains it: `Create` → if `host == me` loopback-serve via a local duplex into `serve_create` (single-node exercises the real frame path, `gateway_plane.rs:117`), else `service.open(PeerId(host), term_flow(), CONTROL_INTENT, Vec::new())`, write the `Create` request frame, read the `SessionControlReply`, resolve the oneshot (`Created` → `Ok`, `Refused` → `Err("<reason>: <detail>")`). `Close` → open `CONTROL`, write `Close`, read reply, ignore. `Input` → hold ONE persistent `INPUT` stream per host (open lazily, reopen on error like `send_peer` `:265`), write the `SessionInputEvent` frame. Reuse the file's `write_frame`/`read_frame`.
  - **Boot wiring:** `surfaces.rs` — `let (session_lane, session_requests) = tokio::sync::mpsc::channel::<SessionJob>(32);` (mirror `:145`), `http_handle.with_session_lane(session_lane)`, thread `session_requests` into `run_validator` → `term_plane::spawn`. A parked joiner / sync-only node: no `session_lane` on the handle (routes 503) and the plane still binds (output-only), matching today.

- [ ] **Step 3: Run.** `cargo test -p noded create_body && cargo test -p noded forward_target` → PASS.

- [ ] **Step 4: Lint + commit.**
```bash
cargo clippy -p node --tests --no-deps && cargo clippy -p noded --tests --no-deps
git add bin/node bin/noded
git commit -m "feat(term): guest-side remote create/close + input forwarding over the session lane"
```

---

### Task 6: Two-node e2e — directed create, forwarded input echo, close + reap

**Files:**
- Create: `bin/node/tests/remote_session.rs` (or extend the existing real-socket two-node cluster fixture — check `bin/node/tests/` first and reuse its cluster + commit-watch helpers, as Phase 1 Task 7 does)
- Test: itself

**Interfaces:**
- Consumes: everything above. A scripted child (`cat`, echoes stdin to stdout) stands in for a provider so the test needs no real API — the credential path is proven by Phase 1's e2e; THIS test proves directed create + the forwarded INPUT lane + output fan-out + creator gate.

- [ ] **Step 1: Write the test** against the existing two-node cluster fixture, event-driven throughout:

```rust
#[tokio::test]
async fn guest_drives_a_scripted_child_on_the_host_over_the_forwarded_lane() {
    let cluster = two_node_cluster().await;             // existing fixture (both Podman-capable, or a test provider whose interactive argv is `cat`)
    let (guest, host) = (cluster.node(0), cluster.node(1));

    // guest creates a session ON the host, naming a credential the guest owns
    // (seeded on the host's committed gateway state via the Phase 1 helpers).
    seed_owned_credential(&host, guest.account_id(), "guest-fable-1").await;
    cluster.wait_committed().await;                     // existing commit-watch, not a sleep
    let created = guest.http_create_session(CreateSessionBody {
        agent: "echo".into(), mode: SessionMode::Single,
        node: Some(hex(host.node_key())), cred: Some("guest-fable-1".into()),
        cpu: Some(1), mem_gb: Some(1),
    }).await.expect("remote create");

    // guest subscribes term:<id> on ITS OWN node and forwards a keystroke line;
    // the host writes it to the child's pty, the child echoes, output fans back.
    let mut out = guest.subscribe_term(&created.topic).await;   // ws, wakes on the ring watch
    guest.ws_term_input(&created.session_id, b"ping\n").await;  // ClientMsg::TermInput → forwarded
    let echoed = out.wait_for_chunk_containing(b"ping").await;  // event-driven: ring append, never a timer
    assert!(echoed);

    // a non-creator node's forged input frame is dropped host-side (creator gate).
    let stranger = cluster.node_outside_the_session();
    stranger.forge_input_frame(&created.session_id, b"HACK\n").await;
    assert!(!out.saw_chunk_containing(b"HACK").await_within_one_output_cycle());

    // close reaps the host container + releases the slot.
    guest.http_close_session(&created.session_id).await;
    assert!(host.wait_session_gone(&created.session_id).await); // watches the manager's finish, event-driven
}
```

Adjust helper names to the fixture's real API — the assertions + the event-driven waits (commit-watch, ring `watch`, `finish`) are the contract; no `sleep`.

- [ ] **Step 2: Run.** `cargo test -p node --test remote_session` → PASS. Rerun the full touched-crate suites: `cargo test -p node -p noded -p capability-host`.

- [ ] **Step 3: Lint + commit.**
```bash
git add bin/node/tests
git commit -m "test(node): two-node directed session + forwarded-input echo + creator gate e2e"
```

---

### Task 7: Live QA (manual, before merge)

- [ ] On this dev box (Podman available), with a Phase-1-registered credential: from the guest node's `/v1/term/sessions` (curl the HTTP shape below with `node` = the host's node key, `cred` = the name), confirm a real `claude` TUI spawns on the host, output streams to the guest's `term:<id>` ws, and a forwarded keystroke reaches the child. Document the exact curl + ws steps for the user (a real provider needs the live Anthropic round-trip Phase 1 Task 9 validates).
- [ ] Confirm the creator gate live: a second member on a third node subscribing `term:<id>` can READ output (stated non-goal) but a forged `INPUT` frame is dropped with `reason = "input_not_creator"` in the host's logs.
- [ ] Record the two-node WAN result (or note sim-lane coverage and defer WAN to a follow-up) in the PR body.

Merge policy per repo rules: high confidence + green gates → PR(s) to `dev`. Natural PR split: Tasks 1–2 (wire + broker seam), 3–5 (host + guest planes), 6 (e2e). Stack them if review size demands.

---

## Interfaces for the CLI attach builder (`ducktape agent pty` — OUT of Phase 2 scope)

A separate builder writes the CLI client that drives this plumbing. It talks ONLY to its own node — no cross-node dialing (the node does the mesh). The exact contracts:

- **Create (HTTP, guest's own node):** `POST /v1/term/sessions`, body `CreateSessionBody` (Task 5):
  `{ "agent": "<claude|codex>", "mode": "single", "node": "<64-hex host node key>", "cred": "<name>", "cpu": <u64?>, "mem_gb": <u64?> }`.
  Reply `200 { "sessionId": "<16 hex>", "topic": "term:<id>" }`. The CLI resolves `--node <display-name>` → node key itself via `identity::IdentityQuery::{OfMember, Get}` + `AccountView.display_name` (`crates/modules/system/identity/src/interface.rs:177-196`), erroring with the candidate node keys when an account operates more than one node; `--node` also accepts a raw hex node key. `--cred`'s kind decides the provider when `agent`/`<provider>` is omitted; an explicit provider contradicting the cred is the host's `provider_kind_mismatch` refusal.
  Error strings the CLI surfaces verbatim: `a cross-node session requires --cred` (400), `node must be a 32-byte hex node key` (400), `host refused: <reason>: <detail>` (502, reasons: `no_sandbox`, `unknown_credential`, `credential_not_granted`, `provider_kind_mismatch`, `at_capacity`, `unknown_provider`), `terminal sessions are not enabled on this node` (503).
- **Attach + drive (ws, guest's own node):** subscribe the ws to `topic` (`term:<id>`); output arrives as `ServerFrame::TermChunk { topic, cursor, item }` where `item` is base64 of the pty bytes (`bin/noded/src/stream.rs:113`). Send keystrokes as `ClientMsg::TermInput { session, data }` (`data` = base64 raw bytes, `:52`) and window changes as `ClientMsg::TermResize { session, cols, rows }` (`:58`). The node forwards both to the host over the `INPUT` lane; there is NO CLI-visible difference between a local and a remote session. Subscription to `term:<id>` is the entitlement gate — send it before any input.
- **Close (HTTP, guest's own node):** `POST /v1/term/sessions/{id}/close` → `204` (idempotent). Send it on unmount / SIGINT; the host's 4 h wall-clock + kill-on-drop are the backstops if it never arrives.
- **Raw terminal mode (no new dependency):** `libc` is already a workspace dep. The CLI puts the local tty in raw mode with `libc::tcgetattr` (save the original `termios`), `libc::cfmakeraw` on a copy, `libc::tcsetattr(fd, TCSANOW, &raw)`, and RESTORES the saved `termios` on exit (including panic — a drop guard). Poll the tty for the initial `cols`/`rows` with `libc::ioctl(fd, TIOCGWINSZ, &winsize)` and re-send `TermResize` on `SIGWINCH`. No `crossterm`/`termion` dependency is warranted for this.
