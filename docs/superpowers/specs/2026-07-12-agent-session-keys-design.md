# Host-owned agent session signer — consensus ACL + provenance

Date: 2026-07-12
Status: accepted; amended 2026-07-15 to keep the private key host-side
Stacks on: `feat/agent-mcp-plane` (PR #423)

## The defect this closes

PR #423 gave agents a tool plane, gated **host-side** in `ducktape-mcp`, submitting
through the frameless `/v1/submit` lane with `origin` set to the agent's owner.

**On a real node, that origin is discarded.**

```rust
// bin/node/src/validator/run/ingress.rs
noded::NodeCommand::Submit { target, payload, origin: _, reply } => {
    let frame = node::encode_frame(signer, seq, &Msg { target, payload });
```

`bin/node` — the binary the desktop actually spawns (`ducktape-node`) — throws the
caller's origin away and signs the frame with its **own node key**. Only
`bin/noded`, the embedded dev daemon, honours the origin string. #423's e2e used an
in-process actor of the `noded` shape, so it validated a code path production never
runs: the test passed and the attribution was wrong anyway.

The consequences, in order of severity:

1. **No audit trail.** An agent's write is a node-key op, byte-indistinguishable
   from one the human made at the keyboard. There is no way to ask "what did this
   agent do?"
2. **Wrong human, cross-node.** The op resolves through `identity` to the account
   owning the *executing* node, not the account owning the *agent*. Same-node runs
   land right only by accident.
3. **The ACL is advisory.** `allowed_actions` and `ResourceCaps` are checked in a
   host binary. Consensus never sees a claim it could refuse.

## What already exists (and why this is smaller than it looks)

Three signed artifacts are already in the tree. This design invents no cryptography.

| Layer | Authentication | Notes |
|---|---|---|
| **Op frames** (`crates/kernel/node`) | ed25519 over `FRAME_NS`; `decode_frame` binds `(origin, seq, target, payload)` | **Every validator verifies.** The codec's own doc: *"every honest validator rejects the identical forged frame identically. the verified `origin` becomes the block's root `Origin::External(pubkey)` — authorship a module can trust."* |
| **Account certs** (`identity::MemberAuth`) | member key signs a domain-namespaced, chain+nonce-scoped preimage | Nonce bump kills stale certs. |
| **Frameless `/v1/submit`** | none — caller-supplied string | Honoured by `bin/noded`, **discarded** by `bin/node`. |

A frame's origin *is* its verified public key. That is the entire mechanism this
design needs.

### The owner's delegation is already on-chain

The tempting move is a new owner-signed session certificate. It is **redundant**.
`AgentRecord { owner, allowed_actions, caps }` **is** the owner's committed
capability grant — registering an agent with `chat.post` *is* the act of
authorising it. Consensus already holds the authority.

What is missing is not authority but **proof that an op came from that agent's
run**. That is all a session key supplies.

## Design

```
 provisioner (the assignee node)              consensus
 ┌───────────────────────────────┐
 │ mint ephemeral ed25519        │
 │   session keypair, per run    │
 │                               │   RunsMsg::OpenAgentSession
 │ submit (signed, NODE key) ────┼──▶  { run_id, session_key }
 │                               │      runs: origin == the run's committed
 │                               │            lease-holder?  → bind
 │ host-only scoped signer       │
 │ accepts AgentAction or        │
 │ DelegateRun for this run      │
 └──────────────┬────────────────┘
                │ random URL token, never the private key
        ┌───────▼────────┐            RunsMsg::{AgentAction, DelegateRun}
        │  ducktape-mcp  │──typed──────▶ host signs with session key
        │                │  request       runs: origin == the bound session key?
        └────────────────┘               ∧ run in-flight
                                         ∧ action ∈ allowed_actions
                                         ∧ caps permit
                                              → emit as MODULE origin
                                              → AuthorRef::Agent
```

1. **Mint.** The provisioner generates a fresh ed25519 keypair per run and keeps
   its private half in the host process. The MCP child receives only a random
   token for a host endpoint that accepts two message shapes for the exact
   run id: `AgentAction` and `DelegateRun`. It receives neither the session key
   nor the node key.

2. **Bind.** The node submits `RunsMsg::OpenAgentSession { run_id, session_key }`
   as a frame signed with its **node key**. `runs` validates that the origin is
   the run's committed lease-holder — reachable in one hop via
   `DispatchQuery::Dispatch` → `DispatchView.assignee`, which the read facade
   resolves from saga's committed lease. Self-authorising: no owner interaction,
   so automated issue-mention runs work, and it is correct cross-node.

3. **Act or call.** The MCP posts a typed Runs message to the scoped endpoint.
   The host checks the token, message variant, and exact run id, signs the frame
   with the private session key, and submits it on the existing frame lane.

4. **Enforce, in consensus.** `runs` checks: origin *is* the session key bound to
   that run ∧ the run is still in-flight ∧ the action is in the agent's
   `allowed_actions` ∧ its caps permit it — **reusing `response.rs`'s validator
   and `pages_effects.rs`'s cap gate verbatim**. There is one definition of "what
   an agent may do", not two.

5. **Emit.** Validated actions become **module-origin** follow-ups, exactly like
   the response path's. Chat and pages accept `as_agent` from a module origin, so
   posts land as **`AuthorRef::Agent { module: "runs", agent_id }`** — the correct
   attribution, which #423 could not produce at all.

6. **Close.** The session is pruned when the run settles. Actions per session are
   capped (`MAX_ACTIONS_PER_SESSION`), mirroring `MAX_ACTIONS_PER_RUN`'s
   blast-radius bound.

### Why the session key stays host-side

A raw ed25519 key is a general frame signer, even if Runs only recognizes it as
an agent session for one lane. A child that recovered it could address another
module directly. The scoped endpoint token fixes that mismatch: the host will
sign only the two run-scoped Runs messages above and drops the endpoint with the
workspace. The consensus gate still decides the actual action/call authority.

### Deterministic ids

Mid-run actions mint chat/task/pages ids exactly as the settle path does —
derived from `(run_id, action_seq)` where `action_seq` is the session's committed
action counter. Every validator replays identical ids. No host randomness enters
consensus.

## The honest limit

Under `claude` (no sandbox) the agent process runs as the user. It can read
`user.key` off the disk and submit as the owner through the frameless lane, and
no node-side design can prevent that — `origin_guard` says as much. What the
session key changes:

- Agent actions become **unforgeable and attributable**: an op signed by the
  session key provably came from that run, and consensus refuses it if it exceeds
  the grant.
- Under **codex** (network-less sandbox) the MCP is the only door, so the ACL is a
  **complete** boundary.
- Under **claude** it is a complete boundary *for the honest path* and an audit
  trail for everything the agent does through it — but a **malicious** agent can
  still act as the human out-of-band.

Closing that last gap requires sandboxing the claude runner (deny network), not
more consensus machinery. Deliberately **not** in this scope; recorded here so
nobody mistakes what shipped.

## Wire additions

```rust
// runs::interface
pub const MAX_ACTIONS_PER_SESSION: u32 = 32;
pub const SESSION_KEY_LEN: usize = 32; // ed25519 public key

pub enum RunsMsg {
    // ...existing...
    /// the EXECUTING node binds an ephemeral session key to a live run.
    /// origin MUST be the run's committed lease-holder.
    OpenAgentSession { run_id: String, session_key: Vec<u8> },
    /// one agent action, mid-run. origin MUST BE the bound session key.
    AgentAction { run_id: String, action: AgentAction },
}

pub struct AgentSession {
    pub run_id: String,
    pub agent_id: String,
    pub session_key: Vec<u8>,
    pub opened_at: u64,
    pub actions: u32,   // the audit counter AND the deterministic id salt
}

pub enum RunsQuery {
    // ...existing...
    AgentSessions,      // the audit surface: who is acting right now
}
```

The existing `POST /v1/submit/frame` route remains the node's authenticated
frame ingress. Children do not receive the key needed to use it. The
provisioner instead exposes a per-run `POST /v1/run-action` endpoint that
accepts only typed `AgentAction` and `DelegateRun` requests for its bound run,
then signs and submits the frame inside the trusted host process.

New env: `DUCKTAPE_RUN_ACTION_URL`, `DUCKTAPE_RUN_ACTION_TOKEN`, and
`DUCKTAPE_RUN_ID`. `DUCKTAPE_RUN_SESSION_KEY` is deliberately absent.

## Flag day

New `runs` state (the session registry) and two new ops move the app-hash.
Pre-production, so a re-genesis is acceptable — but it IS a flag day and must be
called out in the PR.

## Testing

- **runs (unit):** a non-assignee cannot open a session; an origin that is not the
  bound session key cannot act; an action outside `allowed_actions` is refused; a
  caps-denied `pages.comment` is refused; a settled run's session is gone; the
  action cap bounds a session; ids are deterministic across replay.
- **frame ingress:** a tampered frame is refused; a frame's verified pubkey — not
  any caller string — becomes the origin, **on both binaries**.
- **mcp e2e:** writes and peer calls carry only typed run-scoped input; no raw
  key or caller-selected identity/authority crosses the child boundary.
- **provisioner boundary:** the endpoint rejects every other message and run id,
  and frames accepted requests with the public key consensus bound.
- **live:** the existing `live_runner` test, re-pointed at the session lane.
