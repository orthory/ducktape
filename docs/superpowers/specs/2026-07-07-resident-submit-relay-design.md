# Resident Submit Relay — Design

Date: 2026-07-07
Status: approved direction (user), spec for implementation
Naming: the tier shipped as "resident" (renamed from "observer", user decision 2026-07-07).
Branch: `feat/observer-submit-relay` (base `origin/dev`)

## Problem

A resident-standing node (the staged-admission tier) is a real member of the
product: it follows boundaries, serves reads locally, and its node key is the
user's chat identity. But both of its local surfaces hard-refuse writes:

- rpc: `RpcRequest::Submit` → "resident standing serves reads only" (`bin/node/src/main.rs` park-loop serve window)
- app surface: `noded::NodeCommand::Submit` → same refusal

So a user whose node holds resident standing can read chat but never post.
Product-wise that is wrong: the resident tier is exactly the "member on a
laptop" seat — flaky machines must not sit in the validator set (quorum(n)=n
for n≤3, so one sleeping laptop halts writes for everyone), yet the human
behind one still needs to talk.

## Insight the design leans on

Authorship is already decoupled from consensus injection:

1. A frame is `(origin, seq, msg, sig)`; the signature binds
   `(origin, seq, target, payload)` to the origin key
   (`crates/kernel/node/src/lib.rs`, `encode_frame`/`decode_frame`). The kernel
   never requires `origin ∈ valset`; `decode_frame` yields
   `Origin::External(pubkey)` for any valid ed25519 signer.
2. The consensus lane cares only about **custody** — which node pinned and
   proposed the frame ("custody, not origin, gates it", cutover carry doc).
   Byte-identical duplicates collapse in the exactly-once digest gate.
3. Modules derive authorship from `ctx.env().origin` and enforce their own
   policy deterministically: chat gates on channel membership (not valset);
   governance/valset ops are member-gated and deterministically reject a
   non-member origin. Relaying therefore grants a resident no authority
   beyond what modules already grant its key.

So the only missing piece is transport: carry a resident-signed frame to a
validator that will take custody.

## Approaches considered

**A. Mesh relay lane (chosen).** New static discovery channel; the resident
signs the frame with its own identity key and ships the bytes to a current
validator; the validator verifies, gates on committed resident standing, and
injects via a new kernel `submit_frame` (pin + propose + custody). Reply rides
the same channel when the frame drains.
— Pros: authenticated end-to-end transport already exists (the resident is on
the mesh); authorship stays cryptographically the resident's; custody/carry
and the digest dedup gate come for free; no consensus or module changes.
— Cons: one new wire surface to maintain.

**B. Extend the json-lines RPC with a `SubmitSigned`.** Resident POSTs a
pre-signed frame to a validator's rpc listener.
— Rejected: `rpc_listen` is typically loopback/private (sentry deployments
explicitly keep validator surfaces unreachable); the mesh path is the only
transport guaranteed to exist, and it authenticates peers.

**C. Seat residents in consensus (zero-weight or full).**
— Rejected: exactly what the resident tier exists to avoid — quorum(n)=n for
n≤3 means flaky members cost liveness; a "zero-weight member" is a large
consensus change for no transport gain.

## Design (approach A)

### Wire

- `const CHANNEL_SUBMIT_RELAY: u64 = 3;` — the last free static slot below
  `CHANNEL_STATE_SYNC = 4` (engine banks start at 8; 0–2 stay free). Like
  every static lane it must be **registered in every mode** (unregistered
  channels kill the sender's connection) and black-holed where unserved.
- `bin/node/src/relay.rs`, modeled on `lobby.rs` (json on the wire; this lane
  is low-volume — a human posting chat messages):

```rust
pub enum RelayMsg {
    /// a resident-signed frame, bytes exactly as `encode_frame` produced.
    Submit { frame: Vec<u8> },
    /// the validator's answer, keyed by the frame's content address.
    Reply { frame_id: [u8; 32], outcome: RelayOutcome },
}
pub enum RelayOutcome {
    /// drained Applied at `height` with the block's `app_hash`.
    Applied { height: u64, app_hash: String },
    /// finalized but deterministically rejected.
    Rejected { detail: String },
    /// refused at the door (bad frame / origin lacks resident standing) or
    /// the hold expired before finalization.
    Refused { detail: String },
}
```

### Kernel: `OrderedNode::submit_frame`

`submit` today signs locally then pins/proposes/tracks. Split it:

```rust
pub async fn submit(&mut self, signer, seq, msg) -> Result<FrameId, Error> {
    let frame = encode_frame(signer, seq, &msg);
    self.submit_frame(frame).await
}
/// take custody of an ALREADY-SIGNED frame: verify it decodes (signature
/// binds origin/seq/target/payload), pin, propose, track outstanding.
pub async fn submit_frame(&mut self, frame: Vec<u8>) -> Result<FrameId, Error>
```

`submit_frame` calls `decode_frame` first — junk or a bad signature errors
before anything is pinned. Custody semantics (outstanding map, cutover carry,
digest dedup) are unchanged — a relayed frame is carried across epochs exactly
like a local one.

### Validator side (pump loop)

Register `CHANNEL_SUBMIT_RELAY`; add a select arm:

1. decode `RelayMsg::Submit` — junk drops silently (lobby idiom);
2. `decode_frame` the bytes — refusal on error (the signature must bind
   `(origin, seq, target, payload)` to the origin key; forgery is impossible);
3. **origin ∈ committed resident standing** (`read_valset_residents` on this
   node's host) — validators submit locally and parked joiners have no
   standing; refusal otherwise. the sending PEER is deliberately not consulted:
   residents ride the network's derived lobby transport identity (derivable by
   any invite holder), so an `origin == peer` check could never pass for a real
   resident and would gate nothing — the frame signature is the authorization
   and the exactly-once digest gate collapses byte-identical replays;
4. `node.submit_frame(frame)` → `FrameId`; record in a new
   `pending_relays: HashMap<FrameId, (peer, deadline)>` with the same
   `SUBMIT_HOLD` budget as local submits.

The drain arm that resolves `pending_submits` also resolves `pending_relays`:
Applied → `Reply{Applied{height, app_hash}}` (the per-block boundary hash, as
for local holds), Rejected → `Reply{Rejected}`, expiry → `Reply{Refused}`.
Discards stay held — the cutover carry keeps the FrameId alive.

### Resident side (park-loop serve window)

Register the channel in the joiner path (and black-hole it in sync-only mode;
the validator path serves it). Replace both write refusals:

- Gate: writes require `resident_standing && serving.is_some()`; a parked
  joiner without standing keeps today's refusal, a pre-first-sync resident
  answers "no boundary yet — retry".
- On submit: build `Msg`, `seq = persisted counter++`,
  `frame = node::encode_frame(&signer, seq, &msg)` (the node identity key —
  the same key that is the user's chat identity, `status.publicKey`),
  `frame_id = node::frame_id(&frame)`; send `RelayMsg::Submit` to ONE current
  validator (round-robin over the manifest's participants, the announce
  idiom); hold the caller's reply in
  `pending_relayed: HashMap<FrameId, (ReplySlot, Instant)>` where `ReplySlot`
  is the rpc reply sender or the app-surface oneshot.
- On `RelayMsg::Reply`: resolve the slot — `Applied` maps to `RpcReply::ok()`
  / `Ok(BlockSummary{height, app_hash})`; `Rejected`/`Refused` map to the
  error path verbatim.
- Expiry: swept on the serve-window tick with the `SUBMIT_HOLD` budget — the
  validator's own hold, not a longer one. A larger budget buys nothing: the
  rpc bridge (`spawn_rpc_listener`) caps every request at 10s = `SUBMIT_HOLD`,
  so a hold that outlives the caller's own timeout only leaks memory. A sweep
  that races the bridge's timeout reads as a stuck node — exactly as it does
  on a validator, the same accepted behavior. The message mirrors the
  validator's truthful timeout — the op may still land, clients re-query on
  block events.
- v1 keeps ONE target per submit (no auto-retry): a byte-identical re-send is
  safe (digest gate) but the client's re-submit already provides it. The
  round-robin spreads load across validators between submits.

### Resident submit sequence

Per-origin `seq` is persisted at `<storage>/relay-submit-seq` (a plain u64,
bumped before use). The kernel does not require per-origin monotonicity —
a lost counter file restarts at 0 and same-`(origin, seq)` frames with
different payloads are distinct digests that both apply — so this is hygiene,
not safety. The pre-existing accepted edge ("a REJOINING key resubmitting a
byte-identical (seq, msg)") is unchanged and stays documented at the validator
submit site; per-origin replay nonces remain a roadmap item.

### Mixed versions / upgrade gating

None needed. The relay is node-binary capability, not consensus semantics: a
resident-origin frame applies identically on every binary that can apply
frames at all (authorship-from-origin is original kernel behavior). An old
binary would kill a connection that speaks channel 3 at it — a mesh-level
flag-day, in line with the no-backwards-compat policy; nothing can fork.
`MAX_PROTOCOL_VERSION` stays 3.

### What does NOT change

- Parked joiners without standing: still read-and-write-refused.
- Sync-only mode: channel black-holed.
- Sentry: still pure path.
- Validator local submits, chat policy, governance member-gates: untouched.
- No proxying of arbitrary claimed origins: the app-surface `origin` field
  stays ignored; the signed frame origin is the only authorship.

## Error handling summary

| failure | behavior |
|---|---|
| junk on the relay channel | dropped (validator), like the lobby doorbell |
| bad signature / undecodable frame | `Refused` reply, nothing pinned |
| sending peer ≠ origin | not a failure — residents ride the shared, invite-derivable lobby transport identity, so the peer is never consulted; the frame signature + committed resident standing are the whole gate |
| origin lacks committed resident standing | `Refused` |
| validator dies after accepting | frame is pinned + carried by that validator's custody; if it never finalizes, the resident's hold expires honestly; client re-submit produces a byte-identical or fresh frame — both safe |
| relay reply lost | resident hold expires honestly; the op may still have landed — the app re-queries on block events (same contract as a validator timeout) |
| resident restarts mid-hold | in-memory holds die like a validator's; the frame remains under the validator's custody |

## Testing

1. **Kernel unit** (`crates/kernel/node`): `submit_frame` takes custody of a
   pre-signed frame (pin + outstanding + carried across `cutover`); a
   tampered/bad-signature frame errors without pinning; `submit` still equals
   sign + `submit_frame`.
2. **Relay codec + door unit** (`bin/node`, in-module tests like `lobby.rs`):
   round-trip encode/decode; outcome mapping; and the pure door check
   `verify_relay_submit` — a standing origin's frame is accepted, a frame whose
   origin holds no committed resident standing is refused, and a
   signature-tampered frame that still parses as json is refused at the
   signature (not the json parser).
3. **E2E** (`bin/node/tests/resident_submit_e2e.rs`, on the
   `live_admission_e2e` harness which already grants resident standing):
   - resident with standing submits a chat post through its app surface →
     reply carries `Applied{height}`; a validator's query shows the message
     with `author == resident key`; the resident's own read surface shows it
     after the next boundary follow.
   - a parked joiner WITHOUT standing gets the not-a-validator refusal.
   - (a frame whose origin holds no committed resident standing is refused at
     the door — now covered by the `verify_relay_submit` unit test in §2, not a
     separate e2e; there is no origin≠peer gate to exercise.)
   - member-gated module op (e.g. a governance proposal) from a resident
     finalizes Rejected — proving relay grants no authority.

## Open items deliberately out of scope

- Multi-validator retry/fan-out for a single submit (client re-submit covers).
- Per-origin replay nonces in app state (existing roadmap item).
- User-level keys distinct from node identity (the console custodian model is
  unchanged).
- Desktop app UX: no change needed — the app already talks the same surface
  and reads `status.publicKey`.
